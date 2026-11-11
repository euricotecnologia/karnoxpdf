//! Unlock PDF workspace: Remove password protection and permission restrictions from PDF files.

use std::sync::Arc;

use egui::{Align, Color32, CornerRadius, Layout, Stroke, vec2};
use karnoxpdf_engine::{ProtectionStatus, inspect_protection, unlock_pdf};

use crate::panels::human_size;
use crate::theme::{self, Tokens};
use crate::{KarnoxPDFApp, icons};

/// Loaded PDF file source for unlocking.
#[derive(Clone, Debug)]
pub struct UnlockPdfSource {
    pub name: String,
    pub bytes: Arc<Vec<u8>>,
    pub path: Option<String>,
    pub status: ProtectionStatus,
    pub page_count: Option<usize>,
}

/// Workspace state for Unlock PDF.
#[derive(Clone, Debug, Default)]
pub struct UnlockPdfWorkspace {
    pub open: bool,
    pub active: bool,
    pub source: Option<UnlockPdfSource>,

    /// Password entered by the user.
    pub password: String,
    /// Toggle visibility of the password in the UI.
    pub show_password: bool,

    /// Error message to display, if any.
    pub error: Option<String>,
    /// Informational or success note.
    pub success_note: Option<String>,
}

/// Draw an icon by allocating exact space.
fn draw_icon(ui: &mut egui::Ui, name: &str, size: f32, tint: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), egui::Sense::hover());
    icons::paint(ui, rect, name, size, tint);
}

/// Draw a clearly visible bordered input box for passwords with an optional eye toggle.
fn bordered_password_input(ui: &mut egui::Ui, t: &Tokens, text: &mut String, hint: &str, show_password: bool, mut eye_toggle: Option<&mut bool>) {
    let stroke_color = if t.kind == theme::ThemeKind::Light { Color32::from_rgb(0xB0, 0xB6, 0xC4) } else { Color32::from_rgb(0x4A, 0x50, 0x60) };
    let bg_color = if t.kind == theme::ThemeKind::Light { Color32::from_rgb(0xFA, 0xFA, 0xFC) } else { Color32::from_rgb(0x20, 0x24, 0x2E) };

    let frame = egui::Frame::NONE
        .fill(bg_color)
        .stroke(Stroke::new(1.5, stroke_color))
        .corner_radius(CornerRadius::same(7))
        .inner_margin(egui::Margin::symmetric(10, 7));

    frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            let width_deduction = if eye_toggle.is_some() { 32.0 } else { 0.0 };
            let edit = egui::TextEdit::singleline(text)
                .password(!show_password)
                .hint_text(hint)
                .frame(egui::Frame::NONE)
                .desired_width((ui.available_width() - width_deduction).max(60.0))
                .font(theme::regular(13.5));
            ui.add(edit);

            if let Some(toggle_val) = eye_toggle.as_mut() {
                let icon = if **toggle_val { "eye-off" } else { "eye" };
                if crate::icons::button(ui, icon, 28.0, false, tl!("Toggle password visibility")).clicked() {
                    **toggle_val = !**toggle_val;
                }
            }
        });
    });
}

/// Render the Unlock PDF workspace.
pub fn workspace(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens) {
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

    // Top Header Banner
    egui::Frame::NONE.fill(t.card).stroke(Stroke::new(1.0, t.border)).inner_margin(egui::Margin::symmetric(24, 14)).show(ui, |ui| {
        ui.horizontal(|ui| {
            draw_icon(ui, "lock-open", 22.0, t.accent);
            ui.add_space(10.0);
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(tl!("Unlock PDF")).font(theme::semibold(17.0)).color(t.text));
                ui.label(
                    egui::RichText::new(tl!("Remove password protection and permission restrictions from your PDF document."))
                        .font(theme::regular(12.0))
                        .color(t.text_muted),
                );
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if app.unlock_workspace.source.is_some() && crate::widgets::dialog_action_button(ui, "x", tl!("Clear"), false).clicked() {
                    app.unlock_workspace.source = None;
                    app.unlock_workspace.password.clear();
                    app.unlock_workspace.error = None;
                    app.unlock_workspace.success_note = None;
                }
            });
        });
    });

    ui.add_space(16.0);

    // Main workspace content
    if app.unlock_workspace.source.is_none() {
        show_empty_upload(ui, app, t);
    } else {
        show_unlock_form(ui, app, t);
    }
}

