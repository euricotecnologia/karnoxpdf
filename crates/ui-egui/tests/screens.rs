//! Every tool screen opens with a document and draws without panicking; with
//! `KARNOXPDF_SCREENS_DIR=dir` each one is also saved as a PNG for a visual check.

use egui_kittest::Harness;
use karnoxpdf_ui_egui::{FilePurpose, KarnoxPDFApp};

/// A three-page document with a proper xref table.
fn pdf() -> Vec<u8> {
    let n = 3;
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

fn png() -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, 30, 20);
        enc.set_color(png::ColorType::Rgb);
        let mut w = enc.write_header().unwrap();
        w.write_image_data(&[90u8; 1800]).unwrap();
    }
    out
}

#[test]
fn tool_screens_with_a_document_draw_and_can_be_photographed() {
    let dir = std::env::var("KARNOXPDF_SCREENS_DIR").ok().map(std::path::PathBuf::from);
    if let Some(dir) = &dir {
        std::fs::create_dir_all(dir).unwrap();
    }
    let screens: Vec<(&str, FilePurpose, &str, Vec<u8>)> = vec![
        ("sign", FilePurpose::SignPdf, "source.pdf", pdf()),
        ("watermark", FilePurpose::WatermarkPdf, "source.pdf", pdf()),
        ("rotate", FilePurpose::RotatePdf, "source.pdf", pdf()),
        ("repair", FilePurpose::RepairPdf, "source.pdf", pdf()),
        ("protect", FilePurpose::ProtectPdf, "source.pdf", pdf()),
        ("unlock", FilePurpose::UnlockPdf, "source.pdf", pdf()),
        ("image-to-pdf", FilePurpose::ImageToPdf, "photo.png", png()),
    ];
    for (name, purpose, file, bytes) in screens {
        let mut h = Harness::builder().with_size(egui::vec2(1400.0, 1000.0)).build_eframe(|_cc| KarnoxPDFApp::new());
        h.run_steps(3);
        h.state_mut().use_files(purpose, vec![(file.into(), bytes)]);
        h.run_steps(4);
        if let Some(dir) = &dir {
            h.render().expect("render").save(dir.join(format!("{name}.png"))).expect("save screenshot");
        }
    }
}

#[test]
fn html_screen_draws_and_can_be_photographed() {
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 1500.0)).build_eframe(|_cc| KarnoxPDFApp::new());
    h.run_steps(3);
    h.state_mut().open_html_workspace();
    h.run_steps(4);
    if let Ok(dir) = std::env::var("KARNOXPDF_SCREENS_DIR") {
        std::fs::create_dir_all(&dir).unwrap();
        h.render().expect("render").save(std::path::Path::new(&dir).join("html.png")).expect("save screenshot");
    }
}

#[test]
fn home_about_and_splash_draw_and_can_be_photographed() {
    let dir = std::env::var("KARNOXPDF_SCREENS_DIR").ok().map(std::path::PathBuf::from);
    if let Some(dir) = &dir {
        std::fs::create_dir_all(dir).unwrap();
    }
    let shot = |h: &mut Harness<'static, KarnoxPDFApp>, name: &str| {
        if let Some(dir) = &dir {
            h.render().expect("render").save(dir.join(name)).expect("save screenshot");
        }
    };
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(|_cc| {
        let mut app = KarnoxPDFApp::new();
        app.set_option("language", "pt-br").unwrap();
        app
    });
    h.run_steps(4);
    shot(&mut h, "home.png");
    h.state_mut().dialog = Some(karnoxpdf_ui_egui::Dialog::About);
    h.run_steps(4);
    shot(&mut h, "about.png");
    let mut h = Harness::builder().with_size(egui::vec2(840.0, 500.0)).build_eframe(|_cc| {
        let mut app = KarnoxPDFApp::new();
        app.enable_splash();
        app
    });
    h.run_steps(3);
    shot(&mut h, "splash.png");
}

#[test]
fn print_dialog_draws_in_portuguese_and_can_be_photographed() {
    let mut h = Harness::builder().with_size(egui::vec2(1300.0, 920.0)).build_eframe(|_cc| {
        let mut app = KarnoxPDFApp::new();
        app.set_option("language", "pt-br").unwrap();
        app.open_bytes("source.pdf", None, pdf()).expect("opens");
        app
    });
    h.run_steps(4);
    assert!(h.state_mut().execute("print.dialog"));
    h.run_steps(4);
    // Never print for real from a test: pick "Save as PDF" and leave.
    h.state_mut().print_draft.printer = None;
    h.run_steps(2);
    if let Ok(dir) = std::env::var("KARNOXPDF_SCREENS_DIR") {
        std::fs::create_dir_all(&dir).unwrap();
        h.render().expect("render").save(std::path::Path::new(&dir).join("print-pt.png")).expect("save screenshot");
    }
}
