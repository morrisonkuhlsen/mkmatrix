//! View › Matrix Heat Map: each matrix on the sheet (a named matrix, or a spilled result of more
//! than one cell) shades its numbers by their values, so a matrix's structure (diagonal
//! dominance, bands, blocks, sparsity, signs) shows at a glance. The numbers stay readable over
//! the shading.
//!
//! Each block has its own scale. When its numbers take both signs the scale diverges from zero:
//! positive values in the warm hue, negative in the cool one, the largest magnitude deepest. When
//! they take one sign it runs from the smallest magnitude (lightest) to the largest (deepest) in
//! that sign's hue. Text, logicals, errors and empty cells stay unshaded.

use egui::Color32;
use mkmatrix_engine::core::{CellRef, RangeRef, Value};
use mkmatrix_engine::model::{Sheet, Workbook};

use crate::theme::Tokens;

/// The largest block (cells) that gets a scale: a matrix bigger than this is left unshaded rather
/// than scanned every frame.
pub const MAX_CELLS: u64 = 65_536;
/// How many blocks a frame shades at most.
const MAX_BLOCKS: usize = 256;
/// The lightest a shaded number gets, so the smallest of a one-sign block still shows as shaded.
const FLOOR: f32 = 0.10;

/// A block's scale: its numbers' range.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Scale {
    lo: f64,
    hi: f64,
}

impl Scale {
    /// Where `v` sits: −1 (deepest cool) … 0 (unshaded) … 1 (deepest warm).
    fn place(self, v: f64) -> f32 {
        let Scale { lo, hi } = self;
        let t = if lo < 0.0 && hi > 0.0 {
            v / lo.abs().max(hi)
        } else if hi - lo <= f64::EPSILON * hi.abs().max(lo.abs()).max(1.0) {
            // One value throughout: half shaded, in its sign's hue.
            if v < 0.0 { -0.5 } else { 0.5 }
        } else if lo >= 0.0 {
            FLOOR as f64 + (1.0 - FLOOR as f64) * (v - lo) / (hi - lo)
        } else {
            -(FLOOR as f64 + (1.0 - FLOOR as f64) * (hi - v) / (hi - lo))
        };
        if t.is_finite() { t.clamp(-1.0, 1.0) as f32 } else { 0.0 }
    }
}

/// The blocks shaded this frame, smallest first so a matrix inside another shades by its own scale.
#[derive(Debug, Default)]
pub struct HeatMap {
    blocks: Vec<(RangeRef, Scale)>,
    /// The last cell looked up and its block, since cells are painted row by row.
    cache: std::cell::Cell<Option<(CellRef, usize)>>,
}

