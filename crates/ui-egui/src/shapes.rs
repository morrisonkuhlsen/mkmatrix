//! The shapes of a formula being typed, as MK Matrix reads them: each range, named matrix and array
//! a rows×columns block, carried through `'`, `@`, `\`, elementwise operators and the `MX.`
//! functions, so the formula bar can say what the result will be (`→ 3×1`) or why the dimensions
//! don't fit (`X@y: 8×3 @ 8×1, 3 vs 8`) before Enter.

use mkmatrix_engine::core::CellRef;
use mkmatrix_engine::formula::{BinOp, Expr, UnOp};
use mkmatrix_engine::model::{Sheet, Workbook};

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

/// The shapes a product or solve at the top of formula text meets, `3×8 @ 8×1`, so the bar can
/// show how the result's shape comes about; `None` for any other formula, or when a shape isn't
/// known.
pub fn operands(text: &str, wb: &Workbook, si: usize) -> Option<String> {
    let body = text.strip_prefix('=')?;
    let expr = mkmatrix_engine::formula::parse_closing_parens(body, mkmatrix_engine::formula::parse).ok()?;
    let matrices = crate::matrices::on_sheet(wb, si);
    let mut top = &expr;
    while let Expr::Paren(x) = top {
        top = x;
    }
    let (op, a, b) = match top {
        Expr::Binary(BinOp::MatMul, a, b) => ("@", a.as_ref(), b.as_ref()),
        Expr::Binary(BinOp::Solve, a, b) => ("\\", a.as_ref(), b.as_ref()),
        Expr::Call(name, args) if name == "MX.MUL" || name == "MX.SOLVE" => match args.as_slice() {
            [a, b] => (if name == "MX.MUL" { "@" } else { "\\" }, a, b),
            _ => return None,
        },
        _ => return None,
    };
    let mut depth = 0;
    let (Ok(Some(x)), Ok(Some(y))) = (shape(a, &matrices, &mut depth), shape(b, &matrices, &mut depth)) else { return None };
    Some(format!("{} {op} {}", fmt(x), fmt(y)))
}

/// A caution for formula text whose operators act entry by entry on matrices, where someone from
/// MATLAB or a textbook may mean the matrix operation (#30): `X^-1` on a square matrix inverts
/// each entry (`MX.INV(X)` is the inverse), and `A*B` between two matrices multiplies entry by entry
/// (`A@B` is the product). Only where a named matrix or a matrix operation is involved: `*` and
/// `^` over plain ranges are the spreadsheet's own, as in Excel. Worded in `lang`.
pub fn caution(text: &str, wb: &Workbook, si: usize, lang: Language) -> Option<String> {
    let body = text.strip_prefix('=')?;
    let expr = mkmatrix_engine::formula::parse_closing_parens(body, mkmatrix_engine::formula::parse).ok()?;
    let matrices = crate::matrices::on_sheet(wb, si);
    let matrix_like = |e: &Expr| {
        let mut found = has_matrix(e);
        e.walk(&mut |x| found |= matches!(x, Expr::Name(n) if matrices.iter().any(|(m, _)| mkmatrix_engine::formula::same_name(m, n))));
        found
    };
    let known = |e: &Expr| {
        let mut depth = 0;
        shape(e, &matrices, &mut depth).ok().flatten()
    };
    let minus_one = |e: &Expr| match e {
        Expr::Number(n) => *n == -1.0,
        Expr::Unary(UnOp::Neg, x) => matches!(x.as_ref(), Expr::Number(n) if *n == 1.0),
        _ => false,
    };
    let mut found = None;
    expr.walk(&mut |x| {
        if found.is_some() {
            return;
        }
        match x {
            Expr::Binary(BinOp::Pow, a, b) if minus_one(b) && matrix_like(a) => {
                if let Some((r, c)) = known(a)
                    && r == c
                    && r > 1
                {
                    // `MX.INV(X'@X)` for `(X'@X)^-1`: the call's own parentheses do.
                    let mut inner = a.as_ref();
                    while let Expr::Paren(x) = inner {
                        inner = x;
                    }
                    let (a, m) = (mkmatrix_engine::formula::print(a), mkmatrix_engine::formula::print(inner));
                    found = Some(lang.tr("{a}^-1 inverts each entry: MX.INV({m}) is the inverse").replace("{a}", &a).replace("{m}", &m));
                }
            }
            Expr::Binary(BinOp::Mul, a, b) if matrix_like(a) || matrix_like(b) => {
                if let (Some(x), Some(y)) = (known(a), known(b))
                    && x != (1, 1)
                    && y != (1, 1)
                {
                    let (a, b) = (mkmatrix_engine::formula::print(a), mkmatrix_engine::formula::print(b));
                    found = Some(lang.tr("{a}*{b} multiplies entry by entry: {a}@{b} is the matrix product").replace("{a}", &a).replace("{b}", &b));
                }
            }
            _ => {}
        }
    });
    found
}

