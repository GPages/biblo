// No console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod gui;

use std::path::PathBuf;

use eframe::egui;

/// Data lives next to the executable (not in Program Files, which is not writable).
fn data_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("biblo-data")))
        .unwrap_or_else(|| PathBuf::from("biblo-data"))
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Biblo")
            .with_inner_size([1200.0, 760.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Biblo",
        options,
        Box::new(|cc| Ok(Box::new(gui::App::new(&cc.egui_ctx, data_dir())))),
    )
}
