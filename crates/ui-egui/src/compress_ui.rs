//! Home ▸ Compress PDF: choose a source, compare three quality presets, and save a smaller copy.

use std::sync::Arc;

use egui::{Align, CornerRadius, Layout, Stroke};
use karnoxpdf_engine::Session;
use karnoxpdf_engine::optimize::CompressionLevel;

use crate::theme::{self, Tokens};
use crate::{KarnoxPDFApp, i18n, icons, panels::human_size, widgets};

#[derive(Clone, Debug)]
pub struct CompressSource {
    pub name: String,
    pub bytes: Arc<Vec<u8>>,
    pub pages: usize,
    /// Present for desktop file picks; used to protect the original during export.
    pub path: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CompressResult {
    pub bytes: Arc<Vec<u8>>,
    pub level: CompressionLevel,
}

fn level_text(level: CompressionLevel) -> (&'static str, &'static str) {
    match level {
        CompressionLevel::Low => (tl!("Low compression"), tl!("Larger file, higher image quality · 200 ppi")),
        CompressionLevel::Balanced => (tl!("Balanced compression"), tl!("Good balance of size and quality · 150 ppi")),
        CompressionLevel::High => (tl!("High compression"), tl!("Smaller file, lower image quality · 96 ppi")),
    }
}

pub(crate) fn workspace(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(28.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(900.0);
            ui.heading(egui::RichText::new(tl!("Compress PDF")).font(theme::semibold(28.0)).color(t.text));
            ui.add_space(6.0);
            ui.label(egui::RichText::new(tl!("Reduce file size and choose how much image quality to keep.")).color(t.text_muted));
            ui.add_space(24.0);
            egui::Frame::NONE
                .fill(t.card)
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(CornerRadius::same(12))
                .inner_margin(egui::Margin::same(24))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width().min(850.0));
                    ui.label(egui::RichText::new(tl!("Source PDF")).font(theme::semibold(15.0)).color(t.text));
                    ui.add_space(10.0);
                    let source = app.compress_source.clone();
                    if let Some(source) = source.as_ref() {
                        egui::Frame::NONE.fill(t.field).corner_radius(CornerRadius::same(7)).inner_margin(egui::Margin::symmetric(14, 12)).show(
                            ui,
                            |ui| {
                                ui.horizontal(|ui| {
                                    ui.add(icons::image("file-text", 20.0, t.accent));
                                    ui.add_space(6.0);
                                    ui.add(egui::Label::new(egui::RichText::new(&source.name).font(theme::medium(14.0)).color(t.text)).truncate());
                                    ui.label(egui::RichText::new(human_size(source.bytes.len())).color(t.text_muted));
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if widgets::dialog_action_button(ui, "folder-open", tl!("Change file"), false).clicked() {
                                            app.compress_source_dialog();
                                        }
                                    });
                                });
                            },
                        );
                    } else {
                        ui.add_space(20.0);
                        ui.vertical_centered(|ui| {
                            ui.add(icons::image("file-down", 38.0, t.accent));
                            ui.add_space(10.0);
                            ui.label(egui::RichText::new(tl!("Select a PDF to compress")).font(theme::semibold(18.0)).color(t.text));
                            ui.add_space(14.0);
                            if widgets::dialog_action_button(ui, "folder-open", tl!("Open file"), true).clicked() {
                                app.compress_source_dialog();
                            }
                        });
                        ui.add_space(24.0);
                    }
                    if let Some(source) = source.as_ref() {
                        ui.add_space(22.0);
                        ui.separator();
                        ui.add_space(18.0);
                        ui.label(egui::RichText::new(tl!("Compression level")).font(theme::semibold(15.0)).color(t.text));
                        ui.add_space(5.0);
                        ui.label(egui::RichText::new(tl!("Choose a level, then compare the result before saving.")).color(t.text_muted).small());
                        ui.add_space(16.0);
                        let mut selected = app.compress_level;
                        ui.columns(3, |columns| {
                            for (column, level) in columns.iter_mut().zip([CompressionLevel::Low, CompressionLevel::Balanced, CompressionLevel::High])
                            {
                                let (label, detail) = level_text(level);
                                egui::Frame::NONE
                                    .fill(if selected == level { t.field } else { t.card })
                                    .stroke(Stroke::new(1.0, if selected == level { t.accent } else { t.border }))
                                    .corner_radius(CornerRadius::same(7))
                                    .inner_margin(egui::Margin::same(12))
                                    .show(column, |ui| {
                                        ui.set_min_height(82.0);
                                        if widgets::radio_row(ui, selected == level, label) {
                                            selected = level;
                                        }
                                        ui.add_space(4.0);
                                        ui.label(egui::RichText::new(detail).color(t.text_muted).small());
                                        ui.add_space(4.0);
                                    });
                            }
                        });
                        if selected != app.compress_level {
                            app.compress_level = selected;
                            app.compress_result = None;
                        }
                        ui.add_space(20.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(tl!("The original PDF will not be changed.")).color(t.text_muted).small());
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if widgets::dialog_action_button(ui, "file-down", tl!("Compress PDF"), true).clicked() {
                                    app.compress_now();
                                }
                            });
                        });
                        if let Some(result) = app.compress_result.as_ref().filter(|r| r.level == app.compress_level) {
                            let before = source.bytes.len();
                            let after = result.bytes.len();
                            ui.add_space(22.0);
                            ui.separator();
                            ui.add_space(16.0);
                            ui.label(egui::RichText::new(tl!("Result")).font(theme::semibold(15.0)).color(t.text));
                            ui.add_space(12.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(i18n::fmt(tl!("Original: {size}"), &[("size", &human_size(before))])).color(t.text_muted),
                                );
                                ui.add_space(18.0);
                                ui.label(egui::RichText::new(i18n::fmt(tl!("Compressed: {size}"), &[("size", &human_size(after))])).color(t.text));
                                if after < before {
                                    let percent = 100.0 * (before - after) as f64 / before.max(1) as f64;
                                    ui.add_space(18.0);
                                    ui.label(
                                        egui::RichText::new(i18n::fmt(tl!("{percent}% smaller"), &[("percent", &format!("{percent:.0}"))]))
                                            .color(t.accent)
                                            .strong(),
                                    );
                                }
                            });
                            ui.add_space(12.0);
                            if after < before {
                                if widgets::dialog_action_button(ui, "save", tl!("Save compressed PDF"), true).clicked() {
                                    app.save_compressed_result();
                                }
                            } else {
                                ui.label(egui::RichText::new(tl!("This PDF did not get smaller. Try another level.")).color(t.text_muted));
                            }
                        }
                    }
                });
        });
    });
}

