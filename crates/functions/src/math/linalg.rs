//! MK Matrix's linear algebra: the `M*` functions beyond Excel's MMULT, MDETERM, MINVERSE and
//! MUNIT. Solving systems, decompositions (LU, QR, Cholesky, SVD, eigen), rank, trace, norms,
//! condition numbers and powers, on top of `nalgebra`.
//!
//! Results are deterministic: singular values and eigenvalues come largest first, and each
//! decomposition's signs are fixed (R's diagonal non-negative; in each singular or eigenvector,
//! its largest component positive). Rounding noise far below the matrix's own scale reads as 0.

use nalgebra::DMatrix;
use nalgebra::linalg::{Cholesky, SVD, Schur, SymmetricEigen};

use super::matrix::{matrix, to_value_array};
use super::*;

/// The largest side a decomposition takes: the work grows as its cube.
const MAX_SIDE: usize = 500;
/// The most elements a decomposition takes.
const MAX_ELEMENTS: usize = 250_000;
/// Iterations an iterative decomposition (SVD, eigen, Schur) may take before it gives up (#NUM!).
const MAX_ITERATIONS: usize = 10_000;

/// A numeric argument as a matrix, refusing sizes too large to decompose.
fn mat(a: &[Arg], i: usize) -> R<DMatrix<f64>> {
    let (r, c, d) = matrix(&crate::util::arg(a, i)?.value)?;
    if r == 0 || c == 0 {
        return Err(CellError::Value);
    }
    if r > MAX_SIDE.max(MAX_ELEMENTS / c.max(1)) || c > MAX_SIDE || r.saturating_mul(c) > MAX_ELEMENTS {
        return Err(CellError::Num);
    }
    Ok(DMatrix::from_row_slice(r, c, &d))
}

/// A square numeric argument.
fn square_mat(a: &[Arg], i: usize) -> R<DMatrix<f64>> {
    let m = mat(a, i)?;
    if m.nrows() != m.ncols() || m.nrows() > MAX_SIDE {
        return Err(CellError::Value);
    }
    Ok(m)
}

/// The largest absolute element: the scale noise is measured against.
fn scale(m: &DMatrix<f64>) -> f64 {
    m.iter().fold(0.0f64, |acc, x| acc.max(x.abs()))
}

/// A matrix as a spilled array, with rounding noise (far below `scale`) read as 0.
fn out(m: &DMatrix<f64>, scale: f64) -> R<Value> {
    let noise = f64::EPSILON * 64.0 * scale.max(f64::MIN_POSITIVE);
    let mut data = Vec::with_capacity(m.len());
    for r in 0..m.nrows() {
        for c in 0..m.ncols() {
            let x = m.get((r, c)).copied().unwrap_or(f64::NAN);
            data.push(if x.abs() < noise { 0.0 } else { x });
        }
    }
    to_value_array(m.nrows(), m.ncols(), data)
}

/// A column of numbers as a spilled array.
fn column(v: &[f64], scale: f64) -> R<Value> {
    out(&DMatrix::from_column_slice(v.len(), 1, v), scale)
}

/// An optional text argument (`part`, `type`), upper-cased and trimmed.
fn opt_part(a: &[Arg], i: usize, default: &str) -> R<String> {
    match a.get(i) {
        None => Ok(default.to_string()),
        Some(x) if matches!(x.value, Value::Empty) => Ok(default.to_string()),
        Some(_) => Ok(text(a, i)?.trim().to_ascii_uppercase()),
    }
}

/// Whether a square matrix is symmetric, to rounding.
fn is_symmetric(m: &DMatrix<f64>) -> bool {
    let tol = 1e-12 * scale(m).max(1.0);
    (0..m.nrows()).all(|r| (r + 1..m.ncols()).all(|c| m.get((r, c)).zip(m.get((c, r))).is_some_and(|(x, y)| (x - y).abs() <= tol)))
}

/// Flips each column so that its largest-magnitude component is positive; `also` (if given)
/// gets the same flips, column by column (U with V in an SVD).
fn fix_signs(m: &mut DMatrix<f64>, mut also: Option<&mut DMatrix<f64>>) {
    for c in 0..m.ncols() {
        let mut best = 0.0f64;
        for r in 0..m.nrows() {
            let x = m.get((r, c)).copied().unwrap_or(0.0);
            if x.abs() > best.abs() + 1e-12 * best.abs().max(1.0) {
                best = x;
            }
        }
        if best < 0.0 {
            m.column_mut(c).neg_mut();
            if let Some(o) = also.as_deref_mut()
                && c < o.ncols()
            {
                o.column_mut(c).neg_mut();
            }
        }
    }
}

