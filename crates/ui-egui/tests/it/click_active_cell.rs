//! A single click on the cell that is already active only selects it, as in Excel: only a
//! double-click (two clicks within the interval) or F2 edits. The interval is measured on egui's
//! clock, so the harness's quarter-second frames are the time between clicks (#90).

use egui::{Event, Modifiers, PointerButton, Pos2, pos2};
use mkmatrix_engine::Session;
use mkmatrix_ui_egui::SheetApp;
use mkmatrix_ui_egui::grid::Geo;
use serde_json::json;

type Harness = egui_kittest::Harness<'static, SheetApp>;

fn harness() -> Harness {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("cell.set", json!({"cell": "D8", "input": "=SUM(1,2)"})).unwrap();
    let mut h = crate::support::harness(s, 3);
    h.run_steps(3);
    h
}

fn cell_center(h: &Harness, a1: &str) -> Pos2 {
    let app = h.state();
    let sheet = app.session.active().unwrap().wb.active().unwrap();
    let geo = Geo::new(sheet, app.grid.rect.unwrap(), app.view().scroll);
    let c = mkmatrix_engine::core::CellRef::parse(a1).unwrap();
    pos2(geo.x(sheet, c.col) + sheet.col_width(c.col) * geo.z * 0.5, geo.y(sheet, c.row) + sheet.row_height(c.row) * geo.z * 0.5)
}

/// A press and release in one frame, followed by `frames` quiet frames (a quarter second each).
fn click(h: &mut Harness, a1: &str, frames: usize) {
    let pos = cell_center(h, a1);
    h.input_mut().events.push(Event::PointerMoved(pos));
    for pressed in [true, false] {
        h.input_mut().events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
    }
    h.step();
    h.run_steps(frames);
}

fn editing(h: &Harness) -> bool {
    h.state().editor.is_some()
}

#[test]
fn a_second_click_after_a_pause_only_selects() {
    let mut h = harness();
    click(&mut h, "D8", 3);
    assert!(!editing(&h), "the first click selects");
    click(&mut h, "D8", 3);
    assert!(!editing(&h), "a click on the active cell, three quarters of a second later, doesn't edit it");
    assert_eq!(h.state().session.active().unwrap().selection.active.a1(), "D8");
}

#[test]
fn a_double_click_edits_in_the_cell() {
    let mut h = harness();
    click(&mut h, "D8", 0);
    click(&mut h, "D8", 3);
    assert!(h.state().editor.as_ref().is_some_and(|e| !e.from_formula_bar), "two clicks a frame apart are a double-click");
}
