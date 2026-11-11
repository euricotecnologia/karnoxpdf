//! Image to PDF workspace: add images (PNG, JPG, TIFF, GIF, BMP, JP2…) in any order and
//! combine them into a single PDF, one page per image.

use std::sync::Arc;

use egui::{Align, CornerRadius, Layout, Stroke};

use crate::theme::{self, Tokens};
use crate::{KarnoxPDFApp, icons, widgets};

/// One image staged for conversion.
#[derive(Clone, Debug)]
pub struct ImageEntry {
    pub name: String,
    pub bytes: Arc<Vec<u8>>,
}

/// Supported image file extensions shown in the picker filter.
pub const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "tif", "tiff", "gif", "bmp", "jp2", "j2k", "jpx", "webp"];

/// DPI options the user can choose for the output PDF page size.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum OutputDpi {
    /// Honour the DPI tag embedded in the image; fall back to 72 if absent.
    #[default]
    Embedded,
    /// Always treat every pixel as one point (72 DPI).
    Dpi72,
    /// Custom DPI value.
    Custom(f64),
}

/// Persistent state for the Image → PDF workspace.
#[derive(Clone, Debug)]
pub struct ImageToPdfWorkspace {
    pub open: bool,
    pub active: bool,
    pub images: Vec<ImageEntry>,
    pub dpi: OutputDpi,
    /// Custom DPI value shown in the DragValue widget.
    pub custom_dpi: f64,
}

impl Default for ImageToPdfWorkspace {
    fn default() -> Self {
        Self { open: false, active: false, images: Vec::new(), dpi: OutputDpi::default(), custom_dpi: 150.0 }
    }
}

// ---------------------------------------------------------------------------
// Row actions
// ---------------------------------------------------------------------------

enum RowAction {
    Up(usize),
    Down(usize),
    Remove(usize),
}

// ---------------------------------------------------------------------------
// Image list body (reused inside the workspace frame)
// ---------------------------------------------------------------------------

fn icon_for_image(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "png" => "file-image",
        "jpg" | "jpeg" => "file-image",
        "gif" => "film",
        "bmp" | "tif" | "tiff" => "file-image",
        _ => "image",
    }
}

