//! Watermark PDF workspace: Add custom text or image watermarks to a PDF,
//! adjust opacity, rotation, scale, layers and page ranges, and save directly.

use std::sync::Arc;

use egui::{Align, Align2, Color32, CornerRadius, Layout, Sense, Stroke, vec2};
use karnoxpdf_engine::{Edit, MarkFile, Session, Watermark};

use crate::marks_ui::{PageRange, Subset};
use crate::panels::human_size;
use crate::theme::{self, Tokens};
use crate::{KarnoxPDFApp, icons, widgets};

/// Source kind of the watermark.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatermarkSourceKind {
    Text,
    Image,
}

/// Staged PDF source to apply watermark to.
#[derive(Clone, Debug)]
pub struct WatermarkPdfSource {
    pub name: String,
    pub bytes: Arc<Vec<u8>>,
    pub pages: usize,
    pub path: Option<String>,
}

/// State for the Watermark workspace.
#[derive(Clone, Debug)]
pub struct WatermarkWorkspace {
    pub open: bool,
    pub active: bool,
    pub source: Option<WatermarkPdfSource>,

    pub source_kind: WatermarkSourceKind,

    // Text options
    pub text: String,
    pub fit_to_page: bool,
    pub font_size: f64,
    pub color: [f64; 3],
    pub rotation: f64,
    pub opacity: f64,
    pub behind: bool,

    // Image options
    pub image_name: Option<String>,
    pub image_bytes: Option<Vec<u8>>,
    pub scale: f64,

    // Page range
    pub range: PageRange,

    pub error: Option<String>,
}

impl Default for WatermarkWorkspace {
    fn default() -> Self {
        Self {
            open: false,
            active: false,
            source: None,
            source_kind: WatermarkSourceKind::Text,
            text: "CONFIDENCIAL".to_string(),
            fit_to_page: true,
            font_size: 64.0,
            color: [0.75, 0.2, 0.2], // soft red default
            rotation: 45.0,
            opacity: 0.35,
            behind: false,
            image_name: None,
            image_bytes: None,
            scale: 0.5,
            range: PageRange::default(),
            error: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Workspace UI
// ---------------------------------------------------------------------------

pub(crate) fn workspace(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(28.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(900.0);
            ui.heading(egui::RichText::new(tl!("Watermark")).font(theme::semibold(28.0)).color(t.text));
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(tl!("Place an image or text watermark on a PDF. Adjust position, opacity, and typography.")).color(t.text_muted),
            );
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

                    let source = app.watermark_workspace.source.clone();
                    if let Some(source) = source.as_ref() {
                        egui::Frame::NONE.fill(t.field).corner_radius(CornerRadius::same(7)).inner_margin(egui::Margin::symmetric(14, 12)).show(
                            ui,
                            |ui| {
                                ui.horizontal(|ui| {
                                    ui.add(icons::image("file-text", 20.0, t.accent));
                                    ui.add_space(6.0);
                                    ui.add(egui::Label::new(egui::RichText::new(&source.name).font(theme::medium(14.0)).color(t.text)).truncate());
                                    let pages_str = if source.pages == 1 {
                                        tl!("1 page").to_string()
                                    } else {
                                        crate::i18n::fmt(tl!("{n} pages"), &[("n", &source.pages.to_string())])
                                    };
                                    ui.label(egui::RichText::new(format!("{pages_str} · {}", human_size(source.bytes.len()))).color(t.text_muted));
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if widgets::dialog_action_button(ui, "folder-open", tl!("Change file"), false).clicked() {
                                            app.watermark_source_dialog();
                                        }
                                    });
                                });
                            },
                        );

                        ui.add_space(20.0);
                        ui.separator();
                        ui.add_space(16.0);

                        // Source Type Selector (Text vs Image)
                        ui.label(egui::RichText::new(tl!("Watermark type")).font(theme::semibold(15.0)).color(t.text));
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 10.0;
                            let is_text = app.watermark_workspace.source_kind == WatermarkSourceKind::Text;
                            if crate::widgets::option_button(ui, Some("type"), tl!("Text watermark"), is_text, 170.0).clicked() {
                                app.watermark_workspace.source_kind = WatermarkSourceKind::Text;
                            }
                            let is_img = app.watermark_workspace.source_kind == WatermarkSourceKind::Image;
                            if crate::widgets::option_button(ui, Some("image"), tl!("Image watermark"), is_img, 170.0).clicked() {
                                app.watermark_workspace.source_kind = WatermarkSourceKind::Image;
                            }
                        });

