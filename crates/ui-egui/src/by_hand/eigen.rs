//! Eigenvalues of 2×2 and 3×3 matrices from the characteristic polynomial `det(A − λI) = 0`
//! (a whole-number root divided out, else the cubic formulas, then the quadratic formula), and a
//! symmetric matrix's eigenvectors as the null space of `A − λI` for each one.

use mkmatrix_engine::formula::Expr;

use super::factor::{marked, pivot_row};
use super::{Ctx, Mat, Math, Step, call, det, magnitude, matrix, minor, number, op, root, row, square, told, var};

/// `(re, im)`: an eigenvalue, complex when a matrix isn't symmetric.
type Complex = (f64, f64);

/// The largest matrix whose eigenvalues are worked by hand.
const EIG_MAX: usize = 3;

fn lambda() -> Math {
    var("λ")
}

/// A number, or a complex one as `a + bi`.
fn complex((re, im): Complex) -> Math {
    if im == 0.0 {
        return Math::Num(re);
    }
    row(vec![Math::Num(re), op(if im < 0.0 { "−" } else { "+" }), Math::Num(im.abs()), var("i")])
}

/// Rounding noise read as a hand would write it: the integer beside it, 0 for a tiny part.
fn tidy_root((re, im): Complex, scale: f64) -> Complex {
    let clean = |x: f64| {
        if x.abs() < 1e-10 * scale.max(1.0) {
            0.0
        } else if (x - x.round()).abs() < 1e-9 * x.abs().max(1.0) {
            x.round()
        } else {
            x
        }
    };
    (clean(re), clean(im))
}

/// A polynomial equation in `x` with descending coefficients (`[1, −5, 6]` is `x² − 5x + 6 = 0`).
fn poly(coef: &[f64], x: &str) -> Math {
    let mut items = poly_terms(coef, x);
    items.extend([op("="), Math::Num(0.0)]);
    row(items)
}

/// The terms of [`poly`]'s left side.
fn poly_terms(coef: &[f64], x: &str) -> Vec<Math> {
    let degree = coef.len().saturating_sub(1);
    let mut items = Vec::new();
    for (i, c) in coef.iter().enumerate() {
        let power = degree - i;
        if *c == 0.0 && !(items.is_empty() && power == 0) {
            continue;
        }
        if !items.is_empty() {
            items.push(op(if *c < 0.0 { "−" } else { "+" }));
        } else if *c < 0.0 {
            items.push(op("−"));
        }
        let size = c.abs();
        if size != 1.0 || power == 0 {
            items.push(Math::Num(size));
        }
        match power {
            0 => {}
            1 => items.push(var(x)),
            p => items.push(Math::Sup(Box::new(var(x)), Box::new(op(&p.to_string())))),
        }
    }
    items
}

/// `p(x)` by Horner's rule.
fn eval(coef: &[f64], x: f64) -> f64 {
    coef.iter().fold(0.0, |acc, c| acc * x + c)
}

/// A few Newton steps on a real root found by formula.
fn polish(coef: &[f64], mut x: f64) -> f64 {
    let degree = coef.len().saturating_sub(1);
    let deriv: Vec<f64> = coef.iter().take(degree).enumerate().map(|(i, c)| c * (degree - i) as f64).collect();
    for _ in 0..4 {
        let d = eval(&deriv, x);
        if d == 0.0 {
            break;
        }
        let next = x - eval(coef, x) / d;
        if !next.is_finite() || (eval(coef, next)).abs() >= eval(coef, x).abs() {
            break;
        }
        x = next;
    }
    x
}

/// `A − λI` with λ written on the diagonal.
fn shifted(m: &Mat) -> Math {
    Math::Matrix(
        m.iter()
            .enumerate()
            .map(|(i, line)| {
                line.iter().enumerate().map(|(j, x)| if i == j { row(vec![Math::Num(*x), op("−"), lambda()]) } else { Math::Num(*x) }).collect()
            })
            .collect(),
    )
}

