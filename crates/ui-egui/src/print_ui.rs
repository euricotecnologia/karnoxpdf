//! The Print dialog (Acrobat's File ▸ Print, execution plan M10.5): printer, copies, grayscale;
//! pages to print (all, current, range with labels; odd/even, reverse); page sizing & handling
//! (Size, Poster, Multiple, Booklet); orientation; comments & forms; and a live preview of the
//! sheets. Printing sends the print-ready PDF to the system spooler; "Save as PDF" writes it.

use egui::{Color32, Pos2, Rect, Stroke, pos2, vec2};
use karnoxpdf_engine::print::{self, Binding, BookletSubset, Content, Layout, Orientation, PAPERS, PageOrder, SizeMode, Subset, spool};

use crate::theme::{self, Tokens};
use crate::{KarnoxPDFApp, widgets};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    All,
    Current,
    Range,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handling {
    Size,
    Poster,
    Multiple,
    Booklet,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrintDraft {
    pub printers: Vec<spool::Printer>,
    /// `None` = Save as PDF.
    pub printer: Option<String>,
    pub copies: u32,
    pub collate: bool,
    pub grayscale: bool,
    pub duplex: spool::Duplex,
    pub which: Which,
    pub range: String,
    pub subset: Subset,
    pub reverse: bool,
    pub handling: Handling,
    pub size: SizeMode,
    pub custom_scale: f64,
    pub per_sheet: usize,
    pub order: PageOrder,
    pub border: bool,
    pub auto_rotate: bool,
    pub booklet_subset: BookletSubset,
    pub binding: Binding,
    pub poster_scale: f64,
    pub overlap: f64,
    pub cut_marks: bool,
    pub orientation: Orientation,
    pub content: Content,
    pub paper: usize,
    /// The sheet shown in the preview (0-based).
    pub sheet: usize,
    pub current_page: usize,
}

impl Default for PrintDraft {
    fn default() -> Self {
        PrintDraft {
            printers: Vec::new(),
            printer: None,
            copies: 1,
            collate: true,
            grayscale: false,
            duplex: spool::Duplex::Off,
            which: Which::All,
            range: String::new(),
            subset: Subset::All,
            reverse: false,
            handling: Handling::Size,
            size: SizeMode::Fit,
            custom_scale: 100.0,
            per_sheet: 2,
            order: PageOrder::Horizontal,
            border: false,
            auto_rotate: true,
            booklet_subset: BookletSubset::BothSides,
            binding: Binding::Left,
            poster_scale: 200.0,
            overlap: 18.0,
            cut_marks: true,
            orientation: Orientation::Auto,
            content: Content::DocumentAndMarkups,
            paper: 0,
            sheet: 0,
            current_page: 0,
        }
    }
}

impl PrintDraft {
    /// The engine settings for this draft (page count and labels from the document).
    pub fn settings(&self, count: usize, labels: &[String]) -> Result<print::Settings, String> {
        if self.handling == Handling::Multiple && self.order == PageOrder::CutStack && self.duplex != spool::Duplex::Off {
            return Err("Cut and stack needs Two-sided: Off. Print single-sided sheets.".into());
        }
        let range = match self.which {
            Which::All => None,
            Which::Current => Some((self.current_page + 1).to_string()),
            Which::Range => Some(self.range.clone()),
        };
        let pages = print::select_pages(count, range.as_deref(), labels, self.subset, self.reverse).map_err(|e| e.to_string())?;
        let layout = match self.handling {
            Handling::Size => Layout::Size(match self.size {
                SizeMode::Custom(_) => SizeMode::Custom(self.custom_scale),
                m => m,
            }),
            Handling::Multiple => match Layout::multiple(self.per_sheet) {
                Layout::Multiple { cols, rows, .. } => {
                    Layout::Multiple { cols, rows, order: self.order, border: self.border, auto_rotate: self.auto_rotate }
                }
                other => other,
            },
            Handling::Booklet => Layout::Booklet { subset: self.booklet_subset, binding: self.binding },
            Handling::Poster => Layout::Poster { scale: self.poster_scale, overlap: self.overlap, cut_marks: self.cut_marks },
        };
        Ok(print::Settings { pages, paper: PAPERS[self.paper.min(PAPERS.len() - 1)].1, orientation: self.orientation, layout, content: self.content })
    }

    pub fn job(&self, title: &str) -> spool::Job {
        spool::Job {
            printer: self.printer.clone(),
            copies: self.copies.max(1),
            collate: self.collate,
            duplex: self.duplex,
            grayscale: self.grayscale,
            title: title.to_string(),
            to_file: None,
        }
    }
}

impl KarnoxPDFApp {
    pub fn open_print(&mut self) {
        let Some((i, _)) = self.active_ids() else { return };
        let printers = spool::printers();
        let default = printers.iter().find(|p| p.default).or(printers.first()).map(|p| p.name.clone());
        let current = self.views[i].current;
        let keep = std::mem::take(&mut self.print_draft);
        self.print_draft = PrintDraft { printers, printer: default, current_page: current, sheet: 0, ..keep };
        self.dialog = Some(crate::Dialog::Print);
    }

    /// Print (or save) with the dialog's settings. Returns `true` on success. Save as PDF without
    /// a preset path returns `true` once the save picker is showing; the file is written on a
    /// later frame, when the user has chosen where.
    pub fn print_now(&mut self) -> bool {
        let Some((_, id)) = self.active_ids() else { return false };
        let Some(doc) = self.session.get(id) else { return false };
        let labels: Vec<String> = doc.info.pages.iter().map(|p| p.label.clone()).collect();
        let settings = match self.print_draft.settings(doc.info.pages.len(), &labels) {
            Ok(s) => s,
            Err(e) => {
                self.notify_error(e);
                return false;
            }
        };
        let name = doc.name.clone();
        let bytes = match self.session.print_pdf(id, &settings) {
            Ok(b) => b,
            Err(e) => {
                self.notify_error(e);
                return false;
            }
        };
        match self.print_draft.printer.clone() {
            Some(printer) => match karnoxpdf_engine::submit_print_job(&bytes, &self.print_draft.job(&name)) {
                Ok(msg) => {
                    self.notify(if msg.is_empty() {
                        crate::i18n::fmt(tl!("Sent to {printer}"), &[("printer", &printer)])
                    } else {
                        crate::i18n::fmt(tl!("Sent to {printer}: {msg}"), &[("printer", &printer), ("msg", &msg)])
                    });
                    true
                }
                Err(e) => {
                    self.notify_error(e);
                    false
                }
            },
            None => self.save_print_pdf(&name, bytes),
        }
    }

    /// Print ▸ Save as PDF on the desktop: write to `save_override` (tests and automation), or
    /// ask where and write on a later frame. Returns `true` once written, or once the save picker
    /// is showing.
    #[cfg(not(target_arch = "wasm32"))]
    fn save_print_pdf(&mut self, name: &str, bytes: Vec<u8>) -> bool {
        let write = move |app: &mut Self, path: std::path::PathBuf| match crate::editing::write_atomically(&path.to_string_lossy(), &bytes) {
            Ok(()) => {
                app.notify_fmt("Saved the print-ready PDF to {path}", &[("path", &path.display().to_string())]);
                true
            }
            Err(e) => {
                app.notify_fmt("Could not save: {e}", &[("e", &e.to_string())]);
                false
            }
        };
        match self.save_override.clone() {
            Some(p) => write(self, p.into()),
            None => {
                let stem = name.trim_end_matches(".pdf").trim_end_matches(".PDF");
                let dialog = rfd::AsyncFileDialog::new().add_filter("PDF", &["pdf"]).set_file_name(format!("{stem} (print).pdf"));
                self.ask_one(crate::pickers::Ask::Save(dialog), None, move |app, path| {
                    write(app, path);
                })
            }
        }
    }

    /// Print ▸ Save as PDF in a browser: download it, as Save does (#170). There is no folder to
    /// choose and no file system to write to, so `save_override` doesn't apply.
    #[cfg(target_arch = "wasm32")]
    fn save_print_pdf(&mut self, name: &str, bytes: Vec<u8>) -> bool {
        let stem = name.trim_end_matches(".pdf").trim_end_matches(".PDF");
        let file = format!("{stem} (print).pdf");
        match crate::editing::download(&file, &bytes) {
            Ok(()) => {
                self.notify_fmt("Downloaded {name}", &[("name", &file)]);
                true
            }
            Err(e) => {
                self.notify_fmt("Couldn't download {name}: {e}", &[("name", &file), ("e", &e)]);
                false
            }
        }
    }
}

fn combo<T: PartialEq + Copy>(ui: &mut egui::Ui, id: &str, value: &mut T, choices: &[(T, &str)], width: f32) {
    let shown = choices.iter().find(|c| c.0 == *value).map_or("", |c| c.1);
    egui::ComboBox::from_id_salt(id).selected_text(shown).width(width).show_ui(ui, |ui| {
        for (v, label) in choices {
            ui.selectable_value(value, *v, *label);
        }
    });
}

/// A bordered group of the dialog, with an optional bold title (Acrobat's grouped sections).
fn group(ui: &mut egui::Ui, t: &Tokens, title: Option<&str>, body: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::NONE
        .fill(t.card)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if let Some(title) = title {
                ui.label(egui::RichText::new(title).font(theme::semibold(13.5)));
                ui.add_space(6.0);
            }
            body(ui);
        });
    ui.add_space(8.0);
}

