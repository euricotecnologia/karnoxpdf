//! Dedicated PDF to JPG workspace.

use std::sync::Arc;

use egui::{Align, CornerRadius, Layout, Stroke};
use karnoxpdf_engine::Session;
use karnoxpdf_engine::export::{Exporter, ImageFormat};

use crate::marks_ui::PageRange;
use crate::theme::{self, Tokens};
use crate::{KarnoxPDFApp, i18n, icons, panels::human_size, widgets};

#[derive(Clone, Debug)]
pub struct JpgSource {
    pub name: String,
    pub bytes: Arc<Vec<u8>>,
    pub pages: usize,
    pub path: Option<String>,
}

#[derive(Clone, Debug)]
pub struct JpgImage {
    pub name: String,
    pub bytes: Arc<Vec<u8>>,
}

#[derive(Clone, Debug)]
pub struct JpgWorkspace {
    pub open: bool,
    pub active: bool,
    pub source: Option<JpgSource>,
    pub dpi: f64,
    pub quality: u8,
    pub range: PageRange,
    pub result: Vec<JpgImage>,
}

impl Default for JpgWorkspace {
    fn default() -> Self {
        Self { open: false, active: false, source: None, dpi: 150.0, quality: 85, range: PageRange::default(), result: Vec::new() }
    }
}

pub(crate) fn workspace(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens) {
    let state = app.jpg_workspace.clone();
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(28.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(900.0);
            ui.heading(egui::RichText::new(tl!("PDF to JPG")).font(theme::semibold(28.0)).color(t.text));
            ui.add_space(6.0);
            ui.label(egui::RichText::new(tl!("Convert PDF pages to high-quality JPG images.")).color(t.text_muted));
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
                    if let Some(source) = state.source.as_ref() {
                        crate::widgets::style_card_inputs(ui);
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
                                            app.jpg_source_dialog();
                                        }
                                    });
                                });
                            },
                        );
                    } else {
                        ui.add_space(20.0);
                        ui.add(icons::image("image", 38.0, t.accent));
                        ui.add_space(10.0);
                        ui.label(egui::RichText::new(tl!("Select a PDF to convert")).font(theme::semibold(18.0)).color(t.text));
                        ui.add_space(14.0);
                        if widgets::dialog_action_button(ui, "folder-open", tl!("Open file"), true).clicked() {
                            app.jpg_source_dialog();
                        }
                        ui.add_space(24.0);
                    }

                    if let Some(source) = state.source.as_ref() {
                        ui.add_space(22.0);
                        ui.separator();
                        ui.add_space(18.0);
                        ui.label(egui::RichText::new(tl!("About the conversion")).font(theme::semibold(15.0)).color(t.text));
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(tl!("Each PDF page becomes a JPG image. Choose the resolution, quality, and pages to convert."))
                                .color(t.text_muted),
                        );
                        ui.add_space(18.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(tl!("Resolution")).font(theme::medium(14.0)).color(t.text));
                            egui::ComboBox::from_id_salt("jpg-workspace-dpi")
                                .selected_text(i18n::fmt(tl!("{dpi} pixels/inch"), &[("dpi", &app.jpg_workspace.dpi.to_string())]))
                                .show_ui(ui, |ui| {
                                    for dpi in [72.0, 96.0, 150.0, 300.0, 600.0] {
                                        ui.selectable_value(
                                            &mut app.jpg_workspace.dpi,
                                            dpi,
                                            i18n::fmt(tl!("{dpi} pixels/inch"), &[("dpi", &dpi.to_string())]),
                                        );
                                    }
                                });
                            ui.add_space(22.0);
                            ui.label(egui::RichText::new(tl!("Quality")).font(theme::medium(14.0)).color(t.text));
                            let mut quality = u32::from(app.jpg_workspace.quality);
                            crate::widgets::stepper_u32(ui, &mut quality, 10, 100, 5, "%");
                            ui.add_space(6.0);
                            ui.add(egui::Slider::new(&mut quality, 10..=100).show_value(false));
                            app.jpg_workspace.quality = quality.clamp(10, 100) as u8;
                        });
                        ui.add_space(14.0);
                        ui.label(egui::RichText::new(tl!("Pages")).font(theme::medium(14.0)).color(t.text));
                        app.jpg_workspace.range.ui(ui, source.pages);
                        ui.add_space(20.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(tl!("The source PDF will not be changed.")).color(t.text_muted).small());
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let has_pages = !app.jpg_workspace.range.pages(source.pages).is_empty();
                                if ui
                                    .add_enabled_ui(has_pages, |ui| widgets::dialog_action_button(ui, "image", tl!("Convert to JPG"), true))
                                    .inner
                                    .clicked()
                                {
                                    app.convert_to_jpg();
                                }
                            });
                        });
                    }

                    if !state.result.is_empty() {
                        ui.add_space(22.0);
                        ui.separator();
                        ui.add_space(16.0);
                        ui.label(egui::RichText::new(tl!("JPG images ready")).font(theme::semibold(15.0)).color(t.text));
                        ui.add_space(8.0);
                        let total = state.result.iter().fold(0usize, |sum, image| sum.saturating_add(image.bytes.len()));
                        let summary = if state.result.len() == 1 {
                            i18n::fmt(tl!("1 JPG image · {size}"), &[("size", &human_size(total))])
                        } else {
                            i18n::fmt(tl!("{n} JPG images · {size}"), &[("n", &state.result.len().to_string()), ("size", &human_size(total))])
                        };
                        ui.label(egui::RichText::new(summary).color(t.text_muted));
                        ui.add_space(14.0);
                        if widgets::dialog_action_button(ui, "save", tl!("Save JPG images"), true).clicked() {
                            app.save_jpg_result();
                        }
                    }
                });
        });
    });
}

