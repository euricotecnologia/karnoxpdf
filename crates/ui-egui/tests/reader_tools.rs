//! The reading screen's tool bar: each button opens its tool with the document on screen attached.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use karnoxpdf_ui_egui::{KarnoxPDFApp, Mode};

/// An `n`-page document with a proper xref table.
fn fixture(n: usize) -> Vec<u8> {
    let mut objs: Vec<String> = vec!["<< /Type /Catalog /Pages 2 0 R >>".into()];
    let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 4 + 2 * i)).collect();
    objs.push(format!("<< /Type /Pages /Kids [{}] /Count {n} /MediaBox [0 0 200 300] >>", kids.join(" ")));
    objs.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into());
    for i in 0..n {
        objs.push(format!("<< /Type /Page /Parent 2 0 R /Contents {} 0 R /Resources << /Font << /F1 3 0 R >> >> >>", 5 + 2 * i));
        let body = format!("BT /F1 24 Tf 20 150 Td (Page {}) Tj ET", i + 1);
        objs.push(format!("<< /Length {} >>\nstream\n{body}\nendstream", body.len()));
    }
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for o in offsets {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1).as_bytes());
    out
}

fn harness() -> Harness<'static, KarnoxPDFApp> {
    let mut h = Harness::builder().with_size(egui::vec2(1500.0, 900.0)).build_eframe(|_cc| {
        let mut app = KarnoxPDFApp::new();
        app.set_option("language", "en").unwrap();
        app.open_bytes("contract.pdf", None, fixture(4)).expect("opens");
        app
    });
    h.run_steps(4);
    h
}

#[test]
fn the_bar_replaces_the_side_list_and_offers_every_tool() {
    let mut h = harness();
    assert!(!h.state().left_open, "the All tools list is not open on the reading screen");
    assert_eq!(h.state().mode, Mode::AllTools);
    if let Ok(path) = std::env::var("KARNOXPDF_READER_SHOT") {
        h.render().expect("render").save(path).expect("save screenshot");
    }
    for label in [
        "Merge PDF",
        "Split PDF",
        "Compress PDF",
        "PDF to Word",
        "PDF to PowerPoint",
        "PDF to Excel",
        "Edit PDF",
        "PDF to JPG",
        "Sign PDF",
        "Watermark",
        "Rotate PDF",
        "Protect PDF",
        "Page Numbers",
        "Delete Pages",
        "Translate PDF",
        "More tools",
    ] {
        h.get_by_label(label);
    }
}

#[test]
fn a_button_opens_its_tool_with_the_document_attached() {
    let mut h = harness();
    h.get_by_label("Split PDF").click();
    h.run_steps(3);
    assert!(h.state().split_workspace_active);
    let source = h.state().split_source.clone().expect("the document is attached");
    assert_eq!((source.name.as_str(), source.pages), ("contract.pdf", 4));
    // Back on the document, another tool.
    h.get_by_role_and_label(egui::accesskit::Role::Button, "contract.pdf").click();
    h.run_steps(3);
    h.get_by_label("Rotate PDF").click();
    h.run_steps(3);
    assert!(h.state().rotate_workspace.active);
    assert_eq!(h.state().rotate_workspace.source.as_ref().map(|s| s.name.as_str()), Some("contract.pdf"));
}

#[test]
fn edit_opens_the_editor_and_more_tools_toggles_the_full_list() {
    let mut h = harness();
    h.get_by_label("More tools").click();
    h.run_steps(3);
    assert!(h.state().left_open, "the full list opens on request");
    h.get_by_label("More tools").click();
    h.run_steps(3);
    assert!(!h.state().left_open, "and closes again");
    h.get_by_label("Edit PDF").click();
    h.run_steps(3);
    assert_eq!(h.state().mode, Mode::Edit, "Edit PDF works on the open document");
}

#[test]
fn the_tab_bar_has_no_search_preferences_shortcuts_or_theme_buttons() {
    let h = harness();
    for gone in ["Find tools and commands", "Preferences…", "Keyboard shortcuts", "Display theme"] {
        assert_eq!(h.query_all_by_label_contains(gone).count(), 0, "{gone} is not on the tab bar any more");
    }
    // The theme and language menus remain on the top toolbar.
    h.get_by_label("System themes");
    h.get_by_label("Language");
}
