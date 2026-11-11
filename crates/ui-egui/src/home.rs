//! Home screen: modern PDF resource cards grid, quick open, and recent files.

use egui::{Align, Align2, Color32, CornerRadius, Layout, Pos2, Rect, Sense, Stroke, pos2, vec2};

use crate::theme::{self, Tokens};
use crate::{KarnoxPDFApp, chrome, i18n, icons, panels::human_size};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum HomeTab {
    #[default]
    AllTools,
    RecentFiles,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum HomeViewMode {
    #[default]
    Grid,
    List,
}

enum BadgeKind {
    Icon(&'static str),
    Office(&'static str),
    OfficePage(&'static str),
    #[allow(dead_code)]
    Text(&'static str),
}

enum CardAction {
    #[allow(dead_code)]
    Command(&'static str),
    CombineWorkspace,
    SplitWorkspace,
    CompressWorkspace,
    WordWorkspace,
    PowerPointWorkspace,
    ExcelWorkspace,
    WordToPdfWorkspace,
    PowerPointToPdfWorkspace,
    ExcelToPdfWorkspace,
    JpgWorkspace,
    ImageToPdfWorkspace,
    SignPdfWorkspace,
    WatermarkWorkspace,
    RotateWorkspace,
    HtmlWorkspace,
    UnlockWorkspace,
    ProtectWorkspace,
    RepairWorkspace,
    PageNumbersWorkspace,
    DeletePagesWorkspace,
    TranslateWorkspace,
    EditWorkspace,
}

struct ToolCard {
    title: &'static str,
    description: &'static str,
    badge_color: Color32,
    badge: BadgeKind,
    action: CardAction,
}

const CARDS: &[ToolCard] = &[
    ToolCard {
        title: "Merge PDF",
        description: "Combine PDF files in any order. Fast and easy.",
        badge_color: Color32::from_rgb(0xFF, 0x2D, 0x37),
        badge: BadgeKind::Icon("files"),
        action: CardAction::CombineWorkspace,
    },
    ToolCard {
        title: "Split PDF",
        description: "Select a page range, extract one page, or save each page as a separate PDF.",
        badge_color: Color32::from_rgb(0xFF, 0x6E, 0x00),
        badge: BadgeKind::Icon("scissors"),
        action: CardAction::SplitWorkspace,
    },
    ToolCard {
        title: "Compress PDF",
        description: "Reduce PDF file size while keeping the best possible quality.",
        badge_color: Color32::from_rgb(0x00, 0xB5, 0x50),
        badge: BadgeKind::Icon("minimize"),
        action: CardAction::CompressWorkspace,
    },
    ToolCard {
        title: "PDF to Word",
        description: "Convert PDF files to editable Word DOCX documents.",
        badge_color: Color32::from_rgb(0x00, 0x78, 0xD4),
        badge: BadgeKind::Office("W"),
        action: CardAction::WordWorkspace,
    },
    ToolCard {
        title: "PDF to PowerPoint",
        description: "Convert PDF files to editable PowerPoint PPTX presentations.",
        badge_color: Color32::from_rgb(0xEA, 0x4B, 0x24),
        badge: BadgeKind::Office("P"),
        action: CardAction::PowerPointWorkspace,
    },
    ToolCard {
        title: "PDF to Excel",
        description: "Extract data from PDFs into Excel spreadsheets in seconds.",
        badge_color: Color32::from_rgb(0x0E, 0x7A, 0x3D),
        badge: BadgeKind::Office("X"),
        action: CardAction::ExcelWorkspace,
    },
    ToolCard {
        title: "Word to PDF",
        description: "Convert Word DOCX documents to PDF locally, with text and tables.",
        badge_color: Color32::from_rgb(0x00, 0x78, 0xD4),
        badge: BadgeKind::OfficePage(""),
        action: CardAction::WordToPdfWorkspace,
    },
    ToolCard {
        title: "PowerPoint to PDF",
        description: "Convert PowerPoint PPTX presentations to PDF locally, with text, shapes, pictures and tables.",
        badge_color: Color32::from_rgb(0xEA, 0x4B, 0x24),
        badge: BadgeKind::OfficePage("P"),
        action: CardAction::PowerPointToPdfWorkspace,
    },
    ToolCard {
        title: "Excel to PDF",
        description: "Convert Excel XLSX workbooks to PDF locally. Choose portrait or landscape orientation.",
        badge_color: Color32::from_rgb(0x0E, 0x7A, 0x3D),
        badge: BadgeKind::OfficePage("X"),
        action: CardAction::ExcelToPdfWorkspace,
    },
    ToolCard {
        title: "Edit PDF",
        description: "Add text, images, shapes, and freehand annotations to a PDF. Adjust their size, font, and color.",
        badge_color: Color32::from_rgb(0x9B, 0x4B, 0xEA),
        badge: BadgeKind::Icon("pencil"),
        action: CardAction::EditWorkspace,
    },
    ToolCard {
        title: "PDF to JPG",
        description: "Extract images from a PDF or convert each page to a JPG file.",
        badge_color: Color32::from_rgb(0xF5, 0x9E, 0x0B),
        badge: BadgeKind::Icon("image"),
        action: CardAction::JpgWorkspace,
    },
    ToolCard {
        title: "Image to PDF",
        description: "Convert PNG, JPG, TIFF, GIF, BMP and more to PDF. Add several images and set their order.",
        badge_color: Color32::from_rgb(0xEC, 0x48, 0x99),
        badge: BadgeKind::Icon("image-plus"),
        action: CardAction::ImageToPdfWorkspace,
    },
    ToolCard {
        title: "Sign PDF",
        description: "Sign a PDF or request electronic signatures from others.",
        badge_color: Color32::from_rgb(0x22, 0x24, 0x7A),
        badge: BadgeKind::Icon("signature"),
        action: CardAction::SignPdfWorkspace,
    },
    ToolCard {
        title: "Watermark",
        description: "Place an image or text watermark on a PDF. Adjust position, opacity, and typography.",
        badge_color: Color32::from_rgb(0x00, 0xA5, 0xB5),
        badge: BadgeKind::Icon("droplet"),
        action: CardAction::WatermarkWorkspace,
    },
    ToolCard {
        title: "Rotate PDF",
        description: "Rotate one PDF or several PDF files at once.",
        badge_color: Color32::from_rgb(0x8B, 0x4B, 0xEA),
        badge: BadgeKind::Icon("rotate-cw"),
        action: CardAction::RotateWorkspace,
    },
    ToolCard {
        title: "HTML to PDF",
        description: "Convert HTML web pages to PDF by pasting a URL.",
        badge_color: Color32::from_rgb(0xF5, 0x9E, 0x0B),
        badge: BadgeKind::Icon("code-xml"),
        action: CardAction::HtmlWorkspace,
    },
    ToolCard {
        title: "Unlock PDF",
        description: "Remove a PDF's security password so you can use the file.",
        badge_color: Color32::from_rgb(0x00, 0x8C, 0xE5),
        badge: BadgeKind::Icon("lock-open"),
        action: CardAction::UnlockWorkspace,
    },
    ToolCard {
        title: "Protect PDF",
        description: "Protect a PDF with a password to prevent unauthorized access.",
        badge_color: Color32::from_rgb(0x11, 0x2F, 0x53),
        badge: BadgeKind::Icon("shield-check"),
        action: CardAction::ProtectWorkspace,
    },
    ToolCard {
        title: "Repair PDF",
        description: "Recover damaged or corrupted PDF files.",
        badge_color: Color32::from_rgb(0xE5, 0x22, 0x22),
        badge: BadgeKind::Icon("wrench"),
        action: CardAction::RepairWorkspace,
    },
    ToolCard {
        title: "Page Numbers",
        description: "Add page numbers to PDF documents. Choose position, formatting, and typography.",
        badge_color: Color32::from_rgb(0x00, 0xB0, 0x92),
        badge: BadgeKind::Icon("hash"),
        action: CardAction::PageNumbersWorkspace,
    },
    ToolCard {
        title: "Delete Pages",
        description: "Remove unwanted pages from your PDF document and save a new file.",
        badge_color: Color32::from_rgb(0xE8, 0x31, 0x5B),
        badge: BadgeKind::Icon("trash-2"),
        action: CardAction::DeletePagesWorkspace,
    },
    ToolCard {
        title: "Translate PDF",
        description: "Translate PDF document text into multiple languages and generate a new PDF.",
        badge_color: Color32::from_rgb(0x1A, 0x73, 0xE8),
        badge: BadgeKind::Icon("languages"),
        action: CardAction::TranslateWorkspace,
    },
];

fn handle_card_click(app: &mut KarnoxPDFApp, action: &CardAction) {
    match action {
        CardAction::CombineWorkspace => app.open_combine_workspace(),
        CardAction::SplitWorkspace => app.open_split_workspace(),
        CardAction::CompressWorkspace => app.open_compress_workspace(),
        CardAction::WordWorkspace => app.open_word_workspace(),
        CardAction::PowerPointWorkspace => app.open_office_workspace(crate::office_ui::OfficeKind::PowerPoint),
        CardAction::ExcelWorkspace => app.open_office_workspace(crate::office_ui::OfficeKind::Excel),
        CardAction::WordToPdfWorkspace => app.open_office_workspace(crate::office_ui::OfficeKind::WordToPdf),
        CardAction::PowerPointToPdfWorkspace => app.open_office_workspace(crate::office_ui::OfficeKind::PowerPointToPdf),
        CardAction::ExcelToPdfWorkspace => app.open_office_workspace(crate::office_ui::OfficeKind::ExcelToPdf),
        CardAction::JpgWorkspace => app.open_jpg_workspace(),
        CardAction::ImageToPdfWorkspace => app.open_image_to_pdf_workspace(),
        CardAction::SignPdfWorkspace => app.open_sign_pdf_workspace(),
        CardAction::WatermarkWorkspace => app.open_watermark_workspace(),
        CardAction::RotateWorkspace => app.open_rotate_workspace(),
        CardAction::HtmlWorkspace => app.open_html_workspace(),
        CardAction::UnlockWorkspace => app.open_unlock_workspace(),
        CardAction::ProtectWorkspace => app.open_protect_workspace(),
        CardAction::RepairWorkspace => app.open_repair_workspace(),
        CardAction::PageNumbersWorkspace => app.open_page_numbers_workspace(),
        CardAction::DeletePagesWorkspace => app.open_delete_pages_workspace(),
        CardAction::TranslateWorkspace => app.open_translate_workspace(),
        CardAction::EditWorkspace => app.open_edit_dialog(),
        CardAction::Command(cmd) => {
            if app.active.is_none() {
                if matches!(*cmd, "page.combine" | "create.file" | "create.images") {
                    app.execute(cmd);
                } else if !app.views.is_empty() {
                    app.active = Some(0);
                    app.execute(cmd);
                } else {
                    app.open_dialog();
                }
            } else {
                app.execute(cmd);
            }
        }
    }
}

/// Paints the 3D document stack with soft shadows, folded corner, red PDF tag, and floating cyan diamonds.
fn paint_pdf_stack_3d(painter: &egui::Painter, center: Pos2, scale: f32) {
    let rotate = |cx: f32, cy: f32, dx: f32, dy: f32, angle: f32| {
        let (sin, cos) = angle.sin_cos();
        pos2(cx + dx * cos - dy * sin, cy + dx * sin + dy * cos)
    };

    let (bw, bh) = (60.0 * scale, 78.0 * scale);
    let s_off = vec2(3.0 * scale, 5.0 * scale);

    // 1. Back sheet (-8 deg tilt)
    let back_angle = -0.14_f32;
    let back_center = center + vec2(-10.0 * scale, -5.0 * scale);
    let shadow_pts = vec![
        rotate(back_center.x + s_off.x, back_center.y + s_off.y, -bw * 0.5, -bh * 0.5, back_angle),
        rotate(back_center.x + s_off.x, back_center.y + s_off.y, bw * 0.5, -bh * 0.5, back_angle),
        rotate(back_center.x + s_off.x, back_center.y + s_off.y, bw * 0.5, bh * 0.5, back_angle),
        rotate(back_center.x + s_off.x, back_center.y + s_off.y, -bw * 0.5, bh * 0.5, back_angle),
    ];
    painter.add(egui::epaint::Shape::convex_polygon(shadow_pts, Color32::from_rgba_unmultiplied(20, 60, 120, 25), Stroke::NONE));

    let back_pts = vec![
        rotate(back_center.x, back_center.y, -bw * 0.5, -bh * 0.5, back_angle),
        rotate(back_center.x, back_center.y, bw * 0.5, -bh * 0.5, back_angle),
        rotate(back_center.x, back_center.y, bw * 0.5, bh * 0.5, back_angle),
        rotate(back_center.x, back_center.y, -bw * 0.5, bh * 0.5, back_angle),
    ];
    painter.add(egui::epaint::Shape::convex_polygon(
        back_pts,
        Color32::from_rgb(0xF1, 0xF5, 0xF9),
        Stroke::new(1.0, Color32::from_rgb(0xE2, 0xE8, 0xF0)),
    ));

    // 2. Middle sheet (-2 deg tilt)
    let mid_angle = -0.04_f32;
    let mid_center = center + vec2(-3.0 * scale, 1.0 * scale);
    let mid_shadow_pts = vec![
        rotate(mid_center.x + s_off.x, mid_center.y + s_off.y, -bw * 0.5, -bh * 0.5, mid_angle),
        rotate(mid_center.x + s_off.x, mid_center.y + s_off.y, bw * 0.5, -bh * 0.5, mid_angle),
        rotate(mid_center.x + s_off.x, mid_center.y + s_off.y, bw * 0.5, bh * 0.5, mid_angle),
        rotate(mid_center.x + s_off.x, mid_center.y + s_off.y, -bw * 0.5, bh * 0.5, mid_angle),
    ];
    painter.add(egui::epaint::Shape::convex_polygon(mid_shadow_pts, Color32::from_rgba_unmultiplied(20, 60, 120, 30), Stroke::NONE));

    let mid_pts = vec![
        rotate(mid_center.x, mid_center.y, -bw * 0.5, -bh * 0.5, mid_angle),
        rotate(mid_center.x, mid_center.y, bw * 0.5, -bh * 0.5, mid_angle),
        rotate(mid_center.x, mid_center.y, bw * 0.5, bh * 0.5, mid_angle),
        rotate(mid_center.x, mid_center.y, -bw * 0.5, bh * 0.5, mid_angle),
    ];
    painter.add(egui::epaint::Shape::convex_polygon(
        mid_pts,
        Color32::from_rgb(0xF8, 0xFA, 0xFC),
        Stroke::new(1.0, Color32::from_rgb(0xDB, 0xE4, 0xEE)),
    ));

    // 3. Front sheet (+4 deg tilt)
    let front_angle = 0.07_f32;
    let front_center = center + vec2(6.0 * scale, 5.0 * scale);
    let front_shadow_pts = vec![
        rotate(front_center.x + s_off.x, front_center.y + s_off.y, -bw * 0.5, -bh * 0.5, front_angle),
        rotate(front_center.x + s_off.x, front_center.y + s_off.y, bw * 0.5, -bh * 0.5, front_angle),
        rotate(front_center.x + s_off.x, front_center.y + s_off.y, bw * 0.5, bh * 0.5, front_angle),
        rotate(front_center.x + s_off.x, front_center.y + s_off.y, -bw * 0.5, bh * 0.5, front_angle),
    ];
    painter.add(egui::epaint::Shape::convex_polygon(front_shadow_pts, Color32::from_rgba_unmultiplied(20, 60, 120, 40), Stroke::NONE));

    let front_pts = vec![
        rotate(front_center.x, front_center.y, -bw * 0.5, -bh * 0.5, front_angle),
        rotate(front_center.x, front_center.y, bw * 0.5, -bh * 0.5, front_angle),
        rotate(front_center.x, front_center.y, bw * 0.5, bh * 0.5, front_angle),
        rotate(front_center.x, front_center.y, -bw * 0.5, bh * 0.5, front_angle),
    ];
    painter.add(egui::epaint::Shape::convex_polygon(front_pts, Color32::WHITE, Stroke::new(1.0, Color32::from_rgb(0xCB, 0xD5, 0xE1))));

    // Folded top-right corner on front sheet
    let fold_size = 11.0 * scale;
    let fold_top = rotate(front_center.x, front_center.y, bw * 0.5 - fold_size, -bh * 0.5, front_angle);
    let fold_right = rotate(front_center.x, front_center.y, bw * 0.5, -bh * 0.5 + fold_size, front_angle);
    let fold_inner = rotate(front_center.x, front_center.y, bw * 0.5 - fold_size, -bh * 0.5 + fold_size, front_angle);
    painter.add(egui::epaint::Shape::convex_polygon(
        vec![fold_top, fold_inner, fold_right],
        Color32::from_rgb(0xEA, 0xEE, 0xF3),
        Stroke::new(1.0, Color32::from_rgb(0xCB, 0xD5, 0xE1)),
    ));

    // Content lines on front sheet
    for line_idx in 0..4 {
        let line_y = (-bh * 0.28) + (line_idx as f32) * (11.0 * scale);
        let line_w = if line_idx == 0 {
            bw * 0.40
        } else if line_idx == 3 {
            bw * 0.35
        } else {
            bw * 0.65
        };
        let lp1 = rotate(front_center.x, front_center.y, -bw * 0.35, line_y, front_angle);
        let lp2 = rotate(front_center.x, front_center.y, -bw * 0.35 + line_w, line_y, front_angle);
        painter.line_segment([lp1, lp2], Stroke::new(2.5 * scale, Color32::from_rgb(0xE2, 0xE8, 0xF0)));
    }

    // 4. Red "PDF" Badge / Tag
    let badge_angle = -0.16_f32;
    let (bdw, bdh) = (44.0 * scale, 23.0 * scale);
    let bc = front_center + vec2(15.0 * scale, 18.0 * scale);

    let badge_shadow_pts = vec![
        rotate(bc.x + 2.0 * scale, bc.y + 3.0 * scale, -bdw * 0.5, -bdh * 0.5, badge_angle),
        rotate(bc.x + 2.0 * scale, bc.y + 3.0 * scale, bdw * 0.5, -bdh * 0.5, badge_angle),
        rotate(bc.x + 2.0 * scale, bc.y + 3.0 * scale, bdw * 0.5, bdh * 0.5, badge_angle),
        rotate(bc.x + 2.0 * scale, bc.y + 3.0 * scale, -bdw * 0.5, bdh * 0.5, badge_angle),
    ];
    painter.add(egui::epaint::Shape::convex_polygon(badge_shadow_pts, Color32::from_rgba_unmultiplied(220, 20, 30, 60), Stroke::NONE));

    let badge_pts = vec![
        rotate(bc.x, bc.y, -bdw * 0.5, -bdh * 0.5, badge_angle),
        rotate(bc.x, bc.y, bdw * 0.5, -bdh * 0.5, badge_angle),
        rotate(bc.x, bc.y, bdw * 0.5, bdh * 0.5, badge_angle),
        rotate(bc.x, bc.y, -bdw * 0.5, bdh * 0.5, badge_angle),
    ];
    painter.add(egui::epaint::Shape::convex_polygon(badge_pts, Color32::from_rgb(0xFF, 0x1E, 0x27), Stroke::NONE));

    let text_pos = bc + vec2(0.0, -0.5 * scale);
    painter.text(text_pos, Align2::CENTER_CENTER, "PDF", theme::bold(12.5 * scale), Color32::WHITE);

    // 5. Floating decorative 3D diamonds
    let diamond = |p: Pos2, size: f32, color: Color32| {
        let pts = vec![p + vec2(0.0, -size), p + vec2(size, 0.0), p + vec2(0.0, size), p + vec2(-size, 0.0)];
        painter.add(egui::epaint::Shape::convex_polygon(pts, color, Stroke::NONE));
    };

    diamond(center + vec2(-36.0 * scale, -30.0 * scale), 6.5 * scale, Color32::from_rgb(0x60, 0xA5, 0xFA));
    diamond(center + vec2(42.0 * scale, -38.0 * scale), 8.5 * scale, Color32::from_rgb(0x38, 0xBD, 0xF8));
    diamond(center + vec2(-38.0 * scale, 34.0 * scale), 5.5 * scale, Color32::from_rgb(0x93, 0xC5, 0xFD));
    diamond(center + vec2(52.0 * scale, 28.0 * scale), 7.5 * scale, Color32::from_rgb(0x25, 0x63, 0xEB));
}

fn render_sidebar(app: &mut KarnoxPDFApp, ui: &mut egui::Ui, t: &Tokens, height: f32) {
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

    // Logo section
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        ui.add_space(18.0);
        if let Some(logo) = crate::dialogs::about_logo(ui.ctx()) {
            let logo_w = 175.0;
            let logo_h = (logo_w * crate::dialogs::ABOUT_LOGO_RATIO).round();
            ui.add(egui::Image::new(&logo).fit_to_exact_size(vec2(logo_w, logo_h)));
        } else {
            let (icon_rect, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
            ui.painter().rect_filled(icon_rect, CornerRadius::same(9), Color32::from_rgb(0xFF, 0x1E, 0x27));
            ui.painter().text(icon_rect.center(), Align2::CENTER_CENTER, "K", theme::bold(21.0), Color32::WHITE);

            ui.add_space(10.0);
            let text_color = if t.dark() { Color32::WHITE } else { Color32::from_rgb(0x11, 0x18, 0x27) };
            ui.label(egui::RichText::new("Karnox").font(theme::bold(20.0)).color(text_color));
            ui.label(egui::RichText::new("PDF").font(theme::bold(20.0)).color(Color32::from_rgb(0xFF, 0x1E, 0x27)));
        }
    });

    ui.add_space(20.0);

    // Navigation buttons
    let nav_width = ui.available_width() - 36.0;

    // 1. Todas as ferramentas
    let all_tools_active = app.home_tab == HomeTab::AllTools;
    ui.horizontal(|ui| {
        ui.add_space(18.0);
        let (rect, resp) = ui.allocate_exact_size(vec2(nav_width, 42.0), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, all_tools_active, tl!("All tools")));
        if resp.clicked() {
            app.home_tab = HomeTab::AllTools;
        }

        let bg = if all_tools_active {
            Color32::from_rgb(0x00, 0x78, 0xFF)
        } else if resp.hovered() {
            if t.dark() { t.hover } else { Color32::from_rgb(0xF1, 0xF5, 0xF9) }
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, CornerRadius::same(10), bg);

        let icon_tint = if all_tools_active { Color32::WHITE } else { Color32::from_rgb(0x64, 0x74, 0x8B) };
        let text_tint = if all_tools_active || t.dark() { Color32::WHITE } else { Color32::from_rgb(0x33, 0x41, 0x55) };

        icons::paint(ui, Rect::from_min_size(rect.min + vec2(14.0, 12.0), vec2(18.0, 18.0)), "layout-grid", 18.0, icon_tint);
        ui.painter().text(rect.min + vec2(40.0, 21.0), Align2::LEFT_CENTER, tl!("All tools"), theme::semibold(13.5), text_tint);
    });

    ui.add_space(6.0);

    // 2. Arquivos recentes
    let recent_active = app.home_tab == HomeTab::RecentFiles;
    ui.horizontal(|ui| {
        ui.add_space(18.0);
        let (rect, resp) = ui.allocate_exact_size(vec2(nav_width, 42.0), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, recent_active, tl!("Recent files")));
        if resp.clicked() {
            app.home_tab = HomeTab::RecentFiles;
        }

        let bg = if recent_active {
            Color32::from_rgb(0x00, 0x78, 0xFF)
        } else if resp.hovered() {
            if t.dark() { t.hover } else { Color32::from_rgb(0xF1, 0xF5, 0xF9) }
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, CornerRadius::same(10), bg);

        let icon_tint = if recent_active { Color32::WHITE } else { Color32::from_rgb(0x64, 0x74, 0x8B) };
        let text_tint = if recent_active || t.dark() { Color32::WHITE } else { Color32::from_rgb(0x33, 0x41, 0x55) };

        icons::paint(ui, Rect::from_min_size(rect.min + vec2(14.0, 12.0), vec2(18.0, 18.0)), "clock", 18.0, icon_tint);
        ui.painter().text(rect.min + vec2(40.0, 21.0), Align2::LEFT_CENTER, tl!("Recent files"), theme::medium(13.5), text_tint);
    });

    // Bottom Promo Card
    let promo_h = 190.0;
    let space_to_bottom = (height - ui.cursor().min.y - promo_h - 18.0).max(10.0);
    ui.add_space(space_to_bottom);

    ui.horizontal(|ui| {
        ui.add_space(16.0);
        let promo_w = ui.available_width() - 16.0;
        let (promo_rect, _) = ui.allocate_exact_size(vec2(promo_w, promo_h), Sense::hover());

        let promo_bg = if t.dark() { Color32::from_rgb(0x1E, 0x24, 0x33) } else { Color32::from_rgb(0xEE, 0xF6, 0xFF) };
        let promo_stroke = Stroke::new(1.0, if t.dark() { Color32::from_rgb(0x2D, 0x37, 0x48) } else { Color32::from_rgb(0xDB, 0xEA, 0xFE) });
        ui.painter().rect(promo_rect, CornerRadius::same(16), promo_bg, promo_stroke, egui::StrokeKind::Inside);

        // 3D document stack
        let stack_center = pos2(promo_rect.center().x, promo_rect.min.y + 56.0);
        paint_pdf_stack_3d(ui.painter(), stack_center, 0.62);

        // Promo text
        let promo_text = tl!("Everything you need to work with PDFs in one place.");
        let text_w = promo_w - 24.0;
        let font = theme::medium(12.0);
        let text_color = if t.dark() { Color32::from_rgb(0x94, 0xA3, 0xB8) } else { Color32::from_rgb(0x47, 0x55, 0x69) };
        let galley = ui.fonts_mut(|f| f.layout(promo_text.to_string(), font, text_color, text_w));
        let text_pos = pos2(promo_rect.center().x - galley.size().x * 0.5, promo_rect.min.y + 124.0);
        ui.painter().galley(text_pos, galley, text_color);
    });
}

fn render_main_content(app: &mut KarnoxPDFApp, ui: &mut egui::Ui, t: &Tokens) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let margin_x = 36.0;
        let full_w = ui.available_width();

        if app.is_home_screen_only() {
            // 1. Top bar with Theme toggle and Language selector (top right)
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.add_space(margin_x);
                let top_bar_w = full_w - margin_x * 2.0;
                ui.allocate_ui_with_layout(vec2(top_bar_w, 36.0), Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;

                    let btn_stroke =
                        Stroke::new(1.0, if t.dark() { Color32::from_rgb(0x3B, 0x42, 0x54) } else { Color32::from_rgb(0xDA, 0xE2, 0xED) });

                    // Sobre (About button) - positioned to the right of "Idioma"
                    let (about_rect, about_resp) = ui.allocate_exact_size(vec2(86.0, 32.0), Sense::click());
                    about_resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tl!("About")));
                    if about_resp.clicked() {
                        app.dialog = Some(crate::Dialog::About);
                    }
                    let abtn_bg = if about_resp.hovered() {
                        if t.dark() { t.hover } else { Color32::from_rgb(0xF1, 0xF5, 0xF9) }
                    } else if t.dark() {
                        Color32::from_rgb(0x26, 0x2A, 0x36)
                    } else {
                        Color32::WHITE
                    };
                    ui.painter().rect(about_rect, CornerRadius::same(8), abtn_bg, btn_stroke, egui::StrokeKind::Inside);
                    icons::paint(ui, Rect::from_center_size(about_rect.left_center() + vec2(16.0, 0.0), vec2(14.0, 14.0)), "info", 14.0, t.text);
                    ui.painter().text(about_rect.left_center() + vec2(30.0, 0.0), Align2::LEFT_CENTER, tl!("About"), theme::medium(13.0), t.text);

                    // Language dropdown
                    let (lang_rect, lang_resp) = ui.allocate_exact_size(vec2(106.0, 32.0), Sense::click());
                    lang_resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tl!("Language")));
                    let btn_bg = if lang_resp.hovered() {
                        if t.dark() { t.hover } else { Color32::from_rgb(0xF1, 0xF5, 0xF9) }
                    } else if t.dark() {
                        Color32::from_rgb(0x26, 0x2A, 0x36)
                    } else {
                        Color32::WHITE
                    };
                    ui.painter().rect(lang_rect, CornerRadius::same(8), btn_bg, btn_stroke, egui::StrokeKind::Inside);
                    ui.painter().text(lang_rect.left_center() + vec2(14.0, 0.0), Align2::LEFT_CENTER, tl!("Language"), theme::medium(13.0), t.text);
                    icons::paint(
                        ui,
                        Rect::from_center_size(lang_rect.right_center() - vec2(14.0, 0.0), vec2(12.0, 12.0)),
                        "chevron-down",
                        12.0,
                        t.text_muted,
                    );
                    egui::Popup::menu(&lang_resp).show(|ui| chrome::language_menu(app, ui));

                    // Moon / Sun theme button
                    let (theme_rect, theme_resp) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::click());
                    theme_resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tl!("System themes")));
                    let t_bg = if theme_resp.hovered() {
                        if t.dark() { t.hover } else { Color32::from_rgb(0xF1, 0xF5, 0xF9) }
                    } else if t.dark() {
                        Color32::from_rgb(0x26, 0x2A, 0x36)
                    } else {
                        Color32::WHITE
                    };
                    ui.painter().rect(theme_rect, CornerRadius::same(8), t_bg, btn_stroke, egui::StrokeKind::Inside);
                    let theme_icon = if t.dark() { "sun" } else { "moon" };
                    icons::paint(ui, Rect::from_center_size(theme_rect.center(), vec2(16.0, 16.0)), theme_icon, 16.0, t.text);
                    egui::Popup::menu(&theme_resp).show(|ui| chrome::theme_menu(app, ui));

                    // Notification button (32x32 icon button, always present on main screen)
                    let (notif_rect, notif_resp) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::click());
                    notif_resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tl!("Notification")));
                    let has_res = app.has_open_resources();
                    let nbtn_bg = if has_res {
                        if notif_resp.hovered() { Color32::from_rgb(0x15, 0x54, 0xC8) } else { t.accent }
                    } else if notif_resp.hovered() {
                        if t.dark() { t.hover } else { Color32::from_rgb(0xF1, 0xF5, 0xF9) }
                    } else if t.dark() {
                        Color32::from_rgb(0x26, 0x2A, 0x36)
                    } else {
                        Color32::WHITE
                    };
                    let stroke = if has_res { Stroke::NONE } else { btn_stroke };
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

            ui.add_space(8.0);
        } else {
            ui.add_space(16.0);
        }

        // 2. Header area
        ui.horizontal(|ui| {
            ui.add_space(margin_x);
            let content_w = full_w - margin_x * 2.0;

            // Decorative background curves and 3D PDF illustration on the top-right
            let (header_rect, _) = ui.allocate_exact_size(vec2(content_w, 110.0), Sense::hover());

            // Soft curved pastel sky-blue gradient aura
            let aura_center = pos2(header_rect.max.x - 70.0, header_rect.min.y + 40.0);
            if !t.dark() {
                ui.painter().circle_filled(aura_center + vec2(10.0, 10.0), 120.0, Color32::from_rgba_unmultiplied(0xE0, 0xF2, 0xFE, 120));
                ui.painter().circle_filled(aura_center + vec2(30.0, 25.0), 85.0, Color32::from_rgba_unmultiplied(0xBA, 0xE6, 0xFD, 100));
            } else {
                ui.painter().circle_filled(aura_center + vec2(10.0, 10.0), 110.0, Color32::from_rgba_unmultiplied(0x1E, 0x2D, 0x45, 90));
            }

            // 3D paper stack illustration
            paint_pdf_stack_3d(ui.painter(), pos2(header_rect.max.x - 75.0, header_rect.min.y + 55.0), 1.05);

            // Left header text: Category, Title, Subtitle
            let text_x = header_rect.min.x;
            let text_y = header_rect.min.y + 4.0;

            // Category tag
            let category_text = match app.home_tab {
                HomeTab::AllTools => tl!("PDF tools").to_uppercase(),
                HomeTab::RecentFiles => tl!("Recent files").to_uppercase(),
            };
            ui.painter().text(pos2(text_x, text_y), Align2::LEFT_TOP, category_text, theme::bold(11.0), Color32::from_rgb(0x94, 0xA3, 0xB8));

            // Main heading
            let heading_text = match app.home_tab {
                HomeTab::AllTools => tl!("All tools"),
                HomeTab::RecentFiles => tl!("Recent files"),
            };
            let heading_color = if t.dark() { Color32::WHITE } else { Color32::from_rgb(0x0F, 0x17, 0x2A) };
            ui.painter().text(pos2(text_x, text_y + 16.0), Align2::LEFT_TOP, heading_text, theme::bold(28.0), heading_color);

            // Subtitle
            let subtitle_text = match app.home_tab {
                HomeTab::AllTools => tl!("Edit, convert, organize and work with PDFs quickly, simply and securely."),
                HomeTab::RecentFiles => tl!("Files you open in KarnoxPDF appear here. Drop a PDF anywhere to open it."),
            };
            ui.painter().text(
                pos2(text_x, text_y + 54.0),
                Align2::LEFT_TOP,
                subtitle_text,
                theme::regular(13.0),
                Color32::from_rgb(0x64, 0x74, 0x8B),
            );

            // Grid / List toggle pill (only in AllTools)
            if app.home_tab == HomeTab::AllTools {
                let toggle_w = 68.0;
                let toggle_h = 32.0;
                let toggle_rect = Rect::from_min_size(pos2(header_rect.max.x - 200.0, header_rect.min.y + 72.0), vec2(toggle_w, toggle_h));
                let toggle_bg = if t.dark() { Color32::from_rgb(0x24, 0x2B, 0x3A) } else { Color32::from_rgb(0xED, 0xF2, 0xF7) };
                let toggle_stroke =
                    Stroke::new(1.0, if t.dark() { Color32::from_rgb(0x3B, 0x45, 0x59) } else { Color32::from_rgb(0xDA, 0xE2, 0xED) });
                ui.painter().rect(toggle_rect, CornerRadius::same(8), toggle_bg, toggle_stroke, egui::StrokeKind::Inside);

                // Grid button
                let grid_active = app.home_view_mode == HomeViewMode::Grid;
                let grid_rect = Rect::from_min_size(toggle_rect.min + vec2(2.0, 2.0), vec2(30.0, 28.0));
                let grid_resp = ui.interact(grid_rect, ui.id().with("btn_view_grid"), Sense::click());
                grid_resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, grid_active, tl!("Grid view")));
                if grid_resp.clicked() {
                    app.home_view_mode = HomeViewMode::Grid;
                }
                if grid_active {
                    ui.painter().rect_filled(grid_rect, CornerRadius::same(6), Color32::from_rgb(0x00, 0x78, 0xFF));
                }
                icons::paint(
                    ui,
                    Rect::from_center_size(grid_rect.center(), vec2(15.0, 15.0)),
                    "layout-grid",
                    15.0,
                    if grid_active { Color32::WHITE } else { Color32::from_rgb(0x64, 0x74, 0x8B) },
                );

                // List button
                let list_active = app.home_view_mode == HomeViewMode::List;
                let list_rect = Rect::from_min_size(toggle_rect.min + vec2(36.0, 2.0), vec2(30.0, 28.0));
                let list_resp = ui.interact(list_rect, ui.id().with("btn_view_list"), Sense::click());
                list_resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, list_active, tl!("List view")));
                if list_resp.clicked() {
                    app.home_view_mode = HomeViewMode::List;
                }
                if list_active {
                    ui.painter().rect_filled(list_rect, CornerRadius::same(6), Color32::from_rgb(0x00, 0x78, 0xFF));
                }
                icons::paint(
                    ui,
                    Rect::from_center_size(list_rect.center(), vec2(15.0, 15.0)),
                    "list",
                    15.0,
                    if list_active { Color32::WHITE } else { Color32::from_rgb(0x64, 0x74, 0x8B) },
                );
            }
        });

        ui.add_space(20.0);

        // 3. Content view: All tools vs Recent files
        match app.home_tab {
            HomeTab::AllTools => {
                ui.horizontal(|ui| {
                    ui.add_space(margin_x);
                    let grid_w = full_w - margin_x * 2.0;

                    ui.allocate_ui_with_layout(vec2(grid_w, ui.available_height()), Layout::top_down(Align::Min), |ui| match app.home_view_mode {
                        HomeViewMode::Grid => render_tools_grid(app, ui, t, grid_w),
                        HomeViewMode::List => render_tools_list(app, ui, t, grid_w),
                    });
                });
            }
            HomeTab::RecentFiles => {
                ui.horizontal(|ui| {
                    ui.add_space(margin_x);
                    let content_w = full_w - margin_x * 2.0;

                    ui.allocate_ui_with_layout(vec2(content_w, ui.available_height()), Layout::top_down(Align::Min), |ui| {
                        render_recent_files_view(app, ui, t, content_w);
                    });
                });
            }
        }

        ui.add_space(32.0);
    });
}

