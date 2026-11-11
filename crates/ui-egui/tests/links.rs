//! Source attribution links in Help, About and the home screen.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use karnoxpdf_engine::links;
use karnoxpdf_ui_egui::{Dialog, KarnoxPDFApp};

fn harness(setup: impl FnOnce(&mut KarnoxPDFApp) + 'static) -> Harness<'static, KarnoxPDFApp> {
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(move |_cc| {
        let mut app = KarnoxPDFApp::new();
        setup(&mut app);
        app
    });
    h.run_steps(4);
    h
}

#[test]
fn home_screen_links() {
    let h = harness(|_| {});
    assert_eq!(h.query_all_by_label("Source and credits").count(), 0);
    assert_eq!(h.query_all_by_label("Original source on GitHub").count(), 0);
}

#[test]
fn help_commands_open_each_link() {
    for l in links::LINKS {
        let mut h = harness(|_| {});
        assert!(h.state_mut().execute(l.command), "{}", l.command);
        assert_eq!(h.state().last_opened_url.as_deref(), Some(l.url));
        assert_eq!(karnoxpdf_engine::commands::command(l.command).unwrap().menu, Some("Help"));
    }
}

#[test]
fn the_about_dialog_has_no_tabs_and_no_source_link() {
    let pdf = b"%PDF-1.7\n1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj\n3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >> endobj\ntrailer << /Root 1 0 R >>\n%%EOF";
    // With a document open, so the home screen's own links are not on screen.
    let mut h = harness(move |app| {
        app.open_bytes("one.pdf", None, pdf.to_vec()).unwrap();
        app.dialog = Some(Dialog::About);
    });
    for gone in ["Contributors", "Models", "Original source on GitHub"] {
        assert_eq!(h.query_all_by_label(gone).count(), 0, "{gone} is not in the About window");
    }
    h.get_by_label("Developer");
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Close").click();
    h.run_steps(2);
    assert!(h.state().dialog.is_none(), "Close closes it");
}
