//! Rotate PDF workspace: Rotate all or specific pages of a PDF document,
//! with live interactive preview, direction buttons, parity/orientation filters, and direct save.

use std::sync::Arc;

use egui::{Align, Align2, Color32, CornerRadius, Layout, Rect, Sense, Stroke, vec2};
use karnoxpdf_engine::{Edit, Session};
use karnoxpdf_render::PageInfo;

use crate::panels::human_size;
use crate::theme::{self, Tokens};
use crate::{KarnoxPDFApp, icons, widgets};

/// Rotation direction choices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RotateDirection {
    Clockwise90,
    CounterClockwise90,
    Flip180,
}

impl RotateDirection {
    pub fn degrees(self) -> i64 {
        match self {
            Self::Clockwise90 => 90,
            Self::CounterClockwise90 => -90,
            Self::Flip180 => 180,
        }
    }
}

/// Which pages to rotate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RotatePagesMode {
    All,
    Range,
    FirstPage,
    LastPage,
}

/// Parity filter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageParityFilter {
    All,
    EvenOnly,
    OddOnly,
}

/// Orientation filter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageOrientationFilter {
    All,
    PortraitOnly,
    LandscapeOnly,
}

/// Source document to rotate.
#[derive(Clone, Debug)]
pub struct RotatePdfSource {
    pub name: String,
    pub bytes: Arc<Vec<u8>>,
    pub pages: Vec<PageInfo>,
    pub path: Option<String>,
}

/// State for the Rotate PDF workspace.
#[derive(Clone, Debug)]
pub struct RotatePdfWorkspace {
    pub open: bool,
    pub active: bool,
    pub source: Option<RotatePdfSource>,

    pub direction: RotateDirection,
    /// Accumulated rotation angle (0, 90, 180, 270) for the live preview.
    pub preview_angle: i64,

    pub pages_mode: RotatePagesMode,
    pub from_page: usize,
    pub to_page: usize,
    pub parity: PageParityFilter,
    pub orientation: PageOrientationFilter,

    pub error: Option<String>,
}

impl Default for RotatePdfWorkspace {
    fn default() -> Self {
        Self {
            open: false,
            active: false,
            source: None,
            direction: RotateDirection::Clockwise90,
            preview_angle: 90,
            pages_mode: RotatePagesMode::All,
            from_page: 1,
            to_page: 1,
            parity: PageParityFilter::All,
            orientation: PageOrientationFilter::All,
            error: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Custom UI Controls (Tactile Buttons & Chips)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Workspace UI
// ---------------------------------------------------------------------------

pub(crate) fn workspace(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, _t: &Tokens) {
    show(app, ui);
}

pub fn show(app: &mut KarnoxPDFApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::NONE.inner_margin(egui::Margin { left: 40, right: 40, top: 20, bottom: 40 }).show(ui, |ui| {
            // Header
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new(tl!("Rotate PDF")).font(theme::semibold(24.0)).color(t.text));
                ui.add_space(4.0);
                ui.label(egui::RichText::new(tl!("Rotate one PDF or several PDF files at once.")).color(t.text_muted).font(theme::regular(13.0)));
            });
            ui.add_space(20.0);

            // Main container card
            egui::Frame::NONE
                .fill(t.card)
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(CornerRadius::same(12))
                .inner_margin(egui::Margin::same(24))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width().min(850.0));

                    ui.label(egui::RichText::new(tl!("Source PDF")).font(theme::semibold(15.0)).color(t.text));
                    ui.add_space(10.0);

                    let source = app.rotate_workspace.source.clone();
                    if let Some(source) = source.as_ref() {
                        let total_pages = source.pages.len();

                        // Source document summary card
                        egui::Frame::NONE.fill(t.field).corner_radius(CornerRadius::same(7)).inner_margin(egui::Margin::symmetric(14, 12)).show(
                            ui,
                            |ui| {
                                ui.horizontal(|ui| {
                                    ui.add(icons::image("file-text", 20.0, t.accent));
                                    ui.add_space(6.0);
                                    ui.add(egui::Label::new(egui::RichText::new(&source.name).font(theme::medium(14.0)).color(t.text)).truncate());
                                    let pages_str = if total_pages == 1 {
                                        tl!("1 page").to_string()
                                    } else {
                                        crate::i18n::fmt(tl!("{n} pages"), &[("n", &total_pages.to_string())])
                                    };
                                    ui.label(egui::RichText::new(format!("{pages_str} · {}", human_size(source.bytes.len()))).color(t.text_muted));
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if widgets::dialog_action_button(ui, "folder-open", tl!("Change file"), false).clicked() {
                                            app.rotate_source_dialog();
                                        }
                                    });
                                });
                            },
                        );

