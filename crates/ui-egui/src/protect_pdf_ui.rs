//! Protect PDF workspace: Encrypt and add password security to PDF documents.

use std::sync::Arc;

use egui::{Align, Color32, CornerRadius, Layout, Stroke, vec2};
use karnoxpdf_engine::{Algorithm, Changes, Printing, Protection, inspect_protection, protect_pdf};

use crate::panels::human_size;
use crate::theme::{self, Tokens};
use crate::{KarnoxPDFApp, icons, protect};

/// Loaded PDF file source for protection.
#[derive(Clone, Debug)]
pub struct ProtectPdfSource {
    pub name: String,
    pub bytes: Arc<Vec<u8>>,
    pub path: Option<String>,
    pub page_count: usize,
    pub is_already_encrypted: bool,
}

/// Workspace state for Protect PDF.
#[derive(Clone, Debug)]
pub struct ProtectPdfWorkspace {
    pub open: bool,
    pub active: bool,
    pub source: Option<ProtectPdfSource>,

    /// Existing password if the source PDF is already encrypted.
    pub source_password: String,

    /// Whether to require a password to open/view the document.
    pub require_open_password: bool,
    pub open_password: String,
    pub confirm_open_password: String,
    pub show_open_password: bool,

    /// Whether to configure separate permissions password and restrictions.
    pub require_permissions_password: bool,
    pub permissions_password: String,
    pub confirm_permissions_password: String,
    pub show_permissions_password: bool,

    /// Encryption compatibility level.
    pub algorithm: Algorithm,

    /// Permission restrictions.
    pub printing: Printing,
    pub changes: Changes,
    pub allow_copy: bool,
    pub allow_accessibility: bool,
    pub encrypt_metadata: bool,

    /// Error message to display, if any.
    pub error: Option<String>,
}

impl Default for ProtectPdfWorkspace {
    fn default() -> Self {
        Self {
            open: false,
            active: false,
            source: None,
            source_password: String::new(),
            require_open_password: true,
            open_password: String::new(),
            confirm_open_password: String::new(),
            show_open_password: false,
            require_permissions_password: false,
            permissions_password: String::new(),
            confirm_permissions_password: String::new(),
            show_permissions_password: false,
            algorithm: Algorithm::Aes256,
            printing: Printing::High,
            changes: Changes::None,
            allow_copy: false,
            allow_accessibility: true,
            encrypt_metadata: true,
            error: None,
        }
    }
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

/// Render the Protect PDF workspace.
pub fn workspace(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens) {
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

    // Top Header Banner
    egui::Frame::NONE.fill(t.card).stroke(Stroke::new(1.0, t.border)).inner_margin(egui::Margin::symmetric(24, 14)).show(ui, |ui| {
        ui.horizontal(|ui| {
            draw_icon(ui, "shield-check", 22.0, t.accent);
            ui.add_space(10.0);
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(tl!("Protect PDF")).font(theme::semibold(17.0)).color(t.text));
                ui.label(
                    egui::RichText::new(tl!("Encrypt your PDF document with a password to prevent unauthorized access and modification."))
                        .font(theme::regular(12.0))
                        .color(t.text_muted),
                );
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if app.protect_workspace.source.is_some() && crate::widgets::dialog_action_button(ui, "x", tl!("Clear"), false).clicked() {
                    app.protect_workspace.source = None;
                    app.protect_workspace.open_password.clear();
                    app.protect_workspace.confirm_open_password.clear();
                    app.protect_workspace.permissions_password.clear();
                    app.protect_workspace.confirm_permissions_password.clear();
                    app.protect_workspace.error = None;
                }
            });
        });
    });

    ui.add_space(16.0);

    // Main workspace content
    if app.protect_workspace.source.is_none() {
        show_empty_upload(ui, app, t);
    } else {
        show_protect_form(ui, app, t);
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
                draw_icon(ui, "shield-check", 56.0, t.accent);
                ui.add_space(16.0);

                ui.label(egui::RichText::new(tl!("Select PDF to protect")).font(theme::semibold(19.0)).color(t.text));
                ui.add_space(6.0);

                ui.label(
                    egui::RichText::new(tl!("Add industry-standard AES-256 encryption and configure open and permissions passwords for your PDF."))
                        .font(theme::regular(13.0))
                        .color(t.text_muted),
                );

                ui.add_space(26.0);

                if crate::widgets::dialog_action_button(ui, "folder-open", tl!("Select PDF file"), true).clicked() {
                    app.protect_source_dialog();
                }

                ui.add_space(14.0);
                ui.label(
                    egui::RichText::new(tl!("Local and confidential: your file is encrypted entirely on your machine."))
                        .font(theme::regular(11.0))
                        .color(t.text_muted),
                );
            });
        });
    });
}