/// The roots of `λ² + bλ + c`, by the quadratic formula.
fn quadratic(b: f64, c: f64, scale: f64, steps: &mut Vec<Step>) -> [Complex; 2] {
    let mut disc = b * b - 4.0 * c;
    if disc.abs() <= 1e-12 * scale.max(1.0).powi(2) {
        disc = 0.0;
    }
    steps.push(Step::new(
        "The quadratic formula",
        row(vec![lambda(), op("="), op("("), Math::Num(-b), op("±"), root(Math::Num(disc)), op(")"), op("/"), Math::Num(2.0)]),
    ));
    let roots = if disc >= 0.0 {
        let s = disc.sqrt();
        [((-b + s) / 2.0, 0.0), ((-b - s) / 2.0, 0.0)]
    } else {
        let s = (-disc).sqrt() / 2.0;
        [(-b / 2.0, s), (-b / 2.0, -s)]
    };
    let roots = roots.map(|z| tidy_root(z, scale));
    let line = if disc >= 0.0 {
        vec![lambda(), op("="), Math::Num(roots[0].0), op(","), Math::Num(roots[1].0)]
    } else {
        vec![lambda(), op("="), Math::Num(roots[0].0), op("±"), Math::Num(roots[0].1.abs()), var("i")]
    };
    steps.push(Step::new("", row(line)));
    roots
}

/// A whole-number root of a cubic with whole coefficients: a divisor of the constant term.
fn whole_root(coef: &[f64]) -> Option<f64> {
    if coef.iter().any(|c| c.fract() != 0.0 || c.abs() > 1e9) {
        return None;
    }
    let d = coef.last().copied()?.abs();
    if d == 0.0 {
        return Some(0.0);
    }
    let d = d as u64;
    (1..=d.min(1_000_000)).filter(|k| d.is_multiple_of(*k)).flat_map(|k| [k as f64, -(k as f64)]).find(|&r| eval(coef, r) == 0.0)
}

/// The characteristic polynomial of `m` (n ≤ 3) and its roots, the steps written into `steps`;
/// the roots sorted as `MX.EIG` gives them, largest first.
fn roots_by_hand(m: &Mat, shown: &Math, steps: &mut Vec<Step>) -> Vec<Complex> {
    let n = m.len();
    let scale = magnitude(m);
    steps.push(Step::new(
        "The characteristic equation",
        row(vec![call("det", row(vec![shown.clone(), op("−"), lambda(), var("I")])), op("="), Math::Num(0.0)]),
    ));
    steps.push(Step::new("", row(vec![call("det", shifted(m)), op("="), Math::Num(0.0)])));
    let coef: Vec<f64> = match n {
        1 => vec![1.0, -m[0][0]],
        2 => {
            steps.push(Step::new(
                "Main diagonal minus the other",
                row(vec![
                    op("("),
                    Math::Num(m[0][0]),
                    op("−"),
                    lambda(),
                    op(")"),
                    op("("),
                    Math::Num(m[1][1]),
                    op("−"),
                    lambda(),
                    op(")"),
                    op("−"),
                    Math::Num(m[0][1]),
                    op("·"),
                    Math::Num(m[1][0]),
                    op("="),
                    Math::Num(0.0),
                ]),
            ));
            vec![1.0, -(m[0][0] + m[1][1]), det(m)]
        }
        _ => {
            let tr = m[0][0] + m[1][1] + m[2][2];
            let minors: Vec<f64> = (0..3).map(|i| det(&minor(m, i, i))).collect();
            let d = det(m);
            steps.push(Step::new(
                "With the trace, the principal 2×2 minors and the determinant",
                row(vec![
                    Math::Sup(Box::new(lambda()), Box::new(op("3"))),
                    op("−"),
                    call("tr", shown.clone()),
                    Math::Sup(Box::new(lambda()), Box::new(op("2"))),
                    op("+"),
                    op("("),
                    super::sub(var("M"), op("11")),
                    op("+"),
                    super::sub(var("M"), op("22")),
                    op("+"),
                    super::sub(var("M"), op("33")),
                    op(")"),
                    lambda(),
                    op("−"),
                    call("det", shown.clone()),
                    op("="),
                    Math::Num(0.0),
                ]),
            ));
            steps.push(Step::new("", row(vec![call("tr", shown.clone()), op("="), Math::Num(tr)])));
            steps.push(Step::new(
                "",
                row(vec![
                    super::sub(var("M"), op("11")),
                    op("+"),
                    super::sub(var("M"), op("22")),
                    op("+"),
                    super::sub(var("M"), op("33")),
                    op("="),
                    Math::Num(minors[0]),
                    op("+"),
                    Math::Num(minors[1]),
                    op("+"),
                    Math::Num(minors[2]),
                    op("="),
                    Math::Num(minors.iter().sum()),
                ]),
            ));
            steps.push(Step::new("", row(vec![call("det", shown.clone()), op("="), Math::Num(d)])));
            vec![1.0, -tr, minors.iter().sum(), -d]
        }
    };
    let mut coef = coef;
    let mut clean = vec![coef.clone()];
    super::tidy(&mut clean, scale);
    coef = clean.pop().unwrap_or(coef);
    steps.push(Step::new("The characteristic polynomial", poly(&coef, "λ")));
    let mut roots: Vec<Complex> = match n {
        1 => vec![(-coef[1], 0.0)],
        2 => quadratic(coef[1], coef[2], scale, steps).to_vec(),
        _ => cubic(&coef, scale, steps),
    };
    roots.iter_mut().for_each(|z| *z = tidy_root(*z, scale));
    roots.sort_by(|x, y| y.0.total_cmp(&x.0).then(y.1.total_cmp(&x.1)));
    roots
}

