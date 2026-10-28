//! "Calculation by hand": the steps that take a matrix formula's operands to its result, as a
//! textbook writes them: transposes, products element by element (cᵢⱼ = Σₖ aᵢₖ bₖⱼ), determinants
//! by cofactors, inverses by Gauss–Jordan, traces and powers; ranks, LU and Cholesky factors
//! ([`factor`]); eigenvalues and eigenvectors ([`eigen`]). Each step is mathematical notation
//! ([`Math`]) that the side panel draws and [`latex`] writes out for a paper.

mod eigen;
mod factor;

use mkmatrix_engine::core::Value;
use mkmatrix_engine::formula::{BinOp, Expr, UnOp};
use mkmatrix_engine::model::Workbook;

/// Mathematical notation, enough for the steps.
#[derive(Clone, Debug, PartialEq)]
pub enum Math {
    /// A variable or matrix name, set in italics.
    Var(String),
    /// A number.
    Num(f64),
    /// Upright text: an operator, a word, punctuation.
    Op(String),
    /// A base with a subscript.
    Sub(Box<Math>, Box<Math>),
    /// A base with a superscript.
    Sup(Box<Math>, Box<Math>),
    /// A matrix in square brackets.
    Matrix(Vec<Vec<Math>>),
    /// Pieces side by side.
    Row(Vec<Math>),
    /// A root: `√x` (index 2) or `∛x` (index 3).
    Root(u8, Box<Math>),
    /// Framed, as a pivot is marked.
    Boxed(Box<Math>),
}

/// One step: what it says, and the notation. The caption is fixed text (translated); `{}` in it is
/// replaced by `detail` (numbers and row operations, never translated).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Step {
    pub caption: String,
    pub detail: String,
    pub math: Math,
}

impl Default for Math {
    fn default() -> Self {
        Math::Row(Vec::new())
    }
}

impl Step {
    fn new(caption: &str, math: Math) -> Step {
        Step { caption: caption.into(), detail: String::new(), math }
    }

    /// The caption as read, translated by `tr`.
    pub fn text(&self, tr: impl Fn(&str) -> String) -> String {
        tr(&self.caption).replace("{}", &self.detail)
    }
}

/// The largest square matrix worked by hand; past it the steps would fill a book.
pub const HAND_MAX: usize = 5;
/// The most elements of a product expanded one by one.
const EXPAND_MAX: usize = 9;
/// The most terms written out in one sum.
const TERMS_MAX: usize = 6;
/// The highest power worked as repeated products.
const POW_MAX: u64 = 8;

type Mat = Vec<Vec<f64>>;

fn op(s: &str) -> Math {
    Math::Op(s.into())
}
fn var(s: &str) -> Math {
    Math::Var(s.into())
}
fn sub(base: Math, i: Math) -> Math {
    Math::Sub(Box::new(base), Box::new(i))
}
fn idx(base: &str, i: usize, j: usize) -> Math {
    sub(var(base), Math::Op(format!("{i}{j}")))
}
fn matrix(m: &Mat) -> Math {
    Math::Matrix(m.iter().map(|r| r.iter().map(|x| Math::Num(*x)).collect()).collect())
}
fn row(v: Vec<Math>) -> Math {
    Math::Row(v)
}
/// `m` with a superscript, in parentheses when it is more than one symbol or already raised
/// (`(XᵀX)⁻¹`, `(A⁻¹)²`).
fn sup(m: Math, s: &str) -> Math {
    let base = if matches!(m, Math::Row(_) | Math::Sup(..)) { row(vec![op("("), m, op(")")]) } else { m };
    Math::Sup(Box::new(base), Box::new(op(s)))
}
fn root(m: Math) -> Math {
    Math::Root(2, Box::new(m))
}
/// `f(x)`: a function's name, upright, and its argument in parentheses.
fn call(f: &str, x: Math) -> Math {
    row(vec![op(f), op("("), x, op(")")])
}
/// `step` with `{}` in its caption filled by `detail`.
fn told(caption: &str, detail: String, math: Math) -> Step {
    Step { caption: caption.into(), detail, math }
}
/// How an operand is named in the steps: a written-out array constant reads as `A`, so formulas
/// about it stay short.
fn symbol(shown: Math) -> Math {
    if matches!(shown, Math::Matrix(_)) { var("A") } else { shown }
}
/// A number after an operator: in parentheses when it is negative (`− (−8)`).
fn signed(x: f64) -> Math {
    if x < 0.0 { row(vec![op("("), Math::Num(x), op(")")]) } else { Math::Num(x) }
}
/// The largest magnitude in `m`, never 0 (a scale for tolerances).
fn magnitude(m: &Mat) -> f64 {
    m.iter().flatten().fold(0.0f64, |acc, x| acc.max(x.abs())).max(1e-300)
}