                        ui.add_space(20.0);
                        ui.separator();
                        ui.add_space(16.0);

                        // Content Layout: Left (Controls) & Right (Interactive Preview)
                        ui.horizontal_top(|ui| {
                            // Left Column: Rotation & Pages Controls
                            ui.vertical(|ui| {
                                ui.set_max_width(490.0);

                                // 1. Rotation Direction Choice
                                ui.label(egui::RichText::new(tl!("Rotation direction")).font(theme::semibold(14.0)).color(t.text));
                                ui.add_space(8.0);
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing.x = 8.0;
                                    let is_cw = app.rotate_workspace.direction == RotateDirection::Clockwise90;
                                    if crate::widgets::option_button(ui, Some("rotate-cw"), tl!("Clockwise 90° (Right)"), is_cw, 140.0).clicked() {
                                        app.rotate_workspace.direction = RotateDirection::Clockwise90;
                                        app.rotate_workspace.preview_angle = 90;
                                    }
                                    let is_ccw = app.rotate_workspace.direction == RotateDirection::CounterClockwise90;
                                    if crate::widgets::option_button(ui, Some("rotate-ccw"), tl!("Counterclockwise 90° (Left)"), is_ccw, 140.0)
                                        .clicked()
                                    {
                                        app.rotate_workspace.direction = RotateDirection::CounterClockwise90;
                                        app.rotate_workspace.preview_angle = 270;
                                    }
                                    let is_flip = app.rotate_workspace.direction == RotateDirection::Flip180;
                                    if crate::widgets::option_button(ui, Some("replace"), tl!("180° (Flip)"), is_flip, 110.0).clicked() {
                                        app.rotate_workspace.direction = RotateDirection::Flip180;
                                        app.rotate_workspace.preview_angle = 180;
                                    }
                                });

                                ui.add_space(18.0);
                                ui.separator();
                                ui.add_space(14.0);

                                // 2. Pages to Rotate (Mode selection)
                                ui.label(egui::RichText::new(tl!("Page Range Options")).font(theme::semibold(14.0)).color(t.text));
                                ui.add_space(8.0);

                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing.x = 8.0;
                                    let m = app.rotate_workspace.pages_mode;
                                    if crate::widgets::option_button(ui, Some("copy"), tl!("All pages"), m == RotatePagesMode::All, 100.0).clicked() {
                                        app.rotate_workspace.pages_mode = RotatePagesMode::All;
                                    }
                                    if crate::widgets::option_button(ui, Some("file-text"), tl!("Page range"), m == RotatePagesMode::Range, 110.0)
                                        .clicked()
                                    {
                                        app.rotate_workspace.pages_mode = RotatePagesMode::Range;
                                        if app.rotate_workspace.to_page == 1 && app.rotate_workspace.from_page == 1 {
                                            app.rotate_workspace.to_page = total_pages.max(1);
                                        }
                                    }
                                    if crate::widgets::option_button(
                                        ui,
                                        Some("chevron-up"),
                                        tl!("First page only"),
                                        m == RotatePagesMode::FirstPage,
                                        110.0,
                                    )
                                    .clicked()
                                    {
                                        app.rotate_workspace.pages_mode = RotatePagesMode::FirstPage;
                                    }
                                    if crate::widgets::option_button(
                                        ui,
                                        Some("chevron-down"),
                                        tl!("Last page only"),
                                        m == RotatePagesMode::LastPage,
                                        110.0,
                                    )
                                    .clicked()
                                    {
                                        app.rotate_workspace.pages_mode = RotatePagesMode::LastPage;
                                    }
                                });

