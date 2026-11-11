//! Sign PDF workspace: Sign with a digital certificate or a simple signature
//! (drawn, typed, or image stamp), and save directly to PDF.

use std::sync::Arc;

use egui::{Align, Align2, Color32, CornerRadius, Layout, Pos2, Sense, Stroke, pos2, vec2};
use karnoxpdf_engine::sign::{self, Appearance, SignError};
use karnoxpdf_engine::{Edit, NewAnnotation, Session, Shape, SignOptions, Style};

use crate::panels::human_size;
use crate::theme::{self, Tokens};
use crate::{KarnoxPDFApp, icons, widgets};

/// Mode of signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignKind {
    /// Cryptographic PAdES digital certificate signature (.pfx / .p12).
    Digital,
    /// Simple signature (handwritten ink, typed calligraphy, or image).
    Simple,
}

/// Type of simple signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimpleType {
    Draw,
    Type,
    Image,
}

/// Page target where signature will be placed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageTarget {
    First,
    Last,
    Specific,
}

/// Placement position on the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignPlacement {
    BottomRight,
    BottomCenter,
    BottomLeft,
    TopRight,
    Invisible,
}

/// Staged PDF source to sign.
#[derive(Clone, Debug)]
pub struct SignPdfSource {
    pub name: String,
    pub bytes: Arc<Vec<u8>>,
    pub pages: usize,
    pub path: Option<String>,
}

/// State for the Sign PDF workspace.
#[derive(Clone, Debug)]
pub struct SignPdfWorkspace {
    pub open: bool,
    pub active: bool,
    pub source: Option<SignPdfSource>,
    pub kind: SignKind,

    // Digital certificate options
    pub cert_name: Option<String>,
    pub cert_bytes: Option<Vec<u8>>,
    pub cert_password: String,
    pub reason: String,
    pub location: String,
    pub certify: bool,

    // Simple signature options
    pub simple_type: SimpleType,
    pub strokes: Vec<Vec<[f32; 2]>>,
    pub typed_name: String,
    pub image_name: Option<String>,
    pub image_bytes: Option<Vec<u8>>,
    pub include_date: bool,

    // Page and placement
    pub page_target: PageTarget,
    pub page_number: usize,
    pub placement: SignPlacement,

    pub error: Option<String>,
}

impl Default for SignPdfWorkspace {
    fn default() -> Self {
        Self {
            open: false,
            active: false,
            source: None,
            kind: SignKind::Digital,
            cert_name: None,
            cert_bytes: None,
            cert_password: String::new(),
            reason: String::new(),
            location: String::new(),
            certify: false,
            simple_type: SimpleType::Draw,
            strokes: Vec::new(),
            typed_name: String::new(),
            image_name: None,
            image_bytes: None,
            include_date: true,
            page_target: PageTarget::Last,
            page_number: 1,
            placement: SignPlacement::BottomRight,
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
            ui.heading(egui::RichText::new(tl!("Sign PDF")).font(theme::semibold(28.0)).color(t.text));
            ui.add_space(6.0);
            ui.label(egui::RichText::new(tl!("Sign a PDF or request electronic signatures from others.")).color(t.text_muted));
            ui.add_space(24.0);

            egui::Frame::NONE
                .fill(t.card)
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(CornerRadius::same(12))
                .inner_margin(egui::Margin::same(24))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width().min(850.0));

                    // Source PDF header
                    ui.label(egui::RichText::new(tl!("Source PDF")).font(theme::semibold(15.0)).color(t.text));
                    ui.add_space(10.0);

                    let source = app.sign_workspace.source.clone();
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
                                            app.sign_pdf_source_dialog();
                                        }
                                    });
                                });
                            },
                        );

                        ui.add_space(20.0);
                        ui.separator();
                        ui.add_space(16.0);

                        // Mode Selector (Digital vs Simple)
                        ui.label(egui::RichText::new(tl!("Signature type")).font(theme::semibold(15.0)).color(t.text));
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            let mut is_digital = app.sign_workspace.kind == SignKind::Digital;
                            if crate::widgets::radio_row(ui, is_digital, tl!("Digital certificate signature")) {
                                app.sign_workspace.kind = SignKind::Digital;
                            }
                            is_digital = app.sign_workspace.kind == SignKind::Simple;
                            if crate::widgets::radio_row(ui, is_digital, tl!("Simple signature (drawn or typed)")) {
                                app.sign_workspace.kind = SignKind::Simple;
                            }
                        });

                        ui.add_space(16.0);

                        match app.sign_workspace.kind {
                            SignKind::Digital => digital_options(ui, app, t),
                            SignKind::Simple => simple_options(ui, app, t),
                        }

                        ui.add_space(18.0);
                        ui.separator();
                        ui.add_space(14.0);

                        // Placement / Page section
                        placement_section(ui, app, t, source.pages);

                        ui.add_space(20.0);

                        if let Some(err) = &app.sign_workspace.error {
                            ui.label(egui::RichText::new(err).color(Color32::from_rgb(0xD7, 0x37, 0x3F)));
                            ui.add_space(10.0);
                        }

                        // Bottom Action Buttons
                        ui.horizontal(|ui| {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if widgets::pill_button(ui, tl!("Sign and save PDF"), true).clicked() {
                                    app.execute_sign_and_save();
                                }
                                if widgets::pill_button(ui, tl!("Close"), false).clicked() {
                                    app.close_sign_pdf_workspace();
                                }
                            });
                        });
                    } else {
                        // Empty state: select PDF
                        ui.add_space(20.0);
                        ui.vertical_centered(|ui| {
                            ui.add(icons::image("pen-tool", 38.0, t.accent));
                            ui.add_space(10.0);
                            ui.label(egui::RichText::new(tl!("Select a PDF to sign")).font(theme::semibold(18.0)).color(t.text));
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new(tl!("Choose a PDF document to apply a digital or simple signature.")).color(t.text_muted));
                            ui.add_space(14.0);
                            if widgets::dialog_action_button(ui, "folder-open", tl!("Open file"), true).clicked() {
                                app.sign_pdf_source_dialog();
                            }
                        });
                        ui.add_space(24.0);
                    }
                });
        });
    });
}

