//! Content-stream writer for slides. Coordinates are slide points with the origin at the top
//! left and y growing downwards; the stream starts by flipping the page to match, so shapes
//! can use PowerPoint's own coordinate system.

use std::io::Write;

use karnoxpdf_cos::{Dict, Object};

use crate::drawingml::{Rgb, Rgba};

fn n(v: f64) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let s = format!("{:.3}", v.clamp(-1.0e7, 1.0e7));
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" || s == "-0" { "0".into() } else { s.to_owned() }
}

/// An affine transform `[a b c d e f]` as PDF's `cm` operator takes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Matrix(pub [f64; 6]);

impl Matrix {
    pub(crate) fn translate(x: f64, y: f64) -> Matrix {
        Matrix([1.0, 0.0, 0.0, 1.0, x, y])
    }

    /// Rotation by `degrees` clockwise as seen on the (y-down) slide, about `(cx, cy)`.
    pub(crate) fn rotate_about(degrees: f64, cx: f64, cy: f64) -> Matrix {
        let (s, c) = degrees.to_radians().sin_cos();
        Matrix([c, s, -s, c, cx - cx * c + cy * s, cy - cx * s - cy * c])
    }

    pub(crate) fn flip(horizontal: bool, vertical: bool, w: f64, h: f64) -> Matrix {
        Matrix([
            if horizontal { -1.0 } else { 1.0 },
            0.0,
            0.0,
            if vertical { -1.0 } else { 1.0 },
            if horizontal { w } else { 0.0 },
            if vertical { h } else { 0.0 },
        ])
    }
}

#[derive(Clone, Copy, Debug)]
enum Seg {
    Move(f64, f64),
    Line(f64, f64),
    Curve(f64, f64, f64, f64, f64, f64),
    Close,
}

/// A path under construction, in the coordinates of the current transform.
#[derive(Clone, Debug, Default)]
pub(crate) struct Path {
    segs: Vec<Seg>,
    current: (f64, f64),
    start: (f64, f64),
}

const KAPPA: f64 = 0.552_284_749_8;

impl Path {
    pub(crate) fn new() -> Path {
        Path::default()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.segs.is_empty()
    }

    pub(crate) fn move_to(&mut self, x: f64, y: f64) {
        self.segs.push(Seg::Move(x, y));
        self.current = (x, y);
        self.start = (x, y);
    }

    pub(crate) fn line_to(&mut self, x: f64, y: f64) {
        self.segs.push(Seg::Line(x, y));
        self.current = (x, y);
    }

    pub(crate) fn curve_to(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64) {
        self.segs.push(Seg::Curve(x1, y1, x2, y2, x, y));
        self.current = (x, y);
    }

    /// A quadratic Bézier, raised to a cubic.
    pub(crate) fn quad_to(&mut self, x1: f64, y1: f64, x: f64, y: f64) {
        let (x0, y0) = self.current;
        self.curve_to(x0 + 2.0 / 3.0 * (x1 - x0), y0 + 2.0 / 3.0 * (y1 - y0), x + 2.0 / 3.0 * (x1 - x), y + 2.0 / 3.0 * (y1 - y), x, y);
    }

    pub(crate) fn close(&mut self) {
        self.segs.push(Seg::Close);
        self.current = self.start;
    }

    pub(crate) fn rect(x: f64, y: f64, w: f64, h: f64) -> Path {
        let mut p = Path::new();
        p.move_to(x, y);
        p.line_to(x + w, y);
        p.line_to(x + w, y + h);
        p.line_to(x, y + h);
        p.close();
        p
    }

    pub(crate) fn round_rect(x: f64, y: f64, w: f64, h: f64, radius: f64) -> Path {
        let r = radius.clamp(0.0, w.min(h) / 2.0);
        if r <= 0.0 {
            return Path::rect(x, y, w, h);
        }
        let k = r * (1.0 - KAPPA);
        let mut p = Path::new();
        p.move_to(x + r, y);
        p.line_to(x + w - r, y);
        p.curve_to(x + w - k, y, x + w, y + k, x + w, y + r);
        p.line_to(x + w, y + h - r);
        p.curve_to(x + w, y + h - k, x + w - k, y + h, x + w - r, y + h);
        p.line_to(x + r, y + h);
        p.curve_to(x + k, y + h, x, y + h - k, x, y + h - r);
        p.line_to(x, y + r);
        p.curve_to(x, y + k, x + k, y, x + r, y);
        p.close();
        p
    }