/// The singular value decomposition, largest singular value first, signs fixed.
fn svd(m: DMatrix<f64>) -> R<(DMatrix<f64>, Vec<f64>, DMatrix<f64>)> {
    let SVD { u, v_t, singular_values } = SVD::try_new(m, true, true, f64::EPSILON, MAX_ITERATIONS).ok_or(CellError::Num)?;
    let (u, v) = (u.ok_or(CellError::Num)?, v_t.ok_or(CellError::Num)?.transpose());
    let mut order: Vec<usize> = (0..singular_values.len()).collect();
    let sv = |i: usize| singular_values.get(i).copied().unwrap_or(0.0);
    order.sort_by(|&a, &b| sv(b).total_cmp(&sv(a)));
    let s: Vec<f64> = order.iter().map(|&i| sv(i)).collect();
    let mut u = u.select_columns(&order);
    let mut v = v.select_columns(&order);
    fix_signs(&mut v, Some(&mut u));
    Ok((u, s, v))
}

/// The default tolerance below which a singular value counts as zero.
fn rank_tolerance(rows: usize, cols: usize, largest: f64) -> f64 {
    rows.max(cols) as f64 * f64::EPSILON * largest
}

/// MSOLVE(A, B): X with A·X = B. A square system is solved exactly (LU with partial pivoting;
/// a singular A gives #NUM!); any other shape by least squares (the minimum-norm solution).
pub(super) fn msolve(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let m = mat(a, 0)?;
    let b = mat(a, 1)?;
    if m.nrows() != b.nrows() {
        return Err(CellError::Value);
    }
    let sc = scale(&m).max(scale(&b));
    if m.is_square() {
        let lu = m.clone().lu();
        let u = lu.u();
        let pivots: Vec<f64> = u.diagonal().iter().map(|x| x.abs()).collect();
        let largest = pivots.iter().fold(0.0f64, |acc, x| acc.max(*x));
        let smallest = pivots.iter().fold(f64::INFINITY, |acc, x| acc.min(*x));
        if largest == 0.0 || smallest <= rank_tolerance(m.nrows(), m.ncols(), largest) {
            return Err(CellError::Num);
        }
        let x = lu.solve(&b).ok_or(CellError::Num)?;
        return out(&x, sc / largest.max(f64::MIN_POSITIVE) * sc.max(1.0));
    }
    let largest = scale(&m);
    let svd = SVD::try_new(m.clone(), true, true, f64::EPSILON, MAX_ITERATIONS).ok_or(CellError::Num)?;
    let tol = rank_tolerance(m.nrows(), m.ncols(), svd.singular_values.max());
    let x = svd.solve(&b, tol).map_err(|_| CellError::Num)?;
    out(&x, sc / largest.max(f64::MIN_POSITIVE) * sc.max(1.0))
}

/// MPINV(A): the Moore–Penrose pseudo-inverse.
pub(super) fn mpinv(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let m = mat(a, 0)?;
    let (r, c) = m.shape();
    let svd = SVD::try_new(m.clone(), true, true, f64::EPSILON, MAX_ITERATIONS).ok_or(CellError::Num)?;
    let tol = rank_tolerance(r, c, svd.singular_values.max());
    let p = svd.pseudo_inverse(tol).map_err(|_| CellError::Num)?;
    out(&p, scale(&p))
}

/// MRANK(A, [tolerance]): the number of singular values above the tolerance.
pub(super) fn mrank(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let m = mat(a, 0)?;
    let (r, c) = m.shape();
    let (_, s, _) = svd(m)?;
    let largest = s.first().copied().unwrap_or(0.0);
    let tol = match a.get(1) {
        Some(x) if !matches!(x.value, Value::Empty) => {
            let t = num(a, 1)?;
            if t < 0.0 {
                return Err(CellError::Num);
            }
            t
        }
        _ => rank_tolerance(r, c, largest),
    };
    num_val(s.iter().filter(|x| **x > tol).count() as f64)
}

/// MTRACE(A): the sum of a square matrix's diagonal.
pub(super) fn mtrace(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    num_val(square_mat(a, 0)?.trace())
}

