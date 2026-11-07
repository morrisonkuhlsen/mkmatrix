//! MK Matrix's named matrices: defined names that refer to one range on a sheet (`X` for
//! `Sheet1!$B$7:$D$14`). The Name Box shows a cell inside one by its place in the matrix,
//! `X(3,2)`, and takes that form to go there; the grid draws the matrix's brackets with its name
//! beside them, lit while the pointer is over either.

use egui::{Align2, Color32, FontFamily, FontId, Painter, Rect, Stroke, pos2, vec2};
use mkmatrix_engine::core::{CellRef, RangeRef, Value};
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

/// The named matrices on every sheet, in sheet order: the names a formula may cite, wherever it
/// is typed.
pub fn all(wb: &Workbook) -> Vec<(String, RangeRef)> {
    (0..wb.sheets.len()).flat_map(|si| on_sheet(wb, si)).collect()
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

/// The smallest named matrix on sheet `si` holding `cell`.
pub fn containing_range(wb: &Workbook, si: usize, cell: CellRef) -> Option<RangeRef> {
    on_sheet(wb, si).into_iter().map(|(_, r)| r).filter(|r| r.contains(cell)).min_by_key(|r| u64::from(r.height()) * u64::from(r.width()))
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

/// The size of a matrix's name beside its bracket, at 100% zoom.
const NAME_SIZE: f32 = 18.0;
/// The gap between the name and the bracket.
const NAME_GAP: f32 = 7.0;

/// The estimated width of a matrix's name at zoom `z`.
fn name_width(name: &str, z: f32) -> f32 {
    name.chars().count() as f32 * NAME_SIZE * z * 0.62 + 2.0
}

/// The width (in pixels at 100%) the column left of matrix `name` needs beyond its contents to
/// show the name beside them: what AutoFit adds to that column.
pub fn name_room(name: &str) -> f32 {
    (name_width(name, 1.0) + NAME_GAP + 3.0 + 4.0).ceil()
}

/// The room AutoFit leaves in column `col` of sheet `si` for the names of the matrices that start
/// just right of it.
pub fn room_left_of(wb: &Workbook, si: usize, col: u32) -> f32 {
    on_sheet(wb, si).iter().filter(|(_, r)| r.start.col == col.saturating_add(1)).map(|(n, _)| name_room(n)).fold(0.0, f32::max)
}

/// Where a matrix's name sits, if anywhere: left of the left bracket, centred on its height;
/// else under the matrix, then over it, at its left bracket. Numbers in the column beside the
/// bracket hug it, so over them the name moves to that column's left edge when the column has
/// room for both (AutoFit's double click leaves it, [`name_room`]). A place is taken only where
/// the name covers no cell's contents (a heading above, a label beside); with none free the name
/// shows only in the matrix's tag (see [`paint_brackets`]). The width is estimated, so
/// hit-testing needs no fonts; the name is drawn right-aligned inside it.
fn name_rect(geo: &Geo, sh: &Sheet, range: RangeRef, r: Rect, name: &str) -> Option<Rect> {
    let size = NAME_SIZE * geo.z;
    let w = name_width(name, geo.z);
    let right = r.left() - 3.0 - NAME_GAP * geo.z;
    let (top, bottom) = (r.center().y - size * 0.7, r.center().y + size * 0.7);
    if range.start.col > 0 {
        let col = range.start.col - 1;
        let left = geo.cell_rect(sh, CellRef::new(range.start.row, col)).left() + 4.0 * geo.z;
        let (first, last) = (geo.row_at(sh, top).max(range.start.row), geo.row_at(sh, bottom).min(range.end.row));
        let hugging = (first..=last).any(|row| matches!(sh.value(CellRef::new(row, col)), Value::Number(_) | Value::Bool(_) | Value::Error(_)));
        let only_numbers = (first..=last).all(|row| !matches!(sh.value(CellRef::new(row, col)), Value::Text(_)));
        if hugging && only_numbers && left >= geo.cells.left() && left + w <= right {
            return Some(Rect::from_min_max(pos2(left, top), pos2(left + w, bottom)));
        }
    }
    let x0 = r.left() - 3.0;
    let candidates = [
        Rect::from_min_max(pos2(right - w, top), pos2(right, bottom)),
        // Under or over the matrix the name keeps to the one row beside the bracket's serif.
        Rect::from_min_size(pos2(x0, r.bottom() + 2.0), vec2(w, size * 1.05)),
        Rect::from_min_max(pos2(x0, r.top() - 2.0 - size * 1.05), pos2(x0 + w, r.top() - 2.0)),
    ];
    // The letters' ink is shorter than the font's line box: only it must stay clear of the
    // neighbours' contents (a name a size larger than a row still fits in one).
    let ink = vec2(0.0, size * 0.2);
    candidates.into_iter().find(|c| c.left() >= geo.cells.left() && c.top() >= geo.cells.top() && covers_nothing(geo, sh, range, c.shrink2(ink)))
}

/// Whether `rect` (sheet coordinates) lies over no cell with contents outside `range`.
fn covers_nothing(geo: &Geo, sh: &Sheet, range: RangeRef, rect: Rect) -> bool {
    let (r0, r1) = (geo.row_at(sh, rect.top()), geo.row_at(sh, rect.bottom() - 1.0));
    let (c0, c1) = (geo.col_at(sh, rect.left()), geo.col_at(sh, rect.right() - 1.0));
    // A name is a few cells wide and tall at most; past that, call it covered.
    if r1.saturating_sub(r0) > 8 || c1.saturating_sub(c0) > 8 {
        return false;
    }
    (r0..=r1).all(|row| {
        (c0..=c1).all(|col| {
            let c = CellRef::new(row, col);
            range.contains(c) || sh.value(c).is_empty()
        })
    })
}

/// The named matrix whose bracket or name is at `p` (sheet coordinates): within a few points of
/// either bracket's upright, along the matrix's height, or over its name.
pub fn bracket_at(geo: &Geo, sh: &Sheet, matrices: &[(String, RangeRef)], p: egui::Pos2) -> Option<(String, RangeRef)> {
    matrices
        .iter()
        .find(|(name, range)| {
            let r = geo.range_rect(sh, *range);
            let on_upright = (p.x - (r.left() - 3.0)).abs() <= 4.0 || (p.x - (r.right() + 3.0)).abs() <= 4.0;
            (on_upright && p.y >= r.top() - 3.0 && p.y <= r.bottom() + 3.0)
                || name_rect(geo, sh, *range, r, name).is_some_and(|n| n.expand(2.0).contains(p))
        })
        .cloned()
}

/// The brackets' stroke at 100%.
const BRACKET_WIDTH: f32 = 1.5;
/// How strongly a spill's brackets show beside a named matrix's.
const SPILL_INK: f32 = 0.5;
/// The tag's text size at 100%.
const TAG_SIZE: f32 = 11.5;

/// The lines of a block's square brackets, just outside `r`.
fn bracket_lines(r: Rect) -> Vec<[egui::Pos2; 2]> {
    let (top, bottom) = (r.top() - 2.0, r.bottom() + 2.0);
    let serif = (r.width() * 0.25).clamp(3.0, 6.0);
    let mut lines = Vec::new();
    for (x, dir) in [(r.left() - 3.0, 1.0), (r.right() + 3.0, -1.0)] {
        lines.push([pos2(x, top), pos2(x, bottom)]);
        lines.push([pos2(x, top), pos2(x + dir * serif, top)]);
        lines.push([pos2(x, bottom), pos2(x + dir * serif, bottom)]);
    }
    lines
}

/// A block's tag, `X · 8×3` (or `8×3` for a spill): over its top-left corner, under it when the
/// sheet's edge leaves no room above. On the sheet's colour, so it reads over a heading.
pub(crate) fn paint_tag(p: &Painter, geo: &Geo, r: Rect, text: &str, ink: Color32, ground: Color32) {
    paint_tag_placed(p, geo, r, text, ink, ground, false);
}

/// A tag like [`paint_tag`]'s, under `r` (over it when the sheet's edge leaves no room below):
/// for what is written about a block rather than its name, so the rows above stay in sight.
pub(crate) fn paint_tag_under(p: &Painter, geo: &Geo, r: Rect, text: &str, ink: Color32, ground: Color32) {
    paint_tag_placed(p, geo, r, text, ink, ground, true);
}

fn paint_tag_placed(p: &Painter, geo: &Geo, r: Rect, text: &str, ink: Color32, ground: Color32, under: bool) {
    let font = FontId::new(TAG_SIZE * geo.z, FontFamily::Name(crate::theme::MONO.into()));
    let galley = p.layout_no_wrap(text.to_string(), font, ink);
    let pad = vec2(4.0, 2.0) * geo.z;
    let size = galley.size() + pad * 2.0;
    let (above, below) = (pos2(r.left() - 3.0, r.top() - 3.0 - size.y), pos2(r.left() - 3.0, r.bottom() + 3.0));
    let at = match under {
        false if above.y >= geo.cells.top() => above,
        false => below,
        true if below.y + size.y <= geo.cells.bottom() => below,
        true => above,
    };
    let tag = Rect::from_min_size(at, size);
    p.rect_filled(tag, 3.0 * geo.z, ground);
    p.rect_stroke(tag, 3.0 * geo.z, Stroke::new(1.0, ink.gamma_multiply(0.5)), egui::StrokeKind::Inside);
    p.galley(tag.min + pad, galley, ink);
}

/// A block's rows×columns, as the tag shows it.
fn dims(r: RangeRef) -> String {
    format!("{}×{}", r.height(), r.width())
}

/// Draws each matrix's square brackets just outside its range, as a matrix is written, and its
/// name beside them in italics. The matrix under `hover` (sheet coordinates) glows. The matrix
/// under the pointer or holding the active cell also shows its tag, `X · 8×3`. A spill that is
/// no named matrix shows lighter brackets and its tag, `8×1`, only while the pointer is over it or
/// the active cell is in it, so blocks side by side don't crowd each other. While a formula is
/// typed, the matrices it cites (`cited`: name and colour) take their reference colours and
/// show their tags, as a cited range takes its colour on the sheet.
#[allow(clippy::too_many_arguments)]
pub fn paint_brackets(
    p: &Painter,
    geo: &Geo,
    sh: &Sheet,
    matrices: &[(String, RangeRef)],
    color: Color32,
    ground: Color32,
    hover: Option<egui::Pos2>,
    active: Option<CellRef>,
    cited: &[(String, Color32)],
) {
    let lit = hover.and_then(|h| bracket_at(geo, sh, matrices, h)).map(|(n, _)| n);
    let inside = |range: RangeRef, r: Rect| hover.is_some_and(|h| r.expand(4.0).contains(h)) || active.is_some_and(|a| range.contains(a));
    let font = FontId::new(NAME_SIZE * geo.z, FontFamily::Name(crate::theme::CELL_ITALIC.into()));
    let mut tags: Vec<(Rect, String, Color32)> = Vec::new();
    // Spills first, under the named matrices' brackets.
    for (anchor, range) in &sh.spill_ranges {
        if range.count() < 2 || matrices.iter().any(|(_, m)| m == range) {
            continue;
        }
        let r = geo.range_rect(sh, *range);
        if !r.is_positive() || !r.expand(8.0).intersects(p.clip_rect()) {
            continue;
        }
        if !inside(*range, r) || matrices.iter().any(|(_, m)| m.contains(*anchor)) {
            continue;
        }
        let ink = color.gamma_multiply(SPILL_INK);
        for l in bracket_lines(r) {
            p.line_segment(l, Stroke::new(1.0, ink));
        }
        tags.push((r, dims(*range), color));
    }
    for (name, range) in matrices {
        let r = geo.range_rect(sh, *range);
        let label = name_rect(geo, sh, *range, r, name);
        let near = r.expand(8.0).intersects(p.clip_rect()) || label.is_some_and(|l| l.intersects(p.clip_rect()));
        if !r.is_positive() || !near {
            continue;
        }
        let glow = lit.as_deref() == Some(name.as_str());
        let quoted = cited.iter().find(|(n, _)| n == name).map(|(_, c)| *c);
        let color = quoted.unwrap_or(color);
        let lines = bracket_lines(r);
        let text_at = label.map(|l| pos2(l.right(), l.center().y));
        if glow {
            // A soft halo: wide, faint strokes under the brackets and copies of the name around it.
            for (width, alpha) in [(12.0, 0.10), (8.0, 0.18), (5.0, 0.30)] {
                let halo = Stroke::new(width, color.gamma_multiply(alpha));
                for l in &lines {
                    p.line_segment(*l, halo);
                }
            }
            if let Some(at) = text_at {
                for (dx, dy) in [(-1.5, 0.0), (1.5, 0.0), (0.0, -1.5), (0.0, 1.5), (-1.0, -1.0), (1.0, 1.0), (-1.0, 1.0), (1.0, -1.0)] {
                    p.text(at + vec2(dx, dy) * 1.4, Align2::RIGHT_CENTER, name, font.clone(), color.gamma_multiply(0.18));
                    p.text(at + vec2(dx, dy) * 0.7, Align2::RIGHT_CENTER, name, font.clone(), color.gamma_multiply(0.30));
                }
            }
        }
        let ink = if glow { color.lerp_to_gamma(Color32::WHITE, 0.4) } else { color };
        let stroke = Stroke::new(if glow { BRACKET_WIDTH + 0.5 } else { BRACKET_WIDTH }, ink);
        for l in &lines {
            p.line_segment(*l, stroke);
        }
        if let Some(at) = text_at {
            p.text(at, Align2::RIGHT_CENTER, name, font.clone(), ink);
        }
        if glow || quoted.is_some() || inside(*range, r) {
            tags.push((r, format!("{name} · {}", dims(*range)), color));
        }
    }
    // Tags last, over everything they may cover.
    for (r, text, ink) in tags {
        paint_tag(p, geo, r, &text, ink, ground);
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

    #[test]
    fn the_headings_know_the_matrix_the_cursor_is_in() {
        let wb = book();
        let at = |a1: &str| CellRef::parse(a1).unwrap();
        assert_eq!(containing_range(&wb, 0, at("C9")).map(|r| r.a1()).as_deref(), Some("B7:D14"));
        assert_eq!(containing_range(&wb, 0, at("F7")).map(|r| r.a1()).as_deref(), Some("F7:F14"));
        assert_eq!(containing_range(&wb, 0, at("A1")), None);
    }

    /// In the regression sample every matrix's name finds a place, and none lies over another
    /// cell's contents (`beta` used to be written over the `Hours` label beside it).
    #[test]
    fn names_keep_off_the_cells_around_them() {
        let Some(wb) = mkmatrix_engine::sample::build("regression") else { panic!("the regression sample builds") };
        let Some(sh) = wb.sheet(0) else { panic!("it has a sheet") };
        let geo = Geo::new(sh, Rect::from_min_size(pos2(0.0, 0.0), vec2(1400.0, 800.0)), vec2(0.0, 0.0));
        let matrices = on_sheet(&wb, 0);
        assert!(matrices.len() >= 3, "{matrices:?}");
        for (name, range) in &matrices {
            let r = geo.range_rect(sh, *range);
            let Some(label) = name_rect(&geo, sh, *range, r, name) else { panic!("{name} has no place for its name") };
            let ink = label.shrink2(vec2(0.0, NAME_SIZE * geo.z * 0.2));
            assert!(covers_nothing(&geo, sh, *range, ink), "{name}'s name at {label:?} covers a cell");
        }
    }
}