/// The Fit, Actual size, Shrink and Custom scale choices.
fn size_options(ui: &mut egui::Ui, d: &mut PrintDraft) {
    ui.horizontal_wrapped(|ui| {
        for (mode, label) in [(SizeMode::Fit, tl!("Fit")), (SizeMode::Actual, tl!("Actual size")), (SizeMode::Shrink, tl!("Shrink oversized pages"))]
        {
            if widgets::radio_row(ui, d.size == mode, label) {
                d.size = mode;
            }
        }
    });
    ui.horizontal(|ui| {
        let custom = matches!(d.size, SizeMode::Custom(_));
        if widgets::radio_row(ui, custom, tl!("Custom scale:")) {
            d.size = SizeMode::Custom(d.custom_scale);
        }
        ui.add_enabled_ui(custom, |ui| widgets::stepper_f64(ui, &mut d.custom_scale, 1.0, 1000.0, 5.0, "%"));
    });
}

/// The options that belong to the chosen page handling.
fn handling_options(ui: &mut egui::Ui, d: &mut PrintDraft) {
    match d.handling {
        Handling::Size => size_options(ui, d),
        Handling::Poster => {
            ui.horizontal(|ui| {
                ui.label(tl!("Tile scale:"));
                widgets::stepper_f64(ui, &mut d.poster_scale, 10.0, 1000.0, 10.0, "%");
                ui.label(tl!("Overlap:"));
                widgets::stepper_f64(ui, &mut d.overlap, 0.0, 144.0, 1.0, "pt");
            });
            widgets::check_row(ui, &mut d.cut_marks, tl!("Cut marks"));
        }
        Handling::Multiple => {
            ui.horizontal(|ui| {
                ui.label(tl!("Pages per sheet:"));
                combo(ui, "per-sheet", &mut d.per_sheet, &[(2, "2"), (4, "4"), (6, "6"), (9, "9"), (16, "16")], 60.0);
                ui.label(tl!("Page order:"));
                combo(
                    ui,
                    "order",
                    &mut d.order,
                    &[
                        (PageOrder::Horizontal, tl!("Horizontal")),
                        (PageOrder::HorizontalReversed, tl!("Horizontal reversed")),
                        (PageOrder::Vertical, tl!("Vertical")),
                        (PageOrder::VerticalReversed, tl!("Vertical reversed")),
                        (PageOrder::CutStack, tl!("Cut and stack")),
                    ],
                    150.0,
                );
            });
            ui.horizontal(|ui| {
                widgets::check_row(ui, &mut d.border, tl!("Print page border"));
                widgets::check_row(ui, &mut d.auto_rotate, tl!("Auto-rotate pages"));
            });
            if d.order == PageOrder::CutStack {
                ui.label(tl!("Print single-sided. Keep the sheets in order, cut at the marks, then stack the piles left to right, top to bottom."));
            }
        }
        Handling::Booklet => {
            ui.horizontal(|ui| {
                ui.label(tl!("Booklet subset:"));
                combo(
                    ui,
                    "booklet",
                    &mut d.booklet_subset,
                    &[
                        (BookletSubset::BothSides, tl!("Both sides")),
                        (BookletSubset::FrontOnly, tl!("Front side only")),
                        (BookletSubset::BackOnly, tl!("Back side only")),
                    ],
                    130.0,
                );
                ui.label(tl!("Binding:"));
                combo(ui, "binding", &mut d.binding, &[(Binding::Left, tl!("Left")), (Binding::Right, tl!("Right"))], 80.0);
            });
        }
    }
}