/// MNORM(A, [type]): "F" Frobenius (default), 1 (largest column sum), 2 (largest singular
/// value), "INF" (largest row sum) or "MAX" (largest element), all of absolute values.
pub(super) fn mnorm(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let m = mat(a, 0)?;
    let n = match opt_part(a, 1, "F")?.as_str() {
        "F" | "FRO" | "FROBENIUS" => m.norm(),
        "1" => (0..m.ncols()).map(|c| m.column(c).iter().map(|x| x.abs()).sum::<f64>()).fold(0.0, f64::max),
        "2" => svd(m)?.1.first().copied().unwrap_or(0.0),
        "INF" | "I" => (0..m.nrows()).map(|r| m.row(r).iter().map(|x| x.abs()).sum::<f64>()).fold(0.0, f64::max),
        "MAX" | "M" => scale(&m),
        _ => return Err(CellError::Value),
    };
    num_val(n)
}

/// MCOND(A): the 2-norm condition number, largest singular value over smallest; #NUM! when
/// the matrix is singular.
pub(super) fn mcond(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let m = mat(a, 0)?;
    let (r, c) = m.shape();
    let (_, s, _) = svd(m)?;
    let largest = s.first().copied().unwrap_or(0.0);
    let smallest = s.last().copied().unwrap_or(0.0);
    if smallest <= rank_tolerance(r, c, largest) {
        return Err(CellError::Num);
    }
    num_val(largest / smallest)
}

/// MEIGVAL(A): a square matrix's eigenvalues, largest first, as a column. Complex eigenvalues
/// (of a matrix that isn't symmetric) give two columns: real parts, then imaginary parts.
pub(super) fn meigval(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let m = square_mat(a, 0)?;
    let sc = scale(&m);
    if is_symmetric(&m) {
        let e = SymmetricEigen::try_new(m, f64::EPSILON, MAX_ITERATIONS).ok_or(CellError::Num)?;
        let mut v: Vec<f64> = e.eigenvalues.iter().copied().collect();
        v.sort_by(|x, y| y.total_cmp(x));
        return column(&v, sc);
    }
    let schur = Schur::try_new(m, f64::EPSILON, MAX_ITERATIONS).ok_or(CellError::Num)?;
    let mut v: Vec<(f64, f64)> = schur.complex_eigenvalues().iter().map(|z| (z.re, z.im)).collect();
    v.sort_by(|x, y| y.0.total_cmp(&x.0).then(y.1.total_cmp(&x.1)));
    let noise = f64::EPSILON * 64.0 * sc.max(f64::MIN_POSITIVE);
    if v.iter().all(|(_, im)| im.abs() < noise.max(1e-12 * sc)) {
        let re: Vec<f64> = v.iter().map(|(re, _)| *re).collect();
        return column(&re, sc);
    }
    let both = DMatrix::from_fn(v.len(), 2, |r, c| v.get(r).map_or(f64::NAN, |z| if c == 0 { z.0 } else { z.1 }));
    out(&both, sc)
}

/// MEIGVEC(A): a symmetric matrix's eigenvectors, as columns in MEIGVAL's order, each of unit
/// length with its largest component positive. #VALUE! for a matrix that isn't symmetric.
pub(super) fn meigvec(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let m = square_mat(a, 0)?;
    if !is_symmetric(&m) {
        return Err(CellError::Value);
    }
    let e = SymmetricEigen::try_new(m, f64::EPSILON, MAX_ITERATIONS).ok_or(CellError::Num)?;
    let mut order: Vec<usize> = (0..e.eigenvalues.len()).collect();
    let ev = |i: usize| e.eigenvalues.get(i).copied().unwrap_or(0.0);
    order.sort_by(|&x, &y| ev(y).total_cmp(&ev(x)));
    let mut v = e.eigenvectors.select_columns(&order);
    fix_signs(&mut v, None);
    out(&v, 1.0)
}

/// MSVD(A, [part]): the singular value decomposition A = U·diag(S)·V'. "S" (default) gives the
/// singular values as a column, largest first; "U" and "V" the matching singular vectors.
pub(super) fn msvd(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let m = mat(a, 0)?;
    let sc = scale(&m);
    let part = opt_part(a, 1, "S")?;
    let (u, s, v) = svd(m)?;
    match part.as_str() {
        "S" => column(&s, sc),
        "U" => out(&u, 1.0),
        "V" => out(&v, 1.0),
        _ => Err(CellError::Value),
    }
}

/// MQR(A, part): the thin QR decomposition A = Q·R, with R's diagonal non-negative. "Q" gives
/// the orthonormal columns, "R" the upper triangle.
pub(super) fn mqr(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let m = mat(a, 0)?;
    let sc = scale(&m);
    let part = opt_part(a, 1, "R")?;
    let qr = m.qr();
    let (mut q, mut r) = (qr.q(), qr.r());
    for i in 0..r.nrows().min(r.ncols()) {
        if r.get((i, i)).is_some_and(|d| *d < 0.0) {
            r.row_mut(i).neg_mut();
            if i < q.ncols() {
                q.column_mut(i).neg_mut();
            }
        }
    }
    match part.as_str() {
        "Q" => out(&q, 1.0),
        "R" => out(&r, sc),
        _ => Err(CellError::Value),
    }
}

