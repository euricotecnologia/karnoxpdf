//! Translate PDF workspace: Extract text from PDF documents, translate into
//! multiple languages (with online API support and offline fallback), review/edit
//! side-by-side, and generate a new translated or bilingual PDF directly.

use std::sync::Arc;

use egui::{Align, Color32, CornerRadius, Layout, Rect, Sense, Stroke, vec2};
use karnoxpdf_engine::{HtmlOptions, MarginKind, PageFormat, Session};

use crate::panels::human_size;
use crate::theme::{self, Tokens};
use crate::{KarnoxPDFApp, icons, widgets};

/// Supported language list: (ISO code, Display label in Portuguese).
pub const SUPPORTED_LANGUAGES: &[(&str, &str)] = &[
    ("pt", "Português (Brasil)"),
    ("en", "Inglês"),
    ("es", "Espanhol"),
    ("fr", "Francês"),
    ("de", "Alemão"),
    ("it", "Italiano"),
    ("ja", "Japonês"),
    ("zh", "Chinês"),
    ("ru", "Russo"),
    ("nl", "Holandês"),
];

/// Mode of output when generating the final PDF.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TranslateOutputMode {
    #[default]
    TranslatedOnly,
    BilingualSideBySide,
}

/// Source PDF document for translation.
#[derive(Clone, Debug)]
pub struct TranslatePdfSource {
    pub name: String,
    pub bytes: Arc<Vec<u8>>,
    pub pages: usize,
    pub page_texts: Vec<String>,
    pub translated_texts: Vec<String>,
    pub path: Option<String>,
}

/// State for the Translate PDF workspace.
#[derive(Clone, Debug)]
pub struct TranslatePdfWorkspace {
    pub open: bool,
    pub active: bool,
    pub source: Option<TranslatePdfSource>,

    /// Source language code ("auto", "en", "pt", etc.)
    pub src_lang: String,
    /// Target language code ("pt", "en", "es", etc.)
    pub tgt_lang: String,

    /// Currently viewed page (0-based)
    pub current_page: usize,

    /// Output mode for PDF generation
    pub output_mode: TranslateOutputMode,

    pub status_message: Option<String>,
    pub error: Option<String>,
    pub success_note: Option<String>,

    /// The translation running in the background, if any.
    pub job: Option<TranslateJob>,
    /// Whether the running job is "all pages" (for the message when it ends).
    pub all_requested: bool,
}

