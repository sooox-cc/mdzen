//! # Application Module
//!
//! This module contains the main application logic for mdzen,
//! including the GUI state management, file operations, and user interactions.

use crate::markdown::MarkdownRenderer;
use egui::*;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Main application state for the markdown reader.
///
/// This struct holds all the state needed for the application including:
/// - Current file and content
/// - UI state (search, TOC, wide mode)
/// - Caches for images and search results
/// - Font and display settings
pub struct MarkdownReaderApp {
    /// Renderer for processing and displaying markdown content
    markdown_renderer: MarkdownRenderer,
    /// Path to the currently loaded file
    current_file: Option<PathBuf>,
    /// Raw markdown content of the current file
    content: String,
    /// Whether the file open dialog should be shown
    show_open_dialog: bool,
    /// Current font size for text rendering
    font_size: f32,
    /// Whether wide mode is enabled (less side padding)
    wide_mode: bool,
    /// Whether the search bar is visible
    show_search: bool,
    /// Current search query text
    search_query: String,
    /// Results from the last search operation
    search_results: Vec<SearchResult>,
    /// Index of the currently selected search result
    current_search_index: usize,
    /// Whether search should be case sensitive
    search_case_sensitive: bool,
    /// Cache for loaded images to avoid reloading
    image_cache: HashMap<String, Result<egui::TextureHandle, String>>,
    /// Whether the table of contents sidebar is visible
    show_toc: bool,
    /// List of headers for the table of contents
    toc_headers: Vec<TocHeader>,
    /// Header to scroll to (if any)
    scroll_to_header: Option<String>,
}

/// Represents a header in the table of contents.
#[derive(Debug, Clone)]
pub struct TocHeader {
    /// Heading level (1-6 for H1-H6)
    pub level: u8,
    /// Text content of the header
    pub title: String,
    /// Line number where the header appears (reserved for future use)
    #[allow(dead_code)]
    pub line_number: usize,
}

/// Represents a search result within the document.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Line number where the match was found
    #[allow(dead_code)]
    pub line_number: usize,
    /// Full content of the line containing the match
    #[allow(dead_code)]
    pub line_content: String,
    /// Character index where the match starts in the line
    #[allow(dead_code)]
    pub match_start: usize,
    /// Character index where the match ends in the line
    #[allow(dead_code)]
    pub match_end: usize,
}

impl Default for MarkdownReaderApp {
    fn default() -> Self {
        Self {
            markdown_renderer: MarkdownRenderer::new(),
            current_file: None,
            content: String::new(),
            show_open_dialog: false,
            font_size: 14.0,
            wide_mode: false,
            show_search: false,
            search_query: String::new(),
            search_results: Vec::new(),
            current_search_index: 0,
            search_case_sensitive: false,
            image_cache: HashMap::new(),
            show_toc: false,
            toc_headers: Vec::new(),
            scroll_to_header: None,
        }
    }
}

impl MarkdownReaderApp {
    /// Creates a new markdown reader application with custom visuals.
    ///
    /// Sets up dark theme colors optimized for readability and initializes
    /// the markdown renderer with the default font size.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Set up nice visuals for better readability
        let mut visuals = egui::Visuals::dark();
        visuals.window_fill = egui::Color32::from_rgb(40, 44, 52);
        visuals.panel_fill = egui::Color32::from_rgb(40, 44, 52);
        visuals.extreme_bg_color = egui::Color32::from_rgb(33, 37, 43);
        visuals.code_bg_color = egui::Color32::from_rgb(33, 37, 43);
        visuals.override_text_color = Some(egui::Color32::from_rgb(171, 178, 191));
        cc.egui_ctx.set_visuals(visuals);

