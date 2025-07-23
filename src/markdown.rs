//! # Markdown Rendering Module
//!
//! This module handles the parsing and rendering of markdown content using pulldown-cmark
//! for parsing and egui for display. It includes syntax highlighting for code blocks,
//! image loading, search highlighting, and various markdown elements.

use crate::app::SearchResult;
use egui::text::LayoutJob;
use egui::*;
use pulldown_cmark::{CodeBlockKind, CowStr, Event, Options, Parser, Tag, TagEnd};
use std::collections::HashMap;
use std::path::PathBuf;
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// Handles rendering of markdown content with syntax highlighting and search functionality.
///
/// The renderer uses pulldown-cmark for parsing markdown and syntect for syntax highlighting
/// of code blocks. It supports various markdown elements including headers, paragraphs,
/// code blocks, images, tables, lists, and more.
pub struct MarkdownRenderer {
    /// Syntax definitions for code highlighting
    syntax_set: SyntaxSet,
    /// Color themes for syntax highlighting
    theme_set: ThemeSet,
    /// Base font size for text rendering
    base_font_size: f32,
}

/// Tracks the state of the current markdown element being processed.
#[derive(Default)]
struct ElementState {
    /// Whether we're currently inside a heading
    is_heading: bool,
    /// The level of the current heading (1-6)
    heading_level: u8,
    /// Whether we're inside emphasized text
    is_emphasis: bool,
    /// Whether we're inside strong text
    is_strong: bool,
    /// Whether we're inside a blockquote
    is_blockquote: bool,
    /// Whether we're inside a link
    is_link: bool,
    /// URL of the current link
    link_url: String,
    /// Text accumulated for the current element
    accumulated_text: String,
}

