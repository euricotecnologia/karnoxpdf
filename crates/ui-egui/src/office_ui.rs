//! Office workspaces: PDF to PowerPoint and Excel, and Word and PowerPoint to PDF.

use std::sync::Arc;

use egui::{Align, CornerRadius, Layout, Stroke};
use karnoxpdf_engine::{ConversionNote, NoteKind, Session};

use crate::theme::{self, Tokens};
use crate::{KarnoxPDFApp, i18n, icons, panels::human_size, widgets};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OfficeKind {
    PowerPoint,
    Excel,
    WordToPdf,
    PowerPointToPdf,
    ExcelToPdf,
}

/// Every kind, in `index` order.
pub const OFFICE_KINDS: [OfficeKind; 5] =
    [OfficeKind::PowerPoint, OfficeKind::Excel, OfficeKind::WordToPdf, OfficeKind::PowerPointToPdf, OfficeKind::ExcelToPdf];

impl OfficeKind {
    pub fn index(self) -> usize {
        match self {
            Self::PowerPoint => 0,
            Self::Excel => 1,
            Self::WordToPdf => 2,
            Self::PowerPointToPdf => 3,
            Self::ExcelToPdf => 4,
        }
    }
    /// Converts an Office file into a PDF (the others convert a PDF).
    pub fn imports_office(self) -> bool {
        matches!(self, Self::WordToPdf | Self::PowerPointToPdf | Self::ExcelToPdf)
    }
    pub fn extension(self) -> &'static str {
        match self {
            Self::PowerPoint => "pptx",
            Self::Excel => "xlsx",
            Self::WordToPdf | Self::PowerPointToPdf | Self::ExcelToPdf => "pdf",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::PowerPoint => tl!("PDF to PowerPoint"),
            Self::Excel => tl!("PDF to Excel"),
            Self::WordToPdf => tl!("Word to PDF"),
            Self::PowerPointToPdf => tl!("PowerPoint to PDF"),
            Self::ExcelToPdf => tl!("Excel to PDF"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Source {
    pub name: String,
    pub bytes: Arc<Vec<u8>>,
    pub pages: usize,
    pub path: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Workspace {
    pub open: bool,
    pub active: bool,
    pub source: Option<Source>,
    pub result: Option<Arc<Vec<u8>>>,
    pub converting: bool,
    /// What the last conversion to PDF could not draw (PowerPoint).
    pub notes: Vec<ConversionNote>,
}

/// A finished conversion to PDF, sent from its worker thread.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct PdfConversion {
    kind: OfficeKind,
    source: Arc<Vec<u8>>,
    result: Result<(Arc<Vec<u8>>, Vec<ConversionNote>), ConvertError>,
}

enum ConvertError {
    UnsupportedDrawing,
    Other(String),
}

impl ConvertError {
    fn from_engine(error: karnoxpdf_engine::EditError) -> Self {
        if matches!(error, karnoxpdf_engine::EditError::Create(karnoxpdf_engine::CreateError::UnsupportedDocxDrawing)) {
            Self::UnsupportedDrawing
        } else {
            Self::Other(error.to_string())
        }
    }

    fn notify(self, app: &mut KarnoxPDFApp, kind: OfficeKind) {
        match (self, kind) {
            (Self::UnsupportedDrawing, _) => app.notify_tr("This DOCX contains a drawing or image that KarnoxPDF cannot convert yet."),
            (Self::Other(error), OfficeKind::PowerPointToPdf) => app.notify_fmt("Couldn't convert PowerPoint presentation: {e}", &[("e", &error)]),
            (Self::Other(error), OfficeKind::ExcelToPdf) => app.notify_fmt("Couldn't convert Excel workbook: {e}", &[("e", &error)]),
            (Self::Other(error), _) => app.notify_fmt("Couldn't convert Word document: {e}", &[("e", &error)]),
        }
    }
}

/// Convert an Office document to PDF: the bytes, and what the PDF leaves out of the source.
fn convert_to_pdf(
    kind: OfficeKind,
    name: &str,
    bytes: &[u8],
    xlsx: &karnoxpdf_engine::XlsxOptions,
) -> Result<(Arc<Vec<u8>>, Vec<ConversionNote>), ConvertError> {
    let converted = karnoxpdf_engine::guard(|| match kind {
        OfficeKind::WordToPdf => Session::new().create_from_docx(name, bytes).map(|bytes| (bytes, Vec::new())),
        OfficeKind::PowerPointToPdf => Session::new().create_from_pptx(name, bytes).map(|pdf| (pdf.bytes, pdf.notes)),
        OfficeKind::ExcelToPdf => Session::new().create_from_xlsx(name, bytes, xlsx).map(|pdf| (pdf.bytes, pdf.notes)),
        OfficeKind::PowerPoint | OfficeKind::Excel => Err(karnoxpdf_engine::EditError::Write("this workspace does not convert Office files".into())),
    });
    match converted {
        Ok(Ok((bytes, notes))) if bytes.starts_with(b"%PDF-") => Ok((bytes, notes)),
        Ok(Ok(_)) => Err(ConvertError::Other("invalid PDF output".into())),
        Ok(Err(error)) => Err(ConvertError::from_engine(error)),
        Err(error) => Err(ConvertError::Other(error)),
    }
}

/// "2, 5, 9" for the slides a note is about, cut short for long lists.
fn slide_list(slides: &[usize]) -> String {
    let mut text = slides.iter().take(12).map(usize::to_string).collect::<Vec<_>>().join(", ");
    if slides.len() > 12 {
        text.push('…');
    }
    text
}

/// One line telling what a conversion left out and where.
fn note_line(note: &ConversionNote, workbook: bool) -> String {
    let slides = slide_list(&note.slides);
    if workbook {
        let pattern = match note.kind {
            NoteKind::HiddenSheet => tl!("Hidden sheets left out: {sheets}"),
            NoteKind::Chart => tl!("Chart sheets not drawn: {sheets}"),
            NoteKind::Drawing => tl!("Charts, pictures and shapes not drawn on sheets {sheets}"),
            NoteKind::ConditionalFormatting => tl!("Conditional formatting not applied on sheets {sheets}"),
            NoteKind::SheetTable => tl!("Table styles not applied on sheets {sheets}"),
            _ => tl!("Content that isn't supported yet on sheets {sheets}"),
        };
        return i18n::fmt(pattern, &[("sheets", &slides)]);
    }
    let pattern = match note.kind {
        NoteKind::HiddenSlide => tl!("Hidden slides left out: {slides}"),
        NoteKind::Chart => tl!("Charts not drawn on slides {slides}"),
        NoteKind::Diagram => tl!("SmartArt not drawn on slides {slides}"),
        NoteKind::EmbeddedObject => tl!("Embedded objects not drawn on slides {slides}"),
        NoteKind::Picture => tl!("Pictures in unsupported formats not drawn on slides {slides}"),
        NoteKind::ShapeGeometry => tl!("Some shapes drawn as rectangles on slides {slides}"),
        NoteKind::VerticalText => tl!("Vertical text drawn horizontally on slides {slides}"),
        NoteKind::TableStyle => tl!("Table styles missing from the file; colors not applied on slides {slides}"),
        NoteKind::HiddenSheet | NoteKind::Drawing | NoteKind::ConditionalFormatting | NoteKind::SheetTable => {
            tl!("Content that isn't supported yet on slides {slides}")
        }
        NoteKind::Other => tl!("Content that isn't supported yet on slides {slides}"),
    };
    i18n::fmt(pattern, &[("slides", &slides)])
}

pub(crate) fn workspace(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens, kind: OfficeKind) {
    let state = app.office_workspaces[kind.index()].clone();
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(28.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(900.0);
            ui.heading(egui::RichText::new(kind.title()).font(theme::semibold(28.0)).color(t.text));
            ui.add_space(6.0);
            let subtitle = match kind {
                OfficeKind::PowerPoint => tl!("Convert PDF pages into editable PowerPoint slides."),
                OfficeKind::Excel => tl!("Extract tables and text into an editable Excel workbook."),
                OfficeKind::WordToPdf => tl!("Convert a Word DOCX document to PDF in KarnoxPDF."),
                OfficeKind::PowerPointToPdf => tl!("Convert a PowerPoint PPTX presentation to PDF in KarnoxPDF."),
                OfficeKind::ExcelToPdf => tl!("Convert an Excel XLSX workbook to PDF in KarnoxPDF."),
            };
            ui.label(egui::RichText::new(subtitle).color(t.text_muted));
            ui.add_space(24.0);
            egui::Frame::NONE.fill(t.card).stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(12)).inner_margin(egui::Margin::same(24)).show(ui, |ui| {
                ui.set_min_width(ui.available_width().min(850.0));
                let source_label = match kind {
                    OfficeKind::WordToPdf => tl!("Source DOCX"),
                    OfficeKind::PowerPointToPdf => tl!("Source PPTX"),
                    OfficeKind::ExcelToPdf => tl!("Source XLSX"),
                    _ => tl!("Source PDF"),
                };
                ui.label(egui::RichText::new(source_label).font(theme::semibold(15.0)).color(t.text));
                ui.add_space(10.0);
                if let Some(source) = state.source.as_ref() {
                    egui::Frame::NONE.fill(t.field).corner_radius(CornerRadius::same(7)).inner_margin(egui::Margin::symmetric(14, 12)).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.add(icons::image("file-text", 20.0, t.accent));
                            ui.add_space(6.0);
                            ui.add(egui::Label::new(egui::RichText::new(&source.name).font(theme::medium(14.0)).color(t.text)).truncate());
                            let detail = if kind.imports_office() { human_size(source.bytes.len()) } else {
                                let pages = if source.pages == 1 { tl!("1 page").to_owned() } else { i18n::fmt(tl!("{n} pages"), &[("n", &source.pages.to_string())]) };
                                format!("{pages} · {}", human_size(source.bytes.len()))
                            };
                            ui.label(egui::RichText::new(detail).color(t.text_muted));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if widgets::dialog_action_button(ui, "folder-open", tl!("Change file"), false).clicked() { app.office_source_dialog(kind); }
                            });
                        });
                    });
                } else {
                    ui.add_space(20.0);
                    ui.add(icons::image("file-text", 38.0, t.accent));
                    ui.add_space(10.0);
                    let prompt = match kind {
                        OfficeKind::WordToPdf => tl!("Select a DOCX to convert"),
                        OfficeKind::PowerPointToPdf => tl!("Select a PPTX to convert"),
                        OfficeKind::ExcelToPdf => tl!("Select an XLSX to convert"),
                        _ => tl!("Select a PDF to convert"),
                    };
                    ui.label(egui::RichText::new(prompt).font(theme::semibold(18.0)).color(t.text));
                    ui.add_space(14.0);
                    if widgets::dialog_action_button(ui, "folder-open", tl!("Open file"), true).clicked() { app.office_source_dialog(kind); }
                    ui.add_space(24.0);
                }
                if state.source.is_some() {
                    ui.add_space(22.0);
                    ui.separator();
                    ui.add_space(18.0);
                    ui.label(egui::RichText::new(tl!("About the conversion")).font(theme::semibold(15.0)).color(t.text));
                    ui.add_space(8.0);
                    let detail = match kind {
                        OfficeKind::PowerPoint => tl!("Each PDF page becomes a slide. Text boxes and images can be edited; fonts and layout may differ from the original."),
                        OfficeKind::Excel => tl!("Detected tables become spreadsheet cells; remaining text follows below. Images and the exact PDF layout are not transferred."),
                        OfficeKind::WordToPdf => tl!("KarnoxPDF converts this document locally. Paragraphs, basic formatting, tables and simple shapes are supported. Complex drawings and images are not supported yet. Save older .doc files as DOCX first."),
                        OfficeKind::PowerPointToPdf => tl!("KarnoxPDF converts this presentation locally, one page per slide. Text, shapes, lines, pictures and tables are drawn; fonts are replaced by Helvetica, so line breaks can differ. Charts, SmartArt and embedded objects are not drawn yet, and text in scripts other than Latin cannot be converted yet. Save older .ppt files as PPTX first."),
                        OfficeKind::ExcelToPdf => tl!("KarnoxPDF prints every visible sheet locally, with the paper size, margins, scale, page breaks and print titles the workbook sets. Values keep their number formats, with fills, borders and alignment. Charts, pictures and conditional formatting are not drawn yet, fonts are replaced by Helvetica, and text in scripts other than Latin cannot be converted yet. Save older .xls files as XLSX first."),
                    };
                    ui.label(egui::RichText::new(detail).color(t.text_muted));
                    ui.add_space(5.0);
                    if !kind.imports_office() {
                        ui.label(egui::RichText::new(tl!("Scanned pages need OCR before their text can be edited.")).color(t.text_muted));
                    }
                    if kind == OfficeKind::ExcelToPdf {
                        ui.add_space(14.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(tl!("Page orientation")).font(theme::medium(14.0)).color(t.text));
                            ui.add_space(8.0);
                            for (choice, label, icon) in [
                                (karnoxpdf_engine::SheetOrientation::Auto, tl!("As set in the workbook"), "file-spreadsheet"),
                                (karnoxpdf_engine::SheetOrientation::Portrait, tl!("Portrait"), "file-text"),
                                (karnoxpdf_engine::SheetOrientation::Landscape, tl!("Landscape"), "rectangle-horizontal"),
                            ] {
                                if crate::widgets::option_button(ui, Some(icon), label, app.office_orientation == choice, 0.0).clicked() {
                                    app.office_orientation = choice;
                                }
                            }
                        });
                    }
                    ui.add_space(20.0);
                    ui.horizontal(|ui| {
                        let unchanged = match kind {
                            OfficeKind::WordToPdf => tl!("The source DOCX will not be changed."),
                            OfficeKind::PowerPointToPdf => tl!("The source PPTX will not be changed."),
                            OfficeKind::ExcelToPdf => tl!("The source XLSX will not be changed."),
                            _ => tl!("The source PDF will not be changed."),
                        };
                        ui.label(egui::RichText::new(unchanged).color(t.text_muted).small());
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let label = match kind {
                                OfficeKind::PowerPoint => tl!("Convert to PowerPoint"),
                                OfficeKind::Excel => tl!("Convert to Excel"),
                                OfficeKind::WordToPdf | OfficeKind::PowerPointToPdf | OfficeKind::ExcelToPdf => tl!("Convert to PDF"),
                            };
                            if state.converting { ui.spinner(); }
                            if ui.add_enabled_ui(!state.converting, |ui| widgets::dialog_action_button(ui, "file-output", label, true)).inner.clicked() {
                                app.convert_to_office(kind);
                            }
                        });
                    });
                    if let Some(result) = state.result.as_ref() {
                        ui.add_space(22.0);
                        ui.separator();
                        ui.add_space(16.0);
                        let ready = match kind {
                            OfficeKind::PowerPoint => tl!("PowerPoint presentation ready"),
                            OfficeKind::Excel => tl!("Excel workbook ready"),
                            OfficeKind::WordToPdf | OfficeKind::PowerPointToPdf | OfficeKind::ExcelToPdf => tl!("PDF ready"),
                        };
                        ui.label(egui::RichText::new(ready).font(theme::semibold(15.0)).color(t.text));
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(format!("{}: {}", kind.extension().to_ascii_uppercase(), human_size(result.len()))).color(t.text_muted));
                        if !state.notes.is_empty() {
                            ui.add_space(10.0);
                            ui.label(egui::RichText::new(tl!("Some content could not be converted:")).font(theme::medium(14.0)).color(t.text));
                            for note in &state.notes {
                                ui.label(egui::RichText::new(format!("• {}", note_line(note, kind == OfficeKind::ExcelToPdf))).color(t.text_muted));
                            }
                        }
                        ui.add_space(14.0);
                        let save = match kind {
                            OfficeKind::PowerPoint => tl!("Save PowerPoint presentation"),
                            OfficeKind::Excel => tl!("Save Excel workbook"),
                            OfficeKind::WordToPdf | OfficeKind::PowerPointToPdf | OfficeKind::ExcelToPdf => tl!("Save PDF"),
                        };
                        if widgets::dialog_action_button(ui, "save", save, true).clicked() { app.save_office_result(kind); }
                    }
                }
            });
        });
    });
}

