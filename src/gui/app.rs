//! Main GUI application state.

use eframe::egui;

pub struct ApkModderApp {
    apk_path: Option<String>,
    status: String,
    classes: Vec<String>,
    selected_class: Option<String>,
    smali_text: String,
}

impl ApkModderApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, apk: Option<&str>) -> Self {
        let mut classes = Vec::new();
        let status = match apk {
            Some(path) => {
                match crate::pipeline::patch_flow::list_classes(path) {
                    Ok(c) => {
                        classes = c;
                        format!("Loaded: {path} ({} classes)", classes.len())
                    }
                    Err(e) => format!("Error loading {path}: {e}"),
                }
            }
            None => "No APK loaded".to_string(),
        };

        Self {
            apk_path: apk.map(|s| s.to_string()),
            status,
            classes,
            selected_class: None,
            smali_text: String::new(),
        }
    }
}

impl eframe::App for ApkModderApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Top status bar
        egui::TopBottomPanel::top("status_bar").show(ctx, |ui| {
            ui.label(&self.status);
        });

        // Left panel: class list
        egui::SidePanel::left("class_list")
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.heading("Classes");
                for class in &self.classes {
                    if ui.selectable_label(self.selected_class.as_deref() == Some(class), class).clicked() {
                        self.selected_class = Some(class.clone());
                        // Load smali for selected class
                        if let Some(path) = &self.apk_path {
                            if let Ok(mut apk) = apkscope_core::apk::Apk::open(path) {
                                if let Ok(smali) = apkscope_core::smali::export_class_from(&mut apk, class) {
                                    self.smali_text = smali;
                                }
                            }
                        }
                    }
                }
            });

        // Central panel: code editor
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Smali Editor");
            if let Some(class) = &self.selected_class {
                ui.label(format!("Class: {class}"));
            }

            egui::TextEdit::multiline(&mut self.smali_text)
                .desired_width(f32::INFINITY)
                .desired_rows(30)
                .show(ui);

            ui.horizontal(|ui| {
                if ui.button("Save Changes").clicked() {
                    self.status = "Save not yet implemented".to_string();
                }
                if ui.button("Patch & Rebuild").clicked() {
                    self.status = "Patch not yet implemented".to_string();
                }
            });
        });
    }
}
