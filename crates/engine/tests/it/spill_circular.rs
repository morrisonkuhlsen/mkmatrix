//! A formula that reads a cell of another formula's spill area depends on that formula: when the
//! anchor reads it back, the two are a circular reference (#114, command-sequence fuzz seed 58).
//! Without the edge the loop went unnoticed and the result depended on evaluation order, so an
//! edit and a full recalculation disagreed.

use mkmatrix_engine::Session;
use mkmatrix_engine::core::{CellError, CellRef, RangeRef, Value};
use serde_json::json;

fn values(s: &Session, range: &str) -> Vec<Value> {
    let sh = s.doc().unwrap().wb.active().unwrap();
    RangeRef::parse(range).unwrap().iter().map(|c| sh.value(c)).collect()
}

fn circular(s: &Session) -> Vec<String> {
    let mut v: Vec<String> = s.doc().unwrap().calc.circular.iter().map(|k| k.1.a1()).collect();
    v.sort();
    v
}

/// The fuzz's minimal sequence: `G5` reads `K17`, in the spill of `I17`, which reads `G5`.
#[test]
fn reading_a_spill_cell_whose_anchor_reads_it_back_is_circular() {
    let mut s = Session::new();
    s.execute("file.new", json!({"sample": "budget"})).unwrap();
    s.execute("cell.set", json!({"cell": "G5", "input": "=Data[@Column3]"})).ok();
    s.execute("cell.set", json!({"cell": "F5", "input": "=FILTER(J1:J20,J1:J20>4)"})).ok();
    s.execute(
        "range.setValues",
        json!({"range": "G16", "values": [["'007", true], ["=TRANSPOSE(B11:C18)", "=SORT(E4:G5)"], [110, ""], [false, 95]]}),
    )
    .ok();
    s.execute("home.insertColumns", json!({"cols": "E:E"})).ok();
    s.execute("cell.set", json!({"cell": "A4", "input": "=TRANSPOSE(I14:L18)"})).ok();
    let before = values(&s, "A1:P30");
    let loops = circular(&s);
    assert!(!loops.is_empty(), "the loop through I17's spill is listed");
    s.execute("formulas.calculateNow", json!({"full": true})).unwrap();
    let after = values(&s, "A1:P30");
    for (c, (a, b)) in RangeRef::parse("A1:P30").unwrap().iter().zip(before.iter().zip(&after)) {
        assert_eq!(a, b, "{} after the edit vs a full recalculation", c.a1());
    }
    assert!(!circular(&s).is_empty(), "a full recalculation lists the loop too");
}

/// The smallest loop: `C1` reads `A2`, a cell of `A1`'s spill, and `A1` reads `C1`.
#[test]
fn a_two_cell_loop_through_a_spill() {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("cell.set", json!({"cell": "A1", "input": "=SEQUENCE(3)"})).unwrap();
    s.execute("cell.set", json!({"cell": "C1", "input": "=A2*10"})).unwrap();
    assert_eq!(values(&s, "C1"), vec![Value::Number(20.0)]);
    s.execute("cell.set", json!({"cell": "A1", "input": "=SEQUENCE(3)+C1"})).unwrap();
    assert!(!circular(&s).is_empty(), "A1 → C1 → A2 (A1's spill) is a loop");
    let sh = s.doc().unwrap().wb.active().unwrap();
    let circ = |a1: &str| sh.value(CellRef::parse(a1).unwrap()) == Value::Error(CellError::Circ);
    assert!(circ("A1") || circ("C1"), "the loop shows #CIRC!: A1 {:?}, C1 {:?}", values(&s, "A1"), values(&s, "C1"));
    let before = values(&s, "A1:C4");
    s.execute("formulas.calculateNow", json!({"full": true})).unwrap();
    assert_eq!(before, values(&s, "A1:C4"));
    // Breaking the loop clears it.
    s.execute("cell.set", json!({"cell": "A1", "input": "=SEQUENCE(3)"})).unwrap();
    assert!(circular(&s).is_empty(), "{:?}", circular(&s));
    assert_eq!(values(&s, "C1"), vec![Value::Number(20.0)]);
}
