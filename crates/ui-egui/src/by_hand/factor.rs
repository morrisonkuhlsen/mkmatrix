//! Rank by row echelon form, LU with partial pivoting (the multipliers collected into L) and
//! Cholesky element by element.

use mkmatrix_engine::formula::Expr;

use super::{Ctx, HAND_MAX, Mat, Math, Step, idx, magnitude, matrix, number, op, root, row, signed, square, sub, symbol, tidy, told, unparen, var};

/// The row at or below `from` with the largest magnitude in column `col`, the first of equals
/// (as the engine's LU picks it).
pub(super) fn pivot_row(m: &Mat, from: usize, col: usize) -> usize {
    let at = |r: usize| m.get(r).and_then(|line| line.get(col)).map_or(0.0, |x| x.abs());
    (from..m.len()).fold(from, |best, r| if at(r) > at(best) { r } else { best })
}

/// `x²`, a negative `x` in parentheses.
fn squared(x: f64) -> Math {
    let base = if x < 0.0 { row(vec![op("("), Math::Num(x), op(")")]) } else { Math::Num(x) };
    Math::Sup(Box::new(base), Box::new(op("2")))
}

/// `m` with the elements at `pivots` framed.
pub(super) fn marked(m: &Mat, pivots: &[(usize, usize)]) -> Math {
    Math::Matrix(
        m.iter()
            .enumerate()
            .map(|(i, line)| {
                line.iter()
                    .enumerate()
                    .map(|(j, x)| if pivots.contains(&(i, j)) { Math::Boxed(Box::new(Math::Num(*x))) } else { Math::Num(*x) })
                    .collect()
            })
            .collect(),
    )
}

/// `R_r ← R_r − (f)·R_p` on `m`, from column `from` on; the row operation as written.
fn subtract(m: &mut Mat, r: usize, p: usize, f: f64, from: usize) -> String {
    let pivot_row = m.get(p).cloned().unwrap_or_default();
    if let Some(line) = m.get_mut(r) {
        for (x, y) in line.iter_mut().zip(&pivot_row).skip(from) {
            *x -= f * y;
        }
    }
    format!("R{} ← R{} − ({})·R{}", r + 1, r + 1, number(f), p + 1)
}

/// `MX.RANK(A)`: Gaussian elimination to row echelon form, the pivots framed; the rank is how
/// many there are.
pub(super) fn rank_steps(ctx: &Ctx, a: &Expr) -> Result<Vec<Step>, String> {
    let (m, shown) = ctx.value(a, 0)?;
    let shown = symbol(shown);
    let (rows, cols) = (m.len(), m.first().map_or(0, Vec::len));
    if rows == 0 || cols == 0 {
        return Err("A rank needs a matrix.".into());
    }
    if rows > HAND_MAX || cols > HAND_MAX {
        return Err(format!("By hand stops at {HAND_MAX} rows and {HAND_MAX} columns; this one is {rows}×{cols}."));
    }
    let scale = magnitude(&m);
    let tol = scale * 1e-10;
    let mut u = m.clone();
    let mut steps = vec![
        Step::new("The matrix", row(vec![shown.clone(), op("="), matrix(&m)])),
        Step::new("Row echelon form: eliminate below each pivot, column by column", row(vec![])),
    ];
    let mut pivots = Vec::new();
    for col in 0..cols {
        let r = pivots.len();
        if r == rows {
            break;
        }
        let piv = pivot_row(&u, r, col);
        if u[piv][col].abs() <= tol {
            // What is left of the column is rounding: it reads as the zeros it is.
            for line in u.iter_mut().skip(r) {
                line[col] = 0.0;
            }
            steps.push(told("Column {}: no pivot", (col + 1).to_string(), marked(&u, &pivots)));
            continue;
        }
        let mut ops = Vec::new();
        if piv != r {
            u.swap(piv, r);
            ops.push(format!("R{} ↔ R{}", r + 1, piv + 1));
        }
        for i in r + 1..rows {
            let f = u[i][col] / u[r][col];
            if f.abs() > 1e-15 {
                ops.push(subtract(&mut u, i, r, f, col));
            }
            u[i][col] = 0.0;
        }
        tidy(&mut u, scale);
        pivots.push((r, col));
        steps.push(if ops.is_empty() {
            told("Column {}: zeros below the pivot already", (col + 1).to_string(), marked(&u, &pivots))
        } else {
            told("Column {}", format!("{}: {}", col + 1, ops.join(",  ")), marked(&u, &pivots))
        });
    }
    steps.push(Step::new("The rank is the number of pivots", row(vec![super::call("rank", shown), op("="), Math::Num(pivots.len() as f64)])));
    Ok(steps)
}

