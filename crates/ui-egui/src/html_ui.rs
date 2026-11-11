//! HTML to PDF workspace: Convert web URLs, HTML files, or raw HTML code into PDFs.

use std::sync::Arc;

use egui::{Align, Color32, CornerRadius, Layout, Rect, Stroke, vec2};
use karnoxpdf_engine::{HtmlOptions, MarginKind, PageFormat};

use crate::theme::{self, Tokens};
use crate::{KarnoxPDFApp, icons, widgets};

/// Mode of input for HTML.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HtmlInputKind {
    Url,
    File,
    Code,
}

/// Selected HTML file source.
#[derive(Clone, Debug)]
pub struct HtmlFileSource {
    pub name: String,
    pub bytes: Arc<Vec<u8>>,
    pub content: String,
    pub path: Option<String>,
}

/// Workspace state for HTML to PDF.
#[derive(Clone, Debug)]
pub struct HtmlToPdfWorkspace {
    pub open: bool,
    pub active: bool,
    pub input_kind: HtmlInputKind,
    pub url_input: String,
    pub file_source: Option<HtmlFileSource>,
    pub html_code: String,
    pub format: PageFormat,
    pub landscape: bool,
    pub margins: MarginKind,
    pub base_font_size: f64,
    pub include_header: bool,
    pub include_footer: bool,
    pub status_message: Option<String>,
    pub error: Option<String>,
    /// The web page being fetched on a worker thread, if any.
    pub fetch: Option<HtmlFetch>,
}

/// A page being fetched: the worker puts `Some(page)` (or `Some(None)` when it could not be
/// fetched) in the slot when it is done.
#[derive(Clone, Debug)]
pub struct HtmlFetch {
    slot: Arc<std::sync::Mutex<Option<Option<String>>>>,
    url: String,
}

fn lock_slot(slot: &std::sync::Mutex<Option<Option<String>>>) -> std::sync::MutexGuard<'_, Option<Option<String>>> {
    slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Download `url` with `curl`; `None` when that fails or returns nothing. `--max-time` keeps a
