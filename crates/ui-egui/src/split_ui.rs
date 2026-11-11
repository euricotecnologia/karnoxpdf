//! Dedicated Split PDF tab. The selected source remains untouched; parts are saved as new files.

use std::sync::Arc;

use egui::{Align, CornerRadius, Layout, Stroke};
use karnoxpdf_engine::{Session, SplitBy};

use crate::theme::{self, Tokens};
use crate::{KarnoxPDFApp, SplitMode, SplitPlan, i18n, icons, widgets};

#[derive(Clone, Debug)]
pub struct SplitSource {
    pub name: String,
    pub bytes: Arc<Vec<u8>>,
    pub pages: usize,
    /// Zero-based pages targeted by top-level bookmarks.
    pub bookmark_cuts: Vec<usize>,
}

/// Convert entered 1-based page numbers into 0-based cut positions.
fn parse_cuts(input: &str, pages: usize) -> Result<Vec<usize>, &'static str> {
    let mut cuts = Vec::new();
    for token in input.split(|c: char| c == ',' || c == ';' || c.is_whitespace()).filter(|s| !s.is_empty()) {
        let page = token.parse::<usize>().map_err(|_| "Enter page numbers separated by commas, such as 3, 6.")?;
        if !(2..=pages).contains(&page) {
            return Err("Choose page numbers from 2 through the last page.");
        }
        cuts.push(page - 1);
    }
    cuts.sort_unstable();
    cuts.dedup();
    if cuts.is_empty() {
        return Err("Enter at least one page where a new file should start.");
    }
    Ok(cuts)
}

fn plan(app: &KarnoxPDFApp, source: &SplitSource) -> Result<(SplitPlan, Option<usize>), &'static str> {
    let n = source.pages;
    match app.split_draft.mode {
        SplitMode::Pages => {
            let every = app.split_draft.every.clamp(1, n.max(1));
            Ok((SplitPlan::By(SplitBy::PageCount(every)), Some(n.div_ceil(every))))
        }
        SplitMode::Selection => {
            let cuts = parse_cuts(&app.split_before_input, n)?;
            let count = cuts.len().saturating_add(1);
            Ok((SplitPlan::By(SplitBy::Before(cuts)), Some(count)))
        }
        SplitMode::Size => {
            let mb = app.split_draft.size_mb;
            if !mb.is_finite() || mb < 0.05 || mb > 2000.0 {
                return Err("Choose a file size between 0.05 and 2000 MB.");
            }
            Ok((SplitPlan::Size((mb * 1_048_576.0) as usize), None))
        }
        SplitMode::Bookmarks => {
            if source.bookmark_cuts.is_empty() {
                return Err("This PDF has no top-level bookmarks that start a new part.");
            }
            Ok((SplitPlan::Bookmarks, Some(source.bookmark_cuts.len().saturating_add(1))))
        }
    }
}

