//! MK Matrix's actions on a selected matrix: a small floating bar beside a selected block of
//! numbers. Naming it, transposing, inverting, its eigenvalues and SVD write the formula one column
//! past the block (the result spills there); multiplying and solving start the formula, `=X@` or
//! `=X\`, for the second matrix to be pointed at.

use egui::{Align2, Rect, Sense, pos2, vec2};
use mkmatrix_engine::core::{CellRef, RangeRef};
use serde_json::json;

use crate::SheetApp;
use crate::theme::{self, Tokens};

/// One action: its glyph, its tooltip, whether it needs a square matrix, and what it does.
struct Action {
    glyph: &'static str,
    tip: &'static str,
    square: bool,
    kind: Kind,
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
    Action { glyph: "A", tip: "Name as a matrix", square: false, kind: Kind::Name },
    Action { glyph: "X'", tip: "Transpose", square: false, kind: Kind::Write("={}'") },
    Action { glyph: "inv", tip: "Inverse", square: true, kind: Kind::Write("=MX.INV({})") },
    Action { glyph: "@", tip: "Multiply by… (point at the second matrix)", square: false, kind: Kind::Start("={}@") },
    Action { glyph: "\\", tip: "Solve… (point at the right-hand side)", square: false, kind: Kind::Start("={}\\") },
    Action { glyph: "λ", tip: "Eigenvalues", square: true, kind: Kind::Write("=MX.EIG({})") },
    Action { glyph: "σ", tip: "Singular values", square: false, kind: Kind::Write("=MX.SVD({})") },
    Action { glyph: "Σ", tip: "Calculation by hand", square: false, kind: Kind::ByHand },
];

/// Draws the bar above the selection's top-right corner (below it when there is no room above),
/// when the selection is one block of numbers and nothing else is going on.
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
    let (bw, bh) = (30.0, 24.0);
    let width = bw * ACTIONS.len() as f32 + 8.0;
    let above = selection.top() - bh - 14.0;
    let y = if above >= cells.top() { above } else { (selection.bottom() + 6.0).min(cells.bottom() - bh - 8.0) };
    let x = (selection.right() - width).clamp(cells.left() + 4.0, (cells.right() - width - 4.0).max(cells.left()));
    let mut picked = None;
    egui::Area::new(egui::Id::unique("mkmatrix.matrix_actions")).order(egui::Order::Foreground).fixed_pos(pos2(x, y)).show(ctx, |ui| {
        egui::Frame::NONE.fill(t.menu_bg).stroke(egui::Stroke::new(1.0, t.ribbon_border)).inner_margin(4.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
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
                        crate::icons::paint(ui.painter(), Rect::from_center_size(rect.center(), vec2(16.0, 16.0)), crate::icons::Icon::Name, ink);
                    } else {
                        ui.painter().text(rect.center(), Align2::CENTER_CENTER, a.glyph, theme::ui_font(14.0), if enabled { t.accent } else { ink });
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
    // The result goes one column past the block, level with its top.
    let target = CellRef::new(r.start.row, r.end.col.saturating_add(2).min(mkmatrix_engine::core::MAX_COLS - 1));
    match action.kind {
        Kind::Name => app.open_dialog("defineName", json!({})),
        Kind::ByHand => app.ui.by_hand = true,
        Kind::Write(f) => {
            app.run_or_alert("cell.set", json!({"cell": target.a1(), "input": f.replace("{}", &reference)}));
            app.run_or_alert("selection.set", json!({"range": RangeRef::cell(target).a1()}));
            app.grid.ensure_visible = true;
        }
        Kind::Start(f) => {
            app.run_or_alert("selection.set", json!({"range": RangeRef::cell(target).a1()}));
            app.begin_edit(Some(f.replace("{}", &reference)), false);
            app.grid.ensure_visible = true;
        }
    }
}