/// The roots of the monic cubic `coef`: a whole-number root divided out, or else the depressed
/// cubic's formulas (trigonometric for three real roots, Cardano's for one).
fn cubic(coef: &[f64], scale: f64, steps: &mut Vec<Step>) -> Vec<Complex> {
    let (b, c, d) = (coef[1], coef[2], coef[3]);
    let first = if let Some(r) = whole_root(coef) {
        steps.push(told(
            "A whole-number root, a divisor of the constant term: λ = {}",
            number(r),
            row(vec![var("p"), op("("), Math::Num(r), op(")"), op("="), Math::Num(0.0)]),
        ));
        r
    } else {
        // λ = t − b/3 takes away the λ² term: t³ + pt + q = 0.
        let shift = b / 3.0;
        let p = c - b * b / 3.0;
        let q = 2.0 * b * b * b / 27.0 - b * c / 3.0 + d;
        let mut depressed = vec![lambda(), op("="), var("t"), op(if shift < 0.0 { "+" } else { "−" }), Math::Num(shift.abs()), op(":")];
        depressed.push(poly(&[1.0, 0.0, p, q], "t"));
        steps.push(Step::new("No whole-number root. With λ = t − b/3, the λ² term goes", row(depressed)));
        let disc = (q / 2.0).powi(2) + (p / 3.0).powi(3);
        let noise = 1e-12 * scale.max(1.0).powi(6);
        if disc <= noise {
            let ts: Vec<f64> = if p.abs() < 1e-14 * scale.max(1.0).powi(2) {
                vec![0.0; 3]
            } else {
                let r = 2.0 * (-p / 3.0).sqrt();
                let arg = ((3.0 * q) / (2.0 * p) * (-3.0 / p).sqrt()).clamp(-1.0, 1.0);
                let phi = arg.acos() / 3.0;
                (0..3).map(|k| r * (phi - 2.0 * std::f64::consts::PI * k as f64 / 3.0).cos()).collect()
            };
            steps.push(told(
                "Three real roots, as the discriminant D = {} ≤ 0: the trigonometric formula",
                number(disc.min(0.0)),
                row(vec![var("φ"), op("="), op("1/3"), op("·"), call("arccos", row(vec![op("3q/2p"), op("·"), root(op("−3/p"))]))]),
            ));
            steps.push(Step::new(
                "",
                row(vec![
                    super::sub(var("t"), var("k")),
                    op("="),
                    Math::Num(2.0),
                    root(row(vec![op("−"), var("p"), op("/"), Math::Num(3.0)])),
                    op("·"),
                    call("cos", row(vec![var("φ"), op("−"), op("2πk/3")])),
                    op(","),
                    var("k"),
                    op("="),
                    op("0, 1, 2"),
                ]),
            ));
            let mut roots = Vec::new();
            for (k, t) in ts.iter().enumerate() {
                let x = polish(coef, t - shift);
                steps.push(Step::new("", row(vec![super::sub(lambda(), op(&(k + 1).to_string())), op("="), Math::Num(x)])));
                roots.push((x, 0.0));
            }
            return roots;
        }
        let s = disc.sqrt();
        let t = (-q / 2.0 + s).cbrt() + (-q / 2.0 - s).cbrt();
        let x = polish(coef, t - shift);
        steps.push(told(
            "One real root, as the discriminant D = {} > 0: Cardano's formula",
            number(disc),
            row(vec![
                var("t"),
                op("="),
                Math::Root(3, Box::new(row(vec![op("−q/2"), op("+"), root(var("D"))]))),
                op("+"),
                Math::Root(3, Box::new(row(vec![op("−q/2"), op("−"), root(var("D"))]))),
                op("="),
                Math::Num(t),
            ]),
        ));
        steps.push(Step::new(
            "",
            row(vec![lambda(), op("="), var("t"), op(if shift < 0.0 { "+" } else { "−" }), Math::Num(shift.abs()), op("="), Math::Num(x)]),
        ));
        x
    };
    // λ³ + bλ² + cλ + d = (λ − r)(λ² + (b + r)λ + (c + r(b + r))).
    let (b2, c2) = (b + first, c + first * (b + first));
    let mut divided = vec![op("("), lambda(), op(if first < 0.0 { "+" } else { "−" }), Math::Num(first.abs()), op(")"), op("(")];
    divided.extend(poly_terms(&[1.0, b2, c2], "λ"));
    divided.extend([op(")"), op("="), Math::Num(0.0)]);
    steps.push(Step::new("Dividing it out leaves a quadratic", row(divided)));
    let mut roots = vec![(first, 0.0)];
    roots.extend(quadratic(b2, c2, scale, steps));
    roots
}

