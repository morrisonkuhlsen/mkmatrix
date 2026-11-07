//! A matrix product, cell by cell: with the pointer over a cell of `C = A@B` (or `MX.MUL(A, B)`),
//! the row of A and the column of B that make it light up on the sheet, and a tag beside the
//! cell writes the sum out with its numbers, `X'(2,:)·y = 1·73 + 2·75 + … = 2915`, as a textbook
//! writes cᵢⱼ = Σₖ aᵢₖ bₖⱼ.
//!
//! The operands are found on the sheet: a range, a named matrix, or either transposed (`X'`,
//! `MX.T(X)`). A product of anything else (a product of products, an array constant) shows
//! nothing.

use egui::{Color32, Painter, Stroke, StrokeKind};
use mkmatrix_engine::core::{CellRef, RangeRef, Value};
use mkmatrix_engine::formula::{BinOp, Expr, SheetSel, UnOp};
use mkmatrix_engine::model::{Sheet, Workbook};

use crate::grid::Geo;

/// The terms written out in full; past them the sum shows `+ …`.
const MAX_TERMS: usize = 4;
/// The longest inner dimension followed: the line of each operand is read cell by cell.
const MAX_INNER: u32 = 100_000;

/// One operand on the sheet: its block, and whether the formula transposes it.
#[derive(Clone, Debug, PartialEq)]
struct Operand {
    label: String,
    block: RangeRef,
    transposed: bool,
}

impl Operand {
    /// Rows × columns as the product sees it.
    fn shape(&self) -> (u32, u32) {
        let (r, c) = (self.block.height(), self.block.width());
        if self.transposed { (c, r) } else { (r, c) }
    }

    /// Row `i` (0-based) of the operand as the product sees it: a row of the block, or a column
    /// when it is transposed.
    fn row(&self, i: u32) -> RangeRef {
        let b = self.block;
        if self.transposed {
            RangeRef::new(CellRef::new(b.start.row, b.start.col + i), CellRef::new(b.end.row, b.start.col + i))
        } else {
            RangeRef::new(CellRef::new(b.start.row + i, b.start.col), CellRef::new(b.start.row + i, b.end.col))
        }
    }

    /// Column `j` (0-based) as the product sees it.
    fn col(&self, j: u32) -> RangeRef {
        let b = self.block;
        if self.transposed {
            RangeRef::new(CellRef::new(b.start.row + j, b.start.col), CellRef::new(b.start.row + j, b.end.col))
        } else {
            RangeRef::new(CellRef::new(b.start.row, b.start.col + j), CellRef::new(b.end.row, b.start.col + j))
        }
    }

    fn name(&self) -> String {
        if self.transposed { format!("{}'", self.label) } else { self.label.clone() }
    }
}

/// What lights up for one cell of a product.
#[derive(Clone, Debug, PartialEq)]
pub struct Focus {
    /// The cell the pointer is over.
    pub cell: CellRef,
    /// The row of A and the column of B that make it.
    pub row: RangeRef,
    pub col: RangeRef,
    /// The whole result the cell is part of.
    pub result: RangeRef,
    /// The sum written out: `X'(2,:)·y = 1·73 + 2·75 + … = 2915`.
    pub text: String,
}

/// The product `cell` on sheet `si` is part of, if any, and what makes that cell.
pub fn at(wb: &Workbook, si: usize, sh: &Sheet, cell: CellRef) -> Option<Focus> {
    let (anchor, block) = match sh.spill_ranges.iter().find(|(_, r)| r.contains(cell)) {
        Some((a, r)) => (*a, *r),
        None => (cell, RangeRef::cell(cell)),
    };
    let expr = sh.cell(anchor)?.formula.as_ref()?.expr.clone()?;
    let (a, b) = product(&expr, wb, si, sh)?;
    let ((ar, ac), (br, bc)) = (a.shape(), b.shape());
    if ac != br || ac == 0 || ac > MAX_INNER {
        return None;
    }
    let (i, j) = (cell.row - block.start.row, cell.col - block.start.col);
    if i >= ar || j >= bc {
        return None;
    }
    let (row, col) = (a.row(i), b.col(j));
    let num = |c: CellRef| match sh.value_ref(c) {
        Some(Value::Number(n)) => Some(*n),
        _ => None,
    };
    let left: Vec<CellRef> = cells(row);
    let right: Vec<CellRef> = cells(col);
    let mut terms = Vec::new();
    for (x, y) in left.iter().zip(&right).take(MAX_TERMS) {
        terms.push(format!("{}·{}", short(num(*x)?), short(num(*y)?)));
    }
    if left.len() > MAX_TERMS {
        terms.push("…".into());
    }
    let (rows, cols) = (ar, bc);
    let place = |name: String, k: u32, of: u32, row: bool| match (of, row) {
        (1, _) => name,
        (_, true) => format!("{name}({},:)", k + 1),
        (_, false) => format!("{name}(:,{})", k + 1),
    };
    let lhs = format!("{}·{}", place(a.name(), i, rows, true), place(b.name(), j, cols, false));
    let total = match sh.value_ref(cell) {
        Some(Value::Number(n)) => short(*n),
        _ => "?".into(),
    };
    Some(Focus { cell, row, col, result: block, text: format!("{lhs} = {} = {total}", terms.join(" + ")) })
}

