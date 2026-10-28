//! The floating actions on a selected matrix.

use egui_kittest::kittest::Queryable;
use mkmatrix_engine::Session;
use serde_json::json;

use crate::support::harness;

fn block() -> Session {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("range.setValues", json!({"range": "B2", "values": [[2, 1], [1, 2]]})).unwrap();
    s.execute("selection.set", json!({"range": "B2:C3"})).unwrap();
    s
}

fn formula(h: &crate::common::Harness, a1: &str) -> Option<String> {
    let d = h.state().session.active()?;
    let c = mkmatrix_engine::core::CellRef::parse(a1)?;
    d.wb.active()?.cell(c)?.formula.as_ref().map(|f| f.text.clone())
}

#[test]
fn transpose_writes_the_formula_past_the_block() {
    let mut h = harness(block(), 4);
    h.get_by_label("Transpose").click();
    h.run_steps(3);
    assert_eq!(formula(&h, "E2").as_deref(), Some("B2:C3'"));
    assert_eq!(h.state().session.active().unwrap().selection.active.a1(), "E2", "the result is selected");
}

#[test]
fn multiply_starts_a_formula_to_point_the_second_matrix() {
    let mut h = harness(block(), 4);
    h.get_by_label("Multiply by… (point at the second matrix)").click();
    h.run_steps(3);
    assert_eq!(h.state().editor.as_ref().map(|e| e.text.as_str()), Some("=B2:C3@"));
}

#[test]
fn the_actions_sit_beside_the_block_not_over_the_rows_above() {
    let h = harness(block(), 4);
    let app = h.state();
    let sheet = app.session.active().unwrap().wb.active().unwrap();
    let sel = mkmatrix_ui_egui::grid::Geo::new(sheet, app.grid.rect.unwrap(), app.view().scroll)
        .range_rect(sheet, mkmatrix_engine::core::RangeRef::parse("B2:C3").unwrap());
    for label in ["Transpose", "Calculation by hand"] {
        let bar = h.get_by_label(label).rect();
        assert!(bar.left() >= sel.right(), "{label} is right of the block: {bar:?} vs {sel:?}");
        assert!(bar.top() >= sel.top() - 1.0, "{label} doesn't rise over the row above: {bar:?} vs {sel:?}");
    }
}

#[test]
fn no_actions_for_a_block_that_isnt_numbers() {
    let mut s = block();
    s.execute("cell.set", json!({"cell": "B2", "input": "x"})).unwrap();
    let h = harness(s, 4);
    assert!(h.query_by_label("Transpose").is_none());
}

mod brackets {
    use egui::{Event, Modifiers, PointerButton, Pos2, pos2, vec2};
    use mkmatrix_engine::Session;
    use mkmatrix_ui_egui::grid::Geo;
    use serde_json::json;

    /// A 2×2 matrix named M in C3:D4.
    fn named() -> Session {
        let mut s = Session::new();
        s.new_workbook();
        s.execute("range.setValues", json!({"range": "C3", "values": [[1, 2], [3, 4]]})).unwrap();
        s.execute("formulas.defineName", json!({"name": "M", "refersTo": "=Sheet1!$C$3:$D$4"})).unwrap();
        s.execute("selection.set", json!({"range": "A1"})).unwrap();
        s
    }

    /// The left bracket's upright, halfway down the matrix.
    fn left_bracket(h: &crate::common::Harness) -> Pos2 {
        let app = h.state();
        let sheet = app.session.active().unwrap().wb.active().unwrap();
        let geo = Geo::new(sheet, app.grid.rect.unwrap(), app.view().scroll);
        let r = geo.range_rect(sheet, mkmatrix_engine::core::RangeRef::parse("C3:D4").unwrap());
        pos2(r.left() - 3.0, r.center().y)
    }

    fn press(h: &mut crate::common::Harness, pos: Pos2, pressed: bool) {
        h.input_mut().events.push(Event::PointerMoved(pos));
        h.input_mut().events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        h.step();
    }

    #[test]
    fn a_click_on_a_bracket_selects_the_matrix() {
        let mut h = crate::support::harness(named(), 4);
        let at = left_bracket(&h);
        press(&mut h, at, true);
        press(&mut h, at, false);
        h.run_steps(2);
        assert_eq!(h.state().session.active().unwrap().selection.current().a1(), "C3:D4");
    }

    #[test]
    fn dragging_a_bracket_moves_the_matrix_and_its_name() {
        let mut h = crate::support::harness(named(), 4);
        let at = left_bracket(&h);
        let row = {
            let app = h.state();
            let sheet = app.session.active().unwrap().wb.active().unwrap();
            sheet.row_height(0) * Geo::new(sheet, app.grid.rect.unwrap(), app.view().scroll).z
        };
        press(&mut h, at, true);
        for k in 1..=6 {
            h.input_mut().events.push(Event::PointerMoved(at + vec2(0.0, row * 3.0 * k as f32 / 6.0)));
            h.step();
        }
        press(&mut h, at + vec2(0.0, row * 3.0), false);
        h.run_steps(3);
        let d = h.state().session.active().unwrap();
        let sheet = d.wb.active().unwrap();
        assert_eq!(sheet.value(mkmatrix_engine::core::CellRef::parse("C6").unwrap()).display(), "1", "the block moved three rows down");
        let m = d.wb.names.iter().find(|n| n.name == "M").unwrap();
        assert_eq!(m.formula.replace('$', ""), "Sheet1!C6:D7", "the name follows the block");
    }
}
