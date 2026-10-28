//! The "Calculation by hand" side panel: the active cell's matrix formula worked step by step in
//! mathematical notation ([`crate::by_hand`]), drawn here, with Copy LaTeX for a paper.

use egui::{Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Sense, Stroke, pos2, vec2};

use crate::SheetApp;
use crate::by_hand::{Math, Step, derive, latex, number};
use crate::icons::Icon;
use crate::theme::{self, Tokens};

/// The notation's base size, in points.
const SIZE: f32 = 15.0;
/// Space between the pieces of a row.
const GAP: f32 = 4.0;

/// The formula the panel works: the active cell's own, or its spill anchor's.
fn active_formula(app: &SheetApp) -> Option<(String, String)> {
    let d = app.session.active()?;
    let sh = d.wb.active()?;
    let at = crate::formula_bar::active_spill_anchor(app).unwrap_or(d.selection.active);
    let f = sh.cell(at)?.formula.as_ref()?;
    Some((at.a1(), format!("={}", f.text)))
}

/// Draws the panel when it is open (View › Calculation by Hand, or Σ on a selected matrix).
pub(crate) fn show(app: &mut SheetApp, ui: &mut egui::Ui) {
    if !app.ui.by_hand {
        return;
    }
    let t = Tokens::get(ui.ctx());
    let lang = app.ui.language;
    egui::Panel::right("by_hand")
        .default_size(440.0)
        .min_size(300.0)
        .resizable(true)
        .show_separator_line(true)
        .frame(egui::Frame::NONE.fill(t.window).inner_margin(egui::Margin::symmetric(14, 10)))
        .show(ui, |ui| {
            let work = active_formula(app).map(|(cell, f)| {
                let steps = app.session.active().map(|d| derive(&f, &d.wb, d.wb.active_sheet())).unwrap_or_else(|| Err(String::new()));
                (cell, f, steps)
            });
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("Σ  {}", lang.tr("Calculation by hand"))).font(theme::ui_bold(14.0)).color(t.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if crate::widgets::icon_button(ui, Icon::Close, t.text_dim, lang.tr("Close"), vec2(22.0, 22.0)).clicked() {
                        app.ui.by_hand = false;
                    }
                    if let Some((_, _, Ok(steps))) = &work
                        && ui.button(lang.tr("Copy LaTeX")).clicked()
                    {
                        ui.ctx().copy_text(latex(steps));
                        app.toast = Some((lang.tr("LaTeX copied").to_string(), crate::now_ms()));
                    }
                });
            });
            ui.add_space(4.0);
            match work {
                None => hint(
                    ui,
                    lang.tr("Select a cell with a matrix formula: a transpose (X'), a product (A@B), a system (A\\b), MX.DET or MX.INV."),
                    &t,
                ),
                Some((cell, formula, Err(why))) => {
                    ui.label(egui::RichText::new(format!("{cell}   {formula}")).font(FontId::monospace(12.5)).color(t.text_dim));
                    hint(ui, lang.tr(&why), &t);
                }
                Some((cell, formula, Ok(steps))) => {
                    ui.label(egui::RichText::new(format!("{cell}   {formula}")).font(FontId::monospace(12.5)).color(t.accent));
                    ui.add_space(6.0);
                    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| steps_ui(ui, &steps, &t, lang));
                }
            }
        });
}

fn hint(ui: &mut egui::Ui, text: &str, t: &Tokens) {
    ui.add_space(6.0);
    ui.label(egui::RichText::new(text).font(theme::ui_font(13.0)).color(t.text_dim));
}

fn steps_ui(ui: &mut egui::Ui, steps: &[Step], t: &Tokens, lang: crate::i18n::Language) {
    for s in steps {
        if !s.caption.is_empty() {
            ui.add_space(6.0);
            ui.label(egui::RichText::new(s.text(|c| lang.tr(c).to_string())).font(theme::ui_font(12.0)).color(t.text_dim));
        }
        if s.math != Math::Row(vec![]) {
            let size = measure(ui.painter(), &s.math, SIZE);
            let (rect, _) = ui.allocate_exact_size(size + vec2(4.0, 6.0), Sense::hover());
            paint(ui.painter(), &s.math, SIZE, pos2(rect.left() + 2.0, rect.top() + 3.0), t);
        }
    }
    ui.add_space(12.0);
}

// ------------------------------------------------------------------------------- typesetting

fn font(m: &Math, size: f32) -> FontId {
    match m {
        Math::Var(_) => FontId::new(size, FontFamily::Name(theme::CELL_ITALIC.into())),
        _ => FontId::proportional(size),
    }
}

fn text_of(m: &Math) -> Option<String> {
    match m {
        Math::Var(v) | Math::Op(v) => Some(v.clone()),
        Math::Num(x) => Some(number(*x)),
        _ => None,
    }
}