// ---------------------------------------------------------------------------
// Digital Certificate Options
// ---------------------------------------------------------------------------

fn digital_options(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens) {
    egui::Frame::NONE
        .fill(t.field)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(tl!("Digital Certificate (.pfx / .p12)")).font(theme::semibold(14.0)).color(t.text));
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if let Some(name) = &app.sign_workspace.cert_name {
                    ui.add(icons::image("badge-check", 18.0, t.accent));
                    ui.label(egui::RichText::new(name).font(theme::medium(13.0)).color(t.text));
                    if crate::widgets::action_button(ui, tl!("Change"), false).clicked() {
                        app.sign_cert_dialog();
                    }
                } else {
                    if widgets::dialog_action_button(ui, "folder-open", tl!("Select certificate file…"), false).clicked() {
                        app.sign_cert_dialog();
                    }
                    if !app.digital_ids.is_empty() {
                        ui.add_space(10.0);
                        ui.label(egui::RichText::new(tl!("or choose existing:")).color(t.text_muted));
                        for id in &app.digital_ids {
                            if crate::widgets::action_button(ui, &id.name, false).clicked()
                                && let Ok(bytes) = std::fs::read(&id.path)
                            {
                                app.sign_workspace.cert_name = Some(id.name.clone());
                                app.sign_workspace.cert_bytes = Some(bytes);
                            }
                        }
                    }
                }
            });

            ui.add_space(10.0);
            egui::Grid::new("cert-grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                let l = ui.label(tl!("Certificate password:"));
                ui.add(egui::TextEdit::singleline(&mut app.sign_workspace.cert_password).password(true).desired_width(220.0)).labelled_by(l.id);
                ui.end_row();

                let l = ui.label(tl!("Reason (optional):"));
                ui.add(
                    egui::TextEdit::singleline(&mut app.sign_workspace.reason).hint_text(tl!("e.g. I approve this document")).desired_width(260.0),
                )
                .labelled_by(l.id);
                ui.end_row();

                let l = ui.label(tl!("Location (optional):"));
                ui.add(egui::TextEdit::singleline(&mut app.sign_workspace.location).hint_text(tl!("e.g. City, Country")).desired_width(260.0))
                    .labelled_by(l.id);
                ui.end_row();
            });

            ui.add_space(6.0);
            crate::widgets::check_row(ui, &mut app.sign_workspace.certify, tl!("Lock document after signing (prevent future changes)"));
        });
}

// ---------------------------------------------------------------------------
// Simple Signature Options
// ---------------------------------------------------------------------------