/// `MX.LU(A, [part])`: `P·A = L·U` by elimination with partial pivoting. Each column's
/// multipliers `lᵢⱼ = uᵢⱼ / uⱼⱼ` go into L; a row swap swaps the multipliers already there.
pub(super) fn lu_steps(ctx: &Ctx, a: &Expr, part: Option<&Expr>) -> Result<Vec<Step>, String> {
    let part = match part.map(unparen) {
        None | Some(Expr::Missing) => "U".to_string(),
        Some(Expr::Text(s)) => s.trim().to_uppercase(),
        Some(_) => return Err("MX.LU's part is \"L\", \"U\" or \"P\".".into()),
    };
    if !matches!(part.as_str(), "L" | "U" | "P") {
        return Err("MX.LU's part is \"L\", \"U\" or \"P\".".into());
    }
    let (m, shown) = ctx.value(a, 0)?;
    let shown = symbol(shown);
    let n = square(&m, "An LU factorization")?;
    let scale = magnitude(&m);
    let mut u = m.clone();
    let mut l: Mat = vec![vec![0.0; n]; n];
    let mut perm: Vec<usize> = (0..n).collect();
    let with_ones =
        |l: &Mat| -> Mat { l.iter().enumerate().map(|(i, r)| r.iter().enumerate().map(|(j, x)| if i == j { 1.0 } else { *x }).collect()).collect() };
    let mut steps = vec![
        Step::new("The matrix", row(vec![shown.clone(), op("="), matrix(&m)])),
        Step::new(
            "Eliminate below each pivot; each multiplier goes into L",
            row(vec![sub(var("l"), op("ij")), op("="), sub(var("u"), op("ij")), op("/"), sub(var("u"), op("jj"))]),
        ),
    ];
    for col in 0..n {
        let piv = pivot_row(&u, col, col);
        if u[piv][col].abs() <= scale * 1e-12 {
            for line in u.iter_mut().skip(col + 1) {
                line[col] = 0.0;
            }
            steps.push(told(
                "Column {}: no pivot, nothing to eliminate",
                (col + 1).to_string(),
                row(vec![var("U"), op("="), matrix(&u), op(","), var("L"), op("="), matrix(&with_ones(&l))]),
            ));
            continue;
        }
        let mut ops = Vec::new();
        if piv != col {
            u.swap(piv, col);
            perm.swap(piv, col);
            // piv > col: the multipliers already found trade rows too.
            let (above, below) = l.split_at_mut(piv);
            above[col][..col].swap_with_slice(&mut below[0][..col]);
            ops.push(format!("R{} ↔ R{}", col + 1, piv + 1));
        }
        for i in col + 1..n {
            let f = u[i][col] / u[col][col];
            l[i][col] = f;
            if f.abs() > 1e-15 {
                let done = subtract(&mut u, i, col, f, col);
                ops.push(format!("l{}{} = {}: {done}", i + 1, col + 1, number(f)));
            }
            u[i][col] = 0.0;
        }
        tidy(&mut u, scale);
        tidy(&mut l, 1.0);
        let math = row(vec![var("U"), op("="), matrix(&u), op(","), var("L"), op("="), matrix(&with_ones(&l))]);
        steps.push(if ops.is_empty() {
            told("Column {}: zeros below the pivot already", (col + 1).to_string(), math)
        } else {
            told("Column {}", format!("{}: {}", col + 1, ops.join(",  ")), math)
        });
    }
    let l = with_ones(&l);
    let p: Mat = perm.iter().map(|&src| (0..n).map(|j| if j == src { 1.0 } else { 0.0 }).collect()).collect();
    steps.push(Step::new("The factors", row(vec![var("P"), shown, op("="), var("L"), var("U")])));
    let mut parts = vec![("L", l), ("U", u), ("P", p)];
    // The part the cell shows comes last.
    parts.sort_by_key(|(name, _)| *name == part);
    for (name, value) in parts {
        let caption = match name {
            "L" => "L: ones on the diagonal, the multipliers below",
            "U" => "U: the eliminated matrix",
            _ => "P: the row swaps, applied to the identity",
        };
        steps.push(Step::new(caption, row(vec![var(name), op("="), matrix(&value)])));
    }
    Ok(steps)
}

