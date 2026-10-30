//! A PivotTable's header cells get the AutoFilter's dropdown: Row Labels sorts and filters the row
//! fields through `pivot.sort` and `pivot.filter`, one at a time (picked above the list), and each
//! outer item's ⊕/⊖ hides or shows the items under it through `pivot.collapse` (issue #71).

use egui::{Event, Modifiers, PointerButton, Pos2};
use egui_kittest::kittest::Queryable as _;
use mkmatrix_engine::Session;
use mkmatrix_engine::core::CellRef;
use mkmatrix_ui_egui::{SheetApp, grid};
use serde_json::json;

type Harness = egui_kittest::Harness<'static, SheetApp>;

/// Region and Product in Rows (compact): E3 "Row Labels", then E4 East, E5 Apples, E6 Pears, E7
/// North, E8 Apples, E9 West, E10 Pears, E11 Grand Total.
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
            json!({"source": "Sheet1!A1:C5", "destination": "Sheet1!E3", "rows": ["Region", "Product"], "values": ["Sales"]}),
        )
        .unwrap();
    session.execute("selection.set", json!({"cell": "A1"})).unwrap();
    crate::support::stepped_harness(session)
}

fn text(h: &Harness, a1: &str) -> String {
    h.state().session.active().unwrap().wb.active().unwrap().value(CellRef::parse(a1).unwrap()).display()
}

fn column(h: &Harness, from: u32, to: u32) -> Vec<String> {
    (from..=to).map(|r| text(h, &format!("E{r}"))).collect()
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

/// Clicks the first control named `label` (a sort button's name also labels its icon).
fn click(h: &mut Harness, label: &str) {
    let at = h.get_all_by_label(label).next().unwrap().rect().center();
    click_at(h, at);
}

/// Where a button of `a1` is on screen: the header's dropdown, or the item's ⊕/⊖.
fn button(h: &Harness, a1: &str, toggle: bool) -> Pos2 {
    let app = h.state();
    let wb = &app.session.active().unwrap().wb;
    let sh = wb.active().unwrap();
    let geo = grid::Geo::new(sh, app.grid.rect.unwrap(), app.view().scroll);
    let c = CellRef::parse(a1).unwrap();
    let r = if toggle { grid::pivot_buttons::toggle_rect(&geo, wb, sh, c) } else { grid::pivot_buttons::field_rect(&geo, sh, c) };
    geo.mirror_pos(r.center())
}

fn open_menu(h: &mut Harness) {
    let at = button(h, "E3", false);
    click_at(h, at);
    let e3 = CellRef::parse("E3").unwrap();
    assert_eq!(h.state().grid.filter_menu.map(|(c, r, _)| (c, r)), Some((e3.col, e3.row)), "Row Labels opens the filter menu");
}

#[test]
fn row_labels_sorts_the_picked_field() {
    let mut h = harness();
    assert_eq!(column(&h, 3, 4), ["Row Labels", "East"]);
    open_menu(&mut h);
    // Both row fields to pick from; Region, the outer one, is shown first.
    assert!(h.query_all_by_label("Region").next().is_some() && h.query_by_label("Product").is_some(), "the field picker lists the row fields");
    for item in ["East", "North", "West"] {
        assert!(h.query_by_label(item).is_some(), "{item} is listed");
    }
    if let Ok(path) = std::env::var("MKMATRIX_PIVOT_HEADER_SCREENSHOT") {
        h.render().unwrap().save(path).unwrap();
    }
    click(&mut h, "Sort Descending");
    assert!(h.state().grid.filter_menu.is_none(), "sorting closes the menu");
    assert_eq!(column(&h, 4, 10), ["West", "Pears", "North", "Apples", "East", "Apples", "Pears"]);
}

#[test]
fn row_labels_filters_the_inner_field_once_it_is_picked() {
    let mut h = harness();
    open_menu(&mut h);
    click(&mut h, "Product");
    assert!(h.query_by_label("Apples").is_some() && h.query_by_label("East").is_none(), "Product's items replace Region's");
    click(&mut h, "(Select All)");
    click(&mut h, "Apples");
    click(&mut h, "Apply");
    assert!(h.state().grid.filter_menu.is_none(), "Apply closes the menu");
    let pt = h.state().session.active().unwrap().wb.active().unwrap().pivots[0].clone();
    assert_eq!(pt.filters.iter().find(|f| f.source_col == "Product").and_then(|f| f.selected.clone()), Some(vec!["Apples".to_string()]));
    assert_eq!(column(&h, 4, 8), ["East", "Apples", "North", "Apples", "Grand Total"]);
}

#[test]
fn an_items_toggle_collapses_and_expands_it() {
    let mut h = harness();
    let at = button(&h, "E4", true);
    click_at(&mut h, at);
    let pt = h.state().session.active().unwrap().wb.active().unwrap().pivots[0].clone();
    assert_eq!(pt.rows[0].collapsed_items, ["East"]);
    if let Ok(path) = std::env::var("MKMATRIX_PIVOT_TOGGLE_SCREENSHOT") {
        h.render().unwrap().save(path).unwrap();
    }
    assert_eq!(column(&h, 4, 6), ["East", "North", "Apples"], "East's products are hidden");
    let at = button(&h, "E4", true);
    click_at(&mut h, at);
    assert_eq!(column(&h, 4, 6), ["East", "Apples", "Pears"], "and shown again");
}
