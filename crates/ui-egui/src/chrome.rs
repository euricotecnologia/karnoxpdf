//! Window chrome: tab strip (with the integrated macOS title bar), mode bar, right rail.

use egui::{Align, Align2, Color32, CornerRadius, Layout, Rect, Response, Sense, Stroke, vec2};

use crate::canvas::{DocView, Fit, PageLayout};
use crate::theme::{self, ThemePreference, Tokens};
use crate::{Dialog, KarnoxPDFApp, QuickTool, RightPanel, i18n, icons, panels, widgets};

pub fn toolbar(app: &mut KarnoxPDFApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let left = if cfg!(target_os = "macos") && app.integrated_titlebar { 80 } else { 8 };
    egui::Panel::top("primary_toolbar")
        .exact_size(48.0)
        .frame(
            egui::Frame::NONE.fill(t.titlebar).inner_margin(egui::Margin { left, right: 10, top: 0, bottom: 0 }).stroke(Stroke::new(1.0, t.divider)),
        )
        .show(ui, |ui| {
            let full = ui.max_rect();
            let drag = ui.interact(full, ui.id().with("titledrag"), Sense::click_and_drag());
            if drag.drag_started() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            if drag.double_clicked() {
                let max = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
            }
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;

                // Main toolbar action buttons
                if widgets::ghost_button(ui, "plus", tl!("Open")).on_hover_text(tl!("Open a PDF")).clicked() {
                    app.open_dialog();
                }

                let has_doc = app.active.is_some();
                if app.mode != crate::Mode::Edit && app.dialog.is_none() && app.close_request.is_none() {
                    ui.add_enabled_ui(has_doc, |ui| {
                        if widgets::ghost_button(ui, "save", tl!("Save")).on_hover_text(tl!("Save")).clicked() {
                            app.run_command("file.save");
                        }
                    });
                }
                ui.add_enabled_ui(has_doc, |ui| {
                    if widgets::ghost_button(ui, "printer", tl!("Print")).on_hover_text(tl!("Print")).clicked() {
                        app.run_command("print.dialog");
                    }
                });

                let can_revert = has_doc && karnoxpdf_engine::commands::command("file.revert").is_some_and(|s| app.command_enabled(s));
                ui.add_enabled_ui(can_revert, |ui| {
                    if widgets::ghost_button(ui, "rotate-ccw", tl!("Revert")).on_hover_text(tl!("Revert")).clicked() {
                        app.run_command("file.revert");
                    }
                });

                ui.add_enabled_ui(has_doc, |ui| {
                    if widgets::ghost_button(ui, "x", tl!("Close file")).on_hover_text(tl!("Close file")).clicked() {
                        app.run_command("file.close");
                    }
                });

                let has_docs = !app.views.is_empty();
                ui.add_enabled_ui(has_docs, |ui| {
                    if widgets::ghost_button(ui, "files", tl!("Close all")).on_hover_text(tl!("Close all")).clicked() {
                        app.run_command("file.close_all");
                    }
                });

                if widgets::ghost_button(ui, "info", tl!("About")).on_hover_text(tl!("About KarnoxPDF")).clicked() {
                    app.dialog = Some(Dialog::About);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let theme = widgets::ghost_button(ui, "palette", tl!("System themes"));
                    egui::Popup::menu(&theme).show(|ui| theme_menu(app, ui));
                    let language = widgets::ghost_button(ui, "languages", tl!("Language"));
                    egui::Popup::menu(&language).show(|ui| language_menu(app, ui));
                    let (notif_rect, notif_resp) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::click());
                    notif_resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tl!("Notification")));
                    let has_res = app.has_open_resources();
                    let nbtn_bg = if has_res {
                        if notif_resp.hovered() { Color32::from_rgb(0x15, 0x54, 0xC8) } else { t.accent }
                    } else if notif_resp.hovered() {
                        t.hover
                    } else {
                        Color32::TRANSPARENT
                    };
                    let stroke = if has_res { Stroke::NONE } else { Stroke::new(1.0, t.border) };
                    ui.painter().rect(notif_rect, CornerRadius::same(8), nbtn_bg, stroke, egui::StrokeKind::Inside);
                    let icon_color = if has_res { Color32::WHITE } else { t.text };
                    icons::paint(ui, Rect::from_center_size(notif_rect.center(), vec2(16.0, 16.0)), "bell", 16.0, icon_color);
                    if has_res {
                        ui.painter().circle_filled(notif_rect.right_top() + vec2(-4.0, 4.0), 3.5, Color32::from_rgb(0xFF, 0x3B, 0x30));
                    }
                    let tip = if has_res {
                        format!("{} ({})", tl!("Go to open files or active tools"), app.open_resource_count())
                    } else {
                        tl!("No open files or active tools").to_string()
                    };
                    if notif_resp.on_hover_text(tip).clicked() {
                        app.navigate_to_active_resource();
                    }
                });
            });
        });
}