        let mut app = Self::default();
        app.markdown_renderer.set_font_size(app.font_size);
        app
    }

    /// Loads a markdown file from the given path.
    ///
    /// Reads the file content, clears caches, and regenerates the table of contents.
    /// Returns an error if the file cannot be read.
    pub fn load_file(&mut self, path: PathBuf) -> anyhow::Result<()> {
        let content = fs::read_to_string(&path)?;
        self.content = content;
        self.current_file = Some(path);
        self.image_cache.clear(); // Clear cache when loading new file
        self.search_results.clear();
        self.current_search_index = 0;
        self.generate_toc(); // Generate TOC when loading new file
        Ok(())
    }

    /// Generates the table of contents by parsing markdown headers.
    ///
    /// Scans through the document content and extracts all heading elements
    /// to populate the TOC sidebar.
    pub fn generate_toc(&mut self) {
        use pulldown_cmark::{Event, Parser, Tag, TagEnd};

        self.toc_headers.clear();
        let parser = Parser::new(&self.content);
        let mut current_header: Option<(u8, String)> = None;
        let mut line_number = 0;

        for event in parser {
            match event {
                Event::Start(Tag::Heading { level, .. }) => {
                    let level_num = match level {
                        pulldown_cmark::HeadingLevel::H1 => 1,
                        pulldown_cmark::HeadingLevel::H2 => 2,
                        pulldown_cmark::HeadingLevel::H3 => 3,
                        pulldown_cmark::HeadingLevel::H4 => 4,
                        pulldown_cmark::HeadingLevel::H5 => 5,
                        pulldown_cmark::HeadingLevel::H6 => 6,
                    };
                    current_header = Some((level_num, String::new()));
                }
                Event::End(TagEnd::Heading(_)) => {
                    if let Some((level, title)) = current_header.take() {
                        if !title.trim().is_empty() {
                            self.toc_headers.push(TocHeader {
                                level,
                                title: title.trim().to_string(),
                                line_number,
                            });
                        }
                    }
                }
                Event::Text(text) => {
                    if let Some((_, ref mut title)) = current_header {
                        title.push_str(&text);
                    }
                }
                Event::Code(text) => {
                    if let Some((_, ref mut title)) = current_header {
                        title.push_str(&text);
                    }
                }
                Event::SoftBreak | Event::HardBreak => {
                    line_number += 1;
                }
                _ => {}
            }
        }
    }

    /// Performs a text search through the document content.
    ///
    /// Searches for the current query string in all lines of the document,
    /// respecting case sensitivity settings. Updates the search results list.
    pub fn perform_search(&mut self) {
        self.search_results.clear();
        self.current_search_index = 0;

        if self.search_query.is_empty() {
            return;
        }

        let query = if self.search_case_sensitive {
            self.search_query.clone()
        } else {
            self.search_query.to_lowercase()
        };

        for (line_number, line) in self.content.lines().enumerate() {
            let search_line = if self.search_case_sensitive {
                line.to_string()
            } else {
                line.to_lowercase()
            };

            let mut start = 0;
            while let Some(pos) = search_line[start..].find(&query) {
                let match_start = start + pos;
                let match_end = match_start + query.len();

                self.search_results.push(SearchResult {
                    line_number,
                    line_content: line.to_string(),
                    match_start,
                    match_end,
                });

                start = match_end;
            }
        }
    }

    /// Moves to the next search result in the list.
    pub fn next_search_result(&mut self) {
        if !self.search_results.is_empty() {
            self.current_search_index = (self.current_search_index + 1) % self.search_results.len();
        }
    }

    /// Moves to the previous search result in the list.
    pub fn previous_search_result(&mut self) {
        if !self.search_results.is_empty() {
            self.current_search_index = if self.current_search_index == 0 {
                self.search_results.len() - 1
            } else {
                self.current_search_index - 1
            };
        }
    }

    fn show_menu_bar(&mut self, ctx: &Context) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open").clicked() {
                        self.show_open_dialog = true;
                        ui.close_menu();
                    }
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.menu_button("View", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Font Size:");
                        if ui.button("➖").clicked() {
                            self.font_size = (self.font_size - 2.0).max(8.0);
                            self.markdown_renderer.set_font_size(self.font_size);
                        }
                        ui.label(format!("{:.0}", self.font_size));
                        if ui.button("➕").clicked() {
                            self.font_size = (self.font_size + 2.0).min(32.0);
                            self.markdown_renderer.set_font_size(self.font_size);
                        }
                    });
                    ui.separator();
                    if ui
                        .button(if self.wide_mode {
                            "Normal Width"
                        } else {
                            "Wide Mode"
                        })
                        .clicked()
                    {
                        self.wide_mode = !self.wide_mode;
                    }
                    if ui
                        .button(if self.show_toc {
                            "Hide TOC"
                        } else {
                            "Show TOC"
                        })
                        .clicked()
                    {
                        self.show_toc = !self.show_toc;
                    }
                });

                ui.menu_button("Edit", |ui| {
                    if ui.button("Copy as Markdown").clicked() {
                        ui.output_mut(|o| o.copied_text = self.content.clone());
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Search (Ctrl+F)").clicked() {
                        self.show_search = !self.show_search;
                        ui.close_menu();
                    }
                });
            });
        });
    }

    fn handle_file_dialog(&mut self) {
        if self.show_open_dialog {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Markdown", &["md", "markdown"])
                .pick_file()
            {
                if let Err(e) = self.load_file(path) {
                    eprintln!("Error loading file: {e}");
                }
            }
            self.show_open_dialog = false;
        }
    }

    fn show_search_bar(&mut self, ctx: &Context) {
        egui::TopBottomPanel::top("search_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Search:");
                let response = ui.text_edit_singleline(&mut self.search_query);

                // Auto-focus the search box when opened
                if self.show_search {
                    response.request_focus();
                }

                // Perform search when text changes
                if response.changed() {
                    self.perform_search();
                }

                // Handle Enter key to go to next result
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.next_search_result();
                }

                // Handle Escape key to close search
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.show_search = false;
                    self.search_results.clear();
                }

                ui.separator();

                // Case sensitivity toggle
                ui.checkbox(&mut self.search_case_sensitive, "Case sensitive");
                if ui.button("🔄").on_hover_text("Refresh search").clicked() {
                    self.perform_search();
                }

                ui.separator();

                // Navigation buttons
                let has_results = !self.search_results.is_empty();
                ui.add_enabled_ui(has_results, |ui| {
                    if ui.button("⬆").on_hover_text("Previous result").clicked() {
                        self.previous_search_result();
                    }
                    if ui.button("⬇").on_hover_text("Next result").clicked() {
                        self.next_search_result();
                    }
                });

                // Show result count
                if has_results {
                    ui.label(format!(
                        "{}/{}",
                        self.current_search_index + 1,
                        self.search_results.len()
                    ));
                } else if !self.search_query.is_empty() {
                    ui.label("No results");
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("✖").clicked() {
                        self.show_search = false;
                        self.search_results.clear();
                    }
                });
            });
        });
    }

    fn show_drop_zone(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(50.0);

            ui.heading("mdzen");
            ui.add_space(20.0);

            // Create a large drop zone area
            let _drop_zone = egui::Frame::none()
                .fill(ui.visuals().faint_bg_color)
                .stroke(egui::Stroke::new(2.0, ui.visuals().weak_text_color()))
                .inner_margin(40.0)
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(20.0);
                        ui.label("📁");
                        ui.add_space(10.0);
                        ui.label("Drag and drop a markdown file here");
                        ui.label("or");
                        ui.add_space(10.0);
                        if ui.button("Choose File").clicked() {
                            self.show_open_dialog = true;
                        }
                        ui.add_space(20.0);
                    });
                });

            // Handle file drops
            if !ui.ctx().input(|i| i.raw.dropped_files.is_empty()) {
                if let Some(dropped_file) = ui.ctx().input(|i| i.raw.dropped_files.first().cloned())
                {
                    if let Some(path) = dropped_file.path {
                        if let Some(extension) = path.extension() {
                            if extension == "md" || extension == "markdown" || extension == "txt" {
                                if let Err(e) = self.load_file(path) {
                                    eprintln!("Error loading dropped file: {e}");
                                }
                            }
                        }
                    }
                }
            }

            // Visual feedback for drag over
            let is_drag_over = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());
            if is_drag_over {
                ui.painter().rect_filled(
                    ui.max_rect(),
                    0.0,
                    ui.visuals().selection.bg_fill.gamma_multiply(0.5),
                );
            }
        });
    }
}