/// The left column: printer, copies, pages, sizing, orientation and what to print.
fn settings_column(ui: &mut egui::Ui, d: &mut PrintDraft, t: &Tokens, page_count: usize) {
    widgets::style_card_inputs(ui);
    group(ui, t, None, |ui| {
        form_row(ui, tl!("Printer:"), |ui| {
            let shown = d.printer.clone().unwrap_or_else(|| tl!("Save as PDF").to_string());
            egui::ComboBox::from_id_salt("printer").selected_text(shown).width(300.0).show_ui(ui, |ui| {
                for p in &d.printers {
                    let label = if p.default { crate::i18n::fmt(tl!("{name} (default)"), &[("name", &p.name)]) } else { p.name.clone() };
                    ui.selectable_value(&mut d.printer, Some(p.name.clone()), label);
                }
                ui.selectable_value(&mut d.printer, None, tl!("Save as PDF"));
            });
        });
        form_row(ui, tl!("Copies:"), |ui| {
            widgets::stepper_u32(ui, &mut d.copies, 1, 999, 1, "");
            ui.add_space(8.0);
            widgets::check_row(ui, &mut d.collate, tl!("Collate"));
            widgets::check_row(ui, &mut d.grayscale, tl!("Print in grayscale"));
        });
        form_row(ui, tl!("Two-sided:"), |ui| {
            combo(
                ui,
                "duplex",
                &mut d.duplex,
                &[
                    (spool::Duplex::Off, tl!("Off")),
                    (spool::Duplex::LongEdge, tl!("Flip on long edge")),
                    (spool::Duplex::ShortEdge, tl!("Flip on short edge")),
                ],
                200.0,
            );
        });
        form_row(ui, tl!("Paper:"), |ui| {
            d.paper = d.paper.min(PAPERS.len() - 1);
            egui::ComboBox::from_id_salt("paper").selected_text(paper_name(d.paper)).width(200.0).show_ui(ui, |ui| {
                for i in 0..PAPERS.len() {
                    ui.selectable_value(&mut d.paper, i, paper_name(i));
                }
            });
        });
    });
    group(ui, t, Some(tl!("Pages to Print")), |ui| {
        ui.horizontal(|ui| {
            for (which, label) in [(Which::All, tl!("All")), (Which::Current, tl!("Current page")), (Which::Range, tl!("Pages"))] {
                if widgets::radio_row(ui, d.which == which, label) {
                    d.which = which;
                }
            }
            let r = ui.add_enabled(
                d.which == Which::Range,
                egui::TextEdit::singleline(&mut d.range)
                    .hint_text(format!("1-{page_count}"))
                    .desired_width(100.0)
                    .background_color(t.field)
                    .margin(egui::Margin::symmetric(6, 4)),
            );
            if r.gained_focus() {
                d.which = Which::Range;
            }
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(tl!("More options:"));
            combo(
                ui,
                "subset",
                &mut d.subset,
                &[(Subset::All, tl!("All pages in range")), (Subset::Odd, tl!("Odd pages only")), (Subset::Even, tl!("Even pages only"))],
                160.0,
            );
            widgets::check_row(ui, &mut d.reverse, tl!("Reverse pages"));
        });
    });
    group(ui, t, Some(tl!("Page Sizing & Handling")), |ui| {
        ui.horizontal(|ui| {
            for (h, icon, label) in [
                (Handling::Size, "maximize-2", tl!("Size")),
                (Handling::Poster, "layout-grid", tl!("Poster")),
                (Handling::Multiple, "files", tl!("Multiple")),
                (Handling::Booklet, "book-open", tl!("Booklet")),
            ] {
                if widgets::option_button(ui, Some(icon), label, d.handling == h, 112.0).clicked() {
                    d.handling = h;
                    d.sheet = 0;
                }
            }
        });
        ui.add_space(8.0);
        handling_options(ui, d);
    });
    group(ui, t, Some(tl!("Orientation:")), |ui| {
        ui.horizontal_wrapped(|ui| {
            for (o, label) in [
                (Orientation::Auto, tl!("Auto portrait/landscape")),
                (Orientation::Portrait, tl!("Portrait")),
                (Orientation::Landscape, tl!("Landscape")),
            ] {
                if widgets::radio_row(ui, d.orientation == o, label) {
                    d.orientation = o;
                }
            }
        });
    });
    group(ui, t, Some(tl!("Comments & Forms")), |ui| {
        combo(
            ui,
            "content",
            &mut d.content,
            &[
                (Content::Document, tl!("Document")),
                (Content::DocumentAndMarkups, tl!("Document and markups")),
                (Content::DocumentAndStamps, tl!("Document and stamps")),
                (Content::FormFieldsOnly, tl!("Form fields only")),
            ],
            240.0,
        );
    });
}

