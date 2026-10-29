//! A field in a PivotTable's Filters area gets the filter dropdown on its value cell: clicking it
//! opens the filter menu with the field's items and their record counts, and Apply narrows the
//! report through `pivot.filter` (issue #85).

use egui::{Event, Modifiers, PointerButton, Pos2};
use egui_kittest::kittest::Queryable as _;
use mkmatrix_engine::Session;
use mkmatrix_engine::core::CellRef;
use mkmatrix_ui_egui::{SheetApp, grid};
use serde_json::json;

type Harness = egui_kittest::Harness<'static, SheetApp>;

/// Region as a report filter: E1 "Region", F1 "(All)" with its dropdown, the body from E3.
fn harness() -> Harness {
    let mut session = Session::new();
    session.new_workbook();
    session
        .execute(
            "range.setValues",
            json!({"range": "A1", "values": [["Region", "Product", "Sales"], ["East", "Apples", 100], ["West", "Pears", 80], ["East", "Pears", 30], ["North", "Apples", 5]]}),
        )
        .unwrap();
    session
        .execute(
            "insert.pivotTable",
            json!({"source": "Sheet1!A1:C5", "destination": "Sheet1!E3", "rows": ["Product"], "values": ["Sales"], "filters": ["Region"]}),
        )
        .unwrap();
    session.execute("selection.set", json!({"cell": "A1"})).unwrap();
    crate::support::stepped_harness(session)
}

fn text(h: &Harness, a1: &str) -> String {
    h.state().session.active().unwrap().wb.active().unwrap().value(CellRef::parse(a1).unwrap()).display()
}

fn click_at(h: &mut Harness, at: Pos2) {
    h.input_mut().events.push(Event::PointerMoved(at));
    h.run_steps(1);
    for pressed in [true, false] {
        h.input_mut().events.push(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        h.run_steps(1);
    }
    h.run_steps(2);
}

fn click(h: &mut Harness, label: &str) {
    let at = h.get_by_label(label).rect().center();
    click_at(h, at);
}

fn open_menu(h: &mut Harness) {
    let (cell, button) = {
        let app = h.state();
        let sh = app.session.active().unwrap().wb.active().unwrap();
        let cell = CellRef::parse("F1").unwrap();
        assert!(sh.has_filter_button(cell), "the report filter's value cell has a dropdown");
        let geo = grid::Geo::new(sh, app.grid.rect.unwrap(), app.view().scroll);
        (cell, grid::filter_button_rect(&geo, sh, cell).center())
    };
    click_at(h, button);
    assert_eq!(h.state().grid.filter_menu.map(|(c, r, _)| (c, r)), Some((cell.col, cell.row)), "the filter menu opens on F1");
}

#[test]
fn clicking_a_report_filter_opens_its_menu_and_apply_filters_the_report() {
    let mut h = harness();
    assert_eq!((text(&h, "E1"), text(&h, "F1")), ("Region".into(), "(All)".into()));
    open_menu(&mut h);
    // The field's items, sorted, with no sort buttons (the report orders itself).
    for item in ["East", "North", "West"] {
        assert!(h.query_by_label(item).is_some(), "{item} is listed");
    }
    assert!(h.query_by_label("Sort Ascending").is_none(), "a report filter has no sort");
    click(&mut h, "(Select All)");
    click(&mut h, "East");
    click(&mut h, "Apply");

    assert!(h.state().grid.filter_menu.is_none(), "Apply closes the menu");
    assert_eq!(text(&h, "F1"), "East");
    let wb = &h.state().session.active().unwrap().wb;
    let pt = &wb.active().unwrap().pivots[0];
    assert_eq!(pt.filters[0].selected, Some(vec!["East".to_string()]));
    let sh = wb.active().unwrap();
    let total = (3..12).find(|r| sh.value(CellRef::new(*r, 4)).display() == "Grand Total").expect("a Grand Total row");
    assert_eq!(sh.value(CellRef::new(total, 5)).display(), "130");
}

#[test]
fn clear_filter_shows_every_item_again() {
    let mut h = harness();
    h.state_mut().session.execute("pivot.filter", json!({"field": "Region", "selected": ["West"]})).unwrap();
    h.run_steps(2);
    assert_eq!(text(&h, "F1"), "West");
    open_menu(&mut h);
    click(&mut h, "Clear Filter");
    assert_eq!(text(&h, "F1"), "(All)");
    assert_eq!(h.state().session.active().unwrap().wb.active().unwrap().pivots[0].filters[0].selected, None);
}
