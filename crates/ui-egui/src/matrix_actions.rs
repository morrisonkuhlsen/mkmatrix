//! MK Matrix's actions on a selected matrix: a narrow floating column just past a selected block
//! of numbers, in the gap before where its results go, so it never covers the labels above it. Naming it, transposing, inverting, its eigenvalues and SVD write the formula one column
//! past the block (the result spills there); multiplying and solving start the formula, `=X@` or
//! `=X\`, for the second matrix to be pointed at.

use egui::{Align2, Rect, Sense, pos2, vec2};
use mkmatrix_engine::core::{CellRef, MAX_COLS, MAX_ROWS, RangeRef};
use mkmatrix_engine::model::Sheet;
use serde_json::json;

use crate::SheetApp;
use crate::theme::{self, Tokens};

/// One action: its glyph, its tooltip, whether it needs a square matrix, what it does, and the
/// rows × columns its result takes for a block of the given rows × columns.
struct Action {
    glyph: &'static str,
    tip: &'static str,
    square: bool,
    kind: Kind,
    shape: fn(u32, u32) -> (u32, u32),
}

fn same(r: u32, c: u32) -> (u32, u32) {
    (r, c)
}
fn transposed(r: u32, c: u32) -> (u32, u32) {
    (c, r)
}
/// Eigenvalues: a column, or two (real and imaginary parts) when some are complex.
fn eigen(r: u32, _: u32) -> (u32, u32) {
    (r, 2)
}
fn singular(r: u32, c: u32) -> (u32, u32) {
    (r.min(c), 1)
}
/// A formula started for a second operand: only the cell it is typed in is known.
fn one(_: u32, _: u32) -> (u32, u32) {
    (1, 1)
}

