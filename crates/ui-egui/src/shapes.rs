//! The shapes of a formula being typed, as MK Matrix reads them: each range, named matrix and array
//! a rows×columns block, carried through `'`, `@`, `\`, elementwise operators and the `MX.`
//! functions, so the formula bar can say what the result will be (`→ 3×1`) or why the dimensions
//! don't fit (`X@y: 8×3 @ 8×1, 3 vs 8`) before Enter.

use mkmatrix_engine::core::{CellRef, RangeRef, Value};
use mkmatrix_engine::formula::SheetSel;
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
    let env = Env { matrices: crate::matrices::on_sheet(wb, si), sheet: wb.sheet(si) };
    let mut depth = 0;
    match shape(&expr, &env, &mut depth) {
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

/// The largest matrix whose cells are read to tell whether it is symmetric: the preview runs
/// every frame while a formula is typed.
const MAX_READ_SIDE: u32 = 64;

/// What a formula's shapes are read against: the sheet's named matrices and its cells.
struct Env<'a> {
    matrices: Vec<(String, RangeRef)>,
    sheet: Option<&'a Sheet>,
}

impl Env<'_> {
    fn matrix(&self, name: &str) -> Option<RangeRef> {
        self.matrices.iter().find(|(m, _)| mkmatrix_engine::formula::same_name(m, name)).map(|(_, r)| *r)
    }
}

/// The shape two elementwise operands broadcast to, as the engine does: a side of 1 stretches
/// to the other's; `None` when two sides differ and neither is 1 (the engine pads with #N/A).
fn broadcast(x: Shape, y: Shape) -> Option<Shape> {
    let side = |a: u32, b: u32| {
        if a == b || b == 1 {
            Some(a)
        } else if a == 1 {
            Some(b)
        } else {
            None
        }
    };
    Some((side(x.0, y.0)?, side(x.1, y.1)?))
}

/// `Ok(Some(shape))` when known, `Ok(None)` when not (a name that isn't a matrix, most functions),
/// `Err(why)` when two shapes can't combine.
fn shape(e: &Expr, env: &Env, depth: &mut usize) -> Result<Option<Shape>, Why> {
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
        Expr::Name(n) => Ok(env.matrix(n).map(|r| (r.height(), r.width()))),
        Expr::Paren(x) => shape(x, env, depth),
        Expr::Unary(UnOp::Transpose, x) => Ok(shape(x, env, depth)?.map(|(r, c)| (c, r))),
        Expr::Unary(UnOp::Neg | UnOp::Plus | UnOp::Percent, x) => shape(x, env, depth),
        Expr::Binary(op @ (BinOp::MatMul | BinOp::Solve), a, b) => {
            let (Some(x), Some(y)) = (shape(a, env, depth)?, shape(b, env, depth)?) else { return Ok(None) };
            let (sym, ok, out, inner) =
                if *op == BinOp::MatMul { ("@", x.1 == y.0, (x.0, y.1), (x.1, y.0)) } else { ("\\", x.0 == y.0, (x.1, y.1), (x.0, y.0)) };
            if ok { Ok(Some(out)) } else { Err(Why::Inner { what: mkmatrix_engine::formula::print(e), a: x, op: sym, b: y, n: inner.0, m: inner.1 }) }
        }
        Expr::Binary(BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Pow, a, b) => {
            let (Some(x), Some(y)) = (shape(a, env, depth)?, shape(b, env, depth)?) else { return Ok(None) };
            match broadcast(x, y) {
                Some(s) => Ok(Some(s)),
                None => Err(Why::Sizes { what: mkmatrix_engine::formula::print(e), a: x, b: y }),
            }
        }
        Expr::Call(name, args) => call(name, args, env, depth),
        _ => Ok(None),
    };
    *depth -= 1;
    result
}

fn call(name: &str, args: &[Expr], env: &Env, depth: &mut usize) -> Result<Option<Shape>, Why> {
    let arg = |i: usize, depth: &mut usize| -> Result<Option<Shape>, Why> { args.get(i).map_or(Ok(None), |a| shape(a, env, depth)) };
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
        // A symmetric matrix's eigenvalues are real, one column; any other may be complex, which
        // adds a column of imaginary parts that only its values decide.
        "MX.EIG" => match arg(0, depth)? {
            Some(s) => square(s, "MX.EIG")?.filter(|_| args.first().is_some_and(|a| symmetric(a, env))).map(|(n, _)| (n, 1)),
            None => None,
        },
        "MX.EIGVEC" | "MX.CHOL" => match arg(0, depth)? {
            Some(s) => square(s, name)?,
            None => None,
        },
        // The thin decomposition: k = min(rows, columns) singular values, U rows×k, V columns×k.
        "MX.SVD" => match (arg(0, depth)?, part(args.get(1), "S")) {
            (Some((r, c)), Some(p)) => match p.as_str() {
                "S" => Some((r.min(c), 1)),
                "U" => Some((r, r.min(c))),
                "V" => Some((c, r.min(c))),
                _ => None,
            },
            _ => None,
        },
        "MX.DET" | "MX.TRACE" => match arg(0, depth)? {
            Some(s) => square(s, name)?.map(|_| (1, 1)),
            None => None,
        },
        "MX.RANK" | "MX.NORM" | "MX.COND" => Some((1, 1)),
        _ => None,
    })
}

