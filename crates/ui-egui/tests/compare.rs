//! Compare files in the real shell (egui_kittest): the dialog, the Compare panel, marks and
//! the report.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use karnoxpdf_engine::Session;
use karnoxpdf_ui_egui::{KarnoxPDFApp, RightPanel};

#[test]
fn compare_two_versions() {
    let s = Session::new();
    let v1 = s.create_from_text("t", "Delivery within five days. Returns accepted.").unwrap().to_vec();
    let v2 = s.create_from_text("t", "Delivery within three days. Returns accepted for a week.").unwrap().to_vec();
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(move |_cc| {
        let mut app = KarnoxPDFApp::new();
        app.open_bytes("v1.pdf", None, v1.clone()).unwrap();
        app.open_bytes("v2.pdf", None, v2.clone()).unwrap();
        app
    });
    h.run_steps(4);
    assert!(h.state_mut().execute("doc.compare"));
    h.run_steps(2);
    h.get_by_label("Compare Files");
    h.get_by_label("Compare").click();
    h.run_steps(3);
    assert_eq!(h.state().right, Some(RightPanel::Compare));
    assert_eq!(h.state().toast.clone().unwrap().0, "2 differences found");
    h.get_by_label("2 Replaced");
    h.get_by_label("\"five\" → \"three\"").click();
    h.run_steps(2);
    assert_eq!(h.state().compare.as_ref().unwrap().selected, Some(0));
    let id = h.state().active_ids().unwrap().1;
    let i = h.state().active_ids().unwrap().0;
    assert_eq!(h.state().views[i].compare_marks.len(), 2);
    h.get_by_label("Mark as comments").click();
    h.run_steps(3);
    assert_eq!(h.state().session.get(id).unwrap().info.annotations.len(), 2);
    h.get_by_label("Report…").click();
    h.run_steps(3);
    assert_eq!(h.state().session.get(h.state().active_ids().unwrap().1).unwrap().name, "Compare Report.pdf");
}

