//! Small custom widgets built on the design tokens.

use egui::{Align2, Color32, CornerRadius, Rect, Response, Sense, Stroke, vec2};

use crate::theme::{self, Tokens};
use crate::{KarnoxPDFApp, icons};

/// A mode-bar tab: text with an underline when active.
pub fn mode_tab(ui: &mut egui::Ui, label: &str, active: bool) -> Response {
    let t = Tokens::get(ui.ctx());
    let font = if active { theme::semibold(13.5) } else { theme::medium(13.5) };
    let w = ui.fonts_mut(|f| f.layout_no_wrap(label.to_owned(), font.clone(), t.text).size().x);
    let (rect, resp) = ui.allocate_exact_size(vec2(w + 22.0, 48.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), active, label));
    if resp.hovered() && !active {
        ui.painter().rect_filled(rect.shrink2(vec2(2.0, 9.0)), CornerRadius::same(6), t.hover);
    }
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, font, if active { t.text } else { t.text_muted });
    if active {
        let r = Rect::from_min_max(rect.left_bottom() + vec2(11.0, -3.0), rect.right_bottom() - vec2(11.0, 0.0));
        ui.painter().rect_filled(r, CornerRadius::same(1), t.text);
    }
    resp
}

/// The one look of every action button (the Watermark screen's): 36 px tall, 6 px corners, an
/// optional icon on the left, filled with the accent when `primary` and a bordered card otherwise.
/// Disabled buttons are dimmed and do not react to the pointer.
fn paint_action_button(ui: &mut egui::Ui, icon: Option<&str>, label: &str, primary: bool, height: f32) -> Response {
    let t = Tokens::get(ui.ctx());
    let font = theme::medium(13.0);
    let enabled = ui.is_enabled();
    let text_width = ui.fonts_mut(|f| f.layout_no_wrap(label.to_owned(), font.clone(), t.text).size().x);
    let width = text_width + if icon.is_some() { 48.0 } else { 32.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    let hovered = enabled && resp.hovered();
    let (fill, stroke, text) = match (primary, enabled) {
        (true, true) => (if hovered { t.accent_text } else { t.accent }, Stroke::NONE, Color32::WHITE),
        (true, false) => (t.accent.gamma_multiply(0.4), Stroke::NONE, Color32::WHITE.gamma_multiply(0.85)),
        (false, true) => (if hovered { t.hover } else { t.card }, Stroke::new(1.0, t.border), t.text),
        (false, false) => (t.card, Stroke::new(1.0, t.border), t.text_faint),
    };
    ui.painter().rect(rect, CornerRadius::same(6), fill, stroke, egui::StrokeKind::Inside);
    match icon {
        Some(icon) => {
            icons::paint(ui, Rect::from_center_size(rect.left_center() + vec2(20.0, 0.0), vec2(16.0, 16.0)), icon, 15.0, text);
            ui.painter().text(rect.left_center() + vec2(34.0, 0.0), Align2::LEFT_CENTER, label, font, text);
        }
        None => {
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, font, text);
        }
    }
    resp
}

/// Action button without an icon, in the look of [`dialog_action_button`]; `primary` fills with the accent.
pub fn action_button(ui: &mut egui::Ui, label: &str, primary: bool) -> Response {
    paint_action_button(ui, None, label, primary, 36.0)
}

/// Kept for the many call sites that predate the shared look: it is [`action_button`].
pub fn pill_button(ui: &mut egui::Ui, label: &str, primary: bool) -> Response {
    paint_action_button(ui, None, label, primary, 36.0)
}

/// Compact action button for confirmation dialogs, with a clear icon and restrained corners.
pub fn dialog_action_button(ui: &mut egui::Ui, icon: &str, label: &str, primary: bool) -> Response {
    paint_action_button(ui, Some(icon), label, primary, 36.0)
}

/// Icon + label, transparent until hovered.
pub fn ghost_button(ui: &mut egui::Ui, icon: &str, label: &str) -> Response {
    let t = Tokens::get(ui.ctx());
    let font = theme::medium(13.0);
    let enabled = ui.is_enabled();
    let text_color = if enabled { t.text } else { t.text_muted };
    let icon_color = if enabled { t.icon } else { t.text_faint };
    let w = ui.fonts_mut(|f| f.layout_no_wrap(label.to_owned(), font.clone(), text_color).size().x);
    let (rect, resp) = ui.allocate_exact_size(vec2(w + 38.0, 30.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    if enabled && resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(6), t.hover);
    }
    icons::paint(ui, Rect::from_min_size(rect.min + vec2(6.0, 6.0), vec2(18.0, 18.0)), icon, 17.0, icon_color);
    ui.painter().text(rect.left_center() + vec2(30.0, 0.0), Align2::LEFT_CENTER, label, font, text_color);
    resp
}

