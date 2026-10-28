//! A host that embeds the sheet in its own window can turn off the app's title bar and File tab.

use egui_kittest::kittest::Queryable as _;
use mkmatrix_engine::Session;
use mkmatrix_ui_egui::{Chrome, Services, SheetApp, backstage::Page};

fn harness(chrome: Chrome) -> egui_kittest::Harness<'static, SheetApp> {
    let mut session = Session::new();
    session.new_workbook();
    let mut app = crate::common::prepare(SheetApp::new(session, Services::default()));
    app.chrome = chrome;
    let mut h = crate::common::builder().with_size(egui::vec2(1200.0, 800.0)).build_ui_state(
        |ui, app: &mut SheetApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    );
    h.run_steps(4);
    h
}

#[test]
fn the_title_bar_and_file_tab_are_drawn_by_default() {
    let h = harness(Chrome::default());
    assert!(h.query_by_label("AutoSave").is_some(), "the title bar is drawn");
    assert!(h.query_by_label("File").is_some(), "the File tab is drawn");
}

#[test]
fn a_host_can_leave_out_the_title_bar_and_the_file_tab() {
    let full = harness(Chrome::default());
    let h = harness(Chrome { title_bar: false, file_tab: false, ..Default::default() });
    assert!(h.query_by_label("AutoSave").is_none(), "no title bar");
    assert!(h.query_by_label("File").is_none(), "no File tab");
    // The ribbon tabs are painted text, so measure the move with the first ribbon control.
    let (with_bar, without) = (full.get_by_label("Paste").rect(), h.get_by_label("Paste").rect());
    let moved = with_bar.min.y - without.min.y;
    assert!((25.0..=40.0).contains(&moved), "the ribbon moves up by the title bar's height (28 on macOS, 38 elsewhere), moved {moved}");
    assert_eq!(with_bar.min.x, without.min.x, "the ribbon's groups keep their place across");
}

#[test]
fn without_the_file_tab_the_backstage_view_never_opens() {
    let mut h = harness(Chrome { title_bar: true, file_tab: false, ..Default::default() });
    h.state_mut().open_backstage(Page::Home);
    h.run_steps(3);
    assert!(h.state().backstage.is_none(), "opening Backstage directly is ignored");
    h.state_mut().open_backstage(Page::Print);
    h.run_steps(3);
    assert!(h.state().backstage.is_none(), "so is every page, Print and Save As included");
    assert!(h.query_by_label("AutoSave").is_some(), "the title bar can stay on its own");
}

#[test]
fn turning_the_file_tab_off_closes_an_open_backstage() {
    let mut h = harness(Chrome::default());
    h.state_mut().open_backstage(Page::Home);
    h.run_steps(3);
    assert!(h.state().backstage.is_some(), "Backstage opens with the File tab on");
    h.state_mut().chrome.file_tab = false;
    h.run_steps(3);
    assert!(h.state().backstage.is_none(), "and closes when the host turns the File tab off");
}

#[test]
fn a_frameless_window_gets_its_buttons_in_the_title_bar() {
    let h = harness(Chrome::default());
    assert!(h.query_by_label("Minimize").is_none(), "a window with the system's frame has its own buttons");
    let h = harness(Chrome { window_controls: true, ..Default::default() });
    let min = h.get_by_label("Minimize").rect();
    let max = h.get_by_label("Maximize").rect();
    let close = h.get_all_by_label("Close").map(|n| n.rect()).find(|r| r.min.y < 40.0).expect("Close in the title bar");
    assert!(min.right() <= max.left() + 0.5 && max.right() <= close.left() + 0.5, "minimize, maximize, close from left to right");
    let search = h.get_by_label("Search commands").rect();
    assert!(search.right() <= min.left(), "they end the title bar, right of its other controls");
    assert!(1200.0 - close.right() <= 8.5, "close reaches the window's right edge (the harness keeps an 8 px margin): {close:?}");
    assert!(h.query_by_label("AutoSave").is_some(), "the rest of the title bar stays");
}
