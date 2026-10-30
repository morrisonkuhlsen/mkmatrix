//! The shapes of a formula being typed, as MK Matrix reads them: each range, named matrix and array
//! a rows×columns block, carried through `'`, `@`, `\`, elementwise operators and the `MX.`
//! functions, so the formula bar can say what the result will be (`→ 3×1`) or why the dimensions
//! don't fit (`X@y: 8×3 @ 8×1, 3 vs 8`) before Enter.

use mkmatrix_engine::formula::{BinOp, Expr, UnOp};
use mkmatrix_engine::model::Workbook;

use crate::i18n::Language;

/// A block's rows and columns.
pub type Shape = (u32, u32);

/// What the shapes say of a formula: its result's shape, a mismatch to explain, or nothing known.
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    Fits(Shape),
    Mismatch(String),
}

/// The verdict for formula text (with its `=`) on sheet `si`, a mismatch worded in `lang`; `None`
/// when it isn't a formula, doesn't parse yet, or holds nothing whose shape is known beyond a single
/// value.
pub fn of_formula(text: &str, wb: &Workbook, si: usize, lang: Language) -> Option<Verdict> {
    let body = text.strip_prefix('=')?;
    let expr = mkmatrix_engine::formula::parse_closing_parens(body, mkmatrix_engine::formula::parse).ok()?;
    let matrices = crate::matrices::on_sheet(wb, si);
    let mut depth = 0;
    match shape(&expr, &matrices, &mut depth) {
        Ok(Some(s)) if s != (1, 1) || has_matrix(&expr) => Some(Verdict::Fits(s)),
        Ok(_) => None,
        Err(why) => Some(Verdict::Mismatch(why.text(lang))),
    }
}

fn has_matrix(e: &Expr) -> bool {
    let mut found = false;
    e.walk(&mut |x| found |= matches!(x, Expr::Binary(BinOp::MatMul | BinOp::Solve, ..) | Expr::Unary(UnOp::Transpose, _)));
    found
}

fn fmt(s: Shape) -> String {
    format!("{}×{}", s.0, s.1)
}