                        ui.add_space(16.0);

                        match app.watermark_workspace.source_kind {
                            WatermarkSourceKind::Text => text_options(ui, app, t),
                            WatermarkSourceKind::Image => image_options(ui, app, t),
                        }

                        ui.add_space(18.0);
                        ui.separator();
                        ui.add_space(14.0);

                        // Appearance & Page Range section
                        appearance_and_range_section(ui, app, t, source.pages);

                        ui.add_space(20.0);

                        if let Some(err) = &app.watermark_workspace.error {
                            ui.label(egui::RichText::new(err).color(Color32::from_rgb(0xD7, 0x37, 0x3F)));
                            ui.add_space(10.0);
                        }

                        // Bottom Action Buttons
                        ui.horizontal(|ui| {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if widgets::pill_button(ui, tl!("Apply watermark and save PDF"), true).clicked() {
                                    app.execute_watermark_and_save();
                                }
                                if widgets::pill_button(ui, tl!("Close"), false).clicked() {
                                    app.close_watermark_workspace();
                                }
                            });
                        });
                    } else {
                        // Empty state: select PDF
                        ui.add_space(20.0);
                        ui.vertical_centered(|ui| {
                            ui.add(icons::image("stamp", 38.0, t.accent));
                            ui.add_space(10.0);
                            ui.label(egui::RichText::new(tl!("Select a PDF to add watermark")).font(theme::semibold(18.0)).color(t.text));
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new(tl!("Choose a PDF document to apply a text or image watermark.")).color(t.text_muted));
                            ui.add_space(14.0);
                            if widgets::dialog_action_button(ui, "folder-open", tl!("Open file"), true).clicked() {
                                app.watermark_source_dialog();
                            }
                        });
                        ui.add_space(24.0);
                    }
                });
        });
    });
}

// ---------------------------------------------------------------------------
// Custom UI Controls (Tactile Buttons & Chips)
// ---------------------------------------------------------------------------

fn color_chip(ui: &mut egui::Ui, t: &Tokens, label: &str, rgb: [f64; 3], selected: bool) -> egui::Response {
    let font = if selected { theme::semibold(12.0) } else { theme::medium(12.0) };
    let text_w = ui.fonts_mut(|f| f.layout_no_wrap(label.to_owned(), font.clone(), t.text).size().x);
    let (rect, resp) = ui.allocate_exact_size(vec2(text_w + 34.0, 26.0), Sense::click());

    let (bg, stroke, text_color) = if selected {
        (t.accent, Stroke::NONE, Color32::WHITE)
    } else if resp.hovered() {
        (t.hover, Stroke::new(1.2, t.accent), t.text)
    } else {
        (t.card, Stroke::new(1.0, t.border), t.text)
    };

    ui.painter().rect(rect, CornerRadius::same(7), bg, stroke, egui::StrokeKind::Inside);
    let dot_color = Color32::from_rgb((rgb[0] * 255.0) as u8, (rgb[1] * 255.0) as u8, (rgb[2] * 255.0) as u8);
    let circle_center = rect.left_center() + vec2(13.0, 0.0);
    ui.painter().circle_filled(circle_center, 4.5, dot_color);
    if selected {
        ui.painter().circle_stroke(circle_center, 5.8, Stroke::new(1.0, Color32::WHITE));
    }
    ui.painter().text(rect.left_center() + vec2(23.0, 0.0), Align2::LEFT_CENTER, label, font, text_color);
    resp
}

// ---------------------------------------------------------------------------
// Text Watermark Options
// ---------------------------------------------------------------------------

