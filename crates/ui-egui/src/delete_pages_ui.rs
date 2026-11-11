//! Delete Pages workspace: Select a PDF, pick or type pages to remove
//! via an interactive grid or range input, and save the resulting PDF directly.

use std::collections::BTreeSet;
use std::sync::Arc;

use egui::{Align, Align2, Color32, CornerRadius, Layout, Rect, Sense, Stroke, vec2};
use karnoxpdf_engine::{Edit, Session};
use karnoxpdf_render::PageInfo;

use crate::panels::human_size;
use crate::theme::{self, Tokens};
use crate::{KarnoxPDFApp, icons, widgets};

/// Source PDF document for deleting pages.
#[derive(Clone, Debug)]
pub struct DeletePdfSource {
    pub name: String,
    pub bytes: Arc<Vec<u8>>,
    pub pages: Vec<PageInfo>,
    pub path: Option<String>,
}

/// State for the Delete Pages workspace.
#[derive(Clone, Debug, Default)]
pub struct DeletePagesWorkspace {
    pub open: bool,
    pub active: bool,
    pub source: Option<DeletePdfSource>,

    /// Set of 0-based page indices marked for deletion.
    pub selected_pages: BTreeSet<usize>,

    /// Text representation of page numbers/ranges (1-based for the user).
    pub range_text: String,

    pub error: Option<String>,
    pub success_note: Option<String>,
}

// ---------------------------------------------------------------------------
// Range parsing & formatting helpers
// ---------------------------------------------------------------------------

/// Format a set of 0-based page indices into a clean, human-readable range string
/// such as `"1, 3-5, 8"` (1-based page numbers).
pub fn format_page_ranges(selected: &BTreeSet<usize>) -> String {
    if selected.is_empty() {
        return String::new();
    }

    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for &idx in selected {
        let page = idx + 1;
        if let Some(last) = ranges.last_mut()
            && last.1 + 1 == page
        {
            last.1 = page;
            continue;
        }
        ranges.push((page, page));
    }

    ranges.into_iter().map(|(start, end)| if start == end { format!("{start}") } else { format!("{start}-{end}") }).collect::<Vec<_>>().join(", ")
}

/// Parse a user-entered page range string (e.g. `"1, 3-5, 8"`) into a set of 0-based page indices.
pub fn parse_page_ranges(input: &str, total_pages: usize) -> Result<BTreeSet<usize>, String> {
    let mut result = BTreeSet::new();
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(result);
    }

    for part in trimmed.split([',', ';']).map(str::trim).filter(|s| !s.is_empty()) {
        if let Some((start_str, end_str)) = part.split_once('-') {
            let start = start_str.trim().parse::<usize>().map_err(|_| format!("'{}' {}", start_str.trim(), tl!("is not a valid number")))?;
            let end = end_str.trim().parse::<usize>().map_err(|_| format!("'{}' {}", end_str.trim(), tl!("is not a valid number")))?;

            if start < 1 || start > total_pages {
                return Err(format!("{} {start} (1..{total_pages})", tl!("Page out of range:")));
            }
            if end < 1 || end > total_pages {
                return Err(format!("{} {end} (1..{total_pages})", tl!("Page out of range:")));
            }

            let (s, e) = if start <= end { (start, end) } else { (end, start) };
            for p in s..=e {
                result.insert(p - 1);
            }
        } else {
            let page = part.parse::<usize>().map_err(|_| format!("'{}' {}", part, tl!("is not a valid number")))?;
            if page < 1 || page > total_pages {
                return Err(format!("{} {page} (1..{total_pages})", tl!("Page out of range:")));
            }
            result.insert(page - 1);
        }
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn draw_icon(ui: &mut egui::Ui, name: &str, size: f32, tint: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    icons::paint(ui, rect, name, size, tint);
}

fn feature_pill(ui: &mut egui::Ui, t: &Tokens, icon: &str, label: &str) {
    egui::Frame::NONE
        .fill(t.field)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(egui::Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                draw_icon(ui, icon, 13.0, t.accent);
                ui.label(egui::RichText::new(label).font(theme::medium(11.5)).color(t.text));
            });
        });
}

