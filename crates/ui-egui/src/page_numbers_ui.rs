//! Page Numbers workspace: Add custom page numbers to a PDF document,
//! choose position, formatting, range, and typography, and save directly.

use std::sync::Arc;

use egui::{Align, Align2, Color32, CornerRadius, Layout, Rect, Sense, Stroke, vec2};
use karnoxpdf_engine::{Edit, HeaderFooter, PageNumberPosition, Session};

use crate::panels::human_size;
use crate::theme::{self, Tokens};
use crate::{KarnoxPDFApp, icons, widgets};

/// Preset formats for page numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PageNumberFormatKind {
    #[default]
    NumberOnly, // "1, 2, 3..." -> "<<1>>"
    PageN,        // "Página 1"
    PageNOfTotal, // "Página 1 de 10"
    NOfTotal,     // "1 / 10"
    Custom,       // user custom template with <<1>> and <<n>>
}

/// Page range options.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PageNumberRangeChoice {
    #[default]
    AllPages,
    Range,
}

/// Staged PDF source for page numbering.
#[derive(Clone, Debug)]
pub struct PageNumbersPdfSource {
    pub name: String,
    pub bytes: Arc<Vec<u8>>,
    pub pages: usize,
    pub path: Option<String>,
}

/// State for the Page Numbers workspace.
#[derive(Clone, Debug)]
pub struct PageNumbersWorkspace {
    pub open: bool,
    pub active: bool,
    pub source: Option<PageNumbersPdfSource>,

    // Options
    pub position: PageNumberPosition,
    pub format_kind: PageNumberFormatKind,
    pub custom_format: String,
    pub skip_first_page: bool,
    pub range_choice: PageNumberRangeChoice,
    pub range_from: usize,
    pub range_to: usize,
    pub start_number: u32,
    pub font_size: f64,
    pub margin: f64,
    pub color: [f64; 3],

    pub error: Option<String>,
    pub success_note: Option<String>,
}

impl Default for PageNumbersWorkspace {
    fn default() -> Self {
        Self {
            open: false,
            active: false,
            source: None,
            position: PageNumberPosition::BottomCenter,
            format_kind: PageNumberFormatKind::NumberOnly,
            custom_format: "Pág. <<1>> de <<n>>".to_string(),
            skip_first_page: false,
            range_choice: PageNumberRangeChoice::AllPages,
            range_from: 1,
            range_to: 1,
            start_number: 1,
            font_size: 10.0,
            margin: 36.0,
            color: [0.15, 0.15, 0.15],
            error: None,
            success_note: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn draw_icon(ui: &mut egui::Ui, name: &str, size: f32, tint: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    icons::paint(ui, rect, name, size, tint);
}

fn feature_pill(ui: &mut egui::Ui, t: &Tokens, icon: &str, label: &str) {
    egui::Frame::NONE
        .fill(t.field)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(egui::Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                draw_icon(ui, icon, 13.0, t.accent);
                ui.label(egui::RichText::new(label).font(theme::medium(11.5)).color(t.text));
            });
        });
}

/// Build the format template string with tokens `<<1>>` and `<<n>>`.
pub fn resolve_template(kind: PageNumberFormatKind, custom: &str) -> String {
    match kind {
        PageNumberFormatKind::NumberOnly => "<<1>>".to_string(),
        PageNumberFormatKind::PageN => tl!("Page <<1>>").to_string(),
        PageNumberFormatKind::PageNOfTotal => tl!("Page <<1>> of <<n>>").to_string(),
        PageNumberFormatKind::NOfTotal => "<<1>> / <<n>>".to_string(),
        PageNumberFormatKind::Custom => {
            if custom.trim().is_empty() {
                "<<1>>".to_string()
            } else {
                custom.to_string()
            }
        }
    }
}

