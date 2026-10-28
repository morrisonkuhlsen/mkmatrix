use super::kit::{ROW_H, button, form, heading, hint, label, list_row, nav_item, nav_rail, option_row, panel, rule, segments};
use super::show::ok_cancel;
use super::*;

use crate::theme::Tokens;

/// Find and Replace's sections, as `d.tab` names them, with their icon.
const SECTIONS: [(&str, Icon); 2] = [("Find", Icon::Search), ("Replace", Icon::Pen)];
/// Find and Replace's body height: one size for both sections, so the window never jumps.
const FIND_BODY_H: f32 = 236.0;
/// Find and Replace's section rail.
const FIND_NAV_W: f32 = 150.0;

/// Find and Replace: Find and Replace down the left, the search and its options as a form on the
/// right, the cells Find All found listed under them, and the actions in the footer with Find
/// Next (the default, Enter) in the accent.
pub(super) fn find(app: &mut SheetApp, ui: &mut egui::Ui, d: &mut Dialog, open: &mut bool) {
    let lang = app.ui.language;
    let t = Tokens::get(ui.ctx());
    let mut what = d.values.get("what").and_then(Json::as_str).unwrap_or("").to_string();
    let mut with = d.values.get("with").and_then(Json::as_str).unwrap_or("").to_string();
    let mut case = d.values.get("matchCase").and_then(Json::as_bool).unwrap_or(false);
    let mut whole = d.values.get("wholeCell").and_then(Json::as_bool).unwrap_or(false);
    let mut within = d.values.get("within").and_then(Json::as_str).unwrap_or("sheet").to_string();
    let mut look_in = d.values.get("lookIn").and_then(Json::as_str).unwrap_or("formulas").to_string();
    let replacing = d.tab == "Replace";
    ui.horizontal_top(|ui| {
        nav_rail(ui, FIND_NAV_W, FIND_BODY_H, &t, |ui| {
            for (tab, icon) in SECTIONS {
                if nav_item(ui, icon, lang.tr(tab), d.tab == tab, None, &t).clicked() {
                    d.tab = tab.to_string();
                }
            }
        });
        ui.vertical(|ui| {
            ui.set_min_height(FIND_BODY_H);
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 8.0;
            heading(ui, lang.tr("Search"));
            form(ui, "find.form", |ui| {
                let l = label(ui, lang.tr("Find what:"));
                let r = ui.add(egui::TextEdit::singleline(&mut what).desired_width(f32::INFINITY)).labelled_by(l);
                // Like Excel, the cursor starts in "Find what" - asked for once, so clicking another field still works.
                if !d.values.contains_key("focused") {
                    r.request_focus();
                    d.values.insert("focused".into(), json!(true));
                }
                ui.end_row();
                if replacing {
                    let l = label(ui, lang.tr("Replace with:"));
                    ui.add(egui::TextEdit::singleline(&mut with).desired_width(f32::INFINITY)).labelled_by(l);
                    ui.end_row();
                }
            });
            heading(ui, lang.tr("Options"));
            form(ui, "find.options", |ui| {
                label(ui, lang.tr("Within:"));
                if let Some(v) = segments(ui, &within, &[("sheet", lang.tr("Sheet")), ("workbook", lang.tr("Workbook"))], 140.0) {
                    within = v.to_string();
                }
                ui.end_row();
                label(ui, lang.tr("Look in:"));
                if let Some(v) = segments(ui, &look_in, &[("formulas", lang.tr("Formulas")), ("values", lang.tr("Values"))], 140.0) {
                    look_in = v.to_string();
                }
                ui.end_row();
                ui.label("");
                ui.vertical(|ui| {
                    ui.checkbox(&mut case, lang.tr("Match case"));
                    ui.checkbox(&mut whole, lang.tr("Match entire cell contents"));
                });
                ui.end_row();
            });
        });
    });
    d.values.insert("what".into(), json!(what));
    d.values.insert("with".into(), json!(with));
    d.values.insert("matchCase".into(), json!(case));
    d.values.insert("wholeCell".into(), json!(whole));
    d.values.insert("within".into(), json!(within));
    d.values.insert("lookIn".into(), json!(look_in));
    let base = json!({"what": what, "matchCase": case, "wholeCell": whole, "within": within, "lookIn": look_in});
    if let Some(r) = &d.result
        && let Some(list) = r["results"].as_array()
    {
        rule(ui);
        hint(ui, &lang.tr("{count} cell(s) found").replace("{count}", &list.len().to_string()));
        ui.add_space(4.0);
        let height = (list.len().max(1) as f32 * ROW_H + 2.0).min(6.0 * ROW_H + 2.0);
        panel(ui, height, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            egui::ScrollArea::vertical().id_salt("find.results").auto_shrink([false, false]).show_rows(ui, ROW_H, list.len(), |ui, rows| {
                for item in list.get(rows).unwrap_or_default() {
                    let place = format!("{}!{}", item["sheet"].as_str().unwrap_or(""), item["cell"].as_str().unwrap_or(""));
                    if list_row(ui, &place, item["value"].as_str(), false, 120.0).clicked() {
                        if let Some(s) = item["sheet"].as_str() {
                            let _ = app.run("sheet.activate", json!({"sheet": s}));
                        }
                        let _ = app.run("selection.set", json!({"cell": item["cell"]}));
                        app.grid.ensure_visible = true;
                    }
                }
            });
        });
    }
    rule(ui);
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if button(ui, lang.tr("Close"), false).clicked() {
                *open = false;
            }
            if button(ui, lang.tr("Find Next"), true).clicked() || ui.input(|i| i.key_pressed(Key::Enter)) {
                if let Err(e) = app.run("edit.find", base.clone()) {
                    app.message = Some((lang.tr("Find and Replace").into(), crate::clean_error(&e)));
                }
                app.grid.ensure_visible = true;
            }
            if button(ui, lang.tr("Find All"), false).clicked() {
                let mut p = base.clone();
                p["all"] = json!(true);
                d.result = app.run("edit.find", p).ok();
            }
            if replacing {
                if button(ui, lang.tr("Replace"), false).clicked() {
                    let mut p = base.clone();
                    p["with"] = json!(with);
                    p["all"] = json!(false);
                    let _ = app.run("edit.replace", p);
                    let _ = app.run("edit.find", base.clone());
                }
                if button(ui, lang.tr("Replace All"), false).clicked() {
                    let mut p = base.clone();
                    p["with"] = json!(with);
                    p["all"] = json!(true);
                    match app.run("edit.replace", p) {
                        Ok(r) => {
                            let done = lang.tr("All done. We made {count} replacements.").replace("{count}", &r["replaced"].to_string());
                            app.toast = Some((done, crate::now_ms()));
                        }
                        Err(e) => app.message = Some((lang.tr("Find and Replace").into(), crate::clean_error(&e))),
                    }
                }
            }
        });
    });
}