/// Protection form with password controls, algorithm selection, and permission chips.
fn show_protect_form(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens) {
    let avail_width = ui.available_width().max(600.0);
    let left_width = (avail_width * 0.58).clamp(380.0, 580.0);

    ui.horizontal_top(|ui| {
        // Left Column: Controls & Configuration
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
                    let (name, size_str, pages_str) = if let Some(src) = &app.protect_workspace.source {
                        let sz = human_size(src.bytes.len());
                        let pg = if src.page_count == 1 { tl!("1 page").to_string() } else { format!("{} {}", src.page_count, tl!("pages")) };
                        (src.name.clone(), sz, pg)
                    } else {
                        (String::new(), String::new(), String::new())
                    };

                    ui.horizontal(|ui| {
                        draw_icon(ui, "file-text", 26.0, t.accent);
                        ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(&name).font(theme::semibold(14.0)).color(t.text));
                            ui.label(egui::RichText::new(format!("{size_str} • {pages_str}")).font(theme::regular(12.0)).color(t.text_muted));
                        });

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if crate::widgets::dialog_action_button(ui, "folder-open", tl!("Change file"), false).clicked() {
                                app.protect_source_dialog();
                            }
                        });
                    });
                });

            ui.add_space(14.0);

            // Protection Type Toggles
            egui::Frame::NONE
                .fill(t.card)
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(CornerRadius::same(10))
                .inner_margin(egui::Margin::symmetric(18, 16))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(egui::RichText::new(tl!("Protection Modes")).font(theme::semibold(14.0)).color(t.text));
                    ui.add_space(10.0);

                    ui.horizontal_wrapped(|ui| {
                        let req_open = app.protect_workspace.require_open_password;
                        if crate::widgets::option_button(ui, Some("eye"), tl!("Require password to open"), req_open, 0.0).clicked() {
                            app.protect_workspace.require_open_password = !req_open;
                            if !app.protect_workspace.require_open_password && !app.protect_workspace.require_permissions_password {
                                app.protect_workspace.require_permissions_password = true;
                            }
                        }

                        ui.add_space(8.0);

                        let req_perms = app.protect_workspace.require_permissions_password;
                        if crate::widgets::option_button(ui, Some("settings"), tl!("Restrict editing & permissions"), req_perms, 0.0).clicked() {
                            app.protect_workspace.require_permissions_password = !req_perms;
                            if !app.protect_workspace.require_open_password && !app.protect_workspace.require_permissions_password {
                                app.protect_workspace.require_open_password = true;
                            }
                        }
                    });

                    // Open Password Fields
                    if app.protect_workspace.require_open_password {
                        ui.add_space(14.0);
                        ui.separator();
                        ui.add_space(10.0);

                        ui.label(egui::RichText::new(tl!("Password to open document")).font(theme::semibold(13.0)).color(t.text));
                        ui.add_space(4.0);

                        let show_open_pw = app.protect_workspace.show_open_password;
                        bordered_password_input(
                            ui,
                            t,
                            &mut app.protect_workspace.open_password,
                            tl!("Enter password"),
                            show_open_pw,
                            Some(&mut app.protect_workspace.show_open_password),
                        );

                        // Password strength meter
                        if !app.protect_workspace.open_password.is_empty() {
                            ui.add_space(6.0);
                            let (strength_text, color) = match protect::strength(&app.protect_workspace.open_password) {
                                ("Strong", color) => (tl!("Strong"), color),
                                ("Medium", color) => (tl!("Medium"), color),
                                (_, color) => (tl!("Weak"), color),
                            };
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(tl!("Strength:")).font(theme::regular(11.5)).color(t.text_muted));
                                ui.add_space(4.0);
                                ui.label(egui::RichText::new(strength_text).font(theme::semibold(11.5)).color(color));
                            });
                        }

                        ui.add_space(10.0);
                        ui.label(egui::RichText::new(tl!("Confirm password")).font(theme::semibold(13.0)).color(t.text));
                        ui.add_space(4.0);

                        let show_open_pw = app.protect_workspace.show_open_password;
                        bordered_password_input(ui, t, &mut app.protect_workspace.confirm_open_password, tl!("Re-type password"), show_open_pw, None);

                        if !app.protect_workspace.confirm_open_password.is_empty() {
                            ui.add_space(4.0);
                            if app.protect_workspace.open_password == app.protect_workspace.confirm_open_password {
                                ui.label(
                                    egui::RichText::new(format!("✔ {}", tl!("Passwords match")))
                                        .font(theme::medium(11.0))
                                        .color(Color32::from_rgb(0x2E, 0x85, 0x55)),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new(format!("✖ {}", tl!("Passwords do not match")))
                                        .font(theme::medium(11.0))
                                        .color(Color32::from_rgb(0xD9, 0x53, 0x4F)),
                                );
                            }
                        }
                    }

                    // Permissions Password Fields
                    if app.protect_workspace.require_permissions_password {
                        ui.add_space(14.0);
                        ui.separator();
                        ui.add_space(10.0);

                        ui.label(egui::RichText::new(tl!("Permissions Password (Owner)")).font(theme::semibold(13.0)).color(t.text));
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(tl!("Required to change security or lift restrictions:"))
                                .font(theme::regular(11.5))
                                .color(t.text_muted),
                        );
                        ui.add_space(6.0);

                        let show_perms_pw = app.protect_workspace.show_permissions_password;
                        bordered_password_input(
                            ui,
                            t,
                            &mut app.protect_workspace.permissions_password,
                            tl!("Enter permissions password"),
                            show_perms_pw,
                            Some(&mut app.protect_workspace.show_permissions_password),
                        );

                        if !app.protect_workspace.permissions_password.is_empty() {
                            ui.add_space(6.0);
                            let (strength_text, color) = match protect::strength(&app.protect_workspace.permissions_password) {
                                ("Strong", color) => (tl!("Strong"), color),
                                ("Medium", color) => (tl!("Medium"), color),
                                (_, color) => (tl!("Weak"), color),
                            };
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(tl!("Strength:")).font(theme::regular(11.5)).color(t.text_muted));
                                ui.add_space(4.0);
                                ui.label(egui::RichText::new(strength_text).font(theme::semibold(11.5)).color(color));
                            });
                        }

                        ui.add_space(10.0);
                        ui.label(egui::RichText::new(tl!("Confirm permissions password")).font(theme::semibold(13.0)).color(t.text));
                        ui.add_space(4.0);

                        let show_perms_pw = app.protect_workspace.show_permissions_password;
                        bordered_password_input(
                            ui,
                            t,
                            &mut app.protect_workspace.confirm_permissions_password,
                            tl!("Re-type permissions password"),
                            show_perms_pw,
                            None,
                        );

                        if !app.protect_workspace.confirm_permissions_password.is_empty() {
                            ui.add_space(4.0);
                            if app.protect_workspace.permissions_password == app.protect_workspace.confirm_permissions_password {
                                ui.label(
                                    egui::RichText::new(format!("✔ {}", tl!("Passwords match")))
                                        .font(theme::medium(11.0))
                                        .color(Color32::from_rgb(0x2E, 0x85, 0x55)),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new(format!("✖ {}", tl!("Passwords do not match")))
                                        .font(theme::medium(11.0))
                                        .color(Color32::from_rgb(0xD9, 0x53, 0x4F)),
                                );
                            }
                        }
                    }
                });

            ui.add_space(14.0);

            // Encryption Algorithm Selection
            egui::Frame::NONE
                .fill(t.card)
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(CornerRadius::same(10))
                .inner_margin(egui::Margin::symmetric(18, 14))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(egui::RichText::new(tl!("Encryption Algorithm")).font(theme::semibold(14.0)).color(t.text));
                    ui.add_space(8.0);

                    ui.horizontal_wrapped(|ui| {
                        let aes256_text = tl!("AES-256 (Recommended)");
                        if crate::widgets::option_button(
                            ui,
                            Some("shield-check"),
                            aes256_text,
                            app.protect_workspace.algorithm == Algorithm::Aes256,
                            0.0,
                        )
                        .clicked()
                        {
                            app.protect_workspace.algorithm = Algorithm::Aes256;
                        }
                        ui.add_space(6.0);
                        let aes128_text = tl!("AES-128 (Acrobat 7+)");
                        if crate::widgets::option_button(ui, Some("lock"), aes128_text, app.protect_workspace.algorithm == Algorithm::Aes128, 0.0)
                            .clicked()
                        {
                            app.protect_workspace.algorithm = Algorithm::Aes128;
                        }
                        ui.add_space(6.0);
                        let rc4_text = tl!("RC4-128 (Legacy)");
                        if crate::widgets::option_button(ui, Some("clock"), rc4_text, app.protect_workspace.algorithm == Algorithm::Rc4_128, 0.0)
                            .clicked()
                        {
                            app.protect_workspace.algorithm = Algorithm::Rc4_128;
                        }
                    });
                });

            // Permission Specific Rules (when permissions mode is checked)
            if app.protect_workspace.require_permissions_password {
                ui.add_space(14.0);
                egui::Frame::NONE
                    .fill(t.card)
                    .stroke(Stroke::new(1.0, t.border))
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(egui::Margin::symmetric(18, 14))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(egui::RichText::new(tl!("Permissions & Restrictions")).font(theme::semibold(14.0)).color(t.text));
                        ui.add_space(10.0);

                        // Printing options
                        ui.label(egui::RichText::new(tl!("Printing Allowed")).font(theme::medium(12.0)).color(t.text));
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            if crate::widgets::option_button(ui, None, tl!("High Resolution"), app.protect_workspace.printing == Printing::High, 0.0)
                                .clicked()
                            {
                                app.protect_workspace.printing = Printing::High;
                            }
                            ui.add_space(4.0);
                            if crate::widgets::option_button(ui, None, tl!("Low Resolution"), app.protect_workspace.printing == Printing::Low, 0.0)
                                .clicked()
                            {
                                app.protect_workspace.printing = Printing::Low;
                            }
                            ui.add_space(4.0);
                            if crate::widgets::option_button(ui, None, tl!("None (Blocked)"), app.protect_workspace.printing == Printing::None, 0.0)
                                .clicked()
                            {
                                app.protect_workspace.printing = Printing::None;
                            }
                        });

                        ui.add_space(10.0);

                        // Modifications options
                        ui.label(egui::RichText::new(tl!("Changes Allowed")).font(theme::medium(12.0)).color(t.text));
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            if crate::widgets::option_button(ui, None, tl!("None"), app.protect_workspace.changes == Changes::None, 0.0).clicked() {
                                app.protect_workspace.changes = Changes::None;
                            }
                            ui.add_space(4.0);
                            if crate::widgets::option_button(ui, None, tl!("Page Operations"), app.protect_workspace.changes == Changes::Pages, 0.0)
                                .clicked()
                            {
                                app.protect_workspace.changes = Changes::Pages;
                            }
                            ui.add_space(4.0);
                            if crate::widgets::option_button(ui, None, tl!("Fill & Sign"), app.protect_workspace.changes == Changes::FillSign, 0.0)
                                .clicked()
                            {
                                app.protect_workspace.changes = Changes::FillSign;
                            }
                            ui.add_space(4.0);
                            if crate::widgets::option_button(
                                ui,
                                None,
                                tl!("Commenting & Forms"),
                                app.protect_workspace.changes == Changes::CommentFillSign,
                                0.0,
                            )
                            .clicked()
                            {
                                app.protect_workspace.changes = Changes::CommentFillSign;
                            }
                            ui.add_space(4.0);
                            if crate::widgets::option_button(ui, None, tl!("All"), app.protect_workspace.changes == Changes::AnyExceptExtract, 0.0)
                                .clicked()
                            {
                                app.protect_workspace.changes = Changes::AnyExceptExtract;
                            }
                        });

                        ui.add_space(10.0);

                        // Copying and screen readers
                        ui.label(egui::RichText::new(tl!("Content Access")).font(theme::medium(12.0)).color(t.text));
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            crate::widgets::check_row(ui, &mut app.protect_workspace.allow_copy, tl!("Enable text & image copying"));
                            ui.add_space(10.0);
                            crate::widgets::check_row(ui, &mut app.protect_workspace.allow_accessibility, tl!("Enable screen readers"));
                        });
                    });
            }

            ui.add_space(14.0);

            // Error Display
            if let Some(err) = &app.protect_workspace.error {
                egui::Frame::NONE
                    .fill(Color32::from_rgb(0xF8, 0xD7, 0xDA))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(0xF5, 0xC6, 0xC8)))
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

            // Primary & Secondary Action Buttons
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if crate::widgets::dialog_action_button(ui, "lock", tl!("Protect and save PDF"), true).clicked() {
                    app.execute_protect_and_save();
                }

                ui.add_space(10.0);

                if crate::widgets::dialog_action_button(ui, "book-open", tl!("Open in KarnoxPDF"), false).clicked() {
                    app.execute_protect_and_open();
                }
            });
        });

        ui.add_space(20.0);

        // Right Column: Live Security Card & Visual Preview
        ui.vertical(|ui| {
            egui::Frame::NONE
                .fill(t.card)
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(CornerRadius::same(10))
                .inner_margin(egui::Margin::symmetric(24, 22))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(tl!("Protection Overview")).font(theme::semibold(15.0)).color(t.text));
                    ui.add_space(14.0);

                    // Shield emblem
                    ui.vertical_centered(|ui| {
                        draw_icon(ui, "shield-check", 48.0, t.accent);
                        ui.add_space(8.0);
                        let alg_txt = match app.protect_workspace.algorithm {
                            Algorithm::Aes256 => "AES-256",
                            Algorithm::Aes128 => "AES-128",
                            Algorithm::Rc4_128 => "RC4-128",
                            _ => "AES",
                        };
                        ui.label(egui::RichText::new(format!("{} {}", tl!("Encrypted with"), alg_txt)).font(theme::semibold(14.0)).color(t.text));
                    });

                    ui.add_space(16.0);
                    ui.separator();
                    ui.add_space(12.0);

                    let open_status = if app.protect_workspace.require_open_password && !app.protect_workspace.open_password.is_empty() {
                        tl!("Password configured")
                    } else if app.protect_workspace.require_open_password {
                        tl!("Required (awaiting entry)")
                    } else {
                        tl!("Not required (public reading)")
                    };

                    let perms_status = if app.protect_workspace.require_permissions_password && !app.protect_workspace.permissions_password.is_empty()
                    {
                        tl!("Restrictions active")
                    } else if app.protect_workspace.require_permissions_password {
                        tl!("Awaiting password entry")
                    } else {
                        tl!("No restrictions")
                    };

                    let printing_status = match app.protect_workspace.printing {
                        Printing::High => tl!("High Resolution"),
                        Printing::Low => tl!("Low Resolution"),
                        Printing::None => tl!("Blocked"),
                    };

                    let copy_status = if app.protect_workspace.allow_copy { tl!("Allowed") } else { tl!("Blocked") };

                    let items = [
                        (tl!("Open Password:"), open_status),
                        (tl!("Permissions:"), perms_status),
                        (tl!("Printing:"), printing_status),
                        (tl!("Text/Image Copying:"), copy_status),
                    ];

                    for (label, val) in items {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(label).font(theme::medium(12.0)).color(t.text_muted));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(egui::RichText::new(val).font(theme::semibold(12.0)).color(t.text));
                            });
                        });
                        ui.add_space(8.0);
                    }
                });
        });
    });
}