fn quick_btn(ui: &mut egui::Ui, t: &Tokens, icon: Option<&str>, label: &str) -> egui::Response {
    let font = theme::medium(12.0);
    let text_w = ui.fonts_mut(|f| f.layout_no_wrap(label.to_owned(), font.clone(), t.text).size().x);
    let total_w = (text_w + if icon.is_some() { 28.0 } else { 16.0 }).max(60.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(total_w, 28.0), Sense::click());

    let (bg, stroke, text_color, icon_tint) = if resp.is_pointer_button_down_on() {
        (t.pressed, Stroke::new(1.2, t.accent), t.accent, t.accent)
    } else if resp.hovered() {
        (t.hover, Stroke::new(1.2, t.accent), t.text, t.accent)
    } else {
        (t.field, Stroke::new(1.0, t.border), t.text, t.text_muted)
    };

    ui.painter().rect(rect, CornerRadius::same(6), bg, stroke, egui::StrokeKind::Inside);

    if let Some(ic) = icon {
        icons::paint(ui, Rect::from_center_size(rect.left_center() + vec2(12.0, 0.0), vec2(13.0, 13.0)), ic, 13.0, icon_tint);
        ui.painter().text(rect.left_center() + vec2(22.0, 0.0), Align2::LEFT_CENTER, label, font, text_color);
    } else {
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, font, text_color);
    }

    resp
}

// ---------------------------------------------------------------------------
// Workspace UI
// ---------------------------------------------------------------------------

