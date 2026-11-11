//! Headless screenshot of the real KarnoxPDF shell (egui_kittest + wgpu, no window needed).
//!
//! ```text
//! cargo run -p karnoxpdf-ui-egui --example shot -- out.png [file.pdf] [--size 1440x900] [--scale 2] [--page 3 --panel pages …]
//! ```
//! `--width N` downscales the image to N pixels wide (README screenshots). Any other
//! `--key value` pair is passed to `KarnoxPDFApp::set_option`
//! (the same verbs as the desktop app's command-line flags and the future control channel).

use std::time::{Duration, Instant};

use egui_kittest::Harness;
use karnoxpdf_ui_egui::KarnoxPDFApp;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let out = args.next().ok_or("usage: shot <out.png> [file.pdf] [--option value …]")?;
    let (mut file, mut opts) = (None, Vec::new());
    let (mut size, mut scale) = (egui::vec2(1440.0, 900.0), 2.0f32);
    let mut explicit_size = false;
    let mut width: Option<u32> = None;
    let mut splash = false;
    while let Some(a) = args.next() {
        match a.strip_prefix("--") {
            Some("size") => {
                let v = args.next().unwrap_or_default();
                let (w, h) = v.split_once('x').ok_or("--size WxH")?;
                size = egui::vec2(w.parse().map_err(|_| "bad width")?, h.parse().map_err(|_| "bad height")?);
                explicit_size = true;
            }
            Some("scale") => scale = args.next().and_then(|v| v.parse().ok()).ok_or("bad --scale")?,
            Some("width") => width = Some(args.next().and_then(|v| v.parse().ok()).ok_or("bad --width")?),
            Some("splash") => splash = true,
            Some(k) => opts.push((k.to_string(), args.next().unwrap_or_default())),
            None => file = Some(a),
        }
    }
    if splash && !explicit_size {
        size = karnoxpdf_ui_egui::SPLASH_WINDOW_SIZE.into();
    }
    let mut harness = Harness::builder().with_size(size).with_pixels_per_point(scale).build_eframe(move |_cc| {
        let mut app = KarnoxPDFApp::new();
        if splash {
            app.enable_splash();
        }
        if let Some(f) = &file {
            let bytes = std::fs::read(f).unwrap_or_default();
            let name = std::path::Path::new(f).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if let Err(e) = app.open_bytes(&name, Some(f.clone()), bytes) {
                eprintln!("shot: {f}: {e}");
            }
        }
        for (k, v) in &opts {
            if let Err(e) = app.set_option(k, v) {
                eprintln!("shot: --{k} {v}: {e}");
            }
        }
        app
    });
    if splash {
        harness.run_steps(2);
    } else {
        // Let fonts install, layout settle and background renders arrive.
        let start = Instant::now();
        harness.run_steps(4);
        while start.elapsed() < Duration::from_secs(20) {
            harness.run_steps(2);
            if !harness.state().render_pending() {
                harness.run_steps(3);
                if !harness.state().render_pending() {
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(30));
        }
    }
    let mut image = harness.render()?;
    if let Some(w) = width.filter(|w| *w < image.width()) {
        let h = (image.height() as f64 * w as f64 / image.width() as f64).round() as u32;
        image = image::imageops::resize(&image, w, h, image::imageops::FilterType::Lanczos3);
    }
    image.save(&out).map_err(|e| e.to_string())?;
    eprintln!("shot: wrote {out} ({}×{})", image.width(), image.height());
    Ok(())
}