/// The steps for a cell's formula (with or without `=`) on sheet `si`; `Err(why)` when it isn't an
/// operation worked by hand.
pub fn derive(formula: &str, wb: &Workbook, si: usize) -> Result<Vec<Step>, String> {
    let body = formula.trim().strip_prefix('=').unwrap_or(formula.trim());
    let expr = mkmatrix_engine::formula::parse(body).map_err(|_| "This formula doesn't read.".to_string())?;
    let expr = unparen(&expr);
    let ctx = Ctx { wb, si };
    match expr {
        Expr::Unary(UnOp::Transpose, a) => transpose_steps(&ctx, a),
        Expr::Call(name, args) if name == "MX.T" => transpose_steps(&ctx, args.first().ok_or("MX.T needs a matrix.")?),
        Expr::Binary(BinOp::MatMul, a, b) => product_steps(&ctx, a, b),
        Expr::Call(name, args) if name == "MX.MUL" && args.len() == 2 => product_steps(&ctx, &args[0], &args[1]),
        Expr::Call(name, args) if name == "MX.DET" => det_steps(&ctx, args.first().ok_or("MX.DET needs a matrix.")?),
        Expr::Call(name, args) if name == "MX.INV" => inverse_steps(&ctx, args.first().ok_or("MX.INV needs a matrix.")?),
        Expr::Binary(BinOp::Solve, a, b) => solve_steps(&ctx, a, b),
        Expr::Call(name, args) if name == "MX.SOLVE" && args.len() == 2 => solve_steps(&ctx, &args[0], &args[1]),
        Expr::Call(name, args) if name == "MX.TRACE" => trace_steps(&ctx, args.first().ok_or("MX.TRACE needs a matrix.")?),
        Expr::Call(name, args) if name == "MX.POW" && args.len() == 2 => pow_steps(&ctx, &args[0], &args[1]),
        Expr::Call(name, args) if name == "MX.RANK" && args.len() == 1 => factor::rank_steps(&ctx, &args[0]),
        Expr::Call(name, _) if name == "MX.RANK" => Err("By hand works MX.RANK without a tolerance.".into()),
        Expr::Call(name, args) if name == "MX.LU" && !args.is_empty() => factor::lu_steps(&ctx, &args[0], args.get(1)),
        Expr::Call(name, args) if name == "MX.CHOL" => factor::chol_steps(&ctx, args.first().ok_or("MX.CHOL needs a matrix.")?),
        Expr::Call(name, args) if name == "MX.EIG" => eigen::eig_steps(&ctx, args.first().ok_or("MX.EIG needs a matrix.")?),
        Expr::Call(name, args) if name == "MX.EIGVEC" => eigen::eigvec_steps(&ctx, args.first().ok_or("MX.EIGVEC needs a matrix.")?),
        _ => Err(UNSUPPORTED.into()),
    }
}

/// What the panel works, said when a formula is something else.
pub const UNSUPPORTED: &str =
    "By hand: a transpose (X'), a product (A@B), a system (A\\b), MX.DET, MX.INV, MX.TRACE, MX.POW, MX.RANK, MX.LU, MX.CHOL, MX.EIG or MX.EIGVEC.";

fn unparen(e: &Expr) -> &Expr {
    match e {
        Expr::Paren(x) => unparen(x),
        other => other,
    }
}

struct Ctx<'a> {
    wb: &'a Workbook,
    si: usize,
}

impl Ctx<'_> {
    /// An operand's numbers and how it is written: ranges, named matrices, array constants, and
    /// transposes and products of those.
    fn value(&self, e: &Expr, depth: usize) -> Result<(Mat, Math), String> {
        if depth > 16 {
            return Err("The formula nests too deeply to work by hand.".into());
        }
        match unparen(e) {
            Expr::Ref(r) if r.sheet == mkmatrix_engine::formula::SheetSel::Current => {
                let range = r.range();
                Ok((self.read(range)?, Math::Op(range.a1())))
            }
            Expr::Name(n) => {
                let (_, range) = crate::matrices::on_sheet(self.wb, self.si)
                    .into_iter()
                    .find(|(m, _)| mkmatrix_engine::formula::same_name(m, n))
                    .ok_or_else(|| format!("{n} isn't a matrix on this sheet."))?;
                Ok((self.read(range)?, var(n)))
            }
            Expr::Array(rows) => {
                let m: Option<Mat> = rows.iter().map(|r| r.iter().map(|x| if let Expr::Number(v) = x { Some(*v) } else { None }).collect()).collect();
                let m = m.ok_or("Only numbers in an array constant.")?;
                // Every row as long as the first, so the steps can index them.
                let width = m.first().map_or(0, Vec::len);
                if width == 0 || m.iter().any(|r| r.len() != width) {
                    return Err("An array constant's rows must be the same length.".into());
                }
                let shown = matrix(&m);
                Ok((m, shown))
            }
            Expr::Number(v) => Ok((vec![vec![*v]], Math::Num(*v))),
            Expr::Unary(UnOp::Transpose, a) => {
                let (m, shown) = self.value(a, depth + 1)?;
                Ok((transpose(&m), sup(shown, "T")))
            }
            Expr::Call(name, args) if name == "MX.T" && args.len() == 1 => {
                self.value(&Expr::Unary(UnOp::Transpose, Box::new(args[0].clone())), depth + 1)
            }
            Expr::Call(name, args) if name == "MX.MUL" && args.len() == 2 => {
                self.value(&Expr::Binary(BinOp::MatMul, Box::new(args[0].clone()), Box::new(args[1].clone())), depth + 1)
            }
            Expr::Call(name, args) if name == "MX.INV" && args.len() == 1 => {
                let (m, shown) = self.value(&args[0], depth + 1)?;
                let n = square(&m, "An inverse")?;
                let mut aug = beside(&m, &identity(n));
                eliminate(&mut aug, n, &mut Vec::new()).map_err(|()| "The matrix is singular: it has no inverse (a pivot is 0).".to_string())?;
                Ok((right_of(&aug, n), sup(shown, "−1")))
            }
            Expr::Binary(BinOp::MatMul, a, b) => {
                let (x, xs) = self.value(a, depth + 1)?;
                let (y, ys) = self.value(b, depth + 1)?;
                Ok((mul(&x, &y)?, row(vec![xs, ys])))
            }
            _ => Err("By hand works on ranges, named matrices, array constants, their transposes, products and inverses.".into()),
        }
    }

    fn read(&self, range: mkmatrix_engine::core::RangeRef) -> Result<Mat, String> {
        let sh = self.wb.sheet(self.si).ok_or("No sheet.")?;
        if range.height() as usize > 64 || range.width() as usize > 64 {
            return Err("The matrix is too large to work by hand.".into());
        }
        let mut m = Vec::new();
        for r in range.start.row..=range.end.row {
            let mut line = Vec::new();
            for c in range.start.col..=range.end.col {
                match sh.value(mkmatrix_engine::core::CellRef::new(r, c)) {
                    Value::Number(n) => line.push(n),
                    Value::Empty => line.push(0.0),
                    _ => return Err(format!("{} isn't a number.", mkmatrix_engine::core::CellRef::new(r, c).a1())),
                }
            }
            m.push(line);
        }
        Ok(m)
    }
}