impl eframe::App for MarkdownReaderApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Handle keyboard shortcuts
        if ctx.input(|i| i.key_pressed(egui::Key::T) && i.modifiers.ctrl)
            && ctx.input(|i| i.key_pressed(egui::Key::W))
        {
            self.wide_mode = !self.wide_mode;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::F) && i.modifiers.ctrl) {
            self.show_search = !self.show_search;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.show_search = false;
        }

        self.show_menu_bar(ctx);
        self.handle_file_dialog();

        // Show search bar
        if self.show_search {
            self.show_search_bar(ctx);
        }

        // Show TOC sidebar
        self.show_toc_sidebar(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(file_path) = &self.current_file {
                ui.heading(format!("File: {}", file_path.display()));
                ui.separator();

                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        ui.add_space(10.0);

                        // Center the content horizontally with padding on both sides
                        ui.horizontal(|ui| {
                            let total_width = ui.available_width();

                            if self.wide_mode {
                                // Wide mode: 5% side padding (minimal)
                                let side_padding = total_width * 0.05;
                                ui.add_space(side_padding);
                                let content_width = ui.available_width() - side_padding;

                                ui.vertical(|ui| {
                                    let current_search_result = if !self.search_results.is_empty() {
                                        Some(&self.search_results[self.current_search_index])
                                    } else {
                                        None
                                    };
                                    let content = self.content.clone();
                                    let search_query = self.search_query.clone();
                                    let scroll_to = self.scroll_to_header.clone();
                                    if self
                                        .markdown_renderer
                                        .render(
                                            ui,
                                            &content,
                                            &search_query,
                                            current_search_result,
                                            &mut self.image_cache,
                                            &self.current_file,
                                            &scroll_to,
                                            Some(content_width),
                                        )
                                        .is_some()
                                    {
                                        self.scroll_to_header = None; // Clear the scroll target after use
                                    }
                                });
                            } else {
                                // Normal mode: 25% side padding for centered reading column
                                let side_padding = total_width * 0.25;
                                ui.add_space(side_padding);
                                let content_width = ui.available_width() - side_padding;

                                ui.vertical(|ui| {
                                    let current_search_result = if !self.search_results.is_empty() {
                                        Some(&self.search_results[self.current_search_index])
                                    } else {
                                        None
                                    };
                                    let content = self.content.clone();
                                    let search_query = self.search_query.clone();
                                    let scroll_to = self.scroll_to_header.clone();
                                    if self
                                        .markdown_renderer
                                        .render(
                                            ui,
                                            &content,
                                            &search_query,
                                            current_search_result,
                                            &mut self.image_cache,
                                            &self.current_file,
                                            &scroll_to,
                                            Some(content_width),
                                        )
                                        .is_some()
                                    {
                                        self.scroll_to_header = None; // Clear the scroll target after use
                                    }
                                });
                            }
                        });
                    });
            } else {
                self.show_drop_zone(ui);
            }
        });
    }
}

impl MarkdownReaderApp {
    fn show_toc_sidebar(&mut self, ctx: &Context) {
        if self.show_toc && !self.toc_headers.is_empty() {
            egui::SidePanel::left("toc_panel")
                .default_width(200.0)
                .width_range(150.0..=400.0)
                .show(ctx, |ui| {
                    ui.heading("Table of Contents");
                    ui.separator();

                    egui::ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            for header in &self.toc_headers {
                                let indent = (header.level as f32 - 1.0) * 12.0;
                                ui.horizontal(|ui| {
                                    ui.add_space(indent);
                                    if ui.button(&header.title).clicked() {
                                        self.scroll_to_header = Some(header.title.clone());
                                    }
                                });
                            }
                        });
                });
        }
    }
}
