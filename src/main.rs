//! # mdzen
//!
//! A minimalist, zen-like markdown reader with syntax highlighting built with egui.
//!
//! mdzen provides a clean, distraction-free interface for reading markdown files with:
//! - Syntax highlighting for code blocks
//! - Dark theme optimized for readability
//! - Search functionality
//! - Table of contents navigation
//! - File drag-and-drop support
//! - Wide/normal viewing modes

mod app;
mod markdown;

use app::MarkdownReaderApp;
use std::env;

/// Main entry point for mdzen.
///
/// Sets up the egui application with a native window and initializes the markdown reader.
/// If a file path is provided as a command line argument, it will be loaded automatically.
fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("mdzen")
            .with_icon(eframe::icon_data::from_png_bytes(&[]).unwrap_or_default()),
        ..Default::default()
    };

    eframe::run_native(
        "mdzen",
        options,
        Box::new(|cc| {
            let mut app = MarkdownReaderApp::new(cc);

            // Check if a file was passed as command line argument
            let args: Vec<String> = env::args().collect();
            if args.len() > 1 {
                let file_path = std::path::PathBuf::from(&args[1]);
                if file_path.exists() {
                    if let Err(e) = app.load_file(file_path) {
                        eprintln!("Error loading file: {e}");
                    }
                }
            }

            Ok(Box::new(app))
        }),
    )
}