/// Draw the sheet `d.sheet` of the layout into `area`: the paper, then every placed page.
fn preview(
    ui: &mut egui::Ui,
    d: &PrintDraft,
    area: Rect,
    error: Option<&str>,
    sheets: &[print::Sheet],
    sizes: &[(f64, f64)],
    thumb: &dyn Fn(usize) -> Option<egui::TextureId>,
) {
    let t = &Tokens::get(ui.ctx());
    ui.painter().rect_filled(area, 8.0, t.hover);
    match (error, sheets.get(d.sheet)) {
        (Some(e), _) => {
            ui.put(area.shrink(16.0), egui::Label::new(egui::RichText::new(e).color(t.text_muted)).wrap());
        }
        (None, Some(sheet)) => {
            let k = ((area.width() - 24.0) / sheet.size.0 as f32).min((area.height() - 24.0) / sheet.size.1 as f32);
            let paper = Rect::from_center_size(area.center(), vec2(sheet.size.0 as f32 * k, sheet.size.1 as f32 * k));
            ui.painter().rect_filled(paper, 0.0, Color32::WHITE);
            ui.painter().rect_stroke(paper, 0.0, Stroke::new(1.0, t.border), egui::StrokeKind::Outside);
            let to_screen = |x: f64, y: f64| -> Pos2 { pos2(paper.left() + x as f32 * k, paper.bottom() - y as f32 * k) };
            let painter = ui.painter().with_clip_rect(paper);
            for pl in &sheet.placed {
                let (dw, dh) = sizes[pl.page];
                let [x0, y0, x1, y1] = pl.clip;
                let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)].map(|(x, y)| {
                    let (sx, sy) = pl.matrix.apply(x, y);
                    to_screen(sx, sy)
                });
                // UVs of the visible part (texture y runs down from the page top).
                let uv = |x: f64, y: f64| pos2((x / dw) as f32, (1.0 - y / dh) as f32);
                let uvs = [uv(x0, y0), uv(x1, y0), uv(x1, y1), uv(x0, y1)];
                match thumb(pl.page) {
                    Some(tex) => {
                        let mut mesh = egui::Mesh::with_texture(tex);
                        for (p, u) in corners.iter().zip(uvs) {
                            mesh.vertices.push(egui::epaint::Vertex {
                                pos: *p,
                                uv: u,
                                color: if d.grayscale { Color32::from_gray(235) } else { Color32::WHITE },
                            });
                        }
                        mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
                        painter.add(egui::Shape::mesh(mesh));
                    }
                    None => {
                        painter.add(egui::Shape::convex_polygon(
                            corners.to_vec(),
                            Color32::from_gray(245),
                            Stroke::new(0.5, Color32::from_gray(180)),
                        ));
                        let c = corners.iter().fold(vec2(0.0, 0.0), |a, p| a + p.to_vec2()) / 4.0;
                        painter.text(
                            c.to_pos2(),
                            egui::Align2::CENTER_CENTER,
                            (pl.page + 1).to_string(),
                            theme::regular(11.0),
                            Color32::from_gray(120),
                        );
                    }
                }
            }
            for b in &sheet.borders {
                painter.rect_stroke(
                    Rect::from_two_pos(to_screen(b[0], b[1]), to_screen(b[2], b[3])),
                    0.0,
                    Stroke::new(0.6, Color32::BLACK),
                    egui::StrokeKind::Middle,
                );
            }
            for l in &sheet.lines {
                painter.line_segment([to_screen(l[0], l[1]), to_screen(l[2], l[3])], Stroke::new(0.6, Color32::BLACK));
            }
        }
        _ => {}
    }
}

