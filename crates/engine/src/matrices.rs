//! MK Matrix's named matrices: defined names that refer to one range on a sheet (`X` for
//! `Sheet1!$B$7:$D$14`), and where a result computed from one goes without covering anything.

use mkmatrix_core::{CellRef, MAX_COLS, MAX_ROWS, RangeRef};
use mkmatrix_model::{Sheet, Workbook};

/// The named matrices on sheet `si`: names in the workbook's or that sheet's scope whose formula is
/// one range on it. Hidden names and Excel's own (`_xlnm.Print_Area`…) are not matrices.
pub fn on_sheet(wb: &Workbook, si: usize) -> Vec<(String, RangeRef)> {
    let Some(sheet) = wb.sheet(si) else { return Vec::new() };
    wb.names
        .iter()
        .filter(|n| !n.hidden && !n.name.starts_with('_') && n.scope.is_none_or(|s| s == si))
        .filter_map(|n| {
            let target = n.formula.trim_start_matches('=').replace('$', "");
            let (sheet_name, body) = target.rsplit_once('!')?;
            let same = mkmatrix_core::unquote_sheet(sheet_name).eq_ignore_ascii_case(&sheet.name);
            let range = RangeRef::parse(body)?;
            (same && !range.is_full_rows() && !range.is_full_cols()).then(|| (n.name.clone(), range))
        })
        .collect()
}

/// How a formula on sheet `si` cites the block `r` there: by its name when it is a named matrix,
/// else by its address.
pub fn reference(wb: &Workbook, si: usize, r: RangeRef) -> String {
    on_sheet(wb, si).into_iter().find(|(_, m)| *m == r).map_or_else(|| r.a1(), |(n, _)| n)
}

/// How far right of (or below) a block [`free_spot`] looks.
const REACH: u32 = 64;

/// Where a result of `rows` × `cols` goes without covering anything: one column past the block,
/// level with its top, or the first column further right whose cells are all empty; failing that,
/// one row below the block, or further down. `None` when nothing within reach is free.
pub fn free_spot(sh: &Sheet, block: RangeRef, (rows, cols): (u32, u32)) -> Option<CellRef> {
    let right = (0..REACH).filter_map(|k| block.end.col.checked_add(2 + k).map(|col| CellRef::new(block.start.row, col)));
    let below = (0..REACH).filter_map(|k| block.end.row.checked_add(2 + k).map(|row| CellRef::new(row, block.start.col)));
    right.chain(below).find(|&at| {
        let (Some(last_row), Some(last_col)) = (at.row.checked_add(rows.saturating_sub(1)), at.col.checked_add(cols.saturating_sub(1))) else {
            return false;
        };
        last_row < MAX_ROWS && last_col < MAX_COLS && is_free(sh, RangeRef::new(at, CellRef::new(last_row, last_col)))
    })
}

/// Whether no cell of `area` has contents, its own or spilled into it.
fn is_free(sh: &Sheet, area: RangeRef) -> bool {
    !sh.cells.iter_range(area).any(|(_, c)| c.formula.is_some() || !c.value.is_empty()) && !sh.spill_ranges.values().any(|r| r.intersects(&area))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mkmatrix_core::Value;
    use mkmatrix_model::DefinedName;

    fn at(a1: &str) -> CellRef {
        CellRef::parse(a1).unwrap_or_default()
    }

    #[test]
    fn a_result_goes_past_the_block_skipping_filled_columns() {
        let mut wb = Workbook::new();
        let Some(sh) = wb.sheet_mut(0) else { return };
        let block = RangeRef::new(at("A1"), at("B2"));
        assert_eq!(free_spot(sh, block, (2, 2)), Some(at("D1")));
        sh.set_value(at("E2"), Value::Number(1.0));
        // D1:E2 holds E2; E1… too; F1:G2 is free.
        assert_eq!(free_spot(sh, block, (2, 2)), Some(at("F1")));
        assert_eq!(free_spot(sh, block, (1, 1)), Some(at("D1")));
    }

    #[test]
    fn a_named_block_is_cited_by_its_name() {
        let mut wb = Workbook::new();
        wb.names.push(DefinedName { name: "X".into(), scope: None, formula: "Sheet1!$B$2:$C$3".into(), comment: String::new(), hidden: false });
        assert_eq!(reference(&wb, 0, RangeRef::new(at("B2"), at("C3"))), "X");
        assert_eq!(reference(&wb, 0, RangeRef::new(at("B2"), at("C4"))), "B2:C4");
    }
}
