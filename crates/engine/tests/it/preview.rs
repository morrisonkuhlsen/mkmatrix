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
