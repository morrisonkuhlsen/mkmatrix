//! Formulas › Linear Algebra lists the matrix functions: the MX. family.

use serde_json::json;

#[test]
fn linear_algebra_lists_the_matrix_functions() {
    let mut s = mkmatrix_engine::Session::new();
    s.new_workbook();
    let listed = s.execute("formulas.linearAlgebra", json!({})).unwrap();
    let names: Vec<&str> = listed.as_array().unwrap().iter().filter_map(|f| f["name"].as_str()).collect();
    for name in ["MX.MUL", "MX.INV", "MX.DET", "MX.SOLVE", "MX.EIG", "MX.T", "MX.EYE", "MX.SVD"] {
        assert!(names.contains(&name), "{name} in {names:?}");
    }
    assert!(!names.contains(&"SUM") && !names.contains(&"VLOOKUP"), "only matrix functions: {names:?}");
}