fn text_options(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens) {
    egui::Frame::NONE
        .fill(t.field)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(tl!("Text")).font(theme::semibold(14.0)).color(t.text));
                ui.add_space(8.0);
                ui.label(egui::RichText::new(tl!("Presets:")).color(t.text_muted).small());
                ui.add_space(4.0);
                for preset in &["CONFIDENCIAL", "RASCUNHO", "CÓPIA", "APROVADO", "URGENTE"] {
                    let is_sel = app.watermark_workspace.text == *preset;
                    if crate::widgets::chip_button(ui, preset, is_sel).clicked() {
                        app.watermark_workspace.text = (*preset).to_string();
                    }
                }
            });

            ui.add_space(10.0);
            ui.add(
                egui::TextEdit::singleline(&mut app.watermark_workspace.text)
                    .hint_text("CONFIDENCIAL")
                    .desired_width(ui.available_width().min(520.0)),
            );

            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(tl!("Font size:")).color(t.text_muted));
                ui.add_space(4.0);
                let is_fit = app.watermark_workspace.fit_to_page;
                if crate::widgets::option_button(ui, Some("maximize-2"), tl!("Fit to page"), is_fit, 130.0).clicked() {
                    app.watermark_workspace.fit_to_page = true;
                }
                if crate::widgets::option_button(ui, Some("type"), tl!("Font size:"), !is_fit, 110.0).clicked() {
                    app.watermark_workspace.fit_to_page = false;
                }
                if !app.watermark_workspace.fit_to_page {
                    ui.add_space(8.0);
                    ui.add(egui::DragValue::new(&mut app.watermark_workspace.font_size).range(12.0..=240.0).suffix(" pt"));
                }
            });

            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(tl!("Color:")).color(t.text_muted));
                ui.add_space(6.0);

                let color_presets: &[(&str, [f64; 3])] = &[
                    (tl!("Red"), [0.85, 0.2, 0.2]),
                    (tl!("Gray"), [0.6, 0.6, 0.6]),
                    (tl!("Blue"), [0.2, 0.4, 0.85]),
                    (tl!("Black"), [0.0, 0.0, 0.0]),
                ];

                for (name, rgb) in color_presets {
                    let is_sel = (app.watermark_workspace.color[0] - rgb[0]).abs() < 0.01
                        && (app.watermark_workspace.color[1] - rgb[1]).abs() < 0.01
                        && (app.watermark_workspace.color[2] - rgb[2]).abs() < 0.01;
                    if color_chip(ui, t, name, *rgb, is_sel).clicked() {
                        app.watermark_workspace.color = *rgb;
                    }
                    ui.add_space(4.0);
                }
            });
        });
}

// ---------------------------------------------------------------------------
// Image Watermark Options
// ---------------------------------------------------------------------------

fn image_options(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens) {
    egui::Frame::NONE
        .fill(t.field)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(tl!("Upload watermark image (PNG, JPG):")).color(t.text_muted).small());
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if let Some(name) = &app.watermark_workspace.image_name {
                    ui.add(icons::image("image", 20.0, t.accent));
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(name).font(theme::medium(13.5)).color(t.text));
                    ui.add_space(10.0);
                    if widgets::pill_button(ui, tl!("Remove"), false).clicked() {
                        app.watermark_workspace.image_name = None;
                        app.watermark_workspace.image_bytes = None;
                    }
                } else if widgets::dialog_action_button(ui, "folder-open", tl!("Choose image file…"), true).clicked() {
                    app.watermark_image_dialog();
                }
            });

            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(tl!("Scale relative to target page:")).color(t.text_muted));
                ui.add_space(6.0);
                for (scale_val, lbl) in [(0.25, "25%"), (0.50, "50%"), (0.75, "75%"), (1.00, "100%")] {
                    let is_sel = (app.watermark_workspace.scale - scale_val).abs() < 0.03;
                    if crate::widgets::chip_button(ui, lbl, is_sel).clicked() {
                        app.watermark_workspace.scale = scale_val;
                    }
                }
                ui.add_space(6.0);
                let mut pct = app.watermark_workspace.scale * 100.0;
                if ui.add(egui::Slider::new(&mut pct, 10.0..=150.0).suffix("%")).changed() {
                    app.watermark_workspace.scale = pct / 100.0;
                }
            });
        });
}

// ---------------------------------------------------------------------------
// Appearance & Page Range Section
// ---------------------------------------------------------------------------