fn transpose(m: &Mat) -> Mat {
    let cols = m.first().map_or(0, Vec::len);
    (0..cols).map(|j| m.iter().map(|r| r.get(j).copied().unwrap_or(0.0)).collect()).collect()
}

fn mul(a: &Mat, b: &Mat) -> Result<Mat, String> {
    let (n, k) = (a.len(), a.first().map_or(0, Vec::len));
    let p = b.first().map_or(0, Vec::len);
    if k != b.len() {
        return Err(format!("{n}×{k} @ {}×{p}: {k} vs {}.", b.len(), b.len()));
    }
    let mut c: Mat = (0..n).map(|i| (0..p).map(|j| (0..k).map(|t| a[i][t] * b[t][j]).sum()).collect()).collect();
    let scale = c.iter().flatten().fold(0.0f64, |acc, x| acc.max(x.abs())).max(1e-300);
    tidy(&mut c, scale);
    Ok(c)
}

fn transpose_steps(ctx: &Ctx, a: &Expr) -> Result<Vec<Step>, String> {
    let (m, shown) = ctx.value(a, 0)?;
    let t = transpose(&m);
    let a_t = Math::Sup(Box::new(shown.clone()), Box::new(op("T")));
    Ok(vec![
        Step { caption: "The matrix".into(), detail: String::new(), math: row(vec![shown.clone(), op("="), matrix(&m)]) },
        Step {
            caption: "Rows become columns".into(),
            detail: String::new(),
            math: row(vec![sub(Math::Row(vec![Math::Op("(".into()), a_t.clone(), Math::Op(")".into())]), op("ij")), op("="), sub(shown, op("ji"))]),
        },
        Step { caption: "The transpose".into(), detail: String::new(), math: row(vec![a_t, op("="), matrix(&t)]) },
    ])
}

fn product_steps(ctx: &Ctx, a: &Expr, b: &Expr) -> Result<Vec<Step>, String> {
    let (x, xs) = ctx.value(a, 0)?;
    let (y, ys) = ctx.value(b, 0)?;
    let c = mul(&x, &y)?;
    let k = y.len();
    let mut steps = vec![
        Step {
            caption: "The factors".into(),
            detail: String::new(),
            math: row(vec![xs.clone(), op("="), matrix(&x), op(","), ys.clone(), op("="), matrix(&y)]),
        },
        Step {
            caption: "Each element is a row times a column".into(),
            detail: String::new(),
            math: row(vec![idx("c", 0, 0).relabel("ij"), op("="), sub(op("Σ"), var("k")), sub(var("a"), op("ik")), sub(var("b"), op("kj"))]),
        },
    ];
    let mut shown = 0;
    'cells: for (i, line) in c.iter().enumerate() {
        for (j, value) in line.iter().enumerate() {
            if shown >= EXPAND_MAX {
                steps.push(Step {
                    caption: "…and so on for the other {} elements".into(),
                    detail: (c.len() * line.len() - shown).to_string(),
                    math: row(vec![]),
                });
                break 'cells;
            }
            let mut terms = vec![idx("c", i + 1, j + 1), op("=")];
            for t in 0..k.min(TERMS_MAX) {
                if t > 0 {
                    terms.push(op("+"));
                }
                terms.push(Math::Num(x[i][t]));
                terms.push(op("·"));
                terms.push(Math::Num(y[t][j]));
            }
            if k > TERMS_MAX {
                terms.push(op("+ …"));
            }
            terms.push(op("="));
            terms.push(Math::Num(*value));
            steps.push(Step { caption: String::new(), detail: String::new(), math: row(terms) });
            shown += 1;
        }
    }
    steps.push(Step { caption: "The product".into(), detail: String::new(), math: row(vec![xs, ys, op("="), matrix(&c)]) });
    Ok(steps)
}

impl Math {
    /// A subscripted name with its subscript replaced (`c₁₁` → `cᵢⱼ`).
    fn relabel(self, s: &str) -> Math {
        match self {
            Math::Sub(b, _) => Math::Sub(b, Box::new(op(s))),
            other => other,
        }
    }
}

fn det(m: &Mat) -> f64 {
    let n = m.len();
    match n {
        0 => 1.0,
        1 => m[0][0],
        2 => m[0][0] * m[1][1] - m[0][1] * m[1][0],
        _ => (0..n).map(|j| sign(j) * m[0][j] * det(&minor(m, 0, j))).sum(),
    }
}

fn sign(j: usize) -> f64 {
    if j.is_multiple_of(2) { 1.0 } else { -1.0 }
}

fn minor(m: &Mat, r: usize, c: usize) -> Mat {
    m.iter()
        .enumerate()
        .filter(|(i, _)| *i != r)
        .map(|(_, line)| line.iter().enumerate().filter(|(j, _)| *j != c).map(|(_, x)| *x).collect())
        .collect()
}

fn square(m: &Mat, what: &str) -> Result<usize, String> {
    let n = m.len();
    if n == 0 || m.iter().any(|r| r.len() != n) {
        return Err(format!("{what} needs a square matrix."));
    }
    if n > HAND_MAX {
        return Err(format!("By hand stops at {HAND_MAX}×{HAND_MAX}; this one is {n}×{n}."));
    }
    Ok(n)
}

