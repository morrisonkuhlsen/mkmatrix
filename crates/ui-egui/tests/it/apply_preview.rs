//! Hovering Sort A to Z previews the sorted column on the grid, faded, without sorting it; moving
//! away clears the preview.

use egui_kittest::kittest::Queryable as _;
use mkmatrix_engine::Session;
use serde_json::json;

#[test]
fn hovering_sort_previews_it_without_sorting() {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("range.setValues", json!({"range": "A1", "values": [["Nome", "Idade"], ["Carla", 30], ["Ana", 25], ["Bruno", 41]]})).unwrap();
    s.execute("selection.set", json!({"cell": "A2"})).unwrap();
    let mut h = crate::common::harness_with(crate::common::builder().with_size(egui::vec2(1400.0, 700.0)), crate::common::app(s), 3);
    h.state_mut().run("ui.ribbonTab", json!({"tab": "Data"})).ok();
    h.state_mut().ui.ribbon_tab = "Data".into();
    h.run_steps(3);
    h.get_by_label("Sort A to Z").hover();
    h.run_steps(3);
    if let Ok(path) = std::env::var("MKMATRIX_PREVIEW_SCREENSHOT") {
        h.render().unwrap().save(path).unwrap();
    }
    let preview: Vec<(String, String)> = h.state().preview_cells().iter().map(|(c, t)| (c.a1(), t.clone())).collect();
    assert!(preview.contains(&("A2".into(), "Ana".into())), "{preview:?}");
    let read = h.state_mut().session.execute("sheet.read", json!({"range": "A2:A4"})).unwrap();
    assert_eq!(read["values"], json!([["Carla"], ["Ana"], ["Bruno"]]), "nothing sorted yet");
    // Away from the button, the preview goes.
    h.input_mut().events.push(egui::Event::PointerMoved(egui::pos2(700.0, 500.0)));
    h.run_steps(3);
    assert!(h.state().preview_cells().is_empty());
}