impl Default for TranslatePdfWorkspace {
    fn default() -> Self {
        Self {
            open: false,
            active: false,
            source: None,
            src_lang: "auto".to_string(),
            tgt_lang: "pt".to_string(),
            current_page: 0,
            output_mode: TranslateOutputMode::TranslatedOnly,
            status_message: None,
            error: None,
            success_note: None,
            job: None,
            all_requested: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Text extraction & Translation logic
// ---------------------------------------------------------------------------

/// Extract reading-order text for all pages in a document.
pub fn extract_all_page_texts(bytes: Arc<Vec<u8>>, total_pages: usize) -> Vec<String> {
    let mut renderer = karnoxpdf_render::PageRenderer::new(bytes, Default::default());
    (0..total_pages)
        .map(|page| {
            let r = renderer.render(karnoxpdf_render::RenderRequest {
                page,
                kind: karnoxpdf_render::RequestKind::Text,
                scale: 1.0,
                ..Default::default()
            });
            r.text.map(|t| t.plain_text()).unwrap_or_default()
        })
        .collect()
}

/// Progress of a translation running on a worker thread, shared with the interface.
#[derive(Debug, Default)]
pub struct JobState {
    /// Pages translated since the interface last looked: (page index, text).
    done: Vec<(usize, String)>,
    pages_done: usize,
    pages_total: usize,
    parts: usize,
    failed: usize,
    error: Option<String>,
    finished: bool,
}

/// A running translation, and the document it belongs to (the result is dropped if that changed).
#[derive(Clone, Debug)]
pub struct TranslateJob {
    state: Arc<std::sync::Mutex<JobState>>,
    source: Arc<Vec<u8>>,
}

fn lock(state: &std::sync::Mutex<JobState>) -> std::sync::MutexGuard<'_, JobState> {
    state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Translate the pages one after the other, publishing each as it is done. Never panics: the
/// service, the network and the text are all untrusted.
fn run_translation(work: Vec<(usize, String)>, src: String, tgt: String, state: &std::sync::Mutex<JobState>, ctx: Option<egui::Context>) {
    lock(state).pages_total = work.len();
    let mut detected = None;
    for (page, text) in work {
        let out = crate::translate_service::translate_text(&text, &src, &tgt, &mut detected, &mut crate::translate_service::query_online);
        let mut s = lock(state);
        s.done.push((page, out.text));
        s.pages_done += 1;
        s.parts += out.parts;
        s.failed += out.failed;
        if s.error.is_none() {
            s.error = out.error;
        }
        drop(s);
        if let Some(ctx) = &ctx {
            ctx.request_repaint();
        }
    }
    lock(state).finished = true;
    if let Some(ctx) = &ctx {
        ctx.request_repaint();
    }
}

fn escape_html(input: &str) -> String {
    input.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

/// Build HTML document from translated pages.
pub fn generate_translated_html(
    title: &str,
    pages_translated: &[String],
    pages_original: &[String],
    mode: TranslateOutputMode,
    src_lang_name: &str,
    tgt_lang_name: &str,
) -> String {
    let mut html = String::new();
    html.push_str("<!DOCTYPE html>\n<html>\n<head>\n");
    html.push_str("<meta charset=\"utf-8\">\n");
    html.push_str(&format!("<title>{}</title>\n", escape_html(title)));
    html.push_str(
        r#"<style>
        body { font-family: sans-serif; font-size: 11pt; line-height: 1.5; color: #1f2937; margin: 0; padding: 0; }
        h1 { font-size: 18pt; margin-bottom: 8px; color: #111827; }
        h2 { font-size: 14pt; margin-top: 16px; margin-bottom: 6px; color: #1f2937; }
        p { margin: 0 0 10px 0; text-align: justify; }
        .page-break { page-break-after: always; }
        .bilingual-table { width: 100%; border-collapse: collapse; margin-bottom: 16px; }
        .bilingual-col-orig { width: 50%; vertical-align: top; padding-right: 14px; color: #4b5563; }
        .bilingual-col-trans { width: 50%; vertical-align: top; padding-left: 14px; border-left: 1px solid #e5e7eb; color: #111827; }
        .header-bar { font-size: 9pt; color: #9ca3af; text-transform: uppercase; border-bottom: 1px solid #e5e7eb; padding-bottom: 4px; margin-bottom: 14px; }
    </style>
</head>
<body>
"#,
    );

    match mode {
        TranslateOutputMode::TranslatedOnly => {
            for (idx, text) in pages_translated.iter().enumerate() {
                if idx > 0 {
                    html.push_str("<div class=\"page-break\"></div>\n");
                }
                html.push_str(&format!("<div class=\"header-bar\">{} · Página {}</div>\n", escape_html(tgt_lang_name), idx + 1));
                for paragraph in text.split("\n\n").map(str::trim).filter(|p| !p.is_empty()) {
                    html.push_str(&format!("<p>{}</p>\n", escape_html(paragraph).replace('\n', "<br>")));
                }
            }
        }
        TranslateOutputMode::BilingualSideBySide => {
            for idx in 0..pages_translated.len().max(pages_original.len()) {
                if idx > 0 {
                    html.push_str("<div class=\"page-break\"></div>\n");
                }
                let orig = pages_original.get(idx).map(String::as_str).unwrap_or("");
                let trans = pages_translated.get(idx).map(String::as_str).unwrap_or("");
                html.push_str(&format!(
                    "<div class=\"header-bar\">Página {} · {} (Esquerda) | {} (Direita)</div>\n",
                    idx + 1,
                    escape_html(src_lang_name),
                    escape_html(tgt_lang_name)
                ));
                html.push_str("<table class=\"bilingual-table\">\n<tr>\n");
                html.push_str("<td class=\"bilingual-col-orig\">\n");
                for p in orig.split("\n\n").map(str::trim).filter(|p| !p.is_empty()) {
                    html.push_str(&format!("<p>{}</p>\n", escape_html(p).replace('\n', "<br>")));
                }
                html.push_str("</td>\n<td class=\"bilingual-col-trans\">\n");
                for p in trans.split("\n\n").map(str::trim).filter(|p| !p.is_empty()) {
                    html.push_str(&format!("<p>{}</p>\n", escape_html(p).replace('\n', "<br>")));
                }
                html.push_str("</td>\n</tr>\n</table>\n");
            }
        }
    }

    html.push_str("</body>\n</html>\n");
    html
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

fn lang_name(code: &str) -> &str {
    if code == "auto" { "Detectar automaticamente" } else { SUPPORTED_LANGUAGES.iter().find(|(c, _)| *c == code).map(|(_, n)| *n).unwrap_or(code) }
}

// ---------------------------------------------------------------------------
// Workspace UI
// ---------------------------------------------------------------------------

pub(crate) fn workspace(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens) {
    app.poll_translation(ui.ctx());
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(28.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(960.0);

            // Workspace Header
            ui.heading(egui::RichText::new(tl!("Translate PDF")).font(theme::semibold(28.0)).color(t.text));
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(tl!("Translate PDF document text into multiple languages and generate a new PDF."))
                    .color(t.text_muted)
                    .font(theme::regular(14.0)),
            );

            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                feature_pill(ui, t, "languages", tl!("Multilingual translation"));
                feature_pill(ui, t, "book-open", tl!("Side-by-side bilingual mode"));
                feature_pill(ui, t, "edit", tl!("Review & edit translated text"));
            });

            ui.add_space(22.0);

            // Main Card Container
            egui::Frame::NONE
                .fill(t.card)
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(CornerRadius::same(12))
                .inner_margin(egui::Margin::same(24))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width().min(910.0));

                    if let Some(source) = app.translate_workspace.source.clone() {
                        let total_pages = source.pages;

                        // 1. Source Document Summary Bar
                        egui::Frame::NONE
                            .fill(t.field)
                            .stroke(Stroke::new(1.0, t.border))
                            .corner_radius(CornerRadius::same(8))
                            .inner_margin(egui::Margin::symmetric(14, 12))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    draw_icon(ui, "file-text", 20.0, t.accent);
                                    ui.add_space(6.0);
                                    ui.add(egui::Label::new(egui::RichText::new(&source.name).font(theme::medium(14.0)).color(t.text)).truncate());

                                    let pages_str = if total_pages == 1 {
                                        tl!("1 page").to_string()
                                    } else {
                                        crate::i18n::fmt(tl!("{n} pages"), &[("n", &total_pages.to_string())])
                                    };
                                    ui.label(
                                        egui::RichText::new(format!("{} · {}", human_size(source.bytes.len()), pages_str))
                                            .color(t.text_muted)
                                            .font(theme::regular(12.5)),
                                    );

                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if widgets::dialog_action_button(ui, "folder-open", tl!("Change file"), false).clicked() {
                                            app.translate_source_dialog();
                                        }
                                    });
                                });
                            });

                        ui.add_space(18.0);

                        // 2. Translation Settings Controls (Languages & Output Mode)
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 12.0;

                            // Source Language Selector
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(tl!("From:")).font(theme::medium(13.0)).color(t.text_muted));
                                egui::ComboBox::from_id_salt("src_lang_combo").selected_text(lang_name(&app.translate_workspace.src_lang)).show_ui(
                                    ui,
                                    |ui| {
                                        ui.selectable_value(&mut app.translate_workspace.src_lang, "auto".to_string(), tl!("Auto-detect"));
                                        for &(code, name) in SUPPORTED_LANGUAGES {
                                            ui.selectable_value(&mut app.translate_workspace.src_lang, code.to_string(), name);
                                        }
                                    },
                                );
                            });

