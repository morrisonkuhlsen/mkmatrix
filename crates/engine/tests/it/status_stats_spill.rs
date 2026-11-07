//! The status bar's statistics count a spill's values over formatted cells. A formatted cell in a
//! spill area is stored blank; it used to be counted (as blank) before the value spilled over it,
//! so a styled 3×3 result showed `Count: 1`.

use mkmatrix_engine::Session;
use mkmatrix_engine::core::RangeRef;
use serde_json::json;

#[test]
fn a_spill_over_formatted_cells_counts_every_value() {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("cell.set", json!({"cell": "B2", "input": "=SEQUENCE(3,3)"})).unwrap();
    s.execute("home.borders", json!({"range": "B2:D4", "preset": "all"})).unwrap();
    s.execute("home.fillColor", json!({"range": "B2:D4", "color": "#DCF4EE"})).unwrap();
    let wb = &s.doc().unwrap().wb;
    let st = mkmatrix_engine::display::stats(wb.active().unwrap(), &[RangeRef::parse("B2:D4").unwrap()]);
    assert_eq!((st.count, st.numeric, st.sum), (9, 9, 45.0));
    assert_eq!((st.min, st.max), (1.0, 9.0));
}

/// The regression sample's X'X block, styled and spilled, as the status bar sees it.
#[test]
fn the_regression_samples_cross_product_block() {
    let wb = mkmatrix_engine::sample::build("regression").unwrap();
    let st = mkmatrix_engine::display::stats(wb.sheet(0).unwrap(), &[RangeRef::parse("J6:L8").unwrap()]);
    assert_eq!(st.count, 9);
}