/// stalled server from holding the worker forever.
#[cfg(not(target_arch = "wasm32"))]
fn fetch_page(url: &str) -> Option<String> {
    let mut command = std::process::Command::new("curl");
    command.args(["-s", "-L", "--max-time", "60"]).arg(url);
    // A window application starting a console program makes Windows flash a console window.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command.output().ok()?;
    (output.status.success() && !output.stdout.is_empty()).then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The page used when a URL could not be fetched (offline, or no `curl`).
fn placeholder_page(url: &str) -> String {
    format!(
        "<!DOCTYPE html>\n<html>\n<head><title>{url}</title></head>\n<body>\n<h1>{url}</h1>\n<p>Página importada da URL: <a href=\"{url}\">{url}</a></p>\n<hr>\n<p>Conteúdo da página web carregado no KarnoxPDF.</p>\n</body>\n</html>"
    )
}

impl Default for HtmlToPdfWorkspace {
    fn default() -> Self {
        Self {
            open: false,
            active: false,
            input_kind: HtmlInputKind::Code,
            url_input: "https://".to_string(),
            file_source: None,
            html_code: DEFAULT_TEMPLATE_ARTICLE.to_string(),
            format: PageFormat::A4,
            landscape: false,
            margins: MarginKind::Standard,
            base_font_size: 11.0,
            include_header: true,
            include_footer: true,
            status_message: None,
            error: None,
            fetch: None,
        }
    }
}

pub const DEFAULT_TEMPLATE_ARTICLE: &str = r#"<!DOCTYPE html>
<html>
<head>
    <title>Documento Informativo</title>
</head>
<body>
    <h1>Visão Geral do Documento</h1>
    <p>Este documento foi gerado a partir de código <b>HTML</b> diretamente no <i>KarnoxPDF</i>. O motor nativo converte a estrutura do texto em um arquivo PDF de alta precisão e resolução vetorial.</p>
    
    <h2>Principais Benefícios</h2>
    <ul>
        <li>Conversão 100% nativa e rápida em Rust</li>
        <li>Preservação de títulos, parágrafos, listas e tabelas</li>
        <li>Quebra automática de páginas e rodapé inteligente</li>
    </ul>

    <blockquote>"A simplicidade e a portabilidade tornam o formato PDF essencial para a troca segura de informações."</blockquote>
    
    <hr>
    <p>Para testar outros modelos, selecione os atalhos de <b>Modelos Prontos</b> acima ou cole sua própria página web.</p>
</body>
</html>"#;

pub const DEFAULT_TEMPLATE_INVOICE: &str = r#"<!DOCTYPE html>
<html>
<head>
    <title>Fatura de Serviços #1042</title>
</head>
<body>
    <h1>FATURA COMERCIAL</h1>
    <p><b>Emitente:</b> Karnox Soluções Digitais Ltda.<br>
    <b>Cliente:</b> Empresa Parceira S.A.<br>
    <b>Data:</b> 12 de Outubro de 2026</p>

    <hr>
    <h2>Detalhamento dos Serviços</h2>
    <table>
        <tr>
            <th>Descrição do Item</th>
            <th>Qtd</th>
            <th>Valor Unitário</th>
            <th>Subtotal</th>
        </tr>
        <tr>
            <td>Consultoria e Arquitetura de Software</td>
            <td>1</td>
            <td>R$ 4.500,00</td>
            <td>R$ 4.500,00</td>
        </tr>
        <tr>
            <td>Implementação de Módulo PDF</td>
            <td>1</td>
            <td>R$ 3.200,00</td>
            <td>R$ 3.200,00</td>
        </tr>
        <tr>
            <td>Licenciamento de Ferramentas</td>
            <td>1</td>
            <td>R$ 800,00</td>
            <td>R$ 800,00</td>
        </tr>
    </table>

    <p style="color: #2980b9;"><b>Total Geral: R$ 8.500,00</b></p>
    <p>Condição de Pagamento: À vista via Pix ou Transferência Bancária em até 10 dias.</p>
</body>
</html>"#;

pub const DEFAULT_TEMPLATE_REPORT: &str = r#"<!DOCTYPE html>
<html>
<head>
    <title>Relatório de Desempenho</title>
</head>
<body>
    <h1>Relatório Mensal de Operações</h1>
    <p>Análise consolidada de métricas operacionais e indicadores de performance chave do último período.</p>
    
    <h2>Indicadores Principais</h2>
    <table>
        <tr>
            <th>Métrica</th>
            <th>Meta</th>
            <th>Realizado</th>
            <th>Desvio</th>
        </tr>
        <tr>
            <td>Documentos Processados</td>
            <td>10.000</td>
            <td>12.450</td>
            <td>+24,5%</td>
        </tr>
        <tr>
            <td>Disponibilidade do Sistema</td>
            <td>99.9%</td>
            <td>99.98%</td>
            <td>+0,08%</td>
        </tr>
        <tr>
            <td>Tempo Médio de Conversão</td>
            <td>1.5s</td>
            <td>0.8s</td>
            <td>-46,6%</td>
        </tr>
    </table>

    <h3>Conclusões e Próximos Passos</h3>
    <ol>
        <li>Expandir capacidade de processamento concorrente</li>
        <li>Aprimorar suporte a tabelas responsivas</li>
        <li>Automatizar rotinas de auditoria de acessibilidade</li>
    </ol>
</body>
</html>"#;

// ---------------------------------------------------------------------------
// UI Helper Widgets
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Main Workspace View
// ---------------------------------------------------------------------------

pub fn workspace(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens) {
    app.poll_html_fetch(ui.ctx());
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::NONE.inner_margin(egui::Margin { left: 40, right: 40, top: 24, bottom: 40 }).show(ui, |ui| {
            // Header
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(tl!("HTML to PDF")).font(theme::semibold(24.0)).color(t.text));
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new(tl!("Convert web pages, HTML files, or code into beautiful vector PDF documents."))
                            .color(t.text_muted)
                            .font(theme::regular(13.5)),
                    );
                });
            });

            ui.add_space(20.0);

            // Source Mode Selector
            ui.horizontal(|ui| {
                let is_url = app.html_workspace.input_kind == HtmlInputKind::Url;
                if crate::widgets::option_button(ui, Some("external-link"), tl!("Web URL"), is_url, 130.0).clicked() {
                    app.html_workspace.input_kind = HtmlInputKind::Url;
                    app.html_workspace.error = None;
                }
                let is_file = app.html_workspace.input_kind == HtmlInputKind::File;
                if crate::widgets::option_button(ui, Some("folder-open"), tl!("HTML File"), is_file, 130.0).clicked() {
                    app.html_workspace.input_kind = HtmlInputKind::File;
                    app.html_workspace.error = None;
                }
                let is_code = app.html_workspace.input_kind == HtmlInputKind::Code;
                if crate::widgets::option_button(ui, Some("code-xml"), tl!("HTML Code"), is_code, 130.0).clicked() {
                    app.html_workspace.input_kind = HtmlInputKind::Code;
                    app.html_workspace.error = None;
                }
            });

            ui.add_space(16.0);

            // Two-column layout: Left column = Content & Settings; Right column = Live Preview & Actions
            let total_w = ui.available_width();
            let left_w = (total_w * 0.58).clamp(380.0, 680.0);
            let right_w = (total_w - left_w - 24.0).max(280.0);

            ui.horizontal_top(|ui| {
                // Left Column
                ui.allocate_ui_with_layout(vec2(left_w, 0.0), Layout::top_down(Align::Min), |ui| {
                    // Source Input Body
                    egui::Frame::NONE
                        .fill(t.card)
                        .stroke(Stroke::new(1.0, t.divider))
                        .corner_radius(CornerRadius::same(10))
                        .inner_margin(egui::Margin::same(16))
                        .show(ui, |ui| match app.html_workspace.input_kind {
                            HtmlInputKind::Url => {
                                ui.label(egui::RichText::new(tl!("Web Page Address")).font(theme::semibold(14.0)).color(t.text));
                                ui.add_space(6.0);
                                ui.horizontal(|ui| {
                                    ui.add(
                                        egui::TextEdit::singleline(&mut app.html_workspace.url_input)
                                            .hint_text("https://example.com/page")
                                            .desired_width(left_w - 130.0),
                                    );
                                    if widgets::dialog_action_button(ui, "download", tl!("Fetch"), false).clicked() {
                                        app.load_html_url();
                                    }
                                });
                                ui.add_space(8.0);
                                if let Some(msg) = &app.html_workspace.status_message {
                                    ui.label(egui::RichText::new(msg).color(t.accent).small());
                                } else {
                                    ui.label(
                                        egui::RichText::new(tl!("Type or paste a web address to fetch and convert into PDF."))
                                            .color(t.text_muted)
                                            .small(),
                                    );
                                }
                            }
                            HtmlInputKind::File => {
                                ui.label(egui::RichText::new(tl!("Local HTML Document")).font(theme::semibold(14.0)).color(t.text));
                                ui.add_space(8.0);
                                let file_info = app.html_workspace.file_source.as_ref().map(|f| (f.name.clone(), f.bytes.len()));
                                if let Some((name, size)) = file_info {
                                    let mut change = false;
                                    ui.horizontal(|ui| {
                                        icons::paint(ui, Rect::from_min_size(ui.cursor().min, vec2(24.0, 24.0)), "file-code", 20.0, t.accent);
                                        ui.add_space(28.0);
                                        ui.vertical(|ui| {
                                            ui.label(egui::RichText::new(&name).font(theme::medium(13.5)).color(t.text));
                                            ui.label(egui::RichText::new(format!("{size} bytes")).color(t.text_muted).small());
                                        });
                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                            if widgets::dialog_action_button(ui, "folder-open", tl!("Change file"), false).clicked() {
                                                change = true;
                                            }
                                        });
                                    });
                                    if change {
                                        app.html_source_dialog();
                                    }
                                } else {
                                    ui.vertical_centered(|ui| {
                                        ui.add_space(10.0);
                                        icons::paint(
                                            ui,
                                            Rect::from_center_size(ui.cursor().center() + vec2(0.0, 16.0), vec2(36.0, 36.0)),
                                            "file-code",
                                            32.0,
                                            t.accent,
                                        );
                                        ui.add_space(40.0);
                                        ui.label(egui::RichText::new(tl!("Select an HTML file (.html, .htm)")).color(t.text_muted));
                                        ui.add_space(8.0);
                                        if widgets::dialog_action_button(ui, "folder-open", tl!("Open file"), true).clicked() {
                                            app.html_source_dialog();
                                        }
                                        ui.add_space(10.0);
                                    });
                                }
                            }
                            HtmlInputKind::Code => {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(tl!("HTML Editor")).font(theme::semibold(14.0)).color(t.text));
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if crate::widgets::chip_button(ui, tl!("Report"), false).clicked() {
                                            app.html_workspace.html_code = DEFAULT_TEMPLATE_REPORT.to_string();
                                        }
                                        if crate::widgets::chip_button(ui, tl!("Invoice"), false).clicked() {
                                            app.html_workspace.html_code = DEFAULT_TEMPLATE_INVOICE.to_string();
                                        }
                                        if crate::widgets::chip_button(ui, tl!("Article"), false).clicked() {
                                            app.html_workspace.html_code = DEFAULT_TEMPLATE_ARTICLE.to_string();
                                        }
                                        ui.label(egui::RichText::new(tl!("Templates:")).color(t.text_muted).small());
                                    });
                                });
                                ui.add_space(6.0);
                                let text_edit = egui::TextEdit::multiline(&mut app.html_workspace.html_code)
                                    .desired_width(left_w - 32.0)
                                    .desired_rows(12)
                                    .font(egui::TextStyle::Monospace);
                                ui.add(text_edit);
                                ui.add_space(4.0);
                                let char_count = app.html_workspace.html_code.len();
                                let line_count = app.html_workspace.html_code.lines().count();
                                ui.label(egui::RichText::new(format!("{line_count} linhas · {char_count} caracteres")).color(t.text_faint).small());
                            }
                        });

                    ui.add_space(16.0);

                    // Page Configuration Section
                    egui::Frame::NONE
                        .fill(t.card)
                        .stroke(Stroke::new(1.0, t.divider))
                        .corner_radius(CornerRadius::same(10))
                        .inner_margin(egui::Margin::same(16))
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new(tl!("Page Layout & Format")).font(theme::semibold(14.0)).color(t.text));
                            ui.add_space(12.0);

                            // Format selector
                            ui.label(egui::RichText::new(tl!("Paper format:")).color(t.text_muted).small());
                            ui.horizontal(|ui| {
                                for (fmt, label) in
                                    [(PageFormat::A4, "A4"), (PageFormat::Letter, "Letter"), (PageFormat::Legal, "Legal"), (PageFormat::A3, "A3")]
                                {
                                    let is_sel = app.html_workspace.format == fmt;
                                    if crate::widgets::option_button(ui, None, label, is_sel, 110.0).clicked() {
                                        app.html_workspace.format = fmt;
                                    }
                                }
                            });

                            ui.add_space(12.0);

                            // Orientation selector
                            ui.label(egui::RichText::new(tl!("Orientation:")).color(t.text_muted).small());
                            ui.horizontal(|ui| {
                                let is_portrait = !app.html_workspace.landscape;
                                if crate::widgets::option_button(ui, Some("file-text"), tl!("Portrait"), is_portrait, 110.0).clicked() {
                                    app.html_workspace.landscape = false;
                                }
                                let is_landscape = app.html_workspace.landscape;
                                if crate::widgets::option_button(ui, Some("layout"), tl!("Landscape"), is_landscape, 110.0).clicked() {
                                    app.html_workspace.landscape = true;
                                }
                            });

                            ui.add_space(12.0);

                            // Margins selector
                            ui.label(egui::RichText::new(tl!("Margins:")).color(t.text_muted).small());
                            ui.horizontal(|ui| {
                                for (m, label) in [
                                    (MarginKind::Standard, tl!("Standard (20mm)")),
                                    (MarginKind::Narrow, tl!("Narrow (10mm)")),
                                    (MarginKind::Wide, tl!("Wide (30mm)")),
                                    (MarginKind::None, tl!("None (5mm)")),
                                ] {
                                    let is_sel = app.html_workspace.margins == m;
                                    if crate::widgets::option_button(ui, None, label, is_sel, 110.0).clicked() {
                                        app.html_workspace.margins = m;
                                    }
                                }
                            });

                            ui.add_space(12.0);

                            // Base Font Size
                            ui.label(egui::RichText::new(tl!("Base font size:")).color(t.text_muted).small());
                            ui.horizontal(|ui| {
                                for (sz, label) in [(9.0, tl!("Small (9 pt)")), (11.0, tl!("Normal (11 pt)")), (13.0, tl!("Large (13 pt)"))] {
                                    let is_sel = (app.html_workspace.base_font_size - sz).abs() < 0.5;
                                    if crate::widgets::option_button(ui, None, label, is_sel, 110.0).clicked() {
                                        app.html_workspace.base_font_size = sz;
                                    }
                                }
                            });

                            ui.add_space(12.0);

                            // Header and Footer toggles
                            ui.label(egui::RichText::new(tl!("Headers and footers:")).color(t.text_muted).small());
                            ui.horizontal(|ui| {
                                let hdr = app.html_workspace.include_header;
                                if crate::widgets::option_button(ui, None, tl!("Document Header"), hdr, 140.0).clicked() {
                                    app.html_workspace.include_header = !hdr;
                                }
                                let ftr = app.html_workspace.include_footer;
                                if crate::widgets::option_button(ui, None, tl!("Page Numbers"), ftr, 140.0).clicked() {
                                    app.html_workspace.include_footer = !ftr;
                                }
                            });
                        });
                });

                ui.add_space(16.0);

                // Right Column: Live Sheet Preview & Actions
                ui.allocate_ui_with_layout(vec2(right_w, 0.0), Layout::top_down(Align::Min), |ui| {
                    egui::Frame::NONE
                        .fill(t.card)
                        .stroke(Stroke::new(1.0, t.divider))
                        .corner_radius(CornerRadius::same(10))
                        .inner_margin(egui::Margin::same(16))
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new(tl!("Preview")).font(theme::semibold(14.0)).color(t.text));
                            ui.add_space(12.0);

                            // Graphic paper sheet
                            let preview_w = right_w - 32.0;
                            let preview_h = 240.0;
                            let (sheet_container, _) = ui.allocate_exact_size(vec2(preview_w, preview_h), egui::Sense::hover());
                            ui.painter().rect_filled(sheet_container, CornerRadius::same(8), t.pasteboard);

                            // Calculate sheet dimensions inside preview container
                            let (aspect_w, aspect_h) = if app.html_workspace.landscape { (1.414, 1.0) } else { (1.0, 1.414) };
                            let max_s_w = preview_w * 0.72;
                            let max_s_h = preview_h * 0.78;
                            let scale = (max_s_w / aspect_w).min(max_s_h / aspect_h);
                            let sheet_w = aspect_w * scale;
                            let sheet_h = aspect_h * scale;
                            let sheet_rect = Rect::from_center_size(sheet_container.center(), vec2(sheet_w, sheet_h));

                            // White paper with drop shadow
                            let shadow_rect = sheet_rect.translate(vec2(2.0, 3.0));
                            ui.painter().rect_filled(shadow_rect, CornerRadius::same(4), Color32::from_black_alpha(20));
                            ui.painter().rect(
                                sheet_rect,
                                CornerRadius::same(4),
                                Color32::WHITE,
                                Stroke::new(1.0, Color32::from_gray(200)),
                                egui::StrokeKind::Inside,
                            );

                            // Draw simulated content inside the sheet preview
                            let margin_inset = match app.html_workspace.margins {
                                MarginKind::Standard => 10.0,
                                MarginKind::Narrow => 6.0,
                                MarginKind::Wide => 14.0,
                                MarginKind::None => 3.0,
                            };
                            let content_area = sheet_rect.shrink(margin_inset);

                            // Header line in preview
                            if app.html_workspace.include_header {
                                ui.painter().rect_filled(
                                    Rect::from_min_size(content_area.min, vec2(content_area.width() * 0.45, 3.0)),
                                    CornerRadius::same(1),
                                    Color32::from_gray(180),
                                );
                                ui.painter().line_segment(
                                    [content_area.min + vec2(0.0, 6.0), content_area.min + vec2(content_area.width(), 6.0)],
                                    Stroke::new(0.5, Color32::from_gray(215)),
                                );
                            }

                            // Title heading bar
                            let top_content_y = content_area.min.y + if app.html_workspace.include_header { 12.0 } else { 2.0 };
                            ui.painter().rect_filled(
                                Rect::from_min_size(egui::pos2(content_area.min.x, top_content_y), vec2(content_area.width() * 0.65, 7.0)),
                                CornerRadius::same(2),
                                Color32::from_rgb(0x34, 0x49, 0x5E),
                            );

                            // Paragraph simulated lines
                            let mut line_y = top_content_y + 12.0;
                            for line_len in [0.95, 0.90, 0.85, 0.60] {
                                if line_y + 4.0 < content_area.max.y - 12.0 {
                                    ui.painter().rect_filled(
                                        Rect::from_min_size(egui::pos2(content_area.min.x, line_y), vec2(content_area.width() * line_len, 3.0)),
                                        CornerRadius::same(1),
                                        Color32::from_gray(210),
                                    );
                                    line_y += 6.5;
                                }
                            }

                            // Table or block preview
                            line_y += 4.0;
                            if line_y + 20.0 < content_area.max.y - 12.0 {
                                let tbl_rect = Rect::from_min_size(egui::pos2(content_area.min.x, line_y), vec2(content_area.width(), 22.0));
                                ui.painter().rect(
                                    tbl_rect,
                                    CornerRadius::same(2),
                                    Color32::from_gray(245),
                                    Stroke::new(0.5, Color32::from_gray(200)),
                                    egui::StrokeKind::Inside,
                                );
                                ui.painter().line_segment(
                                    [tbl_rect.min + vec2(0.0, 7.0), tbl_rect.min + vec2(tbl_rect.width(), 7.0)],
                                    Stroke::new(0.5, Color32::from_gray(200)),
                                );
                            }

                            // Footer page number preview
                            if app.html_workspace.include_footer {
                                ui.painter().rect_filled(
                                    Rect::from_center_size(egui::pos2(sheet_rect.center().x, content_area.max.y - 2.0), vec2(16.0, 2.5)),
                                    CornerRadius::same(1),
                                    Color32::from_gray(180),
                                );
                            }

                            ui.add_space(8.0);

                            // Format badges under preview
                            ui.horizontal_wrapped(|ui| {
                                let fmt_name = match app.html_workspace.format {
                                    PageFormat::A4 => "A4",
                                    PageFormat::Letter => "Carta",
                                    PageFormat::Legal => "Ofício",
                                    PageFormat::A3 => "A3",
                                };
                                let orient_name = if app.html_workspace.landscape { tl!("Landscape") } else { tl!("Portrait") };
                                ui.label(egui::RichText::new(format!("{fmt_name} · {orient_name}")).font(theme::medium(12.0)).color(t.text_muted));
                            });

                            ui.add_space(16.0);
                            ui.separator();
                            ui.add_space(12.0);

                            // Error display if any
                            if let Some(err) = &app.html_workspace.error {
                                ui.label(egui::RichText::new(err).color(Color32::from_rgb(0xE7, 0x4C, 0x3C)).small());
                                ui.add_space(8.0);
                            }

                            // Main Action Buttons
                            ui.vertical_centered_justified(|ui| {
                                if widgets::pill_button(ui, tl!("Convert and save PDF"), true).clicked() {
                                    app.execute_html_convert_and_save();
                                }
                                ui.add_space(6.0);
                                if widgets::pill_button(ui, tl!("Open in KarnoxPDF"), false).clicked() {
                                    app.execute_html_convert_and_open();
                                }
                                ui.add_space(6.0);
                                if widgets::pill_button(ui, tl!("Close"), false).clicked() {
                                    app.close_html_workspace();
                                }
                            });
                        });
                });
            });
        });
    });
}