pub(crate) fn workspace(ui: &mut egui::Ui, app: &mut KarnoxPDFApp, t: &Tokens) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(28.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(940.0);

            // Workspace Header
            ui.heading(egui::RichText::new(tl!("Delete pages")).font(theme::semibold(28.0)).color(t.text));
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(tl!("Remove unwanted pages from PDF documents and generate a new file."))
                    .color(t.text_muted)
                    .font(theme::regular(14.0)),
            );

            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                feature_pill(ui, t, "lock", tl!("100% Local & Private"));
                feature_pill(ui, t, "zap", tl!("Fast & lossless"));
                feature_pill(ui, t, "check-circle-2", tl!("Visual page selection"));
            });

            ui.add_space(22.0);

            // Main Card Container
            egui::Frame::NONE
                .fill(t.card)
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(CornerRadius::same(12))
                .inner_margin(egui::Margin::same(24))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width().min(890.0));

                    if let Some(source) = app.delete_pages_workspace.source.clone() {
                        let total_pages = source.pages.len();

                        // 1. Source Document Summary Bar
                        egui::Frame::NONE
                            .fill(t.field)
                            .stroke(Stroke::new(1.0, t.border))
                            .corner_radius(CornerRadius::same(8))
                            .inner_margin(egui::Margin::symmetric(14, 12))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    draw_icon(ui, "file-text", 20.0, t.accent);
                                    ui.add_space(6.0);
                                    ui.add(egui::Label::new(egui::RichText::new(&source.name).font(theme::medium(14.0)).color(t.text)).truncate());

                                    let pages_str = if total_pages == 1 {
                                        tl!("1 page").to_string()
                                    } else {
                                        crate::i18n::fmt(tl!("{n} pages"), &[("n", &total_pages.to_string())])
                                    };
                                    ui.label(
                                        egui::RichText::new(format!("{} · {}", human_size(source.bytes.len()), pages_str))
                                            .color(t.text_muted)
                                            .font(theme::regular(12.5)),
                                    );

                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if widgets::dialog_action_button(ui, "folder-open", tl!("Change file"), false).clicked() {
                                            app.delete_pages_source_dialog();
                                        }
                                    });
                                });
                            });

                        ui.add_space(18.0);
                        ui.separator();
                        ui.add_space(14.0);

                        // 2. Selection Toolbar & Quick Actions
                        let selected_count = app.delete_pages_workspace.selected_pages.len();
                        let remaining_count = total_pages.saturating_sub(selected_count);

                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(tl!("Pages to delete")).font(theme::semibold(14.5)).color(t.text));

                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                // Status Pill
                                if selected_count == 0 {
                                    egui::Frame::NONE
                                        .fill(t.field)
                                        .stroke(Stroke::new(1.0, t.border))
                                        .corner_radius(CornerRadius::same(12))
                                        .inner_margin(egui::Margin::symmetric(10, 4))
                                        .show(ui, |ui| {
                                            ui.label(egui::RichText::new(tl!("No pages selected")).font(theme::regular(12.0)).color(t.text_muted));
                                        });
                                } else if selected_count >= total_pages {
                                    egui::Frame::NONE
                                        .fill(Color32::from_rgb(0xFE, 0xED, 0xED))
                                        .stroke(Stroke::new(1.2, Color32::from_rgb(0xE7, 0x4C, 0x3C)))
                                        .corner_radius(CornerRadius::same(12))
                                        .inner_margin(egui::Margin::symmetric(10, 4))
                                        .show(ui, |ui| {
                                            ui.label(
                                                egui::RichText::new(tl!("Cannot delete all pages"))
                                                    .font(theme::semibold(12.0))
                                                    .color(Color32::from_rgb(0xC0, 0x39, 0x2B)),
                                            );
                                        });
                                } else {
                                    egui::Frame::NONE
                                        .fill(Color32::from_rgb(0xFE, 0xED, 0xED))
                                        .stroke(Stroke::new(1.2, Color32::from_rgb(0xE7, 0x4C, 0x3C)))
                                        .corner_radius(CornerRadius::same(12))
                                        .inner_margin(egui::Margin::symmetric(10, 4))
                                        .show(ui, |ui| {
                                            let text = format!(
                                                "🗑️ {} · {}",
                                                crate::i18n::fmt(tl!("{n} pages to delete"), &[("n", &selected_count.to_string())]),
                                                crate::i18n::fmt(tl!("{n} remaining"), &[("n", &remaining_count.to_string())])
                                            );
                                            ui.label(
                                                egui::RichText::new(text).font(theme::semibold(12.0)).color(Color32::from_rgb(0xC0, 0x39, 0x2B)),
                                            );
                                        });
                                }
                            });
                        });

                        ui.add_space(10.0);

                        // Quick action buttons
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;

                            if quick_btn(ui, t, Some("x"), tl!("Clear all")).clicked() {
                                app.delete_pages_workspace.selected_pages.clear();
                                app.delete_pages_workspace.range_text.clear();
                                app.delete_pages_workspace.error = None;
                            }

                            if quick_btn(ui, t, None, tl!("Even pages")).clicked() {
                                app.delete_pages_workspace.selected_pages = (0..total_pages).filter(|i| (i + 1) % 2 == 0).collect();
                                app.delete_pages_workspace.range_text = format_page_ranges(&app.delete_pages_workspace.selected_pages);
                                app.delete_pages_workspace.error = None;
                            }

                            if quick_btn(ui, t, None, tl!("Odd pages")).clicked() {
                                app.delete_pages_workspace.selected_pages = (0..total_pages).filter(|i| (i + 1) % 2 == 1).collect();
                                app.delete_pages_workspace.range_text = format_page_ranges(&app.delete_pages_workspace.selected_pages);
                                app.delete_pages_workspace.error = None;
                            }

                            if quick_btn(ui, t, Some("replace"), tl!("Invert selection")).clicked() {
                                app.delete_pages_workspace.selected_pages =
                                    (0..total_pages).filter(|i| !app.delete_pages_workspace.selected_pages.contains(i)).collect();
                                app.delete_pages_workspace.range_text = format_page_ranges(&app.delete_pages_workspace.selected_pages);
                                app.delete_pages_workspace.error = None;
                            }

                            if quick_btn(ui, t, None, tl!("First page")).clicked() {
                                app.delete_pages_workspace.selected_pages.insert(0);
                                app.delete_pages_workspace.range_text = format_page_ranges(&app.delete_pages_workspace.selected_pages);
                                app.delete_pages_workspace.error = None;
                            }

                            if total_pages > 1 && quick_btn(ui, t, None, tl!("Last page")).clicked() {
                                app.delete_pages_workspace.selected_pages.insert(total_pages - 1);
                                app.delete_pages_workspace.range_text = format_page_ranges(&app.delete_pages_workspace.selected_pages);
                                app.delete_pages_workspace.error = None;
                            }
                        });

                        ui.add_space(12.0);

                        // Range text input box
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(tl!("Or type page numbers:")).font(theme::medium(13.0)).color(t.text_muted));
                            ui.add_space(6.0);

                            egui::Frame::NONE
                                .fill(t.field)
                                .stroke(Stroke::new(1.0, t.border))
                                .corner_radius(CornerRadius::same(6))
                                .inner_margin(egui::Margin::symmetric(10, 5))
                                .show(ui, |ui| {
                                    ui.set_min_width(240.0);
                                    let mut current = app.delete_pages_workspace.range_text.clone();
                                    let resp = ui.add(
                                        egui::TextEdit::singleline(&mut current)
                                            .hint_text("ex: 1, 3-5, 8")
                                            .desired_width(240.0)
                                            .font(theme::regular(13.0)),
                                    );
                                    if resp.changed() {
                                        app.delete_pages_workspace.range_text = current.clone();
                                        match parse_page_ranges(&current, total_pages) {
                                            Ok(set) => {
                                                app.delete_pages_workspace.selected_pages = set;
                                                app.delete_pages_workspace.error = None;
                                            }
                                            Err(err) => {
                                                app.delete_pages_workspace.error = Some(err);
                                            }
                                        }
                                    }
                                });
                        });

                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(tl!("Tip: Click individual page cards or type numbers/ranges separated by commas."))
                                .font(theme::regular(11.5))
                                .color(t.text_faint),
                        );

                        ui.add_space(18.0);
                        ui.separator();
                        ui.add_space(16.0);

                        // 3. Interactive Page Grid
                        ui.label(egui::RichText::new(tl!("Click pages to mark for deletion")).font(theme::semibold(14.0)).color(t.text));
                        ui.add_space(12.0);

                        let card_w = 110.0;
                        let card_h = 145.0;

                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = vec2(14.0, 14.0);

                            for idx in 0..total_pages {
                                let is_to_delete = app.delete_pages_workspace.selected_pages.contains(&idx);
                                let page_num = idx + 1;

                                let (rect, resp) = ui.allocate_exact_size(vec2(card_w, card_h), Sense::click());

                                if resp.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }

                                let painter = ui.painter();

                                // Visuals for Card
                                let red_bg = Color32::from_rgb(0xFD, 0xED, 0xEC);
                                let red_border = Color32::from_rgb(0xE7, 0x4C, 0x3C);
                                let red_accent = Color32::from_rgb(0xC0, 0x39, 0x2B);

                                if is_to_delete {
                                    // Drop shadow
                                    painter.rect_filled(rect.translate(vec2(2.0, 2.0)), CornerRadius::same(8), Color32::from_black_alpha(20));
                                    // Card fill & stroke
                                    painter.rect_filled(rect, CornerRadius::same(8), red_bg);
                                    painter.rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.8, red_border), egui::StrokeKind::Inside);

                                    // Top Pill: "Delete"
                                    let pill_rect = Rect::from_min_size(rect.left_top() + vec2(10.0, 8.0), vec2(card_w - 20.0, 22.0));
                                    painter.rect_filled(pill_rect, CornerRadius::same(11), Color32::WHITE);
                                    painter.rect_stroke(pill_rect, CornerRadius::same(11), Stroke::new(1.0, red_border), egui::StrokeKind::Inside);
                                    icons::paint(
                                        ui,
                                        Rect::from_center_size(pill_rect.left_center() + vec2(12.0, 0.0), vec2(11.0, 11.0)),
                                        "trash-2",
                                        11.0,
                                        red_accent,
                                    );
                                    painter.text(
                                        pill_rect.left_center() + vec2(23.0, 0.0),
                                        Align2::LEFT_CENTER,
                                        tl!("Delete"),
                                        theme::semibold(11.0),
                                        red_accent,
                                    );

                                    // Diagonal Strikethrough Line across the page sheet
                                    let p1 = rect.left_top() + vec2(16.0, 42.0);
                                    let p2 = rect.right_bottom() - vec2(16.0, 32.0);
                                    painter.line_segment([p1, p2], Stroke::new(2.0, Color32::from_rgba_unmultiplied(0xE7, 0x4C, 0x3C, 180)));

                                    // Simulated lines (faint)
                                    for y_offset in [52.0, 68.0, 84.0] {
                                        painter.hline(
                                            (rect.left() + 20.0)..=(rect.right() - 20.0),
                                            rect.top() + y_offset,
                                            Stroke::new(2.0, Color32::from_gray(0xE0)),
                                        );
                                    }

                                    // Bottom Page Number
                                    let label = crate::i18n::fmt(tl!("Page {n}"), &[("n", &page_num.to_string())]);
                                    painter.text(
                                        rect.center_bottom() - vec2(0.0, 14.0),
                                        Align2::CENTER_CENTER,
                                        label,
                                        theme::semibold(12.5),
                                        red_accent,
                                    );
                                } else {
                                    // Drop shadow
                                    painter.rect_filled(rect.translate(vec2(2.0, 2.0)), CornerRadius::same(8), Color32::from_black_alpha(15));
                                    // Card fill & stroke
                                    let stroke = if resp.hovered() { Stroke::new(1.4, t.accent) } else { Stroke::new(1.0, t.border) };
                                    painter.rect_filled(rect, CornerRadius::same(8), Color32::WHITE);
                                    painter.rect_stroke(rect, CornerRadius::same(8), stroke, egui::StrokeKind::Inside);

                                    // Top Pill: "Keep"
                                    let pill_rect = Rect::from_min_size(rect.left_top() + vec2(10.0, 8.0), vec2(card_w - 20.0, 22.0));
                                    painter.rect_filled(pill_rect, CornerRadius::same(11), t.field);
                                    painter.rect_stroke(pill_rect, CornerRadius::same(11), Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
                                    painter.text(pill_rect.center(), Align2::CENTER_CENTER, tl!("Keep"), theme::medium(11.0), t.text_muted);

                                    // Simulated text content lines
                                    let line_c = Color32::from_gray(0xEB);
                                    for y_offset in [46.0, 60.0, 74.0, 88.0] {
                                        let shrink = if y_offset == 88.0 { 35.0 } else { 18.0 };
                                        painter.hline(
                                            (rect.left() + 18.0)..=(rect.right() - shrink),
                                            rect.top() + y_offset,
                                            Stroke::new(2.0, line_c),
                                        );
                                    }

                                    // Bottom Page Number
                                    let label = crate::i18n::fmt(tl!("Page {n}"), &[("n", &page_num.to_string())]);
                                    painter.text(rect.center_bottom() - vec2(0.0, 14.0), Align2::CENTER_CENTER, label, theme::semibold(12.5), t.text);
                                }

                                // Handle Card Click (Toggle)
                                if resp.clicked() {
                                    if is_to_delete {
                                        app.delete_pages_workspace.selected_pages.remove(&idx);
                                    } else {
                                        app.delete_pages_workspace.selected_pages.insert(idx);
                                    }
                                    app.delete_pages_workspace.range_text = format_page_ranges(&app.delete_pages_workspace.selected_pages);
                                    app.delete_pages_workspace.error = None;
                                }
                            }
                        });

                        ui.add_space(20.0);

                        // Messages / Alerts
                        if let Some(err) = &app.delete_pages_workspace.error {
                            egui::Frame::NONE
                                .fill(Color32::from_rgb(0xFD, 0xED, 0xEC))
                                .stroke(Stroke::new(1.0, Color32::from_rgb(0xE7, 0x4C, 0x3C)))
                                .corner_radius(CornerRadius::same(6))
                                .inner_margin(egui::Margin::symmetric(14, 10))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        draw_icon(ui, "alert-circle", 15.0, Color32::from_rgb(0xC0, 0x39, 0x2B));
                                        ui.label(egui::RichText::new(err).font(theme::medium(12.5)).color(Color32::from_rgb(0xC0, 0x39, 0x2B)));
                                    });
                                });
                            ui.add_space(10.0);
                        }

                        if let Some(note) = &app.delete_pages_workspace.success_note {
                            egui::Frame::NONE
                                .fill(Color32::from_rgb(0xEA, 0xFA, 0xEA))
                                .stroke(Stroke::new(1.0, Color32::from_rgb(0x27, 0xAE, 0x60)))
                                .corner_radius(CornerRadius::same(6))
                                .inner_margin(egui::Margin::symmetric(14, 10))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        draw_icon(ui, "check-circle-2", 15.0, Color32::from_rgb(0x27, 0xAE, 0x60));
                                        ui.label(egui::RichText::new(note).font(theme::medium(12.5)).color(Color32::from_rgb(0x1E, 0x84, 0x49)));
                                    });
                                });
                            ui.add_space(10.0);
                        }

                        ui.separator();
                        ui.add_space(14.0);

                        // Action Buttons Footer
                        let can_execute =
                            !app.delete_pages_workspace.selected_pages.is_empty() && app.delete_pages_workspace.selected_pages.len() < total_pages;

                        ui.horizontal(|ui| {
                            if widgets::dialog_action_button(ui, "rotate-ccw", tl!("Select another file"), false).clicked() {
                                app.delete_pages_workspace.source = None;
                                app.delete_pages_workspace.selected_pages.clear();
                                app.delete_pages_workspace.range_text.clear();
                                app.delete_pages_workspace.error = None;
                                app.delete_pages_workspace.success_note = None;
                            }

                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.add_enabled_ui(can_execute, |ui| {
                                    if widgets::dialog_action_button(ui, "save", tl!("Save PDF without deleted pages"), true).clicked() {
                                        app.execute_delete_pages_and_save();
                                    }
                                });

                                ui.add_space(8.0);

                                ui.add_enabled_ui(can_execute, |ui| {
                                    if widgets::dialog_action_button(ui, "book-open", tl!("Open in KarnoxPDF"), false).clicked() {
                                        app.execute_delete_pages_and_open();
                                    }
                                });
                            });
                        });
                    } else {
                        // Empty / Drag & Drop Selection Screen
                        ui.vertical_centered(|ui| {
                            ui.add_space(20.0);

                            let (rect, _) = ui.allocate_exact_size(vec2(72.0, 72.0), Sense::hover());
                            ui.painter().circle_filled(rect.center(), 36.0, t.hover);
                            icons::paint(ui, Rect::from_center_size(rect.center(), vec2(36.0, 36.0)), "trash-2", 36.0, t.accent);

                            ui.add_space(16.0);
                            ui.label(egui::RichText::new(tl!("Select a PDF to delete pages")).font(theme::semibold(18.0)).color(t.text));
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(tl!("Choose a document to review and delete specific pages."))
                                    .font(theme::regular(13.5))
                                    .color(t.text_muted),
                            );

                            ui.add_space(20.0);

                            if widgets::dialog_action_button(ui, "folder-open", tl!("Select PDF File"), true).clicked() {
                                app.delete_pages_source_dialog();
                            }

                            if app.active.is_some() {
                                ui.add_space(10.0);
                                if widgets::dialog_action_button(ui, "file-text", tl!("Use currently open document"), false).clicked() {
                                    app.open_delete_pages_workspace();
                                }
                            }

                            ui.add_space(20.0);
                        });
                    }
                });

            ui.add_space(32.0);
        });
    });
}