pub(crate) fn workspace(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(28.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(900.0);
            ui.heading(egui::RichText::new(tl!("Split PDF")).font(theme::semibold(28.0)).color(t.text));
            ui.add_space(6.0);
            ui.label(egui::RichText::new(tl!("Choose a PDF and how its pages should be divided.")).color(t.text_muted));
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
                    let source = app.split_source.clone();
                    if let Some(source) = source.as_ref() {
                        egui::Frame::NONE.fill(t.field).corner_radius(CornerRadius::same(7)).inner_margin(egui::Margin::symmetric(14, 12)).show(
                            ui,
                            |ui| {
                                ui.horizontal(|ui| {
                                    ui.add(icons::image("file-text", 20.0, t.accent));
                                    ui.add_space(6.0);
                                    ui.label(egui::RichText::new(&source.name).font(theme::medium(14.0)).color(t.text));
                                    ui.label(
                                        egui::RichText::new(if source.pages == 1 {
                                            tl!("1 page").to_string()
                                        } else {
                                            i18n::fmt(tl!("{n} pages"), &[("n", &source.pages.to_string())])
                                        })
                                        .color(t.text_muted),
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if widgets::dialog_action_button(ui, "folder-open", tl!("Change file"), false).clicked() {
                                            app.split_source_dialog();
                                        }
                                    });
                                });
                            },
                        );
                    } else {
                        ui.add_space(20.0);
                        ui.vertical_centered(|ui| {
                            ui.add(icons::image("scissors", 38.0, t.accent));
                            ui.add_space(10.0);
                            ui.label(egui::RichText::new(tl!("Select a PDF to split")).font(theme::semibold(18.0)).color(t.text));
                            ui.add_space(14.0);
                            if widgets::dialog_action_button(ui, "folder-open", tl!("Open file"), true).clicked() {
                                app.split_source_dialog();
                            }
                        });
                        ui.add_space(24.0);
                    }
                    if let Some(source) = source.as_ref() {
                        ui.add_space(22.0);
                        ui.separator();
                        ui.add_space(18.0);
                        ui.with_layout(Layout::top_down(Align::Min), |ui| {
                            ui.label(egui::RichText::new(tl!("Split method")).font(theme::semibold(15.0)).color(t.text));
                            ui.add_space(10.0);
                            let draft = &mut app.split_draft;
                            if widgets::radio_row(ui, draft.mode == SplitMode::Pages, tl!("Number of pages")) {
                                draft.mode = SplitMode::Pages;
                            }
                            ui.add_enabled_ui(draft.mode == SplitMode::Pages, |ui| {
                                ui.horizontal(|ui| {
                                    ui.add_space(22.0);
                                    ui.label(tl!("Pages per file"));
                                    ui.add(egui::DragValue::new(&mut draft.every).range(1..=source.pages.max(1)));
                                });
                            });
                            ui.add_space(8.0);
                            if widgets::radio_row(ui, draft.mode == SplitMode::Selection, tl!("Before page numbers")) {
                                draft.mode = SplitMode::Selection;
                            }
                            ui.add_enabled_ui(draft.mode == SplitMode::Selection, |ui| {
                                ui.horizontal(|ui| {
                                    ui.add_space(22.0);
                                    ui.add_sized([230.0, 30.0], egui::TextEdit::singleline(&mut app.split_before_input).hint_text("3, 6"));
                                    ui.label(egui::RichText::new(tl!("Start a new file at each page listed.")).color(t.text_muted).small());
                                });
                            });
                            ui.add_space(8.0);
                            if widgets::radio_row(ui, draft.mode == SplitMode::Size, tl!("File size")) {
                                draft.mode = SplitMode::Size;
                            }
                            ui.add_enabled_ui(draft.mode == SplitMode::Size, |ui| {
                                ui.horizontal(|ui| {
                                    ui.add_space(22.0);
                                    ui.label(tl!("At most"));
                                    ui.add(egui::DragValue::new(&mut draft.size_mb).range(0.05..=2000.0).speed(0.1).suffix(" MB"));
                                });
                            });
                            ui.add_space(8.0);
                            let has_bookmarks = !source.bookmark_cuts.is_empty();
                            ui.add_enabled_ui(has_bookmarks, |ui| {
                                let label = i18n::fmt(tl!("Top-level bookmarks ({n})"), &[("n", &source.bookmark_cuts.len().to_string())]);
                                if widgets::radio_row(ui, draft.mode == SplitMode::Bookmarks, &label) {
                                    draft.mode = SplitMode::Bookmarks;
                                }
                            });
                            if !has_bookmarks && draft.mode == SplitMode::Bookmarks {
                                draft.mode = SplitMode::Pages;
                            }
                        });
                        ui.add_space(22.0);
                        let result = plan(app, source);
                        match &result {
                            Ok((_, Some(count))) if *count > 1 => ui.label(
                                egui::RichText::new(i18n::fmt(
                                    tl!("Creates {f} files from {n} pages."),
                                    &[("f", &count.to_string()), ("n", &source.pages.to_string())],
                                ))
                                .color(t.text_muted),
                            ),
                            Ok((_, None)) => ui.label(
                                egui::RichText::new(i18n::fmt(
                                    tl!("Each file holds as many of the {n} pages as fit."),
                                    &[("n", &source.pages.to_string())],
                                ))
                                .color(t.text_muted),
                            ),
                            Ok(_) => ui.label(egui::RichText::new(tl!("Choose settings that create at least two files.")).color(t.text_muted)),
                            Err(message) => ui.label(egui::RichText::new(tl!(message)).color(t.text_muted)),
                        };
                        ui.add_space(18.0);
                        ui.separator();
                        ui.add_space(14.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(tl!("The source PDF will not be changed.")).color(t.text_muted).small());
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let ready = matches!(&result, Ok((_, None)) | Ok((_, Some(2..))));
                                if ui
                                    .add_enabled_ui(ready, |ui| widgets::dialog_action_button(ui, "scissors", tl!("Split and save"), true))
                                    .inner
                                    .clicked()
                                    && let Ok((plan, _)) = result
                                {
                                    app.split_workspace_now(&plan);
                                }
                            });
                        });
                    }
                });
        });
    });
}

impl KarnoxPDFApp {
    pub fn open_split_workspace(&mut self) {
        self.deactivate_all_workspaces();
        self.split_workspace_open = true;
        self.split_workspace_active = true;
        self.active = None;
        self.dialog = None;
    }

    pub(crate) fn close_split_workspace(&mut self) {
        self.split_workspace_open = false;
        self.split_workspace_active = false;
        self.active = self.views.len().checked_sub(1);
    }

    pub(crate) fn stage_split_source(&mut self, name: String, bytes: Vec<u8>) {
        let bytes = Arc::new(bytes);
        match karnoxpdf_render::inspect(bytes.clone(), None) {
            Ok(info) => {
                let pages = info.pages.len();
                let mut bookmark_cuts: Vec<usize> = info.outline.iter().filter_map(|m| m.page).filter(|p| *p > 0 && *p < pages).collect();
                bookmark_cuts.sort_unstable();
                bookmark_cuts.dedup();
                self.split_source = Some(SplitSource { name, bytes, pages, bookmark_cuts });
                self.open_split_workspace();
            }
            Err(e) => self.notify_fmt("Couldn't open {name}: {e}", &[("name", &name), ("e", &e.to_string())]),
        }
    }

    pub fn split_workspace_now(&mut self, plan: &SplitPlan) {
        let Some(source) = self.split_source.clone() else { return };
        let mut session = Session::new();
        let id = match session.open(&source.name, None, source.bytes, None) {
            Ok(id) => id,
            Err(e) => {
                self.notify_fmt("Couldn't split: {e}", &[("e", &e.to_string())]);
                return;
            }
        };
        match crate::files::split_named_parts(&session, id, plan, crate::files::strip_pdf(&source.name)) {
            Ok(named) if named.len() > 1 => self.write_files(&named, "Choose a folder for the split files"),
            Ok(_) => self.notify_tr("Choose settings that create at least two files."),
            Err(e) => self.notify_fmt("Couldn't split: {e}", &[("e", &e)]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_cuts;

    #[test]
    fn page_boundaries_are_checked_and_deduplicated() {
        assert_eq!(parse_cuts("6, 3; 3", 8), Ok(vec![2, 5]));
        assert!(parse_cuts("1, 3", 8).is_err());
        assert!(parse_cuts("9", 8).is_err());
        assert!(parse_cuts("three", 8).is_err());
        assert!(parse_cuts("", 8).is_err());
    }
}