impl KarnoxPDFApp {
    pub(crate) fn deactivate_office(&mut self) {
        for state in &mut self.office_workspaces {
            state.active = false;
        }
    }

    pub fn open_office_workspace(&mut self, kind: OfficeKind) {
        self.deactivate_all_workspaces();
        let state = &mut self.office_workspaces[kind.index()];
        state.open = true;
        state.active = true;
        self.active = None;
        self.dialog = None;
    }

    pub(crate) fn close_office_workspace(&mut self, kind: OfficeKind) {
        let state = &mut self.office_workspaces[kind.index()];
        let was_active = state.active;
        state.open = false;
        state.active = false;
        state.result = None;
        state.notes.clear();
        state.converting = false;
        if was_active {
            self.active = self.views.len().checked_sub(1);
        }
    }

    pub(crate) fn stage_office_source(&mut self, kind: OfficeKind, name: String, bytes: Vec<u8>, path: Option<String>) {
        let bytes = Arc::new(bytes);
        if kind.imports_office() {
            let (extension, label, legacy) = match kind {
                OfficeKind::PowerPointToPdf => (".pptx", "PPTX", "Choose a .pptx file. Save older .ppt files as PPTX first."),
                OfficeKind::ExcelToPdf => (".xlsx", "XLSX", "Choose a .xlsx file. Save older .xls files as XLSX first."),
                _ => (".docx", "DOCX", "Choose a .docx file. Save older .doc files as DOCX first."),
            };
            let limit = match kind {
                OfficeKind::PowerPointToPdf => karnoxpdf_engine::MAX_PPTX_BYTES,
                OfficeKind::ExcelToPdf => karnoxpdf_engine::MAX_XLSX_BYTES,
                _ => 64 * 1024 * 1024,
            };
            if bytes.len() > limit {
                self.notify_fmt(
                    "Couldn't open {name}: {e}",
                    &[("name", &name), ("e", &format!("{label} exceeds the {} MB import limit", limit / (1024 * 1024)))],
                );
                return;
            }
            if !name.to_ascii_lowercase().ends_with(extension) {
                self.notify_tr(legacy);
                return;
            }
            if !bytes.starts_with(b"PK") {
                match kind {
                    OfficeKind::PowerPointToPdf => self.notify_tr("Couldn't open PowerPoint presentation: invalid PPTX package."),
                    OfficeKind::ExcelToPdf => self.notify_tr("Couldn't open Excel workbook: invalid XLSX package."),
                    _ => self.notify_tr("Couldn't open Word document: invalid DOCX package."),
                }
                return;
            }
            let state = &mut self.office_workspaces[kind.index()];
            state.source = Some(Source { name, bytes, pages: 0, path });
            state.result = None;
            state.notes.clear();
            state.converting = false;
            self.open_office_workspace(kind);
            return;
        }
        match karnoxpdf_render::inspect(bytes.clone(), None) {
            Ok(info) if !info.pages.is_empty() => {
                let state = &mut self.office_workspaces[kind.index()];
                state.source = Some(Source { name, bytes, pages: info.pages.len(), path });
                state.result = None;
                state.notes.clear();
                state.converting = false;
                self.open_office_workspace(kind);
            }
            Ok(_) => self.notify_tr("The PDF has no pages to convert."),
            Err(e) => self.notify_fmt("Couldn't open {name}: {e}", &[("name", &name), ("e", &e.to_string())]),
        }
    }

