//! DyeTrace window.
//!
//! The interface only talks to the engine through a worker thread, so the
//! client can be driven without freezing the window.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod hsv;
mod picker;
mod presets;
mod preview;
mod worker;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([640.0, 386.0])
            .with_min_inner_size([620.0, 366.0])
            .with_title("DyeTrace"),
        ..Default::default()
    };

    eframe::run_native(
        "DyeTrace",
        options,
        Box::new(|cc| Ok(Box::new(app::DyeTraceApp::new(cc)))),
    )
}
