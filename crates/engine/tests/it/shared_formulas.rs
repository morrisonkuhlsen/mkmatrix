//! Filled formulas share one tree (#93): each cell keeps only its offset from the first. They
//! must behave exactly as if each had its own tree: the same values, texts, recalculation, edits,
//! LAMBDAs, names, INDIRECT, D-function criteria and XLSX round trip.

use mkmatrix_engine::Session;
use mkmatrix_engine::core::{CellError, CellRef, Value};
use serde_json::json;

fn session() -> Session {
    let mut s = Session::new();
    s.new_workbook();
    let rows: Vec<serde_json::Value> = (1..=20).map(|i| json!([i, i * 10, i * 100])).collect();
    s.execute("range.setValues", json!({"range": "A1", "values": rows})).unwrap();
    s
}

fn cell(s: &Session, a1: &str) -> mkmatrix_engine::model::Cell {
    s.doc().unwrap().wb.active().unwrap().cell(CellRef::parse(a1).unwrap()).cloned().unwrap_or_default()
}

fn value(s: &Session, a1: &str) -> Value {
    s.doc().unwrap().wb.active().unwrap().value(CellRef::parse(a1).unwrap())
}

fn text(s: &Session, a1: &str) -> String {
    cell(s, a1).formula.map(|f| f.text.clone()).unwrap_or_default()
}

/// Fills `formula` from `first` down to `last` and checks every filled cell against the same
/// formula typed into a scratch cell (which has its own tree).
fn fill_and_compare(s: &mut Session, first: &str, last: &str, formula: &str) {
    s.execute("cell.set", json!({"cell": first, "input": formula})).unwrap();
    s.execute("edit.autoFill", json!({"source": first, "target": format!("{first}:{last}")})).unwrap();
    let (a, b) = (CellRef::parse(first).unwrap(), CellRef::parse(last).unwrap());
    for row in a.row..=b.row {
        let at = CellRef::new(row, a.col).a1();
        let f = cell(s, &at).formula.expect("a formula");
        if row > a.row {
            assert!(f.shared.is_some(), "{at} shares the first cell's tree");
        }
        // The same text typed into the cell gives the same value.
        let shared_value = value(s, &at);
        let typed = format!("={}", f.text);
        s.execute("cell.set", json!({"cell": at, "input": typed})).unwrap();
        assert!(cell(s, &at).formula.unwrap().shared.is_none());
        assert_eq!(value(s, &at), shared_value, "{at}: {typed}");
    }
}

#[test]
fn relative_absolute_and_mixed_references() {
    let mut s = session();
    fill_and_compare(&mut s, "E1", "E20", "=A1*B1+$C$1+A$2+$B3");
    assert_eq!(text(&s, "E5"), "A5*B5+$C$1+A$2+$B7");
    assert_eq!(value(&s, "E5"), Value::Number(5.0 * 50.0 + 100.0 + 2.0 + 70.0));
}

#[test]
fn ranges_functions_and_whole_columns() {
    let mut s = session();
    fill_and_compare(&mut s, "F1", "F20", "=SUM($A$1:A1)+COUNTIF(B:B,\">\"&B1)+MAX(A1:C1)");
}

#[test]
fn a_lambda_keeps_its_cells_references() {
    let mut s = session();
    fill_and_compare(&mut s, "G1", "G20", "=LAMBDA(x,x+B1)(A1)");
    assert_eq!(value(&s, "G7"), Value::Number(7.0 + 70.0));
    fill_and_compare(&mut s, "H1", "H20", "=MAP(A1:A2,LAMBDA(v,v*C1))");
}

#[test]
fn names_with_relative_references_and_indirect_are_not_moved_by_the_offset() {
    let mut s = session();
    s.execute("formulas.defineName", json!({"name": "First", "refersTo": "=Sheet1!$A$1"})).unwrap();
    fill_and_compare(&mut s, "I1", "I20", "=First+INDIRECT(\"B1\")+A1");
    assert_eq!(value(&s, "I9"), Value::Number(1.0 + 10.0 + 9.0));
    // A name with relative references moves with the cell using it, never by a shared formula's
    // offset on top of that: each filled cell agrees with the same formula typed in.
    s.execute("formulas.defineName", json!({"name": "Near", "refersTo": "=Sheet1!A1048576"})).unwrap();
    fill_and_compare(&mut s, "K2", "K20", "=Near+1");
}

