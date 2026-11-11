//! Edit a PDF ▸ edit existing vector graphics: the filled and stroked paths a page draws in its
//! own content streams (rectangles, lines, polygons, curves), and changing one: move, resize,
//! rotate or flip it (its path coordinates are rewritten, so a stroke keeps its width), change
//! its fill or stroke colour or its line width, or delete it.
//!
//! One *shape* is one painting operator (`f`, `S`, `B`, ...) with the path it paints. Only the
//! stream that draws it is rewritten, as a new object. Colour changes are wrapped in `q … Q` so
//! they never reach the shapes after it; a shape that also sets a clipping path (`W`) keeps its
//! clip and cannot be restyled, only moved or deleted. Graphics inside form XObjects, shadings,
//! patterns and inline images are not shapes of the page.

use karnoxpdf_content::{Matrix, Op, parse, serialize_ops};
use karnoxpdf_cos::{Dict, Document, Object, Stream};

use crate::EditError;
use crate::images::{page_of, streams};

/// How a shape is coloured (a gradient, pattern or special colour space reads as `Other`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Paint {
    Rgb([f64; 3]),
    Other,
}

/// A path painted on a page.
#[derive(Clone, Debug, PartialEq)]
pub struct PageShape {
    /// The path's box in user space (the geometry only; a stroke extends half a line width beyond).
    pub rect: [f64; 4],
    /// `None` when the shape is not filled / not stroked.
    pub fill: Option<Paint>,
    pub stroke: Option<Paint>,
    /// The stroke width as drawn, in user-space points.
    pub line_width: f64,
    pub even_odd: bool,
    /// The path flattened into polygons in user space, for hit tests.
    pub polys: Vec<Vec<[f64; 2]>>,
    /// The shape also sets a clipping path.
    pub clips: bool,
    stream: usize,
    first: usize,
    last: usize,
    ctm: [f64; 6],
}

#[derive(Clone, Copy, Debug)]
enum Seg {
    Move(f64, f64),
    Line(f64, f64),
    Curve([f64; 6]),
    Close,
}

#[derive(Clone, Copy)]
struct State {
    ctm: Matrix,
    fill: Option<Paint>,
    stroke: Option<Paint>,
    width: f64,
}

fn cmyk(c: f64, m: f64, y: f64, k: f64) -> [f64; 3] {
    [(1.0 - c.min(1.0)) * (1.0 - k.min(1.0)), (1.0 - m.min(1.0)) * (1.0 - k.min(1.0)), (1.0 - y.min(1.0)) * (1.0 - k.min(1.0))]
}

/// A colour operator's operands as a paint: gray, RGB or CMYK numbers; anything else is `Other`.
fn paint_of(op: &Op) -> Paint {
    let numbers: Vec<f64> = op.operands.iter().filter_map(Object::as_f64).collect();
    if numbers.len() != op.operands.len() {
        return Paint::Other;
    }
    match numbers.as_slice() {
        [g] => Paint::Rgb([*g; 3]),
        [r, g, b] => Paint::Rgb([*r, *g, *b]),
        [c, m, y, k] => Paint::Rgb(cmyk(*c, *m, *y, *k)),
        _ => Paint::Other,
    }
}