impl KarnoxPDFApp {
    /// Open the Protect PDF workspace.
    pub fn open_protect_workspace(&mut self) {
        self.deactivate_all_workspaces();
        self.active = None;
        self.protect_workspace.open = true;
        self.protect_workspace.active = true;
        self.protect_workspace.error = None;
    }

    /// Close the Protect PDF workspace.
    pub fn close_protect_workspace(&mut self) {
        self.protect_workspace.active = false;
        self.protect_workspace.open = false;
        self.protect_workspace.source = None;
        self.protect_workspace.open_password.clear();
        self.protect_workspace.confirm_open_password.clear();
        self.protect_workspace.permissions_password.clear();
        self.protect_workspace.confirm_permissions_password.clear();
        self.protect_workspace.error = None;
    }

    /// Stage a picked PDF file for the Protect PDF workspace.
    pub fn stage_protect_source(&mut self, name: String, bytes: Vec<u8>, path: Option<String>) {
        let is_already_encrypted = inspect_protection(&bytes).is_ok_and(|s| s != karnoxpdf_engine::ProtectionStatus::NotProtected);
        let bytes_arc = Arc::new(bytes);
        let page_count = self.session.page_count_of(&name, &bytes_arc).unwrap_or(1);

        self.protect_workspace.source = Some(ProtectPdfSource { name, bytes: bytes_arc, path, page_count, is_already_encrypted });
        self.protect_workspace.source_password.clear();
        self.protect_workspace.open_password.clear();
        self.protect_workspace.confirm_open_password.clear();
        self.protect_workspace.permissions_password.clear();
        self.protect_workspace.confirm_permissions_password.clear();
        self.protect_workspace.error = None;
        self.open_protect_workspace();
    }