/// Empty upload drop zone when no document is loaded.
fn show_empty_upload(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens) {
    ui.vertical_centered(|ui| {
        ui.add_space(36.0);

        let card_frame = egui::Frame::NONE
            .fill(t.card)
            .stroke(Stroke::new(1.5, t.border))
            .corner_radius(CornerRadius::same(12))
            .inner_margin(egui::Margin::symmetric(48, 40));

        card_frame.show(ui, |ui| {
            ui.set_max_width(580.0);
            ui.vertical_centered(|ui| {
                draw_icon(ui, "lock-open", 56.0, t.accent);
                ui.add_space(16.0);

                ui.label(egui::RichText::new(tl!("Select PDF to unlock")).font(theme::semibold(19.0)).color(t.text));
                ui.add_space(6.0);

                ui.label(
                    egui::RichText::new(tl!(
                        "Choose a password-protected PDF to decrypt and strip all security restrictions (printing, editing, copying)."
                    ))
                    .font(theme::regular(13.0))
                    .color(t.text_muted),
                );

                ui.add_space(26.0);

                if crate::widgets::dialog_action_button(ui, "folder-open", tl!("Select PDF file"), true).clicked() {
                    app.unlock_source_dialog();
                }

                ui.add_space(14.0);
                ui.label(
                    egui::RichText::new(tl!("Local and confidential: your file is processed entirely on your machine."))
                        .font(theme::regular(11.0))
                        .color(t.text_muted),
                );
            });
        });
    });
}