/// Expand template tokens for UI preview display.
pub fn preview_text(template: &str, page_number: u32, total_pages: usize) -> String {
    let mut out = template.replace("<<1>>", &format!("{page_number}"));
    out = out.replace("<<n>>", &format!("{total_pages}"));
    out
}

// ---------------------------------------------------------------------------
// Workspace UI
// ---------------------------------------------------------------------------

pub(crate) fn workspace(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(28.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(960.0);

            // Workspace Header
            ui.heading(egui::RichText::new(tl!("Page Numbers")).font(theme::semibold(28.0)).color(t.text));
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(tl!("Add page numbers to PDF documents. Choose position, formatting, and typography."))
                    .color(t.text_muted)
                    .font(theme::regular(14.0)),
            );
            ui.add_space(22.0);

            // Main Card
            egui::Frame::NONE
                .fill(t.card)
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(CornerRadius::same(12))
                .inner_margin(egui::Margin::same(24))
                .show(ui, |ui| {
                    crate::widgets::style_card_inputs(ui);

                    if let Some(source) = app.page_numbers_workspace.source.clone() {
                        // Header: File Info Bar
                        ui.horizontal(|ui| {
                            draw_icon(ui, "file-text", 22.0, t.accent);
                            ui.add_space(8.0);
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(&source.name).font(theme::semibold(14.0)).color(t.text));
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{}  ·  {}",
                                        human_size(source.bytes.len()),
                                        crate::i18n::trn(crate::i18n::current(), source.pages as u64, "{n} page", "{n} pages")
                                    ))
                                    .font(theme::regular(12.0))
                                    .color(t.text_muted),
                                );
                            });
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if widgets::dialog_action_button(ui, "folder-open", tl!("Change file"), false).clicked() {
                                    app.page_numbers_source_dialog();
                                }
                            });
                        });

                        ui.add_space(14.0);
                        ui.separator();
                        ui.add_space(16.0);

                        // Two Column Layout: Options (Left 60%) + Live Preview (Right 40%)
                        ui.columns(2, |cols| {
                            // Left Column: Configuration Controls
                            let left_ui = &mut cols[0];

                            // 1. Position Selection Grid
                            left_ui.label(egui::RichText::new(tl!("Position")).font(theme::semibold(13.5)).color(t.text));
                            left_ui.add_space(6.0);

                            let current_pos = app.page_numbers_workspace.position;
                            let positions = [
                                (PageNumberPosition::TopLeft, tl!("Top Left")),
                                (PageNumberPosition::TopCenter, tl!("Top Center")),
                                (PageNumberPosition::TopRight, tl!("Top Right")),
                                (PageNumberPosition::BottomLeft, tl!("Bottom Left")),
                                (PageNumberPosition::BottomCenter, tl!("Bottom Center")),
                                (PageNumberPosition::BottomRight, tl!("Bottom Right")),
                            ];

                            egui::Grid::new("position_grid").num_columns(3).spacing(vec2(8.0, 8.0)).show(left_ui, |ui| {
                                for (i, (pos, label)) in positions.iter().enumerate() {
                                    let selected = current_pos == *pos;
                                    if crate::widgets::option_button(ui, None, label, selected, 140.0).clicked() {
                                        app.page_numbers_workspace.position = *pos;
                                    }

                                    if (i + 1) % 3 == 0 {
                                        ui.end_row();
                                    }
                                }
                            });

                            left_ui.add_space(16.0);

                            // 2. Format Selection
                            left_ui.label(egui::RichText::new(tl!("Format")).font(theme::semibold(13.5)).color(t.text));
                            left_ui.add_space(8.0);

                            let formats = [
                                (PageNumberFormatKind::NumberOnly, tl!("Number only (1, 2, 3...)")),
                                (PageNumberFormatKind::PageN, tl!("Page {n}")),
                                (PageNumberFormatKind::PageNOfTotal, tl!("Page {n} of {total}")),
                                (PageNumberFormatKind::NOfTotal, tl!("{n} / {total}")),
                                (PageNumberFormatKind::Custom, tl!("Custom")),
                            ];

                            // The choices show how the number will look on this document, not the template.
                            let total = source.pages.max(1).to_string();
                            let example = [("n", "1"), ("total", total.as_str())];
                            for (fmt_kind, label) in formats {
                                let selected = app.page_numbers_workspace.format_kind == fmt_kind;
                                let shown = crate::i18n::fmt(label, &example);
                                if crate::widgets::radio_row(left_ui, selected, &shown) {
                                    app.page_numbers_workspace.format_kind = fmt_kind;
                                }
                                left_ui.add_space(4.0);
                            }

                            if app.page_numbers_workspace.format_kind == PageNumberFormatKind::Custom {
                                left_ui.add_space(6.0);
                                left_ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(tl!("Custom format:")).font(theme::regular(12.0)).color(t.text_muted));
                                    ui.add(egui::TextEdit::singleline(&mut app.page_numbers_workspace.custom_format).desired_width(180.0));
                                });
                                left_ui.label(
                                    egui::RichText::new(tl!("Use <<1>> for page number, <<n>> for total count"))
                                        .font(theme::regular(11.0))
                                        .color(t.text_faint),
                                );
                            }

                            left_ui.add_space(16.0);

                            // 3. Page Range & Cover options
                            left_ui.label(egui::RichText::new(tl!("Page range")).font(theme::semibold(13.5)).color(t.text));
                            left_ui.add_space(8.0);

                            left_ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 18.0;
                                let all_sel = app.page_numbers_workspace.range_choice == PageNumberRangeChoice::AllPages;
                                if crate::widgets::radio_row(ui, all_sel, tl!("All pages")) {
                                    app.page_numbers_workspace.range_choice = PageNumberRangeChoice::AllPages;
                                }

                                let range_sel = app.page_numbers_workspace.range_choice == PageNumberRangeChoice::Range;
                                if crate::widgets::radio_row(ui, range_sel, tl!("Page range:")) {
                                    app.page_numbers_workspace.range_choice = PageNumberRangeChoice::Range;
                                }
                            });

                            if app.page_numbers_workspace.range_choice == PageNumberRangeChoice::Range {
                                left_ui.add_space(8.0);
                                left_ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(tl!("Pages from")).font(theme::regular(12.0)).color(t.text_muted));
                                    ui.add_space(4.0);
                                    let mut from = app.page_numbers_workspace.range_from as u32;
                                    crate::widgets::stepper_u32(ui, &mut from, 1, source.pages as u32, 1, "");
                                    app.page_numbers_workspace.range_from = from as usize;

                                    ui.add_space(8.0);
                                    ui.label(egui::RichText::new(tl!("to")).font(theme::regular(12.0)).color(t.text_muted));
                                    ui.add_space(4.0);
                                    let mut to = (app.page_numbers_workspace.range_to.max(app.page_numbers_workspace.range_from)) as u32;
                                    crate::widgets::stepper_u32(ui, &mut to, from, source.pages as u32, 1, "");
                                    app.page_numbers_workspace.range_to = to as usize;
                                });
                            }

                            left_ui.add_space(10.0);
                            crate::widgets::check_row(left_ui, &mut app.page_numbers_workspace.skip_first_page, tl!("Skip first page (cover)"));

                            left_ui.add_space(16.0);

                            // 4. Style & Typography
                            left_ui.label(egui::RichText::new(tl!("Style & Typography")).font(theme::semibold(13.5)).color(t.text));
                            left_ui.add_space(8.0);

                            left_ui.horizontal(|ui| {
                                ui.set_min_height(28.0);
                                ui.label(egui::RichText::new(tl!("Font size:")).font(theme::regular(12.5)).color(t.text_muted));
                                ui.add_space(4.0);
                                crate::widgets::stepper_f64(ui, &mut app.page_numbers_workspace.font_size, 7.0, 36.0, 1.0, "pt");
                                ui.add_space(8.0);
                                ui.add(egui::Slider::new(&mut app.page_numbers_workspace.font_size, 7.0..=36.0).show_value(false));
                            });

                            left_ui.add_space(8.0);
                            left_ui.horizontal(|ui| {
                                ui.set_min_height(28.0);
                                ui.label(egui::RichText::new(tl!("Margin (distance from edge):")).font(theme::regular(12.5)).color(t.text_muted));
                                ui.add_space(4.0);
                                crate::widgets::stepper_f64(ui, &mut app.page_numbers_workspace.margin, 10.0, 100.0, 2.0, "pt");
                                ui.add_space(8.0);
                                ui.add(egui::Slider::new(&mut app.page_numbers_workspace.margin, 10.0..=100.0).show_value(false));
                            });

                            left_ui.add_space(8.0);
                            left_ui.horizontal(|ui| {
                                ui.set_min_height(28.0);
                                ui.label(egui::RichText::new(tl!("Start numbering at:")).font(theme::regular(12.5)).color(t.text_muted));
                                ui.add_space(4.0);
                                crate::widgets::stepper_u32(ui, &mut app.page_numbers_workspace.start_number, 1, 99999, 1, "");
                            });

                            // -----------------------------------------------------------
                            // Right Column: Live Interactive Page Preview
                            // -----------------------------------------------------------
                            let right_ui = &mut cols[1];
                            right_ui.vertical_centered(|ui| {
                                ui.label(egui::RichText::new(tl!("Preview")).font(theme::semibold(13.5)).color(t.text));
                                ui.add_space(10.0);

                                // Simulated Sheet
                                let sheet_w = 230.0;
                                let sheet_h = 320.0;
                                let (rect, _) = ui.allocate_exact_size(vec2(sheet_w, sheet_h), Sense::hover());
                                let painter = ui.painter_at(rect);

                                // Drop Shadow
                                painter.rect_filled(rect.translate(vec2(3.0, 3.0)), CornerRadius::same(6), Color32::from_black_alpha(24));
                                // White Page
                                painter.rect_filled(rect, CornerRadius::same(6), Color32::WHITE);
                                painter.rect_stroke(rect, CornerRadius::same(6), Stroke::new(1.0, t.border), egui::StrokeKind::Inside);

                                // Header bar indicator
                                painter.rect_filled(
                                    Rect::from_min_size(rect.min + vec2(16.0, 16.0), vec2(50.0, 5.0)),
                                    CornerRadius::same(2),
                                    t.accent.gamma_multiply(0.4),
                                );

                                // Document Body lines (Simulated document text)
                                let line_color = Color32::from_gray(0xEA);
                                for y_step in (38..270).step_by(18) {
                                    let shrink_w = if (y_step / 18) % 3 == 0 { 40.0 } else { 20.0 };
                                    painter.hline(rect.x_range().shrink(shrink_w), rect.top() + y_step as f32, Stroke::new(2.5, line_color));
                                }

                                // Resolved template and sample text
                                let template = resolve_template(app.page_numbers_workspace.format_kind, &app.page_numbers_workspace.custom_format);
                                let sample_num = if app.page_numbers_workspace.skip_first_page {
                                    app.page_numbers_workspace.start_number + 1
                                } else {
                                    app.page_numbers_workspace.start_number
                                };
                                let total = source.pages;
                                let label_text = preview_text(&template, sample_num, total);

                                // Position calculation on sheet
                                let pos = app.page_numbers_workspace.position;
                                let margin_pt = (app.page_numbers_workspace.margin as f32 * 0.4).clamp(12.0, 32.0);
                                let font_sz = (app.page_numbers_workspace.font_size as f32).clamp(9.0, 15.0);

                                let (anchor_pos, align) = match pos {
                                    PageNumberPosition::TopLeft => (rect.min + vec2(margin_pt, margin_pt), Align2::LEFT_TOP),
                                    PageNumberPosition::TopCenter => (rect.center_top() + vec2(0.0, margin_pt), Align2::CENTER_TOP),
                                    PageNumberPosition::TopRight => (rect.right_top() + vec2(-margin_pt, margin_pt), Align2::RIGHT_TOP),
                                    PageNumberPosition::BottomLeft => (rect.left_bottom() + vec2(margin_pt, -margin_pt), Align2::LEFT_BOTTOM),
                                    PageNumberPosition::BottomCenter => (rect.center_bottom() + vec2(0.0, -margin_pt), Align2::CENTER_BOTTOM),
                                    PageNumberPosition::BottomRight => (rect.right_bottom() + vec2(-margin_pt, -margin_pt), Align2::RIGHT_BOTTOM),
                                };

                                let c = app.page_numbers_workspace.color;
                                let text_c = Color32::from_rgb((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8);

                                painter.text(anchor_pos, align, &label_text, theme::semibold(font_sz), text_c);

                                ui.add_space(14.0);
                                ui.label(egui::RichText::new(tl!("PAGE PREVIEW")).font(theme::semibold(11.0)).color(t.text_faint));
                            });
                        });

                        ui.add_space(20.0);
                        ui.separator();
                        ui.add_space(14.0);

                        // Action Buttons Row
                        ui.horizontal(|ui| {
                            if widgets::dialog_action_button(ui, "rotate-ccw", tl!("Select another file"), false).clicked() {
                                app.page_numbers_workspace.source = None;
                                app.page_numbers_workspace.error = None;
                                app.page_numbers_workspace.success_note = None;
                            }

                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if widgets::dialog_action_button(ui, "save", tl!("Save PDF with Page Numbers"), true).clicked() {
                                    app.execute_page_numbers_and_save();
                                }
                                ui.add_space(8.0);
                                if widgets::dialog_action_button(ui, "book-open", tl!("Open in KarnoxPDF"), false).clicked() {
                                    app.execute_page_numbers_and_open();
                                }
                            });
                        });
                    } else {
                        // Empty State: prompt user to pick file
                        ui.add_space(24.0);
                        ui.vertical_centered(|ui| {
                            draw_icon(ui, "hash", 46.0, t.accent);
                            ui.add_space(14.0);
                            ui.label(egui::RichText::new(tl!("Select a PDF to add page numbers")).font(theme::semibold(18.0)).color(t.text));
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(tl!(
                                    "Insert customized page numbering into headers or footers with full control over style and position."
                                ))
                                .color(t.text_muted)
                                .font(theme::regular(13.0)),
                            );
                            ui.add_space(18.0);

                            if widgets::dialog_action_button(ui, "folder-open", tl!("Open file"), true).clicked() {
                                app.page_numbers_source_dialog();
                            }

                            ui.add_space(24.0);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                feature_pill(ui, t, "hash", tl!("Custom formatting"));
                                feature_pill(ui, t, "layers", tl!("6 positions"));
                                feature_pill(ui, t, "file-check", tl!("Page range selection"));
                            });
                        });
                        ui.add_space(16.0);
                    }

                    // Display errors if any
                    if let Some(err) = &app.page_numbers_workspace.error {
                        ui.add_space(14.0);
                        egui::Frame::NONE
                            .fill(Color32::from_rgb(0xFF, 0xEB, 0xEE))
                            .stroke(Stroke::new(1.0, Color32::from_rgb(0xEF, 0x9A, 0x9A)))
                            .corner_radius(CornerRadius::same(6))
                            .inner_margin(egui::Margin::symmetric(14, 10))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    draw_icon(ui, "circle-x", 16.0, Color32::from_rgb(0xC6, 0x28, 0x28));
                                    ui.add_space(6.0);
                                    ui.label(egui::RichText::new(err).color(Color32::from_rgb(0xC6, 0x28, 0x28)).font(theme::medium(13.0)));
                                });
                            });
                    }

                    // Display success note if any
                    if let Some(note) = &app.page_numbers_workspace.success_note {
                        ui.add_space(14.0);
                        egui::Frame::NONE
                            .fill(Color32::from_rgb(0xE8, 0xF5, 0xE9))
                            .stroke(Stroke::new(1.0, Color32::from_rgb(0xA5, 0xD6, 0xA7)))
                            .corner_radius(CornerRadius::same(6))
                            .inner_margin(egui::Margin::symmetric(14, 10))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    draw_icon(ui, "check", 16.0, Color32::from_rgb(0x2E, 0x7D, 0x32));
                                    ui.add_space(6.0);
                                    ui.label(egui::RichText::new(note).color(Color32::from_rgb(0x2E, 0x7D, 0x32)).font(theme::medium(13.0)));
                                });
                            });
                    }
                });
        });
    });
}