    pub(crate) fn ellipse(x: f64, y: f64, w: f64, h: f64) -> Path {
        let (rx, ry) = (w / 2.0, h / 2.0);
        let (cx, cy) = (x + rx, y + ry);
        let (kx, ky) = (rx * KAPPA, ry * KAPPA);
        let mut p = Path::new();
        p.move_to(cx + rx, cy);
        p.curve_to(cx + rx, cy + ky, cx + kx, cy + ry, cx, cy + ry);
        p.curve_to(cx - kx, cy + ry, cx - rx, cy + ky, cx - rx, cy);
        p.curve_to(cx - rx, cy - ky, cx - kx, cy - ry, cx, cy - ry);
        p.curve_to(cx + kx, cy - ry, cx + rx, cy - ky, cx + rx, cy);
        p.close();
        p
    }

    /// DrawingML `arcTo` (§20.1.9.3): an elliptical arc of radii `(rx, ry)` that starts at the
    /// current point, which lies at visual angle `start` (degrees clockwise from the +x axis) on
    /// the ellipse, and sweeps `sweep` degrees.
    pub(crate) fn arc_to(&mut self, rx: f64, ry: f64, start: f64, sweep: f64) {
        if rx <= 0.0 || ry <= 0.0 || !sweep.is_finite() || sweep == 0.0 {
            return;
        }
        let parametric = |deg: f64| {
            let (s, c) = deg.to_radians().sin_cos();
            (rx * s).atan2(ry * c)
        };
        let t1 = parametric(start);
        let mut dt = parametric(start + sweep) - t1;
        if sweep.abs() >= 360.0 {
            dt = if sweep > 0.0 { std::f64::consts::TAU } else { -std::f64::consts::TAU };
        } else {
            while sweep > 0.0 && dt < 0.0 {
                dt += std::f64::consts::TAU;
            }
            while sweep < 0.0 && dt > 0.0 {
                dt -= std::f64::consts::TAU;
            }
        }
        let (x0, y0) = self.current;
        let (cx, cy) = (x0 - rx * t1.cos(), y0 - ry * t1.sin());
        let pieces = ((dt.abs() / std::f64::consts::FRAC_PI_2).ceil() as usize).clamp(1, 4);
        let step = dt / pieces as f64;
        let alpha = 4.0 / 3.0 * (step / 4.0).tan();
        let mut t = t1;
        for _ in 0..pieces {
            let (s0, c0) = t.sin_cos();
            let (s1, c1) = (t + step).sin_cos();
            self.curve_to(
                cx + rx * (c0 - alpha * s0),
                cy + ry * (s0 + alpha * c0),
                cx + rx * (c1 + alpha * s1),
                cy + ry * (s1 - alpha * c1),
                cx + rx * c1,
                cy + ry * s1,
            );
            t += step;
        }
    }

