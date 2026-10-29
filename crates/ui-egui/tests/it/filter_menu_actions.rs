//! The AutoFilter menu's buttons act: Ascending and Descending sort the list by the column, and
//! Clear Filter shows the rows the column's filter hid.

use egui::{Event, Key, Modifiers, PointerButton};
use egui_kittest::kittest::Queryable;
use mkmatrix_engine::Session;
use mkmatrix_engine::core::CellRef;
use serde_json::json;

type Harness = crate::common::Harness;

fn open(filtered: bool) -> Harness {
    let mut session = Session::new();
    session.new_workbook();
    session.execute("range.setValues", json!({"range": "A1", "values": [["Fruit"], ["Banana"], ["Cherry"], ["Apple"]]})).unwrap();
    session.execute("data.filter", json!({"range": "A1:A4"})).unwrap();
    if filtered {
        session.execute("data.filterBy", json!({"column": "A", "values": ["Banana"], "blanks": false})).unwrap();
    }
    session.execute("selection.set", json!({"cell": "A1"})).unwrap();
    let mut h = crate::common::harness_with(crate::common::builder().with_size(egui::vec2(1200.0, 800.0)), crate::common::app(session), 4);
    for pressed in [true, false] {
        h.input_mut().events.push(Event::Key {
            key: Key::ArrowDown,
            physical_key: Some(Key::ArrowDown),
            pressed,
            repeat: false,
            modifiers: Modifiers::ALT,
        });
    }
    h.run_steps(3);
    assert!(h.state().grid.filter_menu.is_some(), "Alt+Down opens the filter menu");
    h
}

/// A real click: the pointer moves there, presses and releases.
fn click(h: &mut Harness, label: &str) {
    let at = h.get_by_label(label).rect().center();
    h.input_mut().events.push(Event::PointerMoved(at));
    h.run_steps(1);
    for pressed in [true, false] {
        h.input_mut().events.push(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        h.run_steps(1);
    }
    h.run_steps(2);
}

fn column(h: &Harness) -> Vec<String> {
    let sh = h.state().session.active().unwrap().wb.active().unwrap();
    (1..=3)
        .map(|r| match sh.value(CellRef::new(r, 0)) {
            mkmatrix_engine::core::Value::Text(t) => t.to_string(),
            v => format!("{v:?}"),
        })
        .collect()
}

#[test]
fn ascending_and_descending_sort_the_column() {
    let mut h = open(false);
    click(&mut h, "Sort Ascending");
    assert_eq!(column(&h), ["Apple", "Banana", "Cherry"]);
    assert!(h.state().grid.filter_menu.is_none(), "the menu closes");
    for pressed in [true, false] {
        h.input_mut().events.push(Event::Key {
            key: Key::ArrowDown,
            physical_key: Some(Key::ArrowDown),
            pressed,
            repeat: false,
            modifiers: Modifiers::ALT,
        });
    }
    h.run_steps(3);
    click(&mut h, "Sort Descending");
    assert_eq!(column(&h), ["Cherry", "Banana", "Apple"]);
}

#[test]
fn clear_filter_shows_the_hidden_rows() {
    let mut h = open(true);
    let hidden = |h: &Harness| {
        let sh = h.state().session.active().unwrap().wb.active().unwrap();
        (1..=3).map(|r| sh.is_row_hidden(r)).collect::<Vec<_>>()
    };
    assert_eq!(hidden(&h), [false, true, true]);
    click(&mut h, "Clear Filter");
    assert_eq!(hidden(&h), [false, false, false]);
    assert!(h.state().grid.filter_menu.is_none(), "the menu closes");
}

#[test]
fn on_an_unfiltered_column_clear_offers_every_filter() {
    let mut session = Session::new();
    session.new_workbook();
    session.execute("range.setValues", json!({"range": "A1", "values": [["Fruit", "Colour"], ["Banana", "yellow"], ["Cherry", "red"]]})).unwrap();
    session.execute("data.filter", json!({"range": "A1:B3"})).unwrap();
    // A condition, not a list of values, on column B.
    session.execute("data.filterBy", json!({"column": "B", "custom": {"op": "beginsWith", "value": "y"}})).unwrap();
    session.execute("selection.set", json!({"cell": "A1"})).unwrap();
    let mut h = crate::common::harness_with(crate::common::builder().with_size(egui::vec2(1200.0, 800.0)), crate::common::app(session), 4);
    let hidden = |h: &Harness| h.state().session.active().unwrap().wb.active().unwrap().is_row_hidden(2);
    assert!(hidden(&h), "the condition hides Cherry's row");
    for pressed in [true, false] {
        h.input_mut().events.push(Event::Key {
            key: Key::ArrowDown,
            physical_key: Some(Key::ArrowDown),
            pressed,
            repeat: false,
            modifiers: Modifiers::ALT,
        });
    }
    h.run_steps(3);
    click(&mut h, "Clear All Filters");
    assert!(!hidden(&h), "every filter is cleared");
}
