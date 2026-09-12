//! Cleaner binary entry point.
//!
//! The application logic lives in the library crate. This binary only creates the
//! icon and starts the native eframe loop.
//!
//! `windows_subsystem = "windows"` keeps a stray console window from opening
//! when the GUI launches. CLI mode re-attaches to the parent console instead.

#![windows_subsystem = "windows"]

use cleaner::app::CleanerApp;
use cleaner::themes;
use eframe::egui;

/// Give CLI mode a console to print to when run from a terminal.
/// No-op when there's no console to attach to (e.g. Task Scheduler).
#[cfg(windows)]
fn attach_parent_console() {
    extern "system" {
        fn AttachConsole(dwProcessId: u32) -> i32;
    }
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(not(windows))]
fn attach_parent_console() {}

fn main() -> Result<(), eframe::Error> {
    // Any command-line argument means headless mode; no args = GUI.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        attach_parent_console();
        std::process::exit(cleaner::cli::run(&args));
    }

    let icon = themes::generate_icon();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([880.0, 560.0])
            .with_icon(icon),
        ..Default::default()
    };
    eframe::run_native(
        "Cleaner",
        options,
        Box::new(|_cc| Box::new(CleanerApp::default())),
    )
}