fn body(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens) -> (bool, bool) {
    let mut action: Option<RowAction> = None;
    let n = app.image_to_pdf.images.len();

    egui::ScrollArea::vertical().max_height(360.0).auto_shrink([false, true]).show(ui, |ui| {
        egui::Grid::new("image-to-pdf-list").num_columns(3).min_col_width(40.0).spacing([10.0, 6.0]).striped(true).show(ui, |ui| {
            ui.label(egui::RichText::new(tl!("Image")).color(t.text_muted));
            ui.label(egui::RichText::new(tl!("Size")).color(t.text_muted));
            ui.label("");
            ui.end_row();

            for (i, entry) in app.image_to_pdf.images.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("{}.", i + 1)).strong().color(t.accent));
                    ui.add(icons::image(icon_for_image(&entry.name), 16.0, t.icon));
                    ui.add(egui::Label::new(&entry.name).truncate());
                });
                ui.label(egui::RichText::new(crate::panels::human_size(entry.bytes.len())).small().color(t.text_faint));
                ui.horizontal(|ui| {
                    if ui.add_enabled_ui(i > 0, |ui| icons::button(ui, "chevron-up", 24.0, false, tl!("Move up"))).inner.clicked() {
                        action = Some(RowAction::Up(i));
                    }
                    if ui.add_enabled_ui(i + 1 < n, |ui| icons::button(ui, "chevron-down", 24.0, false, tl!("Move down"))).inner.clicked() {
                        action = Some(RowAction::Down(i));
                    }
                    if icons::button(ui, "trash-2", 24.0, false, &crate::i18n::fmt(tl!("Remove {name}"), &[("name", &entry.name)])).clicked() {
                        action = Some(RowAction::Remove(i));
                    }
                });
                ui.end_row();
            }
        });
    });

    match action {
        Some(RowAction::Up(i)) => app.image_to_pdf.images.swap(i, i - 1),
        Some(RowAction::Down(i)) => app.image_to_pdf.images.swap(i, i + 1),
        Some(RowAction::Remove(i)) => {
            app.image_to_pdf.images.remove(i);
        }
        None => {}
    }

    ui.add_space(14.0);

    // DPI / resolution options
    ui.label(egui::RichText::new(tl!("Page size")).font(theme::semibold(13.0)).color(t.text));
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let mut embedded = app.image_to_pdf.dpi == OutputDpi::Embedded;
        if crate::widgets::radio_row(ui, embedded, tl!("Use image resolution")) {
            app.image_to_pdf.dpi = OutputDpi::Embedded;
        }
        embedded = app.image_to_pdf.dpi == OutputDpi::Dpi72;
        if crate::widgets::radio_row(ui, embedded, tl!("72 DPI (one point per pixel)")) {
            app.image_to_pdf.dpi = OutputDpi::Dpi72;
        }
        let is_custom = matches!(app.image_to_pdf.dpi, OutputDpi::Custom(_));
        if crate::widgets::radio_row(ui, is_custom, tl!("Custom DPI")) {
            app.image_to_pdf.dpi = OutputDpi::Custom(app.image_to_pdf.custom_dpi);
        }
        if is_custom {
            let before = app.image_to_pdf.custom_dpi;
            crate::widgets::stepper_whole(ui, &mut app.image_to_pdf.custom_dpi, 1.0, 1200.0, 10.0, "DPI");
            if app.image_to_pdf.custom_dpi != before {
                app.image_to_pdf.dpi = OutputDpi::Custom(app.image_to_pdf.custom_dpi);
            }
        }
    });

    ui.add_space(10.0);

    let (mut go, mut cancel) = (false, false);
    ui.horizontal(|ui| {
        if widgets::pill_button(ui, tl!("Add images…"), false).clicked() {
            app.image_to_pdf_dialog();
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let ok = !app.image_to_pdf.images.is_empty();
            if ui
                .add_enabled_ui(ok, |ui| widgets::pill_button(ui, tl!("Convert to PDF"), true))
                .inner
                .on_disabled_hover_text(tl!("Add at least one image"))
                .clicked()
            {
                go = true;
            }
            if widgets::pill_button(ui, tl!("Close"), false).clicked() {
                cancel = true;
            }
        });
    });
    (go, cancel)
}

// ---------------------------------------------------------------------------
// Full-page workspace
// ---------------------------------------------------------------------------

pub(crate) fn workspace(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(28.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(900.0);
            ui.heading(egui::RichText::new(tl!("Image to PDF")).font(theme::semibold(28.0)).color(t.text));
            ui.add_space(6.0);
            ui.label(egui::RichText::new(tl!("Add images in any order. Each image becomes one page in the PDF.")).color(t.text_muted));
            ui.add_space(24.0);

            egui::Frame::NONE
                .fill(t.card)
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(CornerRadius::same(12))
                .inner_margin(egui::Margin::same(24))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width().min(850.0));

                    if app.image_to_pdf.images.is_empty() {
                        // Empty state
                        ui.add_space(24.0);
                        ui.vertical_centered(|ui| {
                            ui.add(icons::image("image", 40.0, t.accent));
                            ui.add_space(12.0);
                            ui.label(egui::RichText::new(tl!("Image to PDF")).font(theme::semibold(20.0)).color(t.text));
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new(tl!("Supported formats: PNG, JPG, TIFF, GIF, BMP, JPEG 2000")).color(t.text_muted));
                            ui.add_space(18.0);
                            if widgets::pill_button(ui, tl!("Add images…"), true).clicked() {
                                app.image_to_pdf_dialog();
                            }
                        });
                        ui.add_space(30.0);
                    } else {
                        let (go, close) = body(ui, app, t);
                        if go {
                            app.convert_images_to_pdf();
                        }
                        if close {
                            app.close_image_to_pdf_workspace();
                        }
                    }
                });
        });
    });
}

// ---------------------------------------------------------------------------
// KarnoxPDFApp methods
// ---------------------------------------------------------------------------

