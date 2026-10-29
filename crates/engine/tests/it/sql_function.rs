//! =SQL(query, table1, …): a SELECT over ranges spills with a header row, recalculates when its
//! data changes, and keeps its formula through an XLSX save (as `_xludf.SQL`).

use mkmatrix_engine::Session;
use serde_json::{Value as Json, json};

fn session() -> Session {
    let mut s = Session::new();
    s.new_workbook();
    s.execute(
        "range.setValues",
        json!({"range": "A1", "values": [["Região", "Produto", "Total"], ["Norte", "Canetas", 10], ["Sul", "Canetas", 30], ["Norte", "Tinta", 5], ["Sul", "Papel", 20]]}),
    )
    .unwrap();
    s.execute(
        "cell.set",
        json!({"cell": "E1", "input": "=SQL(\"SELECT Região, SUM(Total) AS Total, COUNT(*) AS Vendas FROM t1 GROUP BY Região ORDER BY Total DESC\", A1:C5)"}),
    )
    .unwrap();
    s
}

fn read(s: &mut Session, range: &str) -> Json {
    s.execute("sheet.read", json!({"range": range})).unwrap()["values"].clone()
}

#[test]
fn the_result_spills_and_follows_its_data() {
    let mut s = session();
    assert_eq!(read(&mut s, "E1:G3"), json!([["Região", "Total", "Vendas"], ["Sul", 50.0, 2.0], ["Norte", 15.0, 2.0]]));
    // A change to the table recalculates the query.
    s.execute("cell.set", json!({"cell": "C2", "input": "100"})).unwrap();
    assert_eq!(read(&mut s, "E2:G3"), json!([["Norte", 105.0, 2.0], ["Sul", 50.0, 2.0]]));
    // A query that doesn't parse is #VALUE!, not a crash.
    s.execute("cell.set", json!({"cell": "E1", "input": "=SQL(\"SELECT FROM\", A1:C5)"})).unwrap();
    assert_eq!(read(&mut s, "E1"), json!([[{"error": "#VALUE!"}]]));
}

#[test]
fn the_formula_survives_an_xlsx_round_trip() {
    let s = session();
    let bytes = mkmatrix_xlsx::write_xlsx(&s.doc().unwrap().wb).unwrap();
    let (wb, _) = mkmatrix_xlsx::read_xlsx(&bytes).unwrap();
    let sh = wb.sheets.first().unwrap();
    let f = sh.cell(mkmatrix_engine::core::CellRef::new(0, 4)).and_then(|c| c.formula.as_ref()).map(|f| f.text.clone()).unwrap();
    assert!(f.starts_with("SQL(\"SELECT Região"), "{f}");
}