                                if app.rotate_workspace.pages_mode == RotatePagesMode::Range {
                                    ui.add_space(8.0);
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new(tl!("Pages from")).color(t.text_muted));
                                        let max = total_pages.max(1) as u32;
                                        let mut from = app.rotate_workspace.from_page.clamp(1, max as usize) as u32;
                                        crate::widgets::stepper_u32(ui, &mut from, 1, max, 1, "");
                                        app.rotate_workspace.from_page = from as usize;
                                        ui.label(tl!("to"));
                                        let mut to = app.rotate_workspace.to_page.clamp(1, max as usize) as u32;
                                        crate::widgets::stepper_u32(ui, &mut to, 1, max, 1, "");
                                        app.rotate_workspace.to_page = to as usize;
                                        ui.label(egui::RichText::new(format!("({})", total_pages)).color(t.text_muted).small());
                                    });
                                }

                                ui.add_space(14.0);

                                // 3. Filters: Parity & Orientation
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(tl!("Parity:")).color(t.text_muted));
                                    ui.add_space(4.0);
                                    for (p, lbl) in [
                                        (PageParityFilter::All, tl!("All")),
                                        (PageParityFilter::EvenOnly, tl!("Even pages only")),
                                        (PageParityFilter::OddOnly, tl!("Odd pages only")),
                                    ] {
                                        let is_sel = app.rotate_workspace.parity == p;
                                        if crate::widgets::chip_button(ui, lbl, is_sel).clicked() {
                                            app.rotate_workspace.parity = p;
                                        }
                                    }
                                });

                                ui.add_space(10.0);

                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(tl!("Orientation:")).color(t.text_muted));
                                    ui.add_space(4.0);
                                    for (o, lbl) in [
                                        (PageOrientationFilter::All, tl!("All")),
                                        (PageOrientationFilter::PortraitOnly, tl!("Portrait")),
                                        (PageOrientationFilter::LandscapeOnly, tl!("Landscape")),
                                    ] {
                                        let is_sel = app.rotate_workspace.orientation == o;
                                        if crate::widgets::chip_button(ui, lbl, is_sel).clicked() {
                                            app.rotate_workspace.orientation = o;
                                        }
                                    }
                                });
                            });

                            ui.add_space(28.0);

                            // Right Column: Interactive Visual Sheet Preview
                            ui.vertical_centered(|ui| {
                                ui.label(egui::RichText::new(tl!("Preview")).font(theme::medium(13.0)).color(t.text_muted));
                                ui.add_space(8.0);

                                let norm_angle = ((app.rotate_workspace.preview_angle % 360) + 360) % 360;
                                let is_landscape = norm_angle == 90 || norm_angle == 270;

                                let (preview_w, preview_h) = if is_landscape { (210.0, 150.0) } else { (150.0, 210.0) };

                                let (paper_rect, _) = ui.allocate_exact_size(vec2(220.0, 220.0), Sense::hover());
                                let sheet_rect = Rect::from_center_size(paper_rect.center(), vec2(preview_w, preview_h));
                                let painter = ui.painter_at(paper_rect);

                                // Paper drop shadow & background
                                painter.rect_filled(sheet_rect.translate(vec2(3.0, 3.0)), CornerRadius::same(5), Color32::from_black_alpha(25));
                                painter.rect_filled(sheet_rect, CornerRadius::same(5), Color32::WHITE);
                                painter.rect_stroke(sheet_rect, CornerRadius::same(5), Stroke::new(1.2, t.border), egui::StrokeKind::Inside);

                                // Simulated lines of text on paper
                                let line_color = Color32::from_gray(0xDD);
                                let header_color = t.accent_soft;

                                // Header strip on paper
                                let header_rect =
                                    Rect::from_min_size(sheet_rect.min + vec2(14.0, 14.0), vec2((sheet_rect.width() - 28.0).max(10.0), 12.0));
                                painter.rect_filled(header_rect, CornerRadius::same(2), header_color);

                                // Body lines
                                let start_y = sheet_rect.top() + 36.0;
                                let end_y = sheet_rect.bottom() - 16.0;
                                let mut y = start_y;
                                while y < end_y {
                                    painter.hline(sheet_rect.x_range().shrink(14.0), y, Stroke::new(2.0, line_color));
                                    y += 14.0;
                                }

                                // Orientation badge in center of preview sheet
                                let badge_rect = Rect::from_center_size(sheet_rect.center(), vec2(70.0, 24.0));
                                painter.rect_filled(badge_rect, CornerRadius::same(12), Color32::from_black_alpha(180));
                                painter.text(
                                    badge_rect.center(),
                                    Align2::CENTER_CENTER,
                                    format!("{}°", norm_angle),
                                    theme::semibold(12.0),
                                    Color32::WHITE,
                                );

                                ui.add_space(8.0);

                                // Interactive Rotate Buttons below preview
                                ui.horizontal(|ui| {
                                    if crate::widgets::option_button(ui, Some("rotate-ccw"), tl!("Rotate left"), false, 95.0).clicked() {
                                        app.rotate_workspace.preview_angle = (app.rotate_workspace.preview_angle - 90 + 360) % 360;
                                        app.rotate_workspace.direction = match app.rotate_workspace.preview_angle {
                                            90 => RotateDirection::Clockwise90,
                                            180 => RotateDirection::Flip180,
                                            270 => RotateDirection::CounterClockwise90,
                                            _ => RotateDirection::Clockwise90,
                                        };
                                    }
                                    ui.add_space(6.0);
                                    if crate::widgets::option_button(ui, Some("rotate-cw"), tl!("Rotate right"), false, 95.0).clicked() {
                                        app.rotate_workspace.preview_angle = (app.rotate_workspace.preview_angle + 90) % 360;
                                        app.rotate_workspace.direction = match app.rotate_workspace.preview_angle {
                                            90 => RotateDirection::Clockwise90,
                                            180 => RotateDirection::Flip180,
                                            270 => RotateDirection::CounterClockwise90,
                                            _ => RotateDirection::Clockwise90,
                                        };
                                    }
                                });

                                ui.add_space(6.0);
                                let orient_name = if is_landscape { tl!("Landscape") } else { tl!("Portrait") };
                                ui.label(egui::RichText::new(orient_name).color(t.text_muted).small());
                            });
                        });

                        ui.add_space(20.0);

                        if let Some(err) = &app.rotate_workspace.error {
                            ui.label(egui::RichText::new(err).color(Color32::from_rgb(0xD7, 0x37, 0x3F)));
                            ui.add_space(10.0);
                        }

                        // Bottom Action Buttons
                        ui.horizontal(|ui| {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if widgets::pill_button(ui, tl!("Rotate and save PDF"), true).clicked() {
                                    app.execute_rotate_and_save();
                                }
                                if widgets::pill_button(ui, tl!("Close"), false).clicked() {
                                    app.close_rotate_workspace();
                                }
                            });
                        });
                    } else {
                        // Empty state: select PDF
                        ui.add_space(20.0);
                        ui.vertical_centered(|ui| {
                            ui.add(icons::image("rotate-cw", 38.0, t.accent));
                            ui.add_space(10.0);
                            ui.label(egui::RichText::new(tl!("Select a PDF to rotate")).font(theme::semibold(18.0)).color(t.text));
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new(tl!("Choose a PDF document to rotate pages.")).color(t.text_muted));
                            ui.add_space(14.0);
                            if widgets::dialog_action_button(ui, "folder-open", tl!("Open file"), true).clicked() {
                                app.rotate_source_dialog();
                            }
                        });
                        ui.add_space(24.0);
                    }
                });
        });
    });
}