fn render_tools_grid(app: &mut KarnoxPDFApp, ui: &mut egui::Ui, t: &Tokens, available: f32) {
    let gap = 10.0;
    let card_h = 80.0;
    let cols = 3;
    let card_w = ((available - (cols - 1) as f32 * gap) / cols as f32).floor();

    for chunk in CARDS.chunks(3) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for card in chunk {
                let (rect, resp) = ui.allocate_exact_size(vec2(card_w, card_h), Sense::click());
                let title = i18n::t(card.title);
                let description = i18n::t(card.description);
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, title));

                let hovered = resp.hovered();
                let bg = if hovered {
                    if t.dark() { Color32::from_rgb(0x25, 0x2D, 0x3E) } else { Color32::from_rgb(0xFA, 0xFB, 0xFD) }
                } else if t.dark() {
                    Color32::from_rgb(0x1E, 0x24, 0x33)
                } else {
                    Color32::WHITE
                };

                let border_stroke = if hovered {
                    Stroke::new(1.2, card.badge_color)
                } else {
                    Stroke::new(1.0, if t.dark() { Color32::from_rgb(0x33, 0x3E, 0x52) } else { Color32::from_rgb(0xEE, 0xF2, 0xF6) })
                };

                ui.painter().rect(rect, CornerRadius::same(12), bg, border_stroke, egui::StrokeKind::Inside);

                // Badge (left)
                let badge_rect = Rect::from_min_size(rect.min + vec2(14.0, (card_h - 44.0) * 0.5), vec2(44.0, 44.0));
                ui.painter().rect_filled(badge_rect, CornerRadius::same(10), card.badge_color);

                match card.badge {
                    BadgeKind::Icon(icon_name) => {
                        icons::paint(ui, Rect::from_center_size(badge_rect.center(), vec2(22.0, 22.0)), icon_name, 22.0, Color32::WHITE);
                    }
                    BadgeKind::Office(letter) => {
                        let doc_tile = Rect::from_center_size(badge_rect.center(), vec2(24.0, 26.0));
                        ui.painter().rect_filled(doc_tile, CornerRadius::same(4), Color32::WHITE);
                        ui.painter().text(doc_tile.center(), Align2::CENTER_CENTER, letter, theme::bold(14.5), card.badge_color);
                    }
                    BadgeKind::OfficePage(letter) => {
                        let sheet_rect = Rect::from_center_size(badge_rect.center(), vec2(22.0, 26.0));
                        let fold = 6.0;
                        let pts = vec![
                            sheet_rect.left_top(),
                            pos2(sheet_rect.right() - fold, sheet_rect.top()),
                            pos2(sheet_rect.right(), sheet_rect.top() + fold),
                            sheet_rect.right_bottom(),
                            sheet_rect.left_bottom(),
                        ];
                        ui.painter().add(egui::epaint::Shape::convex_polygon(pts, Color32::WHITE, Stroke::NONE));
                        let fold_pts = vec![
                            pos2(sheet_rect.right() - fold, sheet_rect.top()),
                            pos2(sheet_rect.right() - fold, sheet_rect.top() + fold),
                            pos2(sheet_rect.right(), sheet_rect.top() + fold),
                        ];
                        ui.painter().add(egui::epaint::Shape::convex_polygon(
                            fold_pts,
                            Color32::from_rgb(0xEA, 0xEE, 0xF5),
                            Stroke::new(0.5, Color32::from_rgb(0xD0, 0xD7, 0xE2)),
                        ));
                        if letter.is_empty() {
                            for i in 0..3 {
                                let ly = sheet_rect.top() + 8.0 + (i as f32) * 5.0;
                                let lw = if i == 0 {
                                    9.0
                                } else if i == 2 {
                                    10.0
                                } else {
                                    14.0
                                };
                                ui.painter().line_segment(
                                    [pos2(sheet_rect.left() + 4.0, ly), pos2(sheet_rect.left() + 4.0 + lw, ly)],
                                    Stroke::new(1.8, card.badge_color),
                                );
                            }
                        } else {
                            ui.painter().text(
                                sheet_rect.center() + vec2(-0.5, 1.0),
                                Align2::CENTER_CENTER,
                                letter,
                                theme::bold(13.0),
                                card.badge_color,
                            );
                        }
                    }
                    BadgeKind::Text(txt) => {
                        ui.painter().text(badge_rect.center(), Align2::CENTER_CENTER, txt, theme::bold(15.0), Color32::WHITE);
                    }
                }

                // Text (middle)
                let text_x = rect.min.x + 68.0;
                let title_pos = pos2(text_x, rect.min.y + 12.0);
                let title_color = if t.dark() { Color32::WHITE } else { Color32::from_rgb(0x0F, 0x17, 0x2A) };
                ui.painter().text(title_pos, Align2::LEFT_TOP, title, theme::bold(14.0), title_color);

                let desc_max_w = (card_w - 68.0 - 38.0).max(10.0);
                let font_size = 11.5;
                let galley =
                    ui.fonts_mut(|f| f.layout(description.to_string(), theme::regular(font_size), Color32::from_rgb(0x64, 0x74, 0x8B), desc_max_w));
                ui.painter().galley(pos2(text_x, rect.min.y + 31.0), galley, Color32::from_rgb(0x64, 0x74, 0x8B));

                // Action arrow (right circle)
                let arrow_center = pos2(rect.max.x - 22.0, rect.center().y);
                let arrow_bg_alpha = if hovered { 48 } else { 24 };
                let arrow_bg = Color32::from_rgba_unmultiplied(card.badge_color.r(), card.badge_color.g(), card.badge_color.b(), arrow_bg_alpha);
                ui.painter().circle_filled(arrow_center, 13.0, arrow_bg);
                icons::paint(ui, Rect::from_center_size(arrow_center, vec2(13.0, 13.0)), "move-right", 13.0, card.badge_color);

                if resp.clicked() {
                    handle_card_click(app, &card.action);
                }
            }
        });
        ui.add_space(gap);
    }
}