fn simple_options(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens) {
    egui::Frame::NONE
        .fill(t.field)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for (kind, label, icon) in
                    [(SimpleType::Draw, tl!("Draw"), "pen-line"), (SimpleType::Type, tl!("Type"), "type"), (SimpleType::Image, tl!("Image"), "image")]
                {
                    if crate::widgets::option_button(ui, Some(icon), label, app.sign_workspace.simple_type == kind, 0.0).clicked() {
                        app.sign_workspace.simple_type = kind;
                    }
                }
            });

            ui.add_space(10.0);

            match app.sign_workspace.simple_type {
                SimpleType::Draw => {
                    ui.label(egui::RichText::new(tl!("Draw your signature below:")).color(t.text_muted).small());
                    ui.add_space(4.0);

                    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width().min(500.0), 140.0), Sense::drag());
                    let painter = ui.painter_at(rect);
                    painter.rect_filled(rect, CornerRadius::same(6), Color32::WHITE);
                    painter.rect_stroke(rect, CornerRadius::same(6), Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
                    painter.hline(rect.x_range().shrink(20.0), rect.bottom() - 30.0, Stroke::new(1.0, Color32::from_gray(0xE0)));

                    let norm = |p: Pos2| [(p.x - rect.left()) / rect.width(), (rect.bottom() - p.y) / rect.width()];
                    let strokes = &mut app.sign_workspace.strokes;
                    if resp.drag_started() {
                        strokes.push(Vec::new());
                    }
                    if resp.dragged()
                        && let (Some(p), Some(s)) = (resp.interact_pointer_pos(), strokes.last_mut())
                    {
                        let n = norm(p.clamp(rect.min, rect.max));
                        if s.last().is_none_or(|l| (l[0] - n[0]).abs() + (l[1] - n[1]).abs() > 0.002) {
                            s.push(n);
                        }
                    }
                    for s in strokes.iter() {
                        let pts: Vec<Pos2> = s.iter().map(|p| pos2(rect.left() + p[0] * rect.width(), rect.bottom() - p[1] * rect.width())).collect();
                        painter.add(egui::Shape::line(pts, Stroke::new(2.0, Color32::BLACK)));
                    }

                    ui.add_space(6.0);
                    if crate::widgets::action_button(ui, tl!("Clear"), false).clicked() {
                        app.sign_workspace.strokes.clear();
                    }
                }
                SimpleType::Type => {
                    ui.label(egui::RichText::new(tl!("Type your name:")).color(t.text_muted).small());
                    ui.add_space(4.0);
                    ui.add(egui::TextEdit::singleline(&mut app.sign_workspace.typed_name).desired_width(320.0));
                    ui.add_space(8.0);
                    if !app.sign_workspace.typed_name.trim().is_empty() {
                        let preview_rect = ui.allocate_exact_size(vec2(ui.available_width().min(400.0), 60.0), Sense::hover()).0;
                        ui.painter().rect_filled(preview_rect, CornerRadius::same(4), Color32::WHITE);
                        ui.painter().rect_stroke(preview_rect, CornerRadius::same(4), Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
                        ui.painter().text(
                            preview_rect.center(),
                            Align2::CENTER_CENTER,
                            &app.sign_workspace.typed_name,
                            theme::semibold(22.0),
                            Color32::from_rgb(0x1A, 0x36, 0x5D),
                        );
                    }
                }
                SimpleType::Image => {
                    ui.label(egui::RichText::new(tl!("Upload signature image (PNG, JPG):")).color(t.text_muted).small());
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if let Some(name) = &app.sign_workspace.image_name {
                            ui.add(icons::image("image", 18.0, t.accent));
                            ui.label(egui::RichText::new(name).font(theme::medium(13.0)).color(t.text));
                            if crate::widgets::action_button(ui, tl!("Remove"), false).clicked() {
                                app.sign_workspace.image_name = None;
                                app.sign_workspace.image_bytes = None;
                            }
                        } else if widgets::dialog_action_button(ui, "folder-open", tl!("Choose image file…"), false).clicked() {
                            app.sign_image_dialog();
                        }
                    });
                }
            }

            ui.add_space(10.0);
            crate::widgets::check_row(ui, &mut app.sign_workspace.include_date, tl!("Include signature date stamp"));
        });
}

// ---------------------------------------------------------------------------
// Page & Placement Section
// ---------------------------------------------------------------------------