// ---------------------------------------------------------------------------
// App Methods & Conversion
// ---------------------------------------------------------------------------

impl KarnoxPDFApp {
    pub fn open_html_workspace(&mut self) {
        self.deactivate_all_workspaces();
        self.html_workspace.open = true;
        self.html_workspace.active = true;
        self.active = None;
        self.dialog = None;
    }

    pub(crate) fn close_html_workspace(&mut self) {
        let was_active = self.html_workspace.active;
        self.html_workspace.open = false;
        self.html_workspace.active = false;
        if was_active {
            self.active = self.views.len().checked_sub(1);
        }
    }

    pub(crate) fn stage_html_source(&mut self, name: String, bytes: Vec<u8>, path: Option<String>) {
        let content = String::from_utf8(bytes.clone()).unwrap_or_else(|_| bytes.iter().map(|&b| b as char).collect());
        self.html_workspace.file_source = Some(HtmlFileSource { name, bytes: Arc::new(bytes), content, path });
        self.html_workspace.input_kind = HtmlInputKind::File;
        self.open_html_workspace();
    }

    pub(crate) fn load_html_url(&mut self) {
        let url = self.html_workspace.url_input.trim().to_string();
        if url.is_empty() || url == "https://" || url == "http://" {
            self.html_workspace.error = Some(tl!("Please enter a valid URL.").to_string());
            return;
        }

        // Handle local file URLs or web URLs
        if let Some(file_path) = url.strip_prefix("file://") {
            #[cfg(not(target_arch = "wasm32"))]
            {
                match std::fs::read_to_string(file_path) {
                    Ok(content) => {
                        self.html_workspace.html_code = content;
                        self.html_workspace.input_kind = HtmlInputKind::Code;
                        self.html_workspace.status_message = Some(format!("Arquivo local carregado: {file_path}"));
                        self.html_workspace.error = None;
                    }
                    Err(e) => {
                        self.html_workspace.error = Some(format!("Não foi possível ler {file_path}: {e}"));
                    }
                }
            }
            #[cfg(target_arch = "wasm32")]
            {
                self.html_workspace.error = Some("file:// não é suportado no navegador".to_string());
            }
            return;
        }

        // On desktop, fetch the page on a worker thread so the window keeps responding.
        #[cfg(not(target_arch = "wasm32"))]
        {
            if self.html_workspace.fetch.is_some() {
                return;
            }
            let slot = Arc::new(std::sync::Mutex::new(None));
            let (worker, target, ctx) = (slot.clone(), url.clone(), self.ctx.clone());
            self.html_workspace.fetch = Some(HtmlFetch { slot, url });
            self.html_workspace.error = None;
            self.html_workspace.status_message = Some(tl!("Loading the page…").to_string());
            std::thread::spawn(move || {
                let page = fetch_page(&target);
                *lock_slot(&worker) = Some(page);
                if let Some(ctx) = ctx {
                    ctx.request_repaint();
                }
            });
        }

        #[cfg(target_arch = "wasm32")]
        {
            self.html_workspace.status_message = Some(format!("URL definida: {url}"));
        }
    }