/// A search-field lookalike that opens the command palette.
#[allow(dead_code)]
pub fn search_box(ui: &mut egui::Ui, placeholder: &str, width: f32) -> Response {
    let t = Tokens::get(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 32.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), placeholder));
    let fill = if resp.hovered() { t.hover } else { t.field };
    ui.painter().rect(rect, CornerRadius::same(16), fill, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
    icons::paint(ui, Rect::from_min_size(rect.min + vec2(10.0, 8.0), vec2(16.0, 16.0)), "search", 15.0, t.text_muted);
    ui.painter().text(rect.left_center() + vec2(34.0, 0.0), Align2::LEFT_CENTER, placeholder, theme::regular(13.0), t.text_faint);
    resp.on_hover_cursor(egui::CursorIcon::Text)
}

pub fn menu_item(ui: &mut egui::Ui, label: &str) -> Response {
    ui.add(egui::Button::new(label))
}

pub fn section_title(ui: &mut egui::Ui, text: &str) {
    let t = Tokens::get(ui.ctx());
    ui.add_space(10.0);
    // Section titles across every panel go through here, so one translation point covers them.
    ui.label(egui::RichText::new(tl!(text).to_uppercase()).font(theme::semibold(10.5)).color(t.text_faint).extra_letter_spacing(0.6));
    ui.add_space(2.0);
}

/// Transient message at the bottom centre.
pub fn toast(app: &mut KarnoxPDFApp, ctx: &egui::Context) {
    let Some((msg, start)) = app.toast.clone() else { return };
    let now = ctx.input(|i| i.time);
    let start = if start == 0.0 { now } else { start };
    app.toast = Some((msg.clone(), start));
    if now - start > 3.5 {
        app.toast = None;
        return;
    }
    let t = Tokens::get(ctx);
    let screen = ctx.content_rect();
    egui::Area::new(egui::Id::new("toast"))
        .order(egui::Order::Tooltip)
        .pivot(Align2::CENTER_BOTTOM)
        .fixed_pos(screen.center_bottom() - vec2(0.0, 28.0))
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(if t.dark() { Color32::from_rgb(0xEC, 0xEC, 0xEF) } else { Color32::from_rgb(0x2A, 0x2A, 0x2F) })
                .corner_radius(CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(16, 10))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(msg).color(if t.dark() { Color32::from_rgb(0x22, 0x22, 0x26) } else { Color32::WHITE }));
                });
        });
    ctx.request_repaint_after(std::time::Duration::from_millis(100));
}

/// A radio choice drawn as an outlined circle with an accent dot when on, as on the Page numbers
/// screen. Returns `true` when it was clicked. Disabled rows are dimmed.
pub fn radio_row(ui: &mut egui::Ui, selected: bool, label: &str) -> bool {
    let t = Tokens::get(ui.ctx());
    let enabled = ui.is_enabled();
    let font = theme::regular(13.0);
    let text_w = ui.fonts_mut(|f| f.layout_no_wrap(label.to_owned(), font.clone(), t.text).size().x);
    let (rect, resp) = ui.allocate_exact_size(vec2((text_w + 32.0).max(120.0), 24.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, enabled, selected, label));
    let hovered = enabled && resp.hovered();
    let center = rect.left_center() + vec2(9.0, 0.0);
    let (circle_bg, circle_stroke) = if selected {
        (t.field, Stroke::new(1.8, t.accent))
    } else if hovered {
        (t.hover, Stroke::new(1.2, t.text_muted))
    } else {
        (t.field, Stroke::new(1.2, t.border))
    };
    let painter = ui.painter();
    painter.circle(center, 7.0, circle_bg, circle_stroke);
    if selected {
        painter.circle_filled(center, 3.5, t.accent);
    }
    let text_color = if !enabled {
        t.text_faint
    } else if selected {
        t.text
    } else {
        t.text_muted
    };
    painter.text(rect.left_center() + vec2(22.0, 0.0), Align2::LEFT_CENTER, label, font, text_color);
    enabled && resp.clicked()
}