/// `MX.EIG(A)` for a 2×2 or 3×3 matrix.
pub(super) fn eig_steps(ctx: &Ctx, a: &Expr) -> Result<Vec<Step>, String> {
    let (m, shown) = ctx.value(a, 0)?;
    let shown = super::symbol(shown);
    let n = square(&m, "Eigenvalues")?;
    if n > EIG_MAX {
        return Err("Eigenvalues by hand: matrices up to 3×3.".into());
    }
    let mut steps = vec![Step::new("The matrix", row(vec![shown.clone(), op("="), matrix(&m)]))];
    let roots = roots_by_hand(&m, &shown, &mut steps);
    steps.push(Step::new(
        "The eigenvalues, largest first",
        row(vec![lambda(), op("="), Math::Matrix(roots.iter().map(|z| vec![complex(*z)]).collect())]),
    ));
    if roots.iter().any(|z| z.1 != 0.0) {
        let both: Mat = roots.iter().map(|z| vec![z.0, z.1]).collect();
        steps.push(Step::new("In the cell: real parts, then imaginary parts", matrix(&both)));
    }
    Ok(steps)
}

/// Gauss–Jordan to reduced row echelon form; entries within `tol` of 0 are 0. The pivots as
/// (row, column).
fn rref(m: &mut Mat, tol: f64) -> Vec<(usize, usize)> {
    let (rows, cols) = (m.len(), m.first().map_or(0, Vec::len));
    let mut pivots = Vec::new();
    for col in 0..cols {
        let r = pivots.len();
        if r == rows {
            break;
        }
        let piv = pivot_row(m, r, col);
        if m[piv][col].abs() <= tol {
            for line in m.iter_mut().skip(r) {
                line[col] = 0.0;
            }
            continue;
        }
        m.swap(piv, r);
        let p = m[r][col];
        for x in &mut m[r] {
            *x /= p;
        }
        let pivot_line = m[r].clone();
        for (i, line) in m.iter_mut().enumerate() {
            if i == r {
                continue;
            }
            let f = line[col];
            for (x, y) in line.iter_mut().zip(&pivot_line) {
                *x -= f * y;
            }
            line[col] = 0.0;
        }
        pivots.push((r, col));
    }
    super::tidy(m, 1.0);
    pivots
}