/// MLU(A, part): the LU decomposition with partial pivoting, P·A = L·U, of a square matrix.
/// "L" gives the unit lower triangle, "U" the upper triangle, "P" the row permutation.
pub(super) fn mlu(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let m = square_mat(a, 0)?;
    let sc = scale(&m);
    let n = m.nrows();
    let part = opt_part(a, 1, "U")?;
    let (p, l, u) = m.lu().unpack();
    match part.as_str() {
        "L" => out(&l, 1.0),
        "U" => out(&u, sc),
        "P" => {
            let mut id = DMatrix::<f64>::identity(n, n);
            p.permute_rows(&mut id);
            out(&id, 1.0)
        }
        _ => Err(CellError::Value),
    }
}

/// MCHOL(A): the lower-triangular L with A = L·L', for a symmetric positive-definite A; #NUM!
/// when A isn't positive definite, #VALUE! when it isn't symmetric.
pub(super) fn mchol(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let m = square_mat(a, 0)?;
    if !is_symmetric(&m) {
        return Err(CellError::Value);
    }
    let sc = scale(&m).sqrt();
    let l = Cholesky::new(m).ok_or(CellError::Num)?.l();
    out(&l, sc)
}

/// MPOWER(A, k): a square matrix to an integer power; 0 gives the identity, a negative power
/// the inverse's (#NUM! for a singular matrix).
pub(super) fn mpower(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let m = square_mat(a, 0)?;
    let k = num(a, 1)?;
    if k.fract() != 0.0 || k.abs() > 1e9 {
        return Err(CellError::Num);
    }
    let n = m.nrows();
    let mut base = if k < 0.0 {
        let lu = m.clone().lu();
        let largest = lu.u().diagonal().iter().fold(0.0f64, |acc, x| acc.max(x.abs()));
        let smallest = lu.u().diagonal().iter().fold(f64::INFINITY, |acc, x| acc.min(x.abs()));
        if largest == 0.0 || smallest <= rank_tolerance(n, n, largest) {
            return Err(CellError::Num);
        }
        lu.try_inverse().ok_or(CellError::Num)?
    } else {
        m
    };
    let mut e = k.abs() as u64;
    let mut acc = DMatrix::<f64>::identity(n, n);
    while e > 0 {
        if e & 1 == 1 {
            acc = &acc * &base;
        }
        e >>= 1;
        if e > 0 {
            base = &base * &base;
        }
        if acc.iter().chain(base.iter()).any(|x| !x.is_finite()) {
            return Err(CellError::Num);
        }
    }
    let sc = scale(&acc);
    out(&acc, sc)
}

/// The exact determinant of a matrix of integers (Bareiss's fraction-free elimination), or
/// `None` when an element isn't a small integer or an intermediate overflows.
pub(super) fn exact_determinant(n: usize, m: &[f64]) -> Option<f64> {
    const LIMIT: f64 = 9_007_199_254_740_992.0; // 2^53: every integer below it is exact.
    let mut a: Vec<i128> = Vec::with_capacity(m.len());
    for x in m {
        if x.fract() != 0.0 || x.abs() >= LIMIT {
            return None;
        }
        a.push(*x as i128);
    }
    let at = |a: &Vec<i128>, r: usize, c: usize| a.get(r * n + c).copied();
    let mut sign = 1i128;
    let mut prev = 1i128;
    for k in 0..n {
        // A zero pivot: swap in a row below with a non-zero one, or the determinant is 0.
        if at(&a, k, k)? == 0 {
            let swap = (k + 1..n).find(|&r| at(&a, r, k).is_some_and(|x| x != 0));
            let Some(r) = swap else { return Some(0.0) };
            for c in 0..n {
                a.swap(k * n + c, r * n + c);
            }
            sign = -sign;
        }
        let pivot = at(&a, k, k)?;
        for i in k + 1..n {
            for j in k + 1..n {
                let v = at(&a, i, j)?.checked_mul(pivot)?.checked_sub(at(&a, i, k)?.checked_mul(at(&a, k, j)?)?)?;
                *a.get_mut(i * n + j)? = v.checked_div(prev)?;
            }
        }
        prev = pivot;
    }
    let det = if n == 0 { 1 } else { at(&a, n - 1, n - 1)?.checked_mul(sign)? };
    let d = det as f64;
    d.is_finite().then_some(d)
}
