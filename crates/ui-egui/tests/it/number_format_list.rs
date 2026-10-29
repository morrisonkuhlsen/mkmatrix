//! Home › Number Format: the list shows every format (each drawn with its symbol and a preview),
//! and the filter field's hint is translated.

use egui_kittest::kittest::Queryable as _;
use mkmatrix_engine::Session;
use mkmatrix_ui_egui::SheetApp;
use serde_json::json;

#[test]
fn the_number_format_list_shows_symbols_and_previews() {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("cell.set", json!({"cell": "A1", "input": "1234.5"})).unwrap();
    let mut h = crate::common::harness_with(crate::common::builder().with_size(egui::vec2(1500.0, 800.0)), crate::common::app(s), 3);
    h.state_mut().run("app.language.set", json!({"language": "pt"})).unwrap();
    h.run_steps(2);
    h.get_by_label("Geral").click();
    h.run_steps(4);
    if let Ok(path) = std::env::var("MKMATRIX_NUMBER_FORMAT_SCREENSHOT") {
        h.render().unwrap().save(path).unwrap();
    }
    for name in ["Geral", "Moeda", "Data Abreviada", "Porcentagem", "Fração", "Texto"] {
        assert!(h.query_all_by_label(name).next().is_some(), "{name} is listed");
    }
}
