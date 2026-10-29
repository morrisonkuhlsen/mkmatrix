//! A formula in mathematical notation: `a/b` as a fraction, `^` as an exponent, `SQRT` under a
//! radical, `MX.INV(A)` as A⁻¹, `A'` as Aᵀ, `A\b` as A⁻¹b, `{1,2;3,4}` as a bracketed matrix,
//! `SUM` as Σ. The formula's tree becomes a tree of boxes ([`Node`]); each is measured (width,
//! ascent above the baseline, descent below) and painted at a baseline.
//!
//! Formulas are user input: nesting past [`MAX_DEPTH`] stops turning into notation and shows the
//! rest as formula text, and the painting is all bounded by the tree it was given.

use egui::{Color32, FontFamily, FontId, Painter, Pos2, Stroke, pos2, vec2};
use mkmatrix_engine::formula::{BinOp, Expr, UnOp};

/// How deep the formula turns into notation; deeper parts stay formula text.
const MAX_DEPTH: usize = 40;
/// The smallest type an exponent or a nested fraction shrinks to.
const MIN_SIZE: f32 = 8.0;

/// One box of the notation.
#[derive(Clone, Debug)]
pub enum Node {
    /// A run of text; `italic`-looking names are drawn in the serif face like the rest.
    Text(String),
    Row(Vec<Node>),
    Frac(Box<Node>, Box<Node>),
    /// Base and its superscript.
    Sup(Box<Node>, Box<Node>),
    /// Base and its subscript.
    Sub(Box<Node>, Box<Node>),
    Sqrt(Box<Node>),
    /// Contents between round brackets as tall as they are.
    Paren(Box<Node>),
    /// Between single (`|x|`) or double (`‖x‖`) bars.
    Bars(Box<Node>, bool),
    /// Rows of cells between square brackets.
    Matrix(Vec<Vec<Node>>),
    /// A large operator (Σ, Π) before its operand.
    BigOp(&'static str, Box<Node>),
}

fn text(s: impl Into<String>) -> Node {
    Node::Text(s.into())
}

fn row(items: Vec<Node>) -> Node {
    Node::Row(items)
}

/// How tightly an expression binds, to know when it needs brackets inside another.
fn precedence(e: &Expr) -> u8 {
    match e {
        Expr::Binary(op, ..) => match op {
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => 1,
            BinOp::Concat => 2,
            BinOp::Add | BinOp::Sub => 3,
            BinOp::Mul | BinOp::MatMul | BinOp::Solve => 4,
            // A fraction is a box of its own: it never needs brackets around it.
            BinOp::Div => 9,
            BinOp::Pow => 6,
            BinOp::Range | BinOp::Union | BinOp::Intersect => 8,
        },
        Expr::Unary(UnOp::Neg | UnOp::Plus, _) => 5,
        _ => 9,
    }
}

/// `e` as notation, in brackets when it binds less tightly than `parent` needs.
fn child(e: &Expr, parent: u8, depth: usize) -> Node {
    let n = to_node(e, depth);
    if precedence(e) < parent { Node::Paren(Box::new(n)) } else { n }
}

/// The formula `e` as a tree of boxes.
pub fn to_node(e: &Expr, depth: usize) -> Node {
    if depth > MAX_DEPTH {
        return text(mkmatrix_engine::formula::print(e));
    }
    let d = depth + 1;
    match e {
        Expr::Number(n) => text(number(*n)),
        Expr::Text(s) => text(format!("“{s}”")),
        Expr::Bool(b) => text(if *b { "TRUE" } else { "FALSE" }),
        Expr::Error(_) | Expr::Ref(_) | Expr::Name(_) | Expr::Struct(_) | Expr::Missing | Expr::Invoke(..) => {
            text(mkmatrix_engine::formula::print(e))
        }
        Expr::Array(rows) => Node::Matrix(rows.iter().map(|r| r.iter().map(|x| to_node(x, d)).collect()).collect()),
        Expr::Paren(x) => Node::Paren(Box::new(to_node(x, d))),
        Expr::Unary(op, x) => match op {
            UnOp::Neg => row(vec![text("−"), child(x, 5, d)]),
            UnOp::Plus => child(x, 5, d),
            UnOp::Percent => row(vec![child(x, 9, d), text("%")]),
            UnOp::Transpose => Node::Sup(Box::new(base(x, d)), Box::new(text("T"))),
            _ => text(mkmatrix_engine::formula::print(e)),
        },
        Expr::Binary(op, a, b) => {
            let p = precedence(e);
            let infix = |sym: &str| row(vec![child(a, p, d), text(sym), child(b, p + 1, d)]);
            match op {
                BinOp::Add => infix(" + "),
                BinOp::Sub => infix(" − "),
                BinOp::Mul => infix(" · "),
                BinOp::MatMul => row(vec![child(a, p, d), text(" "), child(b, p + 1, d)]),
                BinOp::Div => Node::Frac(Box::new(strip(a, d)), Box::new(strip(b, d))),
                BinOp::Pow => Node::Sup(Box::new(if precedence(a) < 7 { child(a, 7, d) } else { base(a, d) }), Box::new(strip(b, d))),
                BinOp::Solve => row(vec![inverse(base(a, d)), child(b, p + 1, d)]),
                BinOp::Concat => infix(" & "),
                BinOp::Eq => infix(" = "),
                BinOp::Ne => infix(" ≠ "),
                BinOp::Lt => infix(" < "),
                BinOp::Gt => infix(" > "),
                BinOp::Le => infix(" ≤ "),
                BinOp::Ge => infix(" ≥ "),
                BinOp::Range | BinOp::Union | BinOp::Intersect => text(mkmatrix_engine::formula::print(e)),
            }
        }
        Expr::Call(name, args) => call(name, args, d),
    }
}

/// A base that carries a superscript: a range (`A1:B2`) goes in brackets, so `(A1:B2)⁻¹` can't
/// read as `B2⁻¹`.
fn base(e: &Expr, depth: usize) -> Node {
    let n = child(e, 9, depth);
    let range = match e {
        Expr::Ref(_) | Expr::Binary(BinOp::Range, ..) => mkmatrix_engine::formula::print(e).contains(':'),
        _ => false,
    };
    if range { Node::Paren(Box::new(n)) } else { n }
}

/// A fraction's or exponent's part: its own brackets are dropped, the box shows the grouping.
fn strip(e: &Expr, depth: usize) -> Node {
    match e {
        Expr::Paren(x) => to_node(x, depth),
        _ => to_node(e, depth),
    }
}

fn inverse(n: Node) -> Node {
    Node::Sup(Box::new(n), Box::new(text("−1")))
}

fn call(name: &str, args: &[Expr], d: usize) -> Node {
    let arg = |i: usize| args.get(i).map_or_else(|| text(""), |a| child(a, 9, d));
    let sup_base = |i: usize| args.get(i).map_or_else(|| text(""), |a| base(a, d));
    let plain = |i: usize| args.get(i).map_or_else(|| text(""), |a| strip(a, d));
    match (name, args.len()) {
        ("MX.INV", 1) | ("MINVERSE", 1) => inverse(sup_base(0)),
        ("MX.PINV", 1) => Node::Sup(Box::new(sup_base(0)), Box::new(text("+"))),
        ("MX.T", 1) | ("TRANSPOSE", 1) => Node::Sup(Box::new(sup_base(0)), Box::new(text("T"))),
        ("MX.MUL", 2) | ("MMULT", 2) => row(vec![arg(0), text(" "), arg(1)]),
        ("MX.SOLVE", 2) => row(vec![inverse(sup_base(0)), arg(1)]),
        ("MX.DET", 1) | ("MDETERM", 1) => Node::Bars(Box::new(plain(0)), false),
        ("MX.NORM", 1) => Node::Bars(Box::new(plain(0)), true),
        ("MX.NORM", 2) => Node::Sub(Box::new(Node::Bars(Box::new(plain(0)), true)), Box::new(plain(1))),
        ("MX.POW", 2) => Node::Sup(Box::new(sup_base(0)), Box::new(plain(1))),
        ("MX.EYE", 1) | ("MUNIT", 1) => Node::Sub(Box::new(text("I")), Box::new(plain(0))),
        ("MX.TRACE", 1) => row(vec![text("tr "), arg(0)]),
        ("MX.RANK", 1) => row(vec![text("rank "), arg(0)]),
        ("SQRT", 1) => Node::Sqrt(Box::new(plain(0))),
        ("ABS", 1) => Node::Bars(Box::new(plain(0)), false),
        ("POWER", 2) => Node::Sup(Box::new(arg(0)), Box::new(plain(1))),
        ("EXP", 1) => Node::Sup(Box::new(text("e")), Box::new(plain(0))),
        ("PI", 0) => text("π"),
        ("SUM", 1) => Node::BigOp("Σ", Box::new(arg(0))),
        ("PRODUCT", 1) => Node::BigOp("Π", Box::new(arg(0))),
        ("LN", 1) => row(vec![text("ln "), arg(0)]),
        ("SIN" | "COS" | "TAN", 1) => row(vec![text(format!("{} ", name.to_lowercase())), arg(0)]),
        _ => {
            // Any other function as name(arguments).
            let mut items = Vec::new();
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    items.push(text(", "));
                }
                items.push(strip(a, d));
            }
            row(vec![text(name.to_string()), Node::Paren(Box::new(row(items)))])
        }
    }
}

