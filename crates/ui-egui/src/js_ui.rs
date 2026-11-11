//! Acrobat JavaScript in the shell: what scripts ask for (alerts, printing, navigation, links),
//! the JavaScript console (⌘J), Document JavaScripts, and Preferences ▸ JavaScript.

use karnoxpdf_engine::js::{JsOutput, Request};
use karnoxpdf_engine::{DocId, Edit};

use crate::theme::{self, Tokens};
use crate::{KarnoxPDFApp, widgets};

/// The JavaScript console: the input and the output so far.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct JsConsole {
    pub input: String,
    pub log: Vec<String>,
}

/// Document JavaScripts: the script being edited.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DocJsDraft {
    pub name: String,
    pub script: String,
}

impl KarnoxPDFApp {
    /// Act on what scripts produced in document `id`: alerts are shown, console output goes to
    /// the console, print and page requests are carried out, links wait for the user's permission
    /// (form submissions are reported, never sent).
    pub fn handle_js(&mut self, id: DocId, out: JsOutput) {
        if out.is_empty() {
            return;
        }
        self.js_console.log.extend(out.console.iter().cloned());
        for e in &out.errors {
            self.js_console.log.push(format!("Error: {e}"));
        }
        let view = self.views.iter().position(|v| v.id == id);
        for r in out.requests {
            match r {
                Request::Print => self.open_print(),
                Request::GoToPage(p) => {
                    if let Some(i) = view {
                        self.views[i].go_to_page(p);
                    }
                }
                Request::LaunchUrl(u) => self.request_document_url(&u, crate::LinkOrigin::Script),
                Request::Submit(u) => self.notify_fmt(
                    "The form asks to be submitted to {u}; KarnoxPDF doesn't send form data. Save the document to keep your entries.",
                    &[("u", &u)],
                ),
                Request::SaveAs => self.run_command("file.save_as"),
                Request::Focus(_) | Request::Beep | Request::Reset(_) => {}
            }
        }
        if let Some(a) = out.alerts.last() {
            self.notify(a.clone());
        }
    }

    /// Run a push button's JavaScript (its Mouse Up action).
    pub fn run_button_script(&mut self, id: DocId, field: &str, script: &str) {
        match self.session.run_javascript(id, script, Some(field)) {
            Ok(o) => {
                if let Some(i) = self.views.iter().position(|v| v.id == id)
                    && let Some(info) = self.session.get(id).map(|d| d.info.clone())
                {
                    self.views[i].document_changed(&info);
                }
                let out = JsOutput { alerts: o.alerts, console: o.console, requests: o.requests, errors: o.error.into_iter().collect() };
                self.handle_js(id, out);
            }
            Err(e) => self.notify_fmt("{field}: {e}", &[("field", field), ("e", &e.to_string())]),
        }
    }

    /// Prepare a form ▸ detect fields from the page's blanks, lines and boxes.
    pub fn detect_fields(&mut self) {
        let Some((i, id)) = self.active_ids() else { return };
        match self.session.auto_detect_fields(id, &[]) {
            Ok(names) if names.is_empty() => self.notify_tr("No form fields were detected"),
            Ok(names) => {
                if let Some(info) = self.session.get(id).map(|d| d.info.clone()) {
                    self.views[i].document_changed(&info);
                }
                if names.len() == 1 {
                    self.notify_tr("Detected 1 form field");
                } else {
                    self.notify_fmt("Detected {n} form fields", &[("n", &names.len().to_string())]);
                }
            }
            Err(e) => self.notify_error(e),
        }
    }

    /// The console's Run: evaluate the input in the active document.
    pub fn run_console(&mut self) {
        let Some((i, id)) = self.active_ids() else { return };
        let script = self.js_console.input.clone();
        if script.trim().is_empty() {
            return;
        }
        self.js_console.log.push(format!("> {}", script.trim()));
        match self.session.run_javascript(id, &script, None) {
            Ok(o) => {
                if let Some(info) = self.session.get(id).map(|d| d.info.clone()) {
                    self.views[i].document_changed(&info);
                }
                let (error, result) = (o.error.clone(), o.result.clone());
                let out = JsOutput { alerts: o.alerts, console: o.console, requests: o.requests, errors: Vec::new() };
                self.handle_js(id, out);
                match error {
                    Some(e) => self.js_console.log.push(format!("Error: {e}")),
                    None => self.js_console.log.extend(result),
                }
            }
            Err(e) => self.js_console.log.push(format!("Error: {e}")),
        }
    }
}