    /// How a workbook is printed: the page orientation chosen in the Excel workspace, and the number
    /// and date conventions of the interface language (read here, on the UI thread).
    fn xlsx_options(&self) -> karnoxpdf_engine::XlsxOptions {
        karnoxpdf_engine::XlsxOptions {
            orientation: self.office_orientation,
            locale: karnoxpdf_engine::Locale::from_language(i18n::current().code()),
            today: None,
        }
    }

    pub fn convert_to_office(&mut self, kind: OfficeKind) {
        let Some(source) = self.office_workspaces[kind.index()].source.clone() else { return };
        self.office_workspaces[kind.index()].result = None;
        self.office_workspaces[kind.index()].notes.clear();
        if kind.imports_office() {
            #[cfg(not(target_arch = "wasm32"))]
            {
                if self.office_workspaces[kind.index()].converting {
                    return;
                }
                self.office_workspaces[kind.index()].converting = true;
                let (sender, receiver) = std::sync::mpsc::channel();
                let ctx = self.ctx.clone();
                let xlsx = self.xlsx_options();
                std::thread::spawn(move || {
                    let result = convert_to_pdf(kind, &source.name, &source.bytes, &xlsx);
                    let _ = sender.send(PdfConversion { kind, source: source.bytes, result });
                    if let Some(ctx) = ctx {
                        ctx.request_repaint();
                    }
                });
                self.pdf_conversions.push((kind, receiver));
                return;
            }
            #[cfg(target_arch = "wasm32")]
            {
                let xlsx = self.xlsx_options();
                match convert_to_pdf(kind, &source.name, &source.bytes, &xlsx) {
                    Ok((bytes, notes)) => {
                        let state = &mut self.office_workspaces[kind.index()];
                        state.result = Some(bytes);
                        state.notes = notes;
                    }
                    Err(error) => error.notify(self, kind),
                }
                return;
            }
        }
        let mut session = Session::new();
        let id = match session.open(&source.name, None, source.bytes, None) {
            Ok(id) => id,
            Err(e) => {
                self.notify_fmt("Couldn't convert the PDF: {e}", &[("e", &e.to_string())]);
                return;
            }
        };
        let Some(doc) = session.get(id) else {
            self.notify_tr("Couldn't convert the PDF: the file could not be opened.");
            return;
        };
        let result = karnoxpdf_engine::guard(|| match kind {
            OfficeKind::PowerPoint => doc.export_powerpoint(),
            OfficeKind::Excel => doc.export_excel(),
            OfficeKind::WordToPdf | OfficeKind::PowerPointToPdf | OfficeKind::ExcelToPdf => Err("unexpected Office import route".into()),
        });
        match result {
            Ok(Ok(bytes)) if bytes.starts_with(b"PK") => self.office_workspaces[kind.index()].result = Some(Arc::new(bytes)),
            Ok(Ok(_)) => self.notify_tr("Couldn't convert the PDF: the Office file was invalid."),
            Ok(Err(e)) if e.contains("no extractable text") => self.notify_tr("Scanned pages need OCR before their text can be edited."),
            Ok(Err(e)) | Err(e) => self.notify_fmt("Couldn't convert the PDF: {e}", &[("e", &e)]),
        }
    }