    /// Build the Protection configuration and encrypt the document, returning (filename, bytes).
    pub fn generate_protected_pdf(&mut self) -> Result<(String, Vec<u8>), String> {
        let Some(source) = &self.protect_workspace.source else {
            return Err(tl!("No document loaded.").to_string());
        };

        // Validate passwords
        if self.protect_workspace.require_open_password {
            if self.protect_workspace.open_password.is_empty() {
                return Err(tl!("Type a password to open the document.").to_string());
            }
            if self.protect_workspace.open_password != self.protect_workspace.confirm_open_password {
                return Err(tl!("The open passwords do not match.").to_string());
            }
        }

        if self.protect_workspace.require_permissions_password {
            if self.protect_workspace.permissions_password.is_empty() {
                return Err(tl!("Type a permissions password.").to_string());
            }
            if self.protect_workspace.permissions_password != self.protect_workspace.confirm_permissions_password {
                return Err(tl!("The permissions passwords do not match.").to_string());
            }
            if self.protect_workspace.require_open_password && self.protect_workspace.open_password == self.protect_workspace.permissions_password {
                return Err(tl!("The open password and permissions password must be different.").to_string());
            }
        }

        if !self.protect_workspace.require_open_password && !self.protect_workspace.require_permissions_password {
            return Err(tl!("Please enable an open password or permissions password.").to_string());
        }

        let open_pw = if self.protect_workspace.require_open_password { Some(self.protect_workspace.open_password.clone()) } else { None };

        let perms_pw =
            if self.protect_workspace.require_permissions_password { Some(self.protect_workspace.permissions_password.clone()) } else { None };

        let protection = Protection {
            open_password: open_pw,
            permissions_password: perms_pw,
            printing: if self.protect_workspace.require_permissions_password { self.protect_workspace.printing } else { Printing::High },
            changes: if self.protect_workspace.require_permissions_password { self.protect_workspace.changes } else { Changes::AnyExceptExtract },
            copy: if self.protect_workspace.require_permissions_password { self.protect_workspace.allow_copy } else { true },
            accessibility: self.protect_workspace.allow_accessibility,
            algorithm: self.protect_workspace.algorithm,
            encrypt_metadata: self.protect_workspace.encrypt_metadata,
        };

        let src_pw = if self.protect_workspace.source_password.is_empty() { None } else { Some(self.protect_workspace.source_password.as_str()) };

        let protected_bytes = protect_pdf(&source.bytes, src_pw, &protection).map_err(|e| e.to_string())?;

        let stem = source.name.trim_end_matches(".pdf").trim_end_matches(".PDF");
        let safe: String = stem.chars().map(|ch| if ch.is_alphanumeric() || " -_.,()".contains(ch) { ch } else { '_' }).collect();
        let trimmed = safe.trim_matches([' ', '.']);
        let stem = if trimmed.is_empty() { "document" } else { trimmed };
        let filename = format!("{stem}_protected.pdf");

        Ok((filename, protected_bytes))
    }