// ---------------------------------------------------------------------------
// App Methods & Execution
// ---------------------------------------------------------------------------

impl KarnoxPDFApp {
    /// Open the Page Numbers workspace.
    pub fn open_page_numbers_workspace(&mut self) {
        if let Some(i) = self.active
            && let Some(doc) = self.session.get(self.views[i].id)
        {
            self.page_numbers_workspace.source =
                Some(PageNumbersPdfSource { name: doc.name.clone(), bytes: doc.bytes.clone(), pages: doc.info.pages.len(), path: doc.path.clone() });
            self.page_numbers_workspace.range_to = doc.info.pages.len();
        }

        self.deactivate_all_workspaces();
        self.active = None;
        self.dialog = None;
        self.page_numbers_workspace.open = true;
        self.page_numbers_workspace.active = true;
        self.page_numbers_workspace.error = None;
        self.page_numbers_workspace.success_note = None;
    }

    /// Close the Page Numbers workspace.
    pub fn close_page_numbers_workspace(&mut self) {
        self.page_numbers_workspace.active = false;
        self.page_numbers_workspace.open = false;
        self.page_numbers_workspace.source = None;
        self.page_numbers_workspace.error = None;
        self.page_numbers_workspace.success_note = None;
    }

    /// Stage a picked PDF file for page numbering.
    pub fn stage_page_numbers_source(&mut self, name: String, bytes: Vec<u8>, path: Option<String>) {
        let bytes = Arc::new(bytes);
        match karnoxpdf_render::inspect(bytes.clone(), None) {
            Ok(info) if !info.pages.is_empty() => {
                let pages = info.pages.len();
                self.page_numbers_workspace.source = Some(PageNumbersPdfSource { name, bytes, pages, path });
                self.page_numbers_workspace.range_from = 1;
                self.page_numbers_workspace.range_to = pages;
                self.page_numbers_workspace.error = None;
                self.page_numbers_workspace.success_note = None;
                self.open_page_numbers_workspace();
            }
            Ok(_) => self.notify_tr("Document has no pages."),
            Err(e) => self.notify_fmt("Couldn't open {name}: {e}", &[("name", &name), ("e", &e.to_string())]),
        }
    }