/// The document editor keeps its tools above the page, leaving the full width for editing.
/// The command catalogue remains the source for the overflow menus and their availability.
pub fn editor_toolbar(app: &mut KarnoxPDFApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::top("editor_toolbar")
        .exact_size(88.0)
        .frame(egui::Frame::NONE.fill(t.panel).inner_margin(egui::Margin::symmetric(16, 7)).stroke(Stroke::new(1.0, t.divider)))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(tl!("Edit a PDF")).font(theme::semibold(15.0)).color(t.text));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let can_save = app.dialog.is_none() && app.close_request.is_none();
                    ui.add_enabled_ui(can_save, |ui| {
                        if widgets::dialog_action_button(ui, "save", tl!("Save"), true).clicked() {
                            app.run_command("file.save");
                        }
                        if widgets::dialog_action_button(ui, "save", tl!("Save as…"), false).clicked() {
                            app.run_command("file.save_as");
                        }
                    });
                });
            });
            ui.add_space(4.0);
            egui::ScrollArea::horizontal().id_salt("editor_actions").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 5.0;
                    let undo = karnoxpdf_engine::commands::command("edit.undo").is_some_and(|spec| app.command_enabled(spec));
                    ui.add_enabled_ui(undo, |ui| {
                        if editor_action(ui, &t, "undo-2", tl!("Undo"), false).clicked() {
                            app.run_command("edit.undo");
                        }
                    });
                    let redo = karnoxpdf_engine::commands::command("edit.redo").is_some_and(|spec| app.command_enabled(spec));
                    ui.add_enabled_ui(redo, |ui| {
                        if editor_action(ui, &t, "redo-2", tl!("Redo"), false).clicked() {
                            app.run_command("edit.redo");
                        }
                    });
                    ui.separator();
                    if editor_action(ui, &t, "text-cursor-input", tl!("Edit text & images"), app.quick_tool == QuickTool::EditText).clicked() {
                        app.run_command("edit.edit_text");
                    }
                    if editor_action(ui, &t, "type", tl!("Text"), app.quick_tool == QuickTool::AddText).clicked() {
                        app.run_command("edit.text");
                    }
                    if editor_action(ui, &t, "image-plus", tl!("Image"), false).clicked() {
                        app.run_command("edit.image");
                    }
                    if editor_action(ui, &t, "link-2", tl!("Link"), app.quick_tool == QuickTool::Link).clicked() {
                        app.run_command("edit.link");
                    }
                    ui.separator();
                    let format_label = editor_format_label(app);
                    let format_icon = match format_label {
                        "Edit image" => "image",
                        "Format shape" => "square",
                        _ => "text-select",
                    };
                    let format = editor_action(ui, &t, format_icon, tl!(format_label), false);
                    egui::Popup::menu(&format).show(|ui| {
                        ui.set_min_width(270.0);
                        panels::format_section(app, ui, &t);
                    });
                    let more = editor_action(ui, &t, "plus", tl!("Add content"), false);
                    egui::Popup::menu(&more).show(|ui| editor_command_menu(app, ui, "edit", 1));
                    let pages = editor_action(ui, &t, "files", tl!("Modify page"), false);
                    egui::Popup::menu(&pages).show(|ui| editor_command_menu(app, ui, "edit", 0));
                });
            });
        });
}

fn editor_format_label(app: &KarnoxPDFApp) -> &'static str {
    let Some((i, id)) = app.active_ids() else { return "Format text" };
    let Some(view) = app.views.get(i) else { return "Format text" };
    if view.shape_selection.is_some() {
        return "Format shape";
    }
    if view.image_selection.is_some() {
        return "Edit image";
    }
    if let Some((page, index)) = view.content.selected
        && let Some(doc) = app.session.get(id)
        && doc
            .added
            .iter()
            .filter(|item| item.page == page)
            .nth(index)
            .is_some_and(|item| matches!(item.content, karnoxpdf_engine::AddedContent::Image(_)))
    {
        return "Edit image";
    }
    "Format text"
}

fn editor_action(ui: &mut egui::Ui, t: &Tokens, icon: &str, label: &str, selected: bool) -> Response {
    let enabled = ui.is_enabled();
    let font = theme::semibold(12.5);
    let width = ui.fonts_mut(|f| f.layout_no_wrap(label.to_owned(), font.clone(), Color32::WHITE).size().x);
    let (rect, resp) = ui.allocate_exact_size(vec2(width + 36.0, 32.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, selected, label));
    let (fill, stroke, color) = if !enabled {
        (t.accent.gamma_multiply(0.35), Stroke::NONE, Color32::WHITE.gamma_multiply(0.65))
    } else if selected {
        (Color32::from_rgb(0x0D, 0x47, 0xA1), Stroke::new(1.5, Color32::from_white_alpha(180)), Color32::WHITE)
    } else if resp.hovered() {
        (Color32::from_rgb(0x15, 0x54, 0xC8), Stroke::NONE, Color32::WHITE)
    } else {
        (t.accent, Stroke::NONE, Color32::WHITE)
    };
    ui.painter().rect(rect, CornerRadius::same(6), fill, stroke, egui::StrokeKind::Inside);
    icons::paint(ui, Rect::from_min_size(rect.min + vec2(8.0, 8.0), vec2(16.0, 16.0)), icon, 15.0, color);
    ui.painter().text(rect.left_center() + vec2(30.0, 0.0), Align2::LEFT_CENTER, label, font, color);
    resp
}

