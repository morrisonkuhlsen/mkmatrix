use super::kit::{ROW_H, button, heading, hint, list_row, panel, rule};
use super::*;

use crate::theme::Tokens;

/// The suggestions list's height.
const LIST_H: f32 = 6.0 * ROW_H + 2.0;

/// Review › Spelling, in Format Cells' style: the word not in the dictionary (and its cell) in
/// large type, the suggestions in a framed list, and Ignore Once, Add to Dictionary, Change and
/// Change All in the footer. Each answer moves on to the next word.
pub(super) fn spelling(app: &mut SheetApp, ui: &mut egui::Ui, d: &mut Dialog, open: &mut bool) {
    let lang = app.ui.language;
    let t = Tokens::get(ui.ctx());
    let r = d.result.clone().unwrap_or(Json::Null);
    ui.spacing_mut().item_spacing.y = 8.0;
    let close = |ui: &mut egui::Ui, open: &mut bool| {
        rule(ui);
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if button(ui, lang.tr("OK"), true).clicked() {
                    *open = false;
                }
            });
        });
    };
    if let Some(e) = r.get("error").and_then(Json::as_str) {
        ui.label(egui::RichText::new(lang.tr_error(e)).font(theme::ui_font(13.0)));
        close(ui, open);
        return;
    }
    let issues = r["issues"].as_array().cloned().unwrap_or_default();
    let Some(it) = issues.get(d.list_index) else {
        ui.add_space(8.0);
        ui.label(egui::RichText::new(lang.tr("The spelling check is complete for the entire sheet.")).font(theme::ui_font(13.5)));
        close(ui, open);
        return;
    };
    let word = it["word"].as_str().unwrap_or("").to_string();
    let cell = it["cell"].as_str().unwrap_or("").to_string();
    heading(ui, &lang.tr("Not in Dictionary ({cell}):").replace("{cell}", &cell));
    ui.label(egui::RichText::new(&word).font(theme::ui_bold(20.0)).color(t.danger));
    let sugg: Vec<String> =
        it["suggestions"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
    heading(ui, lang.tr("Suggestions:"));
    let mut pick = d.values.get("pick").and_then(Json::as_str).unwrap_or("").to_string();
    panel(ui, LIST_H, |ui| {
        if sugg.is_empty() {
            ui.add_space(LIST_H / 2.0 - 10.0);
            ui.vertical_centered(|ui| hint(ui, lang.tr("(No suggestions)")));
            return;
        }
        ui.spacing_mut().item_spacing.y = 0.0;
        egui::ScrollArea::vertical().id_salt("spelling.list").auto_shrink([false, false]).show(ui, |ui| {
            for s in &sugg {
                if list_row(ui, s, None, pick == *s, ui.available_width()).clicked() {
                    pick = s.clone();
                }
            }
        });
    });
    d.values.insert("pick".into(), json!(pick.clone()));
    let _ = app.session.run("selection.set", json!({"cell": cell}));
    rule(ui);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        if button(ui, lang.tr("Ignore Once"), false).clicked() {
            d.list_index += 1;
        }
        if button(ui, lang.tr("Add to Dictionary"), false).clicked() {
            let _ = app.session.run("review.addToDictionary", json!({"word": word}));
            d.list_index += 1;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_enabled_ui(!pick.is_empty(), |ui| {
                if button(ui, lang.tr("Change"), true).clicked() {
                    app.run_or_alert("review.changeSpelling", json!({"cell": cell, "word": word, "to": pick}));
                    d.list_index += 1;
                }
                if button(ui, lang.tr("Change All"), false).clicked() {
                    app.run_or_alert("review.changeSpelling", json!({"word": word, "to": pick, "all": true}));
                    d.list_index += 1;
                }
            });
        });
    });
}
