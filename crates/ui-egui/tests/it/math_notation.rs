//! The Σ button at the formula bar's end shows the active cell's formula in mathematical
//! notation under the field; off, nothing is drawn.

use egui_kittest::kittest::Queryable as _;
use mkmatrix_engine::Session;
use serde_json::json;

#[test]
fn the_formula_shows_in_notation_under_the_bar() {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("range.setValues", json!({"range": "A1", "values": [[2, 1], [1, 3]]})).unwrap();
    s.execute("range.setValues", json!({"range": "D1", "values": [[1], [2]]})).unwrap();
    let formula = std::env::var("MKMATRIX_MATH_FORMULA").unwrap_or_else(|_| "=SQRT(A1^2+B1^2)/(1+MX.DET(A1:B2))+SUM(A1:B2)".into());
    s.execute("cell.set", json!({"cell": "F1", "input": formula})).unwrap();
    s.execute("selection.set", json!({"cell": "F1"})).unwrap();
    let mut h = crate::common::harness_with(crate::common::builder().with_size(egui::vec2(1200.0, 700.0)), crate::common::app(s), 3);
    assert!(h.query_by_label("Mathematical Notation").is_some(), "the toggle is at the bar's end");
    h.state_mut().run("view.mathNotation", json!({"on": true})).unwrap();
    h.run_steps(3);
    if let Ok(path) = std::env::var("MKMATRIX_MATH_SCREENSHOT") {
        h.render().unwrap().save(path).unwrap();
    }
    assert_eq!(h.query_all_by_label("Mathematical Notation").count(), 2, "the toggle and the notation card");
    h.state_mut().run("view.mathNotation", json!({"on": false})).unwrap();
    h.run_steps(3);
    assert_eq!(h.query_all_by_label("Mathematical Notation").count(), 1, "off: only the toggle");
}
