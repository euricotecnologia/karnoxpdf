//! Combine files: the files to combine, in order, each with the pages to take (all, or a range
//! such as "1-3, 6"); reorder, remove or add files, then Combine.

use std::sync::Arc;

use egui::{CornerRadius, Stroke};

use crate::theme::{self, Tokens};
use crate::{Dialog, KarnoxPDFApp, icons, widgets};

#[derive(Clone, Debug)]
pub struct CombineFile {
    pub name: String,
    pub bytes: Arc<Vec<u8>>,
    pub pages: usize,
    /// The pages to take ("" = all).
    pub range: String,
}

enum RowAction {
    Up(usize),
    Down(usize),
    Remove(usize),
}

pub(crate) fn body(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens, workspace: bool) -> (bool, bool) {
    if !workspace {
        ui.label(egui::RichText::new(tl!("Combine files")).font(theme::semibold(18.0)));
        ui.add_space(4.0);
        ui.label(egui::RichText::new(tl!("Files are combined in this order. Leave Pages empty to take every page.")).small().color(t.text_faint));
        ui.add_space(8.0);
    }
    let mut action = None;
    let n = app.combine_draft.len();
    egui::ScrollArea::vertical().max_height(360.0).auto_shrink([false, true]).show(ui, |ui| {
        egui::Grid::new("combine-files").num_columns(4).min_col_width(40.0).spacing([10.0, 6.0]).striped(true).show(ui, |ui| {
            ui.label(egui::RichText::new(tl!("File")).color(t.text_muted));
            ui.label(egui::RichText::new(tl!("Pages")).color(t.text_muted));
            ui.label("");
            ui.label("");
            ui.end_row();
            for (i, f) in app.combine_draft.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("{}.", i + 1)).strong().color(t.accent));
                    ui.add(icons::image("file-text", 16.0, t.icon));
                    ui.add(egui::Label::new(&f.name).truncate());
                    let count =
                        if f.pages == 1 { tl!("1 page").to_string() } else { crate::i18n::fmt(tl!("{n} pages"), &[("n", &f.pages.to_string())]) };
                    ui.label(egui::RichText::new(count).small().color(t.text_faint));
                });
                ui.add_sized([120.0, 22.0], egui::TextEdit::singleline(&mut f.range).hint_text(tl!("All pages")))
                    .on_hover_text(crate::i18n::fmt(tl!("Pages of {name} to combine, e.g. 1-3, 6"), &[("name", &f.name)]));
                ui.horizontal(|ui| {
                    if ui.add_enabled_ui(i > 0, |ui| icons::button(ui, "chevron-up", 24.0, false, tl!("Move up"))).inner.clicked() {
                        action = Some(RowAction::Up(i));
                    }
                    if ui.add_enabled_ui(i + 1 < n, |ui| icons::button(ui, "chevron-down", 24.0, false, tl!("Move down"))).inner.clicked() {
                        action = Some(RowAction::Down(i));
                    }
                });
                if icons::button(ui, "trash-2", 24.0, false, &crate::i18n::fmt(tl!("Remove {name}"), &[("name", &f.name)])).clicked() {
                    action = Some(RowAction::Remove(i));
                }
                ui.end_row();
            }
        });
    });
    match action {
        Some(RowAction::Up(i)) => app.combine_draft.swap(i, i - 1),
        Some(RowAction::Down(i)) => app.combine_draft.swap(i, i + 1),
        Some(RowAction::Remove(i)) => {
            app.combine_draft.remove(i);
        }
        None => {}
    }
    ui.add_space(10.0);
    let (mut go, mut cancel) = (false, false);
    ui.horizontal(|ui| {
        if widgets::pill_button(ui, tl!("Add files…"), false).clicked() {
            app.combine_dialog();
        }
        crate::widgets::button_row(ui, |ui| {
            let ok = app.combine_draft.len() >= 2;
            if ui
                .add_enabled_ui(ok, |ui| widgets::pill_button(ui, tl!("Combine"), true))
                .inner
                .on_disabled_hover_text(tl!("Add at least two files"))
                .clicked()
            {
                go = true;
            }
            if widgets::pill_button(ui, if workspace { tl!("Close") } else { tl!("Cancel") }, false).clicked() {
                cancel = true;
            }
        });
    });
    (go, cancel)
}