#[test]
fn a_reference_that_would_leave_the_sheet_gets_ref_and_a_tree_of_its_own() {
    let mut s = session();
    // `=A1+1` filled up from J3: J2 would read the row above row 1.
    s.execute("cell.set", json!({"cell": "J3", "input": "=A1+1"})).unwrap();
    s.execute("edit.autoFill", json!({"source": "J3", "target": "J1:J3"})).unwrap();
    for at in ["J1", "J2"] {
        let f = cell(&s, at).formula.expect("a formula");
        assert!(f.shared.is_none(), "{at} has a tree of its own");
        assert!(f.text.contains("#REF!"), "{at}: {}", f.text);
        assert_eq!(value(&s, at), Value::Error(CellError::Ref));
    }
}

#[test]
fn edits_inserted_rows_undo_and_recalc_keep_shared_formulas_right() {
    let mut s = session();
    s.execute("cell.set", json!({"cell": "E1", "input": "=A1*2"})).unwrap();
    s.execute("edit.autoFill", json!({"source": "E1", "target": "E1:E20"})).unwrap();
    // A precedent changes: only its formula's value follows.
    s.execute("cell.set", json!({"cell": "A10", "input": "1000"})).unwrap();
    assert_eq!(value(&s, "E10"), Value::Number(2000.0));
    assert_eq!(value(&s, "E11"), Value::Number(22.0));
    // Rows inserted above move the references.
    s.execute("home.insertRows", json!({"rows": "5:6"})).unwrap();
    assert_eq!(text(&s, "E12"), "A12*2");
    assert_eq!(value(&s, "E12"), Value::Number(2000.0));
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(text(&s, "E10"), "A10*2");
    assert_eq!(value(&s, "E10"), Value::Number(2000.0));
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(value(&s, "E10"), Value::Number(20.0));
    // A full recalculation agrees with the incremental one.
    let before: Vec<Value> = (1..=20).map(|r| value(&s, &format!("E{r}"))).collect();
    s.execute("formulas.calculateNow", json!({"full": true})).unwrap();
    let after: Vec<Value> = (1..=20).map(|r| value(&s, &format!("E{r}"))).collect();
    assert_eq!(before, after);
}

#[test]
fn the_xlsx_round_trip_keeps_texts_and_values() {
    let mut s = session();
    s.execute("cell.set", json!({"cell": "E1", "input": "=A1+B$1*$C1"})).unwrap();
    s.execute("edit.autoFill", json!({"source": "E1", "target": "E1:E20"})).unwrap();
    let texts: Vec<String> = (1..=20).map(|r| text(&s, &format!("E{r}"))).collect();
    let values: Vec<Value> = (1..=20).map(|r| value(&s, &format!("E{r}"))).collect();
    let saved = s.execute("file.saveBytes", json!({"format": "xlsx"})).unwrap();
    let mut t = Session::new();
    t.execute("file.open", json!({"name": "shared.xlsx", "base64": saved["base64"]})).unwrap();
    assert_eq!((1..=20).map(|r| text(&t, &format!("E{r}"))).collect::<Vec<_>>(), texts);
    assert_eq!((1..=20).map(|r| value(&t, &format!("E{r}"))).collect::<Vec<_>>(), values);
    // Read back, formulas of one shape share a tree again (the file writes each one in full).
    assert!((2..=20).all(|r| cell(&t, &format!("E{r}")).formula.is_some_and(|f| f.shared.is_some())));
}

#[test]
fn a_shared_and_an_own_formula_with_the_same_text_are_equal() {
    use mkmatrix_engine::model::Formula;
    let first = std::sync::Arc::new(mkmatrix_engine::formula::parse("A1*2").unwrap());
    let shared = Formula::shared(&first, 4, 0).unwrap();
    assert_eq!(shared.text, "A5*2");
    assert_eq!(shared, Formula::new("=A5*2"));
    assert_eq!(shared.expr(), Formula::new("=A5*2").expr());
    // Leaving the sheet: no shared formula.
    assert!(Formula::shared(&first, -1, 0).is_none());
}
