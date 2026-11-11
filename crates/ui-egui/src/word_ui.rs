//! Home ▸ PDF to Word: convert one PDF to a new editable DOCX file.

use std::sync::Arc;

use egui::{Align, CornerRadius, Layout, Stroke};
use karnoxpdf_engine::{Session, compare::OfficeFormat};

use crate::theme::{self, Tokens};
use crate::{KarnoxPDFApp, i18n, icons, panels::human_size, widgets};

#[derive(Clone, Debug)]
pub struct WordSource {
    pub name: String,
    pub bytes: Arc<Vec<u8>>,
    pub pages: usize,
    pub path: Option<String>,
}

#[derive(Clone, Debug)]
pub struct WordResult {
    pub bytes: Arc<Vec<u8>>,
}

pub(crate) fn workspace(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(28.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(900.0);
            ui.heading(egui::RichText::new(tl!("PDF to Word")).font(theme::semibold(28.0)).color(t.text));
            ui.add_space(6.0);
            ui.label(egui::RichText::new(tl!("Convert a PDF into an editable Word DOCX document.")).color(t.text_muted));
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
                    let source = app.word_source.clone();
                    if let Some(source) = source.as_ref() {
                        egui::Frame::NONE.fill(t.field).corner_radius(CornerRadius::same(7)).inner_margin(egui::Margin::symmetric(14, 12)).show(
                            ui,
                            |ui| {
                                ui.horizontal(|ui| {
                                    ui.add(icons::image("file-text", 20.0, t.accent));
                                    ui.add_space(6.0);
                                    ui.add(egui::Label::new(egui::RichText::new(&source.name).font(theme::medium(14.0)).color(t.text)).truncate());
                                    let pages = if source.pages == 1 {
                                        tl!("1 page").to_owned()
                                    } else {
                                        i18n::fmt(tl!("{n} pages"), &[("n", &source.pages.to_string())])
                                    };
                                    ui.label(egui::RichText::new(format!("{pages} · {}", human_size(source.bytes.len()))).color(t.text_muted));
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if widgets::dialog_action_button(ui, "folder-open", tl!("Change file"), false).clicked() {
                                            app.word_source_dialog();
                                        }
                                    });
                                });
                            },
                        );
                    } else {
                        ui.add_space(20.0);
                        ui.vertical_centered(|ui| {
                            ui.add(icons::image("file-text", 38.0, t.accent));
                            ui.add_space(10.0);
                            ui.label(egui::RichText::new(tl!("Select a PDF to convert")).font(theme::semibold(18.0)).color(t.text));
                            ui.add_space(14.0);
                            if widgets::dialog_action_button(ui, "folder-open", tl!("Open file"), true).clicked() {
                                app.word_source_dialog();
                            }
                        });
                        ui.add_space(24.0);
                    }
                    if source.is_some() {
                        ui.add_space(22.0);
                        ui.separator();
                        ui.add_space(18.0);
                        ui.label(egui::RichText::new(tl!("About the conversion")).font(theme::semibold(15.0)).color(t.text));
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(tl!(
                                "Text, headings, tables and images are transferred to DOCX. The layout may differ from the PDF."
                            ))
                            .color(t.text_muted),
                        );
                        ui.add_space(5.0);
                        ui.label(egui::RichText::new(tl!("Scanned pages need OCR before their text can be edited.")).color(t.text_muted));
                        ui.add_space(20.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(tl!("The source PDF will not be changed.")).color(t.text_muted).small());
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if widgets::dialog_action_button(ui, "file-text", tl!("Convert to Word"), true).clicked() {
                                    app.convert_to_word();
                                }
                            });
                        });
                        if let Some(result) = app.word_result.as_ref() {
                            ui.add_space(22.0);
                            ui.separator();
                            ui.add_space(16.0);
                            ui.label(egui::RichText::new(tl!("Word document ready")).font(theme::semibold(15.0)).color(t.text));
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new(i18n::fmt(tl!("DOCX size: {size}"), &[("size", &human_size(result.bytes.len()))]))
                                    .color(t.text_muted),
                            );
                            ui.add_space(14.0);
                            if widgets::dialog_action_button(ui, "save", tl!("Save Word document"), true).clicked() {
                                app.save_word_result();
                            }
                        }
                    }
                });
        });
    });
}

