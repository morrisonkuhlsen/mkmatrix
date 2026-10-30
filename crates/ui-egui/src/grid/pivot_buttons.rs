//! A PivotTable's buttons on its sheet (#71): the dropdown on each area's header cell (Row Labels,
//! Column Labels, or a field's name in Outline and Tabular form), which opens the AutoFilter's menu
//! to sort and filter the area's fields through `pivot.sort` and `pivot.filter`, and the ⊕/⊖ of
//! each outer item, which runs `pivot.collapse`. The engine lists them for a report that is current
//! (`pivot::buttons`); the grid keeps them per workbook revision, as computing them computes the
//! whole report.

use mkmatrix_engine::pivot::ButtonKind;

use super::geo::Geo;
use super::tables::paint_filter_button;
use super::*;

/// A button of the PivotTable `pivot` (its id) on `cell` of the active sheet.
#[derive(Clone, Debug, PartialEq)]
pub struct PivotButton {
    pub cell: CellRef,
    pub pivot: u32,
    pub kind: ButtonKind,
}

/// Brings the cached buttons up to date with the active sheet: call once a frame before painting.
pub(super) fn refresh(app: &mut SheetApp) {
    let Some(d) = app.session.active() else {
        app.grid.pivot_buttons = None;
        return;
    };
    let si = d.wb.active_sheet();
    let key = (d.uid, d.revision, si);
    if app.grid.pivot_buttons.as_ref().is_some_and(|(k, _)| *k == key) {
        return;
    }
    let mut out = Vec::new();
    if let Some(sh) = d.wb.sheet(si) {
        for pt in &sh.pivots {
            let (Some(r), Ok(buttons)) = (pt.last_range, mkmatrix_engine::pivot::buttons(&d.wb, pt, si)) else { continue };
            out.extend(buttons.into_iter().map(|b| PivotButton {
                cell: CellRef::new(r.start.row.saturating_add(b.row), r.start.col.saturating_add(b.col)),
                pivot: pt.id,
                kind: b.kind,
            }));
        }
    }
    app.grid.pivot_buttons = Some((key, out));
}

/// The buttons on the active sheet, as the last [`refresh`] found them.
pub(super) fn cached(app: &SheetApp) -> &[PivotButton] {
    app.grid.pivot_buttons.as_ref().map_or(&[], |(_, b)| b.as_slice())
}

/// The area header dropdown on `c`: (the PivotTable's id, the fields it sorts and filters).
pub(super) fn field_at(app: &SheetApp, c: CellRef) -> Option<(u32, Vec<String>)> {
    cached(app).iter().find_map(|b| match &b.kind {
        ButtonKind::Field { fields, .. } if b.cell == c => Some((b.pivot, fields.clone())),
        _ => None,
    })
}

/// Whether `c` holds an area header's dropdown (Alt+Down opens it).
pub(crate) fn has_field(app: &SheetApp, c: CellRef) -> bool {
    field_at(app, c).is_some()
}

/// A header's dropdown, in the bottom-right corner as the AutoFilter's is.
pub fn field_rect(geo: &Geo, sh: &Sheet, c: CellRef) -> Rect {
    let rect = geo.cell_rect(sh, c);
    let pad = 2.0 * geo.z;
    let s = (rect.height() - 4.0).clamp(10.0, 18.0).min((rect.width() - 2.0 * pad).max(0.0));
    Rect::from_min_size(pos2(rect.right() - s - pad, rect.bottom() - s - pad), vec2(s, s))
}

/// An item's ⊕/⊖: where its label would start (after its indent), the label moved past it.
pub fn toggle_rect(geo: &Geo, wb: &Workbook, sh: &Sheet, c: CellRef) -> Rect {
    let rect = geo.cell_rect(sh, c);
    let z = geo.z;
    let indent = f32::from(wb.styles.get(sh.style_id(c)).align.indent) * 9.0 * z;
    let s = toggle_size(z);
    Rect::from_min_size(pos2(rect.left() + 3.0 * z + indent, rect.center().y - s / 2.0), vec2(s, s))
}

/// The ⊕/⊖ box's side, and so how far the label moves right of it (plus a gap).
pub(super) fn toggle_size(z: f32) -> f32 {
    (9.0 * z).clamp(7.0, 16.0)
}

/// A ⊕ (collapsed: the items under it hidden) or ⊖, in the sheet controls' colours.
pub(super) fn paint_toggle(p: &Painter, r: Rect, collapsed: bool) {
    let t = Tokens::get(p.ctx());
    p.rect_filled(r, 1.0, t.sheet_control_bg);
    p.rect_stroke(r, 1.0, Stroke::new(1.0, t.sheet_control_border), StrokeKind::Inside);
    let arm = r.width() * 0.28;
    let stroke = Stroke::new(1.0, t.sheet_control_text);
    let c = r.center();
    p.line_segment([pos2(c.x - arm, c.y), pos2(c.x + arm, c.y)], stroke);
    if collapsed {
        p.line_segment([pos2(c.x, c.y - arm), pos2(c.x, c.y + arm)], stroke);
    }
}

/// The header dropdowns in `range`, drawn as filtering while one of their fields shows only some
/// items. (The toggles are drawn with the labels they sit before; see `paint_quadrant`.)
pub(super) fn paint_fields(p: &Painter, geo: &Geo, sh: &Sheet, range: RangeRef, buttons: &[PivotButton]) {
    for b in buttons {
        let ButtonKind::Field { fields, .. } = &b.kind else { continue };
        if !range.contains(b.cell) {
            continue;
        }
        let filtered =
            sh.pivots.iter().find(|pt| pt.id == b.pivot).is_some_and(|pt| {
                pt.filters.iter().any(|f| f.selected.is_some() && fields.iter().any(|name| name.eq_ignore_ascii_case(&f.source_col)))
            });
        paint_filter_button(p, field_rect(geo, sh, b.cell), filtered);
    }
}

/// The button under `p` (logical coordinates), if any.
pub(super) fn hit(app: &SheetApp, geo: &Geo, wb: &Workbook, sh: &Sheet, p: Pos2) -> Option<PivotButton> {
    let c = geo.cell_at(sh, p);
    cached(app)
        .iter()
        .find(|b| {
            b.cell == c
                && match b.kind {
                    ButtonKind::Field { .. } => field_rect(geo, sh, c),
                    ButtonKind::Toggle { .. } => toggle_rect(geo, wb, sh, c).expand(2.0 * geo.z),
                }
                .contains(p)
        })
        .cloned()
}