    /// Execute protection and save directly via native file dialog.
    pub fn execute_protect_and_save(&mut self) {
        match self.generate_protected_pdf() {
            Ok((filename, bytes)) => {
                self.protect_workspace.error = None;
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
                                app.protect_workspace.source = None;
                                app.close_protect_workspace();
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
                self.protect_workspace.error = Some(e);
            }
        }
    }

    /// Execute protection and open immediately in a viewer tab.
    pub fn execute_protect_and_open(&mut self) {
        let open_pw = if self.protect_workspace.require_open_password { Some(self.protect_workspace.open_password.clone()) } else { None };

        match self.generate_protected_pdf() {
            Ok((filename, bytes)) => {
                self.protect_workspace.error = None;
                let _ = self.open_bytes(&filename, open_pw, bytes);
                self.close_protect_workspace();
            }
            Err(e) => {
                self.protect_workspace.error = Some(e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_defaults() {
        let ws = ProtectPdfWorkspace::default();
        assert!(!ws.open);
        assert!(!ws.active);
        assert!(ws.source.is_none());
        assert!(ws.require_open_password);
        assert!(!ws.require_permissions_password);
        assert_eq!(ws.algorithm, Algorithm::Aes256);
        assert_eq!(ws.printing, Printing::High);
        assert_eq!(ws.changes, Changes::None);
        assert!(!ws.allow_copy);
        assert!(ws.allow_accessibility);
        assert!(ws.error.is_none());
    }
}