/// An optional text argument (`part`), upper-cased and trimmed, as the function reads it; `None`
/// when it isn't literal text.
fn part(e: Option<&Expr>, default: &str) -> Option<String> {
    match e {
        None => Some(default.to_owned()),
        Some(Expr::Text(t)) => Some(t.trim().to_ascii_uppercase()),
        Some(_) => None,
    }
}

/// A literal number (`2`, `-2`).
fn literal(e: &Expr) -> Option<f64> {
    match e {
        Expr::Number(n) => Some(*n),
        Expr::Unary(UnOp::Neg, x) => literal(x).map(|n| -n),
        Expr::Unary(UnOp::Plus, x) | Expr::Paren(x) => literal(x),
        _ => None,
    }
}

/// `e` without a transpose (`P'` or `MX.T(P)` gives `P`).
fn untransposed(e: &Expr) -> Option<&Expr> {
    match e {
        Expr::Paren(x) => untransposed(x),
        Expr::Unary(UnOp::Transpose, x) => Some(x),
        Expr::Call(name, args) if name == "MX.T" && args.len() == 1 => args.first(),
        _ => None,
    }
}

/// Whether `e` is surely symmetric: `P'@P` or `P@P'` whatever `P` holds, sums and scalar
/// multiples of symmetric matrices, or a small block of numbers that is (to the engine's
/// tolerance). `false` when unsure.
fn symmetric(e: &Expr, env: &Env) -> bool {
    let same = |a: &Expr, b: &Expr| mkmatrix_engine::formula::print(a) == mkmatrix_engine::formula::print(b);
    let gram = |a: &Expr, b: &Expr| untransposed(a).is_some_and(|p| same(p, b)) || untransposed(b).is_some_and(|p| same(a, p));
    match e {
        Expr::Paren(x) | Expr::Unary(UnOp::Neg | UnOp::Plus, x) => symmetric(x, env),
        Expr::Binary(BinOp::MatMul, a, b) => gram(a, b),
        Expr::Call(name, args) if name == "MX.MUL" => matches!(args.as_slice(), [a, b] if gram(a, b)),
        Expr::Binary(BinOp::Add | BinOp::Sub, a, b) => symmetric(a, env) && symmetric(b, env),
        Expr::Binary(BinOp::Mul, a, b) => (symmetric(a, env) && literal(b).is_some()) || (literal(a).is_some() && symmetric(b, env)),
        Expr::Binary(BinOp::Div, a, b) => symmetric(a, env) && literal(b).is_some(),
        Expr::Array(rows) => {
            let n = rows.len();
            let cell = |r: usize, c: usize| rows.get(r).and_then(|row| row.get(c)).and_then(literal);
            n > 0 && rows.iter().all(|row| row.len() == n) && symmetric_values(n, &|r, c| cell(r, c))
        }
        Expr::Ref(r) if r.sheet == SheetSel::Current => block_symmetric(r.range(), env),
        Expr::Name(n) => env.matrix(n).is_some_and(|r| block_symmetric(r, env)),
        _ => false,
    }
}

/// Whether a square block of the sheet's cells holds numbers that are symmetric.
fn block_symmetric(range: RangeRef, env: &Env) -> bool {
    let Some(sheet) = env.sheet else { return false };
    let n = range.height();
    if n != range.width() || n > MAX_READ_SIDE {
        return false;
    }
    let cell = |r: usize, c: usize| match sheet.value(CellRef { row: range.start.row + r as u32, col: range.start.col + c as u32 }) {
        Value::Number(x) => Some(x),
        _ => None,
    };
    symmetric_values(n as usize, &cell)
}