    /// Collect the page the worker thread has fetched, if it is done.
    pub(crate) fn poll_html_fetch(&mut self, ctx: &egui::Context) {
        let Some(fetch) = self.html_workspace.fetch.clone() else { return };
        let Some(page) = lock_slot(&fetch.slot).take() else {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
            return;
        };
        self.html_workspace.fetch = None;
        // The user closed the workspace meanwhile: nothing to show the page in.
        if !self.html_workspace.open {
            return;
        }
        let url = fetch.url;
        let ws = &mut self.html_workspace;
        match page {
            Some(content) => {
                ws.html_code = content;
                ws.status_message = Some(format!("Página carregada com sucesso ({url})"));
            }
            // Offline, or no curl: a page that names the address, as before.
            None => {
                ws.html_code = placeholder_page(&url);
                ws.status_message = Some(format!("Documento criado para: {url}"));
            }
        }
        ws.input_kind = HtmlInputKind::Code;
        ws.error = None;
    }

    /// Retrieve the HTML string to convert based on the active input mode.
    fn active_html_content(&self) -> (String, String) {
        match self.html_workspace.input_kind {
            HtmlInputKind::Url => {
                let url = self.html_workspace.url_input.trim();
                let title = if url.is_empty() { "Web Page".to_string() } else { url.to_string() };
                (self.html_workspace.html_code.clone(), title)
            }
            HtmlInputKind::File => {
                if let Some(f) = &self.html_workspace.file_source {
                    (f.content.clone(), f.name.clone())
                } else {
                    (self.html_workspace.html_code.clone(), "HTML Document".to_string())
                }
            }
            HtmlInputKind::Code => (self.html_workspace.html_code.clone(), "Documento HTML".to_string()),
        }
    }