fn editor_command_menu(app: &mut KarnoxPDFApp, ui: &mut egui::Ui, group: &str, section: usize) {
    let Some(items) = karnoxpdf_engine::catalog::group(group).and_then(|g| g.sections.get(section)).map(|s| s.items) else { return };
    ui.set_min_width(220.0);
    for item in items.iter().filter(|item| {
        item.availability == karnoxpdf_engine::catalog::Availability::Ready
            && !matches!(item.command, "edit.edit_text" | "edit.text" | "edit.image" | "edit.link")
    }) {
        let enabled = karnoxpdf_engine::commands::command(item.command).is_some_and(|spec| app.command_enabled(spec));
        let clicked = ui.add_enabled_ui(enabled, |ui| widgets::ghost_button(ui, item.icon, tl!(item.label))).inner.clicked();
        if clicked {
            app.run_command(item.command);
            ui.close();
        }
    }
}

/// Both theme entry points use the same choices and command path.
pub(crate) fn theme_menu(app: &mut KarnoxPDFApp, ui: &mut egui::Ui) {
    for (preference, command, label) in [
        (ThemePreference::System, "view.theme.system", "Use system setting"),
        (ThemePreference::Light, "view.theme.light", "Light gray"),
        (ThemePreference::Dark, "view.theme.dark", "Dark gray"),
    ] {
        if ui.radio(app.theme_preference == preference, tl!(label)).clicked() {
            app.execute(command);
            ui.close();
        }
    }
}

pub(crate) fn language_menu(app: &mut KarnoxPDFApp, ui: &mut egui::Ui) {
    ui.set_min_width(190.0);
    let selected = i18n::Lang::from_pref(&app.language).code();
    for code in ["pt-br", "pt-pt", "en", "es", "fr"] {
        let Some(language) = i18n::Lang::from_code(code) else { continue };
        if ui.radio(selected == code, language.name()).clicked() {
            app.language = code.to_string();
            i18n::set_current(language);
            ui.close();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn tab(
    ui: &mut egui::Ui,
    t: &Tokens,
    name: &str,
    dirty: bool,
    active: bool,
    close: &mut Option<usize>,
    index: usize,
    icon: &str,
    closable: bool,
) -> egui::Response {
    let font = theme::regular(13.0);
    let label: String = if name.chars().count() > 28 { format!("{}…", name.chars().take(27).collect::<String>()) } else { name.to_string() };
    let text_w = ui.fonts_mut(|f| f.layout_no_wrap(label.clone(), font.clone(), t.text).size().x);
    let (rect, _) = ui.allocate_exact_size(vec2(text_w + if closable { 64.0 } else { 42.0 }, 30.0), Sense::hover());
    let resp = ui.interact(rect, ui.id().with(("doctab", index)), Sense::click());
    let a11y = if dirty { format!("{name} (edited)") } else { name.to_string() };
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, active, &a11y));
    let bg = if active {
        t.chrome
    } else if resp.hovered() {
        t.hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius { nw: 7, ne: 7, sw: 0, se: 0 }, bg);
    if active {
        ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(2.0, t.accent));
    }
    icons::paint(ui, Rect::from_min_size(rect.min + vec2(6.0, 7.0), vec2(16.0, 16.0)), icon, 15.0, if active { t.accent } else { t.text_muted });
    ui.painter().text(rect.min + vec2(28.0, rect.height() / 2.0), Align2::LEFT_CENTER, label, font, if active { t.text } else { t.text_muted });
    if closable {
        let x_rect = Rect::from_center_size(rect.right_center() - vec2(16.0, 0.0), vec2(20.0, 20.0));
        let x = ui.interact(x_rect, ui.id().with(("tabclose", index)), Sense::click());
        if x.hovered() {
            ui.painter().rect_filled(x_rect, CornerRadius::same(4), t.pressed);
        }
        // Unsaved changes: a dot where the close button sits, until the tab is hovered.
        if dirty && !resp.hovered() && !x.hovered() {
            ui.painter().circle_filled(x_rect.center(), 4.0, if active { t.text } else { t.text_muted });
        } else if active || resp.hovered() || x.hovered() {
            icons::paint(ui, x_rect, "x", 13.0, t.text_muted);
        }
        if x.clicked() {
            *close = Some(index);
        }
    }
    resp.on_hover_text(if dirty { crate::i18n::fmt(tl!("{name} — unsaved changes"), &[("name", name)]) } else { name.to_string() })
}

