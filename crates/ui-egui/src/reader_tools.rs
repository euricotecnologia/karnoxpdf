//! The reading screen's tool bar: one button per tool that works on a whole document. A click opens
//! that tool's screen with the PDF on screen already attached.

use egui::{Align2, Color32, CornerRadius, Rect, Response, Sense};

use crate::{KarnoxPDFApp, LeftPanel, Mode};

/// A tool offered on the reading screen's bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReaderTool {
    Merge,
    Split,
    Compress,
    ToWord,
    ToPowerPoint,
    ToExcel,
    Edit,
    ToJpg,
    Sign,
    Watermark,
    Rotate,
    Protect,
    PageNumbers,
    DeletePages,
    Translate,
}

impl ReaderTool {
    /// Left to right on the bar.
    pub(crate) const ALL: [ReaderTool; 15] = [
        ReaderTool::Merge,
        ReaderTool::Split,
        ReaderTool::Compress,
        ReaderTool::ToWord,
        ReaderTool::ToPowerPoint,
        ReaderTool::ToExcel,
        ReaderTool::Edit,
        ReaderTool::ToJpg,
        ReaderTool::Sign,
        ReaderTool::Watermark,
        ReaderTool::Rotate,
        ReaderTool::Protect,
        ReaderTool::PageNumbers,
        ReaderTool::DeletePages,
        ReaderTool::Translate,
    ];

    /// The tool's name, as on the Home card (translated through the catalogue).
    pub(crate) fn label(self) -> &'static str {
        match self {
            ReaderTool::Merge => tl!("Merge PDF"),
            ReaderTool::Split => tl!("Split PDF"),
            ReaderTool::Compress => tl!("Compress PDF"),
            ReaderTool::ToWord => tl!("PDF to Word"),
            ReaderTool::ToPowerPoint => tl!("PDF to PowerPoint"),
            ReaderTool::ToExcel => tl!("PDF to Excel"),
            ReaderTool::Edit => tl!("Edit PDF"),
            ReaderTool::ToJpg => tl!("PDF to JPG"),
            ReaderTool::Sign => tl!("Sign PDF"),
            ReaderTool::Watermark => tl!("Watermark"),
            ReaderTool::Rotate => tl!("Rotate PDF"),
            ReaderTool::Protect => tl!("Protect PDF"),
            ReaderTool::PageNumbers => tl!("Page Numbers"),
            ReaderTool::DeletePages => tl!("Delete Pages"),
            ReaderTool::Translate => tl!("Translate PDF"),
        }
    }

    pub(crate) fn icon(self) -> &'static str {
        match self {
            ReaderTool::Merge => "files",
            ReaderTool::Split => "scissors",
            ReaderTool::Compress => "file-down",
            ReaderTool::ToWord => "file-text",
            ReaderTool::ToPowerPoint => "presentation",
            ReaderTool::ToExcel => "file-spreadsheet",
            ReaderTool::Edit => "file-pen-line",
            ReaderTool::ToJpg => "image",
            ReaderTool::Sign => "signature",
            ReaderTool::Watermark => "stamp",
            ReaderTool::Rotate => "rotate-cw",
            ReaderTool::Protect => "shield-check",
            ReaderTool::PageNumbers => "hash",
            ReaderTool::DeletePages => "trash-2",
            ReaderTool::Translate => "languages",
        }
    }
}

