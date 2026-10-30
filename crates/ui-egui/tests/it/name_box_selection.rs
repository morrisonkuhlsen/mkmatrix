//! The Name Box shows the size while a range is dragged out (`4R x 3C`), then the selected block
//! (`C3:E6`) once the button is released; one cell stays one cell.

use egui::{Event, Modifiers, PointerButton};
use mkmatrix_engine::Session;
use mkmatrix_engine::core::CellRef;
use mkmatrix_ui_egui::SheetApp;
use mkmatrix_ui_egui::grid::Geo;
use serde_json::json;

type Harness = egui_kittest::Harness<'static, SheetApp>;

fn center(h: &Harness, a1: &str) -> egui::Pos2 {
    let app = h.state();
    let sh = app.session.active().unwrap().wb.active().unwrap();
    Geo::new(sh, app.grid.rect.unwrap(), app.view().scroll).cell_rect(sh, CellRef::parse(a1).unwrap()).center()
}

/// Presses on `from`, drags to `to`; the Name Box text while dragging and after the release.
fn drag(h: &mut Harness, from: &str, to: &str) -> (String, String) {
    let (a, b) = (center(h, from), center(h, to));
    h.input_mut()
        .events
        .extend([Event::PointerMoved(a), Event::PointerButton { pos: a, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }]);
    h.step();
    for i in 1..=6 {
        h.input_mut().events.push(Event::PointerMoved(a + (b - a) * (i as f32 / 6.0)));
        h.step();
    }
    let during = h.state().name_box_text();
    h.input_mut().events.push(Event::PointerButton { pos: b, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(3);
    (during, h.state().name_box_text())
}

fn harness(numbered: bool) -> Harness {
    let mut s = Session::new();
    s.new_workbook();
    let mut h = crate::support::stepped_harness(s);
    h.state_mut().ui.numbered_columns = numbered;
    h.run_steps(2);
    h
}

#[test]
fn a_dragged_block_shows_its_range_once_released() {
    let mut h = harness(false);
    assert_eq!(drag(&mut h, "C3", "E6"), ("4R x 3C".to_string(), "C3:E6".to_string()));
    // One cell is one cell.
    assert_eq!(drag(&mut h, "B2", "B2").1, "B2");
}

#[test]
fn with_numbered_columns_the_block_reads_as_row_column_pairs() {
    let mut h = harness(true);
    assert_eq!(drag(&mut h, "C3", "E6").1, "(3, 3):(6, 5)");
}

#[test]
fn a_merged_cell_reads_as_its_first_cell() {
    let mut h = harness(false);
    h.state_mut().run("home.mergeCells", json!({"range": "B2:C3"})).unwrap();
    h.state_mut().run("selection.set", json!({"range": "B2:C3"})).unwrap();
    h.run_steps(2);
    assert_eq!(h.state().name_box_text(), "B2");
}