                            // Arrow indicator
                            draw_icon(ui, "arrow-right", 16.0, t.text_muted);

                            // Target Language Selector
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(tl!("To:")).font(theme::medium(13.0)).color(t.text_muted));
                                egui::ComboBox::from_id_salt("tgt_lang_combo").selected_text(lang_name(&app.translate_workspace.tgt_lang)).show_ui(
                                    ui,
                                    |ui| {
                                        for &(code, name) in SUPPORTED_LANGUAGES {
                                            ui.selectable_value(&mut app.translate_workspace.tgt_lang, code.to_string(), name);
                                        }
                                    },
                                );
                            });

                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(8.0);

                            // Output Mode
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(tl!("Output format:")).font(theme::medium(13.0)).color(t.text_muted));
                                for (mode, label, icon) in [
                                    (TranslateOutputMode::TranslatedOnly, tl!("Translated only"), "file-text"),
                                    (TranslateOutputMode::BilingualSideBySide, tl!("Bilingual side-by-side"), "columns-2"),
                                ] {
                                    if crate::widgets::option_button(ui, Some(icon), label, app.translate_workspace.output_mode == mode, 0.0)
                                        .clicked()
                                    {
                                        app.translate_workspace.output_mode = mode;
                                    }
                                }
                            });
                        });

                        ui.add_space(14.0);

                        // Quick Translation Action Buttons
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;

                            if widgets::dialog_action_button(ui, "languages", tl!("Translate all pages"), true).clicked() {
                                app.translate_all_pages();
                            }

                            if widgets::dialog_action_button(ui, "file-text", tl!("Translate current page"), false).clicked() {
                                app.translate_current_page();
                            }

                            if widgets::dialog_action_button(ui, "copy", tl!("Copy translation"), false).clicked() {
                                let curr = app.translate_workspace.current_page;
                                if let Some(txt) = app.translate_workspace.source.as_ref().and_then(|s| s.translated_texts.get(curr)) {
                                    ui.ctx().copy_text(txt.clone());
                                    app.translate_workspace.status_message = Some(tl!("Translation copied to clipboard.").to_string());
                                }
                            }

                            if widgets::dialog_action_button(ui, "rotate-ccw", tl!("Reset current page"), false).clicked() {
                                let curr = app.translate_workspace.current_page;
                                if let Some(src) = app.translate_workspace.source.as_mut()
                                    && let Some(orig) = src.page_texts.get(curr)
                                    && let Some(trans) = src.translated_texts.get_mut(curr)
                                {
                                    *trans = orig.clone();
                                }
                            }
                        });

                        ui.add_space(14.0);

                        // Page Navigation Bar
                        ui.horizontal(|ui| {
                            let curr = app.translate_workspace.current_page;

                            if ui.add_enabled_ui(curr > 0, |ui| crate::widgets::action_button(ui, tl!("Previous page"), false)).inner.clicked() {
                                app.translate_workspace.current_page = curr.saturating_sub(1);
                            }

                            let page_num = curr + 1;
                            ui.label(
                                egui::RichText::new(format!(
                                    "{} / {}",
                                    crate::i18n::fmt(tl!("Page {n}"), &[("n", &page_num.to_string())]),
                                    total_pages
                                ))
                                .font(theme::semibold(13.0))
                                .color(t.text),
                            );

                            if ui
                                .add_enabled_ui(curr + 1 < total_pages, |ui| crate::widgets::action_button(ui, tl!("Next page"), false))
                                .inner
                                .clicked()
                            {
                                app.translate_workspace.current_page = (curr + 1).min(total_pages - 1);
                            }
                        });

                        ui.add_space(12.0);

                        // 3. Side-by-Side Review & Editing Area
                        let curr_idx = app.translate_workspace.current_page;
                        let orig_text = source.page_texts.get(curr_idx).cloned().unwrap_or_default();

                        ui.columns(2, |cols| {
                            // Left Column: Original Text
                            let col0 = &mut cols[0];
                            col0.vertical(|ui| {
                                ui.label(egui::RichText::new(tl!("Original Text")).font(theme::semibold(13.5)).color(t.text));
                                ui.add_space(6.0);
                                egui::Frame::NONE
                                    .fill(t.field)
                                    .stroke(Stroke::new(1.0, t.border))
                                    .corner_radius(CornerRadius::same(6))
                                    .inner_margin(egui::Margin::same(12))
                                    .show(ui, |ui| {
                                        ui.set_min_height(260.0);
                                        ui.set_width(ui.available_width());
                                        egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                                            if orig_text.trim().is_empty() {
                                                ui.label(egui::RichText::new(tl!("No text detected on this page.")).color(t.text_faint).italics());
                                            } else {
                                                ui.label(egui::RichText::new(&orig_text).font(theme::regular(12.5)).color(t.text));
                                            }
                                        });
                                    });
                            });

                            // Right Column: Editable Translated Text
                            let col1 = &mut cols[1];
                            col1.vertical(|ui| {
                                ui.label(egui::RichText::new(tl!("Translated Text (Editable)")).font(theme::semibold(13.5)).color(t.accent));
                                ui.add_space(6.0);
                                egui::Frame::NONE
                                    .fill(t.card)
                                    .stroke(Stroke::new(1.2, t.accent))
                                    .corner_radius(CornerRadius::same(6))
                                    .inner_margin(egui::Margin::same(10))
                                    .show(ui, |ui| {
                                        ui.set_min_height(260.0);
                                        ui.set_width(ui.available_width());

                                        let mut edit_text = app
                                            .translate_workspace
                                            .source
                                            .as_ref()
                                            .and_then(|s| s.translated_texts.get(curr_idx))
                                            .cloned()
                                            .unwrap_or_default();

                                        let resp = ui.add(
                                            egui::TextEdit::multiline(&mut edit_text)
                                                .desired_rows(14)
                                                .desired_width(ui.available_width())
                                                .font(theme::regular(12.5)),
                                        );

                                        if resp.changed()
                                            && let Some(src) = app.translate_workspace.source.as_mut()
                                            && let Some(entry) = src.translated_texts.get_mut(curr_idx)
                                        {
                                            *entry = edit_text;
                                        }
                                    });
                            });
                        });

                        ui.add_space(16.0);

                        // Messages / Status Alerts
                        if let Some(msg) = &app.translate_workspace.status_message {
                            egui::Frame::NONE
                                .fill(Color32::from_rgb(0xEA, 0xF2, 0xF8))
                                .stroke(Stroke::new(1.0, Color32::from_rgb(0x34, 0x98, 0xDB)))
                                .corner_radius(CornerRadius::same(6))
                                .inner_margin(egui::Margin::symmetric(14, 10))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        draw_icon(ui, "info", 15.0, Color32::from_rgb(0x29, 0x80, 0xB9));
                                        ui.label(egui::RichText::new(msg).font(theme::medium(12.5)).color(Color32::from_rgb(0x29, 0x80, 0xB9)));
                                    });
                                });
                            ui.add_space(10.0);
                        }

                        if let Some(err) = &app.translate_workspace.error {
                            egui::Frame::NONE
                                .fill(Color32::from_rgb(0xFD, 0xED, 0xEC))
                                .stroke(Stroke::new(1.0, Color32::from_rgb(0xE7, 0x4C, 0x3C)))
                                .corner_radius(CornerRadius::same(6))
                                .inner_margin(egui::Margin::symmetric(14, 10))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        draw_icon(ui, "alert-circle", 15.0, Color32::from_rgb(0xC0, 0x39, 0x2B));
                                        ui.label(egui::RichText::new(err).font(theme::medium(12.5)).color(Color32::from_rgb(0xC0, 0x39, 0x2B)));
                                    });
                                });
                            ui.add_space(10.0);
                        }

                        if let Some(note) = &app.translate_workspace.success_note {
                            egui::Frame::NONE
                                .fill(Color32::from_rgb(0xEA, 0xFA, 0xEA))
                                .stroke(Stroke::new(1.0, Color32::from_rgb(0x27, 0xAE, 0x60)))
                                .corner_radius(CornerRadius::same(6))
                                .inner_margin(egui::Margin::symmetric(14, 10))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        draw_icon(ui, "check-circle-2", 15.0, Color32::from_rgb(0x27, 0xAE, 0x60));
                                        ui.label(egui::RichText::new(note).font(theme::medium(12.5)).color(Color32::from_rgb(0x1E, 0x84, 0x49)));
                                    });
                                });
                            ui.add_space(10.0);
                        }

                        ui.separator();
                        ui.add_space(14.0);

                        // 4. Action Buttons Footer
                        ui.horizontal(|ui| {
                            if widgets::dialog_action_button(ui, "rotate-ccw", tl!("Select another file"), false).clicked() {
                                app.translate_workspace.source = None;
                                app.translate_workspace.status_message = None;
                                app.translate_workspace.error = None;
                                app.translate_workspace.success_note = None;
                            }

                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if widgets::dialog_action_button(ui, "save", tl!("Save Translated PDF"), true).clicked() {
                                    app.execute_translate_and_save();
                                }

                                ui.add_space(8.0);

                                if widgets::dialog_action_button(ui, "book-open", tl!("Open in KarnoxPDF"), false).clicked() {
                                    app.execute_translate_and_open();
                                }
                            });
                        });
                    } else {
                        // Empty State / Select File Screen
                        ui.vertical_centered(|ui| {
                            ui.add_space(20.0);

                            let (rect, _) = ui.allocate_exact_size(vec2(72.0, 72.0), Sense::hover());
                            ui.painter().circle_filled(rect.center(), 36.0, t.hover);
                            icons::paint(ui, Rect::from_center_size(rect.center(), vec2(36.0, 36.0)), "languages", 36.0, t.accent);

                            ui.add_space(16.0);
                            ui.label(egui::RichText::new(tl!("Select a PDF to translate")).font(theme::semibold(18.0)).color(t.text));
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(tl!("Choose a document to extract text and translate into multiple languages."))
                                    .font(theme::regular(13.5))
                                    .color(t.text_muted),
                            );

                            ui.add_space(20.0);

                            if widgets::dialog_action_button(ui, "folder-open", tl!("Select PDF File"), true).clicked() {
                                app.translate_source_dialog();
                            }

                            if app.active.is_some() {
                                ui.add_space(10.0);
                                if widgets::dialog_action_button(ui, "file-text", tl!("Use currently open document"), false).clicked() {
                                    app.open_translate_workspace();
                                }
                            }

                            ui.add_space(20.0);
                        });
                    }
                });

            ui.add_space(32.0);
        });
    });
}

