//! MK Matrix's top bar: File, the tabs and Help over a single-row toolbar of the active tab.

use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use mkmatrix_ui_egui::SheetApp;
use mkmatrix_ui_egui::ribbon::RibbonOverflow;

use crate::support::{app_harness, blank_session};

fn toolbar_app() -> SheetApp {
    crate::common::prepare(SheetApp::new(blank_session(), Default::default()))
}

#[test]
fn file_opens_backstage_and_help_is_a_menu() {
    let mut h = app_harness(toolbar_app(), 4);
    h.get_by_label("File").click();
    h.run_steps(3);
    assert!(h.state().backstage.is_some(), "File opens the Backstage view");
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    h.get_by_label("Help").click();
    h.run_steps(2);
    assert!(h.query_by_label("Check for Updates…").is_some(), "the Help menu lists its items");
}

#[test]
fn a_tab_shows_its_controls_in_the_toolbar_row() {
    let mut h = app_harness(toolbar_app(), 4);
    assert_eq!(h.state().ui.ribbon_tab, "Home");
    assert!(h.query_by_label("Sort A to Z").is_none(), "Data's controls aren't shown on Home");
    h.get_by_role_and_label(Role::Tab, "Data").click();
    h.run_steps(3);
    assert_eq!(h.state().ui.ribbon_tab, "Data");
    assert!(!h.state().ui.ribbon_collapsed, "a tab opens its row instead of a menu");
    assert!(h.query_by_label("Sort A to Z").is_some(), "Data's controls in the row");
    // Still one row: a button high, the groups' names under it.
    let row = RibbonOverflow::get(&h.ctx);
    assert!(row.content_height > 0.0 && row.content_height <= 50.5, "{} px of controls and group names", row.content_height);
}

#[test]
fn every_tab_fits_one_row() {
    let mut h = app_harness(toolbar_app(), 4);
    for tab in mkmatrix_ui_egui::ribbon::TABS {
        h.state_mut().ui.ribbon_tab = (*tab).into();
        h.run_steps(3);
        let row = RibbonOverflow::get(&h.ctx);
        assert!(row.content_height > 0.0 && row.content_height <= 50.5, "{tab}: {} px", row.content_height);
    }
}

#[test]
fn clicking_the_active_tab_folds_the_row_away() {
    let mut h = app_harness(toolbar_app(), 4);
    h.get_by_role_and_label(Role::Tab, "Home").click();
    h.run_steps(2);
    assert!(h.state().ui.ribbon_collapsed);
    h.get_by_role_and_label(Role::Tab, "Insert").click();
    h.run_steps(2);
    assert!(!h.state().ui.ribbon_collapsed);
    assert_eq!(h.state().ui.ribbon_tab, "Insert");
}
