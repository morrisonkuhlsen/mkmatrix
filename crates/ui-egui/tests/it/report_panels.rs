//! The report dialogs: Error Checking lists the cells in error and goes to the one clicked, the
//! Action Journal filters its commands, and Workbook Statistics and Evaluate Formula show their
//! figures and steps as text, not JSON.

use egui_kittest::kittest::Queryable as _;
use mkmatrix_engine::Session;
use mkmatrix_ui_egui::SheetApp;
use serde_json::json;

type Harness = egui_kittest::Harness<'static, SheetApp>;

fn harness() -> Harness {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("cell.set", json!({"cell": "A1", "input": "=1/0"})).unwrap();
    s.execute("cell.set", json!({"cell": "C5", "input": "=NA()"})).unwrap();
    s.execute("cell.set", json!({"cell": "B2", "input": "=SUM(1,2)*3"})).unwrap();
    crate::support::harness(s, 4)
}

fn active(h: &Harness) -> String {
    h.state().session.active().unwrap().selection.active.a1()
}

#[test]
fn error_checking_lists_the_errors_and_goes_to_the_one_clicked() {
    let mut h = harness();
    h.state_mut().open_dialog("errorChecking", json!({}));
    h.run_steps(3);
    assert!(h.query_by_label("Sheet1!A1  #DIV/0!").is_some(), "the #DIV/0! cell is listed");
    h.get_by_label("Sheet1!C5  #N/A").click();
    h.run_steps(3);
    assert_eq!(active(&h), "C5", "a click goes to the cell");
    assert!(h.state().dialog.is_some(), "the dialog stays open");
}

#[test]
fn the_journal_filters_its_commands() {
    let mut h = harness();
    h.state_mut().run("selection.set", json!({"cell": "D4"})).unwrap();
    h.state_mut().open_dialog("journal", json!({}));
    h.run_steps(3);
    assert!(h.query_by_label("selection.set").is_some());
    h.state_mut().dialog.as_mut().unwrap().search = "cell.set".into();
    h.run_steps(3);
    assert!(h.query_by_label("selection.set").is_none(), "filtered out");
    assert!(h.query_all_by_label("cell.set").count() >= 3, "every cell.set stays");
}

#[test]
fn statistics_and_evaluate_formula_read_as_text() {
    let mut h = harness();
    h.state_mut().open_dialog("statistics", json!({}));
    h.run_steps(3);
    assert!(h.query_by_label_contains("{").is_none(), "no JSON on show");
    h.state_mut().dialog = None;
    h.state_mut().run("selection.set", json!({"cell": "B2"})).unwrap();
    h.state_mut().open_dialog("evaluateFormula", json!({}));
    h.run_steps(3);
    assert!(h.query_all_by_label("=SUM(1,2)*3").count() >= 2, "the formula is shown (and in the formula bar)");
    assert!(h.query_by_label_contains("Result: 9").is_some(), "and its result, without a trailing .0");
}