/// Writes a screenshot of the Compare panel when KARNOXPDF_SHOT is set (for review).
#[test]
fn compare_panel_screenshot() {
    let Ok(out) = std::env::var("KARNOXPDF_SHOT") else { return };
    let s = Session::new();
    let v1 = s.create_from_text("t", "Delivery within five days. Returns accepted.").unwrap().to_vec();
    let v2 = s.create_from_text("t", "Delivery within three days. Returns accepted for a week.").unwrap().to_vec();
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(move |_cc| {
        let mut app = KarnoxPDFApp::new();
        app.open_bytes("v1.pdf", None, v1.clone()).unwrap();
        app.open_bytes("v2.pdf", None, v2.clone()).unwrap();
        app
    });
    h.run_steps(4);
    h.state_mut().compare_old = h.state().views.first().map(|v| v.id);
    h.state_mut().run_compare();
    for _ in 0..20 {
        h.run_steps(2);
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    h.render().unwrap().save(out).unwrap();
}

#[test]
fn pdfa_dialog_verifies_and_converts() {
    let doc = Session::new().create_from_text("t", "Keep forever").unwrap().to_vec();
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(move |_cc| {
        let mut app = KarnoxPDFApp::new();
        app.open_bytes("keep.pdf", None, doc.clone()).unwrap();
        app
    });
    h.run_steps(4);
    assert!(h.state_mut().execute("standards.pdfa"));
    h.run_steps(2);
    h.get_by_label("Declared conformance: none");
    h.get_by_label("Verify").click();
    h.run_steps(3);
    h.get_by_label("The document has no XMP metadata");
    h.get_by_label("Save as PDF/A").click();
    h.run_steps(3);
    h.get_by_label("Declared conformance: PDF/A-2b");
    let issues = h.state().pdfa.issues.clone().unwrap();
    assert!(issues.iter().all(|i| !i.fixable), "{issues:?}");
    let id = h.state().active_ids().unwrap().1;
    assert_eq!(h.state().session.get(id).unwrap().can_undo(), Some("Save as PDF/A-2b"));
}

#[test]
fn export_to_word_html_and_rtf() {
    let doc = Session::new().create_from_text("t", "Exported words").unwrap().to_vec();
    let dir = std::env::temp_dir().join(format!("karnoxpdf-office-ui-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut app = KarnoxPDFApp::new();
    app.open_bytes("e.pdf", None, doc).unwrap();
    for ext in ["docx", "html", "rtf", "pptx", "xlsx"] {
        let out = dir.join(format!("e.{ext}"));
        app.save_override = Some(out.to_string_lossy().into_owned());
        assert!(app.execute(&format!("export.{ext}")));
        assert!(std::fs::metadata(&out).unwrap().len() > 40, "{ext}");
    }
    assert!(std::fs::read_to_string(dir.join("e.html")).unwrap().contains("Exported words"));
    assert!(std::fs::read(dir.join("e.pptx")).unwrap().starts_with(b"PK"));
    assert!(std::fs::read(dir.join("e.xlsx")).unwrap().starts_with(b"PK"));
}

#[test]
fn home_pdf_to_word_card_converts_and_saves_docx() {
    use egui::accesskit::Role;

    let source = Session::new().create_from_text("invoice", "Invoice 42\nAmount due: 100").unwrap().to_vec();
    let dir = std::env::temp_dir().join(format!("karnoxpdf-word-workspace-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let output = dir.join("converted.docx");
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(move |_cc| {
        let mut app = KarnoxPDFApp::new();
        app.save_override = Some(output.to_string_lossy().into_owned());
        app
    });
    h.run_steps(3);
    h.get_by_label("PDF to Word").click();
    h.run_steps(2);
    assert!(h.state().word_workspace_active);
    h.state_mut().use_files(karnoxpdf_ui_egui::FilePurpose::Word, vec![("invoice.pdf".into(), source.clone())]);
    h.run_steps(2);
    h.get_by_role_and_label(Role::Button, "Convert to Word").click();
    h.run_steps(2);
    assert!(h.state().word_result.as_ref().is_some_and(|result| result.bytes.starts_with(b"PK")));
    if let Ok(path) = std::env::var("KARNOXPDF_WORD_SHOT") {
        h.render().unwrap().save(path).unwrap();
    }
    h.get_by_role_and_label(Role::Button, "Save Word document").click();
    h.run_steps(2);
    let saved = std::fs::read(dir.join("converted.docx")).unwrap();
    assert_eq!(saved.as_slice(), h.state().word_result.as_ref().unwrap().bytes.as_slice());
    assert_eq!(h.state().word_source.as_ref().unwrap().bytes.as_slice(), source.as_slice());
    h.get_by_role_and_label(Role::Button, "All tools").click();
    h.run_steps(2);
    assert!(h.state().word_workspace_open);
    assert!(!h.state().word_workspace_active);
    std::fs::remove_file(dir.join("converted.docx")).unwrap();
    let _ = std::fs::remove_dir(dir);
}

#[test]
fn home_powerpoint_and_excel_cards_convert_and_save() {
    use egui::accesskit::Role;
    let source = Session::new().create_from_text("invoice", "Invoice 42\nAmount due: 100").unwrap().to_vec();
    let dir = std::env::temp_dir().join(format!("karnoxpdf-office-workspaces-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(|_cc| KarnoxPDFApp::new());
    h.run_steps(3);
    for (title, purpose, index, convert, save, ext) in [
        ("PDF to PowerPoint", karnoxpdf_ui_egui::FilePurpose::PowerPoint, 0, "Convert to PowerPoint", "Save PowerPoint presentation", "pptx"),
        ("PDF to Excel", karnoxpdf_ui_egui::FilePurpose::Excel, 1, "Convert to Excel", "Save Excel workbook", "xlsx"),
    ] {
        h.get_by_label(title).click();
        h.run_steps(2);
        assert!(h.state().office_workspaces[index].active);
        h.state_mut().use_files(purpose, vec![("invoice.pdf".into(), source.clone())]);
        h.run_steps(2);
        h.get_by_role_and_label(Role::Button, convert).click();
        h.run_steps(2);
        assert!(h.state().office_workspaces[index].result.as_ref().is_some_and(|bytes| bytes.starts_with(b"PK")));
        if let Ok(dir) = std::env::var("KARNOXPDF_OFFICE_SHOTS") {
            h.render().unwrap().save(std::path::Path::new(&dir).join(format!("result-{ext}.png"))).unwrap();
        }
        let out = dir.join(format!("converted.{ext}"));
        h.state_mut().save_override = Some(out.to_string_lossy().into_owned());
        h.get_by_role_and_label(Role::Button, save).click();
        h.run_steps(2);
        assert_eq!(
            std::fs::read(&out).unwrap_or_else(|e| panic!("{ext}: {e}; toast={:?}", h.state().toast)).as_slice(),
            h.state().office_workspaces[index].result.as_ref().unwrap().as_slice()
        );
        h.get_by_role_and_label(Role::Button, "All tools").click();
        h.run_steps(2);
        assert!(h.state().office_workspaces[index].open);
        assert!(!h.state().office_workspaces[index].active);
        std::fs::remove_file(out).unwrap();
    }
    let _ = std::fs::remove_dir(dir);
}

#[test]
fn home_pdf_to_jpg_card_converts_and_saves_pages() {
    use egui::accesskit::Role;
    let source = Session::new().create_from_text("invoice", "Invoice 42\nAmount due: 100").unwrap().to_vec();
    let dir =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target").join(format!("karnoxpdf-jpg-workspace-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(|_cc| KarnoxPDFApp::new());
    h.run_steps(3);
    h.get_by_label("PDF to JPG").click();
    h.run_steps(2);
    assert!(h.state().jpg_workspace.active);
    h.state_mut().use_files(karnoxpdf_ui_egui::FilePurpose::Jpg, vec![("invoice.pdf".into(), source.clone())]);
    h.run_steps(2);
    h.get_by_role_and_label(Role::Button, "Convert to JPG").click();
    h.run_steps(3);
    assert_eq!(h.state().jpg_workspace.result.len(), 1);
    assert!(h.state().jpg_workspace.result[0].bytes.starts_with(&[0xff, 0xd8, 0xff]));
    if let Ok(path) = std::env::var("KARNOXPDF_JPG_SHOT") {
        h.state_mut().set_option("language", "pt-br").unwrap();
        h.run_steps(2);
        h.render().unwrap().save(path).unwrap();
        h.state_mut().set_option("language", "en").unwrap();
        h.run_steps(2);
    }
    h.state_mut().export_dir_override = Some(dir.to_string_lossy().into_owned());
    h.get_by_role_and_label(Role::Button, "Save JPG images").click();
    h.run_steps(2);
    let saved = std::fs::read(dir.join("invoice_page_1.jpg")).unwrap_or_else(|error| {
        panic!(
            "saved JPG missing: {error}; toast={:?}; files={:?}",
            h.state().toast,
            std::fs::read_dir(&dir).ok().map(|entries| entries.filter_map(Result::ok).map(|entry| entry.file_name()).collect::<Vec<_>>())
        )
    });
    assert_eq!(saved.as_slice(), h.state().jpg_workspace.result[0].bytes.as_slice());
    assert_eq!(h.state().jpg_workspace.source.as_ref().unwrap().bytes.as_slice(), source.as_slice());
    h.get_by_role_and_label(Role::Button, "All tools").click();
    h.run_steps(2);
    assert!(h.state().jpg_workspace.open);
    assert!(!h.state().jpg_workspace.active);
    std::fs::remove_file(dir.join("invoice_page_1.jpg")).unwrap();
    let _ = std::fs::remove_dir(dir);
}

#[test]
fn home_word_to_pdf_card_converts_and_saves() {
    use egui::accesskit::Role;
    let source = karnoxpdf_export::docx(
        &[karnoxpdf_export::Page {
            width: 612.0,
            height: 792.0,
            blocks: vec![karnoxpdf_export::Block {
                text: "Document for conversion".into(),
                rect: [72.0, 690.0, 400.0, 715.0],
                size: 12.0,
                bold: false,
                italic: false,
            }],
            images: vec![],
        }],
        "Document",
    );
    let dir = std::env::temp_dir().join(format!("karnoxpdf-docx-workspace-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(|_cc| KarnoxPDFApp::new());
    h.run_steps(3);
    h.get_by_label("Word to PDF").click();
    h.run_steps(2);
    assert!(h.state().office_workspaces[2].active);
    h.state_mut().use_files(karnoxpdf_ui_egui::FilePurpose::WordToPdf, vec![("document.docx".into(), source)]);
    h.run_steps(2);
    h.get_by_role_and_label(Role::Button, "Convert to PDF").click();
    h.run_steps(2);
    for _ in 0..150 {
        if !h.state().office_workspaces[2].converting {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        h.run_steps(1);
    }
    assert!(!h.state().office_workspaces[2].converting, "Word conversion did not finish");
    assert!(h.state().office_workspaces[2].result.as_ref().is_some_and(|bytes| bytes.starts_with(b"%PDF-")));
    if let Ok(path) = std::env::var("KARNOXPDF_DOCX_SHOT") {
        h.render().unwrap().save(path).unwrap();
    }
    let output = dir.join("converted.pdf");
    h.state_mut().save_override = Some(output.to_string_lossy().into_owned());
    h.get_by_role_and_label(Role::Button, "Save PDF").click();
    h.run_steps(2);
    assert!(std::fs::read(&output).unwrap().starts_with(b"%PDF-"));
    h.state_mut().set_option("language", "pt-br").unwrap();
    h.run_steps(2);
    let _ = h.get_by_role_and_label(Role::Button, "Salvar PDF");
    if let Ok(path) = std::env::var("KARNOXPDF_DOCX_PT_SHOT") {
        h.render().unwrap().save(path).unwrap();
    }
    std::fs::remove_file(output).unwrap();
    let _ = std::fs::remove_dir(dir);
}

#[test]
fn home_powerpoint_to_pdf_card_converts_and_saves() {
    use egui::accesskit::Role;
    let source = karnoxpdf_export::pptx(&[karnoxpdf_export::Page {
        width: 720.0,
        height: 540.0,
        blocks: vec![karnoxpdf_export::Block {
            text: "Slide for conversion".into(),
            rect: [72.0, 400.0, 400.0, 440.0],
            size: 32.0,
            bold: true,
            italic: false,
        }],
        images: vec![],
    }])
    .unwrap();
    let dir = std::env::temp_dir().join(format!("karnoxpdf-pptx-workspace-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(|_cc| KarnoxPDFApp::new());
    h.run_steps(3);
    h.get_by_label("PowerPoint to PDF").click();
    h.run_steps(2);
    assert!(h.state().office_workspaces[3].active);
    // A legacy .ppt (or anything that is not a PPTX package) is refused before conversion.
    h.state_mut().use_files(karnoxpdf_ui_egui::FilePurpose::PowerPointToPdf, vec![("legacy.ppt".into(), source.clone())]);
    h.run_steps(2);
    assert!(h.state().office_workspaces[3].source.is_none());
    h.state_mut().use_files(karnoxpdf_ui_egui::FilePurpose::PowerPointToPdf, vec![("deck.pptx".into(), source)]);
    h.run_steps(2);
    h.get_by_role_and_label(Role::Button, "Convert to PDF").click();
    h.run_steps(2);
    for _ in 0..150 {
        if !h.state().office_workspaces[3].converting {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        h.run_steps(1);
    }
    assert!(!h.state().office_workspaces[3].converting, "PowerPoint conversion did not finish");
    assert!(h.state().office_workspaces[3].result.as_ref().is_some_and(|bytes| bytes.starts_with(b"%PDF-")));
    assert!(h.state().office_workspaces[3].notes.is_empty());
    if let Ok(path) = std::env::var("KARNOXPDF_PPTX_SHOT") {
        h.render().unwrap().save(path).unwrap();
    }
    let output = dir.join("converted.pdf");
    h.state_mut().save_override = Some(output.to_string_lossy().into_owned());
    h.get_by_role_and_label(Role::Button, "Save PDF").click();
    h.run_steps(2);
    assert!(std::fs::read(&output).unwrap().starts_with(b"%PDF-"));
    h.state_mut().set_option("language", "pt-br").unwrap();
    h.run_steps(2);
    let _ = h.get_by_role_and_label(Role::Button, "Salvar PDF");
    if let Ok(path) = std::env::var("KARNOXPDF_PPTX_PT_SHOT") {
        h.render().unwrap().save(path).unwrap();
    }
    std::fs::remove_file(output).unwrap();
    let _ = std::fs::remove_dir(dir);
}

#[test]
fn home_excel_to_pdf_card_converts_and_saves() {
    use egui::accesskit::Role;
    let block = |text: &str, x: f64, y: f64| karnoxpdf_export::Block {
        text: text.into(),
        rect: [x, y, x + 110.0, y + 12.0],
        size: 11.0,
        bold: false,
        italic: false,
    };
    let source = karnoxpdf_export::xlsx(&[karnoxpdf_export::Page {
        width: 612.0,
        height: 792.0,
        blocks: vec![block("Company", 72.0, 620.0), block("Amount", 242.0, 620.0), block("Design", 72.0, 590.0), block("120", 242.0, 590.0)],
        images: vec![],
    }])
    .unwrap();
    let dir = std::env::temp_dir().join(format!("karnoxpdf-xlsx-workspace-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(|_cc| KarnoxPDFApp::new());
    h.run_steps(3);
    h.get_by_label("Excel to PDF").click();
    h.run_steps(2);
    assert!(h.state().office_workspaces[4].active);
    h.state_mut().use_files(karnoxpdf_ui_egui::FilePurpose::ExcelToPdf, vec![("legacy.xls".into(), source.clone())]);
    h.run_steps(2);
    assert!(h.state().office_workspaces[4].source.is_none(), "a legacy .xls is refused");
    h.state_mut().use_files(karnoxpdf_ui_egui::FilePurpose::ExcelToPdf, vec![("book.xlsx".into(), source)]);
    h.run_steps(2);
    h.get_by_label("Landscape").click();
    h.run_steps(2);
    assert_eq!(h.state().office_orientation, karnoxpdf_engine::SheetOrientation::Landscape);
    h.get_by_role_and_label(Role::Button, "Convert to PDF").click();
    h.run_steps(2);
    for _ in 0..150 {
        if !h.state().office_workspaces[4].converting {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        h.run_steps(1);
    }
    let result = h.state().office_workspaces[4].result.clone().expect("the workbook converted");
    assert!(result.starts_with(b"%PDF-"));
    // Landscape: the page is wider than tall.
    let info = karnoxpdf_render::inspect(result.clone(), None).unwrap();
    assert!(info.pages[0].width > info.pages[0].height, "{:?}", (info.pages[0].width, info.pages[0].height));
    if let Ok(path) = std::env::var("KARNOXPDF_XLSX_SHOT") {
        h.render().unwrap().save(path).unwrap();
    }
    let output = dir.join("converted.pdf");
    h.state_mut().save_override = Some(output.to_string_lossy().into_owned());
    h.get_by_role_and_label(Role::Button, "Save PDF").click();
    h.run_steps(2);
    assert!(std::fs::read(&output).unwrap().starts_with(b"%PDF-"));
    h.state_mut().set_option("language", "pt-br").unwrap();
    h.run_steps(2);
    let _ = h.get_by_role_and_label(Role::Button, "Salvar PDF");
    if let Ok(path) = std::env::var("KARNOXPDF_XLSX_PT_SHOT") {
        h.render().unwrap().save(path).unwrap();
    }
    std::fs::remove_file(output).unwrap();
    let _ = std::fs::remove_dir(dir);
}