/// Whether an n×n block of numbers is symmetric to the engine's tolerance (relative to its
/// largest element); `false` when any element isn't a number.
fn symmetric_values(n: usize, cell: &dyn Fn(usize, usize) -> Option<f64>) -> bool {
    let mut scale = 0.0f64;
    for r in 0..n {
        for c in 0..n {
            match cell(r, c) {
                Some(x) if x.is_finite() => scale = scale.max(x.abs()),
                _ => return false,
            }
        }
    }
    let tol = 1e-12 * scale;
    (0..n).all(|r| (r + 1..n).all(|c| cell(r, c).zip(cell(c, r)).is_some_and(|(x, y)| (x - y).abs() <= tol)))
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
        assert_eq!(v("=X+{1,2}"), Some(Verdict::Mismatch("X+{1,2}: 8×3 vs 1×2".into())));
        assert!(matches!(v("=MX.EIG(X)"), Some(Verdict::Mismatch(m)) if m.contains("isn't square")));
    }

    /// #21: a row or column stretches to the other operand's size, as the engine does.
    #[test]
    fn elementwise_operators_broadcast() {
        assert_eq!(v("=X+y"), Some(Verdict::Fits((8, 3))));
        assert_eq!(v("=y+X"), Some(Verdict::Fits((8, 3))));
        assert_eq!(v("=X-{1,2,3}"), Some(Verdict::Fits((8, 3))));
        assert_eq!(v("=y*{1,2,3}"), Some(Verdict::Fits((8, 3))), "a column against a row: the outer table");
        assert_eq!(v("=X/2"), Some(Verdict::Fits((8, 3))));
    }

    /// #21: MX.SVD's parts are the thin decomposition's.
    #[test]
    fn svd_parts_have_their_own_shapes() {
        assert_eq!(v("=MX.SVD(X)"), Some(Verdict::Fits((3, 1))));
        assert_eq!(v("=MX.SVD(X,\"s\")"), Some(Verdict::Fits((3, 1))));
        assert_eq!(v("=MX.SVD(X,\"U\")"), Some(Verdict::Fits((8, 3))));
        assert_eq!(v("=MX.SVD(X,\" v \")"), Some(Verdict::Fits((3, 3))));
        assert_eq!(v("=MX.SVD(X',\"U\")"), Some(Verdict::Fits((3, 3))));
        assert_eq!(v("=MX.SVD(X',\"V\")"), Some(Verdict::Fits((8, 3))));
        assert_eq!(v("=MX.SVD(X,A1)"), None, "a part only known once computed");
    }

    /// #21: MX.EIG is one column only when its eigenvalues are surely real.
    #[test]
    fn eigenvalues_are_one_column_only_for_a_symmetric_matrix() {
        assert_eq!(v("=MX.EIG(X'@X)"), Some(Verdict::Fits((3, 1))));
        assert_eq!(v("=MX.EIG(X@X')"), Some(Verdict::Fits((8, 1))));
        assert_eq!(v("=MX.EIG(MX.MUL(MX.T(X),X))"), Some(Verdict::Fits((3, 1))));
        assert_eq!(v("=MX.EIG(2*(X'@X)+X'@X)"), Some(Verdict::Fits((3, 1))));
        assert_eq!(v("=MX.EIG({2,1;1,2})"), Some(Verdict::Fits((2, 1))));
        // A rotation's eigenvalues are ±i: two columns, so no column count is promised.
        assert_eq!(v("=MX.EIG({0,-1;1,0})"), None);
        assert_eq!(v("=MX.EIG(X'@y@y'@X)"), None, "symmetric, but not in a form the preview knows");
    }

    #[test]
    fn eigenvalues_of_cells_read_their_values() {
        let mut wb = book();
        let sheet = wb.sheet_mut(0).unwrap();
        for (cell, x) in [("A1", 1.0), ("B1", 2.0), ("A2", 2.0), ("B2", 1.0), ("D1", 0.0), ("E1", -1.0), ("D2", 1.0), ("E2", 0.0)] {
            sheet.set_value(CellRef::parse(cell).unwrap(), Value::Number(x));
        }
        let at = |text: &str| of_formula(text, &wb, 0, Language::En);
        assert_eq!(at("=MX.EIG(A1:B2)"), Some(Verdict::Fits((2, 1))));
        assert_eq!(at("=MX.EIG(D1:E2)"), None);
        assert_eq!(at("=MX.EIG(G1:H2)"), None, "empty cells: not known to be symmetric");
    }

    #[test]
    fn mismatches_are_worded_in_the_interface_language() {
        let pt = |text: &str| of_formula(text, &book(), 0, Language::PtBr);
        assert_eq!(pt("=X@y"), Some(Verdict::Mismatch("X@y: 8×3 @ 8×1, 3 contra 8".into())));
        assert_eq!(pt("=MX.INV(X)"), Some(Verdict::Mismatch("MX.INV: 8×3 não é quadrada".into())));
        assert_eq!(pt("=X+{1,2}"), Some(Verdict::Mismatch("X+{1,2}: 8×3 contra 1×2".into())));
    }

    #[test]
    fn plain_formulas_say_nothing() {
        for f in ["=1+2", "=SUM(A1:A9)", "=A1", "hello", "=SUM(", "=Z@Z"] {
            assert_eq!(v(f), None, "{f}");
        }
    }
}