fn det_steps(ctx: &Ctx, a: &Expr) -> Result<Vec<Step>, String> {
    let (m, shown) = ctx.value(a, 0)?;
    let n = square(&m, "A determinant")?;
    let d = det(&m);
    let det_of = |x: Math| row(vec![op("det"), op("("), x, op(")")]);
    let mut steps = vec![Step { caption: "The matrix".into(), detail: String::new(), math: row(vec![shown.clone(), op("="), matrix(&m)]) }];
    if n == 2 {
        steps.push(Step {
            caption: "Main diagonal minus the other".into(),
            detail: String::new(),
            math: row(vec![
                det_of(shown),
                op("="),
                Math::Num(m[0][0]),
                op("·"),
                Math::Num(m[1][1]),
                op("−"),
                Math::Num(m[0][1]),
                op("·"),
                Math::Num(m[1][0]),
                op("="),
                Math::Num(d),
            ]),
        });
        return Ok(steps);
    }
    steps.push(Step {
        caption: "Expansion along the first row".into(),
        detail: String::new(),
        math: row(vec![
            det_of(shown.clone()),
            op("="),
            sub(op("Σ"), var("j")),
            Math::Sup(Box::new(op("(−1)")), Box::new(op("1+j"))),
            sub(var("a"), op("1j")),
            det_of(sub(var("M"), op("1j"))),
        ]),
    });
    let mut terms = vec![det_of(shown), op("=")];
    for j in 0..n {
        let minor_m = minor(&m, 0, j);
        let md = det(&minor_m);
        steps.push(Step {
            caption: "Minor of a1{}".into(),
            detail: (j + 1).to_string(),
            math: row(vec![sub(var("M"), op(&format!("1{}", j + 1))), op("="), matrix(&minor_m), op("→"), op("det ="), Math::Num(md)]),
        });
        if j > 0 {
            terms.push(op(if sign(j) > 0.0 { "+" } else { "−" }));
        }
        terms.push(Math::Num(m[0][j]));
        terms.push(op("·"));
        terms.push(Math::Num(md));
    }
    terms.push(op("="));
    terms.push(Math::Num(d));
    steps.push(Step { caption: "Together".into(), detail: String::new(), math: row(terms) });
    Ok(steps)
}

/// Rounding noise read as the number a hand would write: 0, or the integer it is beside.
fn tidy(m: &mut Mat, scale: f64) {
    for x in m.iter_mut().flatten() {
        if x.abs() < 1e-12 * scale.max(1.0) {
            *x = 0.0;
        } else if (*x - x.round()).abs() < 1e-9 * x.abs().max(1.0) {
            *x = x.round();
        }
    }
}

fn identity(n: usize) -> Mat {
    (0..n).map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect()).collect()
}

/// `[a | b]`: each row of `a` followed by the same row of `b`.
fn beside(a: &Mat, b: &Mat) -> Mat {
    a.iter().zip(b).map(|(x, y)| x.iter().chain(y).copied().collect()).collect()
}

/// The columns of `aug` past the first `n`.
fn right_of(aug: &Mat, n: usize) -> Mat {
    aug.iter().map(|r| r.get(n..).map(<[f64]>::to_vec).unwrap_or_default()).collect()
}

/// Gauss–Jordan on the first `n` columns of `aug` (n rows), one step per column with the row
/// operations named; `Err` when a pivot is 0.
fn eliminate(aug: &mut Mat, n: usize, steps: &mut Vec<Step>) -> Result<(), ()> {
    if aug.len() != n || aug.iter().any(|r| r.len() < n) {
        return Err(());
    }
    let scale = aug.iter().flat_map(|r| r.iter().take(n)).fold(0.0f64, |acc, x| acc.max(x.abs())).max(1e-300);
    for col in 0..n {
        // The pivot: the largest in the column, at or below the diagonal.
        let piv = (col..n).max_by(|&p, &q| aug[p][col].abs().total_cmp(&aug[q][col].abs())).unwrap_or(col);
        if aug[piv][col].abs() <= scale * 1e-12 {
            return Err(());
        }
        let mut ops = Vec::new();
        if piv != col {
            aug.swap(piv, col);
            ops.push(format!("R{} ↔ R{}", col + 1, piv + 1));
        }
        let p = aug[col][col];
        if (p - 1.0).abs() > 1e-15 {
            for x in &mut aug[col] {
                *x /= p;
            }
            ops.push(format!("R{} ← R{} / {}", col + 1, col + 1, number(p)));
        }
        for r in 0..n {
            if r == col {
                continue;
            }
            let f = aug[r][col];
            if f.abs() <= 1e-15 {
                continue;
            }
            let pivot_row = aug[col].clone();
            for (x, y) in aug[r].iter_mut().zip(&pivot_row) {
                *x -= f * y;
            }
            ops.push(format!("R{} ← R{} − ({})·R{}", r + 1, r + 1, number(f), col + 1));
        }
        tidy(aug, scale);
        let (caption, detail) = if ops.is_empty() {
            ("Column {}: already in place", (col + 1).to_string())
        } else {
            ("Column {}", format!("{}: {}", col + 1, ops.join(",  ")))
        };
        steps.push(Step { caption: caption.into(), detail, math: matrix(aug) });
    }
    Ok(())
}

fn inverse_steps(ctx: &Ctx, a: &Expr) -> Result<Vec<Step>, String> {
    let (m, shown) = ctx.value(a, 0)?;
    let n = square(&m, "An inverse")?;
    let mut aug = beside(&m, &identity(n));
    let mut steps =
        vec![Step::new("Next to the identity: [A | I]", row(vec![op("["), shown.clone(), op("|"), var("I"), op("]"), op("="), matrix(&aug)]))];
    eliminate(&mut aug, n, &mut steps).map_err(|()| "The matrix is singular: it has no inverse (a pivot is 0).".to_string())?;
    steps.push(Step::new("The right half is the inverse", row(vec![sup(shown, "−1"), op("="), matrix(&right_of(&aug, n))])));
    Ok(steps)
}

fn trace_steps(ctx: &Ctx, a: &Expr) -> Result<Vec<Step>, String> {
    let (m, shown) = ctx.value(a, 0)?;
    let shown = symbol(shown);
    let n = square(&m, "A trace")?;
    let diagonal: Vec<f64> = (0..n).map(|i| m[i][i]).collect();
    let mut terms = vec![call("tr", shown.clone()), op("=")];
    for i in 0..n {
        if i > 0 {
            terms.push(op("+"));
        }
        terms.push(idx("a", i + 1, i + 1));
    }
    terms.push(op("="));
    for (i, x) in diagonal.iter().enumerate() {
        if i > 0 {
            terms.push(op("+"));
        }
        terms.push(Math::Num(*x));
    }
    terms.push(op("="));
    terms.push(Math::Num(diagonal.iter().sum()));
    Ok(vec![Step::new("The matrix", row(vec![shown, op("="), matrix(&m)])), Step::new("The sum of the main diagonal", row(terms))])
}