/// Points of a flattened Bézier curve (8 steps).
fn flatten_curve(p0: (f64, f64), c: [f64; 6]) -> Vec<(f64, f64)> {
    (1..=8)
        .map(|i| {
            let t = i as f64 / 8.0;
            let u = 1.0 - t;
            let (a, b, cc, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            (a * p0.0 + b * c[0] + cc * c[2] + d * c[4], a * p0.1 + b * c[1] + cc * c[3] + d * c[5])
        })
        .collect()
}

/// The path's subpaths in user space, flattened.
fn polygons(segs: &[Seg], ctm: &Matrix) -> Vec<Vec<[f64; 2]>> {
    let mut polys: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut current: Vec<[f64; 2]> = Vec::new();
    let mut at = (0.0, 0.0);
    let mut start = (0.0, 0.0);
    let push = |p: (f64, f64), current: &mut Vec<[f64; 2]>| {
        let (x, y) = ctm.apply(p.0, p.1);
        current.push([x, y]);
    };
    for seg in segs {
        match *seg {
            Seg::Move(x, y) => {
                if current.len() > 1 {
                    polys.push(std::mem::take(&mut current));
                } else {
                    current.clear();
                }
                at = (x, y);
                start = at;
                push(at, &mut current);
            }
            Seg::Line(x, y) => {
                if current.is_empty() {
                    push(at, &mut current);
                }
                at = (x, y);
                push(at, &mut current);
            }
            Seg::Curve(c) => {
                if current.is_empty() {
                    push(at, &mut current);
                }
                for p in flatten_curve(at, c) {
                    push(p, &mut current);
                }
                at = (c[4], c[5]);
            }
            Seg::Close => {
                if let Some(first) = current.first().copied() {
                    current.push(first);
                }
                at = start;
            }
        }
    }
    if current.len() > 1 {
        polys.push(current);
    }
    polys
}

fn bbox_of(polys: &[Vec<[f64; 2]>]) -> Option<[f64; 4]> {
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    let mut any = false;
    for p in polys.iter().flatten() {
        if p[0].is_finite() && p[1].is_finite() {
            b = [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])];
            any = true;
        }
    }
    any.then_some(b)
}