    /// Collect the conversions to PDF that worker threads have finished.
    pub(crate) fn poll_pdf_conversions(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut finished = Vec::new();
            let mut index = 0;
            while index < self.pdf_conversions.len() {
                let kind = self.pdf_conversions[index].0;
                match self.pdf_conversions[index].1.try_recv() {
                    Ok(done) => {
                        self.pdf_conversions.remove(index);
                        finished.push(done);
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => index += 1,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        self.pdf_conversions.remove(index);
                        self.office_workspaces[kind.index()].converting = false;
                        match kind {
                            OfficeKind::PowerPointToPdf => {
                                self.notify_tr("Couldn't convert PowerPoint presentation: the conversion stopped unexpectedly.")
                            }
                            OfficeKind::ExcelToPdf => self.notify_tr("Couldn't convert Excel workbook: the conversion stopped unexpectedly."),
                            _ => self.notify_tr("Couldn't convert Word document: the conversion stopped unexpectedly."),
                        }
                    }
                }
            }
            for done in finished {
                let state = &mut self.office_workspaces[done.kind.index()];
                // The user closed the workspace or chose another file meanwhile: the result is stale.
                if !state.open || !state.source.as_ref().is_some_and(|source| Arc::ptr_eq(&source.bytes, &done.source)) {
                    continue;
                }
                state.converting = false;
                match done.result {
                    Ok((bytes, notes)) => {
                        state.result = Some(bytes);
                        state.notes = notes;
                    }
                    Err(error) => error.notify(self, done.kind),
                }
            }
        }
    }

    pub fn save_office_result(&mut self, kind: OfficeKind) {
        let state = &self.office_workspaces[kind.index()];
        let (Some(source), Some(bytes)) = (state.source.as_ref(), state.result.as_ref()) else { return };
        let name = if kind.imports_office() {
            let suffix = match kind {
                OfficeKind::PowerPointToPdf => ".pptx",
                OfficeKind::ExcelToPdf => ".xlsx",
                _ => ".docx",
            };
            source
                .name
                .get(..source.name.len().saturating_sub(suffix.len()))
                .filter(|_| source.name.to_ascii_lowercase().ends_with(suffix))
                .unwrap_or(&source.name)
        } else {
            source.name.trim_end_matches(".pdf").trim_end_matches(".PDF")
        };
        let safe: String = name.chars().map(|c| if c.is_alphanumeric() || " -_.,()".contains(c) { c } else { '_' }).collect();
        let stem = if safe.trim_matches([' ', '.']).is_empty() { "document" } else { safe.trim() };
        let filename = format!("{stem}.{}", kind.extension());
        let bytes = bytes.clone();
        #[cfg(not(target_arch = "wasm32"))]
        {
            let write = move |app: &mut Self, path: std::path::PathBuf| {
                if !path.extension().is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case(kind.extension())) {
                    app.notify_fmt("Choose a .{extension} filename.", &[("extension", kind.extension())]);
                    return;
                }
                let target = path.to_string_lossy();
                match crate::editing::write_atomically(&target, &bytes) {
                    Ok(()) => app.notify_fmt("Converted file saved to {path}", &[("path", &target)]),
                    Err(e) => app.notify_fmt("Couldn't write {path}: {e}", &[("path", &target), ("e", &e.to_string())]),
                }
            };
            match self.save_override.clone() {
                Some(path) => write(self, path.into()),
                None => {
                    let mut dialog =
                        rfd::AsyncFileDialog::new().add_filter(kind.extension().to_ascii_uppercase(), &[kind.extension()]).set_file_name(&filename);
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
                Ok(()) => self.notify_fmt("Converted file saved to {path}", &[("path", &filename)]),
                Err(e) => self.notify_fmt("Couldn't download {name}: {e}", &[("name", &filename), ("e", &e.to_string())]),
            }
        }
    }
}