// ---------------------------------------------------------------------------
// App Methods & Execution
// ---------------------------------------------------------------------------

impl KarnoxPDFApp {
    pub fn open_rotate_workspace(&mut self) {
        if let Some(i) = self.active
            && let Some(doc) = self.session.get(self.views[i].id)
        {
            self.rotate_workspace.source =
                Some(RotatePdfSource { name: doc.name.clone(), bytes: doc.bytes.clone(), pages: doc.info.pages.clone(), path: doc.path.clone() });
        }

        self.deactivate_all_workspaces();
        self.rotate_workspace.open = true;
        self.rotate_workspace.active = true;
        self.active = None;
        self.dialog = None;
    }

    pub(crate) fn close_rotate_workspace(&mut self) {
        let was_active = self.rotate_workspace.active;
        self.rotate_workspace.open = false;
        self.rotate_workspace.active = false;
        if was_active {
            self.active = self.views.len().checked_sub(1);
        }
    }

    pub(crate) fn stage_rotate_source(&mut self, name: String, bytes: Vec<u8>, path: Option<String>) {
        let bytes = Arc::new(bytes);
        match karnoxpdf_render::inspect(bytes.clone(), None) {
            Ok(info) if !info.pages.is_empty() => {
                self.rotate_workspace.source = Some(RotatePdfSource { name, bytes, pages: info.pages, path });
                self.open_rotate_workspace();
            }
            Ok(_) => self.notify_tr("The PDF has no pages to convert."),
            Err(e) => self.notify_fmt("Couldn't open {name}: {e}", &[("name", &name), ("e", &e.to_string())]),
        }
    }

