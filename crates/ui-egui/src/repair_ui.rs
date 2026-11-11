//! Repair PDF workspace: Recover damaged or corrupted PDF files.

use std::sync::Arc;

use egui::{Align, Color32, CornerRadius, Layout, Stroke, vec2};
use karnoxpdf_engine::{RepairResult, repair_pdf};

use crate::panels::human_size;
use crate::theme::{self, Tokens};
use crate::{KarnoxPDFApp, icons, widgets};

/// Loaded PDF file source for repair.
#[derive(Clone, Debug)]
pub struct RepairPdfSource {
    pub name: String,
    pub bytes: Arc<Vec<u8>>,
    pub path: Option<String>,
}

/// Workspace state for Repair PDF.
#[derive(Clone, Debug, Default)]
pub struct RepairPdfWorkspace {
    pub open: bool,
    pub active: bool,
    pub source: Option<RepairPdfSource>,

    /// Password entered by the user (if encrypted).
    pub password: String,
    pub show_password: bool,

    /// Outcome of the repair operation.
    pub result: Option<RepairResult>,

    /// Error message to display, if any.
    pub error: Option<String>,
    /// Success notification message.
    pub success_note: Option<String>,
}

/// Draw an icon by allocating exact space.
fn draw_icon(ui: &mut egui::Ui, name: &str, size: f32, tint: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), egui::Sense::hover());
    icons::paint(ui, rect, name, size, tint);
}

/// Feature badge pill shown on the empty state.
fn feature_pill(ui: &mut egui::Ui, t: &Tokens, icon: &str, label: &str) {
    egui::Frame::NONE.fill(t.field).corner_radius(CornerRadius::same(12)).inner_margin(egui::Margin::symmetric(10, 5)).show(ui, |ui| {
        ui.horizontal(|ui| {
            draw_icon(ui, icon, 14.0, t.accent);
            ui.add_space(2.0);
            ui.label(egui::RichText::new(label).font(theme::medium(12.0)).color(t.text));
        });
    });
}

