//! Cleaner binary entry point.
//!
//! The application logic lives in the library crate. This binary only creates the
//! icon and starts the native eframe loop.

use cleaner::app::CleanerApp;
use cleaner::themes;
use eframe::egui;

fn main() -> Result<(), eframe::Error> {
    // Any command-line argument means headless mode; no args = GUI.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        std::process::exit(cleaner::cli::run(&args));
    }

    let icon = themes::generate_icon(cleaner::settings::Settings::load().theme);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_icon(icon),
        ..Default::default()
    };
    eframe::run_native(
        "Cleaner",
        options,
        Box::new(|_cc| Box::new(CleanerApp::default())),
    )
}