    /// Suggested filename for the numbered document.
    pub fn page_numbers_filename(&self) -> String {
        let stem =
            self.page_numbers_workspace.source.as_ref().map(|s| s.name.trim_end_matches(".pdf").trim_end_matches(".PDF")).unwrap_or("document");
        let safe: String = stem.chars().map(|ch| if ch.is_alphanumeric() || " -_.,()".contains(ch) { ch } else { '_' }).collect();
        let trimmed = safe.trim_matches([' ', '.']);
        let stem = if trimmed.is_empty() { "document" } else { trimmed };
        format!("{stem}_numbered.pdf")
    }

    /// Apply page numbering and return the output PDF bytes.
    pub fn execute_page_numbers(&mut self) -> Result<Vec<u8>, String> {
        let Some(source) = self.page_numbers_workspace.source.clone() else {
            return Err(tl!("No document loaded.").to_string());
        };

        let template = resolve_template(self.page_numbers_workspace.format_kind, &self.page_numbers_workspace.custom_format);
        if template.trim().is_empty() {
            return Err(tl!("Type a format template first.").to_string());
        }

        // Calculate target 0-based page indices
        let mut target_pages = Vec::new();
        match self.page_numbers_workspace.range_choice {
            PageNumberRangeChoice::AllPages => {
                let start_idx = if self.page_numbers_workspace.skip_first_page { 1 } else { 0 };
                for i in start_idx..source.pages {
                    target_pages.push(i);
                }
            }
            PageNumberRangeChoice::Range => {
                let from = self.page_numbers_workspace.range_from.clamp(1, source.pages) - 1;
                let to = self.page_numbers_workspace.range_to.clamp(from + 1, source.pages);
                for i in from..to {
                    if self.page_numbers_workspace.skip_first_page && i == 0 {
                        continue;
                    }
                    target_pages.push(i);
                }
            }
        }

        if target_pages.is_empty() {
            return Err(tl!("Document has no pages.").to_string());
        }

        let mut hf = HeaderFooter::default();
        let slot = self.page_numbers_workspace.position.slot();
        hf.text[slot] = template;
        hf.font_size = self.page_numbers_workspace.font_size;
        hf.color = self.page_numbers_workspace.color;
        let m = self.page_numbers_workspace.margin;
        hf.margins = [m, m, m, m];
        hf.start_number = self.page_numbers_workspace.start_number;

        let mut session = Session::new();
        let doc_id = session.open(&source.name, None, source.bytes.clone(), None).map_err(|e| e.to_string())?;

        let edit = Edit::AddHeaderFooter { pages: target_pages, settings: hf, replace: false };
        session.apply(doc_id, edit).map_err(|e| e.to_string())?;

        session.save_bytes(doc_id).map(|b| (*b).clone()).map_err(|e| e.to_string())
    }

