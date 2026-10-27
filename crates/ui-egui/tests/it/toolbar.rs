//! MK Matrix's default top bar: a menu bar over a single-row toolbar.

use egui_kittest::kittest::Queryable;
use mkmatrix_ui_egui::SheetApp;
use mkmatrix_ui_egui::ribbon::RibbonOverflow;

use crate::support::{app_harness, blank_session};

fn toolbar_app() -> SheetApp {
    let mut app = crate::common::prepare(SheetApp::new(blank_session(), Default::default()));
    app.ui.classic_ribbon = false;
    app
}

#[test]
fn the_menu_bar_lists_the_menus_and_runs_an_item() {
    let mut h = app_harness(toolbar_app(), 4);
    for menu in ["File", "Edit", "View", "Insert", "Format", "Data", "Tools", "Help"] {
        assert!(h.query_all_by_label(menu).next().is_some(), "{menu} menu");
    }
    assert!(h.state().ui.numbered_columns);
    h.get_by_label("View").click();
    h.run_steps(2);
    h.get_by_label("Numbered Columns").click();
    h.run_steps(2);
    assert!(!h.state().ui.numbered_columns, "View › Numbered Columns ran");
}

#[test]
fn the_toolbar_is_one_row_of_the_home_controls() {
    let mut h = app_harness(toolbar_app(), 4);
    // One row: a button high, the groups' names under it.
    let row = RibbonOverflow::get(&h.ctx);
    assert!(row.content_height > 0.0 && row.content_height <= 50.5, "{} px of controls and group names", row.content_height);
    // View › Classic Ribbon brings the stacked groups back.
    h.state_mut().run("view.classicRibbon", serde_json::json!({"on": true})).unwrap();
    h.run_steps(3);
    assert!(RibbonOverflow::get(&h.ctx).content_height > 60.0, "the classic ribbon's groups");
}
