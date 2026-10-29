//! Data › What-If Analysis › Sensitivity: the constants a result reads, each tried at ± a
//! change, widest swing first, with the workbook left as it was.

use serde_json::json;

fn session() -> mkmatrix_engine::Session {
    let mut s = mkmatrix_engine::Session::new();
    s.new_workbook();
    s
}

#[test]
fn sensitivity_ranks_the_inputs_by_how_far_they_move_the_result() {
    let mut s = session();
    // Profit = units × (price − cost) − fixed; units also reads a growth rate.
    s.execute("range.setValues", json!({"range": "A1", "values": [[100], [20], [12], [300], [0]]})).unwrap();
    s.execute("cell.set", json!({"cell": "B1", "input": "=A1*(1+A5)"})).unwrap();
    s.execute("cell.set", json!({"cell": "C1", "input": "=B1*(A2-A3)-A4"})).unwrap();
    s.execute("cell.set", json!({"cell": "D1", "input": "2026-01-01"})).unwrap();
    s.execute("cell.set", json!({"cell": "C2", "input": "=C1+D1*0"})).unwrap();
    let revision = s.doc().unwrap().revision;
    let r = s.execute("data.sensitivity", json!({"cell": "C2"})).unwrap();
    assert_eq!(r["value"], 500.0);
    let inputs = r["inputs"].as_array().unwrap();
    let cells: Vec<&str> = inputs.iter().filter_map(|i| i["cell"].as_str()).collect();
    // price ±10% swings 2×100×2 = 400; cost 240; units 160; fixed 60; a zero growth rate at ±0.1
    // swings 2×0.1×100×8 = 160 too. The date in D1 is not an input.
    assert_eq!(cells.first(), Some(&"A2"), "{r}");
    assert_eq!(cells.get(1), Some(&"A3"), "{r}");
    assert!(!cells.contains(&"D1"), "dates are left out: {cells:?}");
    assert_eq!(cells.len(), 5, "{cells:?}");
    let swing = |a: &str| inputs.iter().find(|i| i["cell"] == a).and_then(|i| i["swing"].as_f64()).unwrap();
    assert!((swing("A2") - 400.0).abs() < 1e-9);
    assert!((swing("A4") - 60.0).abs() < 1e-9);
    assert!((swing("A5") - 160.0).abs() < 1e-9);
    let a2 = inputs.iter().find(|i| i["cell"] == "A2").unwrap();
    assert_eq!((a2["low"].as_f64(), a2["high"].as_f64()), (Some(18.0), Some(22.0)));
    assert_eq!((a2["lowResult"].as_f64(), a2["highResult"].as_f64()), (Some(300.0), Some(700.0)));
    // Nothing changed.
    assert_eq!(s.doc().unwrap().revision, revision);
    let c1 = s.execute("cell.get", json!({"cell": "C1"})).unwrap();
    assert_eq!(c1["value"], 500.0);
}

#[test]
fn sensitivity_reads_through_ranges_and_spills_and_refuses_what_it_cannot_vary() {
    let mut s = session();
    s.execute("range.setValues", json!({"range": "A1", "values": [[1], [2], [3]]})).unwrap();
    s.execute("cell.set", json!({"cell": "B1", "input": "=A1:A3*10"})).unwrap();
    s.execute("cell.set", json!({"cell": "C1", "input": "=SUM(B2:B3)"})).unwrap();
    let r = s.execute("data.sensitivity", json!({"cell": "C1", "change": 0.5})).unwrap();
    let cells: Vec<&str> = r["inputs"].as_array().unwrap().iter().filter_map(|i| i["cell"].as_str()).collect();
    assert_eq!(cells, ["A3", "A2", "A1"], "the spill's anchor is followed to its data: {r}");
    // A constant, a text result and a bad change are refused.
    assert!(s.execute("data.sensitivity", json!({"cell": "A1"})).is_err());
    s.execute("cell.set", json!({"cell": "D1", "input": "=\"x\"&A1"})).unwrap();
    assert!(s.execute("data.sensitivity", json!({"cell": "D1"})).is_err());
    assert!(s.execute("data.sensitivity", json!({"cell": "C1", "change": -1})).is_err());
    assert!(s.execute("data.sensitivity", json!({"cell": "C1", "change": "x"})).is_ok(), "an unreadable change is the default");
}

#[test]
fn an_input_that_breaks_the_formula_comes_last() {
    let mut s = session();
    s.execute("range.setValues", json!({"range": "A1", "values": [[5], [0.1]]})).unwrap();
    // A2 at +10% (0.11) takes a square root of a negative; A1 moves the result by ±10% of 5.
    s.execute("cell.set", json!({"cell": "B1", "input": "=A1+SQRT(0.105-A2)"})).unwrap();
    let r = s.execute("data.sensitivity", json!({"cell": "B1"})).unwrap();
    let inputs = r["inputs"].as_array().unwrap();
    assert_eq!(inputs.len(), 2);
    assert_eq!(inputs[0]["cell"], "A1");
    assert_eq!((inputs[1]["cell"].as_str(), inputs[1]["highResult"].as_str()), (Some("A2"), Some("#NUM!")));
    assert!(inputs[1]["swing"].is_null());
}