pub fn tab_strip(app: &mut KarnoxPDFApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::top("document_tabs")
        .exact_size(42.0)
        .frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin::symmetric(10, 0)).stroke(Stroke::new(1.0, t.divider)))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let mut close = None;
                if tab(
                    ui,
                    &t,
                    tl!("All tools"),
                    false,
                    app.active.is_none()
                        && !app.combine_workspace_active
                        && !app.split_workspace_active
                        && !app.compress_workspace_active
                        && !app.word_workspace_active
                        && !app.jpg_workspace.active
                        && !app.image_to_pdf.active
                        && !app.sign_workspace.active
                        && !app.watermark_workspace.active
                        && !app.rotate_workspace.active
                        && !app.html_workspace.active
                        && !app.unlock_workspace.active
                        && !app.protect_workspace.active
                        && !app.repair_workspace.active
                        && !app.page_numbers_workspace.active
                        && !app.delete_pages_workspace.active
                        && !app.translate_workspace.active
                        && !app.office_workspaces.iter().any(|state| state.active),
                    &mut close,
                    0,
                    "layout-grid",
                    false,
                )
                .clicked()
                {
                    app.active = None;
                    app.deactivate_all_workspaces();
                }
                for i in 0..app.views.len() {
                    let Some(doc) = app.session.get(app.views[i].id) else { continue };
                    let (name, dirty) = (doc.display_name(), doc.dirty);
                    let view_mode = app.views[i].mode;
                    let same_name_count = app.views.iter().filter(|v| app.session.get(v.id).is_some_and(|d| d.display_name() == name)).count();
                    let same_name_same_mode_indices: Vec<usize> = app
                        .views
                        .iter()
                        .enumerate()
                        .filter(|(_, v)| v.mode == view_mode && app.session.get(v.id).is_some_and(|d| d.display_name() == name))
                        .map(|(idx, _)| idx)
                        .collect();
                    let num_suffix = if same_name_same_mode_indices.len() > 1 {
                        let pos = same_name_same_mode_indices.iter().position(|&idx| idx == i).unwrap_or(0) + 1;
                        format!(" {pos}")
                    } else {
                        String::new()
                    };
                    let (tab_title, tab_icon) = match view_mode {
                        crate::Mode::Edit => (format!("{} ({}{num_suffix})", name, tl!("Edit PDF")), "pencil"),
                        crate::Mode::Sign => (format!("{} ({}{num_suffix})", name, tl!("E-Sign")), "pen-tool"),
                        crate::Mode::Convert => (format!("{} ({}{num_suffix})", name, tl!("Convert")), "file-output"),
                        _ => {
                            if same_name_count > 1 {
                                (format!("{} ({}{num_suffix})", name, tl!("Read")), "file-text")
                            } else {
                                (name.clone(), "file-text")
                            }
                        }
                    };
                    if tab(ui, &t, &tab_title, dirty, app.active == Some(i), &mut close, i + 1, tab_icon, true).clicked() {
                        app.switch_to_tab(i);
                    }
                }
                let workspace_index = app.views.len() + 1;
                if app.combine_workspace_open
                    && tab(
                        ui,
                        &t,
                        tl!("Merge PDF"),
                        !app.combine_draft.is_empty(),
                        app.combine_workspace_active && app.active.is_none(),
                        &mut close,
                        workspace_index,
                        "files",
                        true,
                    )
                    .clicked()
                {
                    app.active = None;
                    app.combine_workspace_active = true;
                    app.split_workspace_active = false;
                    app.compress_workspace_active = false;
                    app.word_workspace_active = false;
                    app.deactivate_office();
                }
                let split_index = workspace_index + 1;
                if app.split_workspace_open
                    && tab(
                        ui,
                        &t,
                        tl!("Split PDF"),
                        false,
                        app.split_workspace_active && app.active.is_none(),
                        &mut close,
                        split_index,
                        "scissors",
                        true,
                    )
                    .clicked()
                {
                    app.active = None;
                    app.combine_workspace_active = false;
                    app.split_workspace_active = true;
                    app.compress_workspace_active = false;
                    app.word_workspace_active = false;
                    app.deactivate_office();
                }
                let compress_index = split_index + 1;
                if app.compress_workspace_open
                    && tab(
                        ui,
                        &t,
                        tl!("Compress PDF"),
                        false,
                        app.compress_workspace_active && app.active.is_none(),
                        &mut close,
                        compress_index,
                        "file-down",
                        true,
                    )
                    .clicked()
                {
                    app.active = None;
                    app.combine_workspace_active = false;
                    app.split_workspace_active = false;
                    app.compress_workspace_active = true;
                    app.word_workspace_active = false;
                    app.deactivate_office();
                }
                let word_index = compress_index + 1;
                if app.word_workspace_open
                    && tab(
                        ui,
                        &t,
                        tl!("PDF to Word"),
                        false,
                        app.word_workspace_active && app.active.is_none(),
                        &mut close,
                        word_index,
                        "file-text",
                        true,
                    )
                    .clicked()
                {
                    app.active = None;
                    app.combine_workspace_active = false;
                    app.split_workspace_active = false;
                    app.compress_workspace_active = false;
                    app.word_workspace_active = true;
                    app.deactivate_office();
                }
                for kind in crate::office_ui::OFFICE_KINDS {
                    let index = word_index + 1 + kind.index();
                    let state = &app.office_workspaces[kind.index()];
                    if state.open
                        && tab(
                            ui,
                            &t,
                            kind.title(),
                            false,
                            state.active && app.active.is_none(),
                            &mut close,
                            index,
                            match kind {
                                crate::office_ui::OfficeKind::PowerPoint | crate::office_ui::OfficeKind::PowerPointToPdf => "presentation",
                                crate::office_ui::OfficeKind::Excel | crate::office_ui::OfficeKind::ExcelToPdf => "grid-3x3",
                                crate::office_ui::OfficeKind::WordToPdf => "file-text",
                            },
                            true,
                        )
                        .clicked()
                    {
                        app.open_office_workspace(kind);
                    }
                }
                let jpg_index = word_index + 1 + crate::office_ui::OFFICE_KINDS.len();
                if app.jpg_workspace.open
                    && tab(ui, &t, tl!("PDF to JPG"), false, app.jpg_workspace.active && app.active.is_none(), &mut close, jpg_index, "image", true)
                        .clicked()
                {
                    app.open_jpg_workspace();
                }
                let image_to_pdf_index = jpg_index + 1;
                if app.image_to_pdf.open
                    && tab(
                        ui,
                        &t,
                        tl!("Image to PDF"),
                        !app.image_to_pdf.images.is_empty(),
                        app.image_to_pdf.active && app.active.is_none(),
                        &mut close,
                        image_to_pdf_index,
                        "image",
                        true,
                    )
                    .clicked()
                {
                    app.open_image_to_pdf_workspace();
                }
                let sign_index = image_to_pdf_index + 1;
                if app.sign_workspace.open
                    && tab(
                        ui,
                        &t,
                        tl!("Sign PDF"),
                        app.sign_workspace.source.is_some(),
                        app.sign_workspace.active && app.active.is_none(),
                        &mut close,
                        sign_index,
                        "pen-tool",
                        true,
                    )
                    .clicked()
                {
                    app.open_sign_pdf_workspace();
                }
                let watermark_index = sign_index + 1;
                if app.watermark_workspace.open
                    && tab(
                        ui,
                        &t,
                        tl!("Watermark"),
                        app.watermark_workspace.source.is_some(),
                        app.watermark_workspace.active && app.active.is_none(),
                        &mut close,
                        watermark_index,
                        "stamp",
                        true,
                    )
                    .clicked()
                {
                    app.open_watermark_workspace();
                }
                let rotate_index = watermark_index + 1;
                if app.rotate_workspace.open
                    && tab(
                        ui,
                        &t,
                        tl!("Rotate PDF"),
                        app.rotate_workspace.source.is_some(),
                        app.rotate_workspace.active && app.active.is_none(),
                        &mut close,
                        rotate_index,
                        "rotate-cw",
                        true,
                    )
                    .clicked()
                {
                    app.open_rotate_workspace();
                }
                let html_index = rotate_index + 1;
                if app.html_workspace.open
                    && tab(
                        ui,
                        &t,
                        tl!("HTML to PDF"),
                        !app.html_workspace.html_code.is_empty(),
                        app.html_workspace.active && app.active.is_none(),
                        &mut close,
                        html_index,
                        "code-xml",
                        true,
                    )
                    .clicked()
                {
                    app.open_html_workspace();
                }
                let unlock_index = html_index + 1;
                if app.unlock_workspace.open
                    && tab(
                        ui,
                        &t,
                        tl!("Unlock PDF"),
                        app.unlock_workspace.source.is_some(),
                        app.unlock_workspace.active && app.active.is_none(),
                        &mut close,
                        unlock_index,
                        "lock-open",
                        true,
                    )
                    .clicked()
                {
                    app.open_unlock_workspace();
                }
                let protect_index = unlock_index + 1;
                if app.protect_workspace.open
                    && tab(
                        ui,
                        &t,
                        tl!("Protect PDF"),
                        app.protect_workspace.source.is_some(),
                        app.protect_workspace.active && app.active.is_none(),
                        &mut close,
                        protect_index,
                        "shield-check",
                        true,
                    )
                    .clicked()
                {
                    app.open_protect_workspace();
                }
                let repair_index = protect_index + 1;
                if app.repair_workspace.open
                    && tab(
                        ui,
                        &t,
                        tl!("Repair PDF"),
                        app.repair_workspace.source.is_some(),
                        app.repair_workspace.active && app.active.is_none(),
                        &mut close,
                        repair_index,
                        "bandage",
                        true,
                    )
                    .clicked()
                {
                    app.open_repair_workspace();
                }
                let page_numbers_index = repair_index + 1;
                if app.page_numbers_workspace.open
                    && tab(
                        ui,
                        &t,
                        tl!("Page Numbers"),
                        app.page_numbers_workspace.source.is_some(),
                        app.page_numbers_workspace.active && app.active.is_none(),
                        &mut close,
                        page_numbers_index,
                        "hash",
                        true,
                    )
                    .clicked()
                {
                    app.open_page_numbers_workspace();
                }
                let delete_pages_index = page_numbers_index + 1;
                if app.delete_pages_workspace.open
                    && tab(
                        ui,
                        &t,
                        tl!("Delete pages"),
                        app.delete_pages_workspace.source.is_some(),
                        app.delete_pages_workspace.active && app.active.is_none(),
                        &mut close,
                        delete_pages_index,
                        "trash-2",
                        true,
                    )
                    .clicked()
                {
                    app.open_delete_pages_workspace();
                }
                let translate_index = delete_pages_index + 1;
                if app.translate_workspace.open
                    && tab(
                        ui,
                        &t,
                        tl!("Translate PDF"),
                        app.translate_workspace.source.is_some(),
                        app.translate_workspace.active && app.active.is_none(),
                        &mut close,
                        translate_index,
                        "languages",
                        true,
                    )
                    .clicked()
                {
                    app.open_translate_workspace();
                }
                if let Some(i) = close {
                    if i == workspace_index && app.combine_workspace_open {
                        app.close_combine_workspace();
                    } else if i == split_index && app.split_workspace_open {
                        app.close_split_workspace();
                    } else if i == compress_index && app.compress_workspace_open {
                        app.close_compress_workspace();
                    } else if i == word_index && app.word_workspace_open {
                        app.close_word_workspace();
                    } else if let Some(kind) = crate::office_ui::OFFICE_KINDS
                        .into_iter()
                        .find(|kind| i == word_index + 1 + kind.index() && app.office_workspaces[kind.index()].open)
                    {
                        app.close_office_workspace(kind);
                    } else if i == jpg_index && app.jpg_workspace.open {
                        app.close_jpg_workspace();
                    } else if i == image_to_pdf_index && app.image_to_pdf.open {
                        app.close_image_to_pdf_workspace();
                    } else if i == sign_index && app.sign_workspace.open {
                        app.close_sign_pdf_workspace();
                    } else if i == watermark_index && app.watermark_workspace.open {
                        app.close_watermark_workspace();
                    } else if i == rotate_index && app.rotate_workspace.open {
                        app.close_rotate_workspace();
                    } else if i == html_index && app.html_workspace.open {
                        app.close_html_workspace();
                    } else if i == unlock_index && app.unlock_workspace.open {
                        app.close_unlock_workspace();
                    } else if i == protect_index && app.protect_workspace.open {
                        app.close_protect_workspace();
                    } else if i == repair_index && app.repair_workspace.open {
                        app.close_repair_workspace();
                    } else if i == page_numbers_index && app.page_numbers_workspace.open {
                        app.close_page_numbers_workspace();
                    } else if i == delete_pages_index && app.delete_pages_workspace.open {
                        app.close_delete_pages_workspace();
                    } else if i == translate_index && app.translate_workspace.open {
                        app.close_translate_workspace();
                    } else {
                        app.request_close_tab(i - 1);
                    }
                }
            });
        });
}

