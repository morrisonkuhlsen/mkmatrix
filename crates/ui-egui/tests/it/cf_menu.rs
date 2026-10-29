//! Home › Conditional Formatting is one panel: rules as rows, data bars, colour scales and icon
//! sets as tiles. Each item runs the rule or opens its dialog, as the cascading menus did.

use egui_kittest::kittest::Queryable as _;
use mkmatrix_engine::Session;
use mkmatrix_engine::model::CfRule;
use serde_json::json;

fn harness() -> egui_kittest::Harness<'static, mkmatrix_ui_egui::SheetApp> {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("range.setValues", json!({"range": "A1", "values": [[1], [5], [9]]})).unwrap();
    s.execute("selection.set", json!({"range": "A1:A3"})).unwrap();
    let mut h = crate::support::harness(s, 3);
    h.set_size(egui::vec2(2200.0, 1000.0));
    h.run_steps(2);
    h
}

fn open_and_click(h: &mut egui_kittest::Harness<'static, mkmatrix_ui_egui::SheetApp>, item: &str) {
    h.get_by_label("Conditional Formatting").click();
    h.run_steps(3);
    h.get_by_label(item).click();
    h.run_steps(3);
}

fn rules(h: &egui_kittest::Harness<'static, mkmatrix_ui_egui::SheetApp>) -> Vec<CfRule> {
    h.state().session.active().unwrap().wb.active().unwrap().cond_formats.iter().map(|c| c.rule.clone()).collect()
}

#[test]
fn a_tile_applies_its_rule_and_closes_the_panel() {
    let mut h = harness();
    open_and_click(&mut h, "Green - Yellow - Red");
    assert!(matches!(rules(&h).as_slice(), [CfRule::ColorScale { .. }]), "{:?}", rules(&h));
    assert!(h.query_by_label("Icon Sets").is_none(), "the panel closed");
    open_and_click(&mut h, "3 Traffic Lights");
    assert!(rules(&h).iter().any(|r| matches!(r, CfRule::IconSet { .. })), "{:?}", rules(&h));
}

#[test]
fn a_highlight_rule_opens_its_dialog_and_clear_removes_the_rules() {
    let mut h = harness();
    open_and_click(&mut h, "Greater Than…");
    assert_eq!(h.state().dialog.as_ref().map(|d| d.name.as_str()), Some("cfQuick"));
    h.state_mut().dialog = None;
    open_and_click(&mut h, "Top 10 Items");
    assert_eq!(rules(&h).len(), 1);
    open_and_click(&mut h, "Entire Sheet");
    assert!(rules(&h).is_empty(), "Clear Rules › Entire Sheet");
}