fn buttons(ui: &mut egui::Ui, primary: &str, others: &[&str]) -> Option<String> {
    let mut clicked = None;
    ui.horizontal(|ui| {
        crate::widgets::button_row(ui, |ui| {
            // Stable ids: the console's output above changes how many widgets come first.
            // Labels are translated for display; the returned id stays English.
            if ui.push_id(primary, |ui| widgets::pill_button(ui, tl!(primary), true)).inner.clicked() {
                clicked = Some(primary.to_string());
            }
            for o in others {
                if ui.push_id(o, |ui| widgets::pill_button(ui, tl!(o), false)).inner.clicked() {
                    clicked = Some(o.to_string());
                }
            }
        })
    });
    clicked
}

/// The JavaScript console. Returns `true` to close.
pub(crate) fn console_body(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens) -> bool {
    ui.label(egui::RichText::new(tl!("JavaScript Console")).font(theme::semibold(18.0)));
    ui.add_space(6.0);
    if !app.session.javascript() {
        ui.label(egui::RichText::new(tl!("JavaScript is turned off (Preferences ▸ JavaScript).")).small().color(t.text_muted));
    }
    egui::Frame::new().fill(t.hover).corner_radius(egui::CornerRadius::same(6)).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        egui::ScrollArea::vertical().max_height(220.0).stick_to_bottom(true).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(160.0);
            if app.js_console.log.is_empty() {
                ui.label(egui::RichText::new(tl!("Output appears here.")).color(t.text_muted));
            }
            for line in &app.js_console.log {
                ui.label(egui::RichText::new(line).monospace());
            }
        });
    });
    ui.add_space(6.0);
    let input = ui.add(
        egui::TextEdit::multiline(&mut app.js_console.input)
            .code_editor()
            .desired_rows(4)
            .desired_width(f32::INFINITY)
            .hint_text(tl!("JavaScript, e.g. getField(\"total\").value"))
            .id_salt("js-console-input"),
    );
    let run_key = crate::shortcuts_on(ui.ctx()) && input.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.command);
    ui.add_space(8.0);
    match buttons(ui, "Run", &["Close", "Clear"]).as_deref() {
        Some("Run") => app.run_console(),
        Some("Clear") => app.js_console.log.clear(),
        Some("Close") => return true,
        _ if run_key => app.run_console(),
        _ => {}
    }
    false
}

/// Document JavaScripts: list, edit, add and delete. Returns `true` to close.
pub(crate) fn document_js_body(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens) -> bool {
    ui.label(egui::RichText::new(tl!("Document JavaScripts")).font(theme::semibold(18.0)));
    ui.add_space(6.0);
    let scripts = app.active_ids().and_then(|(_, id)| app.session.get(id)).map(|d| d.document_scripts()).unwrap_or_default();
    let mut edit: Option<Edit> = None;
    ui.horizontal(|ui| {
        ui.label(tl!("Script Name:"));
        ui.add(egui::TextEdit::singleline(&mut app.doc_js.name).desired_width(240.0).id_salt("doc-js-name"));
    });
    ui.add_space(4.0);
    egui::Frame::new().fill(t.hover).corner_radius(egui::CornerRadius::same(6)).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        egui::ScrollArea::vertical().max_height(120.0).id_salt("doc-js-list").show(ui, |ui| {
            ui.set_width(ui.available_width());
            if scripts.is_empty() {
                ui.label(egui::RichText::new(tl!("This document has no document-level scripts.")).color(t.text_muted));
            }
            for (name, js) in &scripts {
                if ui.selectable_label(app.doc_js.name == *name, name).clicked() {
                    app.doc_js = DocJsDraft { name: name.clone(), script: js.clone() };
                }
            }
        });
    });
    ui.add_space(6.0);
    ui.add(egui::TextEdit::multiline(&mut app.doc_js.script).code_editor().desired_rows(8).desired_width(f32::INFINITY).id_salt("doc-js-script"));
    ui.add_space(8.0);
    let name = app.doc_js.name.trim().to_string();
    let close = match buttons(ui, "Save", &["Close", "Delete"]).as_deref() {
        Some("Save") if !name.is_empty() => {
            edit = Some(Edit::SetDocumentScript { name, script: Some(app.doc_js.script.clone()) });
            false
        }
        Some("Delete") if scripts.iter().any(|(n, _)| *n == name) => {
            edit = Some(Edit::SetDocumentScript { name, script: None });
            app.doc_js = DocJsDraft::default();
            false
        }
        Some("Close") => true,
        _ => false,
    };
    if let Some(e) = edit {
        app.apply_edit(e);
    }
    close
}