    /// Generate PDF bytes from the active HTML content and options.
    pub fn generate_pdf_from_html(&mut self) -> Result<(String, Vec<u8>), String> {
        let (html, default_title) = self.active_html_content();
        if html.trim().is_empty() {
            return Err(tl!("HTML content is empty.").to_string());
        }

        let options = HtmlOptions {
            title: Some(default_title.clone()),
            format: self.html_workspace.format,
            landscape: self.html_workspace.landscape,
            margins: self.html_workspace.margins,
            base_font_size: self.html_workspace.base_font_size,
            include_header: self.html_workspace.include_header,
            include_footer: self.html_workspace.include_footer,
        };

        let pdf_arc = karnoxpdf_engine::Session::new().create_from_html(&html, &options).map_err(|e| format!("{e}"))?;
        let bytes = (*pdf_arc).clone();

        let stem = default_title.trim_end_matches(".html").trim_end_matches(".htm");
        let safe: String = stem.chars().map(|ch| if ch.is_alphanumeric() || " -_.,()".contains(ch) { ch } else { '_' }).collect();
        let trimmed = safe.trim_matches([' ', '.']);
        let stem = if trimmed.is_empty() { "document" } else { trimmed };
        let filename = format!("{stem}_converted.pdf");

        Ok((filename, bytes))
    }

