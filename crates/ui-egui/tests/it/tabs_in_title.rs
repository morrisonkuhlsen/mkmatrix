//! Compact density moves the ribbon's tabs up into the title bar when they fit there, so the sheet
//! starts higher; in a narrow window, or in normal density, they stay in the ribbon.

use egui::accesskit::Role;
use egui_kittest::kittest::Queryable as _;
use mkmatrix_engine::Session;
use mkmatrix_ui_egui::Density;

fn harness(width: f32, density: Density) -> egui_kittest::Harness<'static, mkmatrix_ui_egui::SheetApp> {
    let mut s = Session::new();
    s.new_workbook();
    let mut app = crate::common::app(s);
    app.ui.density = density;
    crate::common::harness_with(crate::common::builder().with_size(egui::vec2(width, 800.0)), app, 3)
}

/// The tabs' row and the title bar's controls share a band: the Home tab sits level with Undo.
fn tabs_share_the_title_bar(h: &egui_kittest::Harness<'static, mkmatrix_ui_egui::SheetApp>) -> bool {
    let tab = h.get_by_role_and_label(Role::Tab, "Home").rect();
    let undo = h.get_by_label("Undo").rect();
    (tab.center().y - undo.center().y).abs() < 4.0
}

#[test]
fn compact_density_puts_the_tabs_in_the_title_bar() {
    let mut h = harness(1440.0, Density::Compact);
    assert!(h.state().ui.tabs_in_title);
    assert!(tabs_share_the_title_bar(&h));
    // The tabs still work from there.
    h.get_by_role_and_label(Role::Tab, "Insert").click();
    h.run_steps(3);
    assert_eq!(h.state().ui.ribbon_tab, "Insert");
    assert!(h.state().ui.tabs_in_title);
}

#[test]
fn minimal_density_does_too() {
    let h = harness(1440.0, Density::Minimal);
    assert!(h.state().ui.tabs_in_title);
    assert!(tabs_share_the_title_bar(&h));
}

#[test]
fn a_narrow_window_keeps_the_tabs_in_the_ribbon() {
    let h = harness(800.0, Density::Compact);
    assert!(!h.state().ui.tabs_in_title);
    assert!(!tabs_share_the_title_bar(&h));
}

#[test]
fn normal_density_keeps_the_tabs_in_the_ribbon() {
    let h = harness(1440.0, Density::Normal);
    assert!(!h.state().ui.tabs_in_title);
    assert!(!tabs_share_the_title_bar(&h));
}
