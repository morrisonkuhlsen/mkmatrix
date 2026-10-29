//! Alt+drag on a typed number scrubs it, its dependents following, as one undo step (Esc puts it
//! back); Data › What-If Analysis › Sensitivity ranks the inputs of a result.

use egui::{Event, Key, Modifiers, PointerButton, accesskit::Role, vec2};
use egui_kittest::kittest::Queryable as _;
use mkmatrix_engine::Session;
use mkmatrix_engine::core::CellRef;
use mkmatrix_ui_egui::grid::Geo;
use serde_json::json;

type Harness = crate::common::Harness;

fn fixture() -> Harness {
    let mut session = Session::new();
    session.new_workbook();
    session.execute("range.setValues", json!({"range": "A1", "values": [[100], [20], [12]]})).unwrap();
    session.execute("cell.set", json!({"cell": "B1", "input": "=A1*(A2-A3)"})).unwrap();
    session.execute("cell.set", json!({"cell": "C1", "input": "2026-01-01"})).unwrap();
    crate::common::harness_with(crate::common::builder().with_size(vec2(1200.0, 800.0)), crate::common::app(session), 4)
}

fn center(h: &Harness, a1: &str) -> egui::Pos2 {
    let app = h.state();
    let sh = app.session.doc().unwrap().wb.active().unwrap();
    let geo = Geo::new(sh, app.grid.rect.unwrap(), app.view().scroll);
    geo.cell_rect(sh, CellRef::parse(a1).unwrap()).center()
}

fn value(h: &Harness, a1: &str) -> f64 {
    h.state().session.doc().unwrap().wb.active().unwrap().value(CellRef::parse(a1).unwrap()).as_f64().unwrap()
}

fn button(h: &mut Harness, pos: egui::Pos2, pressed: bool, modifiers: Modifiers) {
    h.input_mut().events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers });
    h.run_steps(2);
}

/// Alt held: press on `a1`, move `dx` sideways (in two steps, past the drag threshold), and
/// leave the button down.
fn alt_drag(h: &mut Harness, a1: &str, dx: f32) -> egui::Pos2 {
    let start = center(h, a1);
    h.input_mut().events.push(Event::ModifiersChanged(Modifiers::ALT));
    h.input_mut().events.push(Event::PointerMoved(start));
    h.run_steps(2);
    assert_eq!(h.output().platform_output.cursor_icon, egui::CursorIcon::ResizeHorizontal, "Alt over a number shows the sideways cursor");
    button(h, start, true, Modifiers::ALT);
    for step in [dx / 2.0, dx] {
        h.input_mut().events.push(Event::PointerMoved(start + vec2(step, 0.0)));
        h.run_steps(2);
    }
    start + vec2(dx, 0.0)
}

#[test]
fn alt_drag_scrubs_a_number_and_its_dependents_follow() {
    let mut h = fixture();
    let undo0 = h.state().session.doc().unwrap().undo.len();
    // 30 px is ten steps of 1 (a hundredth of 100).
    let end = alt_drag(&mut h, "A1", 30.0);
    assert_eq!(value(&h, "A1"), 110.0);
    assert_eq!(value(&h, "B1"), 880.0, "the formula recalculates while dragging");
    button(&mut h, end, false, Modifiers::ALT);
    h.input_mut().events.push(Event::ModifiersChanged(Modifiers::NONE));
    h.run_steps(2);
    let d = h.state().session.doc().unwrap();
    assert_eq!(d.undo.len(), undo0 + 1, "the whole drag is one undo step");
    h.state_mut().run("edit.undo", json!({})).unwrap();
    h.run_steps(2);
    assert_eq!((value(&h, "A1"), value(&h, "B1")), (100.0, 800.0));
    // Leftwards lowers it, in steps of 0.1 for 20.
    let end = alt_drag(&mut h, "A2", -30.0);
    assert_eq!(value(&h, "A2"), 19.0);
    button(&mut h, end, false, Modifiers::ALT);
    h.input_mut().events.push(Event::ModifiersChanged(Modifiers::NONE));
    h.run_steps(2);
}

#[test]
fn escape_puts_the_number_back_and_leaves_no_undo_step() {
    let mut h = fixture();
    let undo0 = h.state().session.doc().unwrap().undo.len();
    let end = alt_drag(&mut h, "A1", 60.0);
    assert_eq!(value(&h, "A1"), 120.0);
    h.input_mut().events.push(Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::ALT });
    h.run_steps(2);
    button(&mut h, end, false, Modifiers::ALT);
    h.input_mut().events.push(Event::ModifiersChanged(Modifiers::NONE));
    h.run_steps(2);
    let d = h.state().session.doc().unwrap();
    assert_eq!((value(&h, "A1"), value(&h, "B1")), (100.0, 800.0));
    assert_eq!(d.undo.len(), undo0);
    assert!(d.redo.is_empty(), "nothing to redo either");
}

#[test]
fn a_formula_or_a_date_is_not_scrubbed() {
    let mut h = fixture();
    for a1 in ["B1", "C1"] {
        let start = center(&h, a1);
        h.input_mut().events.push(Event::ModifiersChanged(Modifiers::ALT));
        h.input_mut().events.push(Event::PointerMoved(start));
        h.run_steps(2);
        assert_ne!(h.output().platform_output.cursor_icon, egui::CursorIcon::ResizeHorizontal, "{a1}");
        button(&mut h, start, true, Modifiers::ALT);
        h.input_mut().events.push(Event::PointerMoved(start + vec2(40.0, 0.0)));
        h.run_steps(2);
        button(&mut h, start + vec2(40.0, 0.0), false, Modifiers::ALT);
        h.input_mut().events.push(Event::ModifiersChanged(Modifiers::NONE));
        h.run_steps(2);
    }
    assert_eq!(value(&h, "B1"), 800.0);
    let c1 = h.state().session.doc().unwrap().wb.active().unwrap().value(CellRef::parse("C1").unwrap());
    assert_eq!(c1.as_f64(), mkmatrix_engine::core::parse::parse_input("2026-01-01", Default::default()).value.as_f64());
}

#[test]
fn the_sensitivity_dialog_lists_the_inputs_widest_first() {
    let mut h = fixture();
    h.state_mut().run("selection.set", json!({"cell": "B1"})).unwrap();
    h.state_mut().open_dialog("sensitivity", json!({}));
    h.run_steps(3);
    // A1 swings 2×10×8 = 160, A2 2×100×2 = 400, A3 240.
    let mut rows = ["A1", "A2", "A3"];
    rows.sort_by(|a, b| {
        let y = |l: &str| h.get_by_role_and_label(Role::Button, l).rect().top();
        y(a).total_cmp(&y(b))
    });
    assert_eq!(rows, ["A2", "A3", "A1"]);
    // ±50% widens every bar; the order stays.
    h.get_by_role_and_label(Role::Button, "±50%").click();
    h.run_steps(3);
    let r = h.state_mut().session.run("data.sensitivity", json!({"cell": "B1", "change": 0.5})).unwrap();
    assert_eq!(r["inputs"][0]["swing"], 2000.0);
    // A click on a row goes to the input.
    h.get_by_role_and_label(Role::Button, "A3").click();
    h.run_steps(3);
    assert_eq!(h.state().session.doc().unwrap().selection.active, CellRef::parse("A3").unwrap());
}
