//! The Comments pane: one Show Comments button, and a pane that keeps its width (#710).

use egui::{Event, Modifiers, PointerButton, Pos2, pos2, vec2};
use egui_kittest::kittest::Queryable;
use mkmatrix_engine::Session;
use mkmatrix_ui_egui::SheetApp;
use serde_json::json;

type Harness = egui_kittest::Harness<'static, SheetApp>;

const WIDTH: f32 = 1200.0;

fn harness() -> Harness {
    let mut session = Session::new();
    session.new_workbook();
    let mut h = crate::common::builder().with_step_dt(1.0 / 60.0).with_size(vec2(WIDTH, 800.0)).build_ui_state(
        |ui, app: &mut SheetApp| {
            app.logic(ui.ctx());
            app.ui(ui);
        },
        crate::common::app(session),
    );
    h.run_steps(4);
    h
}

fn grid_right(h: &Harness) -> f32 {
    h.state().grid.rect.unwrap().right()
}

/// A comment whose author line and reply row are wide enough to push the pane if they don't wrap.
fn open_pane_on_long_comment(h: &mut Harness) {
    let text = "A long comment that should wrap inside the pane instead of pushing it wider. ".repeat(6);
    let author = "Someone With A Rather Long Display Name, Finance Department, Head Office";
    h.state_mut().run("review.newComment", json!({"cell": "B2", "text": text, "author": author})).unwrap();
    h.state_mut().run("review.showComments", json!({"on": true})).unwrap();
    h.run_steps(3);
}

fn drag(h: &mut Harness, from: Pos2, to: Pos2) {
    let press = |pos, pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    h.input_mut().events.extend([Event::PointerMoved(from), press(from, true)]);
    h.step();
    for i in 1..=6 {
        let t = i as f32 / 6.0;
        h.input_mut().events.push(Event::PointerMoved(from + (to - from) * t));
        h.step();
    }
    h.input_mut().events.push(press(to, false));
    h.run_steps(3);
}

#[test]
fn the_pane_does_not_grow_on_its_own_and_keeps_a_manual_resize() {
    let mut h = harness();
    // Between the grid and the right edge of the window, before any pane is open.
    let gutter = WIDTH - grid_right(&h);
    open_pane_on_long_comment(&mut h);
    let opened = grid_right(&h);
    assert!(WIDTH - opened < WIDTH / 2.0, "the pane opens at its default width, got {}", WIDTH - opened);
    h.run_steps(60);
    assert_eq!(grid_right(&h), opened, "the pane must not widen itself frame after frame");

    // Drag the pane's left edge 150px to the left: the new width stays.
    let edge = pos2(opened + gutter, 450.0);
    drag(&mut h, edge, edge - vec2(150.0, 0.0));
    let resized = grid_right(&h);
    // The grab point is a few pixels off the exact edge, so allow some slack on the 150px.
    assert!(opened - resized > 120.0, "the drag widens the pane, grid edge {opened} -> {resized}");
    h.run_steps(60);
    assert_eq!(grid_right(&h), resized, "a manual resize must hold");

    // Dragged almost to the window's left edge, the pane stops at half the window.
    let edge = pos2(resized + gutter, 450.0);
    drag(&mut h, edge, pos2(60.0, 450.0));
    assert!(grid_right(&h) > WIDTH / 2.0 - gutter - 30.0, "the pane is capped at half the window, grid edge {}", grid_right(&h));
}

/// How many separate ribbon buttons show `tip` when hovered, scanning the big-button row.
fn buttons_with_tip(h: &mut Harness, tip: &str) -> usize {
    h.ctx.all_styles_mut(|s| s.interaction.tooltip_delay = 0.0);
    let (mut count, mut inside) = (0, false);
    // Along the toolbar's row of buttons.
    let y = h.get_by_role_and_label(egui::accesskit::Role::Tab, "Home").rect().bottom() + 2.0 + 15.0;
    let mut x = 2.0;
    while x < WIDTH {
        h.input_mut().events.push(Event::PointerMoved(pos2(x, y)));
        h.run_steps(6);
        let shown = h.query_all_by_value(tip).next().is_some();
        if shown && !inside {
            count += 1;
        }
        inside = shown;
        x += 8.0;
    }
    count
}

#[test]
fn review_has_a_single_show_comments_button() {
    let mut h = harness();
    h.state_mut().ui.ribbon_tab = "Review".into();
    h.run_steps(2);
    // The scan has to find a known single button, or a count of one below would prove nothing.
    assert_eq!(buttons_with_tip(&mut h, "New Comment"), 1);
    assert_eq!(buttons_with_tip(&mut h, "Comments pane"), 1);
}