/// A whole number written in a formula: `3`, `-2`, `(4)`.
fn whole(e: &Expr) -> Option<i64> {
    let v = match unparen(e) {
        Expr::Number(v) => *v,
        Expr::Unary(UnOp::Neg, x) => -whole(x)? as f64,
        _ => return None,
    };
    (v.fract() == 0.0 && v.abs() <= 1e9).then_some(v as i64)
}

/// `MX.POW(A, k)`: repeated products, `Aᵏ = Aᵏ⁻¹·A`; a negative power multiplies the inverse,
/// worked first by Gauss–Jordan.
fn pow_steps(ctx: &Ctx, a: &Expr, k: &Expr) -> Result<Vec<Step>, String> {
    let k = whole(k).ok_or("By hand works MX.POW with a whole number for k.")?;
    if k.unsigned_abs() > POW_MAX {
        return Err(format!("By hand stops at the power {POW_MAX}."));
    }
    let (m, shown) = ctx.value(a, 0)?;
    let shown = symbol(shown);
    let n = square(&m, "A power")?;
    let mut steps = vec![Step::new("The matrix", row(vec![shown.clone(), op("="), matrix(&m)]))];
    if k == 0 {
        steps.push(Step::new(
            "Any matrix to the power 0 is the identity",
            row(vec![sup(shown, "0"), op("="), var("I"), op("="), matrix(&identity(n))]),
        ));
        return Ok(steps);
    }
    let (base, base_s) = if k < 0 {
        let mut aug = beside(&m, &identity(n));
        steps.push(Step::new(
            "A negative power multiplies the inverse: first [A | I]",
            row(vec![op("["), shown.clone(), op("|"), var("I"), op("]"), op("="), matrix(&aug)]),
        ));
        eliminate(&mut aug, n, &mut steps).map_err(|()| "The matrix is singular: it has no inverse (a pivot is 0).".to_string())?;
        let inv = right_of(&aug, n);
        let inv_s = sup(shown, "−1");
        steps.push(Step::new("The right half is the inverse", row(vec![inv_s.clone(), op("="), matrix(&inv)])));
        (inv, inv_s)
    } else {
        (m, shown)
    };
    let power = k.unsigned_abs();
    if power > 1 {
        steps.push(Step::new("Each power is the one before times the matrix", row(vec![])));
    }
    let mut acc = base.clone();
    for p in 2..=power {
        acc = mul(&acc, &base)?;
        let before = if p == 2 { base_s.clone() } else { sup(base_s.clone(), &(p - 1).to_string()) };
        steps.push(Step::new("", row(vec![sup(base_s.clone(), &p.to_string()), op("="), before, base_s.clone(), op("="), matrix(&acc)])));
    }
    if power == 1 {
        steps.push(Step::new("The power", row(vec![base_s, op("="), matrix(&acc)])));
    }
    Ok(steps)
}

/// `A\b`: Gauss–Jordan on `[A | b]` when A is square; with more equations than unknowns, least
/// squares through the normal equations `AᵀA x = Aᵀb` (a regression's coefficients).
fn solve_steps(ctx: &Ctx, a: &Expr, b: &Expr) -> Result<Vec<Step>, String> {
    let (m, ms) = ctx.value(a, 0)?;
    let (r, rs) = ctx.value(b, 0)?;
    let (rows, cols) = (m.len(), m.first().map_or(0, Vec::len));
    if rows != r.len() {
        return Err(format!("{rows}×{cols} \\ {}×{}: {rows} vs {}.", r.len(), r.first().map_or(0, Vec::len), r.len()));
    }
    if rows < cols {
        return Err("Fewer equations than unknowns: the system has no single solution.".into());
    }
    let mut steps = vec![Step::new("The system", row(vec![ms.clone(), var("x"), op("="), rs.clone()]))];
    let (lhs, rhs, lhs_s, rhs_s) = if rows == cols {
        (m.clone(), r.clone(), ms.clone(), rs.clone())
    } else {
        let mt = transpose(&m);
        let (ata, atb) = (mul(&mt, &m)?, mul(&mt, &r)?);
        let (ata_s, atb_s) = (row(vec![sup(ms.clone(), "T"), ms.clone()]), row(vec![sup(ms.clone(), "T"), rs.clone()]));
        steps.push(Step::new(
            "More equations than unknowns: least squares solves the normal equations",
            row(vec![ata_s.clone(), var("x"), op("="), atb_s.clone()]),
        ));
        steps.push(Step::new("The normal matrix", row(vec![ata_s.clone(), op("="), matrix(&ata)])));
        steps.push(Step::new("The right-hand side", row(vec![atb_s.clone(), op("="), matrix(&atb)])));
        (ata, atb, ata_s, atb_s)
    };
    let n = square(&lhs, "A system")?;
    let mut aug = beside(&lhs, &rhs);
    steps.push(Step::new("The augmented matrix", row(vec![op("["), lhs_s, op("|"), rhs_s, op("]"), op("="), matrix(&aug)])));
    eliminate(&mut aug, n, &mut steps).map_err(|()| "The system is singular: it has no single solution (a pivot is 0).".to_string())?;
    steps.push(Step::new("The right part is the solution", row(vec![var("x"), op("="), matrix(&right_of(&aug, n))])));
    Ok(steps)
}

/// A number as the steps write it: integers whole, others to four significant digits.
pub fn number(x: f64) -> String {
    if x == x.round() && x.abs() < 1e15 {
        return format!("{}", x as i64);
    }
    if x.abs() >= 1e6 || x.abs() < 1e-4 {
        return format!("{x:.3e}");
    }
    let digits = (3 - x.abs().log10().floor() as i32).clamp(0, 8) as usize;
    let s = format!("{x:.digits$}");
    if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s }
}