// ---------------------------------------------------------------------------
// App Methods & Execution
// ---------------------------------------------------------------------------

impl KarnoxPDFApp {
    pub fn open_translate_workspace(&mut self) {
        if let Some(i) = self.active
            && let Some(doc) = self.session.get(self.views[i].id)
        {
            let texts = extract_all_page_texts(doc.bytes.clone(), doc.info.pages.len());
            self.translate_workspace.source = Some(TranslatePdfSource {
                name: doc.name.clone(),
                bytes: doc.bytes.clone(),
                pages: doc.info.pages.len(),
                page_texts: texts.clone(),
                translated_texts: texts,
                path: doc.path.clone(),
            });
            self.translate_workspace.current_page = 0;
        }

        self.deactivate_all_workspaces();
        self.translate_workspace.open = true;
        self.translate_workspace.active = true;
        self.translate_workspace.status_message = None;
        self.translate_workspace.error = None;
        self.translate_workspace.success_note = None;
        self.active = None;
        self.dialog = None;
    }

    pub fn close_translate_workspace(&mut self) {
        let was_active = self.translate_workspace.active;
        self.translate_workspace.active = false;
        self.translate_workspace.open = false;
        self.translate_workspace.source = None;
        if was_active {
            self.active = self.views.len().checked_sub(1);
        }
    }

