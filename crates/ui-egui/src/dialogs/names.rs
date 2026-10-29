use super::kit::{ROW_H, button, footer, heading, hint, list_row, panel, rule};
use super::*;

use egui::pos2;

use crate::theme::Tokens;

/// The Name Manager's table: twelve rows, so the window keeps one size.
const LIST_H: f32 = 12.0 * ROW_H + 2.0;
/// Paste Name's list.
const PASTE_LIST_H: f32 = 8.0 * ROW_H + 2.0;
/// The space before each of the table's columns.
const PAD: f32 = 10.0;

/// Name Manager, in Format Cells' style: New, Edit and Delete above a framed table of the names
/// (Name, Value, Refers To, Scope under muted headings, the chosen row tinted with the accent),
/// then the count and Close. A double-click edits the name.
pub(super) fn name_manager(app: &mut SheetApp, ui: &mut egui::Ui, d: &mut Dialog, open: &mut bool) {
    let lang = app.ui.language;
    let t = Tokens::get(ui.ctx());
    let list = app.session.run("formulas.nameManager", json!({})).unwrap_or(Json::Null);
    let names = list.as_array().cloned().unwrap_or_default();
    d.list_index = d.list_index.min(names.len().saturating_sub(1));
    let selected = names.get(d.list_index).cloned();
    let field = |n: &Json, k: &str| n.get(k).and_then(Json::as_str).unwrap_or("").to_string();
    let edit = |app: &mut SheetApp, n: &Json| {
        app.open_dialog(
            "defineName",
            json!({
                "name": field(n, "name"), "scope": field(n, "scope"), "comment": field(n, "comment"),
                "refersTo": field(n, "refersTo"), "oldName": field(n, "name"), "oldScope": field(n, "scope"),
            }),
        );
    };
    ui.spacing_mut().item_spacing.y = 8.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        if button(ui, lang.tr("New…"), false).clicked() {
            app.open_dialog("defineName", json!({}));
        }
        ui.add_enabled_ui(selected.is_some(), |ui| {
            if button(ui, lang.tr("Edit…"), false).clicked()
                && let Some(n) = &selected
            {
                edit(app, n);
            }
            if button(ui, lang.tr("Delete"), false).clicked()
                && let Some(n) = &selected
            {
                app.run_or_alert("formulas.deleteName", json!({"name": field(n, "name"), "scope": field(n, "scope")}));
            }
        });
    });

    // Name, Value and Scope are fixed; Refers To takes the rest.
    let width = ui.available_width();
    let (name_w, value_w, scope_w) = (150.0, 110.0, 90.0);
    let refers_w = (width - name_w - value_w - scope_w - 4.0 * PAD).max(80.0);
    let cols = [(lang.tr("Name"), name_w), (lang.tr("Value"), value_w), (lang.tr("Refers To"), refers_w), (lang.tr("Scope"), scope_w)];
    // The headings, over the table's columns.
    let (head, _) = ui.allocate_exact_size(vec2(width, 16.0), egui::Sense::hover());
    let mut x = head.left() + PAD;
    for (text, w) in cols {
        ui.painter().text(pos2(x, head.center().y), egui::Align2::LEFT_CENTER, text, theme::ui_bold(11.5), t.text_dim);
        x += w + PAD;
    }
    let mut edit_row = None;
    panel(ui, LIST_H, |ui| {
        if names.is_empty() {
            ui.add_space(LIST_H / 2.0 - 10.0);
            ui.vertical_centered(|ui| hint(ui, lang.tr("No names defined")));
            return;
        }
        ui.spacing_mut().item_spacing.y = 0.0;
        egui::ScrollArea::vertical().id_salt("nameManager.list").auto_shrink([false, false]).show_rows(ui, ROW_H, names.len(), |ui, rows| {
            for i in rows {
                let Some(n) = names.get(i) else { continue };
                let on = i == d.list_index;
                let name = field(n, "name");
                let value: String = n.get("value").map(|v| v.to_string()).unwrap_or_default().chars().take(40).collect();
                let cells = [name.clone(), value, field(n, "refersTo"), field(n, "scope")];
                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), egui::Sense::click());
                resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, &name));
                let p = ui.painter();
                if on {
                    p.rect_filled(r, 0.0, t.accent_soft);
                    p.rect_filled(egui::Rect::from_min_size(r.min, vec2(2.0, r.height())), 0.0, t.accent);
                } else if resp.hovered() {
                    p.rect_filled(r, 0.0, t.hover);
                }
                let mut x = r.left() + PAD;
                for (k, (text, (_, w))) in cells.iter().zip(cols).enumerate() {
                    let clip = egui::Rect::from_min_max(pos2(x, r.top()), pos2((x + w).min(r.right()), r.bottom()));
                    let (font, ink) = match k {
                        0 if on => (theme::ui_bold(13.0), t.accent),
                        0 => (theme::ui_font(13.0), t.text),
                        _ => (theme::ui_font(12.5), t.text_dim),
                    };
                    p.with_clip_rect(clip).text(pos2(x, r.center().y), egui::Align2::LEFT_CENTER, text, font, ink);
                    x += w + PAD;
                }
                if resp.clicked() {
                    d.list_index = i;
                }
                if resp.double_clicked() {
                    edit_row = Some(n.clone());
                }
            }
        });
    });
    if let Some(n) = edit_row {
        edit(app, &n);
    }

    rule(ui);
    ui.horizontal(|ui| {
        hint(ui, &lang.tr("{count} names").replace("{count}", &names.len().to_string()));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if button(ui, lang.tr("Close"), true).clicked() {
                *open = false;
            }
        });
    });
}