/// A number as a formula would show it: whole numbers without a decimal point.
fn number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 { format!("{}", n as i64) } else { format!("{n}") }
}

/// A measured box: width, and the height above and below its baseline.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Metrics {
    pub width: f32,
    pub ascent: f32,
    pub descent: f32,
}

impl Metrics {
    pub fn height(&self) -> f32 {
        self.ascent + self.descent
    }
}

fn font(size: f32) -> FontId {
    FontId::new(size.max(MIN_SIZE), FontFamily::Name(crate::theme::SERIF.into()))
}

/// The face's ascent and descent at `size` (from a tall glyph's box).
fn line(painter: &Painter, size: f32) -> (f32, f32) {
    let h = painter.layout_no_wrap("Hg".into(), font(size), Color32::WHITE).size().y;
    (h * 0.76, h * 0.24)
}

/// Where the fraction bar sits above the baseline (the maths axis).
fn axis(size: f32) -> f32 {
    size * 0.30
}

pub fn measure(n: &Node, painter: &Painter, size: f32) -> Metrics {
    let size = size.max(MIN_SIZE);
    match n {
        Node::Text(s) => {
            let w = painter.layout_no_wrap(s.clone(), font(size), Color32::WHITE).size().x;
            let (a, d) = line(painter, size);
            Metrics { width: w, ascent: a, descent: d }
        }
        Node::Row(items) => items.iter().fold(Metrics::default(), |m, i| {
            let x = measure(i, painter, size);
            Metrics { width: m.width + x.width, ascent: m.ascent.max(x.ascent), descent: m.descent.max(x.descent) }
        }),
        Node::Frac(a, b) => {
            let small = (size * 0.9).max(MIN_SIZE);
            let (ma, mb) = (measure(a, painter, small), measure(b, painter, small));
            let gap = 2.0;
            Metrics {
                width: ma.width.max(mb.width) + 8.0,
                ascent: axis(size) + gap + ma.height(),
                descent: (mb.height() + gap - axis(size)).max(0.0),
            }
        }
        Node::Sup(base, exp) => {
            let mb = measure(base, painter, size);
            let me = measure(exp, painter, size * 0.68);
            let raise = mb.ascent * 0.55;
            Metrics { width: mb.width + me.width + 1.0, ascent: mb.ascent.max(raise + me.ascent), descent: mb.descent }
        }
        Node::Sub(base, sub) => {
            let mb = measure(base, painter, size);
            let ms = measure(sub, painter, size * 0.68);
            let drop = mb.descent * 0.9 + 2.0;
            Metrics { width: mb.width + ms.width + 1.0, ascent: mb.ascent, descent: mb.descent.max(drop + ms.descent) }
        }
        Node::Sqrt(x) => {
            let m = measure(x, painter, size);
            Metrics { width: m.width + size * 0.7 + 3.0, ascent: m.ascent + 4.0, descent: m.descent + 1.0 }
        }
        Node::Paren(x) | Node::Bars(x, _) => {
            let m = measure(x, painter, size);
            let side = match n {
                Node::Bars(_, true) => size * 0.55,
                Node::Bars(_, false) => size * 0.4,
                _ => size * 0.45,
            };
            Metrics { width: m.width + 2.0 * side, ascent: m.ascent + 1.0, descent: m.descent + 1.0 }
        }
        Node::Matrix(rows) => {
            let (cols, widths, heights) = grid(rows, painter, size);
            let gap_x = size * 0.9;
            let gap_y = size * 0.35;
            let w: f32 = widths.iter().sum::<f32>() + gap_x * cols.saturating_sub(1) as f32;
            let h: f32 = heights.iter().map(|m| m.height()).sum::<f32>() + gap_y * heights.len().saturating_sub(1) as f32;
            // Centred on the axis.
            Metrics { width: w + size * 1.0, ascent: h / 2.0 + axis(size) + 2.0, descent: (h / 2.0 - axis(size)).max(0.0) + 2.0 }
        }
        Node::BigOp(sym, x) => {
            let ms = measure(&text(*sym), painter, size * 1.35);
            let mx = measure(x, painter, size);
            Metrics { width: ms.width + 3.0 + mx.width, ascent: ms.ascent.max(mx.ascent), descent: ms.descent.max(mx.descent) }
        }
    }
}