/// The two operands of a product at the top of `e`.
fn product(e: &Expr, wb: &Workbook, si: usize, sh: &Sheet) -> Option<(Operand, Operand)> {
    match e {
        Expr::Paren(x) => product(x, wb, si, sh),
        Expr::Binary(BinOp::MatMul, a, b) => Some((operand(a, wb, si, sh, 0)?, operand(b, wb, si, sh, 0)?)),
        Expr::Call(name, args) if name == "MX.MUL" => match args.as_slice() {
            [a, b] => Some((operand(a, wb, si, sh, 0)?, operand(b, wb, si, sh, 0)?)),
            _ => None,
        },
        _ => None,
    }
}

/// An operand that is a block on this sheet, maybe transposed (twice is back again).
fn operand(e: &Expr, wb: &Workbook, si: usize, sh: &Sheet, depth: u8) -> Option<Operand> {
    if depth > 8 {
        return None;
    }
    let flip = |o: Operand| Operand { transposed: !o.transposed, ..o };
    match e {
        Expr::Paren(x) => operand(x, wb, si, sh, depth + 1),
        Expr::Unary(UnOp::Transpose, x) => operand(x, wb, si, sh, depth + 1).map(flip),
        Expr::Call(name, args) if name == "MX.T" && args.len() == 1 => operand(args.first()?, wb, si, sh, depth + 1).map(flip),
        Expr::Ref(r) => {
            let same = match &r.sheet {
                SheetSel::Current => true,
                SheetSel::Named(s) => mkmatrix_engine::core::unquote_sheet(s).eq_ignore_ascii_case(&sh.name),
                SheetSel::Span(..) => false,
            };
            let block = r.range();
            (same && !block.is_full_rows() && !block.is_full_cols()).then(|| Operand { label: block.a1(), block, transposed: false })
        }
        Expr::Name(n) => crate::matrices::on_sheet(wb, si)
            .into_iter()
            .find(|(m, _)| mkmatrix_engine::formula::same_name(m, n))
            .map(|(label, block)| Operand { label, block, transposed: false }),
        _ => None,
    }
}

/// A row or column's cells in order.
fn cells(line: RangeRef) -> Vec<CellRef> {
    let mut out = Vec::new();
    for row in line.start.row..=line.end.row {
        for col in line.start.col..=line.end.col {
            out.push(CellRef::new(row, col));
        }
    }
    out
}

/// A number as the tag writes it: up to four significant digits, no trailing zeros.
fn short(n: f64) -> String {
    if !n.is_finite() {
        return n.to_string();
    }
    if n == n.trunc() && n.abs() < 1e9 {
        return format!("{n:.0}");
    }
    if n.abs() >= 1e9 || n.abs() < 1e-4 {
        return format!("{n:.3e}");
    }
    let digits = (4 - (n.abs().log10().floor() as i32) - 1).clamp(0, 6) as usize;
    let s = format!("{n:.digits$}");
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" { "0".into() } else { s }
}

/// Lights the row of A in `a_ink` and the column of B in `b_ink` (the formula's first two
/// reference colours), outlines the cell they make in `cell_ink` and writes the sum beside it.
#[allow(clippy::too_many_arguments)]
pub fn paint(p: &Painter, geo: &Geo, sh: &Sheet, f: &Focus, a_ink: Color32, b_ink: Color32, cell_ink: Color32, ground: Color32) {
    for (line, ink) in [(f.row, a_ink), (f.col, b_ink)] {
        let r = geo.range_rect(sh, line);
        if !r.is_positive() {
            continue;
        }
        p.rect_filled(r, 0.0, ink.gamma_multiply(0.14));
        p.rect_stroke(r, 0.0, Stroke::new(1.5, ink), StrokeKind::Inside);
    }
    let c = geo.cell_rect(sh, f.cell);
    p.rect_stroke(c, 0.0, Stroke::new(2.0, cell_ink), StrokeKind::Inside);
    // The sum is written under the whole result, clear of the rows the eye is following.
    crate::matrices::paint_tag_under(p, geo, geo.range_rect(sh, f.result), &f.text, cell_ink, ground);
}