/// The names Paste Names offers in the active workbook.
pub(super) fn pasteable_names(app: &SheetApp) -> Vec<String> {
    app.session.active().map(|doc| mkmatrix_engine::cmd::formulas::pasteable_names(&doc.wb).into_iter().map(|(n, _)| n).collect()).unwrap_or_default()
}

/// Puts a defined name into the formula being edited, at the caret. Without an edit under way it
/// starts one in the active cell with `=` and the name.
pub(crate) fn insert_name(app: &mut SheetApp, name: &str) {
    match app.editor.as_mut() {
        Some(ed) => ed.insert_at_caret(name),
        None => app.begin_edit(Some(format!("={name}")), false),
    }
}

/// Paste Name (F3, Formulas › Use in Formula › Paste Names…): OK, or a double-click, inserts the
/// selected name (`formulas.pasteName` with `name`); Paste List writes every name and what it
/// refers to from the active cell.
pub(super) fn paste_name(app: &mut SheetApp, ui: &mut egui::Ui, d: &mut Dialog, confirm: &mut bool, open: &mut bool) {
    let lang = app.ui.language;
    let names: Vec<String> =
        d.result.as_ref().and_then(Json::as_array).map(|a| a.iter().filter_map(Json::as_str).map(str::to_owned).collect()).unwrap_or_default();
    // The `name` value is the selection, so `ui.dialog.set` can choose one too.
    if let Some(i) = d.values.get("name").and_then(Json::as_str).and_then(|n| names.iter().position(|x| x == n)) {
        d.list_index = i;
    }
    let (down, up) = ui.input_mut(|i| (i.consume_key(egui::Modifiers::NONE, Key::ArrowDown), i.consume_key(egui::Modifiers::NONE, Key::ArrowUp)));
    let last = names.len().saturating_sub(1);
    if down {
        d.list_index = (d.list_index + 1).min(last);
    }
    if up {
        d.list_index = d.list_index.saturating_sub(1);
    }
    d.list_index = d.list_index.min(last);
    ui.spacing_mut().item_spacing.y = 8.0;
    heading(ui, lang.tr("Paste name"));
    panel(ui, PASTE_LIST_H, |ui| {
        if names.is_empty() {
            ui.add_space(PASTE_LIST_H / 2.0 - 10.0);
            ui.vertical_centered(|ui| hint(ui, lang.tr("No names defined")));
            return;
        }
        ui.spacing_mut().item_spacing.y = 0.0;
        // A workbook can hold thousands of names: only the rows in view are laid out.
        egui::ScrollArea::vertical().id_salt("pasteName.list").auto_shrink([false, false]).show_rows(ui, ROW_H, names.len(), |ui, rows| {
            for i in rows {
                let Some(n) = names.get(i) else { continue };
                let on = i == d.list_index;
                let r = list_row(ui, n, None, on, ui.available_width());
                // The arrow keys can move the selection onto a row that isn't in view.
                if (down || up) && on && !ui.clip_rect().contains_rect(r.rect) {
                    r.scroll_to_me(Some(if down { egui::Align::Max } else { egui::Align::Min }));
                }
                if r.clicked() {
                    d.list_index = i;
                }
                if r.double_clicked() {
                    *confirm = true;
                }
            }
        });
    });
    if let Some(n) = names.get(d.list_index) {
        d.values.insert("name".into(), json!(n));
    }
    let mut list = false;
    footer(ui, confirm, open, |ui| {
        // Paste List is disabled while a cell is being edited.
        ui.add_enabled_ui(!names.is_empty() && app.editor.is_none(), |ui| list = button(ui, lang.tr("Paste List"), false).clicked());
    });
    if list {
        *confirm = false;
        app.run_or_alert("formulas.pasteName", json!({"list": true}));
        *open = false;
    }
}
