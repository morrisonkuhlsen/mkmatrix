//! View › History: the timeline of the workbook. Choosing a point previews it on the grid without
//! changing anything; Go Back Here undoes to it, and the later steps stay to redo.

use egui_kittest::kittest::Queryable as _;
use mkmatrix_engine::Session;
use serde_json::json;

#[test]
fn choosing_a_point_previews_it_and_go_back_goes_there() {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("cell.set", json!({"cell": "A1", "input": "Receita"})).unwrap();
    s.execute("cell.set", json!({"cell": "B1", "input": "100"})).unwrap();
    s.execute("cell.set", json!({"cell": "B1", "input": "250"})).unwrap();
    let mut h = crate::common::harness_with(crate::common::builder().with_size(egui::vec2(1200.0, 700.0)), crate::common::app(s), 3);
    h.state_mut().run("view.history", json!({"on": true})).unwrap();
    h.run_steps(3);
    assert!(h.query_by_label("Opened").is_some(), "the timeline starts at the opened workbook");
    h.get_by_label("Opened").click();
    h.run_steps(3);
    if let Ok(path) = std::env::var("MKMATRIX_HISTORY_SCREENSHOT") {
        h.render().unwrap().save(path).unwrap();
    }
    let preview: Vec<String> = h.state().history_cells().iter().map(|(c, _)| c.a1()).collect();
    assert_eq!(preview, ["A1", "B1"], "both cells stood blank then");
    let read = |h: &mut egui_kittest::Harness<'static, mkmatrix_ui_egui::SheetApp>| {
        h.state_mut().session.execute("sheet.read", json!({"range": "A1:B1"})).unwrap()["values"].clone()
    };
    assert_eq!(read(&mut h), json!([["Receita", 250.0]]), "choosing only previews");
    h.get_by_label("Go Back Here").click();
    h.run_steps(3);
    assert_eq!(read(&mut h), json!([[null, null]]), "Go Back Here undoes to it");
    assert!(h.state().history_cells().is_empty(), "now is the chosen point: no preview");
    h.state_mut().run("edit.redo", json!({"steps": 3})).unwrap();
    h.run_steps(2);
    assert_eq!(read(&mut h), json!([["Receita", 250.0]]), "and every step can come back");
}
