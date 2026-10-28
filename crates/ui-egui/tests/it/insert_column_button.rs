//! The + on a column or row heading's edge inserts a column or row there; the rest of the edge
//! still resizes and autofits.

use egui::{Event, Modifiers, PointerButton, Pos2, pos2, vec2};
use mkmatrix_engine::{Session, core::CellRef};
use mkmatrix_ui_egui::{SheetApp, grid::Geo};
use serde_json::json;

fn harness() -> egui_kittest::Harness<'static, SheetApp> {
    let mut session = Session::new();
    session.new_workbook();
    session.execute("range.setValues", json!({"range": "A1", "values": [["A", "B", "C"]]})).unwrap();
    let mut harness = crate::common::builder().with_step_dt(1.0 / 60.0).with_size(vec2(1200.0, 800.0)).build_ui_state(
        |ui, app: &mut SheetApp| {
            app.logic(ui.ctx());
            app.ui(ui);
        },
        crate::common::app(session),
    );
    harness.run_steps(4);
    harness
}

/// The point at the top of the headings on column `col`'s right edge, where its + sits.
fn edge(harness: &egui_kittest::Harness<'_, SheetApp>, col: u32) -> Pos2 {
    let app = harness.state();
    let sheet = app.session.active().unwrap().wb.active().unwrap();
    let geo = Geo::new(sheet, app.grid.rect.unwrap(), app.view().scroll);
    pos2(geo.x(sheet, col) + sheet.col_width(col) * geo.z, geo.head_top() + 4.0)
}

fn click(harness: &mut egui_kittest::Harness<'_, SheetApp>, pos: Pos2) {
    harness.input_mut().events.push(Event::PointerMoved(pos));
    harness.step();
    for pressed in [true, false] {
        harness.input_mut().events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        harness.step();
    }
    harness.run_steps(2);
}

#[test]
fn the_plus_on_a_heading_edge_inserts_a_column_there() {
    let mut h = harness();
    let at = edge(&h, 0);
    click(&mut h, at);
    let sheet = h.state().session.active().unwrap().wb.active().unwrap().clone();
    let row: Vec<String> = (0..4).map(|c| sheet.value(CellRef::new(0, c)).display()).collect();
    assert_eq!(row, ["A", "", "B", "C"], "a new, empty column after A");
}

#[test]
fn a_click_away_from_the_plus_selects_the_column_as_before() {
    let mut h = harness();
    let at = edge(&h, 0);
    click(&mut h, at + vec2(-30.0, 0.0));
    let app = h.state();
    let sheet = app.session.active().unwrap().wb.active().unwrap();
    assert_eq!(sheet.value(CellRef::new(0, 1)).display(), "B", "nothing inserted");
    assert!(app.session.active().unwrap().selection.current().is_full_cols());
}

#[test]
fn a_click_on_the_edge_below_the_plus_inserts_nothing() {
    let mut h = harness();
    let at = edge(&h, 0);
    // The edge at the headings' middle: resize and autofit live there.
    click(&mut h, at + vec2(0.0, 7.0));
    let sheet = h.state().session.active().unwrap().wb.active().unwrap();
    assert_eq!(sheet.value(CellRef::new(0, 1)).display(), "B");
}

#[test]
fn the_plus_does_nothing_while_a_cell_is_being_edited() {
    // #20: the + isn't drawn while editing, but a click there still inserted a column and left
    // the edit on a cell that had moved.
    let mut h = harness();
    let at = edge(&h, 0);
    h.state_mut().begin_edit(Some("=SUM(".into()), false);
    h.run_steps(2);
    click(&mut h, at);
    let sheet = h.state().session.active().unwrap().wb.active().unwrap().clone();
    let row: Vec<String> = (0..3).map(|c| sheet.value(CellRef::new(0, c)).display()).collect();
    assert_eq!(row, ["A", "B", "C"], "no column inserted");
}

#[test]
fn the_plus_on_a_row_heading_edge_inserts_a_row_there() {
    let mut session = Session::new();
    session.new_workbook();
    session.execute("range.setValues", json!({"range": "A1", "values": [["1"], ["2"], ["3"]]})).unwrap();
    let mut h = crate::common::builder().with_step_dt(1.0 / 60.0).with_size(vec2(1200.0, 800.0)).build_ui_state(
        |ui, app: &mut SheetApp| {
            app.logic(ui.ctx());
            app.ui(ui);
        },
        crate::common::app(session),
    );
    h.run_steps(4);
    let at = {
        let app = h.state();
        let sheet = app.session.active().unwrap().wb.active().unwrap();
        let geo = Geo::new(sheet, app.grid.rect.unwrap(), app.view().scroll);
        pos2(geo.head_left() + 4.0, geo.y(sheet, 0) + sheet.row_height(0) * geo.z)
    };
    click(&mut h, at);
    let sheet = h.state().session.active().unwrap().wb.active().unwrap().clone();
    let col: Vec<String> = (0..4).map(|r| sheet.value(CellRef::new(r, 0)).display()).collect();
    assert_eq!(col, ["1", "", "2", "3"], "a new, empty row after row 1");
}