#[allow(dead_code)]
fn main_menu(app: &mut KarnoxPDFApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let resp = widgets::ghost_button(ui, "panel-left", tl!("Menu"));
    egui::Popup::menu(&resp).show(|ui| {
        ui.set_min_width(230.0);
        ui.menu_button(tl!("File"), |ui| crate::commands::registry_menu(app, ui, "File"));
        ui.menu_button(tl!("Edit"), |ui| crate::commands::registry_menu(app, ui, "Edit"));
        ui.menu_button(tl!("Pages"), |ui| crate::commands::registry_menu(app, ui, "Pages"));
        ui.menu_button(tl!("View"), |ui| {
            if let Some(i) = app.active {
                let v = &mut app.views[i];
                ui.label(egui::RichText::new(tl!("Zoom")).color(t.text_faint).small());
                if widgets::menu_item(ui, tl!("Actual size")).clicked() {
                    v.set_zoom(1.0);
                }
                if widgets::menu_item(ui, tl!("Zoom to page level")).clicked() {
                    v.fit = Fit::Page;
                }
                if widgets::menu_item(ui, tl!("Fit to width")).clicked() {
                    v.fit = Fit::Width;
                }
                if widgets::menu_item(ui, tl!("Fit to height")).clicked() {
                    v.fit = Fit::Height;
                    v.goto = Some((v.current, 0.0));
                }
                if widgets::menu_item(ui, tl!("Fit visible")).clicked() {
                    ui.close();
                    app.execute("view.fit_visible");
                    return;
                }
                if widgets::menu_item(ui, tl!("Zoom in")).clicked() {
                    v.zoom_step(true);
                }
                if widgets::menu_item(ui, tl!("Zoom out")).clicked() {
                    v.zoom_step(false);
                }
                if widgets::menu_item(ui, tl!("Rotate view clockwise")).clicked() {
                    v.rotate_view(true);
                }
                if widgets::menu_item(ui, tl!("Rotate view counterclockwise")).clicked() {
                    v.rotate_view(false);
                }
                ui.separator();
                ui.label(egui::RichText::new(tl!("Page navigation")).color(t.text_faint).small());
                if ui.add_enabled(!v.back.is_empty(), egui::Button::new(tl!("Previous view"))).clicked() {
                    v.view_history(false);
                }
                if ui.add_enabled(!v.forward.is_empty(), egui::Button::new(tl!("Next view"))).clicked() {
                    v.view_history(true);
                }
                ui.separator();
                if let Some(id) = page_display_menu(ui, &app.views[i]) {
                    app.execute(id);
                }
                ui.separator();
            }
            crate::commands::registry_menu(app, ui, "View");
            ui.menu_button(tl!("Display theme"), |ui| theme_menu(app, ui));
            ui.menu_button(tl!("Side panels"), |ui| {
                for (p, label) in [
                    (RightPanel::Comments, tl!("Comments")),
                    (RightPanel::Bookmarks, tl!("Bookmarks")),
                    (RightPanel::Pages, tl!("Pages")),
                    (RightPanel::Fields, tl!("Fields")),
                    (RightPanel::Layers, tl!("Layers")),
                    (RightPanel::Attachments, tl!("Attachments")),
                    (RightPanel::Signatures, tl!("Signatures")),
                    (RightPanel::Accessibility, tl!("Accessibility Checker")),
                    (RightPanel::Search, tl!("Search")),
                    (RightPanel::Compare, tl!("Compare")),
                ] {
                    if ui.radio(app.right == Some(p), tl!(label)).clicked() {
                        app.right = Some(p);
                    }
                }
            });
        });
        ui.menu_button(tl!("Help"), |ui| crate::commands::registry_menu(app, ui, "Help"));
    });
}

