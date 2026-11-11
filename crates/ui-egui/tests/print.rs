//! The Print dialog in the real shell (egui_kittest): settings, preview sheets, Save as PDF.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use karnoxpdf_ui_egui::{Dialog, KarnoxPDFApp};

fn harness() -> Harness<'static, KarnoxPDFApp> {
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(|_cc| {
        let mut app = KarnoxPDFApp::new();
        app.open_bytes("form.pdf", None, include_bytes!("data/form.pdf").to_vec()).unwrap();
        app
    });
    h.run_steps(4);
    h
}

#[test]
fn print_dialog_lays_out_sheets_and_saves_a_pdf() {
    let mut h = harness();
    assert!(h.state_mut().execute("print.dialog"));
    h.run_steps(3);
    assert_eq!(h.state().dialog, Some(Dialog::Print));
    h.get_by_label("Pages to Print");
    h.get_by_label("Sheet 1 of 1");
    // Two copies of the one page, as a 2-up poster-free layout: Multiple.
    h.get_by_label("Multiple").click();
    h.run_steps(2);
    h.get_by_label("Sheet 1 of 1");
    // No printer on a test machine: Save as PDF.
    let out = std::env::temp_dir().join(format!("karnoxpdf-print-test-{}.pdf", std::process::id()));
    h.state_mut().save_override = Some(out.to_string_lossy().into_owned());
    h.state_mut().print_draft.printer = None;
    h.run_steps(1);
    h.get_by_label("Save as PDF").click();
    h.run_steps(3);
    assert_eq!(h.state().dialog, None);
    let bytes = std::fs::read(&out).expect("saved");
    let doc = karnoxpdf_cos::Document::open(std::sync::Arc::new(bytes)).unwrap();
    assert_eq!(karnoxpdf_model::pages(&doc).len(), 1);
    let _ = std::fs::remove_file(out);
}

#[test]
fn invalid_ranges_are_explained_in_the_preview() {
    let mut h = harness();
    h.state_mut().execute("print.dialog");
    h.run_steps(2);
    h.state_mut().print_draft.which = karnoxpdf_ui_egui::PrintWhich::Range;
    h.state_mut().print_draft.range = "7".into();
    h.run_steps(2);
    h.get_by_label_contains("out of range");
}

#[test]
fn cut_stack_dialog_previews_saves_and_refuses_duplex() {
    use karnoxpdf_engine::print::{PageOrder, spool::Duplex};
    let mut h = harness();
    // Ten source pages, generated using the headless engine.
    let bytes = h.state().session.create_blank(200.0, 300.0, 10).unwrap();
    h.state_mut().open_bytes("numbered.pdf", None, bytes.as_ref().clone()).unwrap();
    h.state_mut().execute("print.dialog");
    h.run_steps(3);
    h.get_by_label("Multiple").click();
    h.run_steps(2);
    // Pick the new order through the actual widgets.
    h.get_by_value("Horizontal").click();
    h.run_steps(2);
    h.get_by_label("Cut and stack").click();
    h.run_steps(2);
    assert_eq!(h.state().print_draft.order, PageOrder::CutStack);
    h.state_mut().print_draft.per_sheet = 4;
    h.run_steps(2);
    h.get_by_label("Sheet 1 of 3");
    h.get_by_label_contains("Keep the sheets in order");
    h.get_by_label("Next sheet").click();
    h.run_steps(2);
    h.get_by_label("Sheet 2 of 3");
    h.state_mut().print_draft.duplex = Duplex::LongEdge;
    h.run_steps(2);
    h.get_by_label_contains("Cut and stack needs Two-sided: Off");
    assert!(!h.state_mut().print_now());
    h.state_mut().print_draft.duplex = Duplex::Off;
    h.state_mut().print_draft.printer = None;
    let out = std::env::temp_dir().join(format!("karnoxpdf-cut-stack-ui-{}.pdf", std::process::id()));
    h.state_mut().save_override = Some(out.to_string_lossy().into_owned());
    h.run_steps(2);
    h.get_by_label("Save as PDF").click();
    h.run_steps(3);
    assert_eq!(h.state().dialog, None);
    let printed = karnoxpdf_cos::Document::open(std::sync::Arc::new(std::fs::read(&out).unwrap())).unwrap();
    assert_eq!(karnoxpdf_model::pages(&printed).len(), 3);
    let _ = std::fs::remove_file(out);
}

#[test]
fn a_failed_save_as_pdf_keeps_the_dialog_open() {
    let mut h = harness();
    assert!(h.state_mut().execute("print.dialog"));
    h.run_steps(3);
    h.state_mut().print_draft.printer = None;
    // A folder that doesn't exist: the write fails.
    let out = std::env::temp_dir().join(format!("karnoxpdf-missing-{}", std::process::id())).join("x.pdf");
    h.state_mut().save_override = Some(out.to_string_lossy().into_owned());
    h.run_steps(2);
    h.get_by_label("Save as PDF").click();
    h.run_steps(3);
    assert_eq!(h.state().dialog, Some(Dialog::Print), "the dialog stays open, with its settings");
    let toast = h.state().toast.as_ref().map(|(m, _)| m.clone()).unwrap_or_default();
    assert!(toast.contains("Could not save"), "the user is told why: {toast:?}");
}

#[test]
fn the_dialog_lists_the_printers_of_the_system_and_picks_the_default() {
    let mut h = harness();
    let known = karnoxpdf_engine::print::spool::printers();
    assert!(h.state_mut().execute("print.dialog"));
    h.run_steps(3);
    let draft = &h.state().print_draft;
    assert_eq!(draft.printers, known, "the dialog shows what the system has");
    let expected = known.iter().find(|p| p.default).or(known.first()).map(|p| p.name.clone());
    assert_eq!(draft.printer, expected, "the default printer is chosen first, or Save as PDF when there is none");
}

#[test]
fn a_click_outside_the_dialog_keeps_it_open_and_escape_still_closes_it() {
    let mut h = harness();
    assert!(h.state_mut().execute("print.dialog"));
    h.run_steps(3);
    h.get_by_label("Multiple").click();
    h.run_steps(2);
    // The corner of the window is far outside the dialog.
    h.hover_at(egui::pos2(6.0, 6.0));
    h.step();
    h.drag_at(egui::pos2(6.0, 6.0));
    h.step();
    h.drop_at(egui::pos2(6.0, 6.0));
    h.run_steps(3);
    assert_eq!(h.state().dialog, Some(Dialog::Print), "a stray click must not close the dialog");
    h.get_by_label("Pages per sheet:");
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert_eq!(h.state().dialog, None);
}

/// The dialog window remembers its height by id: a short dialog that replaces the tall Print one
/// must not come out as tall (the Close button would float in the middle of white space).
#[test]
fn a_dialog_that_replaces_print_is_not_stretched_to_its_height() {
    let mut h = Harness::builder().with_size(egui::vec2(1338.0, 1299.0)).build_eframe(|_cc| {
        let mut app = KarnoxPDFApp::new();
        app.open_bytes("form.pdf", None, include_bytes!("data/form.pdf").to_vec()).unwrap();
        app
    });
    h.run_steps(3);
    assert!(h.state_mut().execute("print.dialog"));
    h.run_steps(3);
    h.state_mut().dialog = Some(Dialog::About);
    h.run_steps(4);
    let gap = h.get_by_label("Close").rect().max.y - h.get_by_label("Developer").rect().min.y;
    assert!(gap < 150.0, "Close is {gap}px below the first line: the dialog is stretched");
}