/// Unlocking form with file status, password input, and live summary.
fn show_unlock_form(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens) {
    let avail_width = ui.available_width().max(600.0);
    let left_width = (avail_width * 0.58).clamp(360.0, 560.0);

    ui.horizontal_top(|ui| {
        // Left Column: Controls & Input
        ui.allocate_ui_with_layout(vec2(left_width, ui.available_height()), Layout::top_down(Align::Min), |ui| {
            ui.set_min_width(left_width);
            ui.set_max_width(left_width);

            // Source File Card
            egui::Frame::NONE
                .fill(t.card)
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(CornerRadius::same(10))
                .inner_margin(egui::Margin::symmetric(18, 14))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let (name, size_str, status_badge) = if let Some(src) = &app.unlock_workspace.source {
                        let sz = human_size(src.bytes.len());
                        let badge = match src.status {
                            ProtectionStatus::ProtectedNeedsPassword => (tl!("Password Protected"), Color32::from_rgb(0xD9, 0x53, 0x4F)),
                            ProtectionStatus::ProtectedEmptyPassword => (tl!("Restrictions (No password)"), Color32::from_rgb(0x3B, 0x78, 0xC4)),
                            ProtectionStatus::NotProtected => (tl!("Not Protected"), Color32::from_rgb(0x2E, 0x85, 0x55)),
                        };
                        (src.name.clone(), sz, badge)
                    } else {
                        (String::new(), String::new(), (tl!("Unknown"), t.text_muted))
                    };

                    ui.horizontal(|ui| {
                        draw_icon(ui, "file-text", 26.0, t.accent);
                        ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(&name).font(theme::semibold(14.0)).color(t.text));
                            ui.label(egui::RichText::new(&size_str).font(theme::regular(12.0)).color(t.text_muted));
                        });

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if crate::widgets::dialog_action_button(ui, "folder-open", tl!("Change file"), false).clicked() {
                                app.unlock_source_dialog();
                            }
                        });
                    });

                    ui.add_space(10.0);

                    // Status Pill
                    ui.horizontal(|ui| {
                        let (badge_txt, badge_col) = status_badge;
                        let pill = egui::Frame::NONE
                            .fill(badge_col.gamma_multiply(0.15))
                            .stroke(Stroke::new(1.0, badge_col))
                            .corner_radius(CornerRadius::same(5))
                            .inner_margin(egui::Margin::symmetric(8, 3));

                        pill.show(ui, |ui| {
                            ui.label(egui::RichText::new(badge_txt).font(theme::semibold(11.0)).color(badge_col));
                        });
                    });
                });

            ui.add_space(14.0);

            // Password Input Box
            egui::Frame::NONE
                .fill(t.card)
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(CornerRadius::same(10))
                .inner_margin(egui::Margin::symmetric(18, 16))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let is_needs_password =
                        app.unlock_workspace.source.as_ref().is_some_and(|s| s.status == ProtectionStatus::ProtectedNeedsPassword);

                    if is_needs_password {
                        ui.label(egui::RichText::new(tl!("Document Password")).font(theme::semibold(14.0)).color(t.text));
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(tl!("Enter the open password or owner/permissions password to unlock:"))
                                .font(theme::regular(12.0))
                                .color(t.text_muted),
                        );
                        ui.add_space(10.0);

                        let show_pw = app.unlock_workspace.show_password;
                        bordered_password_input(
                            ui,
                            t,
                            &mut app.unlock_workspace.password,
                            tl!("Enter password"),
                            show_pw,
                            Some(&mut app.unlock_workspace.show_password),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new(tl!("No password required!")).font(theme::semibold(14.0)).color(Color32::from_rgb(0x2E, 0x85, 0x55)),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(tl!(
                                "This document can be decrypted and have all its restrictions removed immediately without entering a password."
                            ))
                            .font(theme::regular(12.0))
                            .color(t.text_muted),
                        );
                    }
                });

            ui.add_space(14.0);

            // Error or Note
            if let Some(err) = &app.unlock_workspace.error {
                egui::Frame::NONE
                    .fill(Color32::from_rgb(0xF8, 0xD7, 0xDA))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(0xF5, 0xC6, 0xCB)))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(egui::Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            draw_icon(ui, "alert-circle", 16.0, Color32::from_rgb(0x72, 0x1C, 0x24));
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new(err).font(theme::medium(12.0)).color(Color32::from_rgb(0x72, 0x1C, 0x24)));
                        });
                    });
                ui.add_space(10.0);
            }

            if let Some(note) = &app.unlock_workspace.success_note {
                egui::Frame::NONE
                    .fill(Color32::from_rgb(0xD4, 0xED, 0xDA))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(0xC3, 0xE6, 0xCB)))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(egui::Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            draw_icon(ui, "check-circle", 16.0, Color32::from_rgb(0x15, 0x57, 0x24));
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new(note).font(theme::medium(12.0)).color(Color32::from_rgb(0x15, 0x57, 0x24)));
                        });
                    });
                ui.add_space(10.0);
            }

            // Primary & Secondary Action Buttons
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if crate::widgets::dialog_action_button(ui, "lock-open", tl!("Unlock and save PDF"), true).clicked() {
                    app.execute_unlock_and_save();
                }

                ui.add_space(10.0);

                if crate::widgets::dialog_action_button(ui, "book-open", tl!("Open in KarnoxPDF"), false).clicked() {
                    app.execute_unlock_and_open();
                }
            });
        });

        ui.add_space(20.0);

        // Right Column: Information & Summary Preview
        ui.vertical(|ui| {
            egui::Frame::NONE
                .fill(t.card)
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(CornerRadius::same(10))
                .inner_margin(egui::Margin::symmetric(22, 20))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(tl!("Security Removal Summary")).font(theme::semibold(15.0)).color(t.text));
                    ui.add_space(12.0);

                    let features = [
                        (tl!("Remove Open Password"), tl!("The decrypted PDF will open freely in any viewer without asking for a password.")),
                        (tl!("Restore Printing Rights"), tl!("Allows printing in maximum resolution on all printers.")),
                        (tl!("Restore Content Copying"), tl!("Enables selecting, copying text, and extracting images.")),
                        (tl!("Restore Form & Editing"), tl!("Full permission to edit, sign, comment, and fill forms.")),
                        (tl!("100% Standard ISO PDF"), tl!("Clean, standards-compliant PDF file readable by any device.")),
                    ];

                    for (title, desc) in features {
                        ui.horizontal_top(|ui| {
                            draw_icon(ui, "check", 16.0, Color32::from_rgb(0x2E, 0x85, 0x55));
                            ui.add_space(8.0);
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(title).font(theme::semibold(13.0)).color(t.text));
                                ui.label(egui::RichText::new(desc).font(theme::regular(11.5)).color(t.text_muted));
                            });
                        });
                        ui.add_space(10.0);
                    }
                });
        });
    });
}