impl KarnoxPDFApp {
    pub fn open_jpg_workspace(&mut self) {
        self.deactivate_all_workspaces();
        self.jpg_workspace.open = true;
        self.jpg_workspace.active = true;
        self.active = None;
        self.dialog = None;
    }

    pub(crate) fn close_jpg_workspace(&mut self) {
        let was_active = self.jpg_workspace.active;
        self.jpg_workspace.open = false;
        self.jpg_workspace.active = false;
        self.jpg_workspace.result.clear();
        if was_active {
            self.active = self.views.len().checked_sub(1);
        }
    }

    pub(crate) fn stage_jpg_source(&mut self, name: String, bytes: Vec<u8>, path: Option<String>) {
        let bytes = Arc::new(bytes);
        match karnoxpdf_render::inspect(bytes.clone(), None) {
            Ok(info) if !info.pages.is_empty() => {
                self.jpg_workspace.source = Some(JpgSource { name, bytes, pages: info.pages.len(), path });
                self.jpg_workspace.range = PageRange::default();
                self.jpg_workspace.result.clear();
                self.open_jpg_workspace();
            }
            Ok(_) => self.notify_tr("The PDF has no pages to convert."),
            Err(error) => self.notify_fmt("Couldn't open {name}: {e}", &[("name", &name), ("e", &error.to_string())]),
        }
    }

    pub fn convert_to_jpg(&mut self) {
        let Some(source) = self.jpg_workspace.source.clone() else { return };
        let pages = self.jpg_workspace.range.pages(source.pages);
        if pages.is_empty() {
            self.notify_tr("The PDF has no pages to convert.");
            return;
        }
        let dpi = self.jpg_workspace.dpi;
        let quality = self.jpg_workspace.quality;
        let stem = safe_stem(&source.name);
        let converted = karnoxpdf_engine::guard(|| {
            let mut session = Session::new();
            let id = session.open(&source.name, None, source.bytes.clone(), None).map_err(|error| error.to_string())?;
            let document = session.get(id).ok_or_else(|| "the file could not be opened".to_owned())?;
            let mut exporter = Exporter::from_source(document.export_source());
            let mut result = Vec::with_capacity(pages.len());
            for page in pages {
                let bytes = exporter.image(page, dpi, ImageFormat::Jpeg { quality })?;
                result.push(JpgImage { name: format!("{stem}_page_{}.jpg", page.saturating_add(1)), bytes: Arc::new(bytes) });
            }
            Ok::<_, String>(result)
        });
        match converted {
            Ok(Ok(result)) => self.jpg_workspace.result = result,
            Ok(Err(error)) | Err(error) => self.notify_fmt("Couldn't convert the PDF: {e}", &[("e", &error)]),
        }
    }

    pub fn save_jpg_result(&mut self) {
        if self.jpg_workspace.result.is_empty() {
            return;
        }
        let images = self.jpg_workspace.result.clone();
        #[cfg(not(target_arch = "wasm32"))]
        {
            let write = move |app: &mut Self, dir: std::path::PathBuf| {
                for image in &images {
                    if dir.join(&image.name).exists() {
                        app.notify_fmt("A file named {name} already exists in the chosen folder.", &[("name", &image.name)]);
                        return;
                    }
                }
                let mut written = 0usize;
                for image in &images {
                    let path = dir.join(&image.name);
                    if let Err(error) = crate::editing::write_atomically(&path.to_string_lossy(), &image.bytes) {
                        app.notify_fmt("Couldn't write {path}: {e}", &[("path", &path.display().to_string()), ("e", &error.to_string())]);
                        return;
                    }
                    written = written.saturating_add(1);
                }
                app.notify_fmt("Saved {n} JPG images to {path}", &[("n", &written.to_string()), ("path", &dir.display().to_string())]);
            };
            match self.export_dir_override.clone() {
                Some(dir) => write(self, dir.into()),
                None => {
                    let mut dialog = rfd::AsyncFileDialog::new().set_title(tl!("Save JPG images"));
                    if let Some(parent) = self
                        .jpg_workspace
                        .source
                        .as_ref()
                        .and_then(|source| source.path.as_ref())
                        .and_then(|path| std::path::Path::new(path).parent())
                    {
                        dialog = dialog.set_directory(parent);
                    }
                    self.ask_one(crate::pickers::Ask::Folder(dialog), None, write);
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let mut saved = 0usize;
            for image in &images {
                if let Err(error) = crate::editing::download(&image.name, &image.bytes) {
                    self.notify_fmt("Couldn't download {name}: {e}", &[("name", &image.name), ("e", &error.to_string())]);
                    return;
                }
                saved = saved.saturating_add(1);
            }
            self.notify_fmt("Saved {n} JPG images", &[("n", &saved.to_string())]);
        }
    }
}

fn safe_stem(name: &str) -> String {
    let stem = name.trim_end_matches(".pdf").trim_end_matches(".PDF");
    let safe: String = stem.chars().map(|ch| if ch.is_alphanumeric() || " -_.,()".contains(ch) { ch } else { '_' }).collect();
    let trimmed = safe.trim_matches([' ', '.']);
    if trimmed.is_empty() { "document".to_owned() } else { trimmed.to_owned() }
}
