//! Explain Formula: the active cell's evaluation steps and the tree of what it reads, down to the
//! data; a click on a row goes there.

use egui_kittest::kittest::Queryable as _;
use mkmatrix_engine::Session;
use serde_json::json;

#[test]
fn the_tree_reaches_the_data_and_a_click_goes_there() {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("range.setValues", json!({"range": "A1", "values": [["Preço", "Qtd"], [10, 2], [20, 3], [5, 4]]})).unwrap();
    s.execute("cell.set", json!({"cell": "C2", "input": "=A2*B2"})).unwrap();
    s.execute("cell.set", json!({"cell": "C3", "input": "=A3*B3"})).unwrap();
    s.execute("cell.set", json!({"cell": "C4", "input": "=A4*B4"})).unwrap();
    s.execute("cell.set", json!({"cell": "E2", "input": "=SUM(C2:C4)*(1+E3)"})).unwrap();
    s.execute("cell.set", json!({"cell": "E3", "input": "0.1"})).unwrap();
    s.execute("selection.set", json!({"cell": "E2"})).unwrap();
    let mut h = crate::common::harness_with(crate::common::builder().with_size(egui::vec2(1200.0, 800.0)), crate::common::app(s), 3);
    h.state_mut().open_dialog("explainFormula", json!({}));
    h.run_steps(4);
    if let Ok(path) = std::env::var("MKMATRIX_EXPLAIN_SCREENSHOT") {
        h.render().unwrap().save(path).unwrap();
    }
    // The range it sums, its formula cells, and the rate.
    assert!(h.query_by_label("Sheet1!C2:C4").is_some());
    assert!(h.query_by_label("Sheet1!E3").is_some());
    h.get_by_label("Sheet1!E3").click();
    h.run_steps(3);
    assert_eq!(h.state().session.active().unwrap().selection.active.a1(), "E3", "a click goes to the cell");
    assert!(h.state().dialog.is_some(), "and the dialog stays");
}