fn placement_section(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens, total_pages: usize) {
    ui.label(egui::RichText::new(tl!("Placement and page")).font(theme::semibold(15.0)).color(t.text));
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(tl!("Page:")).color(t.text_muted));
        let target = app.sign_workspace.page_target;
        if crate::widgets::radio_row(ui, target == PageTarget::Last, tl!("Last page")) {
            app.sign_workspace.page_target = PageTarget::Last;
        }
        if crate::widgets::radio_row(ui, target == PageTarget::First, tl!("First page")) {
            app.sign_workspace.page_target = PageTarget::First;
        }
        if crate::widgets::radio_row(ui, target == PageTarget::Specific, tl!("Specific page:")) {
            app.sign_workspace.page_target = PageTarget::Specific;
        }
        if app.sign_workspace.page_target == PageTarget::Specific {
            let max = total_pages.max(1) as u32;
            let mut page = app.sign_workspace.page_number.clamp(1, max as usize) as u32;
            crate::widgets::stepper_u32(ui, &mut page, 1, max, 1, "");
            app.sign_workspace.page_number = page as usize;
        }
    });

    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(tl!("Position:")).color(t.text_muted));
        let pos = app.sign_workspace.placement;
        if crate::widgets::radio_row(ui, pos == SignPlacement::BottomRight, tl!("Bottom right")) {
            app.sign_workspace.placement = SignPlacement::BottomRight;
        }
        if crate::widgets::radio_row(ui, pos == SignPlacement::BottomCenter, tl!("Bottom center")) {
            app.sign_workspace.placement = SignPlacement::BottomCenter;
        }
        if crate::widgets::radio_row(ui, pos == SignPlacement::BottomLeft, tl!("Bottom left")) {
            app.sign_workspace.placement = SignPlacement::BottomLeft;
        }
        if crate::widgets::radio_row(ui, pos == SignPlacement::TopRight, tl!("Top right")) {
            app.sign_workspace.placement = SignPlacement::TopRight;
        }
        if app.sign_workspace.kind == SignKind::Digital && ui.radio(pos == SignPlacement::Invisible, tl!("Invisible")).clicked() {
            app.sign_workspace.placement = SignPlacement::Invisible;
        }
    });
}

// ---------------------------------------------------------------------------
// App Methods & Execution
// ---------------------------------------------------------------------------

impl KarnoxPDFApp {
    pub fn open_sign_pdf_workspace(&mut self) {
        if let Some(i) = self.active
            && let Some(doc) = self.session.get(self.views[i].id)
        {
            self.sign_workspace.source =
                Some(SignPdfSource { name: doc.name.clone(), bytes: doc.bytes.clone(), pages: doc.info.pages.len(), path: doc.path.clone() });
        }

        self.deactivate_all_workspaces();
        self.sign_workspace.open = true;
        self.sign_workspace.active = true;
        self.active = None;
        self.dialog = None;
    }

    pub(crate) fn close_sign_pdf_workspace(&mut self) {
        let was_active = self.sign_workspace.active;
        self.sign_workspace.open = false;
        self.sign_workspace.active = false;
        if was_active {
            self.active = self.views.len().checked_sub(1);
        }
    }

    pub(crate) fn stage_sign_pdf_source(&mut self, name: String, bytes: Vec<u8>, path: Option<String>) {
        let bytes = Arc::new(bytes);
        match karnoxpdf_render::inspect(bytes.clone(), None) {
            Ok(info) if !info.pages.is_empty() => {
                self.sign_workspace.source = Some(SignPdfSource { name, bytes, pages: info.pages.len(), path });
                self.open_sign_pdf_workspace();
            }
            Ok(_) => self.notify_tr("The PDF has no pages to convert."),
            Err(e) => self.notify_fmt("Couldn't open {name}: {e}", &[("name", &name), ("e", &e.to_string())]),
        }
    }

    pub(crate) fn stage_sign_cert(&mut self, name: String, bytes: Vec<u8>) {
        self.sign_workspace.cert_name = Some(name);
        self.sign_workspace.cert_bytes = Some(bytes);
    }

    pub(crate) fn stage_sign_image(&mut self, name: String, bytes: Vec<u8>) {
        self.sign_workspace.image_name = Some(name);
        self.sign_workspace.image_bytes = Some(bytes);
    }