impl KarnoxPDFApp {
    pub fn open_word_workspace(&mut self) {
        self.deactivate_all_workspaces();
        self.word_workspace_open = true;
        self.word_workspace_active = true;
        self.active = None;
        self.dialog = None;
    }

    pub(crate) fn close_word_workspace(&mut self) {
        self.word_workspace_open = false;
        self.word_workspace_active = false;
        self.word_result = None;
        self.active = self.views.len().checked_sub(1);
    }

    pub(crate) fn stage_word_source(&mut self, name: String, bytes: Vec<u8>, path: Option<String>) {
        let bytes = Arc::new(bytes);
        match karnoxpdf_render::inspect(bytes.clone(), None) {
            Ok(info) if !info.pages.is_empty() => {
                self.word_source = Some(WordSource { name, bytes, pages: info.pages.len(), path });
                self.word_result = None;
                self.open_word_workspace();
            }
            Ok(_) => self.notify_tr("The PDF has no pages to convert."),
            Err(e) => self.notify_fmt("Couldn't open {name}: {e}", &[("name", &name), ("e", &e.to_string())]),
        }
    }

    pub fn convert_to_word(&mut self) {
        let Some(source) = self.word_source.clone() else { return };
        self.word_result = None;
        let mut session = Session::new();
        let id = match session.open(&source.name, None, source.bytes, None) {
            Ok(id) => id,
            Err(e) => {
                self.notify_fmt("Couldn't convert to Word: {e}", &[("e", &e.to_string())]);
                return;
            }
        };
        let Some(doc) = session.get(id) else {
            self.notify_tr("Couldn't convert to Word: the PDF could not be opened.");
            return;
        };
        match karnoxpdf_engine::guard(|| doc.export_office(OfficeFormat::Docx)) {
            Ok(bytes) if bytes.starts_with(b"PK") => self.word_result = Some(WordResult { bytes: Arc::new(bytes) }),
            Ok(_) => self.notify_tr("Couldn't convert to Word: the DOCX file was invalid."),
            Err(e) => self.notify_fmt("Couldn't convert to Word: {e}", &[("e", &e)]),
        }
    }

    pub fn save_word_result(&mut self) {
        let (Some(source), Some(result)) = (self.word_source.as_ref(), self.word_result.as_ref()) else { return };
        let name = source.name.trim_end_matches(".pdf").trim_end_matches(".PDF");
        let safe: String = name.chars().map(|c| if c.is_alphanumeric() || " -_.,()".contains(c) { c } else { '_' }).collect();
        let stem = if safe.trim_matches([' ', '.']).is_empty() { "document" } else { safe.trim() };
        let filename = format!("{stem}.docx");
        let bytes = result.bytes.clone();
        #[cfg(not(target_arch = "wasm32"))]
        {
            let write = move |app: &mut Self, path: std::path::PathBuf| {
                let is_docx = path.extension().is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case("docx"));
                if !is_docx {
                    app.notify_tr("Choose a .docx filename.");
                    return;
                }
                let target = path.to_string_lossy();
                match crate::editing::write_atomically(&target, &bytes) {
                    Ok(()) => app.notify_fmt("Word document saved to {path}", &[("path", &target)]),
                    Err(e) => app.notify_fmt("Couldn't write {path}: {e}", &[("path", &target), ("e", &e.to_string())]),
                }
            };
            match self.save_override.clone() {
                Some(path) => write(self, path.into()),
                None => {
                    let mut dialog = rfd::AsyncFileDialog::new().add_filter("DOCX", &["docx"]).set_file_name(&filename);
                    if let Some(parent) = source.path.as_ref().and_then(|path| std::path::Path::new(path).parent()) {
                        dialog = dialog.set_directory(parent);
                    }
                    self.ask_one(crate::pickers::Ask::Save(dialog), None, write);
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            match crate::editing::download(&filename, &bytes) {
                Ok(()) => self.notify_fmt("Word document saved to {path}", &[("path", &filename)]),
                Err(e) => self.notify_fmt("Couldn't download {name}: {e}", &[("name", &filename), ("e", &e.to_string())]),
            }
        }
    }
}