pub fn right_rail(app: &mut KarnoxPDFApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let Some((index, id)) = app.active_ids() else { return };
    let Some(doc) = app.session.get(id) else { return };
    let has_signatures = doc.is_signed();
    let has_check = app.a11y.report.as_ref().is_some_and(|(d, _)| *d == id);
    let (has_comments, has_outline, has_fields, has_layers, has_files) = (
        !doc.info.annotations.is_empty(),
        !doc.info.outline.is_empty(),
        !doc.info.fields.is_empty(),
        !doc.info.layers.is_empty(),
        !doc.info.attachments.is_empty(),
    );
    let page_count = doc.info.pages.len();
    let labels: Vec<String> = doc.info.pages.iter().map(|p| p.label.clone()).collect();
    egui::Panel::right("rail")
        .resizable(false)
        .exact_size(48.0)
        .frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin::symmetric(6, 8)).stroke(Stroke::new(1.0, t.divider)))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            let mut rail_button = |ui: &mut egui::Ui, panel: RightPanel, icon: &str, tip: &str, has: bool| {
                let selected = app.right == Some(panel);
                let r = icons::button(ui, icon, 34.0, selected, tl!(tip));
                if has && !selected {
                    let c = r.rect.right_top() + vec2(-8.0, 8.0);
                    ui.painter().circle_filled(c, 3.0, t.accent);
                }
                if r.clicked() {
                    app.right = if selected { None } else { Some(panel) };
                }
            };
            rail_button(ui, RightPanel::Comments, "message-square-text", "Comments", has_comments);
            rail_button(ui, RightPanel::Bookmarks, "bookmark", "Bookmarks", has_outline);
            rail_button(ui, RightPanel::Pages, "files", "Page thumbnails", false);
            rail_button(ui, RightPanel::Fields, "text-cursor-input", "Form fields", has_fields);
            rail_button(ui, RightPanel::Layers, "layers", "Layers", has_layers);
            rail_button(ui, RightPanel::Attachments, "paperclip", "Attachments", has_files);
            rail_button(ui, RightPanel::Signatures, "signature", "Signatures", has_signatures);
            if has_check {
                rail_button(ui, RightPanel::Accessibility, "accessibility", "Accessibility Checker", false);
            }

            // Page navigation cluster at the bottom (as in Acrobat's rail).
            let view = &mut app.views[index];
            let mut page_display = None;
            ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                if icons::button(ui, "zoom-out", 32.0, false, tl!("Zoom out")).clicked() {
                    view.zoom_step(false);
                }
                if icons::button(ui, "zoom-in", 32.0, false, tl!("Zoom in")).clicked() {
                    view.zoom_step(true);
                }
                // Between rotate and zoom, as in Acrobat. Its command runs once `view` is free.
                let tip = crate::i18n::fmt(tl!("Page display: {layout}"), &[("layout", tl!(view.layout.label()))]);
                let resp = icons::button(ui, view.layout.icon(), 32.0, false, &tip);
                page_display = egui::Popup::menu(&resp)
                    .show(|ui| {
                        ui.set_min_width(220.0);
                        rail_view_menu(ui, view)
                    })
                    .and_then(|r| r.inner);
                if icons::button(ui, "rotate-cw", 32.0, false, tl!("Rotate view clockwise")).clicked() {
                    view.rotate_view(true);
                }
                // Not `columns-2` while fitting the page: that is the two-page view's icon.
                let fit_icon = if view.fit == Fit::Width { "maximize" } else { "maximize-2" };
                if icons::button(ui, fit_icon, 32.0, false, tl!("Toggle fit page / fit width")).clicked() {
                    view.fit = if view.fit == Fit::Width { Fit::Page } else { Fit::Width };
                    view.goto = Some((view.current, 0.0));
                }
                ui.label(egui::RichText::new(format!("{:.0}%", view.zoom * 100.0)).font(theme::regular(10.5)).color(t.text_faint));
                ui.add_space(6.0);
                if icons::button(ui, "chevron-down", 30.0, false, tl!("Next page")).clicked() {
                    view.step_page(true);
                }
                if icons::button(ui, "chevron-up", 30.0, false, tl!("Previous page")).clicked() {
                    view.step_page(false);
                }
                ui.label(egui::RichText::new(page_count.to_string()).font(theme::regular(11.0)).color(t.text_muted));
                let edit = egui::TextEdit::singleline(&mut view.page_input)
                    .id(egui::Id::new("page-input"))
                    .desired_width(34.0)
                    .horizontal_align(Align::Center)
                    .font(theme::medium(12.0))
                    .margin(vec2(2.0, 4.0));
                let r = egui::Frame::NONE
                    .fill(t.field)
                    .stroke(Stroke::new(1.0, t.border))
                    .corner_radius(CornerRadius::same(5))
                    .show(ui, |ui| ui.add(edit))
                    .inner
                    .on_hover_text(tl!("Current page — type a page number or label (such as iv) and press Enter"));
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    // A page label first (logical page numbers, as Acrobat), then a number.
                    let typed = view.page_input.clone();
                    if !view.go_to_typed(&typed, &labels) {
                        view.page_input = (view.current + 1).to_string();
                    }
                }
            });
            if let Some(id) = page_display {
                app.execute(id);
            }
        });
}