    pub(crate) fn translate_source_dialog(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let dialog = rfd::AsyncFileDialog::new().add_filter("PDF", &["pdf"]);
            self.ask_one(crate::pickers::Ask::File(dialog), None, |app, path| {
                if let Ok(bytes) = std::fs::read(&path) {
                    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "document.pdf".into());
                    app.stage_translate_source(name, bytes, Some(path.to_string_lossy().into_owned()));
                }
            });
        }
    }

    pub(crate) fn stage_translate_source(&mut self, name: String, bytes: Vec<u8>, path: Option<String>) {
        let bytes = Arc::new(bytes);
        match karnoxpdf_render::inspect(bytes.clone(), None) {
            Ok(info) if !info.pages.is_empty() => {
                let texts = extract_all_page_texts(bytes.clone(), info.pages.len());
                self.translate_workspace.source =
                    Some(TranslatePdfSource { name, bytes, pages: info.pages.len(), page_texts: texts.clone(), translated_texts: texts, path });
                self.translate_workspace.current_page = 0;
                self.open_translate_workspace();
            }
            Ok(_) => self.notify_tr("The PDF has no pages to convert."),
            Err(e) => self.notify_fmt("Couldn't open {name}: {e}", &[("name", &name), ("e", &e.to_string())]),
        }
    }

    /// Translate the currently viewed page.
    pub fn translate_current_page(&mut self) {
        let curr = self.translate_workspace.current_page;
        self.start_translation(vec![curr], false);
    }

    /// Translate all pages of the document.
    pub fn translate_all_pages(&mut self) {
        let count = self.translate_workspace.source.as_ref().map_or(0, |s| s.page_texts.len());
        self.start_translation((0..count).collect(), true);
    }

    /// Translate the given pages on a worker thread; the interface keeps running and shows progress.
    fn start_translation(&mut self, pages: Vec<usize>, all: bool) {
        if self.translate_workspace.job.is_some() {
            return;
        }
        let Some(source) = self.translate_workspace.source.as_ref() else { return };
        let work: Vec<(usize, String)> = pages.into_iter().filter_map(|i| source.page_texts.get(i).map(|t| (i, t.clone()))).collect();
        if work.is_empty() {
            return;
        }
        let state = Arc::new(std::sync::Mutex::new(JobState { pages_total: work.len(), ..Default::default() }));
        let job = TranslateJob { state: state.clone(), source: source.bytes.clone() };
        let (src, tgt) = (self.translate_workspace.src_lang.clone(), self.translate_workspace.tgt_lang.clone());
        let ctx = self.ctx.clone();
        let ws = &mut self.translate_workspace;
        ws.job = Some(job);
        ws.error = None;
        ws.success_note = None;
        ws.status_message = Some(crate::i18n::fmt(tl!("Translating… {done} of {total} pages"), &[("done", "0"), ("total", &work.len().to_string())]));
        ws.all_requested = all;
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || run_translation(work, src, tgt, &state, ctx));
        #[cfg(target_arch = "wasm32")]
        run_translation(work, src, tgt, &state, ctx);
    }

    /// Collect what the translation thread has finished and report the outcome.
    pub(crate) fn poll_translation(&mut self, ctx: &egui::Context) {
        let Some(job) = self.translate_workspace.job.clone() else { return };
        let (done, pages_done, pages_total, parts, failed, error, finished) = {
            let mut s = lock(&job.state);
            (std::mem::take(&mut s.done), s.pages_done, s.pages_total, s.parts, s.failed, s.error.clone(), s.finished)
        };
        let ws = &mut self.translate_workspace;
        // The user closed the document or chose another meanwhile: the result is stale.
        let current = ws.source.as_ref().is_some_and(|s| Arc::ptr_eq(&s.bytes, &job.source));
        if current && let Some(source) = ws.source.as_mut() {
            for (page, text) in done {
                if let Some(target) = source.translated_texts.get_mut(page) {
                    *target = text;
                }
            }
        }
        if !current {
            if finished {
                ws.job = None;
            }
            return;
        }
        if !finished {
            ws.status_message = Some(crate::i18n::fmt(
                tl!("Translating… {done} of {total} pages"),
                &[("done", &pages_done.to_string()), ("total", &pages_total.to_string())],
            ));
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
            return;
        }
        ws.job = None;
        if failed == 0 {
            ws.error = None;
            ws.status_message = Some(if ws.all_requested {
                tl!("All pages translated successfully!").to_string()
            } else {
                format!("{} {}", crate::i18n::fmt(tl!("Page {n}"), &[("n", &(ws.current_page + 1).to_string())]), tl!("translated successfully!"))
            });
        } else {
            ws.status_message = None;
            ws.error = Some(crate::i18n::fmt(
                tl!("{failed} of {parts} parts could not be translated ({reason}). The original text was kept for them."),
                &[("failed", &failed.to_string()), ("parts", &parts.to_string()), ("reason", error.as_deref().unwrap_or("?"))],
            ));
        }
    }

    fn translate_filename(&self) -> String {
        let source_name = self.translate_workspace.source.as_ref().map(|s| s.name.as_str()).unwrap_or("document.pdf");
        let stem = source_name.trim_end_matches(".pdf").trim_end_matches(".PDF");
        let safe: String = stem.chars().map(|ch| if ch.is_alphanumeric() || " -_.,()".contains(ch) { ch } else { '_' }).collect();
        let trimmed = safe.trim_matches([' ', '.']);
        let stem = if trimmed.is_empty() { "document" } else { trimmed };
        let tgt = &self.translate_workspace.tgt_lang;
        format!("{stem}_translated_{tgt}.pdf")
    }

    /// Generate the translated PDF bytes from current state.
    pub fn generate_translated_pdf(&mut self) -> Result<(String, Vec<u8>), String> {
        let source = self.translate_workspace.source.as_ref().ok_or_else(|| tl!("No document to process.").to_string())?;

        let src_name = lang_name(&self.translate_workspace.src_lang);
        let tgt_name = lang_name(&self.translate_workspace.tgt_lang);

        let html = generate_translated_html(
            &source.name,
            &source.translated_texts,
            &source.page_texts,
            self.translate_workspace.output_mode,
            src_name,
            tgt_name,
        );

        let options = HtmlOptions {
            title: Some(source.name.clone()),
            format: PageFormat::A4,
            landscape: self.translate_workspace.output_mode == TranslateOutputMode::BilingualSideBySide,
            margins: MarginKind::Standard,
            base_font_size: 11.0,
            include_header: true,
            include_footer: true,
        };

        let session = Session::new();
        let pdf_arc = session.create_from_html(&html, &options).map_err(|e| e.to_string())?;
        let filename = self.translate_filename();

        Ok((filename, (*pdf_arc).clone()))
    }

    /// Execute translated PDF generation and save via file dialog.
    pub fn execute_translate_and_save(&mut self) {
        match self.generate_translated_pdf() {
            Ok((filename, bytes)) => {
                self.translate_workspace.error = None;
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
                                app.translate_workspace.success_note = Some(tl!("Translated PDF saved successfully!").to_string());
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
            Err(e) => {
                self.translate_workspace.error = Some(e);
            }
        }
    }

    /// Execute translated PDF generation and open in a new tab.
    pub fn execute_translate_and_open(&mut self) {
        match self.generate_translated_pdf() {
            Ok((filename, bytes)) => {
                self.translate_workspace.error = None;
                let _ = self.open_bytes(&filename, None, bytes);
                self.close_translate_workspace();
            }
            Err(e) => {
                self.translate_workspace.error = Some(e);
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
    fn translate_workspace_defaults() {
        let ws = TranslatePdfWorkspace::default();
        assert!(!ws.open);
        assert!(!ws.active);
        assert_eq!(ws.src_lang, "auto");
        assert_eq!(ws.tgt_lang, "pt");
        assert_eq!(ws.output_mode, TranslateOutputMode::TranslatedOnly);
        assert_eq!(ws.current_page, 0);
    }

    #[test]
    fn offline_fallback_translates_known_headings_and_keeps_the_rest() {
        let res = crate::translate_service::offline_translate_fallback("Introduction\nThis is a test.\nConclusion", "pt");
        assert_eq!(res, "Introdução\nThis is a test.\nConclusão");
    }

    #[test]
    fn html_generation_translated_and_bilingual() {
        let trans = vec!["Olá mundo!".to_string()];
        let orig = vec!["Hello world!".to_string()];

        let html_single = generate_translated_html("Doc", &trans, &orig, TranslateOutputMode::TranslatedOnly, "Inglês", "Português");
        assert!(html_single.contains("Olá mundo!"));
        assert!(!html_single.contains("<table class=\"bilingual-table\">"));

        let html_bi = generate_translated_html("Doc", &trans, &orig, TranslateOutputMode::BilingualSideBySide, "Inglês", "Português");
        assert!(html_bi.contains("<table class=\"bilingual-table\">"));
        assert!(html_bi.contains("Hello world!"));
        assert!(html_bi.contains("Olá mundo!"));
    }
}