fn render_tools_list(app: &mut KarnoxPDFApp, ui: &mut egui::Ui, t: &Tokens, available: f32) {
    let row_h = 56.0;
    for card in CARDS {
        let (rect, resp) = ui.allocate_exact_size(vec2(available, row_h), Sense::click());
        let title = i18n::t(card.title);
        let description = i18n::t(card.description);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, title));

        let hovered = resp.hovered();
        let bg = if hovered {
            if t.dark() { Color32::from_rgb(0x25, 0x2D, 0x3E) } else { Color32::from_rgb(0xFA, 0xFB, 0xFD) }
        } else if t.dark() {
            Color32::from_rgb(0x1E, 0x24, 0x33)
        } else {
            Color32::WHITE
        };
        let border_stroke = if hovered {
            Stroke::new(1.2, card.badge_color)
        } else {
            Stroke::new(1.0, if t.dark() { Color32::from_rgb(0x33, 0x3E, 0x52) } else { Color32::from_rgb(0xEE, 0xF2, 0xF6) })
        };
        ui.painter().rect(rect, CornerRadius::same(10), bg, border_stroke, egui::StrokeKind::Inside);

        // Badge
        let badge_rect = Rect::from_min_size(rect.min + vec2(12.0, 10.0), vec2(36.0, 36.0));
        ui.painter().rect_filled(badge_rect, CornerRadius::same(8), card.badge_color);

        match card.badge {
            BadgeKind::Icon(icon_name) => {
                icons::paint(ui, Rect::from_center_size(badge_rect.center(), vec2(18.0, 18.0)), icon_name, 18.0, Color32::WHITE);
            }
            BadgeKind::Office(letter) => {
                let doc_tile = Rect::from_center_size(badge_rect.center(), vec2(20.0, 22.0));
                ui.painter().rect_filled(doc_tile, CornerRadius::same(3), Color32::WHITE);
                ui.painter().text(doc_tile.center(), Align2::CENTER_CENTER, letter, theme::bold(12.0), card.badge_color);
            }
            BadgeKind::OfficePage(letter) => {
                let sheet_rect = Rect::from_center_size(badge_rect.center(), vec2(18.0, 22.0));
                let fold = 5.0;
                let pts = vec![
                    sheet_rect.left_top(),
                    pos2(sheet_rect.right() - fold, sheet_rect.top()),
                    pos2(sheet_rect.right(), sheet_rect.top() + fold),
                    sheet_rect.right_bottom(),
                    sheet_rect.left_bottom(),
                ];
                ui.painter().add(egui::epaint::Shape::convex_polygon(pts, Color32::WHITE, Stroke::NONE));
                let fold_pts = vec![
                    pos2(sheet_rect.right() - fold, sheet_rect.top()),
                    pos2(sheet_rect.right() - fold, sheet_rect.top() + fold),
                    pos2(sheet_rect.right(), sheet_rect.top() + fold),
                ];
                ui.painter().add(egui::epaint::Shape::convex_polygon(
                    fold_pts,
                    Color32::from_rgb(0xEA, 0xEE, 0xF5),
                    Stroke::new(0.5, Color32::from_rgb(0xD0, 0xD7, 0xE2)),
                ));
                if letter.is_empty() {
                    for i in 0..3 {
                        let ly = sheet_rect.top() + 6.0 + (i as f32) * 4.0;
                        let lw = if i == 0 {
                            7.0
                        } else if i == 2 {
                            8.0
                        } else {
                            11.0
                        };
                        ui.painter().line_segment(
                            [pos2(sheet_rect.left() + 3.0, ly), pos2(sheet_rect.left() + 3.0 + lw, ly)],
                            Stroke::new(1.5, card.badge_color),
                        );
                    }
                } else {
                    ui.painter().text(sheet_rect.center() + vec2(-0.5, 0.5), Align2::CENTER_CENTER, letter, theme::bold(11.0), card.badge_color);
                }
            }
            BadgeKind::Text(txt) => {
                ui.painter().text(badge_rect.center(), Align2::CENTER_CENTER, txt, theme::bold(13.0), Color32::WHITE);
            }
        }

        // Title and description inline
        let title_color = if t.dark() { Color32::WHITE } else { Color32::from_rgb(0x0F, 0x17, 0x2A) };
        ui.painter().text(rect.min + vec2(60.0, 28.0), Align2::LEFT_CENTER, title, theme::bold(14.0), title_color);

        let desc_x = rect.min.x + 220.0;
        let desc_w = (rect.max.x - desc_x - 50.0).max(10.0);
        let galley = ui.fonts_mut(|f| f.layout(description.to_string(), theme::regular(12.0), Color32::from_rgb(0x64, 0x74, 0x8B), desc_w));
        ui.painter().galley(pos2(desc_x, rect.min.y + 20.0), galley, Color32::from_rgb(0x64, 0x74, 0x8B));

        // Arrow button
        let arrow_center = pos2(rect.max.x - 24.0, rect.center().y);
        let arrow_bg_alpha = if hovered { 48 } else { 24 };
        let arrow_bg = Color32::from_rgba_unmultiplied(card.badge_color.r(), card.badge_color.g(), card.badge_color.b(), arrow_bg_alpha);
        ui.painter().circle_filled(arrow_center, 13.0, arrow_bg);
        icons::paint(ui, Rect::from_center_size(arrow_center, vec2(13.0, 13.0)), "move-right", 13.0, card.badge_color);

        if resp.clicked() {
            handle_card_click(app, &card.action);
        }

        ui.add_space(8.0);
    }
}

