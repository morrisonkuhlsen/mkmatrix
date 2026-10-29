//! Session::preview: what a command would change on the active sheet, computed on a copy; the
//! workbook, undo and journal stay as they were.

use mkmatrix_engine::Session;
use serde_json::json;

#[test]
fn a_sort_previews_without_sorting() {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("range.setValues", json!({"range": "A1", "values": [["Nome"], ["Carla"], ["Ana"], ["Bruno"]]})).unwrap();
    s.execute("selection.set", json!({"cell": "A2"})).unwrap();
    let journal = s.journal.len();
    let revision = s.doc().unwrap().revision;
    let changed = s.preview("data.sortAscending", json!({})).unwrap();
    assert!(!changed.is_empty());
    // Nothing happened to the real workbook.
    assert_eq!(s.execute("sheet.read", json!({"range": "A1:A4"})).unwrap()["values"], json!([["Nome"], ["Carla"], ["Ana"], ["Bruno"]]));
    assert_eq!((s.journal.len(), s.doc().unwrap().revision), (journal, revision), "no journal entry, no new revision");
    // And the preview is what the sort then does.
    s.execute("data.sortAscending", json!({})).unwrap();
    for (c, text) in &changed {
        let v = s.execute("cell.get", json!({"cell": c.a1()})).unwrap();
        assert_eq!(v["text"].as_str(), Some(text.as_str()), "{}", c.a1());
    }
    // A command that fails previews nothing.
    assert!(s.preview("no.such.command", json!({})).is_none());
}

#[test]
fn the_history_lists_the_steps_and_diffs_any_point_without_running_anything() {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("cell.set", json!({"cell": "A1", "input": "1"})).unwrap();
    s.execute("cell.set", json!({"cell": "A2", "input": "2"})).unwrap();
    s.execute("cell.set", json!({"cell": "A1", "input": "10"})).unwrap();
    let h = s.execute("edit.history", json!({})).unwrap();
    assert_eq!(h["steps"].as_array().unwrap().len(), 3);
    assert_eq!(h["steps"][1]["at"], "A2", "each step says where it happened");
    assert_eq!(h["position"], 3);
    // At the start, A1 and A2 were blank.
    let diff = s.execute("edit.historyDiff", json!({"position": 0})).unwrap();
    assert_eq!(diff["cells"], json!([{"cell": "A1", "text": ""}, {"cell": "A2", "text": ""}]));
    // After the second step, A1 was 1.
    assert_eq!(s.execute("edit.historyDiff", json!({"position": 2})).unwrap()["cells"], json!([{"cell": "A1", "text": "1"}]));
    // Going back two steps, the redo side keeps the timeline whole.
    s.execute("edit.undo", json!({"steps": 2})).unwrap();
    let h = s.execute("edit.history", json!({})).unwrap();
    assert_eq!((h["steps"].as_array().unwrap().len(), h["position"].as_u64()), (3, Some(1)));
    assert_eq!(
        s.execute("edit.historyDiff", json!({"position": 3})).unwrap()["cells"],
        json!([{"cell": "A1", "text": "10"}, {"cell": "A2", "text": "2"}])
    );
    assert!(s.execute("edit.historyDiff", json!({"position": 9})).is_err(), "past the end is an error");
}

#[test]
fn spilled_cells_are_in_the_history_difference() {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("cell.set", json!({"cell": "A1", "input": "=SEQUENCE(3)"})).unwrap();
    // At the start A2 and A3 (spilled, not stored) were blank too.
    let cells = s.execute("edit.historyDiff", json!({"position": 0})).unwrap()["cells"].clone();
    assert_eq!(cells, json!([{"cell": "A1", "text": ""}, {"cell": "A2", "text": ""}, {"cell": "A3", "text": ""}]));
}
