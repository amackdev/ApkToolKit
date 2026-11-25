//! Main GUI application state for ApkModder.
//!
//! Provides an interactive egui interface for:
//! - Opening an APK and listing its classes
//! - Viewing and editing smali for any class directly in the editor
//! - Detecting unsaved modifications (dirty state)
//! - Patching a single method and rebuilding the APK
//! - Restoring original META-INF signatures (APKEditor-compatible workflow)
//! - Optional signing with a keystore

use eframe::egui;
use std::fs::File;
use std::sync::mpsc;
use std::thread;

// ── Messages from background workers to the UI thread ─────────────────────

/// Messages sent by background threads to update the UI.
#[derive(Debug)]
enum GuiMessage {
    /// A patch operation has started.
    PatchStarted,
    /// A patch operation finished: Ok(output path) or Err(message).
    PatchFinished(Result<String, String>),
    /// Class list loaded from APK.
    ClassesLoaded(Result<Vec<String>, String>),
    /// Smali text loaded for a class.
    SmaliLoaded(Result<String, String>),
}

// ── Main application ──────────────────────────────────────────────────────

pub struct ApkModderApp {
    // ── File paths ──
    apk_path: Option<String>,
    output_path: Option<String>,
    sig_dir: Option<String>, // directory for restored signatures

    // ── Options ──
    no_sign: bool,

    // ── Class list ──
    classes: Vec<String>,
    class_filter: String,
    selected_class: Option<String>,

    // ── Editor ──
    smali_text: String,
    original_smali_text: String, // to detect modifications
    smali_dirty: bool,           // user has made unsaved edits

    // ── Status ──
    status: String,
    is_patching: bool,

    // ── Worker-to-UI channels ──
    tx: mpsc::Sender<GuiMessage>,
    rx: mpsc::Receiver<GuiMessage>,
}

impl ApkModderApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, apk: Option<&str>) -> Self {
        let (tx, rx) = mpsc::channel();

        let mut classes = Vec::new();
        let status = match apk {
            Some(path) => match crate::pipeline::patch_flow::list_classes(path) {
                Ok(c) => {
                    classes = c;
                    format!("Loaded: {path} ({} classes)", classes.len())
                }
                Err(e) => format!("Error loading {path}: {e}"),
            },
            None => "No APK loaded".to_string(),
        };

        let mut app = Self {
            apk_path: apk.map(|s| s.to_string()),
            output_path: None,
            sig_dir: None,
            no_sign: true,
            classes,
            class_filter: String::new(),
            selected_class: None,
            smali_text: String::new(),
            original_smali_text: String::new(),
            smali_dirty: false,
            status,
            is_patching: false,
            tx,
            rx,
        };

        if app.apk_path.is_some() {
            app.load_classes_async();
        }