    /// Convert HTML and save directly to PDF.
    pub fn execute_html_convert_and_save(&mut self) {
        match self.generate_pdf_from_html() {
            Ok((filename, bytes)) => {
                self.html_workspace.error = None;
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
                                app.close_html_workspace();
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
                            self.close_html_workspace();
                        }
                        Err(e) => self.notify_fmt("Couldn't download {name}: {e}", &[("name", &filename), ("e", &e.to_string())]),
                    }
                }
            }
            Err(e) => {
                self.html_workspace.error = Some(e);
            }
        }
    }

    /// Convert HTML and immediately open it as a tab in KarnoxPDF.
    pub fn execute_html_convert_and_open(&mut self) {
        match self.generate_pdf_from_html() {
            Ok((name, bytes)) => {
                self.html_workspace.error = None;
                let _ = self.open_bytes(&name, None, bytes);
                self.close_html_workspace();
            }
            Err(e) => {
                self.html_workspace.error = Some(e);
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
    fn workspace_defaults() {
        let ws = HtmlToPdfWorkspace::default();
        assert!(!ws.open);
        assert!(!ws.active);
        assert_eq!(ws.input_kind, HtmlInputKind::Code);
        assert_eq!(ws.format, PageFormat::A4);
        assert!(!ws.landscape);
        assert_eq!(ws.margins, MarginKind::Standard);
        assert!(ws.include_header);
        assert!(ws.include_footer);
        assert!(!ws.html_code.is_empty());
    }

    fn fetching(url: &str) -> (crate::KarnoxPDFApp, Arc<std::sync::Mutex<Option<Option<String>>>>) {
        let mut app = crate::KarnoxPDFApp::new();
        app.html_workspace.open = true;
        let slot = Arc::new(std::sync::Mutex::new(None));
        app.html_workspace.fetch = Some(HtmlFetch { slot: slot.clone(), url: url.into() });
        (app, slot)
    }

    #[test]
    fn a_fetched_page_is_collected_when_the_worker_is_done() {
        let ctx = egui::Context::default();
        let (mut app, slot) = fetching("https://example.test/");
        app.poll_html_fetch(&ctx);
        assert!(app.html_workspace.fetch.is_some(), "the worker is still running: nothing changes");
        *lock_slot(&slot) = Some(Some("<p>hi</p>".into()));
        app.poll_html_fetch(&ctx);
        let ws = &app.html_workspace;
        assert!(ws.fetch.is_none());
        assert_eq!((ws.html_code.as_str(), ws.input_kind), ("<p>hi</p>", HtmlInputKind::Code));
        assert!(ws.status_message.as_deref().is_some_and(|m| m.contains("https://example.test/")));
        // A page that could not be fetched becomes the placeholder naming the address.
        let (mut app, slot) = fetching("https://example.test/x");
        *lock_slot(&slot) = Some(None);
        app.poll_html_fetch(&ctx);
        assert!(app.html_workspace.html_code.contains("<h1>https://example.test/x</h1>"));
        // The workspace was closed meanwhile: the result is dropped.
        let (mut app, slot) = fetching("https://example.test/");
        app.html_workspace.open = false;
        let before = app.html_workspace.html_code.clone();
        *lock_slot(&slot) = Some(Some("<p>late</p>".into()));
        app.poll_html_fetch(&ctx);
        assert!(app.html_workspace.fetch.is_none());
        assert_eq!(app.html_workspace.html_code, before);
    }

    /// The fetch returns at once and finishes on its own: a closed local port fails quickly, without
    /// depending on the network, and leaves the placeholder page.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn loading_a_url_does_not_block_and_a_second_request_is_ignored() {
        let ctx = egui::Context::default();
        let mut app = crate::KarnoxPDFApp::new();
        app.html_workspace.open = true;
        app.html_workspace.url_input = "http://127.0.0.1:9/".into();
        app.load_html_url();
        assert!(app.html_workspace.fetch.is_some(), "started in the background");
        let first = app.html_workspace.fetch.as_ref().map(|f| Arc::as_ptr(&f.slot));
        app.load_html_url();
        assert_eq!(app.html_workspace.fetch.as_ref().map(|f| Arc::as_ptr(&f.slot)), first, "the running fetch was kept");
        let started = std::time::Instant::now();
        while app.html_workspace.fetch.is_some() {
            assert!(started.elapsed().as_secs() < 30, "the fetch never finished");
            std::thread::sleep(std::time::Duration::from_millis(20));
            app.poll_html_fetch(&ctx);
        }
        assert!(app.html_workspace.html_code.contains("http://127.0.0.1:9/"));
        assert_eq!(app.html_workspace.input_kind, HtmlInputKind::Code);
    }
}