    /// Execute page numbering and save to disk.
    pub fn execute_page_numbers_and_save(&mut self) {
        let bytes = match self.execute_page_numbers() {
            Ok(b) => Arc::new(b),
            Err(err) => {
                self.page_numbers_workspace.error = Some(err);
                return;
            }
        };

        self.page_numbers_workspace.error = None;
        let filename = self.page_numbers_filename();

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
                        app.page_numbers_workspace.success_note = Some(tl!("Page numbers added successfully!").to_string());
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
        let _ = crate::editing::download(&filename, &bytes);
    }

    /// Execute page numbering and open directly in KarnoxPDF tabs.
    pub fn execute_page_numbers_and_open(&mut self) {
        let bytes = match self.execute_page_numbers() {
            Ok(b) => b,
            Err(err) => {
                self.page_numbers_workspace.error = Some(err);
                return;
            }
        };

        let filename = self.page_numbers_filename();
        let _ = self.open_bytes(&filename, None, bytes);
        self.close_page_numbers_workspace();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_templates_resolve_and_preview() {
        assert_eq!(resolve_template(PageNumberFormatKind::NumberOnly, ""), "<<1>>");
        assert_eq!(preview_text("<<1>>", 5, 20), "5");
        assert_eq!(preview_text("<<1>> / <<n>>", 5, 20), "5 / 20");
        assert_eq!(preview_text("Pág. <<1>> de <<n>>", 1, 10), "Pág. 1 de 10");
    }

    #[test]
    fn page_number_workspace_defaults() {
        let ws = PageNumbersWorkspace::default();
        assert_eq!(ws.position, PageNumberPosition::BottomCenter);
        assert_eq!(ws.format_kind, PageNumberFormatKind::NumberOnly);
        assert_eq!(ws.start_number, 1);
        assert!(!ws.skip_first_page);
    }
}