        app
    }

    // ── Helpers ──────────────────────────────────────────────────────────

    fn set_status(&mut self, msg: impl Into<String>) {
        self.status = msg.into();
    }

    fn load_classes_async(&mut self) {
        let path = self.apk_path.clone().unwrap();
        let tx = self.tx.clone();

        self.set_status(format!("Loading classes from {path}..."));

        thread::spawn(move || {
            let result = crate::pipeline::patch_flow::list_classes(&path)
                .map_err(|e| e.to_string());
            let _ = tx.send(GuiMessage::ClassesLoaded(result));
        });
    }

    /// Called whenever the editor text changes by the user.
    fn on_editor_change(&mut self, new_text: String) {
        self.smali_text = new_text;
        self.smali_dirty = self.smali_text != self.original_smali_text;
        if self.smali_dirty {
            self.set_status("Unsaved changes — click Patch & Rebuild to apply");
        } else {
            self.set_status("No unsaved changes");
        }
    }

    fn run_patch_async(&mut self) {
        let input = self.apk_path.clone().unwrap();
        let output = self.output_path.clone().unwrap();
        let class = self.selected_class.clone().unwrap();

        // Get the method name from the first `.method` declaration line.
        let method = self.extract_method_name();

        let smali_text = self.smali_text.clone();
        let sig_dir = self.sig_dir.clone();
        let no_sign = self.no_sign;
        let tx = self.tx.clone();

        self.is_patching = true;
        self.smali_dirty = false;
        self.original_smali_text = smali_text.clone();
        self.set_status("Patching and rebuilding...");

        thread::spawn(move || {
            let _ = tx.send(GuiMessage::PatchStarted);

            // Write the edited smali to a temp file for the pipeline.
            let smali_file = std::env::temp_dir().join("ApkToolKit_patch.smali");
            if let Err(e) = std::fs::write(&smali_file, &smali_text) {
                let _ = tx.send(GuiMessage::PatchFinished(Err(format!(
                    "Failed to write smali temp file: {e}"
                ))));
                return;
            }

            let sig_dir_arg = if no_sign { None } else { sig_dir.as_deref() };

            let result = crate::pipeline::patch_flow::run(
                &input,
                &output,
                &class,
                &method,
                smali_file.to_str().unwrap(),
                sig_dir_arg,
            );

            let _ = tx.send(GuiMessage::PatchFinished(
                result.map(|r| r.output_path).map_err(|e| e.to_string()),
            ));
        });
    }

    /// Extract the method name from the first `.method` declaration line in the smali.
    fn extract_method_name(&self) -> String {
        for line in self.smali_text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with(".method ") {
                let rest = trimmed.trim_start_matches(".method ").trim();
                if let Some(paren) = rest.find('(') {
                    let before_paren = rest[..paren].trim();
                    if let Some(name) = before_paren.split_whitespace().rev().next() {
                        return name.to_string();
                    }
                }
                return rest.split_whitespace().next().unwrap_or("unknown").to_string();
            }
        }
        "unknown".to_string()
    }

    // ── UI panels ────────────────────────────────────────────────────────

    fn top_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.heading("ApkModder");

            ui.separator();

            // APK path input
            ui.horizontal(|ui| {
                ui.label("APK:");
                let apk_path = self.apk_path.get_or_insert_with(String::new);
                let apk_input = ui.text_edit_singleline(apk_path);
                if apk_input.lost_focus() && !apk_path.is_empty() {
                    // User finished typing a path — try to load classes
                    let path = apk_path.clone();
                    if std::path::Path::new(&path).exists() {
                        self.output_path = None;
                        self.classes.clear();
                        self.selected_class = None;
                        self.smali_text.clear();
                        self.original_smali_text.clear();
                        self.smali_dirty = false;
                        self.set_status(format!("Loaded: {path}"));
                        self.load_classes_async();
                    } else {
                        self.set_status(format!("APK not found: {path}"));
                    }
                }
            });

            // Output path input
            if self.apk_path.is_some() {
                ui.horizontal(|ui| {
                    ui.label("Output:");
                    let output_path = self.output_path.get_or_insert_with(String::new);
                    ui.text_edit_singleline(output_path);
                    if ui.button("Reload Classes").clicked() {
                        self.load_classes_async();
                    }
                });
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Quit").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });
    }

    fn left_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("class_panel")
            .default_width(320.0)
            .show(ctx, |ui| {
                ui.heading("Classes");

                // Filter box
                ui.horizontal(|ui| {
                    ui.label("Filter:");
                    ui.text_edit_singleline(&mut self.class_filter);
                });

                ui.separator();

                // Class list (scrollable)
                egui::ScrollArea::vertical()
                    .max_height(f32::INFINITY)
                    .show(ui, |ui| {
                        let filter = self.class_filter.to_lowercase();
                        // Collect matches first to avoid borrow checker issues
                        let matches: Vec<String> = self.classes.iter()
                            .filter(|c| filter.is_empty() || c.to_lowercase().contains(&filter))
                            .cloned()
                            .collect();
                        let mut class_to_load: Option<String> = None;
                        for class in &matches {
                            let selected = self.selected_class.as_deref() == Some(class.as_str());
                            if ui.selectable_label(selected, class.as_str()).clicked() {
                                class_to_load = Some(class.clone());
                                self.selected_class = Some(class.clone());
                                self.smali_dirty = false;
                            }
                        }
                        // Load smali after the loop to avoid borrow issues
                        if let Some(class_name) = class_to_load {
                            self.load_smali_async_for(&class_name);
                        }
                        if self.classes.is_empty() {
                            ui.label("(no classes)");
                        }
                    });

                ui.separator();

                // Info
                ui.label(format!("Total classes: {}", self.classes.len()));
                if let Some(ref c) = self.selected_class {
                    ui.label(format!("Selected: {c}"));
                }
                if self.smali_dirty {
                    ui.colored_label(egui::Color32::YELLOW, "● Unsaved changes");
                }
            });
    }

    /// Load smali for a class (called after class list iteration to avoid borrow issues).
    fn load_smali_async_for(&mut self, class_name: &str) {
        let path = self.apk_path.clone().unwrap();
        let class = class_name.to_string();
        let tx = self.tx.clone();

        self.set_status(format!("Loading smali for {class}..."));

        thread::spawn(move || {
            let file = File::open(&path).expect("APK file not found");
            let mut apk = apkscope_core::apk::Apk::new(file)
                .map_err(|e| format!("Failed to open APK: {e}"))
                .unwrap();
            let result =
                apkscope_core::smali::export_class_from(&mut apk, &class)
                    .map_err(|e| format!("Smali export error: {e}"));
            let _ = tx.send(GuiMessage::SmaliLoaded(result));
        });
    }

    fn center_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Smali Editor");

            if let Some(class) = &self.selected_class {
                ui.label(format!("Class: {class}"));
            }

            ui.separator();

            // Smali text editor
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    let response = egui::TextEdit::multiline(&mut self.smali_text)
                        .desired_width(f32::INFINITY)
                        .desired_rows(25)
                        .show(ui);

                    // Detect text changes — clone first to avoid borrow issues
                    if response.response.changed() {
                        let text = self.smali_text.clone();
                        self.on_editor_change(text);
                    }
                });

            ui.separator();

            // Controls
            ui.horizontal(|ui| {
                let can_patch = self.apk_path.is_some()
                    && self.selected_class.is_some()
                    && self.output_path.is_some()
                    && !self.is_patching;

                if can_patch {
                    let button_text = if self.smali_dirty {
                        "🔨 Patch & Rebuild"
                    } else {
                        "🔨 Patch & Rebuild (no changes)"
                    };
                    if ui.button(button_text).clicked() {
                        if self.smali_dirty {
                            self.run_patch_async();
                        } else {
                            self.set_status("No changes to patch — edit the smali first");
                        }
                    }
                } else {
                    ui.add_enabled(false, egui::Button::new("🔨 Patch & Rebuild"));
                    if self.apk_path.is_none() {
                        ui.label("Open an APK first");
                    } else if self.selected_class.is_none() {
                        ui.label("Select a class first");
                    } else if self.output_path.is_none() {
                        ui.label("Set output path first");
                    } else if self.is_patching {
                        ui.label("Patching in progress...");
                    }
                }

                ui.horizontal(|ui| {
                    ui.label("Sig Dir:");
                    let sig_dir = self.sig_dir.get_or_insert_with(String::new);
                    ui.text_edit_singleline(sig_dir);
                });

                ui.checkbox(&mut self.no_sign, "No signing (restore original signatures)");
            });

            ui.horizontal(|ui| {
                if ui.button("📋 Copy Smali").clicked() {
                    ui.output_mut(|o| o.copied_text = self.smali_text.clone());
                    self.set_status("Smali copied to clipboard");
                }
                if ui.button("🗑️ Clear").clicked() {
                    self.smali_text.clear();
                    self.original_smali_text.clear();
                    self.smali_dirty = true;
                    self.set_status("Editor cleared");
                }
                if ui.button("🔄 Reload").clicked() {
                    if let Some(class) = self.selected_class.clone() {
                        self.load_smali_async_for(&class);
                    }
                }
            });

            if self.is_patching {
                ui.separator();
                ui.label("⏳ Patching and rebuilding APK…");
            }
        });
    }

    fn bottom_panel(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.separator();
            ui.label(&self.status);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(ref p) = self.apk_path {
                    ui.label(format!("APK: {p}"));
                }
                if let Some(ref p) = self.output_path {
                    ui.label(format!("Out: {p}"));
                }
            });
        });
    }
}