/// Full-page combining workspace opened from the Home card.
pub(crate) fn workspace(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(28.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(900.0);
            ui.heading(egui::RichText::new(tl!("Merge PDF")).font(theme::semibold(28.0)).color(t.text));
            ui.add_space(6.0);
            ui.label(egui::RichText::new(tl!("Files are combined in this order. Leave Pages empty to take every page.")).color(t.text_muted));
            ui.add_space(24.0);
            egui::Frame::NONE
                .fill(t.card)
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(CornerRadius::same(12))
                .inner_margin(egui::Margin::same(24))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width().min(850.0));
                    if app.combine_draft.is_empty() {
                        ui.add_space(24.0);
                        ui.vertical_centered(|ui| {
                            ui.add(icons::image("files", 40.0, t.accent));
                            ui.add_space(12.0);
                            ui.label(egui::RichText::new(tl!("Combine files")).font(theme::semibold(20.0)).color(t.text));
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new(tl!("Add at least two files")).color(t.text_muted));
                            ui.add_space(18.0);
                            if widgets::pill_button(ui, tl!("Add files…"), true).clicked() {
                                app.combine_dialog();
                            }
                        });
                        ui.add_space(30.0);
                    } else {
                        let (go, close) = body(ui, app, t, true);
                        if go {
                            app.combine_staged();
                        }
                        if close {
                            app.close_combine_workspace();
                        }
                    }
                });
        });
    });
}

impl KarnoxPDFApp {
    pub fn open_combine_workspace(&mut self) {
        self.deactivate_all_workspaces();
        self.combine_workspace_open = true;
        self.combine_workspace_active = true;
        self.active = None;
        self.dialog = None;
    }

    pub fn close_combine_workspace(&mut self) {
        self.combine_workspace_open = false;
        self.combine_workspace_active = false;
        self.active = self.views.len().checked_sub(1);
    }

    /// Add picked files to the Combine files list (and show it).
    pub(crate) fn stage_combine(&mut self, files: Vec<(String, Vec<u8>)>) {
        for (name, bytes) in files {
            let bytes = Arc::new(bytes);
            match karnoxpdf_render::inspect(bytes.clone(), None) {
                Ok(info) => self.combine_draft.push(CombineFile { name, bytes, pages: info.pages.len(), range: String::new() }),
                Err(e) => self.notify_fmt("Couldn't add {name}: {e}", &[("name", &name), ("e", &e.to_string())]),
            }
        }
        if self.combine_workspace_open {
            self.combine_workspace_active = true;
            self.split_workspace_active = false;
            self.compress_workspace_active = false;
            self.word_workspace_active = false;
            self.deactivate_office();
            self.active = None;
            self.dialog = None;
        } else {
            self.dialog = Some(Dialog::Combine);
        }
    }

    /// Combine the listed files into a new, unsaved document.
    pub fn combine_staged(&mut self) {
        if self.combine_draft.len() < 2 {
            return;
        }
        let count = self.combine_draft.len();
        let sources: Vec<(String, Arc<Vec<u8>>, Option<String>)> = self
            .combine_draft
            .iter()
            .map(|f| (crate::files::strip_pdf(&f.name).to_string(), f.bytes.clone(), Some(f.range.clone()).filter(|r| !r.trim().is_empty())))
            .collect();
        match self.session.combine_ranges(&sources) {
            Ok(bytes) => {
                let message = crate::i18n::fmt(tl!("Combined {n} files"), &[("n", &count.to_string())]);
                let old_count = self.views.len();
                self.open_created("Combined.pdf", bytes, &message);
                if self.views.len() > old_count {
                    self.combine_draft.clear();
                    self.combine_workspace_open = false;
                    self.combine_workspace_active = false;
                    self.left_open = false;
                }
            }
            Err(e) => self.notify_fmt("Couldn't combine files: {e}", &[("e", &e.to_string())]),
        }
    }
}