    /// Calculate target pages to rotate based on mode, parity and orientation.
    pub(crate) fn resolve_rotate_pages(
        mode: RotatePagesMode,
        from_page: usize,
        to_page: usize,
        parity: PageParityFilter,
        orientation: PageOrientationFilter,
        pages: &[PageInfo],
    ) -> Vec<usize> {
        let total = pages.len();
        let candidate_indices: Vec<usize> = match mode {
            RotatePagesMode::All => (0..total).collect(),
            RotatePagesMode::Range => {
                let from = from_page.clamp(1, total.max(1)) - 1;
                let to = to_page.clamp(1, total.max(1)) - 1;
                let (start, end) = if from <= to { (from, to) } else { (to, from) };
                (start..=end).collect()
            }
            RotatePagesMode::FirstPage => {
                if total > 0 {
                    vec![0]
                } else {
                    vec![]
                }
            }
            RotatePagesMode::LastPage => {
                if total > 0 {
                    vec![total - 1]
                } else {
                    vec![]
                }
            }
        };

        candidate_indices
            .into_iter()
            .filter(|&idx| {
                // 1-based page number for parity
                let page_num = idx + 1;
                let parity_match = match parity {
                    PageParityFilter::All => true,
                    PageParityFilter::EvenOnly => page_num % 2 == 0,
                    PageParityFilter::OddOnly => page_num % 2 == 1,
                };
                if !parity_match {
                    return false;
                }

                // Orientation match
                if let Some(info) = pages.get(idx) {
                    let is_portrait = info.width <= info.height;
                    match orientation {
                        PageOrientationFilter::All => true,
                        PageOrientationFilter::PortraitOnly => is_portrait,
                        PageOrientationFilter::LandscapeOnly => !is_portrait,
                    }
                } else {
                    true
                }
            })
            .collect()
    }