impl PageShape {
    /// Whether the point `(x, y)` (user space) is on the shape: inside its filled area, or within
    /// `tolerance` points of its stroke.
    pub fn hit(&self, x: f64, y: f64, tolerance: f64) -> bool {
        if self.fill.is_some() && self.inside(x, y) {
            return true;
        }
        if self.stroke.is_some() {
            let reach = self.line_width / 2.0 + tolerance;
            for poly in &self.polys {
                for w in poly.windows(2) {
                    if distance_to_segment([x, y], w[0], w[1]) <= reach {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn inside(&self, x: f64, y: f64) -> bool {
        let (mut winding, mut crossings) = (0i32, 0u32);
        for poly in &self.polys {
            let n = poly.len();
            for i in 0..n {
                let (a, b) = (poly[i], poly[(i + 1) % n]);
                if (a[1] <= y) != (b[1] <= y) {
                    let t = (y - a[1]) / (b[1] - a[1]);
                    if a[0] + t * (b[0] - a[0]) > x {
                        crossings += 1;
                        winding += if b[1] > a[1] { 1 } else { -1 };
                    }
                }
            }
        }
        if self.even_odd { crossings % 2 == 1 } else { winding != 0 }
    }
}

fn distance_to_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 <= f64::EPSILON { 0.0 } else { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0) };
    ((p[0] - (a[0] + t * dx)).powi(2) + (p[1] - (a[1] + t * dy)).powi(2)).sqrt()
}

fn path_segment(op: &Op) -> Option<Vec<Seg>> {
    Some(match op.op.as_slice() {
        b"m" => {
            let [x, y] = op.nums::<2>()?;
            vec![Seg::Move(x, y)]
        }
        b"l" => {
            let [x, y] = op.nums::<2>()?;
            vec![Seg::Line(x, y)]
        }
        b"c" => vec![Seg::Curve(op.nums::<6>()?)],
        // `v`: the first control point is the current point; `y`: the second is the end point.
        b"v" => {
            let [x2, y2, x3, y3] = op.nums::<4>()?;
            vec![Seg::Curve([f64::NAN, f64::NAN, x2, y2, x3, y3])]
        }
        b"y" => {
            let [x1, y1, x3, y3] = op.nums::<4>()?;
            vec![Seg::Curve([x1, y1, x3, y3, x3, y3])]
        }
        b"h" => vec![Seg::Close],
        b"re" => {
            let [x, y, w, h] = op.nums::<4>()?;
            vec![Seg::Move(x, y), Seg::Line(x + w, y), Seg::Line(x + w, y + h), Seg::Line(x, y + h), Seg::Close]
        }
        _ => return None,
    })
}

/// Replace the NaN placeholders of a `v` curve (first control point = current point).
fn resolve_v(segs: &mut [Seg]) {
    let mut at = (0.0, 0.0);
    let mut start = (0.0, 0.0);
    for seg in segs {
        match seg {
            Seg::Move(x, y) => {
                at = (*x, *y);
                start = at;
            }
            Seg::Line(x, y) => at = (*x, *y),
            Seg::Curve(c) => {
                if c[0].is_nan() {
                    c[0] = at.0;
                    c[1] = at.1;
                }
                at = (c[4], c[5]);
            }
            Seg::Close => at = start,
        }
    }
}

/// The shapes page `page` (0-based) paints, in drawing order.
pub fn page_shapes(doc: &Document, page: usize) -> Result<Vec<PageShape>, EditError> {
    let p = page_of(doc, page)?;
    let mut out = Vec::new();
    let mut state = State { ctm: Matrix::IDENTITY, fill: Some(Paint::Rgb([0.0; 3])), stroke: Some(Paint::Rgb([0.0; 3])), width: 1.0 };
    let mut stack: Vec<State> = Vec::new();
    for (si, (_, data)) in streams(doc, &p.dict).into_iter().enumerate() {
        let ops = parse(&data).ops;
        let mut segs: Vec<Seg> = Vec::new();
        let mut first: Option<usize> = None;
        let mut clips = false;
        for (i, op) in ops.iter().enumerate() {
            match op.op.as_slice() {
                b"q" => stack.push(state),
                b"Q" => {
                    if let Some(s) = stack.pop() {
                        state = s;
                    }
                }
                b"cm" => {
                    if let Some(m) = op.nums::<6>() {
                        state.ctm = Matrix(m).then(&state.ctm);
                    }
                }
                b"w" => {
                    if let Some([w]) = op.nums::<1>() {
                        state.width = w;
                    }
                }
                b"g" | b"rg" | b"k" => state.fill = Some(paint_of(op)),
                b"G" | b"RG" | b"K" => state.stroke = Some(paint_of(op)),
                b"sc" | b"scn" => state.fill = Some(paint_of(op)),
                b"SC" | b"SCN" => state.stroke = Some(paint_of(op)),
                b"cs" => state.fill = Some(Paint::Other),
                b"CS" => state.stroke = Some(Paint::Other),
                b"W" | b"W*" => clips = true,
                b"n" => {
                    segs.clear();
                    first = None;
                    clips = false;
                }
                b"S" | b"s" | b"f" | b"F" | b"f*" | b"B" | b"B*" | b"b" | b"b*" => {
                    let name = op.op.as_slice();
                    let (fills, strokes) = (!matches!(name, b"S" | b"s"), !matches!(name, b"f" | b"F" | b"f*"));
                    if let Some(start) = first.take()
                        && !segs.is_empty()
                    {
                        let mut path = std::mem::take(&mut segs);
                        resolve_v(&mut path);
                        let polys = polygons(&path, &state.ctm);
                        if let Some(rect) = bbox_of(&polys) {
                            let scale = (state.ctm.0[0] * state.ctm.0[3] - state.ctm.0[1] * state.ctm.0[2]).abs().sqrt();
                            out.push(PageShape {
                                rect,
                                fill: fills.then_some(state.fill).flatten(),
                                stroke: strokes.then_some(state.stroke).flatten(),
                                line_width: state.width * scale,
                                even_odd: matches!(name, b"f*" | b"B*" | b"b*"),
                                polys,
                                clips,
                                stream: si,
                                first: start,
                                last: i,
                                ctm: state.ctm.0,
                            });
                        }
                    }
                    segs.clear();
                    first = None;
                    clips = false;
                }
                _ => {
                    if let Some(new) = path_segment(op) {
                        first.get_or_insert(i);
                        segs.extend(new);
                    }
                }
            }
        }
        // A path left open at the end of a stream is not painted.
    }
    Ok(out)
}

/// What to do with a shape.
#[derive(Clone, Debug, PartialEq)]
pub enum ShapeChange {
    /// Apply this user-space transform to the shape (move, resize, rotate, flip).
    Transform([f64; 6]),
    /// Fill with this colour, or stop filling (`None`).
    Fill(Option<[f64; 3]>),
    /// Stroke with this colour, or stop stroking (`None`).
    Stroke(Option<[f64; 3]>),
    /// Stroke width in the shape's own coordinates.
    LineWidth(f64),
    Delete,
}

fn point_op(m: &Matrix, x: f64, y: f64) -> (f64, f64) {
    m.apply(x, y)
}

/// The path operators transformed by `m`; `re` stays a rectangle while `m` only scales and moves.
fn transform_path(ops: &[Op], m: &Matrix) -> Vec<Op> {
    let axis_aligned = m.0[1].abs() < 1e-9 && m.0[2].abs() < 1e-9;
    let n = karnoxpdf_content::num;
    let pair = |x: f64, y: f64| {
        let (a, b) = point_op(m, x, y);
        [n(a), n(b)]
    };
    ops.iter()
        .flat_map(|op| -> Vec<Op> {
            match op.op.as_slice() {
                b"m" | b"l" => match op.nums::<2>() {
                    Some([x, y]) => vec![Op::new(std::str::from_utf8(&op.op).unwrap_or("l"), pair(x, y).to_vec())],
                    None => vec![op.clone()],
                },
                b"c" => match op.nums::<6>() {
                    Some([a, b, c, d, e, f]) => vec![Op::new("c", [pair(a, b), pair(c, d), pair(e, f)].concat())],
                    None => vec![op.clone()],
                },
                b"v" | b"y" => match op.nums::<4>() {
                    Some([a, b, c, d]) => vec![Op::new(std::str::from_utf8(&op.op).unwrap_or("v"), [pair(a, b), pair(c, d)].concat())],
                    None => vec![op.clone()],
                },
                b"re" => match op.nums::<4>() {
                    Some([x, y, w, h]) if axis_aligned => {
                        let ((x0, y0), (x1, y1)) = (point_op(m, x, y), point_op(m, x + w, y + h));
                        vec![Op::new("re", vec![n(x0.min(x1)), n(y0.min(y1)), n((x1 - x0).abs()), n((y1 - y0).abs())])]
                    }
                    Some([x, y, w, h]) => vec![
                        Op::new("m", pair(x, y).to_vec()),
                        Op::new("l", pair(x + w, y).to_vec()),
                        Op::new("l", pair(x + w, y + h).to_vec()),
                        Op::new("l", pair(x, y + h).to_vec()),
                        Op::new("h", vec![]),
                    ],
                    None => vec![op.clone()],
                },
                _ => vec![op.clone()],
            }
        })
        .collect()
}

/// The painting operator for the given combination.
fn paint_op(fills: bool, strokes: bool, even_odd: bool) -> &'static str {
    match (fills, strokes, even_odd) {
        (true, true, false) => "B",
        (true, true, true) => "B*",
        (true, false, false) => "f",
        (true, false, true) => "f*",
        (false, true, _) => "S",
        (false, false, _) => "n",
    }
}

fn color_op(stroke: bool, c: [f64; 3]) -> Op {
    Op::new(if stroke { "RG" } else { "rg" }, c.iter().map(|v| karnoxpdf_content::num(v.clamp(0.0, 1.0))).collect())
}

/// Change shape `index` (from [`page_shapes`]) on `page`.
pub fn change_shape(doc: &mut Document, page: usize, index: usize, change: &ShapeChange) -> Result<(), EditError> {
    let shapes = page_shapes(doc, page)?;
    let shape = shapes.get(index).cloned().ok_or_else(|| EditError::Invalid(format!("page {} has no shape {}", page + 1, index + 1)))?;
    let p = page_of(doc, page)?;
    let all = streams(doc, &p.dict);
    let (obj, data) = all.get(shape.stream).cloned().ok_or_else(|| EditError::Invalid("the shape's content stream is gone".into()))?;
    let mut ops = parse(&data).ops;
    let range = shape.first..=shape.last;
    let group: Vec<Op> = ops.get(range.clone()).map(<[Op]>::to_vec).ok_or_else(|| EditError::Invalid("the shape moved in its stream".into()))?;
    let (path, tail): (Vec<Op>, Vec<Op>) = group.iter().cloned().partition(|op| path_segment(op).is_some());
    let paint = tail.last().cloned().ok_or_else(|| EditError::Invalid("the shape has no painting operator".into()))?;
    let closes = matches!(paint.op.as_slice(), b"s" | b"b" | b"b*");
    let state_ops: Vec<Op> = tail[..tail.len() - 1].to_vec();
    let (fills, strokes) = (shape.fill.is_some(), shape.stroke.is_some());
    let restyle = |new_fill: Option<Option<[f64; 3]>>, new_stroke: Option<Option<[f64; 3]>>, width: Option<f64>| -> Result<Vec<Op>, EditError> {
        if shape.clips {
            return Err(EditError::Invalid("a shape that also sets a clipping path can only be moved or deleted".into()));
        }
        let fill_on = new_fill.map_or(fills, |c| c.is_some());
        let stroke_on = new_stroke.map_or(strokes, |c| c.is_some());
        let mut out = vec![Op::new("q", vec![])];
        out.extend(path.iter().cloned());
        // Colour and width operators that sat between the path and its painting stay, and our changes come after them.
        out.extend(state_ops.iter().cloned());
        if closes {
            out.push(Op::new("h", vec![]));
        }
        if let Some(Some(c)) = new_fill {
            out.push(color_op(false, c));
        }
        if let Some(Some(c)) = new_stroke {
            out.push(color_op(true, c));
        }
        if let Some(w) = width {
            out.push(Op::new("w", vec![karnoxpdf_content::num(w.max(0.0))]));
        }
        out.push(Op::new(paint_op(fill_on, stroke_on, shape.even_odd), vec![]));
        out.push(Op::new("Q", vec![]));
        Ok(out)
    };
    let replacement: Vec<Op> = match change {
        ShapeChange::Delete => {
            // A clip stays in force: only the painting goes. Operators that changed the graphics state
            // between the path and its painting are kept, since later shapes may rely on them.
            if shape.clips {
                let mut kept = group.clone();
                if let Some(last) = kept.last_mut() {
                    *last = Op::new("n", vec![]);
                }
                kept
            } else {
                state_ops.clone()
            }
        }
        ShapeChange::Transform(t) => {
            // The path is in the shape's own space: M' = C · T · C⁻¹ moves it as T moves the page.
            let c = Matrix(shape.ctm);
            let inv = c.invert().ok_or_else(|| EditError::Invalid("the shape has no area".into()))?;
            let m = c.then(&Matrix(*t)).then(&inv);
            if !group.iter().take(path.len()).all(|op| path_segment(op).is_some()) {
                return Err(EditError::Invalid("the shape's path is interleaved with other operators".into()));
            }
            let mut out = transform_path(&path, &m);
            out.extend(tail.iter().cloned());
            out
        }
        ShapeChange::Fill(c) => restyle(Some(*c), None, None)?,
        ShapeChange::Stroke(c) => restyle(None, Some(*c), None)?,
        ShapeChange::LineWidth(w) => {
            if !w.is_finite() {
                return Err(EditError::Invalid("the line width is not a number".into()));
            }
            restyle(None, None, Some(*w))?
        }
    };
    ops.splice(range, replacement);
    let mut dict = match &*doc.resolve(&obj) {
        Object::Stream(s) => s.dict.clone(),
        _ => Dict::new(),
    };
    dict.remove(b"Length");
    let new = doc.add(Object::Stream(Stream::flate(dict, &serialize_ops(&ops))));
    let contents: Vec<Object> = all.iter().enumerate().map(|(i, (o, _))| if i == shape.stream { Object::Ref(new) } else { o.clone() }).collect();
    doc.update_dict(p.obj, |d| {
        d.set(b"Contents".to_vec(), if contents.len() == 1 { contents[0].clone() } else { Object::Array(contents) });
    })?;
    Ok(())
}

/// The user-space transform that fits box `from` onto box `to`. A box with no width or no height
/// (a horizontal or vertical line) keeps that dimension and is only moved along it.
pub fn fit_to(from: [f64; 4], to: [f64; 4]) -> [f64; 6] {
    let axis = |f0: f64, f1: f64, t0: f64, t1: f64| -> (f64, f64) {
        if (f1 - f0).abs() < 1e-6 {
            (1.0, (t0 + t1) / 2.0 - (f0 + f1) / 2.0)
        } else {
            let s = (t1 - t0) / (f1 - f0);
            (s, t0 - f0 * s)
        }
    };
    let (sx, tx) = axis(from[0], from[2], to[0], to[2]);
    let (sy, ty) = axis(from[1], from[3], to[1], to[3]);
    [sx, 0.0, 0.0, sy, tx, ty]
}
