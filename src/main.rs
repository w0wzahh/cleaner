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
    // args_os + lossy conversion — std::env::args() panics on non-UTF8
    // arguments, which Windows paths can legitimately contain.
    // `--minimized`/`--tray` are GUI-mode flags (used by the Windows
    // startup entry) — everything else goes to the CLI.
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let gui_minimized = args
        .iter()
        .any(|a| a == "--minimized" || a == "--tray");
    let cli_args: Vec<String> = args
        .iter()
        .filter(|a| a.as_str() != "--minimized" && a.as_str() != "--tray")
        .cloned()
        .collect();
    if !cli_args.is_empty() {
        attach_parent_console();
        std::process::exit(cleaner::cli::run(&cli_args));
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
        Box::new(move |cc| {
            // Stash the Win32 HWND so the tray can show/hide the window
            // directly — egui viewport commands aren't processed while the
            // window is invisible (egui#3655/#5229).
            #[cfg(windows)]
            {
                use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                if let Ok(h) = cc.window_handle() {
                    if let RawWindowHandle::Win32(w) = h.as_raw() {
                        cleaner::helpers::set_main_hwnd(w.hwnd.get() as isize);
                    }
                }
            }
            let mut app = CleanerApp::default();
            app.start_in_tray |= gui_minimized;
            Box::new(app)
        }),
    )
}