// ---------------------------------------------------------------------------
// App Methods & Execution
// ---------------------------------------------------------------------------

impl KarnoxPDFApp {
    pub fn open_delete_pages_workspace(&mut self) {
        if let Some(i) = self.active
            && let Some(doc) = self.session.get(self.views[i].id)
        {
            self.delete_pages_workspace.source =
                Some(DeletePdfSource { name: doc.name.clone(), bytes: doc.bytes.clone(), pages: doc.info.pages.clone(), path: doc.path.clone() });
            self.delete_pages_workspace.selected_pages.clear();
            self.delete_pages_workspace.range_text.clear();
        }

        self.deactivate_all_workspaces();
        self.delete_pages_workspace.open = true;
        self.delete_pages_workspace.active = true;
        self.delete_pages_workspace.error = None;
        self.delete_pages_workspace.success_note = None;
        self.active = None;
        self.dialog = None;
    }

    pub fn close_delete_pages_workspace(&mut self) {
        let was_active = self.delete_pages_workspace.active;
        self.delete_pages_workspace.active = false;
        self.delete_pages_workspace.open = false;
        self.delete_pages_workspace.source = None;
        self.delete_pages_workspace.selected_pages.clear();
        self.delete_pages_workspace.range_text.clear();
        if was_active {
            self.active = self.views.len().checked_sub(1);
        }
    }