    /// Sign the source document according to chosen mode and save it.
    pub fn execute_sign_and_save(&mut self) {
        let Some(source) = self.sign_workspace.source.clone() else {
            self.sign_workspace.error = Some(tl!("No document to sign.").to_string());
            return;
        };

        let target_page = match self.sign_workspace.page_target {
            PageTarget::First => 0,
            PageTarget::Last => source.pages.saturating_sub(1),
            PageTarget::Specific => self.sign_workspace.page_number.saturating_sub(1).min(source.pages.saturating_sub(1)),
        };

        let stem = source.name.trim_end_matches(".pdf").trim_end_matches(".PDF");
        let safe: String = stem.chars().map(|ch| if ch.is_alphanumeric() || " -_.,()".contains(ch) { ch } else { '_' }).collect();
        let trimmed = safe.trim_matches([' ', '.']);
        let stem = if trimmed.is_empty() { "document" } else { trimmed };
        let filename = format!("{stem}_signed.pdf");

        // Inspect page dimensions
        let info = karnoxpdf_render::inspect(source.bytes.clone(), None);
        let (pw, ph) = info.as_ref().ok().and_then(|i| i.pages.get(target_page).map(|p| (p.width as f64, p.height as f64))).unwrap_or((595.0, 842.0));

        let rect = match self.sign_workspace.placement {
            SignPlacement::BottomRight => [pw - 220.0, 30.0, pw - 20.0, 90.0],
            SignPlacement::BottomCenter => [(pw - 200.0) / 2.0, 30.0, (pw + 200.0) / 2.0, 90.0],
            SignPlacement::BottomLeft => [20.0, 30.0, 220.0, 90.0],
            SignPlacement::TopRight => [pw - 220.0, ph - 90.0, pw - 20.0, ph - 30.0],
            SignPlacement::Invisible => [0.0, 0.0, 0.0, 0.0],
        };

        let result: Result<Vec<u8>, String> = match self.sign_workspace.kind {
            SignKind::Digital => {
                let Some(bytes) = self.sign_workspace.cert_bytes.as_ref() else {
                    self.sign_workspace.error = Some(tl!("Select a digital certificate file (.p12/.pfx).").to_string());
                    return;
                };
                let digital_id = sign::pkcs12::open(bytes, &self.sign_workspace.cert_password).map_err(|e| match e {
                    SignError::WrongPassword => tl!("The password is incorrect.").to_string(),
                    err => err.to_string(),
                });
                let digital_id = match digital_id {
                    Ok(id) => id,
                    Err(e) => {
                        self.sign_workspace.error = Some(e);
                        return;
                    }
                };

                let mut session = Session::new();
                let doc_id = session.open(&source.name, None, source.bytes.clone(), None).map_err(|e| e.to_string());
                let doc_id = match doc_id {
                    Ok(id) => id,
                    Err(e) => {
                        self.sign_workspace.error = Some(e);
                        return;
                    }
                };

                let opts = SignOptions {
                    page: target_page,
                    rect: if self.sign_workspace.placement == SignPlacement::Invisible { None } else { Some(rect) },
                    reason: if self.sign_workspace.reason.trim().is_empty() { None } else { Some(self.sign_workspace.reason.trim().to_string()) },
                    location: if self.sign_workspace.location.trim().is_empty() {
                        None
                    } else {
                        Some(self.sign_workspace.location.trim().to_string())
                    },
                    certify: if self.sign_workspace.certify { Some(2) } else { None },
                    appearance: Appearance {
                        name: true,
                        date: true,
                        reason: !self.sign_workspace.reason.trim().is_empty(),
                        location: !self.sign_workspace.location.trim().is_empty(),
                        labels: true,
                        ..Appearance::default()
                    },
                    ..SignOptions::default()
                };

                session.sign(doc_id, &digital_id, opts).map(|bytes| (*bytes).clone()).map_err(|e| e.to_string())
            }
            SignKind::Simple => {
                let mut session = Session::new();
                let doc_id = session.open(&source.name, None, source.bytes.clone(), None).map_err(|e| e.to_string());
                let doc_id = match doc_id {
                    Ok(id) => id,
                    Err(e) => {
                        self.sign_workspace.error = Some(e);
                        return;
                    }
                };

                let author = self.comment_prefs.author.clone();
                let mut edits: Vec<Edit> = Vec::new();

                match self.sign_workspace.simple_type {
                    SimpleType::Draw => {
                        if self.sign_workspace.strokes.is_empty() {
                            self.sign_workspace.error = Some(tl!("Draw a signature first.").to_string());
                            return;
                        }
                        let width = rect[2] - rect[0];
                        let height = rect[3] - rect[1];
                        let user_strokes: Vec<Vec<[f64; 2]>> = self
                            .sign_workspace
                            .strokes
                            .iter()
                            .map(|s| s.iter().map(|p| [rect[0] + p[0] as f64 * width, rect[1] + p[1] as f64 * height]).collect())
                            .collect();
                        let shape = Shape::Signature { strokes: user_strokes };
                        let mut style = Style::default_for(&shape);
                        style.color = [0.0, 0.0, 0.0];
                        style.width = 2.0;
                        edits.push(Edit::AddAnnotation(NewAnnotation {
                            page: target_page,
                            style,
                            shape,
                            contents: String::new(),
                            author: author.clone(),
                        }));
                    }
                    SimpleType::Type => {
                        let text = self.sign_workspace.typed_name.trim();
                        if text.is_empty() {
                            self.sign_workspace.error = Some(tl!("Type a name for the signature.").to_string());
                            return;
                        }
                        let center_y = (rect[1] + rect[3]) / 2.0;
                        if let Some(shape) = karnoxpdf_engine::typed_signature_shape([rect[0] + 10.0, center_y], text, 26.0) {
                            let mut style = Style::default_for(&shape);
                            style.color = [0.0, 0.0, 0.0];
                            style.fill = Some([0.0, 0.0, 0.0]);
                            edits.push(Edit::AddAnnotation(NewAnnotation {
                                page: target_page,
                                style,
                                shape,
                                contents: text.to_string(),
                                author: author.clone(),
                            }));
                        }
                    }
                    SimpleType::Image => {
                        let Some(img_bytes) = self.sign_workspace.image_bytes.clone() else {
                            self.sign_workspace.error = Some(tl!("Upload an image first.").to_string());
                            return;
                        };
                        edits.push(Edit::AddCustomStamp {
                            page: target_page,
                            rect,
                            name: "Signature".into(),
                            file: karnoxpdf_engine::MarkFile { name: "signature.png".into(), bytes: Arc::new(img_bytes), page: 0 },
                            author: author.clone(),
                        });
                    }
                }

                if self.sign_workspace.include_date {
                    let (y, m, d) = session.today();
                    let display_date = format!("{y:04}-{m:02}-{d:02}");
                    let shape = Shape::Typewriter { rect: [rect[0], rect[1] - 16.0, rect[2], rect[1] - 2.0], font_size: 9.0 };
                    let style = Style::default_for(&shape);
                    edits.push(Edit::AddAnnotation(NewAnnotation {
                        page: target_page,
                        style,
                        shape,
                        contents: format!("Date: {display_date}"),
                        author: author.clone(),
                    }));
                }

                for edit in edits {
                    if let Err(e) = session.apply(doc_id, edit) {
                        self.sign_workspace.error = Some(e.to_string());
                        return;
                    }
                }

                session.save_bytes(doc_id).map(|b| (*b).clone()).map_err(|e| e.to_string())
            }
        };

        match result {
            Ok(bytes) => {
                self.sign_workspace.error = None;
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
                                app.sign_workspace.source = None;
                                app.close_sign_pdf_workspace();
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
                            self.sign_workspace.source = None;
                            self.close_sign_pdf_workspace();
                        }
                        Err(e) => self.notify_fmt("Couldn't download {name}: {e}", &[("name", &filename), ("e", &e.to_string())]),
                    }
                }
            }
            Err(e) => {
                self.sign_workspace.error = Some(e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_defaults() {
        let ws = SignPdfWorkspace::default();
        assert!(!ws.open);
        assert!(!ws.active);
        assert_eq!(ws.kind, SignKind::Digital);
        assert_eq!(ws.simple_type, SimpleType::Draw);
        assert_eq!(ws.page_target, PageTarget::Last);
        assert_eq!(ws.placement, SignPlacement::BottomRight);
        assert!(ws.strokes.is_empty());
        assert!(ws.include_date);
    }

    #[test]
    fn sign_kinds_and_placements() {
        assert_ne!(SignKind::Digital, SignKind::Simple);
        assert_ne!(SignPlacement::BottomRight, SignPlacement::Invisible);
        assert_ne!(PageTarget::First, PageTarget::Last);
    }
}