fn render_recent_files_view(app: &mut KarnoxPDFApp, ui: &mut egui::Ui, t: &Tokens, available: f32) {
    if app.recent.is_empty() {
        let (rect, _) = ui.allocate_exact_size(vec2(available, 180.0), Sense::hover());
        let bg = if t.dark() { Color32::from_rgb(0x1E, 0x24, 0x33) } else { Color32::WHITE };
        let border = Stroke::new(1.0, if t.dark() { Color32::from_rgb(0x33, 0x3E, 0x52) } else { Color32::from_rgb(0xEE, 0xF2, 0xF6) });
        ui.painter().rect(rect, CornerRadius::same(12), bg, border, egui::StrokeKind::Inside);

        let icon_center = pos2(rect.center().x, rect.min.y + 44.0);
        icons::paint(ui, Rect::from_center_size(icon_center, vec2(36.0, 36.0)), "file-text", 36.0, Color32::from_rgb(0xFF, 0x47, 0x57));

        let msg = tl!("Files you open in KarnoxPDF appear here. Drop a PDF anywhere to open it.");
        ui.painter().text(pos2(rect.center().x, rect.min.y + 88.0), Align2::CENTER_CENTER, msg, theme::medium(13.5), t.text_muted);

        let btn_rect = Rect::from_center_size(pos2(rect.center().x, rect.min.y + 130.0), vec2(140.0, 36.0));
        let btn_resp = ui.interact(btn_rect, ui.id().with("empty_open_pdf"), Sense::click());
        btn_resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tl!("Open a PDF")));
        let btn_fill = if btn_resp.hovered() { Color32::from_rgb(0x1B, 0x63, 0xE0) } else { Color32::from_rgb(0x00, 0x78, 0xFF) };
        ui.painter().rect_filled(btn_rect, CornerRadius::same(8), btn_fill);
        ui.painter().text(btn_rect.center(), Align2::CENTER_CENTER, tl!("Open a PDF"), theme::semibold(13.0), Color32::WHITE);
        if btn_resp.clicked() {
            app.open_dialog();
        }
        return;
    }

    let mut open = None;
    for r in &app.recent {
        let (rect, resp) = ui.allocate_exact_size(vec2(available, 56.0), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &r.name));
        let hovered = resp.hovered();

        let bg = if hovered {
            if t.dark() { Color32::from_rgb(0x25, 0x2D, 0x3E) } else { Color32::from_rgb(0xFA, 0xFB, 0xFD) }
        } else if t.dark() {
            Color32::from_rgb(0x1E, 0x24, 0x33)
        } else {
            Color32::WHITE
        };
        let border_stroke = if hovered {
            Stroke::new(1.2, Color32::from_rgb(0x00, 0x78, 0xFF))
        } else {
            Stroke::new(1.0, if t.dark() { Color32::from_rgb(0x33, 0x3E, 0x52) } else { Color32::from_rgb(0xEE, 0xF2, 0xF6) })
        };
        ui.painter().rect(rect, CornerRadius::same(10), bg, border_stroke, egui::StrokeKind::Inside);

        // PDF file icon
        icons::paint(ui, Rect::from_min_size(rect.min + vec2(14.0, 15.0), vec2(26.0, 26.0)), "file-text", 24.0, Color32::from_rgb(0xFF, 0x1E, 0x27));

        // Name and Path
        let title_color = if t.dark() { Color32::WHITE } else { Color32::from_rgb(0x0F, 0x17, 0x2A) };
        ui.painter().text(rect.min + vec2(48.0, 18.0), Align2::LEFT_TOP, &r.name, theme::bold(13.5), title_color);
        ui.painter().text(rect.min + vec2(48.0, 36.0), Align2::LEFT_TOP, &r.path, theme::regular(11.0), t.text_faint);

        // Page count & size
        ui.painter().text(
            rect.right_center() - vec2(44.0, 0.0),
            Align2::RIGHT_CENTER,
            format!("{}  ·  {}", i18n::trn(i18n::current(), r.pages as u64, "{n} page", "{n} pages"), human_size(r.size)),
            theme::regular(12.0),
            t.text_muted,
        );

        // Arrow button
        let arrow_center = pos2(rect.max.x - 22.0, rect.center().y);
        icons::paint(ui, Rect::from_center_size(arrow_center, vec2(14.0, 14.0)), "move-right", 14.0, Color32::from_rgb(0x00, 0x78, 0xFF));

        if resp.clicked() {
            open = Some(r.path.clone());
        }
        ui.add_space(8.0);
    }

    if let Some(p) = open {
        if let Some(i) = app.views.iter().position(|v| app.session.get(v.id).and_then(|d| d.path.as_deref()) == Some(p.as_str())) {
            app.active = Some(i);
        } else {
            #[cfg(not(target_arch = "wasm32"))]
            app.open_path(&p);
        }
    }
}