impl KarnoxPDFApp {
    pub fn open_compress_workspace(&mut self) {
        self.deactivate_all_workspaces();
        self.compress_workspace_open = true;
        self.compress_workspace_active = true;
        self.active = None;
        self.dialog = None;
    }

    pub(crate) fn close_compress_workspace(&mut self) {
        self.compress_workspace_open = false;
        self.compress_workspace_active = false;
        self.active = self.views.len().checked_sub(1);
    }

    pub(crate) fn stage_compress_source(&mut self, name: String, bytes: Vec<u8>, path: Option<String>) {
        let bytes = Arc::new(bytes);
        match karnoxpdf_render::inspect(bytes.clone(), None) {
            Ok(info) if !info.pages.is_empty() => {
                self.compress_source = Some(CompressSource { name, bytes, pages: info.pages.len(), path });
                self.compress_result = None;
                self.open_compress_workspace();
            }
            Ok(_) => self.notify_tr("The PDF has no pages to compress."),
            Err(e) => self.notify_fmt("Couldn't open {name}: {e}", &[("name", &name), ("e", &e.to_string())]),
        }
    }

    pub fn compress_now(&mut self) {
        let Some(source) = self.compress_source.clone() else { return };
        self.compress_result = None;
        let mut session = Session::new();
        let id = match session.open(&source.name, None, source.bytes, None) {
            Ok(id) => id,
            Err(e) => {
                self.notify_fmt("Couldn't optimize the file: {e}", &[("e", &e.to_string())]);
                return;
            }
        };
        match session.optimized_bytes(id, &self.compress_level.settings(), &[]) {
            Ok((bytes, _)) => self.compress_result = Some(CompressResult { bytes, level: self.compress_level }),
            Err(e) => self.notify_fmt("Couldn't optimize the file: {e}", &[("e", &e.to_string())]),
        }
    }

    pub fn save_compressed_result(&mut self) {
        let (Some(source), Some(result)) = (self.compress_source.as_ref(), self.compress_result.as_ref()) else { return };
        if result.level != self.compress_level || result.bytes.len() >= source.bytes.len() {
            return;
        }
        let name = source.name.trim_end_matches(".pdf").trim_end_matches(".PDF");
        let safe: String = name.chars().map(|c| if c.is_alphanumeric() || " -_.,()".contains(c) { c } else { '_' }).collect();
        let stem = if safe.trim_matches([' ', '.']).is_empty() { "document" } else { safe.trim() };
        let filename = format!("{} ({}).pdf", stem, tl!("compressed"));
        let bytes = result.bytes.clone();
        let source_path = source.path.clone();
        #[cfg(not(target_arch = "wasm32"))]
        {
            let write = move |app: &mut Self, path: String| {
                if source_path.as_deref() == Some(path.as_str()) || std::path::Path::new(&path).exists() {
                    app.notify_tr("A file with that name already exists. Choose another name.");
                    return;
                }
                match crate::editing::write_atomically(&path, &bytes) {
                    Ok(()) => app.notify_fmt("Compressed PDF saved to {path}", &[("path", &path)]),
                    Err(e) => app.notify_fmt("Couldn't write {path}: {e}", &[("path", &path), ("e", &e.to_string())]),
                }
            };
            match self.save_override.clone() {
                Some(path) => write(self, path),
                None => {
                    let dialog = rfd::AsyncFileDialog::new().add_filter("PDF", &["pdf"]).set_file_name(&filename);
                    self.ask_one(crate::pickers::Ask::Save(dialog), None, move |app, path| write(app, path.to_string_lossy().into_owned()));
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = source_path;
            match crate::editing::download(&filename, &bytes) {
                Ok(()) => self.notify_fmt("Compressed PDF saved to {path}", &[("path", &filename)]),
                Err(e) => self.notify_fmt("Couldn't download {name}: {e}", &[("name", &filename), ("e", &e.to_string())]),
            }
        }
    }
}