/// The steps as LaTeX (an `align*` of one line per step, each caption a `\text{}` comment).
pub fn latex(steps: &[Step]) -> String {
    let mut out = String::from("\\begin{align*}\n");
    for (i, s) in steps.iter().enumerate() {
        if !s.caption.is_empty() {
            let text = s
                .text(str::to_string)
                .replace('{', "\\{")
                .replace('}', "\\}")
                .replace('←', "$\\leftarrow$")
                .replace('↔', "$\\leftrightarrow$")
                .replace('λ', "$\\lambda$")
                .replace('·', "$\\cdot$")
                .replace('ᵀ', "$^T$")
                .replace('≤', "$\\le$")
                .replace('×', "$\\times$")
                .replace('−', "-");
            out.push_str(&format!("&\\text{{{text}}}\\\\\n"));
        }
        if s.math != Math::Row(vec![]) {
            out.push_str(&format!("&{}", tex(&s.math)));
            if i + 1 < steps.len() {
                out.push_str("\\\\");
            }
            out.push('\n');
        }
    }
    out.push_str("\\end{align*}\n");
    out
}

fn tex(m: &Math) -> String {
    match m {
        Math::Var(v) => v.replace('λ', "\\lambda ").replace('φ', "\\varphi "),
        Math::Num(x) => number(*x).replace('−', "-"),
        Math::Op(s) => match s.as_str() {
            "·" => "\\cdot ".into(),
            "−" => "-".into(),
            "±" => "\\pm ".into(),
            "π" => "\\pi ".into(),
            "Σ" => "\\sum".into(),
            "tr" => "\\operatorname{tr}".into(),
            "cos" | "arccos" => format!("\\{s}"),
            "→" => "\\;\\Rightarrow\\;".into(),
            "+ …" => "+ \\cdots".into(),
            "det" => "\\det".into(),
            "det =" => "\\det =".into(),
            "|" => "\\,|\\,".into(),
            other if other.chars().all(|c| c.is_ascii_alphabetic()) && other.len() > 1 => format!("\\text{{{other}}}"),
            other => other.replace('π', "\\pi ").replace('←', "\\leftarrow ").replace('↔', "\\leftrightarrow ").replace('−', "-"),
        },
        Math::Sub(b, i) => format!("{{{}}}_{{{}}}", tex(b), tex(i)),
        Math::Sup(b, s) => format!("{{{}}}^{{{}}}", tex(b), tex(s)),
        Math::Matrix(rows) => {
            let body: Vec<String> = rows.iter().map(|r| r.iter().map(tex).collect::<Vec<_>>().join(" & ")).collect();
            format!("\\begin{{bmatrix}} {} \\end{{bmatrix}}", body.join(" \\\\ "))
        }
        Math::Row(items) => items.iter().map(tex).collect::<Vec<_>>().join(" "),
        Math::Root(2, x) => format!("\\sqrt{{{}}}", tex(x)),
        Math::Root(i, x) => format!("\\sqrt[{i}]{{{}}}", tex(x)),
        Math::Boxed(x) => format!("\\boxed{{{}}}", tex(x)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book() -> Workbook {
        let mut s = mkmatrix_engine::Session::new();
        s.new_workbook();
        s.execute("range.setValues", serde_json::json!({"range": "A1", "values": [[2, 1], [1, 3]]})).unwrap();
        s.execute("range.setValues", serde_json::json!({"range": "D1", "values": [[1, 2, 3], [0, 1, 4], [5, 6, 0]]})).unwrap();
        s.execute("formulas.defineName", serde_json::json!({"name": "A", "refersTo": "=Sheet1!$A$1:$B$2"})).unwrap();
        (*s.doc().unwrap().wb).clone()
    }

    fn last(steps: &[Step]) -> &Math {
        &steps.last().unwrap().math
    }

    #[test]
    fn a_product_expands_each_element() {
        let steps = derive("=A@A", &book(), 0).unwrap();
        // c11 = 2·2 + 1·1 = 5
        let first = steps.iter().find(|s| matches!(&s.math, Math::Row(v) if v.first() == Some(&idx("c", 1, 1)))).unwrap();
        assert!(matches!(&first.math, Math::Row(v) if v.last() == Some(&Math::Num(5.0))));
        assert!(matches!(last(&steps), Math::Row(v) if v.last() == Some(&matrix(&vec![vec![5.0, 5.0], vec![5.0, 10.0]]))));
    }

    #[test]
    fn determinants_and_inverses_by_hand() {
        let wb = book();
        let d2 = derive("=MX.DET(A)", &wb, 0).unwrap();
        assert!(matches!(last(&d2), Math::Row(v) if v.last() == Some(&Math::Num(5.0))));
        let d3 = derive("=MX.DET(D1:F3)", &wb, 0).unwrap();
        assert!(matches!(last(&d3), Math::Row(v) if v.last() == Some(&Math::Num(1.0))), "det of D1:F3 is 1");
        let inv = derive("=MX.INV(D1:F3)", &wb, 0).unwrap();
        let want = vec![vec![-24.0, 18.0, 5.0], vec![20.0, -15.0, -4.0], vec![-5.0, 4.0, 1.0]];
        assert!(matches!(last(&inv), Math::Row(v) if v.last() == Some(&matrix(&want))), "{:?}", last(&inv));
        assert!(derive("=MX.INV({1,2;2,4})", &wb, 0).unwrap_err().contains("singular"));
    }

    #[test]
    fn transposes_and_unsupported_formulas() {
        let wb = book();
        let t = derive("=D1:F3'", &wb, 0).unwrap();
        assert!(matches!(last(&t), Math::Row(v) if v.last() == Some(&matrix(&vec![vec![1.0, 0.0, 5.0], vec![2.0, 1.0, 6.0], vec![3.0, 4.0, 0.0]]))));
        assert!(derive("=SUM(A1:B2)", &wb, 0).is_err());
        assert!(derive("=A@D1:F3", &wb, 0).unwrap_err().contains("vs"));
    }

    #[test]
    fn systems_by_elimination_and_least_squares() {
        let wb = book();
        // 2x + y = 3, x + 3y = 4 → x = 1, y = 1
        let sq = derive("=A\\{3;4}", &wb, 0).unwrap();
        assert!(matches!(last(&sq), Math::Row(v) if v.last() == Some(&matrix(&vec![vec![1.0], vec![1.0]]))), "{:?}", last(&sq));
        assert_eq!(sq.iter().filter(|s| s.caption.starts_with("Column")).count(), 2);
        assert!(derive("=MX.SOLVE(A,{3;4})", &wb, 0).is_ok());
        // y = 1 + 2t through (0,1), (1,3), (2,5): least squares finds it exactly.
        let ls = derive("={1,0;1,1;1,2}\\{1;3;5}", &wb, 0).unwrap();
        assert!(ls.iter().any(|s| s.caption == "The normal matrix"
            && matches!(&s.math, Math::Row(v) if v.last() == Some(&matrix(&vec![vec![3.0, 3.0], vec![3.0, 5.0]])))));
        assert!(matches!(last(&ls), Math::Row(v) if v.last() == Some(&matrix(&vec![vec![1.0], vec![2.0]]))), "{:?}", last(&ls));
        assert!(derive("={1,2;2,4}\\{1;2}", &wb, 0).unwrap_err().contains("singular"));
        assert!(derive("={1,2,3}\\{1}", &wb, 0).unwrap_err().contains("Fewer"));
    }

    #[test]
    fn inverses_inside_products() {
        // The regression formula (XᵀX)⁻¹Xᵀy works as a product of its parts.
        let steps = derive("=MX.INV({1,0;1,1;1,2}'@{1,0;1,1;1,2})@{1,0;1,1;1,2}'@{1;3;5}", &book(), 0).unwrap();
        assert!(matches!(last(&steps), Math::Row(v) if v.last() == Some(&matrix(&vec![vec![1.0], vec![2.0]]))), "{:?}", last(&steps));
    }

    #[test]
    fn captions_fill_in_their_numbers() {
        let inv = derive("=MX.INV(D1:F3)", &book(), 0).unwrap();
        let col = inv.iter().find(|s| s.caption.starts_with("Column")).unwrap();
        assert!(col.text(|c| c.replace("Column", "Coluna")).starts_with("Coluna 1"));
        assert!(latex(&inv).contains("\\leftarrow"));
    }

    /// What the engine puts in the cells for `formula` (a `rows`×`cols` spill from H10), on the
    /// test book.
    fn engine(formula: &str, rows: u32, cols: u32) -> Mat {
        let mut s = mkmatrix_engine::Session::new();
        s.new_workbook();
        s.execute("range.setValues", serde_json::json!({"range": "A1", "values": [[2, 1], [1, 3]]})).unwrap();
        s.execute("range.setValues", serde_json::json!({"range": "D1", "values": [[1, 2, 3], [0, 1, 4], [5, 6, 0]]})).unwrap();
        s.execute("formulas.defineName", serde_json::json!({"name": "A", "refersTo": "=Sheet1!$A$1:$B$2"})).unwrap();
        s.execute("cell.set", serde_json::json!({"cell": "H10", "input": formula})).unwrap();
        let wb = &s.doc().unwrap().wb;
        let sh = wb.sheet(0).unwrap();
        (9..9 + rows)
            .map(|r| {
                (7..7 + cols)
                    .map(|c| match sh.value(mkmatrix_engine::core::CellRef::new(r, c)) {
                        Value::Number(n) => n,
                        other => panic!("{formula}: {other:?}"),
                    })
                    .collect()
            })
            .collect()
    }

    /// The numbers of the matrix that ends the last step.
    fn result(steps: &[Step]) -> Mat {
        let rows = match last(steps) {
            Math::Matrix(rows) => rows,
            Math::Row(v) => match v.last() {
                Some(Math::Matrix(rows)) => rows,
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        };
        rows.iter().map(|r| r.iter().map(|x| if let Math::Num(n) = x { *n } else { panic!("{x:?}") }).collect()).collect()
    }

    fn close(got: &Mat, want: &Mat) -> bool {
        got.len() == want.len()
            && got.iter().zip(want).all(|(a, b)| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9 * y.abs().max(1.0)))
    }

    fn same_as_engine(formula: &str, rows: u32, cols: u32) -> Vec<Step> {
        let steps = derive(formula, &book(), 0).unwrap_or_else(|e| panic!("{formula}: {e}"));
        let (got, want) = (result(&steps), engine(formula, rows, cols));
        assert!(close(&got, &want), "{formula}: by hand {got:?}, the cell {want:?}");
        steps
    }

    #[test]
    fn traces_and_powers() {
        let wb = book();
        let tr = derive("=MX.TRACE(D1:F3)", &wb, 0).unwrap();
        assert!(matches!(last(&tr), Math::Row(v) if v.last() == Some(&Math::Num(2.0))));
        let p3 = same_as_engine("=MX.POW(A,3)", 2, 2);
        assert_eq!(result(&p3), vec![vec![15.0, 20.0], vec![20.0, 35.0]]);
        same_as_engine("=MX.POW(A,0)", 2, 2);
        same_as_engine("=MX.POW(A,1)", 2, 2);
        same_as_engine("=MX.POW(D1:F3,-2)", 3, 3);
        assert!(derive("=MX.POW(A,9)", &wb, 0).is_err());
        assert!(derive("=MX.POW(A,1.5)", &wb, 0).is_err());
        assert!(derive("=MX.POW({1,2;2,4},-1)", &wb, 0).unwrap_err().contains("singular"));
    }

    #[test]
    fn ranks_by_row_echelon_form() {
        let wb = book();
        let rank = |f: &str| match last(&derive(f, &wb, 0).unwrap()) {
            Math::Row(v) => v.last().cloned(),
            _ => None,
        };
        assert_eq!(rank("=MX.RANK({1,2,3;2,4,6;1,0,1})"), Some(Math::Num(2.0)));
        assert_eq!(rank("=MX.RANK(D1:F3)"), Some(Math::Num(3.0)));
        assert_eq!(rank("=MX.RANK({0,0;0,0})"), Some(Math::Num(0.0)));
        assert_eq!(rank("=MX.RANK({1,2,3,4;2,4,6,8})"), Some(Math::Num(1.0)));
        // The second column has no pivot; the third does.
        let steps = derive("=MX.RANK({1,2,0;2,4,1;3,6,1})", &wb, 0).unwrap();
        assert!(steps.iter().any(|s| s.caption == "Column {}: no pivot" && s.detail == "2"));
        assert!(latex(&steps).contains("\\boxed{"));
        assert!(derive("=MX.RANK(A,0.1)", &wb, 0).unwrap_err().contains("tolerance"));
    }

    #[test]
    fn lu_factors_match_the_cell() {
        for part in ["L", "U", "P"] {
            same_as_engine(&format!("=MX.LU(D1:F3,\"{part}\")"), 3, 3);
            same_as_engine(&format!("=MX.LU({{1,2;3,4}},\"{part}\")"), 2, 2);
            same_as_engine(&format!("=MX.LU({{2,1,1;4,3,3;8,7,9}},\"{part}\")"), 3, 3);
        }
        same_as_engine("=MX.LU(A)", 2, 2);
        // A zero column: no pivot, U keeps the zero.
        same_as_engine("=MX.LU({0,1;0,2},\"U\")", 2, 2);
        assert!(derive("=MX.LU(A,\"Q\")", &book(), 0).is_err());
    }

    #[test]
    fn cholesky_element_by_element() {
        let steps = same_as_engine("=MX.CHOL({4,12,-16;12,37,-43;-16,-43,98})", 3, 3);
        assert_eq!(result(&steps), vec![vec![2.0, 0.0, 0.0], vec![6.0, 1.0, 0.0], vec![-8.0, 5.0, 3.0]]);
        same_as_engine("=MX.CHOL(A)", 2, 2);
        assert!(latex(&steps).contains("\\sqrt{"));
        let wb = book();
        assert!(derive("=MX.CHOL({1,2;2,1})", &wb, 0).unwrap_err().contains("positive definite"));
        assert!(derive("=MX.CHOL(D1:F3)", &wb, 0).unwrap_err().contains("symmetric"));
    }

    #[test]
    fn eigenvalues_from_the_characteristic_polynomial() {
        // λ² − 5λ + 5 = 0: (5 ± √5)/2.
        let two = same_as_engine("=MX.EIG(A)", 2, 1);
        assert!(two.iter().any(|s| s.caption == "The quadratic formula"));
        // A whole-number root (1, 2 or 11) divided out.
        let whole = same_as_engine("=MX.EIG({2,0,0;0,3,4;0,4,9})", 3, 1);
        assert_eq!(result(&whole), vec![vec![11.0], vec![2.0], vec![1.0]]);
        assert!(whole.iter().any(|s| s.caption.starts_with("A whole-number root")));
        // No whole-number root, three real ones: the trigonometric formula.
        let trig = same_as_engine("=MX.EIG({2,1,0;1,3,1;0,1,5})", 3, 1);
        assert!(trig.iter().any(|s| s.caption.starts_with("Three real roots")));
        same_as_engine("=MX.EIG(D1:F3)", 3, 1);
        // Complex pairs: the cell shows real and imaginary parts.
        let rot = same_as_engine("=MX.EIG({0,-1;1,0})", 2, 2);
        assert_eq!(result(&rot), vec![vec![0.0, 1.0], vec![0.0, -1.0]]);
        // λ³ = 2: one real root by Cardano, then a complex pair.
        let cardano = same_as_engine("=MX.EIG({0,0,2;1,0,0;0,1,0})", 3, 2);
        assert!(cardano.iter().any(|s| s.caption.starts_with("One real root")));
        same_as_engine("=MX.EIG({0,0,1;1,0,0;0,1,0})", 3, 2);
        same_as_engine("=MX.EIG({7})", 1, 1);
        let tex = latex(&two);
        assert!(tex.contains("\\lambda") && !tex.contains('λ'), "{tex}");
        assert!(derive("=MX.EIG({1,0,0,0;0,1,0,0;0,0,1,0;0,0,0,1})", &book(), 0).unwrap_err().contains("3×3"));
    }

    #[test]
    fn eigenvectors_as_null_spaces() {
        same_as_engine("=MX.EIGVEC(A)", 2, 2);
        same_as_engine("=MX.EIGVEC({2,0,0;0,3,4;0,4,9})", 3, 3);
        same_as_engine("=MX.EIGVEC({2,1,0;1,3,1;0,1,5})", 3, 3);
        same_as_engine("=MX.EIGVEC({4,1;1,4})", 2, 2);
        // A repeated eigenvalue: any orthonormal basis.
        let repeated = derive("=MX.EIGVEC({2,0;0,2})", &book(), 0).unwrap();
        assert!(repeated.iter().any(|s| s.caption.starts_with("A repeated eigenvalue")));
        assert!(derive("=MX.EIGVEC(D1:F3)", &book(), 0).unwrap_err().contains("symmetric"));
    }

    #[test]
    fn steps_write_out_as_latex() {
        let tex = latex(&derive("=A@A", &book(), 0).unwrap());
        assert!(tex.starts_with("\\begin{align*}") && tex.trim_end().ends_with("\\end{align*}"));
        assert!(tex.contains("\\begin{bmatrix} 5 & 5 \\\\ 5 & 10 \\end{bmatrix}"));
        assert!(tex.contains("\\sum"));
    }
}