impl KarnoxPDFApp {
    pub fn open_image_to_pdf_workspace(&mut self) {
        self.deactivate_all_workspaces();
        self.image_to_pdf.open = true;
        self.image_to_pdf.active = true;
        self.active = None;
        self.dialog = None;
    }

    pub(crate) fn close_image_to_pdf_workspace(&mut self) {
        let was_active = self.image_to_pdf.active;
        self.image_to_pdf.open = false;
        self.image_to_pdf.active = false;
        if was_active {
            self.active = self.views.len().checked_sub(1);
        }
    }

    /// Add images picked by the user to the staging list.
    pub(crate) fn stage_images_for_pdf(&mut self, images: Vec<(String, Vec<u8>)>) {
        for (name, bytes) in images {
            self.image_to_pdf.images.push(ImageEntry { name, bytes: Arc::new(bytes) });
        }
        self.open_image_to_pdf_workspace();
    }

    /// Convert the staged images into a new PDF document and save it to disk.
    pub fn convert_images_to_pdf(&mut self) {
        if self.image_to_pdf.images.is_empty() {
            return;
        }
        let resolution = match self.image_to_pdf.dpi {
            OutputDpi::Embedded => karnoxpdf_engine::ImageResolution::Embedded,
            OutputDpi::Dpi72 => karnoxpdf_engine::ImageResolution::Dpi(72.0),
            OutputDpi::Custom(v) => karnoxpdf_engine::ImageResolution::Dpi(v),
        };
        let images: Vec<(String, Vec<u8>)> = self.image_to_pdf.images.iter().map(|e| (e.name.clone(), (*e.bytes).clone())).collect();
        let filename = if images.len() == 1 {
            let stem = safe_stem_image(&images[0].0);
            format!("{stem}.pdf")
        } else {
            "Images.pdf".to_string()
        };
        let converted = karnoxpdf_engine::guard(|| self.session.create_from_images_with_resolution(&images, resolution).map_err(|e| e.to_string()));
        match converted {
            Ok(Ok(bytes)) => {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let write = move |app: &mut Self, path: std::path::PathBuf| {
                        let path = if path.extension().is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case("pdf")) {
                            path
                        } else {
                            path.with_extension("pdf")
                        };
                        let target = path.to_string_lossy().into_owned();
                        match crate::editing::write_atomically(&target, &bytes) {
                            Ok(()) => {
                                app.notify_fmt("Saved {name}", &[("name", &target)]);
                                app.image_to_pdf.images.clear();
                                app.close_image_to_pdf_workspace();
                            }
                            Err(e) => app.notify_fmt("Couldn't write {path}: {e}", &[("path", &target), ("e", &e.to_string())]),
                        }
                    };
                    match self.save_override.clone() {
                        Some(path) => write(self, path.into()),
                        None => {
                            let dialog = rfd::AsyncFileDialog::new().add_filter("PDF", &["pdf"]).set_file_name(&filename);
                            self.ask_one(crate::pickers::Ask::Save(dialog), None, write);
                        }
                    }
                }
                #[cfg(target_arch = "wasm32")]
                {
                    match crate::editing::download(&filename, &bytes) {
                        Ok(()) => {
                            self.notify_fmt("Saved {name}", &[("name", &filename)]);
                            self.image_to_pdf.images.clear();
                            self.close_image_to_pdf_workspace();
                        }
                        Err(e) => self.notify_fmt("Couldn't download {name}: {e}", &[("name", &filename), ("e", &e.to_string())]),
                    }
                }
            }
            Ok(Err(e)) | Err(e) => {
                self.notify_fmt("Couldn't create PDF: {e}", &[("e", &e)]);
            }
        }
    }
}

fn safe_stem_image(name: &str) -> String {
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    let safe: String = stem.chars().map(|ch| if ch.is_alphanumeric() || " -_.,()".contains(ch) { ch } else { '_' }).collect();
    let trimmed = safe.trim_matches([' ', '.']);
    if trimmed.is_empty() { "Images".to_owned() } else { trimmed.to_owned() }
}