/// The first cell a result of `shape` spilling from `at` would land on that holds something of
/// its own (a value, a formula, another formula's spill): what would make it `#SPILL!`. The
/// result's own spill, from an earlier edit of the same cell, is no obstacle. Past
/// [`MAX_BLOCK_CHECK`] cells nothing is checked.
pub fn blocked(sh: &Sheet, at: CellRef, (rows, cols): Shape) -> Option<CellRef> {
    if u64::from(rows) * u64::from(cols) > MAX_BLOCK_CHECK {
        return None;
    }
    let last_row = at.row.checked_add(rows.checked_sub(1)?)?.min(mkmatrix_engine::core::MAX_ROWS - 1);
    let last_col = at.col.checked_add(cols.checked_sub(1)?)?.min(mkmatrix_engine::core::MAX_COLS - 1);
    (at.row..=last_row).flat_map(|row| (at.col..=last_col).map(move |col| CellRef::new(row, col))).find(|&c| {
        if c == at {
            return false;
        }
        let own = sh.cell(c).is_some_and(|cell| cell.formula.is_some() || !cell.value.is_empty());
        own || sh.spill_anchor(c).is_some_and(|a| a != at)
    })
}

/// The largest result whose landing place is checked for obstacles, in cells.
const MAX_BLOCK_CHECK: u64 = 65_536;

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
    fn products_and_solves_show_their_operands() {
        let ops = |t: &str| operands(t, &book(), 0);
        assert_eq!(ops("=X'@y").as_deref(), Some("3×8 @ 8×1"));
        assert_eq!(ops("=(X\\y)").as_deref(), Some("8×3 \\ 8×1"));
        assert_eq!(ops("=MX.MUL(X',X)").as_deref(), Some("3×8 @ 8×3"));
        for t in ["=X'", "=2*X", "=SUM(X)", "=Z@y", "hello"] {
            assert_eq!(ops(t), None, "{t}");
        }
    }

    #[test]
    fn a_result_landing_on_contents_is_blocked() {
        let mut wb = book();
        let at = |a1: &str| CellRef::parse(a1).unwrap_or(CellRef::new(0, 0));
        if let Some(sh) = wb.sheet_mut(0) {
            sh.set_value(at("H3"), mkmatrix_engine::core::Value::Number(1.0));
            sh.set_value(at("H1"), mkmatrix_engine::core::Value::Text("typed here".into()));
        }
        let Some(sh) = wb.sheet(0) else { panic!("a sheet") };
        assert_eq!(blocked(sh, at("H1"), (3, 1)), Some(at("H3")), "a value in the way");
        assert_eq!(blocked(sh, at("H1"), (2, 1)), None, "the cell typed in doesn't count");
        assert_eq!(blocked(sh, at("J1"), (5, 5)), None);
        assert_eq!(blocked(sh, at("A1"), (100_000, 100)), None, "too big to check");
        assert_eq!(blocked(sh, at("A1"), (0, 3)), None);
    }

    #[test]
    fn entrywise_operators_on_matrices_say_what_they_do() {
        let say = |t: &str| caution(t, &book(), 0, Language::En);
        assert_eq!(say("=(X'@X)^-1").as_deref(), Some("(X'@X)^-1 inverts each entry: MX.INV(X'@X) is the inverse"));
        assert_eq!(say("=X'*X").as_deref(), Some("X'*X multiplies entry by entry: X'@X is the matrix product"));
        assert_eq!(say("=X*X").as_deref(), Some("X*X multiplies entry by entry: X@X is the matrix product"));
        assert!(caution("=X^-1", &book(), 0, Language::PtBr).is_none(), "X is 8×3, not square: nothing to invert");
        // The spreadsheet's own: plain ranges, a scalar, other powers, other operators.
        for t in ["=B7:D14*B7:D14", "=2*X", "=X*2", "=(X'@X)^2", "=X+X", "=SUM(X)", "=A1^-1", "hello"] {
            assert_eq!(say(t), None, "{t}");
        }
        let pt = caution("=X'*X", &book(), 0, Language::PtBr).unwrap_or_default();
        assert!(pt.contains("elemento a elemento") && pt.contains("X'@X"), "{pt}");
    }

    #[test]
    fn plain_formulas_say_nothing() {
        for f in ["=1+2", "=SUM(A1:A9)", "=A1", "hello", "=SUM(", "=Z@Z"] {
            assert_eq!(v(f), None, "{f}");
        }
    }
}