/// `MX.CHOL(A)`: `A = L·Lᵀ`, column by column: `lⱼⱼ = √(aⱼⱼ − Σₖ lⱼₖ²)`, then
/// `lᵢⱼ = (aᵢⱼ − Σₖ lᵢₖ lⱼₖ) / lⱼⱼ` below it.
pub(super) fn chol_steps(ctx: &Ctx, a: &Expr) -> Result<Vec<Step>, String> {
    let (m, shown) = ctx.value(a, 0)?;
    let shown = symbol(shown);
    let n = square(&m, "A Cholesky factorization")?;
    let scale = magnitude(&m);
    if (0..n).any(|i| (i + 1..n).any(|j| (m[i][j] - m[j][i]).abs() > 1e-12 * scale.max(1.0))) {
        return Err("Cholesky needs a symmetric matrix.".into());
    }
    let k_lt_j = || sub(op("Σ"), op("k<j"));
    let mut steps = vec![
        Step::new("The matrix", row(vec![shown.clone(), op("="), matrix(&m)])),
        Step::new("A = L·Lᵀ, column by column: the diagonal element first", {
            let lj = Math::Sup(Box::new(sub(var("l"), op("jk"))), Box::new(op("2")));
            row(vec![sub(var("l"), op("jj")), op("="), root(row(vec![sub(var("a"), op("jj")), op("−"), k_lt_j(), lj]))])
        }),
        Step::new(
            "then the ones below it",
            row(vec![
                sub(var("l"), op("ij")),
                op("="),
                op("("),
                sub(var("a"), op("ij")),
                op("−"),
                k_lt_j(),
                sub(var("l"), op("ik")),
                sub(var("l"), op("jk")),
                op(")"),
                op("/"),
                sub(var("l"), op("jj")),
            ]),
        ),
    ];
    let mut l: Mat = vec![vec![0.0; n]; n];
    for j in 0..n {
        steps.push(told("Column {}", (j + 1).to_string(), row(vec![])));
        let d = m[j][j] - (0..j).map(|k| l[j][k] * l[j][k]).sum::<f64>();
        if d <= scale * 1e-12 {
            return Err("The matrix isn't positive definite: a diagonal element would be the root of a number ≤ 0.".into());
        }
        l[j][j] = d.sqrt();
        let mut symbols = vec![idx("a", j + 1, j + 1)];
        let mut numbers = vec![Math::Num(m[j][j])];
        for (k, x) in l[j].iter().take(j).enumerate() {
            symbols.extend([op("−"), Math::Sup(Box::new(idx("l", j + 1, k + 1)), Box::new(op("2")))]);
            numbers.extend([op("−"), squared(*x)]);
        }
        let mut line = vec![idx("l", j + 1, j + 1), op("=")];
        if j > 0 {
            line.extend([root(row(symbols)), op("=")]);
        }
        line.extend([root(row(numbers)), op("="), Math::Num(l[j][j])]);
        steps.push(Step::new("", row(line)));
        for i in j + 1..n {
            l[i][j] = (m[i][j] - (0..j).map(|k| l[i][k] * l[j][k]).sum::<f64>()) / l[j][j];
            let mut line = vec![idx("l", i + 1, j + 1), op("=")];
            if j == 0 {
                line.extend([Math::Num(m[i][j]), op("/"), Math::Num(l[j][j])]);
            } else {
                line.extend([op("("), Math::Num(m[i][j])]);
                for (x, y) in l[i].iter().zip(&l[j]).take(j) {
                    line.extend([op("−"), signed(*x), op("·"), signed(*y)]);
                }
                line.extend([op(")"), op("/"), Math::Num(l[j][j])]);
            }
            line.extend([op("="), Math::Num(l[i][j])]);
            steps.push(Step::new("", row(line)));
        }
    }
    tidy(&mut l, scale.sqrt());
    steps.push(Step::new("The factor L", row(vec![var("L"), op("="), matrix(&l)])));
    Ok(steps)
}