fn appearance_and_range_section(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens, total_pages: usize) {
    ui.label(egui::RichText::new(tl!("Appearance and position")).font(theme::semibold(15.0)).color(t.text));
    ui.add_space(12.0);

    ui.horizontal_top(|ui| {
        // Controls column
        ui.vertical(|ui| {
            ui.set_max_width(520.0);

            // Rotation
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(tl!("Rotation:")).color(t.text_muted));
                ui.add_space(4.0);
                for (r, lbl) in [(45.0, "45°"), (0.0, tl!("Horizontal")), (90.0, "90°"), (-45.0, "-45°")] {
                    let is_sel = (app.watermark_workspace.rotation - r).abs() < 0.1;
                    if crate::widgets::chip_button(ui, lbl, is_sel).clicked() {
                        app.watermark_workspace.rotation = r;
                    }
                }
                ui.add_space(6.0);
                crate::widgets::stepper_whole(ui, &mut app.watermark_workspace.rotation, -180.0, 180.0, 5.0, "°");
            });

            ui.add_space(12.0);

            // Opacity
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(tl!("Opacity:")).color(t.text_muted));
                ui.add_space(4.0);
                for (op_val, lbl) in [(0.15, "15%"), (0.25, "25%"), (0.35, "35%"), (0.50, "50%"), (0.75, "75%")] {
                    let is_sel = (app.watermark_workspace.opacity - op_val).abs() < 0.02;
                    if crate::widgets::chip_button(ui, lbl, is_sel).clicked() {
                        app.watermark_workspace.opacity = op_val;
                    }
                }
                ui.add_space(6.0);
                let mut pct = app.watermark_workspace.opacity * 100.0;
                crate::widgets::stepper_whole(ui, &mut pct, 5.0, 100.0, 5.0, "%");
                ui.add(egui::Slider::new(&mut pct, 5.0..=100.0).show_value(false));
                app.watermark_workspace.opacity = (pct / 100.0).clamp(0.05, 1.0);
            });

            ui.add_space(12.0);

            // Layer: behind page vs on top
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(tl!("Layer:")).color(t.text_muted));
                ui.add_space(6.0);
                let behind = app.watermark_workspace.behind;
                if crate::widgets::option_button(ui, Some("layers"), tl!("Appear on top of page"), !behind, 150.0).clicked() {
                    app.watermark_workspace.behind = false;
                }
                if crate::widgets::option_button(ui, Some("file-text"), tl!("Appear behind page"), behind, 150.0).clicked() {
                    app.watermark_workspace.behind = true;
                }
            });

            ui.add_space(16.0);
            ui.label(egui::RichText::new(tl!("Page Range Options")).font(theme::semibold(13.0)).color(t.text));
            ui.add_space(6.0);

            // Custom modern page range selector with prominent buttons
            ui.horizontal(|ui| {
                let all = app.watermark_workspace.range.all;
                if crate::widgets::option_button(ui, Some("copy"), tl!("All pages"), all, 120.0).clicked() {
                    app.watermark_workspace.range.all = true;
                }
                if crate::widgets::option_button(ui, Some("file-text"), tl!("Pages from"), !all, 120.0).clicked() {
                    app.watermark_workspace.range.all = false;
                    if app.watermark_workspace.range.to == 1 && app.watermark_workspace.range.from == 1 {
                        app.watermark_workspace.range.to = total_pages.max(1);
                    }
                }
                if !app.watermark_workspace.range.all {
                    ui.add_space(6.0);
                    let max = total_pages.max(1) as u32;
                    let mut from = app.watermark_workspace.range.from.clamp(1, max as usize) as u32;
                    crate::widgets::stepper_u32(ui, &mut from, 1, max, 1, "");
                    app.watermark_workspace.range.from = from as usize;
                    ui.label(tl!("to"));
                    let mut to = app.watermark_workspace.range.to.clamp(1, max as usize) as u32;
                    crate::widgets::stepper_u32(ui, &mut to, 1, max, 1, "");
                    app.watermark_workspace.range.to = to as usize;
                }
                ui.add_space(10.0);
                ui.label(egui::RichText::new(tl!("Subset")).color(t.text_muted));
                let subset_label = match app.watermark_workspace.range.subset {
                    Subset::All => tl!("All pages in range"),
                    Subset::Even => tl!("Even pages only"),
                    Subset::Odd => tl!("Odd pages only"),
                };
                egui::ComboBox::from_id_salt("wm-range-subset").selected_text(subset_label).show_ui(ui, |ui| {
                    ui.selectable_value(&mut app.watermark_workspace.range.subset, Subset::All, tl!("All pages in range"));
                    ui.selectable_value(&mut app.watermark_workspace.range.subset, Subset::Even, tl!("Even pages only"));
                    ui.selectable_value(&mut app.watermark_workspace.range.subset, Subset::Odd, tl!("Odd pages only"));
                });
            });
        });

        ui.add_space(20.0);

        // Visual Preview column
        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new(tl!("Preview")).font(theme::medium(13.0)).color(t.text_muted));
            ui.add_space(6.0);

            let preview_w = 170.0;
            let preview_h = 240.0;
            let (paper_rect, _) = ui.allocate_exact_size(vec2(preview_w, preview_h), Sense::hover());
            let painter = ui.painter_at(paper_rect);

            // Paper shadow and background
            painter.rect_filled(paper_rect.translate(vec2(2.0, 2.0)), CornerRadius::same(4), Color32::from_black_alpha(20));
            painter.rect_filled(paper_rect, CornerRadius::same(4), Color32::WHITE);
            painter.rect_stroke(paper_rect, CornerRadius::same(4), Stroke::new(1.0, t.border), egui::StrokeKind::Inside);

            // Simulated document lines
            let line_color = Color32::from_gray(0xEE);
            for y in (20..220).step_by(16) {
                painter.hline(paper_rect.x_range().shrink(18.0), paper_rect.top() + y as f32, Stroke::new(2.0, line_color));
            }

            // Draw watermark preview
            let alpha = (app.watermark_workspace.opacity.clamp(0.05, 1.0) * 255.0) as u8;
            let c = app.watermark_workspace.color;
            let wm_color = Color32::from_rgba_unmultiplied((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8, alpha);

            match app.watermark_workspace.source_kind {
                WatermarkSourceKind::Text => {
                    let text = if app.watermark_workspace.text.trim().is_empty() { "CONFIDENCIAL" } else { app.watermark_workspace.text.trim() };
                    let font_size =
                        if app.watermark_workspace.fit_to_page { 22.0 } else { (app.watermark_workspace.font_size as f32 * 0.35).clamp(10.0, 36.0) };

                    // Render angled text (simulate with angled rot or centered text)
                    painter.text(paper_rect.center(), Align2::CENTER_CENTER, text, theme::semibold(font_size), wm_color);
                }
                WatermarkSourceKind::Image => {
                    let center = paper_rect.center();
                    let icon_size = (36.0 * app.watermark_workspace.scale as f32).clamp(18.0, 60.0);
                    painter.circle_stroke(center, icon_size / 2.0, Stroke::new(1.5, wm_color));
                    painter.text(center, Align2::CENTER_CENTER, tl!("IMAGE"), theme::semibold(10.0), wm_color);
                }
            }
        });
    });
}