pub(crate) fn workspace(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(28.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(850.0);

            // Workspace Header
            ui.heading(egui::RichText::new(tl!("Repair PDF")).font(theme::semibold(28.0)).color(t.text));
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(tl!(
                    "Recover damaged or corrupted PDF files. Rebuild cross-references, restore page trees, and salvage content."
                ))
                .color(t.text_muted)
                .font(theme::regular(14.0)),
            );
            ui.add_space(24.0);

            // Main Card Frame
            egui::Frame::NONE
                .fill(t.card)
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(CornerRadius::same(12))
                .inner_margin(egui::Margin::same(24))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width().min(800.0));

                    let source = app.repair_workspace.source.clone();
                    if let Some(source) = source.as_ref() {
                        // File loaded card row
                        ui.label(egui::RichText::new(tl!("Selected PDF")).font(theme::semibold(15.0)).color(t.text));
                        ui.add_space(10.0);

                        egui::Frame::NONE.fill(t.field).corner_radius(CornerRadius::same(8)).inner_margin(egui::Margin::symmetric(14, 12)).show(
                            ui,
                            |ui| {
                                ui.horizontal(|ui| {
                                    draw_icon(ui, "bandage", 22.0, t.accent);
                                    ui.add_space(8.0);
                                    ui.add(egui::Label::new(egui::RichText::new(&source.name).font(theme::medium(14.0)).color(t.text)).truncate());
                                    ui.add_space(8.0);
                                    ui.label(egui::RichText::new(human_size(source.bytes.len())).color(t.text_muted));

                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if widgets::dialog_action_button(ui, "folder-open", tl!("Change file"), false).clicked() {
                                            app.repair_source_dialog();
                                        }
                                    });
                                });
                            },
                        );

                        ui.add_space(18.0);

                        // If not yet repaired, show repair options and launch button
                        if app.repair_workspace.result.is_none() {
                            ui.separator();
                            ui.add_space(14.0);

                            // Optional password input if document is protected
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(tl!("Password (if protected):")).font(theme::medium(13.0)).color(t.text_muted));
                                ui.add_space(8.0);
                                let password_edit = egui::TextEdit::singleline(&mut app.repair_workspace.password)
                                    .password(!app.repair_workspace.show_password)
                                    .hint_text(tl!("Leave empty if unencrypted"));
                                ui.add(password_edit.desired_width(280.0).margin(egui::vec2(8.0, 6.0)));
                                let icon = if app.repair_workspace.show_password { "eye-off" } else { "eye" };
                                if crate::icons::button(ui, icon, 30.0, false, tl!("Toggle password visibility")).clicked() {
                                    app.repair_workspace.show_password = !app.repair_workspace.show_password;
                                }
                            });

                            ui.add_space(18.0);
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(tl!("The original file will remain untouched.")).color(t.text_muted).small());
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if widgets::dialog_action_button(ui, "bandage", tl!("Repair PDF"), true).clicked() {
                                        app.execute_repair();
                                    }
                                });
                            });
                        } else if let Some(res) = app.repair_workspace.result.clone() {
                            // Result State
                            ui.separator();
                            ui.add_space(16.0);

                            // Status Banner
                            let (banner_bg, banner_stroke, banner_icon, banner_title, banner_sub) = if res.was_damaged {
                                (
                                    t.accent.gamma_multiply(0.12),
                                    Stroke::new(1.2, t.accent),
                                    "check",
                                    tl!("PDF successfully repaired!"),
                                    tl!("Structural damages were detected and reconstructed."),
                                )
                            } else {
                                (
                                    t.field,
                                    Stroke::new(1.0, t.border),
                                    "badge-check",
                                    tl!("PDF structure verified"),
                                    tl!("The document is valid and was cleanly re-indexed."),
                                )
                            };

                            egui::Frame::NONE
                                .fill(banner_bg)
                                .stroke(banner_stroke)
                                .corner_radius(CornerRadius::same(8))
                                .inner_margin(egui::Margin::symmetric(16, 12))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        draw_icon(ui, banner_icon, 24.0, t.accent);
                                        ui.add_space(10.0);
                                        ui.vertical(|ui| {
                                            ui.label(egui::RichText::new(banner_title).font(theme::semibold(15.0)).color(t.text));
                                            ui.label(egui::RichText::new(banner_sub).font(theme::regular(12.5)).color(t.text_muted));
                                        });
                                    });
                                });

                            ui.add_space(16.0);

                            // Metrics Tiles
                            ui.columns(3, |cols| {
                                // Pages
                                egui::Frame::NONE
                                    .fill(t.field)
                                    .stroke(Stroke::new(1.0, t.border))
                                    .corner_radius(CornerRadius::same(8))
                                    .inner_margin(egui::Margin::same(12))
                                    .show(&mut cols[0], |ui| {
                                        ui.horizontal(|ui| {
                                            draw_icon(ui, "files", 18.0, t.accent);
                                            ui.add_space(6.0);
                                            ui.label(egui::RichText::new(tl!("Pages")).font(theme::medium(12.5)).color(t.text_muted));
                                        });
                                        ui.add_space(4.0);
                                        ui.label(egui::RichText::new(format!("{}", res.page_count)).font(theme::semibold(18.0)).color(t.text));
                                    });

                                // Objects
                                egui::Frame::NONE
                                    .fill(t.field)
                                    .stroke(Stroke::new(1.0, t.border))
                                    .corner_radius(CornerRadius::same(8))
                                    .inner_margin(egui::Margin::same(12))
                                    .show(&mut cols[1], |ui| {
                                        ui.horizontal(|ui| {
                                            draw_icon(ui, "layers", 18.0, t.accent);
                                            ui.add_space(6.0);
                                            ui.label(egui::RichText::new(tl!("Objects")).font(theme::medium(12.5)).color(t.text_muted));
                                        });
                                        ui.add_space(4.0);
                                        ui.label(egui::RichText::new(format!("{}", res.object_count)).font(theme::semibold(18.0)).color(t.text));
                                    });

                                // Repaired Size
                                egui::Frame::NONE
                                    .fill(t.field)
                                    .stroke(Stroke::new(1.0, t.border))
                                    .corner_radius(CornerRadius::same(8))
                                    .inner_margin(egui::Margin::same(12))
                                    .show(&mut cols[2], |ui| {
                                        ui.horizontal(|ui| {
                                            draw_icon(ui, "file-check", 18.0, t.accent);
                                            ui.add_space(6.0);
                                            ui.label(egui::RichText::new(tl!("Output size")).font(theme::medium(12.5)).color(t.text_muted));
                                        });
                                        ui.add_space(4.0);
                                        ui.label(egui::RichText::new(human_size(res.bytes.len())).font(theme::semibold(18.0)).color(t.text));
                                    });
                            });

                            ui.add_space(16.0);

                            // Repair Diagnostics Log
                            ui.label(egui::RichText::new(tl!("Repair details")).font(theme::semibold(13.5)).color(t.text));
                            ui.add_space(6.0);
                            egui::Frame::NONE
                                .fill(t.field)
                                .stroke(Stroke::new(1.0, t.border))
                                .corner_radius(CornerRadius::same(8))
                                .inner_margin(egui::Margin::same(12))
                                .show(ui, |ui| {
                                    for entry in &res.log {
                                        ui.horizontal(|ui| {
                                            draw_icon(ui, "check", 14.0, t.accent);
                                            ui.add_space(4.0);
                                            ui.label(egui::RichText::new(format_repair_log_entry(entry)).font(theme::regular(12.0)).color(t.text));
                                        });
                                        ui.add_space(2.0);
                                    }
                                });

                            ui.add_space(20.0);

                            // Action buttons row
                            ui.horizontal(|ui| {
                                if widgets::dialog_action_button(ui, "rotate-ccw", tl!("Repair another file"), false).clicked() {
                                    app.repair_workspace.source = None;
                                    app.repair_workspace.result = None;
                                    app.repair_workspace.error = None;
                                    app.repair_workspace.success_note = None;
                                }

                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if widgets::dialog_action_button(ui, "save", tl!("Save Repaired PDF"), true).clicked() {
                                        app.execute_repair_and_save();
                                    }
                                    ui.add_space(8.0);
                                    if widgets::dialog_action_button(ui, "book-open", tl!("Open in KarnoxPDF"), false).clicked() {
                                        app.execute_repair_and_open();
                                    }
                                });
                            });
                        }
                    } else {
                        // Empty State: prompt user to pick file
                        ui.add_space(24.0);
                        ui.vertical_centered(|ui| {
                            draw_icon(ui, "bandage", 46.0, t.accent);
                            ui.add_space(14.0);
                            ui.label(egui::RichText::new(tl!("Select a damaged PDF to repair")).font(theme::semibold(18.0)).color(t.text));
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(tl!(
                                    "Files with broken cross-reference tables, damaged headers, or corrupted page trees can be recovered."
                                ))
                                .color(t.text_muted)
                                .font(theme::regular(13.0)),
                            );
                            ui.add_space(18.0);

                            if widgets::dialog_action_button(ui, "folder-open", tl!("Open file"), true).clicked() {
                                app.repair_source_dialog();
                            }

                            ui.add_space(24.0);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                feature_pill(ui, t, "layers", tl!("Xref rebuilding"));
                                feature_pill(ui, t, "files", tl!("Page tree recovery"));
                                feature_pill(ui, t, "file-check", tl!("Clean rewrite"));
                            });
                        });
                        ui.add_space(16.0);
                    }

                    // Display errors if any
                    if let Some(err) = &app.repair_workspace.error {
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
                    if let Some(note) = &app.repair_workspace.success_note {
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

impl KarnoxPDFApp {
    /// Open the Repair PDF workspace.
    pub fn open_repair_workspace(&mut self) {
        self.deactivate_all_workspaces();
        self.active = None;
        self.repair_workspace.open = true;
        self.repair_workspace.active = true;
        self.repair_workspace.error = None;
        self.repair_workspace.success_note = None;
    }

    /// Close the Repair PDF workspace.
    pub fn close_repair_workspace(&mut self) {
        self.repair_workspace.active = false;
        self.repair_workspace.open = false;
        self.repair_workspace.source = None;
        self.repair_workspace.password.clear();
        self.repair_workspace.result = None;
        self.repair_workspace.error = None;
        self.repair_workspace.success_note = None;
    }

    /// Stage a picked PDF file for repairing.
    pub fn stage_repair_source(&mut self, name: String, bytes: Vec<u8>, path: Option<String>) {
        self.repair_workspace.source = Some(RepairPdfSource { name, bytes: Arc::new(bytes), path });
        self.repair_workspace.password.clear();
        self.repair_workspace.result = None;
        self.repair_workspace.error = None;
        self.repair_workspace.success_note = None;
        self.open_repair_workspace();
    }

    /// Execute repair on the current source.
    pub fn execute_repair(&mut self) {
        let Some(source) = &self.repair_workspace.source else {
            self.repair_workspace.error = Some(tl!("No document loaded.").to_string());
            return;
        };

        let password = if self.repair_workspace.password.trim().is_empty() { None } else { Some(self.repair_workspace.password.trim()) };

        match repair_pdf(&source.bytes, password) {
            Ok(result) => {
                self.repair_workspace.error = None;
                self.repair_workspace.result = Some(result);
            }
            Err(e) => {
                self.repair_workspace.error = Some(format!("{e}"));
            }
        }
    }

    /// Suggested filename for the repaired document.
    pub fn repaired_filename(&self) -> String {
        let stem = self.repair_workspace.source.as_ref().map(|s| s.name.trim_end_matches(".pdf").trim_end_matches(".PDF")).unwrap_or("document");
        let safe: String = stem.chars().map(|ch| if ch.is_alphanumeric() || " -_.,()".contains(ch) { ch } else { '_' }).collect();
        let trimmed = safe.trim_matches([' ', '.']);
        let stem = if trimmed.is_empty() { "document" } else { trimmed };
        format!("{stem}_repaired.pdf")
    }

    /// Save the repaired PDF using native file dialog or download.
    pub fn execute_repair_and_save(&mut self) {
        let Some(result) = self.repair_workspace.result.clone() else {
            self.execute_repair();
            if self.repair_workspace.result.is_none() {
                return;
            }
            return self.execute_repair_and_save();
        };

        let filename = self.repaired_filename();
        let bytes = Arc::new(result.bytes);

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
                        app.repair_workspace.success_note = Some(tl!("Repaired PDF saved successfully!").to_string());
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

    /// Open repaired PDF directly in the KarnoxPDF tabs.
    pub fn execute_repair_and_open(&mut self) {
        let Some(result) = self.repair_workspace.result.clone() else {
            self.execute_repair();
            if self.repair_workspace.result.is_none() {
                return;
            }
            return self.execute_repair_and_open();
        };

        let filename = self.repaired_filename();
        let _ = self.open_bytes(&filename, None, result.bytes);
        self.close_repair_workspace();
    }
}

/// Localize a diagnostic message from the repair engine.
pub fn format_repair_log_entry(entry: &str) -> String {
    if let Some(rest) = entry.strip_prefix("restored ")
        && let Some((n, "")) = rest.split_once(" orphaned pages into the document page tree")
    {
        let template = crate::i18n::t("restored {n} orphaned pages into the document page tree");
        return crate::i18n::fmt(template, &[("n", n)]);
    }
    if let Some(rest) = entry.strip_prefix("corrected page count in tree (")
        && let Some((n, "")) = rest.split_once(" pages)")
    {
        let template = crate::i18n::t("corrected page count in tree ({n} pages)");
        return crate::i18n::fmt(template, &[("n", n)]);
    }
    if let Some(rest) = entry.strip_prefix("rebuilt cross-reference index from ")
        && let Some((n, "")) = rest.split_once(" object headers")
    {
        let template = crate::i18n::t("rebuilt cross-reference index from {n} object headers");
        return crate::i18n::fmt(template, &[("n", n)]);
    }
    if let Some(rest) = entry.strip_prefix("synthesized missing catalog and pages root for ")
        && let Some((n, "")) = rest.split_once(" recovered pages")
    {
        let template = crate::i18n::t("synthesized missing catalog and pages root for {n} recovered pages");
        return crate::i18n::fmt(template, &[("n", n)]);
    }
    if let Some(rest) = entry.strip_prefix("cross-reference data unreadable (")
        && let Some((e, "")) = rest.split_once("); rebuilding from object headers")
    {
        let template = crate::i18n::t("cross-reference data unreadable ({e}); rebuilding from object headers");
        return crate::i18n::fmt(template, &[("e", e)]);
    }
    if let Some(rest) = entry.strip_prefix("cross-reference chain loops at offset ")
        && let Some((off, "")) = rest.split_once("; stopped")
    {
        let template = crate::i18n::t("cross-reference chain loops at offset {off}; stopped");
        return crate::i18n::fmt(template, &[("off", off)]);
    }
    if let Some(rest) = entry.strip_prefix("xref at ")
        && let Some((off, tail)) = rest.split_once(" unreadable (")
        && let Some((e, "")) = tail.split_once("); retrying relative to the header")
    {
        let template = crate::i18n::t("xref at {off} unreadable ({e}); retrying relative to the header");
        return crate::i18n::fmt(template, &[("off", off), ("e", e)]);
    }
    if let Some(rest) = entry.strip_prefix("older cross-reference section at ")
        && let Some((off, tail)) = rest.split_once(" is unreadable: ")
    {
        let template = crate::i18n::t("older cross-reference section at {off} is unreadable: {e}");
        return crate::i18n::fmt(template, &[("off", off), ("e", tail)]);
    }
    if let Some(rest) = entry.strip_prefix("hybrid /XRefStm at ")
        && let Some((x, tail)) = rest.split_once(" unreadable: ")
    {
        let template = crate::i18n::t("hybrid /XRefStm at {x} unreadable: {e}");
        return crate::i18n::fmt(template, &[("x", x), ("e", tail)]);
    }

    crate::i18n::t(entry).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{self, Lang};

    #[test]
    fn repair_log_entries_translate_in_portuguese() {
        let pt = Lang::from_code("pt-br").expect("pt-br registered");
        i18n::set_current(pt);

        assert_eq!(
            format_repair_log_entry("verified cross-reference table and page structure (no errors found)"),
            "Tabela de referências cruzadas e estrutura de páginas verificadas (nenhum erro encontrado)"
        );
        assert_eq!(
            format_repair_log_entry("restored 4 orphaned pages into the document page tree"),
            "4 páginas órfãs restauradas na árvore de páginas do documento"
        );
        assert_eq!(format_repair_log_entry("corrected page count in tree (12 pages)"), "Contagem de páginas corrigida na árvore (12 páginas)");
        assert_eq!(format_repair_log_entry("removed broken linearization dictionary"), "Dicionário de linearização corrompido removido");

        i18n::set_current(Lang::EN);
    }
}
