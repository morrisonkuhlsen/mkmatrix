//! Spills that the incremental recalculation left different from a full one (#103, found by the
//! command-sequence fuzz): every sheet here must read the same before and after a full recalc.

use mkmatrix_engine::Session;
use mkmatrix_engine::core::{CellError, CellRef, Value};
use serde_json::json;

fn value(s: &Session, sheet: usize, a1: &str) -> Value {
    s.doc().unwrap().wb.sheet(sheet).unwrap().value(CellRef::parse(a1).unwrap())
}

/// Every value in `range` on `sheet`, before and after a full recalculation, must agree.
fn agrees_with_a_full_recalc(s: &mut Session, sheet: usize, range: &str) {
    let r = mkmatrix_engine::core::RangeRef::parse(range).unwrap();
    let read = |s: &Session| r.iter().map(|c| value(s, sheet, &c.a1())).collect::<Vec<_>>();
    let before = read(s);
    s.execute("formulas.calculateNow", json!({"full": true})).unwrap();
    assert_eq!(before, read(s), "{range} changed in a full recalc");
}

/// Two arrays wanting the same cells: the one anchored first in reading order spills, whichever
/// was entered first (seed 89: a pasted SEQUENCE above a SORT's spill).
#[test]
fn the_array_anchored_first_in_reading_order_wins_a_shared_spill_area() {
    let mut s = Session::new();
    s.new_workbook();
    let rows: Vec<serde_json::Value> = (1..=6).map(|i| json!([i, 7 - i, i * 2])).collect();
    s.execute("range.setValues", json!({"range": "A1", "values": rows})).unwrap();
    // SORT at E4 spills over E4:G9; a SEQUENCE then entered in F3 wants F3:F5.
    s.execute("cell.set", json!({"cell": "E4", "input": "=SORT(A1:C6)"})).unwrap();
    assert_eq!(value(&s, 0, "F5"), Value::Number(5.0));
    s.execute("cell.set", json!({"cell": "F3", "input": "=SEQUENCE(3,1)"})).unwrap();
    // F3 comes first (row 3): it spills; the SORT gives way.
    assert_eq!(value(&s, 0, "F4"), Value::Number(2.0));
    assert_eq!(value(&s, 0, "E4"), Value::Error(CellError::Spill));
    agrees_with_a_full_recalc(&mut s, 0, "A1:H12");
    // Freed, the SORT spills again.
    s.execute("cell.set", json!({"cell": "F3", "input": ""})).unwrap();
    assert_eq!(value(&s, 0, "E4"), Value::Number(1.0));
    agrees_with_a_full_recalc(&mut s, 0, "A1:H12");
    // An array anchored after the SORT (D5 comes after E4), whose spill would run into the
    // SORT's: it is the one #SPILL!.
    s.execute("cell.set", json!({"cell": "D5", "input": "=SEQUENCE(1,3)"})).unwrap();
    assert_eq!(value(&s, 0, "D5"), Value::Error(CellError::Spill));
    assert_eq!(value(&s, 0, "E4"), Value::Number(1.0));
    agrees_with_a_full_recalc(&mut s, 0, "A1:H12");
}

/// The fuzz's minimal sequence for seed 89, as found.
#[test]
fn a_transposed_paste_of_an_array_over_another_ones_spill() {
    let mut s = Session::new();
    s.execute("file.new", json!({"sample": "grades"})).unwrap();
    s.execute("cell.set", json!({"cell": "L15", "input": "=SEQUENCE(3,1)"})).unwrap();
    s.execute("home.insertRows", json!({"rows": "13:14"})).unwrap();
    s.execute("cell.set", json!({"cell": "E25", "input": "=SORT(B7:F12)"})).unwrap();
    s.execute("selection.set", json!({"range": "E20:F25"})).unwrap();
    s.execute("edit.copy", json!({"range": "H15:L19"})).unwrap();
    s.execute("edit.pasteSpecial", json!({"operation": "multiply", "skipBlanks": false, "transpose": true, "what": "all"})).unwrap();
    let si = s.doc().unwrap().wb.active_sheet();
    agrees_with_a_full_recalc(&mut s, si, "A1:M40");
}

/// An array formula entered over a selection clears the cells in it, and a formula cleared that
/// way takes its spill with it (seed 77: a whole-sheet selection left a stale spilled value).
#[test]
fn an_array_entered_over_a_selection_clears_the_spills_of_the_formulas_it_replaces() {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("range.setValues", json!({"range": "A1", "values": [[3], [1], [2]]})).unwrap();
    // The SORT at C4 spills over C4:C6; the array goes over B1:D5, which takes C4 but not C6.
    s.execute("cell.set", json!({"cell": "C4", "input": "=SORT(A1:A3)"})).unwrap();
    assert_eq!(value(&s, 0, "C6"), Value::Number(3.0));
    s.execute("selection.set", json!({"range": "B1:D5"})).unwrap();
    s.execute("cell.set", json!({"cell": "B1", "input": "=1", "array": true})).unwrap();
    assert_eq!(value(&s, 0, "C6"), Value::Empty, "the SORT's spill went with it");
    agrees_with_a_full_recalc(&mut s, 0, "A1:E6");
    // Over the whole sheet: every cell is cleared, not only the first million positions.
    s.execute("cell.set", json!({"cell": "Z5000", "input": "7"})).unwrap();
    s.execute("cell.set", json!({"cell": "E4", "input": "=SEQUENCE(2)"})).unwrap();
    s.execute("selection.set", json!({"range": "A1:XFD1048576"})).unwrap();
    s.execute("cell.set", json!({"cell": "A1", "input": "=2", "array": true})).unwrap();
    assert_eq!(value(&s, 0, "Z5000"), Value::Empty);
    assert_eq!(value(&s, 0, "E5"), Value::Empty);
    agrees_with_a_full_recalc(&mut s, 0, "A1:F8");
}