/// View ▸ Page display and the rail's Page display menu: the layouts and the cover page.
/// Returns the command a click asks for, for the caller to run.
fn page_display_menu(ui: &mut egui::Ui, view: &DocView) -> Option<&'static str> {
    let t = Tokens::get(ui.ctx());
    ui.label(egui::RichText::new(tl!("Page display")).color(t.text_faint).small());
    let mut picked = None;
    for l in PageLayout::ORDER {
        if ui.radio(view.layout == l, tl!(l.label())).clicked() {
            picked = Some(l.command());
        }
    }
    let mut cover = view.cover;
    if ui.add_enabled(view.cover_applies(), egui::Checkbox::new(&mut cover, tl!("Show cover page in two-page view"))).changed() {
        picked = Some("view.layout.cover");
    }
    if picked.is_some() {
        ui.close();
    }
    picked
}

/// The rail's Page display menu: Acrobat's view choices around the page display. Zoom changes
/// apply at once; the command a click asks for is returned for the caller to run.
fn rail_view_menu(ui: &mut egui::Ui, view: &mut DocView) -> Option<&'static str> {
    let mut picked = None;
    for (id, label) in [("view.fit_width_scrolling", "Fit to width scrolling"), ("view.fit_one_page", "Fit one full page")] {
        if ui.button(tl!(label)).clicked() {
            picked = Some(id);
        }
    }
    ui.separator();
    if ui.button(tl!("Actual size")).clicked() {
        view.set_zoom(1.0);
        ui.close();
    }
    if ui.button(tl!("Zoom to page level")).clicked() {
        view.set_fit(Fit::Page);
        ui.close();
    }
    if ui.button(tl!("Fit visible")).clicked() {
        picked = Some("view.fit_visible");
    }
    ui.separator();
    picked = page_display_menu(ui, view).or(picked);
    ui.separator();
    for (id, label) in [("view.read_mode", "Read mode"), ("view.full_screen", "Full screen mode")] {
        if ui.button(tl!(label)).clicked() {
            picked = Some(id);
        }
    }
    if picked.is_some() {
        ui.close();
    }
    picked
}
