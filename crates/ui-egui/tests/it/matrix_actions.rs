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
fn no_actions_for_a_block_that_isnt_numbers() {
    let mut s = block();
    s.execute("cell.set", json!({"cell": "B2", "input": "x"})).unwrap();
    let h = harness(s, 4);
    assert!(h.query_by_label("Transpose").is_none());
}
