//! MK Matrix's actions on a selected matrix: a narrow floating column just past a selected block
//! of numbers, in the gap before where its results go, so it never covers the labels above it.
//! Transposing, inverting, its eigenvalues and SVD run the `matrix.*` commands, which write the
//! formula in a free spot past the block (the result spills there); multiplying and solving start
//! the formula, `=X@` or `=X\`, for the second matrix to be pointed at.

use egui::{Align2, Rect, Sense, pos2, vec2};
use mkmatrix_engine::core::RangeRef;
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
    /// A `matrix.*` command, which writes its formula next to the block.
    Run(&'static str),
    /// A formula started for a second operand to be pointed at.
    Start(&'static str),
    /// Opens the Calculation by Hand panel.
    ByHand,
}

const ACTIONS: [Action; 8] = [
    Action { glyph: "A", tip: "Name as a matrix", square: false, kind: Kind::Name },
    Action { glyph: "X'", tip: "Transpose", square: false, kind: Kind::Run("matrix.transpose") },
    Action { glyph: "inv", tip: "Inverse", square: true, kind: Kind::Run("matrix.inverse") },
    Action { glyph: "@", tip: "Multiply by… (point at the second matrix)", square: false, kind: Kind::Start("={}@") },
    Action { glyph: "\\", tip: "Solve… (point at the right-hand side)", square: false, kind: Kind::Start("={}\\") },
    Action { glyph: "λ", tip: "Eigenvalues", square: true, kind: Kind::Run("matrix.eigenvalues") },
    Action { glyph: "σ", tip: "Singular values", square: false, kind: Kind::Run("matrix.singularValues") },
    Action { glyph: "Σ", tip: "Calculation by hand", square: false, kind: Kind::ByHand },
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
    match action.kind {
        Kind::Name => app.open_dialog("defineName", json!({})),
        Kind::ByHand => app.run_or_alert("view.byHand", json!({"on": true})),
        Kind::Run(id) => run(app, id),
        Kind::Start(f) => {
            let spot = app.session.active().and_then(|d| d.wb.active()).and_then(|sh| mkmatrix_engine::matrices::free_spot(sh, r, (1, 1)));
            let Some(target) = spot else {
                app.message = Some(("MK Matrix".into(), lang.tr("There are no empty cells next to the matrix for the result.").to_string()));
                return;
            };
            app.run_or_alert("selection.set", json!({"range": RangeRef::cell(target).a1()}));
            app.begin_edit(Some(f.replace("{}", &reference)), false);
            app.grid.ensure_visible = true;
        }
    }
}

/// Runs matrix command `id` on the selection and selects the cell its result starts in.
pub(crate) fn run(app: &mut SheetApp, id: &str) {
    let out = match app.run(id, json!({})) {
        Ok(out) => out,
        Err(e) => return app.alert(e),
    };
    if let Some(cell) = out.get("cell").and_then(|c| c.as_str()) {
        app.run_or_alert("selection.set", json!({"range": cell}));
        app.grid.ensure_visible = true;
    }
}