/// A matrix's column count, each column's width and each row's metrics.
fn grid(rows: &[Vec<Node>], painter: &Painter, size: f32) -> (usize, Vec<f32>, Vec<Metrics>) {
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut widths = vec![0.0_f32; cols];
    let mut heights = Vec::with_capacity(rows.len());
    for r in rows {
        let mut m = Metrics::default();
        for (c, cell) in r.iter().enumerate() {
            let x = measure(cell, painter, size);
            if let Some(w) = widths.get_mut(c) {
                *w = w.max(x.width);
            }
            m.ascent = m.ascent.max(x.ascent);
            m.descent = m.descent.max(x.descent);
        }
        heights.push(m);
    }
    (cols, widths, heights)
}

/// Paints `n` with its baseline at `at` (its left edge at `at.x`).
pub fn paint(n: &Node, painter: &Painter, at: Pos2, size: f32, ink: Color32) {
    let size = size.max(MIN_SIZE);
    let stroke = Stroke::new((size / 14.0).clamp(0.8, 1.6), ink);
    match n {
        Node::Text(s) => {
            let (a, _) = line(painter, size);
            painter.text(pos2(at.x, at.y - a), egui::Align2::LEFT_TOP, s, font(size), ink);
        }
        Node::Row(items) => {
            let mut x = at.x;
            for i in items {
                paint(i, painter, pos2(x, at.y), size, ink);
                x += measure(i, painter, size).width;
            }
        }
        Node::Frac(a, b) => {
            let m = measure(n, painter, size);
            let small = (size * 0.9).max(MIN_SIZE);
            let (ma, mb) = (measure(a, painter, small), measure(b, painter, small));
            let bar = at.y - axis(size);
            painter.line_segment([pos2(at.x + 2.0, bar), pos2(at.x + m.width - 2.0, bar)], stroke);
            paint(a, painter, pos2(at.x + (m.width - ma.width) / 2.0, bar - 2.0 - ma.descent), small, ink);
            paint(b, painter, pos2(at.x + (m.width - mb.width) / 2.0, bar + 2.0 + mb.ascent), small, ink);
        }
        Node::Sup(base, exp) => {
            let mb = measure(base, painter, size);
            paint(base, painter, at, size, ink);
            paint(exp, painter, pos2(at.x + mb.width + 1.0, at.y - mb.ascent * 0.55), size * 0.68, ink);
        }
        Node::Sub(base, sub) => {
            let mb = measure(base, painter, size);
            paint(base, painter, at, size, ink);
            paint(sub, painter, pos2(at.x + mb.width + 1.0, at.y + mb.descent * 0.9 + 2.0), size * 0.68, ink);
        }
        Node::Sqrt(x) => {
            let m = measure(x, painter, size);
            let top = at.y - m.ascent - 3.0;
            let bottom = at.y + m.descent;
            let hook = size * 0.7;
            let pts = [
                pos2(at.x, at.y - size * 0.25),
                pos2(at.x + hook * 0.3, at.y - size * 0.35),
                pos2(at.x + hook * 0.55, bottom),
                pos2(at.x + hook, top),
                pos2(at.x + hook + m.width + 3.0, top),
            ];
            painter.add(egui::Shape::line(pts.to_vec(), stroke));
            paint(x, painter, pos2(at.x + hook + 2.0, at.y), size, ink);
        }
        Node::Paren(x) => {
            let m = measure(x, painter, size);
            let side = size * 0.45;
            let (top, bottom) = (at.y - m.ascent - 1.0, at.y + m.descent + 1.0);
            bracket(painter, at.x + side * 0.75, top, bottom, side * 0.45, true, stroke);
            paint(x, painter, pos2(at.x + side, at.y), size, ink);
            bracket(painter, at.x + side + m.width + side * 0.25, top, bottom, side * 0.45, false, stroke);
        }
        Node::Bars(x, double) => {
            let m = measure(x, painter, size);
            let side = if *double { size * 0.55 } else { size * 0.4 };
            let (top, bottom) = (at.y - m.ascent - 1.0, at.y + m.descent + 1.0);
            let bars = |x0: f32| {
                painter.line_segment([pos2(x0, top), pos2(x0, bottom)], stroke);
                if *double {
                    painter.line_segment([pos2(x0 + 3.0, top), pos2(x0 + 3.0, bottom)], stroke);
                }
            };
            bars(at.x + side * 0.4);
            paint(x, painter, pos2(at.x + side, at.y), size, ink);
            bars(at.x + side + m.width + side * 0.2);
        }
        Node::Matrix(rows) => {
            let m = measure(n, painter, size);
            let (_, widths, heights) = grid(rows, painter, size);
            let gap_x = size * 0.9;
            let gap_y = size * 0.35;
            let (top, bottom) = (at.y - m.ascent, at.y + m.descent);
            let arm = size * 0.25;
            // Square brackets.
            for (x0, dir) in [(at.x + 2.0, 1.0), (at.x + m.width - 2.0, -1.0)] {
                painter
                    .add(egui::Shape::line(vec![pos2(x0 + arm * dir, top), pos2(x0, top), pos2(x0, bottom), pos2(x0 + arm * dir, bottom)], stroke));
            }
            let mut y = top + 2.0;
            for (r, cells) in rows.iter().enumerate() {
                let hm = heights.get(r).copied().unwrap_or_default();
                let mut x = at.x + size * 0.5;
                for (c, cell) in cells.iter().enumerate() {
                    let w = widths.get(c).copied().unwrap_or(0.0);
                    let cm = measure(cell, painter, size);
                    // Numbers line up on the right, as in a matrix.
                    paint(cell, painter, pos2(x + w - cm.width, y + hm.ascent), size, ink);
                    x += w + gap_x;
                }
                y += hm.height() + gap_y;
            }
        }
        Node::BigOp(sym, x) => {
            let ms = measure(&text(*sym), painter, size * 1.35);
            paint(&text(*sym), painter, pos2(at.x, at.y + size * 0.08), size * 1.35, ink);
            paint(x, painter, pos2(at.x + ms.width + 3.0, at.y), size, ink);
        }
    }
}

