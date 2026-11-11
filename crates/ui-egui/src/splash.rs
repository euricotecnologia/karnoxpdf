//! Splash screen with progress bar on startup.

use egui::{Color32, CornerRadius, Pos2, Rect, Sense, vec2};
use std::time::Instant;

const SPLASH_PNG: &[u8] = include_bytes!("../../../img/splash1.png");
pub const SPLASH_WINDOW_SIZE: [f32; 2] = [840.0, 500.0];
const IMAGE_RATIO: f32 = 1672.0 / 941.0;

pub struct SplashState {
    pub start: Instant,
    pub duration_secs: f32,
    pub finished: bool,
    pub texture: Option<egui::TextureHandle>,
    restore_viewport: Option<egui::ViewportBuilder>,
    center_pending: bool,
}

impl Default for SplashState {
    fn default() -> Self {
        Self { start: Instant::now(), duration_secs: 2.0, finished: true, texture: None, restore_viewport: None, center_pending: false }
    }
}

impl SplashState {
    pub fn enable(&mut self) {
        self.finished = false;
        self.start = Instant::now();
    }

    pub fn enable_in_compact_window(&mut self, restore_viewport: egui::ViewportBuilder) {
        self.enable();
        self.restore_viewport = Some(restore_viewport);
        self.center_pending = true;
    }

    pub fn restore_window(&mut self, ctx: &egui::Context) {
        let Some(viewport) = self.restore_viewport.take() else { return };
        ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(viewport.min_inner_size.unwrap_or(vec2(820.0, 520.0))));
        ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(viewport.decorations.unwrap_or(true)));
        ctx.send_viewport_cmd(egui::ViewportCommand::Resizable(viewport.resizable.unwrap_or(true)));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(viewport.inner_size.unwrap_or(vec2(1440.0, 920.0))));
        if let Some(position) = viewport.position {
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(position));
        }
        if viewport.maximized.unwrap_or(false) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
        }
        if viewport.fullscreen.unwrap_or(false) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
        }
        self.center_pending = false;
        ctx.request_repaint();
    }

    /// Return true when splash is complete.
    pub fn update_and_show(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) -> bool {
        if self.finished {
            return true;
        }

        if self.center_pending
            && let Some(command) = egui::ViewportCommand::center_on_screen(ctx)
        {
            ctx.send_viewport_cmd(command);
            self.center_pending = false;
        }

        // Allow skipping by clicking or pressing any key.
        let skip = ui.input(|i| {
            i.pointer.any_click() || i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::Space) || i.key_pressed(egui::Key::Enter)
        });

        let elapsed = self.start.elapsed().as_secs_f32();
        let progress = (elapsed / self.duration_secs).clamp(0.0, 1.0);

        if skip || progress >= 1.0 {
            self.finished = true;
            self.restore_window(ctx);
            return true;
        }

        // Keep repainting while splash is active for smooth progress animation.
        ctx.request_repaint();

        // Ensure texture is loaded.
        if self.texture.is_none()
            && let Ok(color_img) = decode_png(SPLASH_PNG)
        {
            self.texture = Some(ctx.load_texture("splash_image", color_img, egui::TextureOptions::LINEAR));
        }

        let full_rect = ui.max_rect();
        // The window fits the artwork; only a slim progress strip sits below it.
        ui.painter().rect_filled(full_rect, CornerRadius::ZERO, Color32::from_rgb(248, 250, 252));
        let artwork_height = (full_rect.height() - 24.0).max(1.0);
        let image_width = full_rect.width().min(artwork_height * IMAGE_RATIO);
        let image_height = image_width / IMAGE_RATIO;
        let img_rect = Rect::from_min_size(
            Pos2::new(full_rect.center().x - image_width / 2.0, full_rect.top() + (artwork_height - image_height) / 2.0),
            vec2(image_width, image_height),
        );

        // Draw splash image
        if let Some(texture) = &self.texture {
            ui.painter().image(texture.id(), img_rect, Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)), Color32::WHITE);
        }

        // Thin rounded track with a luminous leading edge.
        let bar_w = image_width * 0.82;
        let bar_h = 5.0;
        let bar_rect = Rect::from_min_size(Pos2::new(full_rect.center().x - bar_w / 2.0, img_rect.bottom() + 10.0), vec2(bar_w, bar_h));
        ui.painter().rect_filled(bar_rect, CornerRadius::same(3), Color32::from_rgb(216, 229, 247));
        let fill_w = bar_w * progress;
        if fill_w > 0.0 {
            let fill_rect = Rect::from_min_size(bar_rect.min, vec2(fill_w, bar_h));
            ui.painter().rect_filled(fill_rect, CornerRadius::same(3), Color32::from_rgb(0, 105, 235));
            let glow_w = fill_w.min(28.0);
            let glow_rect = Rect::from_min_size(Pos2::new(fill_rect.right() - glow_w, fill_rect.top()), vec2(glow_w, bar_h));
            ui.painter().rect_filled(glow_rect, CornerRadius::same(3), Color32::from_rgba_unmultiplied(120, 211, 255, 135));
        }

        // Handle clicks on whole screen to skip
        let resp = ui.allocate_rect(full_rect, Sense::click());
        if resp.clicked() {
            self.finished = true;
            self.restore_window(ctx);
            return true;
        }

        false
    }
}

pub(crate) fn decode_png(bytes: &[u8]) -> Result<egui::ColorImage, String> {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let buf_size = reader.output_buffer_size().ok_or_else(|| "missing output buffer size".to_string())?;
    let mut buf = vec![0; buf_size];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let (w, h) = (info.width as usize, info.height as usize);
    let raw = &buf[..info.buffer_size()];
    let image = match info.color_type {
        png::ColorType::Rgba => egui::ColorImage::from_rgba_unmultiplied([w, h], raw),
        png::ColorType::Rgb => {
            let mut rgba = Vec::with_capacity(w * h * 4);
            for chunk in raw.as_chunks::<3>().0 {
                rgba.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
            }
            egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba)
        }
        _ => return Err("unsupported png color format".into()),
    };
    Ok(image)
}