    fn write(&self, out: &mut Vec<u8>) {
        for seg in &self.segs {
            let _ = match *seg {
                Seg::Move(x, y) => writeln!(out, "{} {} m", n(x), n(y)),
                Seg::Line(x, y) => writeln!(out, "{} {} l", n(x), n(y)),
                Seg::Curve(a, b, c, d, e, f) => writeln!(out, "{} {} {} {} {} {} c", n(a), n(b), n(c), n(d), n(e), n(f)),
                Seg::Close => writeln!(out, "h"),
            };
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Stroke {
    pub width: f64,
    pub color: Rgba,
    /// Dash and gap lengths in points; empty is solid.
    pub dash: Vec<f64>,
}

/// What the gradient does across the shape's bounding box.
pub(crate) enum GradientKind {
    /// Linear, at this many degrees clockwise from left to right.
    Linear(f64),
    Radial,
}

/// One slide's content stream and the resources it uses.
pub(crate) struct Canvas {
    buf: Vec<u8>,
    alphas: Vec<u32>,
    images: Vec<usize>,
    shadings: Vec<Object>,
}

impl Canvas {
    pub(crate) fn new(page_h: f64) -> Canvas {
        let mut buf = Vec::new();
        let _ = writeln!(buf, "1 0 0 -1 0 {} cm", n(page_h));
        Canvas { buf, alphas: Vec::new(), images: Vec::new(), shadings: Vec::new() }
    }

    pub(crate) fn save(&mut self) {
        self.buf.extend_from_slice(b"q\n");
    }

    pub(crate) fn restore(&mut self) {
        self.buf.extend_from_slice(b"Q\n");
    }

    pub(crate) fn transform(&mut self, m: Matrix) {
        let [a, b, c, d, e, f] = m.0;
        let _ = writeln!(self.buf, "{} {} {} {} {} {} cm", n(a), n(b), n(c), n(d), n(e), n(f));
    }

    fn alpha_state(&mut self, alpha: f64) {
        if alpha >= 0.999 {
            return;
        }
        let permille = (alpha.clamp(0.0, 1.0) * 1000.0).round() as u32;
        let index = match self.alphas.iter().position(|a| *a == permille) {
            Some(i) => i,
            None => {
                self.alphas.push(permille);
                self.alphas.len() - 1
            }
        };
        let _ = writeln!(self.buf, "/A{index} gs");
    }

    fn color(&mut self, rgb: Rgb, op: &str) {
        let _ = writeln!(self.buf, "{} {} {} {op}", n(rgb[0]), n(rgb[1]), n(rgb[2]));
    }

    pub(crate) fn fill(&mut self, path: &Path, color: Rgba) {
        if path.is_empty() || color.alpha <= 0.0 {
            return;
        }
        self.save();
        self.alpha_state(color.alpha);
        self.color(color.rgb, "rg");
        path.write(&mut self.buf);
        self.buf.extend_from_slice(b"f\n");
        self.restore();
    }

    pub(crate) fn stroke(&mut self, path: &Path, stroke: &Stroke) {
        if path.is_empty() || stroke.color.alpha <= 0.0 {
            return;
        }
        self.save();
        self.alpha_state(stroke.color.alpha);
        self.color(stroke.color.rgb, "RG");
        let _ = writeln!(self.buf, "{} w", n(stroke.width.max(0.0)));
        if !stroke.dash.is_empty() {
            let dash: Vec<String> = stroke.dash.iter().map(|d| n(d.max(0.1))).collect();
            let _ = writeln!(self.buf, "[{}] 0 d", dash.join(" "));
        }
        path.write(&mut self.buf);
        self.buf.extend_from_slice(b"S\n");
        self.restore();
    }

    /// Fill `path` with a gradient across the box `(0, 0, w, h)` of the current transform.
    pub(crate) fn gradient(&mut self, path: &Path, stops: &[(f64, Rgba)], kind: &GradientKind, w: f64, h: f64) {
        if path.is_empty() || stops.len() < 2 {
            return;
        }
        let mut functions = Vec::new();
        let mut bounds = Vec::new();
        let mut encode = Vec::new();
        for pair in stops.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            let mut f = Dict::new();
            f.set(b"FunctionType".to_vec(), Object::Int(2));
            f.set(b"Domain".to_vec(), Object::Array(vec![Object::Int(0), Object::Int(1)]));
            f.set(b"C0".to_vec(), Object::Array(a.1.rgb.iter().map(|v| Object::Real(*v)).collect()));
            f.set(b"C1".to_vec(), Object::Array(b.1.rgb.iter().map(|v| Object::Real(*v)).collect()));
            f.set(b"N".to_vec(), Object::Int(1));
            functions.push(Object::Dict(f));
            bounds.push(Object::Real(b.0));
            encode.extend([Object::Int(0), Object::Int(1)]);
        }
        bounds.pop();
        let mut stitched = Dict::new();
        stitched.set(b"FunctionType".to_vec(), Object::Int(3));
        stitched.set(b"Domain".to_vec(), Object::Array(vec![Object::Int(0), Object::Int(1)]));
        stitched.set(b"Functions".to_vec(), Object::Array(functions));
        stitched.set(b"Bounds".to_vec(), Object::Array(bounds));
        stitched.set(b"Encode".to_vec(), Object::Array(encode));
        let mut shading = Dict::new();
        shading.set(b"ColorSpace".to_vec(), Object::name("DeviceRGB"));
        shading.set(b"Function".to_vec(), Object::Dict(stitched));
        shading.set(b"Extend".to_vec(), Object::Array(vec![Object::Bool(true), Object::Bool(true)]));
        let reals = |values: &[f64]| Object::Array(values.iter().map(|v| Object::Real(*v)).collect());
        match kind {
            GradientKind::Linear(angle) => {
                let (s, c) = angle.to_radians().sin_cos();
                let half = (w * c.abs() + h * s.abs()) / 2.0;
                let (cx, cy) = (w / 2.0, h / 2.0);
                shading.set(b"ShadingType".to_vec(), Object::Int(2));
                shading.set(b"Coords".to_vec(), reals(&[cx - c * half, cy - s * half, cx + c * half, cy + s * half]));
            }
            GradientKind::Radial => {
                shading.set(b"ShadingType".to_vec(), Object::Int(3));
                shading.set(b"Coords".to_vec(), reals(&[w / 2.0, h / 2.0, 0.0, w / 2.0, h / 2.0, (w * w + h * h).sqrt() / 2.0]));
            }
        }
        self.shadings.push(Object::Dict(shading));
        let name = self.shadings.len() - 1;
        self.save();
        path.write(&mut self.buf);
        self.buf.extend_from_slice(b"W n\n");
        let _ = writeln!(self.buf, "/Sh{name} sh");
        self.restore();
    }

    /// Restrict drawing to `path` until the matching [`restore`](Self::restore).
    pub(crate) fn clip(&mut self, path: &Path) {
        path.write(&mut self.buf);
        self.buf.extend_from_slice(b"W n\n");
    }

    pub(crate) fn clip_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let _ = writeln!(self.buf, "{} {} {} {} re W n", n(x), n(y), n(w), n(h));
    }

    /// Draw image `index` filling the box. `crop` is the fraction cut off each edge
    /// `[left, top, right, bottom]`.
    pub(crate) fn image(&mut self, index: usize, (x, y, w, h): (f64, f64, f64, f64), crop: [f64; 4], alpha: f64) {
        let visible_w = (1.0 - crop[0] - crop[2]).max(0.01);
        let visible_h = (1.0 - crop[1] - crop[3]).max(0.01);
        let (full_w, full_h) = (w / visible_w, h / visible_h);
        let (ix, iy) = (x - crop[0] * full_w, y - crop[1] * full_h);
        if !self.images.contains(&index) {
            self.images.push(index);
        }
        self.save();
        self.alpha_state(alpha);
        if crop.iter().any(|c| *c != 0.0) {
            self.clip_rect(x, y, w, h);
        }
        // The page is y-down, an image's unit square is y-up: flip it back.
        let _ = writeln!(self.buf, "{} 0 0 {} {} {} cm /Im{index} Do", n(full_w), n(-full_h), n(ix), n(iy + full_h));
        self.restore();
    }

    /// Like [`text`](Self::text), with the glyphs squeezed or stretched horizontally to `hscale` (1 = normal).
    pub(crate) fn text_scaled(&mut self, font: &str, size: f64, (x, y): (f64, f64), hscale: f64, color: Rgba, encoded: &[u8]) {
        self.save();
        self.alpha_state(color.alpha);
        self.color(color.rgb, "rg");
        let _ = write!(self.buf, "BT /{font} {} Tf {} Tz ", n(size), n(hscale * 100.0));
        let _ = write!(self.buf, "1 0 0 -1 {} {} Tm ", n(x), n(y));
        self.buf.extend(karnoxpdf_fonts::literal(encoded));
        self.buf.extend_from_slice(
            b" Tj ET
",
        );
        self.restore();
    }

    /// Show `text` (already WinAnsi-encoded) with its baseline origin at `(x, y)`.
    pub(crate) fn text(&mut self, font: &str, size: f64, (x, y): (f64, f64), word_space: f64, color: Rgba, encoded: &[u8]) {
        self.save();
        self.alpha_state(color.alpha);
        self.color(color.rgb, "rg");
        let _ = write!(self.buf, "BT /{font} {} Tf ", n(size));
        if word_space != 0.0 {
            let _ = write!(self.buf, "{} Tw ", n(word_space));
        }
        // The page is y-down: flip the glyphs back upright.
        let _ = write!(self.buf, "1 0 0 -1 {} {} Tm ", n(x), n(y));
        self.buf.extend(karnoxpdf_fonts::literal(encoded));
        self.buf.extend_from_slice(b" Tj ET\n");
        self.restore();
    }

    pub(crate) fn finish(self) -> (Vec<u8>, Resources) {
        (self.buf, Resources { alphas: self.alphas, images: self.images, shadings: self.shadings })
    }
}

/// The resources a slide's stream names: opacity states, image and shading indices.
pub(crate) struct Resources {
    alphas: Vec<u32>,
    pub images: Vec<usize>,
    shadings: Vec<Object>,
}

impl Resources {
    pub(crate) fn extgstate(&self) -> Dict {
        let mut out = Dict::new();
        for (i, permille) in self.alphas.iter().enumerate() {
            let mut gs = Dict::new();
            gs.set(b"Type".to_vec(), Object::name("ExtGState"));
            gs.set(b"ca".to_vec(), Object::Real(*permille as f64 / 1000.0));
            gs.set(b"CA".to_vec(), Object::Real(*permille as f64 / 1000.0));
            out.set(format!("A{i}").into_bytes(), Object::Dict(gs));
        }
        out
    }