impl MarkdownRenderer {
    /// Creates a new markdown renderer with default syntax highlighting setup.
    pub fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            base_font_size: 14.0,
        }
    }

    /// Sets the base font size for text rendering.
    pub fn set_font_size(&mut self, size: f32) {
        self.base_font_size = size;
    }

    /// Loads an image from a URL or file path, using the cache to avoid reloading.
    ///
    /// Supports both local files (relative to the current markdown file) and web URLs.
    /// Returns None if the image cannot be loaded.
    pub fn load_image(
        &self,
        ctx: &egui::Context,
        url: &str,
        image_cache: &mut HashMap<String, Result<egui::TextureHandle, String>>,
        current_file: &Option<PathBuf>,
    ) -> Option<egui::TextureHandle> {
        if let Some(cached_result) = image_cache.get(url) {
            return cached_result.as_ref().ok().cloned();
        }

        // Try to load image
        let load_result = self.try_load_image(ctx, url, current_file);
        let texture_handle = load_result.as_ref().ok().cloned();
        image_cache.insert(url.to_string(), load_result);
        texture_handle
    }

    fn try_load_image(
        &self,
        ctx: &egui::Context,
        url: &str,
        current_file: &Option<PathBuf>,
    ) -> Result<egui::TextureHandle, String> {
        let image_data = if url.starts_with("http://") || url.starts_with("https://") {
            // Load from URL
            reqwest::blocking::get(url)
                .map_err(|e| format!("Failed to fetch image: {e}"))?
                .bytes()
                .map_err(|e| format!("Failed to read image bytes: {e}"))?
                .to_vec()
        } else {
            // Load from local file
            let image_path = if let Some(current_file) = current_file {
                current_file
                    .parent()
                    .unwrap_or(std::path::Path::new("."))
                    .join(url)
            } else {
                std::path::PathBuf::from(url)
            };

            std::fs::read(&image_path).map_err(|e| format!("Failed to read local image: {e}"))?
        };

        let image = image::load_from_memory(&image_data)
            .map_err(|e| format!("Failed to decode image: {e}"))?;

        let rgba_image = image.to_rgba8();
        let size = [rgba_image.width() as usize, rgba_image.height() as usize];
        let pixels = rgba_image.into_raw();

        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
        Ok(ctx.load_texture(url, color_image, egui::TextureOptions::default()))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        ui: &mut Ui,
        markdown: &str,
        search_query: &str,
        current_search_result: Option<&SearchResult>,
        image_cache: &mut HashMap<String, Result<egui::TextureHandle, String>>,
        current_file: &Option<PathBuf>,
        scroll_to_header: &Option<String>,
        content_width: Option<f32>,
    ) -> Option<String> {
        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        let parser = Parser::new_ext(markdown, options);
        let events = parser.collect::<Vec<_>>();

        self.render_events(
            ui,
            events,
            search_query,
            current_search_result,
            image_cache,
            current_file,
            scroll_to_header,
            content_width,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn render_events(
        &self,
        ui: &mut Ui,
        events: Vec<Event>,
        search_query: &str,
        current_search_result: Option<&SearchResult>,
        image_cache: &mut HashMap<String, Result<egui::TextureHandle, String>>,
        current_file: &Option<PathBuf>,
        scroll_to_header: &Option<String>,
        content_width: Option<f32>,
    ) -> Option<String> {
        let mut current_paragraph = LayoutJob {
            halign: egui::Align::LEFT,
            ..Default::default()
        };
        let mut current_element = ElementState::default();
        let mut in_code_block = false;
        let mut code_block_content = String::new();
        let mut code_block_lang = String::new();
        let mut paragraph_has_content = false;
        let mut in_blockquote = false;
        let mut paragraph_links: Vec<(String, String)> = Vec::new();
        let mut list_stack: Vec<(bool, Vec<(String, usize)>)> = Vec::new(); // (is_ordered, items_with_level)
        let mut current_list_item = String::new();
        let mut current_nesting_level = 0;
        let mut in_table = false;
        let mut table_headers: Vec<String> = Vec::new();
        let mut table_rows: Vec<Vec<String>> = Vec::new();
        let mut current_table_row: Vec<String> = Vec::new();
        let mut current_table_cell = String::new();

        for event in events {
            // Debug: print events to see what we're getting
            // println!("Event: {:?}", event);
            match event {
                Event::Start(Tag::Paragraph) => {
                    current_paragraph = LayoutJob::default();
                    current_paragraph.halign = egui::Align::LEFT;
                    paragraph_has_content = false;
                }
                Event::End(TagEnd::Paragraph) => {
                    if paragraph_has_content {
                        if in_blockquote {
                            self.render_blockquote(ui, current_paragraph.clone(), content_width);
                        } else {
                            self.render_paragraph_with_links(
                                ui,
                                current_paragraph.clone(),
                                &paragraph_links,
                                content_width,
                            );
                        }
                        ui.add_space(8.0);
                    }
                    current_paragraph = LayoutJob::default();
                    current_paragraph.halign = egui::Align::LEFT;
                    paragraph_has_content = false;
                    paragraph_links.clear();
                }
                Event::Start(Tag::Heading { level, .. }) => {
                    current_element.is_heading = true;
                    current_element.heading_level = match level {
                        pulldown_cmark::HeadingLevel::H1 => 1,
                        pulldown_cmark::HeadingLevel::H2 => 2,
                        pulldown_cmark::HeadingLevel::H3 => 3,
                        pulldown_cmark::HeadingLevel::H4 => 4,
                        pulldown_cmark::HeadingLevel::H5 => 5,
                        pulldown_cmark::HeadingLevel::H6 => 6,
                    };
                    current_element.accumulated_text.clear();
                }
                Event::End(TagEnd::Heading(_)) => {
                    if !current_element.accumulated_text.is_empty() {
                        let should_scroll =
                            scroll_to_header.as_ref() == Some(&current_element.accumulated_text);
                        self.render_heading(
                            ui,
                            &current_element.accumulated_text,
                            current_element.heading_level,
                            search_query,
                            should_scroll,
                            content_width,
                        );
                        ui.add_space(12.0);
                    }
                    current_element = ElementState::default();
                }
                Event::Start(Tag::CodeBlock(kind)) => {
                    in_code_block = true;
                    code_block_content.clear();
                    code_block_lang = match kind {
                        CodeBlockKind::Fenced(lang) => lang.to_string(),
                        CodeBlockKind::Indented => String::new(),
                    };
                }
                Event::End(TagEnd::CodeBlock) => {
                    in_code_block = false;
                    self.render_code_block(
                        ui,
                        &code_block_content,
                        &code_block_lang,
                        content_width,
                    );
                    code_block_content.clear();
                    ui.add_space(8.0);
                }
                Event::Start(Tag::Emphasis) => {
                    current_element.is_emphasis = true;
                }
                Event::End(TagEnd::Emphasis) => {
                    current_element.is_emphasis = false;
                }
                Event::Start(Tag::Strong) => {
                    current_element.is_strong = true;
                }
                Event::End(TagEnd::Strong) => {
                    current_element.is_strong = false;
                }
                Event::Code(text) => {
                    if current_element.is_heading {
                        current_element.accumulated_text.push_str(&text);
                    } else {
                        self.append_inline_code(
                            &mut current_paragraph,
                            &text,
                            ui,
                            search_query,
                            current_search_result,
                        );
                        paragraph_has_content = true;
                    }
                }
                Event::Text(text) => {
                    if in_code_block {
                        code_block_content.push_str(&text);
                    } else if current_element.is_heading {
                        current_element.accumulated_text.push_str(&text);
                    } else if in_table {
                        current_table_cell.push_str(&text);
                    } else if !list_stack.is_empty() {
                        current_list_item.push_str(&text);
                    } else if !current_element.link_url.is_empty()
                        && current_element.accumulated_text.is_empty()
                    {
                        // This is alt text for an image
                        current_element.accumulated_text.push_str(&text);
                    } else {
                        if let Some(link_info) = self.append_text(
                            &mut current_paragraph,
                            &text,
                            &current_element,
                            ui,
                            search_query,
                            current_search_result,
                        ) {
                            paragraph_links.push(link_info);
                        }
                        paragraph_has_content = true;
                    }
                }
                Event::SoftBreak => {
                    if !in_code_block {
                        if current_element.is_heading {
                            current_element.accumulated_text.push(' ');
                        } else if !list_stack.is_empty() {
                            current_list_item.push(' ');
                        } else if let Some(link_info) = self.append_text(
                            &mut current_paragraph,
                            &CowStr::from(" "),
                            &current_element,
                            ui,
                            search_query,
                            current_search_result,
                        ) {
                            paragraph_links.push(link_info);
                        }
                    }
                }
                Event::HardBreak => {
                    if !in_code_block {
                        if current_element.is_heading {
                            current_element.accumulated_text.push('\n');
                        } else if !list_stack.is_empty() {
                            current_list_item.push('\n');
                        } else if let Some(link_info) = self.append_text(
                            &mut current_paragraph,
                            &CowStr::from("\n"),
                            &current_element,
                            ui,
                            search_query,
                            current_search_result,
                        ) {
                            paragraph_links.push(link_info);
                        }
                    }
                }
                Event::Start(Tag::BlockQuote(_)) => {
                    in_blockquote = true;
                    current_element.is_blockquote = true;
                }
                Event::End(TagEnd::BlockQuote(_)) => {
                    in_blockquote = false;
                    current_element.is_blockquote = false;
                }
                Event::Start(Tag::Link { dest_url, .. }) => {
                    current_element.is_link = true;
                    current_element.link_url = dest_url.to_string();
                }
                Event::End(TagEnd::Link) => {
                    current_element.is_link = false;
                    current_element.link_url.clear();
                }
                Event::Start(Tag::List(start_number)) => {
                    let is_ordered = start_number.is_some();
                    list_stack.push((is_ordered, Vec::new()));
                    current_nesting_level = list_stack.len() - 1;
                }
                Event::End(TagEnd::List(_)) => {
                    if let Some((is_ordered, items)) = list_stack.pop() {
                        self.render_nested_list(ui, &items, is_ordered, content_width);
                        ui.add_space(8.0);
                    }
                    current_nesting_level = list_stack.len().saturating_sub(1);
                }
                Event::Start(Tag::Item) => {
                    current_list_item.clear();
                }
                Event::End(TagEnd::Item) => {
                    if !list_stack.is_empty() && !current_list_item.is_empty() {
                        if let Some((_, ref mut items)) = list_stack.last_mut() {
                            items.push((current_list_item.clone(), current_nesting_level));
                        }
                        current_list_item.clear();
                    }
                }
                Event::Start(Tag::Table(_)) => {
                    in_table = true;
                    table_headers.clear();
                    table_rows.clear();
                }
                Event::End(TagEnd::Table) => {
                    if in_table {
                        self.render_table(ui, &table_headers, &table_rows, content_width);
                        ui.add_space(8.0);
                    }
                    in_table = false;
                }
                Event::Start(Tag::TableHead) => {
                    current_table_row.clear();
                }
                Event::End(TagEnd::TableHead) => {
                    if in_table {
                        table_headers = current_table_row.clone();
                        current_table_row.clear();
                    }
                }
                Event::Start(Tag::TableRow) => {
                    current_table_row.clear();
                }
                Event::End(TagEnd::TableRow) => {
                    if in_table && !current_table_row.is_empty() {
                        table_rows.push(current_table_row.clone());
                        current_table_row.clear();
                    }
                }
                Event::Start(Tag::TableCell) => {
                    current_table_cell.clear();
                }
                Event::End(TagEnd::TableCell) => {
                    if in_table {
                        current_table_row.push(current_table_cell.clone());
                        current_table_cell.clear();
                    }
                }
                Event::Start(Tag::Image {
                    dest_url, title: _, ..
                }) => {
                    // Image start - we'll get the alt text from the Text event and handle End event
                    current_element.link_url = dest_url.to_string();
                    current_element.accumulated_text.clear();
                }
                Event::End(TagEnd::Image) => {
                    // Render image with accumulated alt text
                    self.render_image(
                        ui,
                        &current_element.link_url,
                        &current_element.accumulated_text,
                        image_cache,
                        current_file,
                        content_width,
                    );
                    ui.add_space(8.0);
                    current_element.link_url.clear();
                    current_element.accumulated_text.clear();
                }
                Event::Rule => {
                    ui.separator();
                    ui.add_space(8.0);
                }
                _ => {}
            }
        }

        // Return the scroll target if we found it
        scroll_to_header.clone()
    }

    fn render_heading(
        &self,
        ui: &mut Ui,
        text: &str,
        level: u8,
        search_query: &str,
        should_scroll: bool,
        content_width: Option<f32>,
    ) {
        let font_size = match level {
            1 => self.base_font_size * 2.0,
            2 => self.base_font_size * 1.7,
            3 => self.base_font_size * 1.4,
            4 => self.base_font_size * 1.2,
            5 => self.base_font_size * 1.1,
            _ => self.base_font_size * 1.0,
        };

        let mut job = LayoutJob::default();
        let max_width = content_width.unwrap_or(ui.available_width());
        job.wrap.max_width = max_width;
        job.wrap.break_anywhere = false; // Break at word boundaries
        job.wrap.overflow_character = Some('…'); // Show ellipsis for overflow
        job.halign = egui::Align::LEFT; // Force left alignment
        job.justify = false; // Disable text justification

        if !search_query.is_empty() {
            self.append_heading_with_search_highlight(&mut job, text, font_size, ui, search_query);
        } else {
            job.append(
                text,
                0.0,
                TextFormat {
                    font_id: FontId::proportional(font_size),
                    color: ui.visuals().text_color(),
                    ..Default::default()
                },
            );
        }

        let response = ui
            .horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    [max_width, 0.0].into(),
                    egui::Layout::left_to_right(egui::Align::TOP),
                    |ui| ui.add(egui::Label::new(job).wrap()),
                )
                .inner
            })
            .inner;

        // If this is the header we want to scroll to, do it now
        if should_scroll {
            response.scroll_to_me(Some(egui::Align::TOP));
        }
    }

    fn append_heading_with_search_highlight(
        &self,
        job: &mut LayoutJob,
        text: &str,
        font_size: f32,
        ui: &Ui,
        search_query: &str,
    ) {
        let text_lower = text.to_lowercase();
        let query_lower = search_query.to_lowercase();

        let mut last_end = 0;
        let mut start_pos = 0;

        while let Some(pos) = text_lower[start_pos..].find(&query_lower) {
            let match_start = start_pos + pos;
            let match_end = match_start + search_query.len();

            // Add text before the match
            if match_start > last_end {
                let before_text = &text[last_end..match_start];
                job.append(
                    before_text,
                    0.0,
                    TextFormat {
                        font_id: FontId::proportional(font_size),
                        color: ui.visuals().text_color(),
                        ..Default::default()
                    },
                );
            }

            // Add the highlighted match
            let match_text = &text[match_start..match_end];
            job.append(
                match_text,
                0.0,
                TextFormat {
                    font_id: FontId::proportional(font_size),
                    color: ui.visuals().warn_fg_color,
                    background: ui.visuals().selection.bg_fill,
                    ..Default::default()
                },
            );

            last_end = match_end;
            start_pos = match_end;
        }

        // Add remaining text after the last match
        if last_end < text.len() {
            let after_text = &text[last_end..];
            job.append(
                after_text,
                0.0,
                TextFormat {
                    font_id: FontId::proportional(font_size),
                    color: ui.visuals().text_color(),
                    ..Default::default()
                },
            );
        }
    }

    fn append_text(
        &self,
        job: &mut LayoutJob,
        text: &CowStr,
        element: &ElementState,
        ui: &Ui,
        search_query: &str,
        _current_search_result: Option<&SearchResult>,
    ) -> Option<(String, String)> {
        let font_size = if element.is_strong {
            self.base_font_size * 1.1 // Slightly larger for bold effect
        } else {
            self.base_font_size
        };

        // Enhanced search highlighting
        if !search_query.is_empty() {
            self.append_text_with_search_highlight(job, text, element, ui, search_query, font_size);
        } else {
            // No search - render normally
            let color = if element.is_link {
                ui.visuals().hyperlink_color
            } else {
                ui.visuals().text_color()
            };

            let mut format = TextFormat {
                font_id: FontId::proportional(font_size),
                color,
                background: Color32::TRANSPARENT,
                underline: if element.is_link {
                    Stroke::new(1.0, color)
                } else {
                    Stroke::NONE
                },
                ..Default::default()
            };

            if element.is_emphasis {
                format.italics = true;
            }

            job.append(text, 0.0, format);
        }

        // Return link info if this is a link
        if element.is_link && !element.link_url.is_empty() {
            Some((element.link_url.clone(), text.to_string()))
        } else {
            None
        }
    }

    fn append_text_with_search_highlight(
        &self,
        job: &mut LayoutJob,
        text: &CowStr,
        element: &ElementState,
        ui: &Ui,
        search_query: &str,
        font_size: f32,
    ) {
        let text_str = text.to_string();
        let text_lower = text_str.to_lowercase();
        let query_lower = search_query.to_lowercase();

        let mut last_end = 0;
        let mut start_pos = 0;

        while let Some(pos) = text_lower[start_pos..].find(&query_lower) {
            let match_start = start_pos + pos;
            let match_end = match_start + search_query.len();

            // Add text before the match
            if match_start > last_end {
                let before_text = &text_str[last_end..match_start];
                self.append_text_segment(job, before_text, element, ui, font_size, false);
            }

            // Add the highlighted match
            let match_text = &text_str[match_start..match_end];
            self.append_text_segment(job, match_text, element, ui, font_size, true);

            last_end = match_end;
            start_pos = match_end;
        }

        // Add remaining text after the last match
        if last_end < text_str.len() {
            let after_text = &text_str[last_end..];
            self.append_text_segment(job, after_text, element, ui, font_size, false);
        }
    }

    fn append_text_segment(
        &self,
        job: &mut LayoutJob,
        text: &str,
        element: &ElementState,
        ui: &Ui,
        font_size: f32,
        is_search_match: bool,
    ) {
        let color = if is_search_match {
            ui.visuals().warn_fg_color
        } else if element.is_link {
            ui.visuals().hyperlink_color
        } else {
            ui.visuals().text_color()
        };

        let background = if is_search_match {
            ui.visuals().selection.bg_fill
        } else {
            Color32::TRANSPARENT
        };

        let mut format = TextFormat {
            font_id: FontId::proportional(font_size),
            color,
            background,
            underline: if element.is_link {
                Stroke::new(1.0, color)
            } else {
                Stroke::NONE
            },
            ..Default::default()
        };

        if element.is_emphasis {
            format.italics = true;
        }

        job.append(text, 0.0, format);
    }

    fn render_paragraph_with_links(
        &self,
        ui: &mut Ui,
        mut job: LayoutJob,
        links: &[(String, String)],
        content_width: Option<f32>,
    ) {
        // Force proper wrapping by using content width constraint
        let max_width = content_width.unwrap_or(ui.available_width());
        job.wrap.max_width = max_width;
        job.wrap.break_anywhere = false;
        job.wrap.overflow_character = Some('…');
        job.halign = egui::Align::LEFT;

        // Force left alignment by using horizontal layout
        let response = ui
            .horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    [max_width, 0.0].into(),
                    egui::Layout::left_to_right(egui::Align::TOP),
                    |ui| ui.add(egui::Label::new(job).wrap()),
                )
                .inner
            })
            .inner;

        // Show pointer cursor when hovering over paragraphs with links
        if !links.is_empty() && response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        // Handle link clicks
        if response.clicked() {
            if let Some((url, _text)) = links.first() {
                if url.starts_with("http://") || url.starts_with("https://") {
                    let _ = webbrowser::open(url);
                }
            }
        }
    }

    fn render_nested_list(
        &self,
        ui: &mut Ui,
        items: &[(String, usize)],
        is_ordered: bool,
        content_width: Option<f32>,
    ) {
        let max_width = content_width.unwrap_or(ui.available_width());

        for (index, (item, nesting_level)) in items.iter().enumerate() {
            ui.horizontal(|ui| {
                // Dynamic indentation based on nesting level
                let base_indent = 20.0;
                let indent_per_level = 30.0;
                let total_indent = base_indent + (indent_per_level * (*nesting_level as f32));
                ui.add_space(total_indent);

                if is_ordered {
                    ui.label(format!("{}.", index + 1));
                } else {
                    ui.label("•");
                }

                ui.add_space(8.0);

                // Create a label with proper text wrapping
                let available_width = max_width - total_indent - 40.0; // Account for indentation, bullet, and spacing
                let mut job = LayoutJob::default();
                job.wrap.max_width = available_width;
                job.wrap.break_anywhere = false;
                job.halign = egui::Align::LEFT;
                job.append(
                    item.trim(),
                    0.0,
                    TextFormat {
                        font_id: FontId::proportional(self.base_font_size),
                        color: ui.visuals().text_color(),
                        ..Default::default()
                    },
                );

                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(
                        [available_width, 0.0].into(),
                        egui::Layout::left_to_right(egui::Align::TOP),
                        |ui| ui.add(egui::Label::new(job).wrap()),
                    );
                });
            });
            ui.add_space(4.0);
        }
    }

    fn render_table(
        &self,
        ui: &mut Ui,
        headers: &[String],
        rows: &[Vec<String>],
        content_width: Option<f32>,
    ) {
        if headers.is_empty() && rows.is_empty() {
            return;
        }

        egui::Frame::none()
            .stroke(egui::Stroke::new(1.0, ui.visuals().weak_text_color()))
            .inner_margin(egui::Margin::same(8.0))
            .show(ui, |ui| {
                egui::Grid::new("table")
                    .num_columns(
                        headers
                            .len()
                            .max(rows.iter().map(|r| r.len()).max().unwrap_or(0)),
                    )
                    .spacing([10.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        // Render headers
                        if !headers.is_empty() {
                            let available_width = content_width.unwrap_or(ui.available_width());
                            for header in headers {
                                let mut job = LayoutJob::default();
                                job.wrap.max_width = available_width / headers.len() as f32;
                                job.wrap.break_anywhere = false;
                                job.halign = egui::Align::LEFT;
                                job.append(
                                    header.trim(),
                                    0.0,
                                    TextFormat {
                                        font_id: FontId::proportional(self.base_font_size),
                                        color: ui.visuals().text_color(),
                                        ..Default::default()
                                    },
                                );
                                ui.horizontal(|ui| {
                                    ui.allocate_ui_with_layout(
                                        [available_width / headers.len() as f32, 0.0].into(),
                                        egui::Layout::left_to_right(egui::Align::TOP),
                                        |ui| ui.add(egui::Label::new(job).wrap()),
                                    );
                                });
                            }
                            ui.end_row();
                        }

                        // Render rows
                        for row in rows {
                            let max_cols = headers.len().max(row.len());
                            let available_width = content_width.unwrap_or(ui.available_width());
                            for col in 0..max_cols {
                                let cell_text = row.get(col).map(|s| s.trim()).unwrap_or("");
                                let mut job = LayoutJob::default();
                                job.wrap.max_width = available_width / max_cols as f32;
                                job.wrap.break_anywhere = false;
                                job.halign = egui::Align::LEFT;
                                job.append(
                                    cell_text,
                                    0.0,
                                    TextFormat {
                                        font_id: FontId::proportional(self.base_font_size),
                                        color: ui.visuals().text_color(),
                                        ..Default::default()
                                    },
                                );
                                ui.horizontal(|ui| {
                                    ui.allocate_ui_with_layout(
                                        [available_width / max_cols as f32, 0.0].into(),
                                        egui::Layout::left_to_right(egui::Align::TOP),
                                        |ui| ui.add(egui::Label::new(job).wrap()),
                                    );
                                });
                            }
                            ui.end_row();
                        }
                    });
            });
    }

    fn render_image(
        &self,
        ui: &mut Ui,
        url: &str,
        title: &str,
        image_cache: &mut HashMap<String, Result<egui::TextureHandle, String>>,
        current_file: &Option<PathBuf>,
        content_width: Option<f32>,
    ) {
        if let Some(texture) = self.load_image(ui.ctx(), url, image_cache, current_file) {
            // Successfully loaded image - render it
            let available_width = content_width.unwrap_or(ui.available_width());
            let max_width = available_width - 20.0; // Leave margin for proper centering
            let max_height = 600.0; // Reasonable max height

            let image_size = texture.size_vec2();
            let scale_factor = (max_width / image_size.x)
                .min(max_height / image_size.y)
                .min(1.0);
            let display_size = image_size * scale_factor;

            // Left-align the image but constrain to available width
            ui.vertical(|ui| {
                let response = ui.add(egui::Image::new(&texture).max_size(display_size));

                // Make image clickable to open in browser
                if response.clicked() && (url.starts_with("http://") || url.starts_with("https://"))
                {
                    let _ = webbrowser::open(url);
                }

                if response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }

                // Show title/alt text if available
                if !title.is_empty() {
                    ui.add_space(4.0);
                    let mut job = LayoutJob::default();
                    job.wrap.max_width = display_size.x;
                    job.wrap.break_anywhere = false;
                    job.halign = egui::Align::LEFT;
                    job.append(
                        title,
                        0.0,
                        TextFormat {
                            font_id: FontId::proportional(self.base_font_size * 0.9),
                            color: ui.visuals().weak_text_color(),
                            italics: true,
                            ..Default::default()
                        },
                    );
                    ui.horizontal(|ui| {
                        ui.allocate_ui_with_layout(
                            [display_size.x, 0.0].into(),
                            egui::Layout::left_to_right(egui::Align::TOP),
                            |ui| ui.add(egui::Label::new(job).wrap()),
                        );
                    });
                }
            });
        } else {
            // Failed to load image - show placeholder
            self.render_image_placeholder(ui, url, title, content_width);
        }
    }

    fn render_image_placeholder(
        &self,
        ui: &mut Ui,
        url: &str,
        title: &str,
        content_width: Option<f32>,
    ) {
        let max_width = content_width.unwrap_or(ui.available_width());
        let frame_width = max_width.min(400.0); // Limit placeholder width

        egui::Frame::none()
            .fill(ui.visuals().faint_bg_color)
            .stroke(egui::Stroke::new(2.0, ui.visuals().weak_text_color()))
            .inner_margin(egui::Margin::same(12.0))
            .show(ui, |ui| {
                ui.set_max_width(frame_width);
                ui.vertical(|ui| {
                    ui.label("🖼️");
                    ui.add_space(4.0);

                    let mut job = LayoutJob::default();
                    job.wrap.max_width = frame_width - 24.0; // Account for margins
                    job.wrap.break_anywhere = false;
                    job.halign = egui::Align::LEFT;
                    job.append(
                        &format!("Image: {}", if title.is_empty() { url } else { title }),
                        0.0,
                        TextFormat {
                            font_id: FontId::proportional(self.base_font_size * 0.9),
                            color: ui.visuals().weak_text_color(),
                            italics: true,
                            ..Default::default()
                        },
                    );
                    ui.horizontal(|ui| {
                        ui.allocate_ui_with_layout(
                            [frame_width - 24.0, 0.0].into(),
                            egui::Layout::left_to_right(egui::Align::TOP),
                            |ui| ui.add(egui::Label::new(job).wrap()),
                        );
                    });

                    if !url.is_empty() {
                        ui.add_space(2.0);
                        let mut url_job = LayoutJob::default();
                        url_job.wrap.max_width = frame_width - 24.0;
                        url_job.wrap.break_anywhere = false;
                        url_job.halign = egui::Align::LEFT;
                        url_job.append(
                            url,
                            0.0,
                            TextFormat {
                                font_id: FontId::monospace(self.base_font_size * 0.8),
                                color: ui.visuals().hyperlink_color,
                                ..Default::default()
                            },
                        );
                        let response = ui
                            .horizontal(|ui| {
                                ui.allocate_ui_with_layout(
                                    [frame_width - 24.0, 0.0].into(),
                                    egui::Layout::left_to_right(egui::Align::TOP),
                                    |ui| ui.add(egui::Label::new(url_job).wrap()),
                                )
                                .inner
                            })
                            .inner;

                        // Make image URLs clickable
                        if response.clicked()
                            && (url.starts_with("http://") || url.starts_with("https://"))
                        {
                            let _ = webbrowser::open(url);
                        }

                        if response.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                    }
                })
            });
    }

    fn render_blockquote(&self, ui: &mut Ui, mut job: LayoutJob, content_width: Option<f32>) {
        // Set word wrap for the blockquote
        let max_width = content_width.unwrap_or(ui.available_width()) - 40.0; // Account for blockquote margins
        job.wrap.max_width = max_width;
        job.wrap.break_anywhere = false; // Break at word boundaries
        job.halign = egui::Align::LEFT;
        egui::Frame::none()
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(egui::Margin::same(12.0))
            .outer_margin(egui::Margin::same(4.0))
            .stroke(egui::Stroke::new(4.0, ui.visuals().weak_text_color()))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(
                        [max_width, 0.0].into(),
                        egui::Layout::left_to_right(egui::Align::TOP),
                        |ui| ui.add(egui::Label::new(job).wrap()),
                    );
                });
            });
    }

    fn append_inline_code(
        &self,
        job: &mut LayoutJob,
        text: &CowStr,
        ui: &Ui,
        search_query: &str,
        _current_search_result: Option<&SearchResult>,
    ) {
        if !search_query.is_empty() {
            self.append_inline_code_with_search_highlight(job, text, ui, search_query);
        } else {
            // No search - render normally
            job.append(
                text,
                0.0,
                TextFormat {
                    font_id: FontId::monospace(self.base_font_size * 0.9),
                    color: ui.visuals().text_color(),
                    background: ui.visuals().code_bg_color,
                    ..Default::default()
                },
            );
        }
    }

    fn append_inline_code_with_search_highlight(
        &self,
        job: &mut LayoutJob,
        text: &CowStr,
        ui: &Ui,
        search_query: &str,
    ) {
        let text_str = text.to_string();
        let text_lower = text_str.to_lowercase();
        let query_lower = search_query.to_lowercase();

        let mut last_end = 0;
        let mut start_pos = 0;

        while let Some(pos) = text_lower[start_pos..].find(&query_lower) {
            let match_start = start_pos + pos;
            let match_end = match_start + search_query.len();

            // Add text before the match
            if match_start > last_end {
                let before_text = &text_str[last_end..match_start];
                job.append(
                    before_text,
                    0.0,
                    TextFormat {
                        font_id: FontId::monospace(self.base_font_size * 0.9),
                        color: ui.visuals().text_color(),
                        background: ui.visuals().code_bg_color,
                        ..Default::default()
                    },
                );
            }

            // Add the highlighted match
            let match_text = &text_str[match_start..match_end];
            job.append(
                match_text,
                0.0,
                TextFormat {
                    font_id: FontId::monospace(self.base_font_size * 0.9),
                    color: ui.visuals().warn_fg_color,
                    background: ui.visuals().selection.bg_fill,
                    ..Default::default()
                },
            );

            last_end = match_end;
            start_pos = match_end;
        }

        // Add remaining text after the last match
        if last_end < text_str.len() {
            let after_text = &text_str[last_end..];
            job.append(
                after_text,
                0.0,
                TextFormat {
                    font_id: FontId::monospace(self.base_font_size * 0.9),
                    color: ui.visuals().text_color(),
                    background: ui.visuals().code_bg_color,
                    ..Default::default()
                },
            );
        }
    }

    fn render_code_block(
        &self,
        ui: &mut Ui,
        content: &str,
        language: &str,
        content_width: Option<f32>,
    ) {
        let max_width = content_width.unwrap_or(ui.available_width());
        egui::Frame::none()
            .fill(ui.visuals().code_bg_color)
            .inner_margin(8.0)
            .show(ui, |ui| {
                if language.is_empty() {
                    // Plain text code block
                    let mut job = LayoutJob::single_section(
                        content.to_string(),
                        TextFormat {
                            font_id: FontId::monospace(self.base_font_size * 0.9),
                            color: ui.visuals().text_color(),
                            ..Default::default()
                        },
                    );
                    job.wrap.max_width = max_width;
                    job.wrap.break_anywhere = false; // Allow breaking long lines
                    job.halign = egui::Align::LEFT;
                    ui.horizontal(|ui| {
                        ui.allocate_ui_with_layout(
                            [max_width, 0.0].into(),
                            egui::Layout::left_to_right(egui::Align::TOP),
                            |ui| ui.add(egui::Label::new(job).wrap()),
                        );
                    });
                } else {
                    // Syntax highlighted code block
                    self.render_highlighted_code(ui, content, language, content_width);
                }
            });
    }

    fn render_highlighted_code(
        &self,
        ui: &mut Ui,
        content: &str,
        language: &str,
        content_width: Option<f32>,
    ) {
        let syntax = self
            .syntax_set
            .find_syntax_by_extension(language)
            .or_else(|| self.syntax_set.find_syntax_by_name(language))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let theme = &self.theme_set.themes["base16-ocean.dark"];
        let mut highlighter = HighlightLines::new(syntax, theme);

        let max_width = content_width.unwrap_or(ui.available_width());
        let mut job = LayoutJob::default();
        job.wrap.max_width = max_width;
        job.wrap.break_anywhere = false; // Allow breaking long lines
        job.halign = egui::Align::LEFT;

        for line in LinesWithEndings::from(content) {
            let ranges = highlighter
                .highlight_line(line, &self.syntax_set)
                .unwrap_or_else(|_| vec![(syntect::highlighting::Style::default(), line)]);

            for (style, text) in ranges {
                let color =
                    Color32::from_rgb(style.foreground.r, style.foreground.g, style.foreground.b);
                job.append(
                    text,
                    0.0,
                    TextFormat {
                        font_id: FontId::monospace(self.base_font_size * 0.9),
                        color,
                        ..Default::default()
                    },
                );
            }
        }

        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                [max_width, 0.0].into(),
                egui::Layout::left_to_right(egui::Align::TOP),
                |ui| ui.add(egui::Label::new(job).wrap()),
            );
        });
    }
}
