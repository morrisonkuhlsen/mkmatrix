//! Every ribbon dropdown opens with the same header (Conditional Formatting's): the button's icon,
//! its name and, when the menu acts on the selection, the selection's range; then a rule.

use egui_kittest::kittest::Queryable as _;
use mkmatrix_engine::Session;
use serde_json::json;

type Harness = egui_kittest::Harness<'static, mkmatrix_ui_egui::SheetApp>;

fn harness() -> Harness {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("range.setValues", json!({"range": "B2", "values": [[3], [1], [2]]})).unwrap();
    s.execute("selection.set", json!({"range": "B2:B4"})).unwrap();
    let mut h = crate::support::harness(s, 3);
    h.set_size(egui::vec2(2200.0, 1000.0));
    h.run_steps(2);
    h
}

/// Opens `button`'s menu on `tab` and returns whether the header shows the selection's range.
fn open(h: &mut Harness, tab: &str, button: &str) -> bool {
    h.key_press(egui::Key::Escape);
    h.state_mut().ui.ribbon_tab = tab.into();
    h.run_steps(2);
    let before = h.query_all_by_label(button).count();
    h.get_by_label(button).click();
    h.run_steps(3);
    // The header names the menu (some menus also have an item of the button's name).
    let opened = h.query_all_by_label(button).count();
    assert!(opened > before, "{tab} › {button}: no header ({before} → {opened})");
    h.query_by_label("B2:B4").is_some()
}

#[test]
fn dropdowns_on_every_tab_open_with_the_same_header() {
    let mut h = harness();
    assert!(open(&mut h, "Home", "Sort & Filter"), "Sort & Filter acts on the selection");
    if let Ok(path) = std::env::var("MKMATRIX_MENU_HEADER_SCREENSHOT") {
        h.render().unwrap().save(path).unwrap();
    }
    assert!(!open(&mut h, "Home", "Find & Select"), "Find & Select doesn't");
    assert!(open(&mut h, "Data", "Data Validation"));
    assert!(!open(&mut h, "Page Layout", "Margins"));
    assert!(!open(&mut h, "View", "Freeze Panes"));
}