/// A checkbox drawn as a rounded square, filled with the accent and a white tick when on, as on the
/// Page numbers screen. Flips `checked` on a click and returns `true` when it did.
pub fn check_row(ui: &mut egui::Ui, checked: &mut bool, label: &str) -> bool {
    let t = Tokens::get(ui.ctx());
    let enabled = ui.is_enabled();
    let font = theme::regular(13.0);
    let text_w = ui.fonts_mut(|f| f.layout_no_wrap(label.to_owned(), font.clone(), t.text).size().x);
    let (rect, resp) = ui.allocate_exact_size(vec2(text_w + 32.0, 24.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, enabled, *checked, label));
    let hovered = enabled && resp.hovered();
    let box_rect = Rect::from_center_size(rect.left_center() + vec2(9.0, 0.0), vec2(16.0, 16.0));
    let (bg_fill, stroke_color) = if *checked {
        (t.accent, t.accent)
    } else if hovered {
        (t.hover, t.text_muted)
    } else {
        (t.field, t.border)
    };
    let painter = ui.painter();
    painter.rect(box_rect, CornerRadius::same(4), bg_fill, Stroke::new(1.2, stroke_color), egui::StrokeKind::Inside);
    if *checked {
        let p1 = box_rect.left_top() + vec2(3.5, 8.5);
        let p2 = box_rect.left_top() + vec2(6.5, 12.0);
        let p3 = box_rect.left_top() + vec2(12.5, 4.5);
        painter.line_segment([p1, p2], Stroke::new(2.0, Color32::WHITE));
        painter.line_segment([p2, p3], Stroke::new(2.0, Color32::WHITE));
    }
    let text_color = if !enabled {
        t.text_faint
    } else if *checked {
        t.text
    } else {
        t.text_muted
    };
    painter.text(rect.left_center() + vec2(22.0, 0.0), Align2::LEFT_CENTER, label, font, text_color);
    if enabled && resp.clicked() {
        *checked = !*checked;
        true
    } else {
        false
    }
}

/// A choice button of the Watermark screen: an optional icon and a label, filled with the accent and
/// white text when `selected`, a bordered card otherwise. `min_width` keeps a row of choices even.
pub fn option_button(ui: &mut egui::Ui, icon: Option<&str>, label: &str, selected: bool, min_width: f32) -> Response {
    let t = Tokens::get(ui.ctx());
    let enabled = ui.is_enabled();
    let font = if selected { theme::semibold(13.0) } else { theme::medium(13.0) };
    let text_w = ui.fonts_mut(|f| f.layout_no_wrap(label.to_owned(), font.clone(), t.text).size().x);
    let total_w = (text_w + if icon.is_some() { 36.0 } else { 22.0 }).max(min_width);
    let (rect, resp) = ui.allocate_exact_size(vec2(total_w, 34.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, selected, label));
    let hovered = enabled && resp.hovered();
    let (bg, stroke, text_color, icon_color) = if !enabled {
        (t.card, Stroke::new(1.0, t.border), t.text_faint, t.text_faint)
    } else if selected {
        (t.accent, Stroke::NONE, Color32::WHITE, Color32::WHITE)
    } else if hovered {
        (t.hover, Stroke::new(1.2, t.accent), t.text, t.accent)
    } else {
        (t.card, Stroke::new(1.0, t.border), t.text, t.text_muted)
    };
    ui.painter().rect(rect, CornerRadius::same(7), bg, stroke, egui::StrokeKind::Inside);
    if let Some(icon) = icon {
        icons::paint(ui, Rect::from_center_size(rect.left_center() + vec2(15.0, 0.0), vec2(16.0, 16.0)), icon, 14.0, icon_color);
        ui.painter().text(rect.left_center() + vec2(29.0, 0.0), Align2::LEFT_CENTER, label, font, text_color);
    } else {
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, font, text_color);
    }
    resp
}

/// A compact rounded choice chip of the Watermark screen (presets, angles, opacities).
pub fn chip_button(ui: &mut egui::Ui, label: &str, selected: bool) -> Response {
    let t = Tokens::get(ui.ctx());
    let enabled = ui.is_enabled();
    let font = if selected { theme::semibold(12.0) } else { theme::medium(12.0) };
    let text_w = ui.fonts_mut(|f| f.layout_no_wrap(label.to_owned(), font.clone(), t.text).size().x);
    let (rect, resp) = ui.allocate_exact_size(vec2(text_w + 18.0, 26.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, selected, label));
    let hovered = enabled && resp.hovered();
    let (bg, stroke, text_color) = if !enabled {
        (t.card, Stroke::new(1.0, t.border), t.text_faint)
    } else if selected {
        (t.accent, Stroke::NONE, Color32::WHITE)
    } else if hovered {
        (t.hover, Stroke::new(1.2, t.accent), t.text)
    } else {
        (t.card, Stroke::new(1.0, t.border), t.text)
    };
    ui.painter().rect(rect, CornerRadius::same(7), bg, stroke, egui::StrokeKind::Inside);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, font, text_color);
    resp
}

/// Makes the inputs of a card (drag values, sliders) look like the Page numbers screen: a bordered
/// field and an accent outline when hovered or dragged.
pub fn style_card_inputs(ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let w = &mut ui.visuals_mut().widgets;
    w.inactive.bg_stroke = Stroke::new(1.0, t.border);
    w.inactive.weak_bg_fill = t.field;
    w.inactive.bg_fill = t.hover;
    w.hovered.bg_stroke = Stroke::new(1.2, t.accent);
    w.active.bg_stroke = Stroke::new(1.5, t.accent);
}

