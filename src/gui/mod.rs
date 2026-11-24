//! GUI module for ApkModder (egui-based, behind `gui` feature).

pub mod app;

#[cfg(feature = "gui")]
pub fn run_gui(apk: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "ApkModder",
        native_options,
        Box::new(|cc| Ok(Box::new(app::ApkModderApp::new(cc, apk)))),
    )
    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}

#[cfg(not(feature = "gui"))]
pub fn run_gui(_apk: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    Err("GUI feature not enabled. Build with --features gui".into())
}