// ---------------------------------------------------------------------------
// App Methods & Execution
// ---------------------------------------------------------------------------

impl KarnoxPDFApp {
    pub fn open_watermark_workspace(&mut self) {
        if let Some(i) = self.active
            && let Some(doc) = self.session.get(self.views[i].id)
        {
            self.watermark_workspace.source =
                Some(WatermarkPdfSource { name: doc.name.clone(), bytes: doc.bytes.clone(), pages: doc.info.pages.len(), path: doc.path.clone() });
        }

        self.deactivate_all_workspaces();
        self.watermark_workspace.open = true;
        self.watermark_workspace.active = true;
        self.active = None;
        self.dialog = None;
    }

    pub(crate) fn close_watermark_workspace(&mut self) {
        let was_active = self.watermark_workspace.active;
        self.watermark_workspace.open = false;
        self.watermark_workspace.active = false;
        if was_active {
            self.active = self.views.len().checked_sub(1);
        }
    }

    pub(crate) fn stage_watermark_source(&mut self, name: String, bytes: Vec<u8>, path: Option<String>) {
        let bytes = Arc::new(bytes);
        match karnoxpdf_render::inspect(bytes.clone(), None) {
            Ok(info) if !info.pages.is_empty() => {
                self.watermark_workspace.source = Some(WatermarkPdfSource { name, bytes, pages: info.pages.len(), path });
                self.open_watermark_workspace();
            }
            Ok(_) => self.notify_tr("The PDF has no pages to convert."),
            Err(e) => self.notify_fmt("Couldn't open {name}: {e}", &[("name", &name), ("e", &e.to_string())]),
        }
    }

