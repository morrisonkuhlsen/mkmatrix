//! Values that must read the same after an XLSX save and reload (#104, found by the
//! command-sequence fuzz, seeds 115 and 207).

use mkmatrix_engine::Session;
use mkmatrix_engine::calc::Calc;
use mkmatrix_engine::core::{CellError, CellRef, Value};
use mkmatrix_engine::model::Workbook;
use serde_json::json;

fn value(wb: &Workbook, sheet: usize, a1: &str) -> Value {
    wb.sheet(sheet).unwrap().value(CellRef::parse(a1).unwrap())
}

fn reloaded(wb: &Workbook) -> Workbook {
    let bytes = mkmatrix_engine::io::save_bytes(wb, "t.xlsx").unwrap();
    let (mut back, _) = mkmatrix_engine::io::open_bytes("t.xlsx", &bytes).unwrap();
    Calc::new().recalc_all(&mut back);
    back
}

/// `#SPILL!` and `#CALC!` are a formula's results: as constants (typed, or pasted as values)
/// they are `#VALUE!`, which is all the file format keeps of them.
#[test]
fn a_constant_is_never_spill_or_calc() {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("cell.set", json!({"cell": "A1", "input": "#SPILL!"})).unwrap();
    s.execute("range.setValues", json!({"range": "A2", "values": [["#CALC!"], ["#N/A"]]})).unwrap();
    // A formula keeps its own: B1 is blocked by B2.
    s.execute("cell.set", json!({"cell": "B1", "input": "=SEQUENCE(2)"})).unwrap();
    s.execute("cell.set", json!({"cell": "B2", "input": "x"})).unwrap();
    let wb = s.doc().unwrap().wb.clone();
    assert_eq!(value(&wb, 0, "A1"), Value::Error(CellError::Value));
    assert_eq!(value(&wb, 0, "A2"), Value::Error(CellError::Value));
    assert_eq!(value(&wb, 0, "A3"), Value::Error(CellError::NA));
    assert_eq!(value(&wb, 0, "B1"), Value::Error(CellError::Spill));
    let back = reloaded(&wb);
    for a1 in ["A1", "A2", "A3", "B1"] {
        assert_eq!(value(&wb, 0, a1), value(&back, 0, a1), "{a1} after a reload");
    }
}

/// The fuzz's minimal sequence for seed 115: an array blocked by a merged column, then a paste
/// special "add" over it, which turns its `#SPILL!` into a constant.
#[test]
fn a_blocked_array_pasted_over_as_a_value_reloads_the_same() {
    let mut s = Session::new();
    s.execute("file.new", json!({"sample": "budget"})).unwrap();
    s.execute("home.mergeCenter", json!({"range": "K:K"})).ok();
    s.execute("cell.set", json!({"cell": "I1", "input": "=E7:H12"})).ok();
    assert_eq!(value(&s.doc().unwrap().wb, s.doc().unwrap().wb.active_sheet(), "I1"), Value::Error(CellError::Spill));
    s.execute("selection.set", json!({"range": "I14:K19"})).ok();
    s.execute("edit.copy", json!({})).ok();
    s.execute("edit.pasteSpecial", json!({"operation": "add", "skipBlanks": false, "transpose": true, "what": "valuesAndNumberFormats"})).ok();
    let wb = s.doc().unwrap().wb.clone();
    let si = wb.active_sheet();
    assert_eq!(value(&wb, si, "I1"), Value::Error(CellError::Value));
    assert_eq!(value(&reloaded(&wb), si, "I1"), Value::Error(CellError::Value));
}

/// Seed 207: Merge Across over whole columns made a merged area per row, a million of them;
/// the saved sheet read back as damaged, without its merges or its table, and `Sales[Q1]` was
/// `#REF!`. Now the command refuses, and a merge of a few rows keeps everything.
#[test]
fn merge_across_whole_columns_is_refused_and_a_few_rows_reload_the_same() {
    let mut s = Session::new();
    s.execute("file.new", json!({"sample": "sales"})).unwrap();
    assert!(s.execute("home.mergeAcross", json!({"range": "E:F"})).is_err());
    assert!(s.doc().unwrap().wb.sheet(0).unwrap().merges.is_empty(), "nothing was merged");
    s.execute("home.mergeAcross", json!({"range": "E1:F30"})).unwrap();
    let wb = s.doc().unwrap().wb.clone();
    let back = reloaded(&wb);
    let (a, b) = (wb.sheet(0).unwrap(), back.sheet(0).unwrap());
    assert_eq!(a.merges.len(), 30);
    assert_eq!(b.merges.len(), 30);
    assert_eq!(b.tables.len(), a.tables.len());
    assert_eq!(value(&back, 0, "C11"), value(&wb, 0, "C11"));
    assert!(matches!(value(&back, 0, "C11"), Value::Number(_)));
}
