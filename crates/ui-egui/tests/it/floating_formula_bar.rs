//! View › Floating Formula Bar: the bar leaves the top of the window, comes up just above the cell a
//! click selects, and goes away with Esc; Ctrl pressed twice brings it up and puts it away.

use egui::{Event, Modifiers, PointerButton, Pos2, pos2};
use egui_kittest::kittest::Queryable as _;
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
    h.state_mut().run("view.floatingFormulaBar", json!({"on": true})).unwrap();
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

fn click(h: &mut Harness, a1: &str) {
    let pos = cell_center(h, a1);
    for pressed in [true, false] {
        h.input_mut()
            .events
            .extend([Event::PointerMoved(pos), Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }]);
        h.step();
    }
    h.run_steps(3);
}

fn modifiers(h: &mut Harness, m: Modifiers) {
    h.input_mut().events.push(Event::ModifiersChanged(m));
    h.step();
}

fn ctrl_tap(h: &mut Harness) {
    modifiers(h, Modifiers::CTRL);
    modifiers(h, Modifiers::NONE);
}

fn bar_shown(h: &Harness) -> bool {
    h.query_by_label("Insert Function").is_some()
}

#[test]
fn a_click_brings_the_bar_up_above_the_cell_and_esc_puts_it_away() {
    let mut h = harness();
    assert!(!bar_shown(&h), "no docked bar, and the floating one waits for a click");
    click(&mut h, "D8");
    if let Ok(path) = std::env::var("MKMATRIX_FLOATING_BAR_SCREENSHOT") {
        h.render().unwrap().save(path).unwrap();
    }
    let fx = h.get_by_label("Insert Function").rect();
    let cell = cell_center(&h, "D8");
    assert!(fx.bottom() < cell.y, "the bar is above the cell: {fx:?} vs {cell:?}");
    assert!(h.query_all_by_label("=SUM(1,2)").next().is_some(), "and shows its formula");
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    assert!(!bar_shown(&h), "Esc puts it away");
}

#[test]
fn ctrl_twice_toggles_the_bar() {
    let mut h = harness();
    ctrl_tap(&mut h);
    ctrl_tap(&mut h);
    h.run_steps(2);
    assert!(bar_shown(&h), "Ctrl Ctrl brings it up");
    ctrl_tap(&mut h);
    ctrl_tap(&mut h);
    h.run_steps(2);
    assert!(!bar_shown(&h), "and puts it away");
    // Ctrl as part of a shortcut isn't a tap.
    modifiers(&mut h, Modifiers::CTRL);
    h.key_press_modifiers(Modifiers::CTRL, egui::Key::B);
    modifiers(&mut h, Modifiers::NONE);
    ctrl_tap(&mut h);
    h.run_steps(2);
    assert!(!bar_shown(&h), "Ctrl+B then one Ctrl is not Ctrl Ctrl");
}

#[test]
fn turning_it_off_brings_the_docked_bar_back() {
    let mut h = harness();
    h.state_mut().run("view.floatingFormulaBar", json!({"on": false})).unwrap();
    h.run_steps(3);
    assert!(bar_shown(&h), "the docked bar is back");
}