/// The size `m` takes at font size `size`.
fn measure(p: &Painter, m: &Math, size: f32) -> egui::Vec2 {
    if let Some(s) = text_of(m) {
        return p.layout_no_wrap(s, font(m, size), Color32::WHITE).size();
    }
    match m {
        Math::Sub(b, i) | Math::Sup(b, i) => {
            let bs = measure(p, b, size);
            let is = measure(p, i, size * 0.68);
            vec2(bs.x + is.x + 1.0, bs.y + is.y * 0.35)
        }
        Math::Row(items) => items.iter().map(|x| measure(p, x, size)).fold(vec2(0.0, 0.0), |acc, s| vec2(acc.x + s.x + GAP, acc.y.max(s.y))),
        Math::Matrix(rows) => {
            let (widths, heights) = grid(p, rows, size);
            vec2(widths.iter().sum::<f32>() + 12.0 * widths.len() as f32 + 12.0, heights.iter().sum::<f32>() + 4.0 * heights.len() as f32 + 6.0)
        }
        _ => vec2(0.0, 0.0),
    }
}

/// A matrix's column widths and row heights.
fn grid(p: &Painter, rows: &[Vec<Math>], size: f32) -> (Vec<f32>, Vec<f32>) {
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut widths = vec![0.0f32; cols];
    let mut heights = Vec::with_capacity(rows.len());
    for r in rows {
        let mut h = 0.0f32;
        for (j, x) in r.iter().enumerate() {
            let s = measure(p, x, size * 0.9);
            if let Some(w) = widths.get_mut(j) {
                *w = w.max(s.x);
            }
            h = h.max(s.y);
        }
        heights.push(h);
    }
    (widths, heights)
}

/// Draws `m` with its top-left at `at`.
fn paint(p: &Painter, m: &Math, size: f32, at: Pos2, t: &Tokens) {
    let ink = match m {
        Math::Var(_) => t.accent,
        _ => t.text,
    };
    if let Some(s) = text_of(m) {
        p.text(at, Align2::LEFT_TOP, s, font(m, size), ink);
        return;
    }
    match m {
        Math::Sub(b, i) | Math::Sup(b, i) => {
            let bs = measure(p, b, size);
            let is = measure(p, i, size * 0.68);
            let down = matches!(m, Math::Sub(..));
            let base_at = if down { at } else { pos2(at.x, at.y + is.y * 0.35) };
            paint(p, b, size, base_at, t);
            let y = if down { base_at.y + bs.y - is.y * 0.65 } else { at.y };
            paint(p, i, size * 0.68, pos2(base_at.x + bs.x + 1.0, y), t);
        }
        Math::Row(items) => {
            let h = measure(p, m, size).y;
            let mut x = at.x;
            for item in items {
                let s = measure(p, item, size);
                paint(p, item, size, pos2(x, at.y + (h - s.y) / 2.0), t);
                x += s.x + GAP;
            }
        }
        Math::Matrix(rows) => {
            let (widths, heights) = grid(p, rows, size);
            let total = measure(p, m, size);
            let r = Rect::from_min_size(at, total);
            let stroke = Stroke::new(1.3, t.text_dim);
            for (x, dir) in [(r.left() + 2.0, 1.0), (r.right() - 2.0, -1.0)] {
                p.line_segment([pos2(x, r.top()), pos2(x, r.bottom())], stroke);
                p.line_segment([pos2(x, r.top()), pos2(x + dir * 4.0, r.top())], stroke);
                p.line_segment([pos2(x, r.bottom()), pos2(x + dir * 4.0, r.bottom())], stroke);
            }
            let mut y = r.top() + 3.0;
            for (i, line) in rows.iter().enumerate() {
                let h = heights.get(i).copied().unwrap_or(0.0);
                let mut x = r.left() + 12.0;
                for (j, x_m) in line.iter().enumerate() {
                    let w = widths.get(j).copied().unwrap_or(0.0);
                    let s = measure(p, x_m, size * 0.9);
                    // Numbers right-aligned in their column, as a matrix is set.
                    paint(p, x_m, size * 0.9, pos2(x + w - s.x, y + (h - s.y) / 2.0), t);
                    x += w + 12.0;
                }
                y += h + 4.0;
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_panel_reads_the_active_cells_formula() {
        let mut s = mkmatrix_engine::Session::new();
        s.new_workbook();
        s.execute("range.setValues", serde_json::json!({"range": "A1", "values": [[2, 1], [1, 3]]})).unwrap();
        s.execute("cell.set", serde_json::json!({"cell": "D1", "input": "=A1:B2@A1:B2"})).unwrap();
        s.execute("selection.set", serde_json::json!({"cell": "E2"})).unwrap();
        let app = crate::SheetApp::new(s, Default::default());
        // A spill child works its anchor's formula.
        assert_eq!(super::active_formula(&app), Some(("D1".into(), "=A1:B2@A1:B2".into())));
    }
}