#[cfg(test)]
mod tests {
    use super::*;
    use mkmatrix_engine::Session;
    use serde_json::json;

    /// A 2×3 matrix `A` at A1:C2, a 3×2 `B` at E1:F3, and `=A@B` at H1 (a 2×2 result).
    fn session(formula: &str) -> Session {
        let mut s = Session::new();
        s.new_workbook();
        let set = |s: &mut Session, cell: &str, input: &str| {
            let _ = s.execute("cell.set", json!({"cell": cell, "input": input}));
        };
        for (cell, v) in [("A1", 1), ("B1", 2), ("C1", 3), ("A2", 4), ("B2", 5), ("C2", 6)] {
            set(&mut s, cell, &v.to_string());
        }
        for (cell, v) in [("E1", 7), ("F1", 8), ("E2", 9), ("F2", 10), ("E3", 11), ("F3", 12)] {
            set(&mut s, cell, &v.to_string());
        }
        for (name, refers) in [("A", "=Sheet1!$A$1:$C$2"), ("B", "=Sheet1!$E$1:$F$3")] {
            let _ = s.execute("formulas.defineName", json!({"name": name, "refersTo": refers, "scope": "Workbook"}));
        }
        set(&mut s, "H1", formula);
        s
    }

    fn focus(s: &Session, a1: &str) -> Option<Focus> {
        let d = s.active()?;
        let sh = d.wb.sheet(0)?;
        at(&d.wb, 0, sh, CellRef::parse(a1)?)
    }

    fn a1(r: RangeRef) -> String {
        r.a1()
    }

    #[test]
    fn a_cell_of_a_product_lights_its_row_and_column() {
        let s = session("=A@B");
        let Some(f) = focus(&s, "I2") else { panic!("I2 is in the product") };
        assert_eq!((a1(f.row), a1(f.col)), ("A2:C2".into(), "F1:F3".into()));
        // 4·8 + 5·10 + 6·12 = 154
        assert_eq!(f.text, "A(2,:)·B(:,2) = 4·8 + 5·10 + 6·12 = 154");
        let Some(f) = focus(&s, "H1") else { panic!("the anchor too") };
        assert_eq!(f.text, "A(1,:)·B(:,1) = 1·7 + 2·9 + 3·11 = 58");
    }

    #[test]
    fn transposed_operands_and_ranges_are_followed() {
        // B'@A': 2×3 @ 3×2 = 2×2; its (1,2) is B's column 1 · A's row 2.
        let s = session("=B'@MX.T(A)");
        let Some(f) = focus(&s, "I1") else { panic!("I1 is in the product") };
        assert_eq!((a1(f.row), a1(f.col)), ("E1:E3".into(), "A2:C2".into()));
        assert_eq!(f.text, "B'(1,:)·A'(:,2) = 7·4 + 9·5 + 11·6 = 139");
        let s = session("=MX.MUL(A1:C2,E1:F3)");
        assert!(focus(&s, "H2").is_some_and(|f| f.text.starts_with("A1:C2(2,:)·E1:F3(:,1)")));
    }

    #[test]
    fn anything_else_shows_nothing() {
        for formula in ["=A+1", "=SUM(A)", "=(A@B)@(A@B)", "={1,2}@{3;4}", "=A@"] {
            let s = session(formula);
            assert_eq!(focus(&s, "H1"), None, "{formula}");
        }
        assert_eq!(focus(&session("=A@B"), "J1"), None, "outside the result");
        assert_eq!(focus(&session("=A@B"), "A1"), None, "an operand is no product");
    }

    #[test]
    fn long_sums_are_cut_and_numbers_kept_short() {
        assert_eq!(short(2.0), "2");
        assert_eq!(short(0.123456), "0.1235");
        assert_eq!(short(-1234.5678), "-1235");
        assert_eq!(short(f64::NAN), "NaN");
        assert_eq!(short(1e300), "1.000e300");
        assert_eq!(short(0.00001234), "1.234e-5");
    }
}
