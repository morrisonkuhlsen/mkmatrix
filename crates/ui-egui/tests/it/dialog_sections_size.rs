//! A dialog keeps its size while the user moves between its sections: Find ▸ Replace and back,
//! Format Cells' six sections, About's three. Each section fits the room the dialog sets aside,
//! and a dialog never shrinks while it is open.

use egui::vec2;
use mkmatrix_engine::Session;
use mkmatrix_ui_egui::SheetApp;
use mkmatrix_ui_egui::control::handle;
use serde_json::{Value as Json, json};

type Harness = egui_kittest::Harness<'static, SheetApp>;

fn harness() -> Harness {
    let mut session = Session::new();
    session.new_workbook();
    let mut h = crate::common::builder().with_size(vec2(1400.0, 900.0)).build_ui_state(
        |ui, app: &mut SheetApp| {
            app.logic(&ui.ctx().clone());
            app.ui(ui);
        },
        crate::common::app(session),
    );
    h.run_steps(6);
    h
}

fn control(h: &mut Harness, method: &str, params: Json) {
    let ctx = h.ctx.clone();
    let _ = handle(h.state_mut(), &ctx, method, &params);
    h.run_steps(4);
}

fn size(h: &Harness, name: &str) -> egui::Vec2 {
    h.ctx.memory(|m| m.area_rect(egui::Id::unique("dialog").with(name))).map(|r| r.size()).unwrap_or_default()
}

/// The dialog's size with each section shown first, in a freshly opened dialog: every section
/// fits the same room, so none grows the window.
fn sizes_opened_on(name: &str, sections: &[&str]) -> Vec<(String, egui::Vec2)> {
    sections
        .iter()
        .map(|s| {
            let mut h = harness();
            control(&mut h, "ui.dialog", json!({"name": name}));
            control(&mut h, "ui.dialog.set", json!({"field": "tab", "value": s}));
            (s.to_string(), size(&h, name))
        })
        .collect()
}

fn assert_one_size(name: &str, sections: &[&str]) {
    let sizes = sizes_opened_on(name, sections);
    let Some((_, first)) = sizes.first().cloned() else { return };
    assert!(first.x > 0.0 && first.y > 0.0, "{name} opened");
    for (section, s) in &sizes {
        assert!((s.x - first.x).abs() < 0.6 && (s.y - first.y).abs() < 0.6, "{name}: {section} is {s:?}, not {first:?} ({sizes:?})");
    }
}

#[test]
fn find_and_replace_are_one_size() {
    assert_one_size("find", &["Find", "Replace"]);
}

#[test]
fn format_cells_sections_are_one_size() {
    assert_one_size("formatCells", &["Number", "Alignment", "Font", "Border", "Fill", "Protection"]);
}

#[test]
fn about_sections_are_one_size() {
    assert_one_size("about", &["About", "Contributors", "Models"]);
}

#[test]
fn data_validation_sections_are_one_size() {
    assert_one_size("dataValidation", &["Settings", "Input Message", "Error Alert"]);
}

/// Moving back to a section with less in it keeps the size the dialog has had.
#[test]
fn a_dialog_never_shrinks_while_it_is_open() {
    let mut h = harness();
    control(&mut h, "ui.dialog", json!({"name": "find", "replace": true}));
    let replace = size(&h, "find");
    control(&mut h, "ui.dialog.set", json!({"field": "tab", "value": "Find"}));
    let find = size(&h, "find");
    assert!(find.y >= replace.y - 0.5 && find.x >= replace.x - 0.5, "Find {find:?} shrank from Replace {replace:?}");
}
