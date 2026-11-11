//! Keyboard shortcuts are off in the product: key combinations do nothing, and the option
//! `shortcuts=on` brings them back (the other test files turn it on to exercise the commands).

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use karnoxpdf_engine::Edit;
use karnoxpdf_ui_egui::KarnoxPDFApp;

fn fixture() -> Vec<u8> {
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 200 300] >>".to_string(),
        "<< /Type /Page /Parent 2 0 R >>".to_string(),
    ];
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

fn harness(shortcuts: bool) -> Harness<'static, KarnoxPDFApp> {
    let mut h = Harness::builder().with_size(egui::vec2(1200.0, 800.0)).build_eframe(move |_cc| {
        let mut app = KarnoxPDFApp::new();
        if shortcuts {
            app.set_option("shortcuts", "on").unwrap();
        }
        app.open_bytes("doc.pdf", None, fixture()).expect("opens");
        app
    });
    h.run_steps(4);
    h
}

#[test]
fn key_combinations_do_nothing_by_default() {
    let mut h = harness(false);
    assert!(!h.state().keyboard_shortcuts);
    // Preferences (Ctrl+,), full screen (Ctrl+L), undo (Ctrl+Z) and Document properties (Ctrl+D).
    for (modifiers, key) in [(Modifiers::COMMAND, Key::Comma), (Modifiers::COMMAND, Key::L), (Modifiers::COMMAND, Key::D)] {
        h.key_press_modifiers(modifiers, key);
        h.run_steps(3);
        assert!(h.state().dialog.is_none(), "{key:?} opened a dialog");
        assert!(!h.state().full_screen, "{key:?} changed the window");
    }
    assert!(h.state_mut().apply_edit(Edit::RotatePages { pages: vec![0], degrees: 90 }));
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run_steps(3);
    let s = h.state();
    assert_eq!(s.session.get(s.views[0].id).unwrap().can_undo(), Some("Rotate page"), "Ctrl+Z did not undo");
}

#[test]
fn the_option_turns_them_back_on() {
    let mut h = harness(true);
    h.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
    h.run_steps(3);
    assert!(h.state().dialog.is_some(), "Ctrl+, opens Preferences when shortcuts are on");
}