impl KarnoxPDFApp {
    /// Open the Unlock PDF workspace.
    pub fn open_unlock_workspace(&mut self) {
        self.deactivate_all_workspaces();
        self.active = None;
        self.unlock_workspace.open = true;
        self.unlock_workspace.active = true;
        self.unlock_workspace.error = None;
        self.unlock_workspace.success_note = None;
    }

    /// Close the Unlock PDF workspace.
    pub fn close_unlock_workspace(&mut self) {
        self.unlock_workspace.active = false;
        self.unlock_workspace.open = false;
        self.unlock_workspace.source = None;
        self.unlock_workspace.password.clear();
        self.unlock_workspace.error = None;
        self.unlock_workspace.success_note = None;
    }

    /// Stage a picked PDF file for the Unlock PDF workspace.
    pub fn stage_unlock_source(&mut self, name: String, bytes: Vec<u8>, path: Option<String>) {
        let status = inspect_protection(&bytes).unwrap_or(ProtectionStatus::NotProtected);
        let bytes_arc = Arc::new(bytes);
        let page_count = self.session.page_count_of(&name, &bytes_arc).ok();

        self.unlock_workspace.source = Some(UnlockPdfSource { name, bytes: bytes_arc, path, status, page_count });
        self.unlock_workspace.password.clear();
        self.unlock_workspace.error = None;
        self.unlock_workspace.success_note = None;
        self.open_unlock_workspace();
    }

    /// Decrypt and unlock the current document, returning (suggested_filename, unlocked_bytes).
    pub fn generate_unlocked_pdf(&mut self) -> Result<(String, Vec<u8>), String> {
        let Some(source) = &self.unlock_workspace.source else {
            return Err(tl!("No document loaded.").to_string());
        };

        let password = if self.unlock_workspace.password.trim().is_empty() { None } else { Some(self.unlock_workspace.password.trim()) };

        let unlocked_bytes = unlock_pdf(&source.bytes, password).map_err(|e| match e {
            karnoxpdf_engine::EditError::Protection(msg) => {
                if msg.contains("incorrect") {
                    tl!("Incorrect password. Please verify and try again.").to_string()
                } else if msg.contains("required") {
                    tl!("This document is protected and requires a password to unlock.").to_string()
                } else {
                    msg
                }
            }
            other => other.to_string(),
        })?;

        let stem = source.name.trim_end_matches(".pdf").trim_end_matches(".PDF");
        let safe: String = stem.chars().map(|ch| if ch.is_alphanumeric() || " -_.,()".contains(ch) { ch } else { '_' }).collect();
        let trimmed = safe.trim_matches([' ', '.']);
        let stem = if trimmed.is_empty() { "document" } else { trimmed };
        let filename = format!("{stem}_unlocked.pdf");

        Ok((filename, unlocked_bytes))
    }

    /// Execute unlocking and save directly via native file dialog.
    pub fn execute_unlock_and_save(&mut self) {
        match self.generate_unlocked_pdf() {
            Ok((filename, bytes)) => {
                self.unlock_workspace.error = None;
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
                                app.unlock_workspace.success_note = Some(tl!("File unlocked and saved successfully!").to_string());
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
                self.unlock_workspace.error = Some(e);
            }
        }
    }

    /// Execute unlocking and open immediately in a viewer tab.
    pub fn execute_unlock_and_open(&mut self) {
        match self.generate_unlocked_pdf() {
            Ok((filename, bytes)) => {
                self.unlock_workspace.error = None;
                let _ = self.open_bytes(&filename, None, bytes);
                self.close_unlock_workspace();
            }
            Err(e) => {
                self.unlock_workspace.error = Some(e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_defaults() {
        let ws = UnlockPdfWorkspace::default();
        assert!(!ws.open);
        assert!(!ws.active);
        assert!(ws.source.is_none());
        assert!(ws.password.is_empty());
        assert!(!ws.show_password);
        assert!(ws.error.is_none());
        assert!(ws.success_note.is_none());
    }
}