    pub(crate) fn shading(&self) -> Dict {
        let mut out = Dict::new();
        for (i, shading) in self.shadings.iter().enumerate() {
            out.set(format!("Sh{i}").into_bytes(), shading.clone());
        }
        out
    }

    pub(crate) fn has_alpha(&self) -> bool {
        !self.alphas.is_empty()
    }

    pub(crate) fn has_shading(&self) -> bool {
        !self.shadings.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_short_and_finite() {
        assert_eq!(n(1.0), "1");
        assert_eq!(n(0.5), "0.5");
        assert_eq!(n(-0.0001), "0");
        assert_eq!(n(f64::NAN), "0");
        assert_eq!(n(12.34567), "12.346");
    }

    #[test]
    fn rotation_is_clockwise_on_the_slide() {
        // A quarter turn about the origin sends +x to +y, which is downwards on a y-down page.
        let m = Matrix::rotate_about(90.0, 0.0, 0.0).0;
        assert!((m[0]).abs() < 1e-9 && (m[1] - 1.0).abs() < 1e-9);
        // The centre stays put.
        let m = Matrix::rotate_about(37.0, 10.0, 20.0).0;
        let (x, y) = (m[0] * 10.0 + m[2] * 20.0 + m[4], m[1] * 10.0 + m[3] * 20.0 + m[5]);
        assert!((x - 10.0).abs() < 1e-9 && (y - 20.0).abs() < 1e-9);
    }

    #[test]
    fn arcs_end_where_the_sweep_says() {
        // Quarter circle of radius 10 from angle 0 (the point (10, 0) from the centre) sweeping
        // 90° clockwise ends at the bottom of the circle on a y-down page.
        let mut p = Path::new();
        p.move_to(10.0, 0.0);
        p.arc_to(10.0, 10.0, 0.0, 90.0);
        assert!((p.current.0 - 0.0).abs() < 1e-9 && (p.current.1 - 10.0).abs() < 1e-9, "{:?}", p.current);
        // A full ellipse returns to its start.
        let mut p = Path::new();
        p.move_to(30.0, 0.0);
        p.arc_to(30.0, 10.0, 0.0, 360.0);
        assert!((p.current.0 - 30.0).abs() < 1e-9 && p.current.1.abs() < 1e-9);
    }

    #[test]
    fn images_are_flipped_and_cropped() {
        let mut c = Canvas::new(100.0);
        c.image(3, (10.0, 20.0, 40.0, 30.0), [0.25, 0.0, 0.25, 0.0], 1.0);
        let (bytes, res) = c.finish();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("10 20 40 30 re W n"), "{text}");
        assert!(text.contains("80 0 0 -30 -10 50 cm /Im3 Do"), "{text}");
        assert_eq!(res.images, vec![3]);
    }
}