impl HeatMap {
    /// The blocks of sheet `si` that intersect `visible`, each with its scale. Empty when nothing
    /// there is a matrix of numbers.
    pub fn new(wb: &Workbook, si: usize, sh: &Sheet, visible: RangeRef) -> HeatMap {
        let mut ranges: Vec<RangeRef> = crate::matrices::on_sheet(wb, si).into_iter().map(|(_, r)| r).collect();
        ranges.extend(sh.spill_ranges.values().copied().filter(|r| r.count() > 1));
        ranges.retain(|r| r.intersects(&visible) && u64::from(r.height()) * u64::from(r.width()) <= MAX_CELLS);
        ranges.sort_by_key(|r| u64::from(r.height()) * u64::from(r.width()));
        ranges.dedup();
        ranges.truncate(MAX_BLOCKS);
        let blocks = ranges.into_iter().filter_map(|r| scale(sh, r).map(|s| (r, s))).collect();
        HeatMap { blocks, cache: std::cell::Cell::new(None) }
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// The shade for cell `c` holding `v`: `None` outside every block, or for anything but a number.
    pub fn fill(&self, c: CellRef, v: Option<&Value>, t: &Tokens) -> Option<Color32> {
        let Some(Value::Number(n)) = v else { return None };
        let i = match self.cache.get() {
            Some((at, i)) if at == c => i,
            _ => {
                let i = self.blocks.iter().position(|(r, _)| r.contains(c))?;
                self.cache.set(Some((c, i)));
                i
            }
        };
        let (_, s) = self.blocks.get(i)?;
        Some(t.heat(s.place(*n)))
    }

    /// The scales by block, for tests.
    #[cfg(test)]
    fn scales(&self) -> std::collections::HashMap<String, (f64, f64)> {
        self.blocks.iter().map(|(r, s)| (r.a1(), (s.lo, s.hi))).collect()
    }
}

/// The range of the numbers in `range`, `None` when it holds none.
fn scale(sh: &Sheet, range: RangeRef) -> Option<Scale> {
    let mut acc: Option<Scale> = None;
    for row in range.start.row..=range.end.row {
        for col in range.start.col..=range.end.col {
            if let Some(Value::Number(v)) = sh.value_ref(CellRef::new(row, col))
                && v.is_finite()
            {
                let s = acc.get_or_insert(Scale { lo: *v, hi: *v });
                s.lo = s.lo.min(*v);
                s.hi = s.hi.max(*v);
            }
        }
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;
    use mkmatrix_engine::model::DefinedName;

    fn book(values: &[(&str, f64)], matrix: &str) -> Workbook {
        let mut wb = Workbook::new();
        if let Some(sh) = wb.sheet_mut(0) {
            for (a1, v) in values {
                if let Some(c) = CellRef::parse(a1) {
                    sh.set_value(c, Value::Number(*v));
                }
            }
        }
        let formula = format!("Sheet1!{matrix}");
        wb.names.push(DefinedName { name: "A".into(), scope: None, formula, comment: String::new(), hidden: false });
        wb
    }

    fn all() -> RangeRef {
        RangeRef::parse("A1:Z99").unwrap_or(RangeRef::cell(CellRef::new(0, 0)))
    }

    #[test]
    fn a_mixed_sign_matrix_diverges_from_zero() {
        let wb = book(&[("A1", -2.0), ("B1", 4.0), ("A2", 0.0), ("B2", 1.0)], "$A$1:$B$2");
        let Some(sh) = wb.sheet(0) else { panic!("a sheet") };
        let h = HeatMap::new(&wb, 0, sh, all());
        assert_eq!(h.scales().get("A1:B2"), Some(&(-2.0, 4.0)));
        let s = Scale { lo: -2.0, hi: 4.0 };
        assert_eq!(s.place(4.0), 1.0);
        assert_eq!(s.place(0.0), 0.0);
        assert_eq!(s.place(-2.0), -0.5);
    }

    #[test]
    fn a_one_sign_matrix_runs_from_its_smallest_to_its_largest() {
        let s = Scale { lo: 2.0, hi: 10.0 };
        assert!((s.place(2.0) - FLOOR).abs() < 1e-6);
        assert_eq!(s.place(10.0), 1.0);
        let n = Scale { lo: -10.0, hi: -2.0 };
        assert_eq!(n.place(-10.0), -1.0);
        assert!((n.place(-2.0) + FLOOR).abs() < 1e-6);
        assert_eq!(Scale { lo: 3.0, hi: 3.0 }.place(3.0), 0.5);
    }

    #[test]
    fn only_numbers_inside_a_matrix_are_shaded() {
        let wb = book(&[("A1", 1.0), ("B1", 2.0), ("D1", 9.0)], "$A$1:$B$1");
        let Some(sh) = wb.sheet(0) else { panic!("a sheet") };
        let h = HeatMap::new(&wb, 0, sh, all());
        let t = Tokens::light();
        let at = |a1: &str| CellRef::parse(a1).unwrap_or(CellRef::new(0, 0));
        assert!(h.fill(at("A1"), Some(&Value::Number(1.0)), &t).is_some());
        assert!(h.fill(at("D1"), Some(&Value::Number(9.0)), &t).is_none(), "outside every matrix");
        assert!(h.fill(at("B1"), Some(&Value::Text("x".into())), &t).is_none(), "text");
        assert!(h.fill(at("B1"), None, &t).is_none(), "empty");
    }

    #[test]
    fn hostile_values_and_huge_blocks_are_left_alone() {
        let s = Scale { lo: f64::MIN, hi: f64::MAX };
        assert!(s.place(f64::MAX).is_finite());
        assert_eq!(Scale { lo: -1.0, hi: 1.0 }.place(f64::NAN), 0.0);
        let wb = book(&[("A1", 1.0)], "$A$1:$Z$100000");
        let Some(sh) = wb.sheet(0) else { panic!("a sheet") };
        assert!(HeatMap::new(&wb, 0, sh, all()).is_empty(), "past MAX_CELLS");
    }
}