fn preferences_card(ui: &mut egui::Ui, t: &Tokens, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(t.card)
        .stroke(egui::Stroke::new(1.0, t.border))
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
}

const PREFERENCE_LANGUAGES: [(&str, &str); 5] =
    [("pt-br", "Português do Brasil"), ("pt-pt", "Português de Portugal"), ("en", "Inglês"), ("es", "Espanhol"), ("fr", "Francês")];

fn preference_language_label(code: &str) -> &'static str {
    PREFERENCE_LANGUAGES.iter().find(|(c, _)| *c == code).map(|(_, label)| *label).unwrap_or_else(|| {
        let resolved = crate::i18n::Lang::from_pref(code).code();
        PREFERENCE_LANGUAGES.iter().find(|(c, _)| *c == resolved).map(|(_, label)| *label).unwrap_or("Português do Brasil")
    })
}

/// Preferences: language, appearance and document defaults. Returns `true` to close.
pub(crate) fn preferences_body(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens) -> bool {
    ui.label(egui::RichText::new(tl!("Preferences")).font(theme::semibold(22.0)));
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 16.0;
        for (tab, label) in [(0, "General"), (1, "Documents and view"), (2, "JavaScript")] {
            ui.selectable_value(&mut app.preferences_tab, tab, tl!(label));
        }
    });
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(10.0);
    let body_height = (ui.ctx().content_rect().height() - 210.0).clamp(260.0, 390.0);
    egui::ScrollArea::vertical()
        .id_salt("preferences-scroll")
        .min_scrolled_height(body_height)
        .max_height(body_height)
        .show(ui, |ui| {
        ui.set_width(ui.available_width());
        if app.preferences_tab == 0 {
        preferences_card(ui, t, |ui| {
            let language_label = ui.label(egui::RichText::new(tl!("Interface language")).font(theme::semibold(14.0)));
            ui.add_space(6.0);
            let selected = preference_language_label(&app.language);
            let before = app.language.clone();
            egui::ComboBox::from_id_salt("interface-language")
                .width(280.0)
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    for (code, label) in PREFERENCE_LANGUAGES {
                        if ui.selectable_value(&mut app.language, code.to_string(), label).clicked() {
                            ui.close();
                        }
                    }
                })
                .response
                .labelled_by(language_label.id);
            if app.language != before {
                crate::i18n::set_current(crate::i18n::Lang::from_pref(&app.language));
            }
            ui.add_space(14.0);
            ui.label(egui::RichText::new(tl!("Display theme")).font(theme::semibold(14.0)));
            ui.add_space(6.0);
            use crate::theme::ThemePreference;
            let mut selected = app.theme_preference;
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 20.0;
                ui.radio_value(&mut selected, ThemePreference::System, tl!("Use system setting"));
                ui.radio_value(&mut selected, ThemePreference::Light, tl!("Light gray"));
                ui.radio_value(&mut selected, ThemePreference::Dark, tl!("Dark gray"));
            });
            if selected != app.theme_preference {
                app.set_theme_preference(selected);
            }
        });
        ui.add_space(10.0);
        }
        if app.preferences_tab == 1 {
        preferences_card(ui, t, |ui| {
            ui.label(egui::RichText::new(tl!("Documents and view")).font(theme::semibold(15.0)));
            ui.add_space(12.0);
            let mode_label = ui.label(egui::RichText::new(tl!("Default workspace mode")).font(theme::medium(13.0)));
            ui.add_space(4.0);
            let modes = [
                (crate::Mode::AllTools, "All Tools"),
                (crate::Mode::Read, "Read"),
                (crate::Mode::Edit, "Edit"),
                (crate::Mode::Convert, "Convert"),
                (crate::Mode::Sign, "E-Sign"),
            ];
            let current = modes.iter().find(|(mode, _)| *mode == app.default_mode).map_or("All Tools", |(_, label)| *label);
            egui::ComboBox::from_id_salt("default-workspace").width(280.0).selected_text(tl!(current)).show_ui(ui, |ui| {
                for (mode, label) in modes {
                    ui.selectable_value(&mut app.default_mode, mode, tl!(label));
                }
            }).response.labelled_by(mode_label.id);
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(tl!("Used when opening PDFs. An explicit launch or control mode takes precedence for the session."))
                    .small()
                    .color(t.text_muted),
            );
            ui.add_space(14.0);
            let defaults = &mut app.view_defaults;
            let page_label = ui.label(egui::RichText::new(tl!("Default page display")).font(theme::medium(13.0)));
            ui.add_space(4.0);
            egui::ComboBox::from_id_salt("default-page-display")
                .width(280.0)
                .selected_text(tl!(defaults.layout.label()))
                .show_ui(ui, |ui| {
                    for layout in crate::canvas::PageLayout::ORDER {
                        ui.selectable_value(&mut defaults.layout, layout, tl!(layout.label()));
                    }
                }).response.labelled_by(page_label.id);
            ui.add_space(12.0);
            let zoom_label = ui.label(egui::RichText::new(tl!("Default zoom")).font(theme::medium(13.0)));
            ui.add_space(4.0);
            use crate::canvas::Fit;
            let fit_label = match defaults.fit {
                Fit::Width => tl!("Fit to width"),
                Fit::Page => tl!("Zoom to page level"),
                Fit::Height => tl!("Fit to height"),
                Fit::None => tl!("Custom"),
            };
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("default-zoom").width(210.0).selected_text(fit_label).show_ui(ui, |ui| {
                    ui.selectable_value(&mut defaults.fit, Fit::Width, tl!("Fit to width"));
                    ui.selectable_value(&mut defaults.fit, Fit::Page, tl!("Zoom to page level"));
                    ui.selectable_value(&mut defaults.fit, Fit::Height, tl!("Fit to height"));
                    ui.selectable_value(&mut defaults.fit, Fit::None, tl!("Custom"));
                }).response.labelled_by(zoom_label.id);
                let mut percent = defaults.zoom * 100.0;
                if ui.add(egui::DragValue::new(&mut percent).range(8.0..=6400.0).max_decimals(0).suffix("%")).changed() {
                    (defaults.fit, defaults.zoom) = (Fit::None, percent / 100.0);
                }
            });
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(tl!("Used when a PDF doesn't ask for a layout or zoom. Continuous scrolling never snaps between pages."))
                    .small()
                    .color(t.text_muted),
            );
        });
        ui.add_space(10.0);
        }
        if app.preferences_tab == 0 {
        preferences_card(ui, t, |ui| {
            ui.label(egui::RichText::new(tl!("Identity")).font(theme::semibold(15.0)));
            ui.add_space(10.0);
            let label = ui.label(tl!("Name on new comments"));
            ui.add(egui::TextEdit::singleline(&mut app.comment_prefs.author).desired_width(320.0).char_limit(crate::MAX_AUTHOR_CHARS))
                .labelled_by(label.id);
        });
        ui.add_space(10.0);
        }
        if app.preferences_tab == 2 {
        preferences_card(ui, t, |ui| {
            ui.label(egui::RichText::new("JavaScript").font(theme::semibold(15.0)));
            ui.add_space(10.0);
            let mut on = app.session.javascript();
            if ui.checkbox(&mut on, tl!("Enable Acrobat JavaScript")).changed() {
                app.session.set_javascript(on);
            }
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(tl!("Scripts run in a sandbox without file or network access. A script that asks to open a web page needs your permission first. With JavaScript off, Acrobat's standard format, validate and calculate functions still work."))
                    .small()
                    .color(t.text_muted),
            );
        });
        }
    });
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(6.0);
    buttons(ui, tl!("OK"), &[]).is_some()
}
