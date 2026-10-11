// No console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod gui;

use std::path::PathBuf;

use eframe::egui;

/// The demonstration build says so in the window title.
const TITLE: &str = if cfg!(feature = "demo") {
    "Biblo (démonstration)"
} else {
    "Biblo"
};

/// Data lives next to the executable (not in Program Files, which is not writable).
fn data_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("biblo-data")))
        .unwrap_or_else(|| PathBuf::from("biblo-data"))
}

/// Demonstration build: the first start fills the new data folder with sample
/// data; later starts keep whatever was changed.
#[cfg(feature = "demo")]
fn seed_demo(dir: &std::path::Path) {
    if dir.join(biblo::store::DATA_FILE).exists() {
        return;
    }
    let today = biblo::today();
    if let Ok(mut store) = biblo::Store::open(dir, today) {
        // A failure only leaves an empty library: the app still opens.
        let _ = store.apply(|lib| biblo::demo::fill(lib, today));
    }
}

fn main() -> eframe::Result {
    let data_dir = data_dir();
    #[cfg(feature = "demo")]
    seed_demo(&data_dir);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(TITLE)
            .with_inner_size([1200.0, 760.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Biblo",
        options,
        Box::new(move |cc| Ok(Box::new(gui::App::new(&cc.egui_ctx, data_dir)))),
    )
}