/// Comments on a table (Region, Owner) and on a second sheet; the pane open.
fn harness_with_comments() -> Harness {
    let mut h = harness();
    let app = h.state_mut();
    app.run("range.setValues", json!({"range": "A1", "values": [["Region", "Owner"], ["East", "Ana"], ["West", "João"], ["North", "Rui"]]})).unwrap();
    app.run("insert.table", json!({"range": "A1:B4", "header": true})).unwrap();
    app.run("review.newComment", json!({"cell": "B3", "text": "Revisar o orçamento de março", "author": "Ana"})).unwrap();
    app.run("review.newComment", json!({"cell": "A2", "text": "Confirmed with the team", "author": "Rui"})).unwrap();
    app.run("review.resolveComment", json!({"cell": "A2", "resolved": true})).unwrap();
    app.run("home.insertSheet", json!({"name": "Sheet2"})).unwrap();
    app.run("review.newComment", json!({"sheet": "Sheet2", "cell": "C5", "text": "Orçamento final aqui", "author": "Bia"})).unwrap();
    app.run("sheet.activate", json!({"sheet": "Sheet1"})).unwrap();
    app.run("selection.set", json!({"cell": "D10"})).unwrap();
    app.run("review.showComments", json!({"on": true})).unwrap();
    h.run_steps(4);
    h
}

/// The active sheet's name and cell.
fn active(h: &Harness) -> (String, String) {
    let d = h.state().session.active().unwrap();
    (d.wb.active().unwrap().name.clone(), d.selection.active.a1())
}

fn search(h: &mut Harness, text: &str) {
    h.get_by_label("Search comments").click();
    h.run_steps(2);
    for c in text.chars() {
        h.input_mut().events.push(Event::Text(c.to_string()));
    }
    h.run_steps(3);
}

#[test]
fn each_card_names_its_cell_row_and_column() {
    let mut h = harness_with_comments();
    assert!(
        h.query_by_label("B3").is_some() && h.query_by_label("Row 3 · Owner").is_some(),
        "the card names the cell, its row and its column header"
    );
    assert!(h.query_by_label("Row 2 · Region").is_some());
    assert!(h.query_by_label("2 comments").is_some(), "the active sheet's two comments");
    if let Ok(path) = std::env::var("MKMATRIX_COMMENTS_PANE_SCREENSHOT") {
        h.render().unwrap().save(path).unwrap();
    }
    h.get_by_label("Row 3 · Owner").click();
    h.run_steps(3);
    assert_eq!(active(&h), ("Sheet1".into(), "B3".into()), "a click on the card's place selects its cell");
}

#[test]
fn the_search_ignores_accents_and_enter_goes_to_the_match() {
    let mut h = harness_with_comments();
    search(&mut h, "ORCAMENTO");
    assert!(h.query_by_label("1 of 2 comments").is_some(), "one of the sheet's comments says orçamento");
    assert!(h.query_by_label("A2").is_none(), "the other card is hidden");
    if let Ok(path) = std::env::var("MKMATRIX_COMMENTS_SEARCH_SCREENSHOT") {
        h.render().unwrap().save(path).unwrap();
    }
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert_eq!(active(&h), ("Sheet1".into(), "B3".into()), "Enter selects the match's cell");
}

#[test]
fn the_workbook_scope_finds_comments_on_other_sheets() {
    let mut h = harness_with_comments();
    h.get_by_label("Workbook").click();
    h.run_steps(3);
    assert!(h.query_by_label("3 comments").is_some());
    search(&mut h, "final");
    h.get_by_label("Next comment").click();
    h.run_steps(3);
    assert_eq!(active(&h), ("Sheet2".into(), "C5".into()), "going to a comment on another sheet switches to it");
}

#[test]
fn the_status_segments_filter_resolved_threads() {
    let mut h = harness_with_comments();
    h.get_by_label("Unresolved (1)").click();
    h.run_steps(3);
    assert!(h.query_by_label("B3").is_some() && h.query_by_label("A2").is_none(), "only the open thread");
    h.get_by_label("Resolved (1)").click();
    h.run_steps(3);
    assert!(h.query_by_label("A2").is_some() && h.query_by_label("B3").is_none(), "only the resolved thread");
}

#[test]
fn a_reply_and_an_edit_go_through_the_review_commands() {
    let mut h = harness_with_comments();
    h.get_by_label("Unresolved (1)").click();
    h.run_steps(3);
    h.get_by_label("Reply…").click();
    h.run_steps(2);
    h.input_mut().events.push(Event::Text("Feito".into()));
    h.run_steps(1);
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    let comment = |h: &Harness| {
        h.state().session.active().unwrap().wb.active().unwrap().comments[&mkmatrix_engine::core::CellRef::parse("B3").unwrap()].clone()
    };
    assert_eq!(comment(&h).replies.last().map(|r| r.1.as_str()), Some("Feito"));
    assert!(h.query_by_label("Feito").is_some(), "the reply shows on the card");

    // The editor opens focused, its text selected: typing replaces it.
    h.get_by_label("Edit text").click();
    h.run_steps(3);
    h.input_mut().events.push(Event::Text("Revisado".into()));
    h.run_steps(2);
    h.get_by_label("Save").click();
    h.run_steps(3);
    assert_eq!(comment(&h).text, "Revisado");
}