/// Why two shapes can't combine, worded only once the language is known.
enum Why {
    /// `what`: `a op b` whose inner sizes `n` and `m` differ.
    Inner { what: String, a: Shape, op: &'static str, b: Shape, n: u32, m: u32 },
    /// `what`: elementwise over two shapes that don't broadcast.
    Sizes { what: String, a: Shape, b: Shape },
    /// `what` needs a square matrix.
    NotSquare { what: String, s: Shape },
}

impl Why {
    fn text(&self, lang: Language) -> String {
        match self {
            Why::Inner { what, a, op, b, n, m } => lang
                .tr("{what}: {a} {op} {b}, {n} vs {m}")
                .replace("{what}", what)
                .replace("{a}", &fmt(*a))
                .replace("{op}", op)
                .replace("{b}", &fmt(*b))
                .replace("{n}", &n.to_string())
                .replace("{m}", &m.to_string()),
            Why::Sizes { what, a, b } => lang.tr("{what}: {a} vs {b}").replace("{what}", what).replace("{a}", &fmt(*a)).replace("{b}", &fmt(*b)),
            Why::NotSquare { what, s } => lang.tr("{what}: {shape} isn't square").replace("{what}", what).replace("{shape}", &fmt(*s)),
        }
    }
}

/// The deepest a formula's nesting is followed.
const MAX_DEPTH: usize = 64;

/// `Ok(Some(shape))` when known, `Ok(None)` when not (a name that isn't a matrix, most functions),
/// `Err(why)` when two shapes can't combine.
fn shape(e: &Expr, matrices: &[(String, mkmatrix_engine::core::RangeRef)], depth: &mut usize) -> Result<Option<Shape>, Why> {
    *depth += 1;
    if *depth > MAX_DEPTH {
        return Ok(None);
    }
    let result = match e {
        Expr::Number(_) | Expr::Bool(_) | Expr::Text(_) => Ok(Some((1, 1))),
        Expr::Array(rows) => Ok(Some((rows.len() as u32, rows.first().map_or(0, |r| r.len() as u32)))),
        Expr::Ref(r) => {
            let range = r.range();
            Ok((!range.is_full_rows() && !range.is_full_cols()).then(|| (range.height(), range.width())))
        }
        Expr::Name(n) => Ok(matrices.iter().find(|(m, _)| mkmatrix_engine::formula::same_name(m, n)).map(|(_, r)| (r.height(), r.width()))),
        Expr::Paren(x) => shape(x, matrices, depth),
        Expr::Unary(UnOp::Transpose, x) => Ok(shape(x, matrices, depth)?.map(|(r, c)| (c, r))),
        Expr::Unary(UnOp::Neg | UnOp::Plus | UnOp::Percent, x) => shape(x, matrices, depth),
        Expr::Binary(op @ (BinOp::MatMul | BinOp::Solve), a, b) => {
            let (Some(x), Some(y)) = (shape(a, matrices, depth)?, shape(b, matrices, depth)?) else { return Ok(None) };
            let (sym, ok, out, inner) =
                if *op == BinOp::MatMul { ("@", x.1 == y.0, (x.0, y.1), (x.1, y.0)) } else { ("\\", x.0 == y.0, (x.1, y.1), (x.0, y.0)) };
            if ok { Ok(Some(out)) } else { Err(Why::Inner { what: mkmatrix_engine::formula::print(e), a: x, op: sym, b: y, n: inner.0, m: inner.1 }) }
        }
        Expr::Binary(BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Pow, a, b) => {
            let (Some(x), Some(y)) = (shape(a, matrices, depth)?, shape(b, matrices, depth)?) else { return Ok(None) };
            if x == y || y == (1, 1) {
                Ok(Some(x))
            } else if x == (1, 1) {
                Ok(Some(y))
            } else {
                Err(Why::Sizes { what: mkmatrix_engine::formula::print(e), a: x, b: y })
            }
        }
        Expr::Call(name, args) => call(name, args, matrices, depth),
        _ => Ok(None),
    };
    *depth -= 1;
    result
}

fn call(name: &str, args: &[Expr], matrices: &[(String, mkmatrix_engine::core::RangeRef)], depth: &mut usize) -> Result<Option<Shape>, Why> {
    let arg = |i: usize, depth: &mut usize| -> Result<Option<Shape>, Why> { args.get(i).map_or(Ok(None), |a| shape(a, matrices, depth)) };
    let square = |s: Shape, what: &str| if s.0 == s.1 { Ok(Some(s)) } else { Err(Why::NotSquare { what: what.to_owned(), s }) };
    Ok(match name {
        "MX.T" => arg(0, depth)?.map(|(r, c)| (c, r)),
        "MX.MUL" => match (arg(0, depth)?, arg(1, depth)?) {
            (Some(x), Some(y)) if x.1 == y.0 => Some((x.0, y.1)),
            (Some(x), Some(y)) => return Err(Why::Inner { what: "MX.MUL".into(), a: x, op: "@", b: y, n: x.1, m: y.0 }),
            _ => None,
        },
        "MX.SOLVE" => match (arg(0, depth)?, arg(1, depth)?) {
            (Some(x), Some(y)) if x.0 == y.0 => Some((x.1, y.1)),
            (Some(x), Some(y)) => return Err(Why::Inner { what: "MX.SOLVE".into(), a: x, op: "\\", b: y, n: x.0, m: y.0 }),
            _ => None,
        },
        "MX.INV" => match arg(0, depth)? {
            Some(s) => square(s, "MX.INV")?,
            None => None,
        },
        "MX.POW" => match arg(0, depth)? {
            Some(s) => square(s, "MX.POW")?,
            None => None,
        },
        "MX.PINV" => arg(0, depth)?.map(|(r, c)| (c, r)),
        "MX.EIG" => match arg(0, depth)? {
            Some(s) => square(s, "MX.EIG")?.map(|(n, _)| (n, 1)),
            None => None,
        },
        "MX.EIGVEC" | "MX.CHOL" => match arg(0, depth)? {
            Some(s) => square(s, name)?,
            None => None,
        },
        "MX.SVD" => arg(0, depth)?.map(|(r, c)| (r.min(c), 1)),
        "MX.DET" | "MX.TRACE" => match arg(0, depth)? {
            Some(s) => square(s, name)?.map(|_| (1, 1)),
            None => None,
        },
        "MX.RANK" | "MX.NORM" | "MX.COND" => Some((1, 1)),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mkmatrix_engine::model::DefinedName;

    fn book() -> Workbook {
        let mut wb = Workbook::new();
        for (n, f) in [("X", "Sheet1!$B$7:$D$14"), ("y", "Sheet1!$F$7:$F$14")] {
            wb.names.push(DefinedName { name: n.into(), scope: None, formula: f.into(), comment: String::new(), hidden: false });
        }
        wb
    }

    fn v(text: &str) -> Option<Verdict> {
        of_formula(text, &book(), 0, Language::En)
    }

    #[test]
    fn shapes_flow_through_the_operators() {
        assert_eq!(v("=X'@X"), Some(Verdict::Fits((3, 3))));
        assert_eq!(v("=(X'@X)\\(X'@y)"), Some(Verdict::Fits((3, 1))));
        assert_eq!(v("=X\\y"), Some(Verdict::Fits((3, 1))));
        assert_eq!(v("=MX.INV(X'@X)@X'@y"), Some(Verdict::Fits((3, 1))));
        assert_eq!(v("=y-X@MX.SOLVE(X,y)"), Some(Verdict::Fits((8, 1))));
        assert_eq!(v("=MX.EIG(X'@X"), Some(Verdict::Fits((3, 1))), "a formula still being typed");
        assert_eq!(v("=2*X'"), Some(Verdict::Fits((3, 8))));
        assert_eq!(v("={1,2;3,4}@{1;1}"), Some(Verdict::Fits((2, 1))));
    }

    #[test]
    fn mismatches_say_why() {
        assert_eq!(v("=X@y"), Some(Verdict::Mismatch("X@y: 8×3 @ 8×1, 3 vs 8".into())));
        assert!(matches!(v("=MX.INV(X)"), Some(Verdict::Mismatch(m)) if m.contains("isn't square")));
        assert!(matches!(v("=X+y"), Some(Verdict::Mismatch(_))));
    }

    #[test]
    fn mismatches_are_worded_in_the_interface_language() {
        let pt = |text: &str| of_formula(text, &book(), 0, Language::PtBr);
        assert_eq!(pt("=X@y"), Some(Verdict::Mismatch("X@y: 8×3 @ 8×1, 3 contra 8".into())));
        assert_eq!(pt("=MX.INV(X)"), Some(Verdict::Mismatch("MX.INV: 8×3 não é quadrada".into())));
        assert_eq!(pt("=X+y"), Some(Verdict::Mismatch("X+y: 8×3 contra 8×1".into())));
    }

    #[test]
    fn plain_formulas_say_nothing() {
        for f in ["=1+2", "=SUM(A1:A9)", "=A1", "hello", "=SUM(", "=Z@Z"] {
            assert_eq!(v(f), None, "{f}");
        }
    }
}