    pub(crate) fn stage_watermark_image(&mut self, name: String, bytes: Vec<u8>) {
        self.watermark_workspace.image_name = Some(name);
        self.watermark_workspace.image_bytes = Some(bytes);
    }

    /// Apply watermark to source PDF and save to disk.
    pub fn execute_watermark_and_save(&mut self) {
        let Some(source) = self.watermark_workspace.source.clone() else {
            self.watermark_workspace.error = Some(tl!("No document to watermark.").to_string());
            return;
        };

        let wm = Watermark {
            text: self.watermark_workspace.text.clone(),
            source: None,
            scale: self.watermark_workspace.scale,
            font_size: if self.watermark_workspace.fit_to_page { 0.0 } else { self.watermark_workspace.font_size },
            color: self.watermark_workspace.color,
            opacity: self.watermark_workspace.opacity,
            rotation: self.watermark_workspace.rotation,
            behind: self.watermark_workspace.behind,
            offset: [0.0, 0.0],
        };

        let file = match self.watermark_workspace.source_kind {
            WatermarkSourceKind::Text => {
                if self.watermark_workspace.text.trim().is_empty() {
                    self.watermark_workspace.error = Some(tl!("Type a text for the watermark.").to_string());
                    return;
                }
                None
            }
            WatermarkSourceKind::Image => {
                let Some(img_bytes) = self.watermark_workspace.image_bytes.clone() else {
                    self.watermark_workspace.error = Some(tl!("Upload an image first.").to_string());
                    return;
                };
                let name = self.watermark_workspace.image_name.clone().unwrap_or_else(|| "watermark.png".into());
                Some(MarkFile { name, bytes: Arc::new(img_bytes), page: 0 })
            }
        };

        let pages = self.watermark_workspace.range.pages(source.pages);
        if pages.is_empty() {
            self.watermark_workspace.error = Some(tl!("Select at least one page.").to_string());
            return;
        }

        let stem = source.name.trim_end_matches(".pdf").trim_end_matches(".PDF");
        let safe: String = stem.chars().map(|ch| if ch.is_alphanumeric() || " -_.,()".contains(ch) { ch } else { '_' }).collect();
        let trimmed = safe.trim_matches([' ', '.']);
        let stem = if trimmed.is_empty() { "document" } else { trimmed };
        let filename = format!("{stem}_watermarked.pdf");

        let mut session = Session::new();
        let doc_id = match session.open(&source.name, None, source.bytes.clone(), None) {
            Ok(id) => id,
            Err(e) => {
                self.watermark_workspace.error = Some(e.to_string());
                return;
            }
        };

        let edit = Edit::AddWatermark { pages, settings: wm, replace: false, file };

        if let Err(e) = session.apply(doc_id, edit) {
            self.watermark_workspace.error = Some(e.to_string());
            return;
        }

        let result = session.save_bytes(doc_id).map(|b| (*b).clone()).map_err(|e| e.to_string());

        match result {
            Ok(bytes) => {
                self.watermark_workspace.error = None;
                let bytes = Arc::new(bytes);

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
                                app.watermark_workspace.source = None;
                                app.close_watermark_workspace();
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
                            self.watermark_workspace.source = None;
                            self.close_watermark_workspace();
                        }
                        Err(e) => self.notify_fmt("Couldn't download {name}: {e}", &[("name", &filename), ("e", &e.to_string())]),
                    }
                }
            }
            Err(e) => {
                self.watermark_workspace.error = Some(e);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watermark_workspace_defaults() {
        let ws = WatermarkWorkspace::default();
        assert!(!ws.open);
        assert!(!ws.active);
        assert_eq!(ws.source_kind, WatermarkSourceKind::Text);
        assert_eq!(ws.text, "CONFIDENCIAL");
        assert!(ws.fit_to_page);
        assert_eq!(ws.rotation, 45.0);
        assert!(!ws.behind);
        assert_eq!(ws.scale, 0.5);
    }
}
