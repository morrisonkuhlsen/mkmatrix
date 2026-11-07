//! The Format Chart Area pane's Chart Type section changes the chart's type and marks the current one.

use egui::accesskit::{Role, Toggled};
use egui_kittest::kittest::{NodeT, Queryable};
use mkmatrix_engine::Session;
use mkmatrix_engine::model::ChartKind;
use mkmatrix_ui_egui::SheetApp;
use serde_json::json;

type Harness = egui_kittest::Harness<'static, SheetApp>;

/// A two-series chart made with `insert.chart`'s `type` `t` and `subtype` `sub`, with Format Chart Area open on its Chart Type section.
fn open_chart_type(t: &str, sub: &str) -> (Harness, u32) {
    let mut session = Session::new();
    session.new_workbook();
    session.execute("range.setValues", json!({"range": "A1", "values": [["Quarter", "North", "South"], ["Q1", 10, 30], ["Q2", 30, 10]]})).unwrap();
    session.execute("selection.set", json!({"range": "A1:C3"})).unwrap();
    let chart = session.execute("insert.chart", json!({"type": t, "subtype": sub, "at": "E3"})).unwrap();
    let id = chart["chart"].as_u64().unwrap() as u32;
    let mut h = crate::support::harness(session, 4);
    h.state_mut().selected_chart = Some(id);
    h.state_mut().grid.pane = Some("formatChart".into());
    h.run_steps(2);
    (h, id)
}

fn kind(h: &Harness, id: u32) -> ChartKind {
    h.state().session.active().unwrap().wb.active().unwrap().charts.iter().find(|c| c.id == id).unwrap().kind
}

/// The pane's left edge: its title's. The ribbon's buttons are further left.
fn pane_left(h: &Harness) -> f32 {
    h.get_by_label("Format Chart Area").rect().left()
}

/// Whether the pane's `label` row is drawn as the chart's current type.
fn marked(h: &Harness, label: &str) -> bool {
    let left = pane_left(h);
    let button = h.get_all_by_role_and_label(Role::Button, label).find(|n| n.rect().left() >= left - 1.0).unwrap();
    button.accesskit_node().toggled() == Some(Toggled::True)
}

fn click(h: &mut Harness, label: &str) {
    let left = pane_left(h);
    h.get_all_by_role_and_label(Role::Button, label).find(|n| n.rect().left() >= left - 1.0).unwrap().click();
    h.run_steps(2);
}

#[test]
fn chart_type_offers_the_stacked_and_hundred_percent_stacked_types() {
    let (mut h, id) = open_chart_type("column", "");
    for (label, want) in [
        ("Stacked Column", ChartKind::ColumnStacked),
        ("100% Stacked Column", ChartKind::ColumnStacked100),
        ("Stacked Bar", ChartKind::BarStacked),
        ("100% Stacked Bar", ChartKind::BarStacked100),
        ("Stacked Line", ChartKind::LineStacked),
        ("Stacked Area", ChartKind::AreaStacked),
        ("100% Stacked Area", ChartKind::AreaStacked100),
        ("Column", ChartKind::ColumnClustered),
    ] {
        click(&mut h, label);
        assert_eq!(kind(&h, id), want, "{label} changes the chart's type");
    }
}

#[test]
fn chart_type_marks_the_chart_s_current_type() {
    for (t, sub, label) in
        [("bar", "stacked100", "100% Stacked Bar"), ("area", "stacked", "Stacked Area"), ("column", "", "Column"), ("line", "markers", "Line")]
    {
        let (h, _) = open_chart_type(t, sub);
        for other in [
            "Column",
            "Stacked Column",
            "100% Stacked Column",
            "Bar",
            "Stacked Bar",
            "100% Stacked Bar",
            "Line",
            "Stacked Line",
            "Area",
            "Stacked Area",
            "100% Stacked Area",
            "Pie",
            "Doughnut",
            "Scatter",
            "Radar",
        ] {
            assert_eq!(marked(&h, other), other == label, "{other} on a {t} {sub} chart");
        }
    }
}

/// The pane's controls fit its width: none asks for more, so the pane doesn't grow by a few
/// points each frame (a text field's margins once pushed it wider until it hit its cap).
#[test]
fn the_pane_keeps_its_width_from_frame_to_frame() {
    let (mut h, _) = open_chart_type("column", "");
    let before = pane_left(&h);
    h.run_steps(6);
    assert_eq!(pane_left(&h), before);
}