/// A round bracket from `top` to `bottom` around `x`, bowing out by `bow`.
fn bracket(painter: &Painter, x: f32, top: f32, bottom: f32, bow: f32, left: bool, stroke: Stroke) {
    let dir = if left { -1.0 } else { 1.0 };
    let steps = 12;
    let pts: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let t = i as f32 / steps as f32;
            // A half ellipse: furthest out at the middle.
            let out = (std::f32::consts::PI * t).sin() * bow;
            pos2(x + dir * out, top + (bottom - top) * t)
        })
        .collect();
    painter.add(egui::Shape::line(pts, stroke));
}

/// The formula `text` (with or without its `=`) as notation, or `None` when it doesn't parse.
pub fn from_formula(text: &str) -> Option<Node> {
    let body = text.strip_prefix('=').unwrap_or(text);
    let e = mkmatrix_engine::formula::parse(body).ok()?;
    Some(to_node(&e, 0))
}

/// Draws `node` in a box `size` points tall per line at `origin` (its top-left) and returns its
/// size.
pub fn show(node: &Node, painter: &Painter, origin: Pos2, size: f32, ink: Color32) -> egui::Vec2 {
    let m = measure(node, painter, size);
    paint(node, painter, pos2(origin.x, origin.y + m.ascent), size, ink);
    vec2(m.width, m.height())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(text: &str) -> String {
        format!("{:?}", from_formula(text).unwrap())
    }

    #[test]
    fn matrix_operations_read_as_notation() {
        assert!(shape("=MX.INV(A1:B2)").starts_with("Sup(Paren("), "a range under an exponent is bracketed: {}", shape("=MX.INV(A1:B2)"));
        assert!(shape("=MX.INV(A1:B2)").contains("Text(\"−1\")"));
        assert!(shape("=A1:B2'").contains("Text(\"T\")"));
        assert!(shape("=1/(2+3)").starts_with("Frac("));
        // The fraction's brackets go: the box shows the grouping.
        assert!(!shape("=1/(2+3)").contains("Paren"));
        assert!(shape("=SQRT(2)").starts_with("Sqrt("));
        assert!(shape("={1,2;3,4}").starts_with("Matrix("));
        assert!(shape("=(1+2)*3").contains("Paren"), "a sum inside a product keeps its brackets");
        assert!(shape("=MX.DET(A1:B2)").starts_with("Bars("));
    }

    #[test]
    fn deep_nesting_falls_back_to_text_without_overflowing() {
        // Past MAX_DEPTH (the parser itself refuses far deeper formulas).
        let deep = format!("={}1{}", "SQRT(".repeat(60), ")".repeat(60));
        let n = from_formula(&deep).unwrap();
        assert!(format!("{n:?}").contains("SQRT("), "the deep end stays formula text");
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::theme::pinned_font_definitions());
        for _ in 0..2 {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                let m = measure(&n, ui.painter(), 14.0);
                assert!(m.width.is_finite() && m.width > 0.0);
            });
            out.textures_delta.clear();
        }
    }
}