/// Draw the sidebar on the left of the reading screen; a click opens the tool with the open document.
pub fn sidebar(app: &mut KarnoxPDFApp, ui: &mut egui::Ui) {
    let t = crate::theme::Tokens::get(ui.ctx());
    let mut picked = None;
    let mut more = false;
    let sidebar_w = 200.0;
    egui::Panel::left("reader_tools_sidebar")
        .exact_size(sidebar_w)
        .frame(egui::Frame::NONE.fill(t.panel).inner_margin(egui::Margin::symmetric(10, 10)).stroke(egui::Stroke::new(1.0, t.divider)))
        .show(ui, |ui| {
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new(tl!("Tools").to_uppercase()).font(crate::theme::semibold(11.0)).color(t.text_faint).extra_letter_spacing(0.5),
            );
            ui.add_space(6.0);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 6.0);
                for tool in ReaderTool::ALL {
                    if reader_sidebar_button(ui, &t, tool.icon(), tool.label()).clicked() {
                        picked = Some(tool);
                    }
                }
                if reader_sidebar_button(ui, &t, "layout-grid", tl!("More tools")).clicked() {
                    more = true;
                }
                ui.add_space(8.0);
            });
        });
    if more {
        // The full list of tools, as a side panel that closes with its ✕.
        let showing = app.left_open && app.left == LeftPanel::AllTools;
        app.left = LeftPanel::AllTools;
        app.left_open = !showing;
        if app.left_open {
            app.all_tools_expanded = true;
        }
    }
    if let Some(tool) = picked {
        app.open_reader_tool(tool);
    }
}

/// Kept for backwards compatibility with any existing caller.
#[allow(dead_code)]
pub fn bar(app: &mut KarnoxPDFApp, ui: &mut egui::Ui) {
    sidebar(app, ui);
}

fn reader_sidebar_button(ui: &mut egui::Ui, t: &crate::theme::Tokens, icon: &str, label: &str) -> Response {
    let font = crate::theme::semibold(12.5);
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, 36.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label));
    let fill = if resp.is_pointer_button_down_on() {
        Color32::from_rgb(0x0E, 0x4B, 0xBA)
    } else if resp.hovered() {
        Color32::from_rgb(0x15, 0x54, 0xC8)
    } else {
        t.accent
    };
    ui.painter().rect_filled(rect, CornerRadius::same(8), fill);
    crate::icons::paint(ui, Rect::from_center_size(rect.left_center() + egui::vec2(18.0, 0.0), egui::vec2(16.0, 16.0)), icon, 15.0, Color32::WHITE);
    ui.painter().text(rect.left_center() + egui::vec2(34.0, 0.0), Align2::LEFT_CENTER, label, font, Color32::WHITE);
    resp
}

impl KarnoxPDFApp {
    /// Open `tool` with the document on screen attached to it.
    pub(crate) fn open_reader_tool(&mut self, tool: ReaderTool) {
        if tool == ReaderTool::Edit {
            self.run_command("edit.edit_text");
            return;
        }
        let Some((_, id)) = self.active_ids() else { return };
        let Some((name, bytes)) = self.session.get(id).map(|doc| (doc.name.clone(), (*doc.bytes).clone())) else { return };
        use crate::files::FilePurpose;
        let purpose = match tool {
            ReaderTool::Merge => FilePurpose::Combine,
            ReaderTool::Split => FilePurpose::Split,
            ReaderTool::Compress => FilePurpose::Compress,
            ReaderTool::ToWord => FilePurpose::Word,
            ReaderTool::ToPowerPoint => FilePurpose::PowerPoint,
            ReaderTool::ToExcel => FilePurpose::Excel,
            ReaderTool::ToJpg => FilePurpose::Jpg,
            ReaderTool::Sign => FilePurpose::SignPdf,
            ReaderTool::Watermark => FilePurpose::WatermarkPdf,
            ReaderTool::Rotate => FilePurpose::RotatePdf,
            ReaderTool::Protect => FilePurpose::ProtectPdf,
            ReaderTool::PageNumbers => FilePurpose::PageNumbersPdf,
            ReaderTool::DeletePages => return self.stage_delete_pages_source(name, bytes, None),
            ReaderTool::Translate => return self.stage_translate_source(name, bytes, None),
            ReaderTool::Edit => return,
        };
        self.use_files(purpose, vec![(name, bytes)]);
    }

    /// Whether the reading screen's tool bar is shown: a document in the plain reading workspace.
    pub(crate) fn shows_reader_tools(&self) -> bool {
        self.active.is_some() && self.mode == Mode::AllTools && self.reader_tools_shown && !self.full_screen
    }
}
