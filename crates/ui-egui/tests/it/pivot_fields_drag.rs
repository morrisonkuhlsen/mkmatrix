//! The PivotTable Fields pane: fields dragged between areas (#70), the grid previewing the report
//! while a field is over an area (#72), dragged out to remove, Esc to cancel; the search box.

use egui::{Event, Modifiers, PointerButton, Pos2, vec2};
use egui_kittest::kittest::Queryable as _;
use mkmatrix_engine::Session;
use serde_json::{Value as Json, json};

type Harness = egui_kittest::Harness<'static, mkmatrix_ui_egui::SheetApp>;

fn harness() -> Harness {
    let mut s = Session::new();
    s.new_workbook();
    s.execute(
        "range.setValues",
        json!({"range": "A1", "values": [["Region", "Product", "Sales"], ["East", "Pens", 100], ["West", "Ink", 80], ["East", "Ink", 30]]}),
    )
    .unwrap();
    s.execute("insert.pivotTable", json!({"source": "Sheet1!A1:C4", "destination": "Sheet1!F3", "rows": ["Region"], "values": ["Sales"]})).unwrap();
    s.execute("selection.set", json!({"cell": "F3"})).unwrap();
    let mut h = crate::support::harness(s, 3);
    h.set_size(vec2(1400.0, 1000.0));
    h.run_steps(3);
    h
}

fn pivot(h: &mut Harness) -> Json {
    h.state_mut().session.execute("pivot.fields", json!({})).unwrap_or_default()
}

fn names(p: &Json, area: &str) -> Vec<String> {
    p[area].as_array().map(|a| a.iter().filter_map(|x| x["name"].as_str().or(x["field"].as_str()).map(str::to_string)).collect()).unwrap_or_default()
}

/// The middle of an area's drop zone, under its name.
fn zone(h: &Harness, area: &str) -> Pos2 {
    h.get_by_label(area).rect().left_bottom() + vec2(40.0, 30.0)
}

fn press(h: &mut Harness, at: Pos2) {
    h.input_mut().events.extend([
        Event::PointerMoved(at),
        Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE },
    ]);
    h.step();
}

fn move_to(h: &mut Harness, from: Pos2, to: Pos2) {
    for i in 1..=6 {
        h.input_mut().events.push(Event::PointerMoved(from + (to - from) * (i as f32 / 6.0)));
        h.step();
    }
}

fn release(h: &mut Harness, at: Pos2) {
    h.input_mut().events.push(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.step();
    h.run_steps(3);
}

#[test]
fn a_field_dragged_to_another_area_moves_and_the_grid_previews_it_first() {
    let mut h = harness();
    let from = h.get_by_label("Region ▾").rect().center();
    let to = zone(&h, "Columns");
    let before = h.state().session.active().unwrap().revision;
    press(&mut h, from);
    move_to(&mut h, from, to);
    if let Ok(path) = std::env::var("MKMATRIX_PIVOT_SCREENSHOT") {
        h.render().unwrap().save(path).unwrap();
    }
    assert!(!h.state().preview_cells().is_empty(), "the report as the drop would leave it, before the drop");
    assert_eq!(h.state().session.active().unwrap().revision, before, "nothing changes until the drop");
    release(&mut h, to);
    let p = pivot(&mut h);
    assert_eq!(names(&p, "columns"), ["Region"], "{p}");
    assert!(names(&p, "rows").is_empty(), "{p}");
    assert!(h.state().preview_cells().is_empty(), "the preview goes with the drag");
}

#[test]
fn a_field_from_the_list_lands_where_it_is_dropped_and_esc_cancels_a_drag() {
    let mut h = harness();
    // Esc mid-drag: nothing happens.
    let from = h.get_by_label("Product").rect().center();
    let to = zone(&h, "Rows");
    press(&mut h, from);
    move_to(&mut h, from, to);
    h.key_press(egui::Key::Escape);
    release(&mut h, to);
    assert_eq!(names(&pivot(&mut h), "rows"), ["Region"], "Esc cancels");
    // Dropped above Region: first in Rows.
    let above = h.get_by_label("Region ▾").rect().center_top() + vec2(0.0, 2.0);
    press(&mut h, from);
    move_to(&mut h, from, above);
    release(&mut h, above);
    assert_eq!(names(&pivot(&mut h), "rows"), ["Product", "Region"]);
}

#[test]
fn a_field_dragged_out_of_the_pane_is_removed() {
    let mut h = harness();
    let from = h.get_by_label("Region ▾").rect().center();
    let out = egui::pos2(300.0, 500.0);
    press(&mut h, from);
    move_to(&mut h, from, out);
    release(&mut h, out);
    assert!(names(&pivot(&mut h), "rows").is_empty(), "{}", pivot(&mut h));
}

#[test]
fn the_search_box_narrows_the_field_list() {
    let mut h = harness();
    h.get_by_label("Search fields").click();
    h.run_steps(1);
    h.get_by_label("Search fields").type_text("pro");
    h.run_steps(3);
    assert!(h.query_by_label("Product").is_some());
    assert!(h.query_by_label("Sales").is_none(), "Sales doesn't match");
}
