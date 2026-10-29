//! A dynamic array that reads its own spill area (`J24 = TRANSPOSE(J23:L28)` spilling over
//! J24:O26) is a loop: its old spilled values must not feed the new result. A recalculation after
//! an edit gave `#CIRC!` where a full recalculation gives 0 (command-sequence fuzz, seed 4).

use mkmatrix_engine::Session;
use mkmatrix_engine::calc::Calc;
use mkmatrix_engine::core::CellRef;
use serde_json::json;

#[test]
fn an_edit_recalculates_it_as_a_full_recalculation_does() {
    let mut s = Session::new();
    s.execute("file.new", json!({"sample": "budget"})).unwrap();
    s.execute("cell.set", json!({"cell": "J24", "input": "=TRANSPOSE(J23:L28)"})).unwrap();
    s.execute("edit.copy", json!({"range": "I:K"})).unwrap();
    s.execute("edit.paste", json!({"at": "L19"})).unwrap();
    let wb = &s.doc().unwrap().wb;
    let mut fresh = (**wb).clone();
    Calc::new().recalc_all(&mut fresh);
    let (now, full) = (wb.active().unwrap(), fresh.active().unwrap());
    for r in 23..26 {
        for c in 9..15 {
            let at = CellRef::new(r, c);
            assert_eq!(now.value(at), full.value(at), "{} after the edit vs a full recalculation", at.a1());
        }
    }
}