    /// Execute page rotation and save directly to PDF.
    pub fn execute_rotate_and_save(&mut self) {
        let Some(source) = self.rotate_workspace.source.clone() else {
            self.rotate_workspace.error = Some(tl!("No document to rotate.").to_string());
            return;
        };

        let target_pages = Self::resolve_rotate_pages(
            self.rotate_workspace.pages_mode,
            self.rotate_workspace.from_page,
            self.rotate_workspace.to_page,
            self.rotate_workspace.parity,
            self.rotate_workspace.orientation,
            &source.pages,
        );

        if target_pages.is_empty() {
            self.rotate_workspace.error = Some(tl!("Select at least one page.").to_string());
            return;
        }

        let degrees = self.rotate_workspace.direction.degrees();

        let stem = source.name.trim_end_matches(".pdf").trim_end_matches(".PDF");
        let safe: String = stem.chars().map(|ch| if ch.is_alphanumeric() || " -_.,()".contains(ch) { ch } else { '_' }).collect();
        let trimmed = safe.trim_matches([' ', '.']);
        let stem = if trimmed.is_empty() { "document" } else { trimmed };
        let filename = format!("{stem}_rotated.pdf");

        let mut session = Session::new();
        let doc_id = match session.open(&source.name, None, source.bytes.clone(), None) {
            Ok(id) => id,
            Err(e) => {
                self.rotate_workspace.error = Some(e.to_string());
                return;
            }
        };

        if let Err(e) = session.apply(doc_id, Edit::RotatePages { pages: target_pages, degrees }) {
            self.rotate_workspace.error = Some(e.to_string());
            return;
        }

        let result = session.save_bytes(doc_id).map(|b| (*b).clone()).map_err(|e| e.to_string());

        match result {
            Ok(bytes) => {
                self.rotate_workspace.error = None;
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
                                app.rotate_workspace.source = None;
                                app.close_rotate_workspace();
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
                            self.rotate_workspace.source = None;
                            self.close_rotate_workspace();
                        }
                        Err(e) => self.notify_fmt("Couldn't download {name}: {e}", &[("name", &filename), ("e", &e.to_string())]),
                    }
                }
            }
            Err(e) => {
                self.rotate_workspace.error = Some(e);
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
    fn rotate_workspace_defaults() {
        let ws = RotatePdfWorkspace::default();
        assert!(!ws.open);
        assert!(!ws.active);
        assert_eq!(ws.direction, RotateDirection::Clockwise90);
        assert_eq!(ws.preview_angle, 90);
        assert_eq!(ws.pages_mode, RotatePagesMode::All);
        assert_eq!(ws.parity, PageParityFilter::All);
        assert_eq!(ws.orientation, PageOrientationFilter::All);
    }

    #[test]
    fn resolve_rotate_pages_filters() {
        let dummy_pages = vec![
            PageInfo { width: 600.0, height: 800.0, label: "1".into(), crop: [0.0, 0.0, 600.0, 800.0], rotation: 0 }, // Portrait, odd
            PageInfo { width: 800.0, height: 600.0, label: "2".into(), crop: [0.0, 0.0, 800.0, 600.0], rotation: 0 }, // Landscape, even
            PageInfo { width: 600.0, height: 800.0, label: "3".into(), crop: [0.0, 0.0, 600.0, 800.0], rotation: 0 }, // Portrait, odd
            PageInfo { width: 600.0, height: 800.0, label: "4".into(), crop: [0.0, 0.0, 600.0, 800.0], rotation: 0 }, // Portrait, even
        ];

        // All pages
        let p = KarnoxPDFApp::resolve_rotate_pages(RotatePagesMode::All, 1, 4, PageParityFilter::All, PageOrientationFilter::All, &dummy_pages);
        assert_eq!(p, vec![0, 1, 2, 3]);

        // Even pages only
        let p = KarnoxPDFApp::resolve_rotate_pages(RotatePagesMode::All, 1, 4, PageParityFilter::EvenOnly, PageOrientationFilter::All, &dummy_pages);
        assert_eq!(p, vec![1, 3]);

        // Landscape only
        let p =
            KarnoxPDFApp::resolve_rotate_pages(RotatePagesMode::All, 1, 4, PageParityFilter::All, PageOrientationFilter::LandscapeOnly, &dummy_pages);
        assert_eq!(p, vec![1]);

        // Portrait and odd only
        let p = KarnoxPDFApp::resolve_rotate_pages(
            RotatePagesMode::All,
            1,
            4,
            PageParityFilter::OddOnly,
            PageOrientationFilter::PortraitOnly,
            &dummy_pages,
        );
        assert_eq!(p, vec![0, 2]);

        // First page only
        let p = KarnoxPDFApp::resolve_rotate_pages(RotatePagesMode::FirstPage, 1, 4, PageParityFilter::All, PageOrientationFilter::All, &dummy_pages);
        assert_eq!(p, vec![0]);

        // Last page only
        let p = KarnoxPDFApp::resolve_rotate_pages(RotatePagesMode::LastPage, 1, 4, PageParityFilter::All, PageOrientationFilter::All, &dummy_pages);
        assert_eq!(p, vec![3]);
    }
}
