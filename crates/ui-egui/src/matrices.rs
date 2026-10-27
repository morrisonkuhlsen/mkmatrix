//! MK Matrix's named matrices: defined names that refer to one range on a sheet (`X` for
//! `Sheet1!$B$7:$D$14`). The Name Box shows a cell inside one by its place in the matrix,
//! `X(3,2)`, and takes that form to go there; the grid draws the matrix's brackets.

use egui::{Color32, Painter, Stroke, pos2};
use mkmatrix_engine::core::{CellRef, RangeRef};
use mkmatrix_engine::model::{Sheet, Workbook};

use crate::grid::Geo;

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
            let same = mkmatrix_engine::core::unquote_sheet(sheet_name).eq_ignore_ascii_case(&sheet.name);
            let range = RangeRef::parse(body)?;
            (same && !range.is_full_rows() && !range.is_full_cols()).then(|| (n.name.clone(), range))
        })
        .collect()
}

/// The smallest named matrix on sheet `si` holding `cell`, with the cell's 1-based row and column
/// in it.
pub fn containing(wb: &Workbook, si: usize, cell: CellRef) -> Option<(String, u32, u32)> {
    on_sheet(wb, si)
        .into_iter()
        .filter(|(_, r)| r.contains(cell))
        .min_by_key(|(_, r)| u64::from(r.height()) * u64::from(r.width()))
        .map(|(name, r)| (name, cell.row - r.start.row + 1, cell.col - r.start.col + 1))
}

/// How the Name Box shows a cell's place in a matrix: `X(3,2)`, or `y(4)` in a single column or
/// row.
pub fn local_index(name: &str, row: u32, col: u32, range: Option<RangeRef>) -> String {
    match range {
        Some(r) if r.width() == 1 => format!("{name}({row})"),
        Some(r) if r.height() == 1 => format!("{name}({col})"),
        _ => format!("{name}({row},{col})"),
    }
}

/// The Name Box text for the active cell when it is inside a named matrix.
pub fn name_box(wb: &Workbook, si: usize, cell: CellRef) -> Option<String> {
    let (name, row, col) = containing(wb, si, cell)?;
    let range = on_sheet(wb, si).into_iter().find(|(n, _)| *n == name).map(|(_, r)| r);
    Some(local_index(&name, row, col, range))
}

/// The cell `X(3,2)` (or `y(4)` for a vector) names on sheet `si`; `None` for anything else or an
/// index outside the matrix.
pub fn resolve(wb: &Workbook, si: usize, text: &str) -> Option<CellRef> {
    let (name, rest) = text.trim().split_once('(')?;
    let inner = rest.strip_suffix(')')?;
    let (_, range) = on_sheet(wb, si).into_iter().find(|(n, _)| mkmatrix_engine::formula::same_name(n, name.trim()))?;
    let idx: Vec<u32> = inner.split(',').map(|p| p.trim().parse().ok()).collect::<Option<Vec<u32>>>()?;
    let (row, col) = match idx.as_slice() {
        [i] if range.width() == 1 => (*i, 1),
        [j] if range.height() == 1 => (1, *j),
        [i, j] => (*i, *j),
        _ => return None,
    };
    if row == 0 || col == 0 || row > range.height() || col > range.width() {
        return None;
    }
    Some(CellRef::new(range.start.row + row - 1, range.start.col + col - 1))
}

/// Draws each matrix's square brackets just outside its range, as a matrix is written.
pub fn paint_brackets(p: &Painter, geo: &Geo, sh: &Sheet, matrices: &[(String, RangeRef)], color: Color32) {
    let stroke = Stroke::new(2.0, color);
    for (_, range) in matrices {
        let r = geo.range_rect(sh, *range);
        if !r.is_positive() || !r.intersects(p.clip_rect()) {
            continue;
        }
        let (top, bottom) = (r.top() - 2.0, r.bottom() + 2.0);
        let serif = (r.width() * 0.25).clamp(3.0, 6.0);
        for (x, dir) in [(r.left() - 3.0, 1.0), (r.right() + 3.0, -1.0)] {
            p.line_segment([pos2(x, top), pos2(x, bottom)], stroke);
            p.line_segment([pos2(x, top), pos2(x + dir * serif, top)], stroke);
            p.line_segment([pos2(x, bottom), pos2(x + dir * serif, bottom)], stroke);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mkmatrix_engine::model::DefinedName;

    fn book() -> Workbook {
        let mut wb = Workbook::new();
        for (name, formula, hidden) in [
            ("X", "Sheet1!$B$7:$D$14", false),
            ("y", "Sheet1!$F$7:$F$14", false),
            ("rate", "0.07", false),
            ("_xlnm.Print_Area", "Sheet1!$A$1:$H$20", false),
            ("Secret", "Sheet1!$A$1:$B$2", true),
        ] {
            wb.names.push(DefinedName { name: name.into(), scope: None, formula: formula.into(), comment: String::new(), hidden });
        }
        wb
    }

    #[test]
    fn only_names_of_one_range_are_matrices() {
        let names: Vec<String> = on_sheet(&book(), 0).into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, ["X", "y"]);
    }

    #[test]
    fn the_name_box_shows_and_takes_places_in_a_matrix() {
        let wb = book();
        let at = |a1: &str| CellRef::parse(a1).unwrap();
        assert_eq!(name_box(&wb, 0, at("C9")).as_deref(), Some("X(3,2)"));
        assert_eq!(name_box(&wb, 0, at("F10")).as_deref(), Some("y(4)"));
        assert_eq!(name_box(&wb, 0, at("A1")), None);
        assert_eq!(resolve(&wb, 0, "X(3,2)"), Some(at("C9")));
        assert_eq!(resolve(&wb, 0, " x( 8 , 3 ) "), Some(at("D14")));
        assert_eq!(resolve(&wb, 0, "y(4)"), Some(at("F10")));
        for bad in ["X(9,1)", "X(0,1)", "X(1)", "y(9)", "Z(1,1)", "X(a,b)", "X", "X(1,2,3)", "rate(1)"] {
            assert_eq!(resolve(&wb, 0, bad), None, "{bad}");
        }
    }
}