fn column(v: &[f64]) -> Math {
    Math::Matrix(v.iter().map(|x| vec![Math::Num(*x)]).collect())
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// `MX.EIGVEC(A)` for a symmetric 2×2 or 3×3 matrix: for each eigenvalue, the null space of
/// `A − λI`, scaled to unit length with its largest component positive (as the cell gives them).
pub(super) fn eigvec_steps(ctx: &Ctx, a: &Expr) -> Result<Vec<Step>, String> {
    let (m, shown) = ctx.value(a, 0)?;
    let shown = super::symbol(shown);
    let n = square(&m, "Eigenvectors")?;
    if n > EIG_MAX {
        return Err("Eigenvectors by hand: matrices up to 3×3.".into());
    }
    let scale = magnitude(&m);
    if (0..n).any(|i| (i + 1..n).any(|j| (m[i][j] - m[j][i]).abs() > 1e-12 * scale.max(1.0))) {
        return Err("Eigenvectors by hand need a symmetric matrix, as MX.EIGVEC does.".into());
    }
    let mut steps = vec![Step::new("The matrix", row(vec![shown.clone(), op("="), matrix(&m)]))];
    let roots = roots_by_hand(&m, &shown, &mut steps);
    steps.push(Step::new("The eigenvalues, largest first", row(vec![lambda(), op("="), column(&roots.iter().map(|z| z.0).collect::<Vec<_>>())])));
    let mut vectors: Vec<Vec<f64>> = Vec::new();
    let mut i = 0;
    while i < roots.len() {
        let l = roots[i].0;
        let repeats = roots.iter().skip(i).take_while(|z| (z.0 - l).abs() <= 1e-8 * scale.max(1.0)).count();
        i += repeats;
        let shifted: Mat =
            m.iter().enumerate().map(|(r, line)| line.iter().enumerate().map(|(c, x)| if r == c { x - l } else { *x }).collect()).collect();
        steps.push(told(
            "For λ = {}: the vectors v with (A − λI)v = 0",
            number(l),
            row(vec![shown.clone(), op("−"), Math::Num(l), var("I"), op("="), matrix(&shifted)]),
        ));
        let mut reduced = shifted.clone();
        let pivots = rref(&mut reduced, 1e-8 * magnitude(&shifted).max(scale));
        steps.push(Step::new("Reduced row echelon form", marked(&reduced, &pivots)));
        let free: Vec<usize> = (0..n).filter(|c| !pivots.iter().any(|p| p.1 == *c)).collect();
        if free.len() != repeats {
            return Err("The eigenvectors don't come out cleanly by hand; the cell's values stand.".into());
        }
        let mut basis: Vec<Vec<f64>> = free
            .iter()
            .map(|&f| {
                let mut v = vec![0.0; n];
                v[f] = 1.0;
                for &(r, c) in &pivots {
                    v[c] = -reduced[r][f];
                }
                v
            })
            .collect();
        let named = |vs: &[Vec<f64>]| {
            let mut items = Vec::new();
            for (k, v) in vs.iter().enumerate() {
                if k > 0 {
                    items.push(op(","));
                }
                let name = if vs.len() > 1 { super::sub(var("v"), op(&(k + 1).to_string())) } else { var("v") };
                items.extend([name, op("="), column(v)]);
            }
            row(items)
        };
        steps.push(Step::new("Each free variable set to 1 gives an eigenvector", named(&basis)));
        // Gram–Schmidt (one vector: just its length), then the cell's sign.
        for k in 0..basis.len() {
            for j in 0..k {
                let f = dot(&basis[k], &basis[j]);
                let prev = basis[j].clone();
                for (x, y) in basis[k].iter_mut().zip(&prev) {
                    *x -= f * y;
                }
            }
            let len = dot(&basis[k], &basis[k]).sqrt();
            if len <= 1e-12 {
                return Err("The eigenvectors don't come out cleanly by hand; the cell's values stand.".into());
            }
            let mut best = 0.0f64;
            for x in &basis[k] {
                if x.abs() > best.abs() + 1e-12 * best.abs().max(1.0) {
                    best = *x;
                }
            }
            let sign = if best < 0.0 { -1.0 } else { 1.0 };
            for x in &mut basis[k] {
                *x *= sign / len;
            }
        }
        let mut tidied = basis.clone();
        super::tidy(&mut tidied, 1.0);
        steps.push(Step::new(
            if repeats > 1 {
                "A repeated eigenvalue: an orthonormal basis of its vectors (any one works; the cell's may differ)"
            } else {
                "Scaled to unit length, the largest component positive"
            },
            named(&tidied),
        ));
        vectors.extend(tidied);
    }
    let v: Mat = (0..n).map(|r| vectors.iter().map(|col| col.get(r).copied().unwrap_or(0.0)).collect()).collect();
    steps.push(Step::new("The eigenvectors as columns, in the eigenvalues' order", row(vec![var("V"), op("="), matrix(&v)])));
    Ok(steps)
}
