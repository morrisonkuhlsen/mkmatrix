//! File › Print: the settings cards change the active sheet's Page Setup, as Excel's print page
//! does, and the preview follows.

use egui_kittest::kittest::Queryable as _;
use mkmatrix_engine::Session;
use mkmatrix_engine::model::Orientation;
use mkmatrix_ui_egui::SheetApp;
use serde_json::json;

type Harness = egui_kittest::Harness<'static, SheetApp>;

fn harness() -> Harness {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("range.setValues", json!({"range": "A1", "values": [["Region", "Q1"], ["North", 10]]})).unwrap();
    let mut app = crate::common::app(s);
    app.run("ui.backstage", json!({"page": "print"})).unwrap();
    crate::common::harness_with(crate::common::builder().with_size(egui::vec2(1200.0, 800.0)), app, 5)
}

fn print(h: &Harness) -> mkmatrix_engine::model::PrintSettings {
    h.state().session.active().unwrap().wb.active().unwrap().print.clone()
}

/// Opens the card whose accessible name starts with `card`, then picks `choice`.
fn pick(h: &mut Harness, card: &str, choice: &str) {
    h.get_by(|n| n.label().is_some_and(|l| l.starts_with(card))).click();
    h.run_steps(3);
    h.get_by_label(choice).click();
    h.run_steps(3);
}

#[test]
fn orientation_and_scaling_cards_change_the_page_setup() {
    let mut h = harness();
    assert_eq!(print(&h).orientation, Orientation::Portrait);
    pick(&mut h, "Orientation:", "Landscape Orientation");
    assert_eq!(print(&h).orientation, Orientation::Landscape);
    pick(&mut h, "Scaling:", "Fit Sheet on One Page");
    assert_eq!(print(&h).fit_to, Some((1, 1)));
    pick(&mut h, "Scaling:", "No Scaling");
    assert_eq!((print(&h).fit_to, print(&h).scale), (None, 100));
    pick(&mut h, "Margins:", "Narrow Margins");
    assert_eq!(&print(&h).margins[..4], &[0.25, 0.25, 0.75, 0.75]);
    assert!(h.state().backstage.is_some(), "the print page stays open");
}