/// The − and + buttons of a stepper.
fn stepper_icon_btn(ui: &mut egui::Ui, t: &Tokens, icon: &str, label: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label));
    let (bg, stroke, tint) = if resp.is_pointer_button_down_on() {
        (t.pressed, Stroke::new(1.2, t.accent), t.accent)
    } else if resp.hovered() {
        (t.hover, Stroke::new(1.2, t.accent), t.text)
    } else {
        (t.field, Stroke::new(1.0, t.border), t.icon)
    };
    ui.painter().rect(rect, CornerRadius::same(5), bg, stroke, egui::StrokeKind::Inside);
    icons::paint(ui, Rect::from_center_size(rect.center(), vec2(13.0, 13.0)), icon, 13.0, tint);
    resp
}

/// A number with − and + buttons and a bordered, draggable field, as on the Page numbers screen.
pub fn stepper_f64(ui: &mut egui::Ui, val: &mut f64, min: f64, max: f64, step: f64, suffix: &str) {
    stepper_number(ui, val, min, max, step, suffix, None);
}

/// [`stepper_f64`] showing whole numbers (angles, percentages).
pub fn stepper_whole(ui: &mut egui::Ui, val: &mut f64, min: f64, max: f64, step: f64, suffix: &str) {
    stepper_number(ui, val, min, max, step, suffix, Some(0));
}

fn stepper_number(ui: &mut egui::Ui, val: &mut f64, min: f64, max: f64, step: f64, suffix: &str, decimals: Option<usize>) {
    let t = Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if stepper_icon_btn(ui, &t, "minus", tl!("Decrease")).clicked() {
            *val = (*val - step).clamp(min, max);
        }
        let mut current = *val;
        egui::Frame::NONE
            .fill(t.field)
            .stroke(Stroke::new(1.0, t.border))
            .corner_radius(CornerRadius::same(5))
            .inner_margin(egui::Margin::symmetric(8, 3))
            .show(ui, |ui| {
                ui.set_min_width(52.0);
                let suffix_str = if suffix.is_empty() { String::new() } else { format!(" {suffix}") };
                let mut drag = egui::DragValue::new(&mut current).range(min..=max).speed(0.5).suffix(suffix_str);
                if let Some(places) = decimals {
                    drag = drag.fixed_decimals(places);
                }
                if ui.add(drag).changed() {
                    *val = current.clamp(min, max);
                }
            });
        if stepper_icon_btn(ui, &t, "plus", tl!("Increase")).clicked() {
            *val = (*val + step).clamp(min, max);
        }
    });
}

/// Like [`stepper_f64`] for whole numbers; `suffix` is shown after the number (e.g. `%`).
pub fn stepper_u32(ui: &mut egui::Ui, val: &mut u32, min: u32, max: u32, step: u32, suffix: &str) {
    let t = Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if stepper_icon_btn(ui, &t, "minus", tl!("Decrease")).clicked() {
            *val = val.saturating_sub(step).max(min);
        }
        let mut num = i64::from(*val);
        egui::Frame::NONE
            .fill(t.field)
            .stroke(Stroke::new(1.0, t.border))
            .corner_radius(CornerRadius::same(5))
            .inner_margin(egui::Margin::symmetric(8, 3))
            .show(ui, |ui| {
                ui.set_min_width(52.0);
                let suffix_str = if suffix.is_empty() { String::new() } else { suffix.to_string() };
                if ui.add(egui::DragValue::new(&mut num).range(i64::from(min)..=i64::from(max)).speed(1.0).suffix(suffix_str)).changed() {
                    *val = num.clamp(i64::from(min), i64::from(max)) as u32;
                }
            });
        if stepper_icon_btn(ui, &t, "plus", tl!("Increase")).clicked() {
            *val = val.saturating_add(step).min(max);
        }
    });
}

/// A compact action button with an icon for tool bars: the look of [`dialog_action_button`], 32 px tall.
#[allow(dead_code)]
pub fn tool_button(ui: &mut egui::Ui, icon: &str, label: &str) -> Response {
    paint_action_button(ui, Some(icon), label, false, 32.0)
}

/// The row of buttons at the foot of a dialog, right to left. It is one button tall: a plain
/// `with_layout` takes all the height still free, which in a modal is the whole screen, and
/// centres the buttons in it, so a short dialog came out as tall as the window.
pub fn button_row<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::InnerResponse<R> {
    let rect = egui::Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), 36.0));
    let out = ui.scope_builder(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::right_to_left(egui::Align::Center)), add);
    ui.allocate_rect(rect, Sense::hover());
    out
}