enum Kind {
    Name,
    /// A formula written next to the block: `{}` is the matrix.
    Write(&'static str),
    /// A formula started for a second operand to be pointed at.
    Start(&'static str),
    /// Opens the Calculation by Hand panel.
    ByHand,
}

const ACTIONS: [Action; 8] = [
    Action { glyph: "A", tip: "Name as a matrix", square: false, kind: Kind::Name, shape: one },
    Action { glyph: "X'", tip: "Transpose", square: false, kind: Kind::Write("={}'"), shape: transposed },
    Action { glyph: "inv", tip: "Inverse", square: true, kind: Kind::Write("=MX.INV({})"), shape: same },
    Action { glyph: "@", tip: "Multiply by… (point at the second matrix)", square: false, kind: Kind::Start("={}@"), shape: one },
    Action { glyph: "\\", tip: "Solve… (point at the right-hand side)", square: false, kind: Kind::Start("={}\\"), shape: one },
    Action { glyph: "λ", tip: "Eigenvalues", square: true, kind: Kind::Write("=MX.EIG({})"), shape: eigen },
    Action { glyph: "σ", tip: "Singular values", square: false, kind: Kind::Write("=MX.SVD({})"), shape: singular },
    Action { glyph: "Σ", tip: "Calculation by hand", square: false, kind: Kind::ByHand, shape: one },
];

/// Draws the column of actions just right of the selection, level with its top (left of it when
/// there is no room on the right), when the selection is one block of numbers and nothing else is
/// going on.
pub(crate) fn show(app: &mut SheetApp, ctx: &egui::Context, selection: Rect, cells: Rect) {
    // Not while editing or dragging, nor while a fill's Auto Fill Options tag sits by the block.
    if app.editor.is_some()
        || app.grid.drag_select
        || app.grid.drag != crate::grid::Drag::None
        || app.grid.fill_tag.is_some()
        || app.grid.fill_menu.is_some()
    {
        return;
    }
    let Some(d) = app.session.active() else { return };
    let r = d.selection.current();
    if d.selection.ranges.len() != 1 || r.is_single() {
        return;
    }
    if crate::tabs::matrix_summary(app).is_none() || !selection.intersects(cells) {
        return;
    }
    let Some(d) = app.session.active() else { return };
    let square = r.height() == r.width();
    let reference = crate::matrices::on_sheet(&d.wb, d.wb.active_sheet()).into_iter().find(|(_, m)| *m == r).map_or_else(|| r.a1(), |(n, _)| n);
    let t = Tokens::get(ctx);
    let lang = app.ui.language;
    let (bw, bh) = (24.0, 22.0);
    let (width, height) = (bw + 4.0, bh * ACTIONS.len() as f32 + 4.0);
    // Clear of the matrix's bracket, which sits 3 points outside the block.
    const GAP: f32 = 8.0;
    let right = selection.right() + GAP;
    let x = if right + width <= cells.right() - 4.0 { right } else { (selection.left() - GAP - width).max(cells.left() + 4.0) };
    let y = selection.top().clamp(cells.top() + 4.0, (cells.bottom() - height - 4.0).max(cells.top()));
    let mut picked = None;
    egui::Area::new(egui::Id::unique("mkmatrix.matrix_actions")).order(egui::Order::Foreground).fixed_pos(pos2(x, y)).show(ctx, |ui| {
        egui::Frame::NONE.fill(t.menu_bg).stroke(egui::Stroke::new(1.0, t.ribbon_border)).inner_margin(2.0).show(ui, |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for (i, a) in ACTIONS.iter().enumerate() {
                    let enabled = square || !a.square;
                    let (rect, resp) = ui.allocate_exact_size(vec2(bw, bh), if enabled { Sense::click() } else { Sense::hover() });
                    let tip = lang.tr(a.tip);
                    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, tip));
                    if enabled && resp.hovered() {
                        ui.painter().rect_filled(rect, 0.0, t.hover);
                    }
                    let ink = if enabled { t.text } else { t.text_disabled };
                    if matches!(a.kind, Kind::Name) {
                        crate::icons::paint(ui.painter(), Rect::from_center_size(rect.center(), vec2(14.0, 14.0)), crate::icons::Icon::Name, ink);
                    } else {
                        ui.painter().text(rect.center(), Align2::CENTER_CENTER, a.glyph, theme::ui_font(13.0), if enabled { t.accent } else { ink });
                    }
                    let resp = resp.on_hover_text(tip);
                    if enabled && resp.clicked() {
                        picked = Some(i);
                    }
                }
            });
        });
    });
    let Some(action) = picked.and_then(|i| ACTIONS.get(i)) else { return };
    let mut target = || {
        let at = app.session.active().and_then(|d| d.wb.active()).and_then(|sh| free_spot(sh, r, (action.shape)(r.height(), r.width())));
        if at.is_none() {
            app.message = Some(("MK Matrix".into(), lang.tr("There are no empty cells next to the matrix for the result.").to_string()));
        }
        at
    };
    match action.kind {
        Kind::Name => app.open_dialog("defineName", json!({})),
        Kind::ByHand => app.run_or_alert("view.byHand", json!({"on": true})),
        Kind::Write(f) => {
            let Some(target) = target() else { return };
            app.run_or_alert("cell.set", json!({"cell": target.a1(), "input": f.replace("{}", &reference)}));
            app.run_or_alert("selection.set", json!({"range": RangeRef::cell(target).a1()}));
            app.grid.ensure_visible = true;
        }
        Kind::Start(f) => {
            let Some(target) = target() else { return };
            app.run_or_alert("selection.set", json!({"range": RangeRef::cell(target).a1()}));
            app.begin_edit(Some(f.replace("{}", &reference)), false);
            app.grid.ensure_visible = true;
        }
    }
}

/// Where a result of `rows` × `cols` goes without covering anything: one column past the block,
/// level with its top, or the first column further right whose cells are all empty; failing that,
/// one row below the block, or further down. `None` when nothing within reach is free.
fn free_spot(sh: &Sheet, block: RangeRef, (rows, cols): (u32, u32)) -> Option<CellRef> {
    const REACH: u32 = 64;
    let right = (0..REACH).filter_map(|k| block.end.col.checked_add(2 + k).map(|col| CellRef::new(block.start.row, col)));
    let below = (0..REACH).filter_map(|k| block.end.row.checked_add(2 + k).map(|row| CellRef::new(row, block.start.col)));
    right.chain(below).find(|&at| {
        let (Some(last_row), Some(last_col)) = (at.row.checked_add(rows.saturating_sub(1)), at.col.checked_add(cols.saturating_sub(1))) else {
            return false;
        };
        last_row < MAX_ROWS
            && last_col < MAX_COLS
            && (at.row..=last_row).all(|row| {
                (at.col..=last_col).all(|col| {
                    let c = CellRef::new(row, col);
                    !sh.cell(c).is_some_and(|cell| cell.formula.is_some() || !cell.value.is_empty()) && sh.value_ref(c).is_none_or(|v| v.is_empty())
                })
            })
    })
}