    pub(crate) fn delete_pages_source_dialog(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let dialog = rfd::AsyncFileDialog::new().add_filter("PDF", &["pdf"]);
            self.ask_one(crate::pickers::Ask::File(dialog), None, |app, path| {
                if let Ok(bytes) = std::fs::read(&path) {
                    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "document.pdf".into());
                    app.stage_delete_pages_source(name, bytes, Some(path.to_string_lossy().into_owned()));
                }
            });
        }
    }

    pub(crate) fn stage_delete_pages_source(&mut self, name: String, bytes: Vec<u8>, path: Option<String>) {
        let bytes = Arc::new(bytes);
        match karnoxpdf_render::inspect(bytes.clone(), None) {
            Ok(info) if !info.pages.is_empty() => {
                self.delete_pages_workspace.source = Some(DeletePdfSource { name, bytes, pages: info.pages, path });
                self.delete_pages_workspace.selected_pages.clear();
                self.delete_pages_workspace.range_text.clear();
                self.open_delete_pages_workspace();
            }
            Ok(_) => self.notify_tr("The PDF has no pages to convert."),
            Err(e) => self.notify_fmt("Couldn't open {name}: {e}", &[("name", &name), ("e", &e.to_string())]),
        }
    }

    fn delete_pages_filename(&self) -> String {
        let source_name = self.delete_pages_workspace.source.as_ref().map(|s| s.name.as_str()).unwrap_or("document.pdf");
        let stem = source_name.trim_end_matches(".pdf").trim_end_matches(".PDF");
        let safe: String = stem.chars().map(|ch| if ch.is_alphanumeric() || " -_.,()".contains(ch) { ch } else { '_' }).collect();
        let trimmed = safe.trim_matches([' ', '.']);
        let stem = if trimmed.is_empty() { "document" } else { trimmed };
        format!("{stem}_edited.pdf")
    }

    /// Execute page deletion on the source bytes and return the modified PDF bytes.
    pub fn execute_delete_pages(&self) -> Result<Vec<u8>, String> {
        let source = self.delete_pages_workspace.source.as_ref().ok_or_else(|| tl!("No document to process.").to_string())?;

        let to_delete: Vec<usize> = self.delete_pages_workspace.selected_pages.iter().copied().collect();
        if to_delete.is_empty() {
            return Err(tl!("Select at least one page to delete.").to_string());
        }

        if to_delete.len() >= source.pages.len() {
            return Err(tl!("Cannot delete all pages. The PDF must keep at least 1 page.").to_string());
        }

        let mut session = Session::new();
        let doc_id = session.open(&source.name, None, source.bytes.clone(), None).map_err(|e| e.to_string())?;

        let edit = Edit::DeletePages { pages: to_delete };
        session.apply(doc_id, edit).map_err(|e| e.to_string())?;

        session.save_bytes(doc_id).map(|b| (*b).clone()).map_err(|e| e.to_string())
    }

    /// Execute page deletion and save to disk via file dialog.
    pub fn execute_delete_pages_and_save(&mut self) {
        let bytes = match self.execute_delete_pages() {
            Ok(b) => Arc::new(b),
            Err(err) => {
                self.delete_pages_workspace.error = Some(err);
                return;
            }
        };

        self.delete_pages_workspace.error = None;
        let filename = self.delete_pages_filename();

        #[cfg(not(target_arch = "wasm32"))]
        {
            let write = move |app: &mut Self, path: std::path::PathBuf| {
                let path = if path.extension().is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case("pdf")) {
                    path
                } else {
                    path.with_extension("pdf")
                };
                let target = path.to_string_lossy().into_owned();
                match crate::editing::write_atomically(&target, &bytes) {
                    Ok(()) => {
                        app.notify_fmt("Saved {name}", &[("name", &target)]);
                        app.delete_pages_workspace.success_note = Some(tl!("Pages deleted and PDF saved successfully!").to_string());
                    }
                    Err(e) => app.notify_fmt("Couldn't write {path}: {e}", &[("path", &target), ("e", &e.to_string())]),
                }
            };

            match self.save_override.clone() {
                Some(path) => write(self, path.into()),
                None => {
                    let dialog = rfd::AsyncFileDialog::new().add_filter("PDF", &["pdf"]).set_file_name(&filename);
                    self.ask_one(crate::pickers::Ask::Save(dialog), None, write);
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        let _ = crate::editing::download(&filename, &bytes);
    }

    /// Execute page deletion and open directly in KarnoxPDF tabs.
    pub fn execute_delete_pages_and_open(&mut self) {
        let bytes = match self.execute_delete_pages() {
            Ok(b) => b,
            Err(err) => {
                self.delete_pages_workspace.error = Some(err);
                return;
            }
        };

        let filename = self.delete_pages_filename();
        let _ = self.open_bytes(&filename, None, bytes);
        self.close_delete_pages_workspace();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_ranges_works() {
        let mut set = BTreeSet::new();
        assert_eq!(format_page_ranges(&set), "");

        set.insert(0);
        assert_eq!(format_page_ranges(&set), "1");

        set.insert(2);
        set.insert(3);
        set.insert(4);
        assert_eq!(format_page_ranges(&set), "1, 3-5");

        set.insert(7);
        assert_eq!(format_page_ranges(&set), "1, 3-5, 8");
    }

    #[test]
    fn parse_ranges_works() {
        let total = 10;
        let set = parse_page_ranges("1, 3-5, 8", total).unwrap();
        assert_eq!(set, BTreeSet::from([0, 2, 3, 4, 7]));

        let set2 = parse_page_ranges("2; 4; 6-7", total).unwrap();
        assert_eq!(set2, BTreeSet::from([1, 3, 5, 6]));

        assert!(parse_page_ranges("0", total).is_err());
        assert!(parse_page_ranges("11", total).is_err());
        assert!(parse_page_ranges("abc", total).is_err());
    }

    #[test]
    fn workspace_defaults() {
        let ws = DeletePagesWorkspace::default();
        assert!(!ws.open);
        assert!(!ws.active);
        assert!(ws.source.is_none());
        assert!(ws.selected_pages.is_empty());
        assert!(ws.range_text.is_empty());
    }

    #[test]
    fn delete_pages_execution_and_validation() {
        let path = "tests/data/form.pdf";
        if let Ok(bytes) = std::fs::read(path) {
            let bytes = Arc::new(bytes);
            if let Ok(info) = karnoxpdf_render::inspect(bytes.clone(), None) {
                let mut app = KarnoxPDFApp::new();
                app.delete_pages_workspace.source =
                    Some(DeletePdfSource { name: "form.pdf".into(), bytes: bytes.clone(), pages: info.pages.clone(), path: Some(path.into()) });

                // 1. Error if nothing selected
                assert!(app.execute_delete_pages().is_err());

                // 2. Error if all pages selected
                app.delete_pages_workspace.selected_pages = (0..info.pages.len()).collect();
                assert!(app.execute_delete_pages().is_err());

                // 3. If multi-page, deleting 1 page succeeds
                if info.pages.len() > 1 {
                    app.delete_pages_workspace.selected_pages = BTreeSet::from([0]);
                    let res = app.execute_delete_pages();
                    assert!(res.is_ok());
                    let new_bytes = Arc::new(res.unwrap());
                    let new_info = karnoxpdf_render::inspect(new_bytes, None).unwrap();
                    assert_eq!(new_info.pages.len(), info.pages.len() - 1);
                }
            }
        }
    }
}