impl eframe::App for ApkModderApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Process any messages from background workers
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                GuiMessage::PatchStarted => {
                    self.is_patching = true;
                    self.set_status("Patch started...");
                }
                GuiMessage::PatchFinished(result) => {
                    self.is_patching = false;
                    match result {
                        Ok(output_path) => {
                            self.set_status(format!("✓ Patched and rebuilt: {output_path}"));
                            // Mark as clean since the current text was patched
                            self.original_smali_text = self.smali_text.clone();
                            self.smali_dirty = false;
                        }
                        Err(e) => {
                            self.set_status(format!("✗ Patch failed: {e}"));
                        }
                    }
                }
                GuiMessage::ClassesLoaded(result) => {
                    match result {
                        Ok(classes) => {
                            self.classes = classes;
                            self.set_status(format!(
                                "Loaded {} classes",
                                self.classes.len()
                            ));
                        }
                        Err(e) => {
                            self.set_status(format!("Failed to load classes: {e}"));
                        }
                    }
                }
                GuiMessage::SmaliLoaded(result) => {
                    match result {
                        Ok(smali) => {
                            self.smali_text = smali.clone();
                            self.original_smali_text = smali;
                            self.smali_dirty = false;
                            self.set_status("Smali loaded");
                        }
                        Err(e) => {
                            self.set_status(format!("Failed to load smali: {e}"));
                        }
                    }
                }
            }
        }

        // Request repaint if patching so the UI stays responsive
        if self.is_patching {
            ctx.request_repaint();
        }

        self.top_panel(ctx);
        self.left_panel(ctx);
        self.center_panel(ctx);
        self.bottom_panel(ctx, frame);
    }
}