/// What the Go To dialog lists: the named ranges of every sheet, then the references gone to
/// before, newest first.
pub(super) fn go_to_entries(app: &SheetApp) -> Vec<String> {
    app.session
        .active()
        .map(|doc| {
            let mut v = mkmatrix_engine::cmd::edit::go_to_names(&doc.wb, doc.wb.active_sheet());
            v.extend(doc.go_to_recent.iter().cloned());
            v
        })
        .unwrap_or_default()
}

/// Go To: the named ranges and the references gone to before in a list (only the rows in view
/// laid out), the reference to go to in a form under it, and Special… beside Cancel / OK.
pub(super) fn go_to(app: &mut SheetApp, ui: &mut egui::Ui, d: &mut Dialog, confirm: &mut bool) {
    let lang = app.ui.language;
    let entries: Vec<String> =
        d.result.as_ref().and_then(Json::as_array).map(|a| a.iter().filter_map(Json::as_str).map(str::to_owned).collect()).unwrap_or_default();
    let current = d.values.get("reference").and_then(Json::as_str).unwrap_or("").to_string();
    ui.spacing_mut().item_spacing.y = 8.0;
    heading(ui, lang.tr("Go to"));
    panel(ui, 6.0 * ROW_H + 2.0, |ui| {
        if entries.is_empty() {
            ui.add_space(2.5 * ROW_H);
            ui.vertical_centered(|ui| hint(ui, lang.tr("No named ranges or recent references yet.")));
            return;
        }
        ui.spacing_mut().item_spacing.y = 0.0;
        // A workbook can hold thousands of names: only the rows in view are laid out.
        egui::ScrollArea::vertical().id_salt("goTo.entries").auto_shrink([false, false]).show_rows(ui, ROW_H, entries.len(), |ui, rows| {
            for n in entries.get(rows).unwrap_or_default() {
                let resp = list_row(ui, n, None, current == *n, ui.available_width());
                if resp.clicked() || resp.double_clicked() {
                    d.values.insert("reference".into(), json!(n));
                }
                if resp.double_clicked() {
                    *confirm = true;
                }
            }
        });
    });
    let mut r = d.values.get("reference").and_then(Json::as_str).unwrap_or("").to_string();
    form(ui, "goTo.form", |ui| {
        let l = label(ui, lang.tr("Reference:"));
        ui.add(egui::TextEdit::singleline(&mut r).desired_width(f32::INFINITY)).labelled_by(l).request_focus();
        ui.end_row();
    });
    d.values.insert("reference".into(), json!(r.clone()));
    let mut open = true;
    let mut special = false;
    super::kit::footer(ui, confirm, &mut open, |ui| {
        special = button(ui, lang.tr("Special…"), false).clicked();
    });
    if special {
        app.open_dialog("goToSpecial", json!({}));
        return;
    }
    if *confirm {
        app.run_or_alert("edit.goTo", json!({"reference": r, "record": true}));
        app.grid.ensure_visible = true;
    }
    if !open {
        *confirm = true;
    }
}

/// Go To Special: its kinds of cells as two columns of choices (a double-click goes at once), and
/// Cancel / OK. OK runs the form's `edit.goToSpecial` with the chosen `kind`.
pub(super) fn go_to_special(app: &SheetApp, ui: &mut egui::Ui, d: &mut Dialog, confirm: &mut bool, open: &mut bool) {
    let lang = app.ui.language;
    let options: Vec<(&'static str, &'static str)> = d
        .fields
        .iter()
        .find_map(|f| match f {
            Field::Choice { key: "kind", options, .. } => Some(options.clone()),
            _ => None,
        })
        .unwrap_or_default();
    let mut kind = d.values.get("kind").and_then(Json::as_str).unwrap_or("blanks").to_string();
    ui.spacing_mut().item_spacing.y = 8.0;
    heading(ui, lang.tr("Select"));
    let half = options.len().div_ceil(2);
    let width = (ui.available_width() - 12.0) / 2.0;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = vec2(12.0, 2.0);
        for column in [options.get(..half), options.get(half..)] {
            ui.vertical(|ui| {
                for (value, text) in column.unwrap_or_default() {
                    let r = option_row(ui, lang.tr(text), kind == *value, width);
                    if r.clicked() || r.double_clicked() {
                        kind = (*value).to_string();
                    }
                    if r.double_clicked() {
                        *confirm = true;
                    }
                }
            });
        }
    });
    d.values.insert("kind".into(), json!(kind));
    rule(ui);
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ok_cancel(ui, confirm, open);
        });
    });
}
