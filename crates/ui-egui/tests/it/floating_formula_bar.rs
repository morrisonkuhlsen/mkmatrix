//! View › Floating Formula Bar: the bar leaves the top of the window; F2 brings it up just above
//! the active cell to edit in, the edit's end puts it away; Ctrl pressed twice shows or hides it.

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
fn f2_brings_the_bar_up_above_the_cell_and_the_edit_ending_puts_it_away() {
    let mut h = harness();
    click(&mut h, "D8");
    assert!(!bar_shown(&h), "a click only selects: the bar waits for F2");
    h.key_press(egui::Key::F2);
    h.run_steps(3);
    if let Ok(path) = std::env::var("MKMATRIX_FLOATING_BAR_SCREENSHOT") {
        h.render().unwrap().save(path).unwrap();
    }
    let fx = h.get_by_label("Insert Function").rect();
    let cell = cell_center(&h, "D8");
    assert!(fx.bottom() < cell.y, "the bar is above the cell: {fx:?} vs {cell:?}");
    assert!(h.state().editor.as_ref().is_some_and(|e| e.from_formula_bar), "F2 edits in the bar");
    // A dialog (the Formula Builder works on the same edit) isn't covered by it.
    h.state_mut().open_dialog("insertFunction", json!({}));
    h.run_steps(3);
    assert!(!bar_shown(&h), "the bar steps aside for a dialog");
    h.state_mut().dialog = None;
    h.run_steps(3);
    assert!(bar_shown(&h), "and comes back while the edit goes on");
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert!(h.state().editor.is_none());
    assert!(!bar_shown(&h), "the edit's end puts it away");
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

fn drag(h: &mut Harness, from: Pos2, to: Pos2) {
    h.input_mut().events.extend([
        Event::PointerMoved(from),
        Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE },
    ]);
    h.step();
    for i in 1..=4 {
        h.input_mut().events.push(Event::PointerMoved(from + (to - from) * (i as f32 / 4.0)));
        h.step();
    }
    h.input_mut().events.push(Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.step();
    h.run_steps(3);
}

/// Two clicks a frame apart: the harness's frames are a quarter second, and egui's double-click
/// interval is 0.3 s.
fn double_click(h: &mut Harness, pos: Pos2) {
    for _ in 0..2 {
        h.input_mut().events.push(Event::PointerMoved(pos));
        for pressed in [true, false] {
            h.input_mut().events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        }
        h.step();
    }
    h.run_steps(3);
}

fn bar_rect(h: &Harness) -> egui::Rect {
    let grip = h.get_by_role_and_label(egui::accesskit::Role::Button, "Move the formula bar").rect();
    let fx = h.get_by_label("Insert Function").rect();
    grip.union(fx)
}

#[test]
fn the_grip_moves_the_bar_and_a_double_click_sends_it_back_to_the_cell() {
    let mut h = harness();
    click(&mut h, "D8");
    h.key_press(egui::Key::F2);
    h.run_steps(3);
    let grip = h.get_by_role_and_label(egui::accesskit::Role::Button, "Move the formula bar").rect().center();
    let before = bar_rect(&h);
    drag(&mut h, grip, grip + egui::vec2(120.0, 160.0));
    let after = bar_rect(&h);
    assert!((after.left() - before.left() - 120.0).abs() < 1.5 && (after.top() - before.top() - 160.0).abs() < 1.5, "{before:?} → {after:?}");
    assert!(h.state().ui.floating_bar_at.is_some());
    assert!(h.state().editor.as_ref().is_some_and(|e| e.from_formula_bar), "moving the bar doesn't end the edit");
    // It stays where it was dropped when another cell is picked.
    h.key_press(egui::Key::Enter);
    click(&mut h, "B3");
    h.key_press(egui::Key::F2);
    h.run_steps(3);
    assert!((bar_rect(&h).left() - after.left()).abs() < 1.5, "the dropped bar doesn't follow the cell");
    let grip = h.get_by_role_and_label(egui::accesskit::Role::Button, "Move the formula bar").rect().center();
    double_click(&mut h, grip);
    assert!(h.state().ui.floating_bar_at.is_none(), "a double-click on the grip sends it back");
    let fx = h.get_by_label("Insert Function").rect();
    assert!(fx.bottom() < cell_center(&h, "B3").y || fx.top() > cell_center(&h, "B3").y);
}

#[test]
fn the_right_and_bottom_edges_resize_the_bar_and_a_double_click_puts_the_size_back() {
    let mut h = harness();
    ctrl_tap(&mut h);
    ctrl_tap(&mut h);
    h.run_steps(3);
    let edges: Vec<egui::Rect> = h.get_all_by_label("Resize the formula bar").map(|n| n.rect()).collect();
    assert_eq!(edges.len(), 3, "the right edge, the bottom edge and their corner");
    let right = edges.iter().copied().max_by(|a, b| a.height().total_cmp(&b.height())).unwrap();
    let w0 = h.state().ui.floating_bar_width;
    assert!(w0.is_none());
    drag(&mut h, right.center(), right.center() - egui::vec2(160.0, 0.0));
    let w = h.state().ui.floating_bar_width.unwrap();
    assert!((w - 400.0).abs() < 2.0, "560 − 160 = {w}");
    let bottom = h.get_all_by_label("Resize the formula bar").map(|n| n.rect()).max_by(|a, b| a.width().total_cmp(&b.width())).unwrap();
    let (h0, top0) = (bar_rect(&h).height(), bar_rect(&h).top());
    drag(&mut h, bottom.center(), bottom.center() + egui::vec2(0.0, 60.0));
    let lines = h.state().ui.floating_bar_lines.unwrap();
    assert!(lines >= 3, "60 px down adds rows: {lines}");
    let grown = bar_rect(&h);
    assert!(grown.height() > h0 + 30.0, "and the bar grows");
    assert!((grown.top() - top0).abs() < 1.5, "down from where it was, under the pointer: {top0} → {}", grown.top());
    let right = h.get_all_by_label("Resize the formula bar").map(|n| n.rect()).max_by(|a, b| a.height().total_cmp(&b.height())).unwrap();
    double_click(&mut h, right.center());
    assert_eq!((h.state().ui.floating_bar_width, h.state().ui.floating_bar_lines), (None, None));
}

#[test]
fn the_bottom_edge_stretches_the_bar_past_five_rows_down_to_the_grid() {
    let mut h = harness();
    ctrl_tap(&mut h);
    ctrl_tap(&mut h);
    h.run_steps(3);
    let bottom = h.get_all_by_label("Resize the formula bar").map(|n| n.rect()).max_by(|a, b| a.width().total_cmp(&b.width())).unwrap();
    drag(&mut h, bottom.center(), bottom.center() + egui::vec2(0.0, 400.0));
    let lines = h.state().ui.floating_bar_lines.unwrap();
    assert!(lines > 15, "400 px down gives many more than five rows: {lines}");
    let grid = h.state().grid.rect.unwrap();
    assert!(bar_rect(&h).bottom() <= grid.bottom(), "and stays inside the grid");
    // Dragged far past the grid, it stops at the grid's bottom.
    let bottom = h.get_all_by_label("Resize the formula bar").map(|n| n.rect()).max_by(|a, b| a.width().total_cmp(&b.width())).unwrap();
    drag(&mut h, bottom.center(), bottom.center() + egui::vec2(0.0, 3000.0));
    assert!(bar_rect(&h).bottom() <= grid.bottom(), "{:?} inside {grid:?}", bar_rect(&h));
}

#[test]
fn the_bar_follows_a_long_formula_wider_and_taller() {
    let mut h = harness();
    let long = format!("=SUM({})", (1..=60).map(|i| i.to_string()).collect::<Vec<_>>().join(","));
    h.state_mut().session.execute("cell.set", json!({"cell": "D8", "input": long})).unwrap();
    click(&mut h, "D8");
    h.key_press(egui::Key::F2);
    h.run_steps(3);
    let wide = bar_rect(&h);
    let field = mkmatrix_ui_egui::formula_bar::BarLayout::get(&h.ctx).field;
    assert!(field.right() - wide.left() > 600.0, "a long line widens the bar past 560 px: {field:?}");
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    // A formula long enough to wrap many times takes more than five rows, without touching the edges.
    let huge = format!("={}", (1000..1600).map(|i| i.to_string()).collect::<Vec<_>>().join("+"));
    h.state_mut().session.execute("cell.set", json!({"cell": "D8", "input": huge})).unwrap();
    h.key_press(egui::Key::F2);
    h.run_steps(3);
    let layout = mkmatrix_ui_egui::formula_bar::BarLayout::get(&h.ctx);
    assert!(layout.visible_lines > 10, "the bar grows with the formula: {} rows shown", layout.visible_lines);
}

#[test]
fn the_chevron_shows_the_whole_formula_and_the_bar_stays_off_the_headings() {
    let mut h = harness();
    // An empty cell near the top: expanded, the bar takes five rows, not the whole grid.
    click(&mut h, "B2");
    h.key_press(egui::Key::F2);
    h.run_steps(3);
    h.get_by_label("Expand formula bar").click();
    h.run_steps(3);
    let layout = mkmatrix_ui_egui::formula_bar::BarLayout::get(&h.ctx);
    assert_eq!(layout.visible_lines, 5, "an empty formula expands to five rows");
    let cells = {
        let app = h.state();
        let sheet = app.session.active().unwrap().wb.active().unwrap();
        Geo::new(sheet, app.grid.rect.unwrap(), app.view().scroll).cells
    };
    let bar = bar_rect(&h);
    assert!(bar.top() >= cells.top() && bar.left() >= cells.left(), "over the cells, not the headings: {bar:?} in {cells:?}");
    // The controls sit on the first line.
    let fx = h.get_by_label("Insert Function").rect();
    assert!(fx.bottom() < layout.field.top() + 40.0, "fx on the first line: {fx:?}, field {:?}", layout.field);
}

#[test]
fn a_size_made_for_one_cell_doesnt_carry_over_and_the_chevron_takes_it_back() {
    let mut h = harness();
    click(&mut h, "B2");
    h.key_press(egui::Key::F2);
    h.run_steps(3);
    let small = bar_rect(&h);
    let bottom = h.get_all_by_label("Resize the formula bar").map(|n| n.rect()).max_by(|a, b| a.width().total_cmp(&b.width())).unwrap();
    drag(&mut h, bottom.center(), bottom.center() + egui::vec2(0.0, 400.0));
    assert!(bar_rect(&h).height() > small.height() + 200.0, "dragged tall");
    // The chevron, on a bar dragged bigger than its formula, brings it back.
    h.get_by_label("Expand formula bar").click();
    h.run_steps(3);
    assert_eq!(h.state().ui.floating_bar_lines, None);
    assert!((bar_rect(&h).height() - small.height()).abs() < 1.5, "back to the formula's size: {:?}", bar_rect(&h));
    // Dragged tall again, then the edit ends: F2 on another empty cell opens it at its own size.
    let bottom = h.get_all_by_label("Resize the formula bar").map(|n| n.rect()).max_by(|a, b| a.width().total_cmp(&b.width())).unwrap();
    drag(&mut h, bottom.center(), bottom.center() + egui::vec2(0.0, 400.0));
    assert!(h.state().ui.floating_bar_lines.is_some());
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert!(!bar_shown(&h));
    h.key_press(egui::Key::ArrowRight);
    h.run_steps(2);
    h.key_press(egui::Key::F2);
    h.run_steps(3);
    assert_eq!((h.state().ui.floating_bar_lines, h.state().ui.floating_bar_width, h.state().floating_bar_expanded), (None, None, false));
    assert!((bar_rect(&h).height() - small.height()).abs() < 1.5, "a fresh bar fits its empty cell: {:?}", bar_rect(&h));
}