pub fn show(app: &mut KarnoxPDFApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let available_height = ui.available_height();
    let sidebar_w = 240.0;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;

        // 1. Left Sidebar
        let sidebar_rect = Rect::from_min_size(ui.cursor().min, vec2(sidebar_w, available_height));
        let sidebar_bg = if t.dark() { t.panel } else { Color32::WHITE };
        let border_stroke = Stroke::new(1.0, if t.dark() { Color32::from_rgb(0x2D, 0x37, 0x48) } else { Color32::from_rgb(0xEB, 0xEE, 0xF2) });
        ui.painter().rect(sidebar_rect, CornerRadius::ZERO, sidebar_bg, border_stroke, egui::StrokeKind::Inside);

        ui.allocate_ui_with_layout(vec2(sidebar_w, available_height), Layout::top_down(Align::Min), |ui| {
            render_sidebar(app, ui, &t, available_height);
        });

        // 2. Main Content Area
        let main_w = ui.available_width();
        let main_bg = if t.dark() { t.pasteboard } else { Color32::from_rgb(0xF8, 0xFA, 0xFC) };
        let main_rect = Rect::from_min_size(ui.cursor().min, vec2(main_w, available_height));
        ui.painter().rect_filled(main_rect, CornerRadius::ZERO, main_bg);

        ui.allocate_ui_with_layout(vec2(main_w, available_height), Layout::top_down(Align::Min), |ui| {
            render_main_content(app, ui, &t);
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_card_is_translated_in_each_supported_language() {
        for language in i18n::Lang::all().filter(|lang| lang.code() != "en") {
            for card in CARDS {
                assert!(i18n::has(language, card.title), "{} title: {}", language.code(), card.title);
                assert!(i18n::has(language, card.description), "{} description: {}", language.code(), card.description);
            }
            assert!(i18n::has(language, "Language"), "{} language label", language.code());
            assert!(i18n::has(language, "System themes"), "{} theme label", language.code());
            assert!(i18n::has(language, "PDF tools"), "{} PDF tools label", language.code());
            assert!(i18n::has(language, "Edit, convert, organize and work with PDFs quickly, simply and securely."), "{} subtitle", language.code());
            assert!(i18n::has(language, "Recent files"), "{} recent files label", language.code());
            assert!(i18n::has(language, "Everything you need to work with PDFs in one place."), "{} promo card", language.code());
            assert!(i18n::has(language, "Grid view"), "{} grid view label", language.code());
            assert!(i18n::has(language, "List view"), "{} list view label", language.code());
        }
    }
}