/// Draw the dialog. `thumb` gives a page's thumbnail texture when there is one. Returns
/// (print, cancel).
pub(crate) fn body(
    ui: &mut egui::Ui,
    d: &mut PrintDraft,
    t: &Tokens,
    sizes: &[(f64, f64)],
    labels: &[String],
    thumb: &dyn Fn(usize) -> Option<egui::TextureId>,
) -> (bool, bool) {
    const SETTINGS_WIDTH: f32 = 540.0;
    const PREVIEW_WIDTH: f32 = 320.0;
    ui.set_width(SETTINGS_WIDTH + PREVIEW_WIDTH + 14.0);
    ui.horizontal(|ui| {
        let at = ui.cursor().left_center() + vec2(11.0, 0.0);
        crate::icons::paint(ui, Rect::from_center_size(at, vec2(22.0, 22.0)), "printer", 20.0, t.text);
        ui.add_space(28.0);
        ui.label(egui::RichText::new(tl!("Print")).font(theme::semibold(18.0)));
    });
    ui.add_space(10.0);
    let settings = d.settings(sizes.len(), labels);
    let sheets = settings.as_ref().ok().and_then(|s| print::layout(sizes, s).ok()).unwrap_or_default();
    d.sheet = d.sheet.min(sheets.len().saturating_sub(1));
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(SETTINGS_WIDTH);
            settings_column(ui, d, t, sizes.len());
        });
        ui.add_space(14.0);
        ui.vertical(|ui| {
            ui.set_width(PREVIEW_WIDTH);
            let size = sheets.first().map_or(String::new(), |s| format!("{:.2} × {:.2} in", s.size.0 / 72.0, s.size.1 / 72.0));
            ui.add_sized([PREVIEW_WIDTH, 22.0], egui::Label::new(egui::RichText::new(size).font(theme::medium(13.0)).color(t.text_muted)));
            let (area, _) = ui.allocate_exact_size(vec2(PREVIEW_WIDTH, 420.0), egui::Sense::hover());
            preview(ui, d, area, settings.as_ref().err().map(String::as_str), &sheets, sizes, thumb);
            ui.add_space(8.0);
            let n = sheets.len();
            ui.horizontal(|ui| {
                ui.add_space((PREVIEW_WIDTH - 190.0) / 2.0);
                ui.add_enabled_ui(d.sheet > 0, |ui| {
                    if crate::icons::button(ui, "chevron-left", 32.0, false, tl!("Previous sheet")).clicked() {
                        d.sheet -= 1;
                    }
                });
                let text = if n == 0 {
                    tl!("No sheets").to_string()
                } else {
                    crate::i18n::fmt(tl!("Sheet {s} of {n}"), &[("s", &(d.sheet + 1).to_string()), ("n", &n.to_string())])
                };
                ui.add_sized([110.0, 32.0], egui::Label::new(text));
                ui.add_enabled_ui(d.sheet + 1 < n, |ui| {
                    if crate::icons::button(ui, "chevron-right", 32.0, false, tl!("Next sheet")).clicked() {
                        d.sheet += 1;
                    }
                });
            });
        });
    });
    ui.add_space(8.0);
    let (mut go, mut cancel) = (false, false);
    crate::widgets::button_row(ui, |ui| {
        let (icon, label) = if d.printer.is_some() { ("printer", tl!("Print")) } else { ("save", tl!("Save as PDF")) };
        if ui.add_enabled_ui(settings.is_ok() && !sheets.is_empty(), |ui| widgets::dialog_action_button(ui, icon, label, true)).inner.clicked() {
            go = true;
        }
        if widgets::dialog_action_button(ui, "x", tl!("Cancel"), false).clicked() {
            cancel = true;
        }
    });
    (go, cancel)
}

/// One row of the top group: the label in a fixed-width cell, then the controls, all centred on
/// the row so labels, combo boxes and steppers line up.
fn form_row(ui: &mut egui::Ui, label: &str, controls: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        let (cell, response) = ui.allocate_exact_size(vec2(100.0, 1.0), egui::Sense::hover());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, label));
        let used = ui.horizontal(controls).response.rect;
        let font = egui::TextStyle::Body.resolve(ui.style());
        let at = egui::pos2(cell.left(), used.center().y);
        ui.painter().text(at, egui::Align2::LEFT_CENTER, label, font, ui.visuals().text_color());
    });
    ui.add_space(4.0);
}

/// The paper's name in the interface language (the sizes themselves are in `PAPERS`).
fn paper_name(index: usize) -> &'static str {
    match PAPERS.get(index).map(|p| p.0) {
        Some("US Letter") => tl!("US Letter"),
        Some("US Legal") => tl!("US Legal"),
        Some("Tabloid") => tl!("Tabloid"),
        Some(other) => other,
        None => "",
    }
}
