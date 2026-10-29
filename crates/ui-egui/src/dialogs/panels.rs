//! The report dialogs, in Format Cells' style: Workbook Statistics, Accessibility, Error
//! Checking, Evaluate Formula, the Action Journal and Agent Control. Each reads what its command
//! returned (`d.result`) and lays it out as figures, framed lists or steps, with Close in the
//! footer; a list row that names a cell goes there.

use super::kit::{ROW_H, button, heading, hint, list_row, panel, rule, search_field};
use super::*;

use egui::pos2;

use crate::theme::Tokens;

/// A framed list's height in these dialogs.
const LIST_H: f32 = 10.0 * ROW_H + 2.0;

/// The footer: a rule, then `left` (a count) and Close.
fn close_footer(ui: &mut egui::Ui, open: &mut bool, left: &str) {
    let lang = crate::i18n::current(ui.ctx());
    rule(ui);
    ui.horizontal(|ui| {
        hint(ui, left);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if button(ui, lang.tr("Close"), true).clicked() {
                *open = false;
            }
        });
    });
}

/// An empty framed list with its message in the middle.
fn empty_panel(ui: &mut egui::Ui, text: &str) {
    panel(ui, LIST_H, |ui| {
        ui.add_space(LIST_H / 2.0 - 10.0);
        ui.vertical_centered(|ui| hint(ui, text));
    });
}

/// Selects `cell` on `sheet` (the grid scrolls to it), leaving the dialog open.
fn go_to(app: &mut SheetApp, sheet: &str, cell: &str) {
    let reference = if sheet.is_empty() { cell.to_string() } else { format!("{}!{cell}", mkmatrix_engine::formula::quote_sheet(sheet)) };
    app.run_or_alert("edit.goTo", json!({"reference": reference}));
}

/// Review › Workbook Statistics: the active sheet's and the workbook's figures, each in a card.
pub(super) fn statistics(ui: &mut egui::Ui, d: &Dialog, open: &mut bool) {
    let lang = crate::i18n::current(ui.ctx());
    let t = Tokens::get(ui.ctx());
    let r = d.result.clone().unwrap_or(Json::Null);
    let n = |v: &Json, k: &str| v.get(k).and_then(Json::as_u64).map(crate::credits::group).unwrap_or_else(|| "0".into());
    let sheet = &r["sheet"];
    let book = &r["workbook"];
    let sheet_name = sheet.get("name").and_then(Json::as_str).unwrap_or("");
    let figures = |ui: &mut egui::Ui, items: &[(String, &str)]| {
        let gap = 8.0;
        let w = (ui.available_width() - 2.0 * gap) / 3.0;
        for row in items.chunks(3) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for (value, what) in row {
                    let (r, _) = ui.allocate_exact_size(vec2(w, 58.0), Sense::hover());
                    let p = ui.painter();
                    p.rect_filled(r, 4.0, t.input_bg);
                    p.rect_filled(egui::Rect::from_min_size(r.min, vec2(3.0, r.height())), 0.0, t.accent);
                    p.text(pos2(r.left() + 14.0, r.top() + 10.0), egui::Align2::LEFT_TOP, value, theme::ui_bold(18.0), t.text);
                    p.text(pos2(r.left() + 14.0, r.bottom() - 10.0), egui::Align2::LEFT_BOTTOM, lang.tr(what), theme::ui_font(11.5), t.text_dim);
                }
            });
        }
    };
    ui.spacing_mut().item_spacing.y = 8.0;
    heading(ui, &lang.tr("Current sheet: {sheet}").replace("{sheet}", sheet_name));
    let end = sheet.get("endOfSheet").and_then(Json::as_str).unwrap_or("—").to_string();
    figures(
        ui,
        &[
            (end, "End of sheet"),
            (n(sheet, "cellsWithData"), "Cells with data"),
            (n(sheet, "formulas"), "Formulas"),
            (n(sheet, "tables"), "Tables"),
            (n(sheet, "charts"), "Charts"),
            (n(sheet, "comments"), "Comments"),
        ],
    );
    ui.add_space(6.0);
    heading(ui, lang.tr("Workbook"));
    figures(
        ui,
        &[
            (n(book, "sheets"), "Sheets"),
            (n(book, "cellsWithData"), "Cells with data"),
            (n(book, "names"), "Names"),
            (n(book, "tables"), "Tables"),
            (n(book, "charts"), "Charts"),
        ],
    );
    close_footer(ui, open, "");
}

/// Review › Check Accessibility: each issue, the sheet it is on and what it concerns.
pub(super) fn accessibility(app: &mut SheetApp, ui: &mut egui::Ui, d: &mut Dialog, open: &mut bool) {
    let lang = app.ui.language;
    let issues: Vec<Json> = d.result.as_ref().and_then(Json::as_array).cloned().unwrap_or_default();
    ui.spacing_mut().item_spacing.y = 8.0;
    heading(ui, lang.tr("Inspection Results"));
    if issues.is_empty() {
        empty_panel(ui, lang.tr("No accessibility issues found."));
    } else {
        let mut go = None;
        panel(ui, LIST_H, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            egui::ScrollArea::vertical().id_salt("accessibility.list").auto_shrink([false, false]).show(ui, |ui| {
                for (i, it) in issues.iter().enumerate() {
                    let sheet = it.get("sheet").and_then(Json::as_str).unwrap_or("");
                    let what = match (it.get("object").and_then(Json::as_str), it.get("count").and_then(Json::as_u64)) {
                        (Some(o), _) => format!("{sheet} · {o}"),
                        (None, Some(c)) => format!("{sheet} · {c}"),
                        _ => sheet.to_string(),
                    };
                    let issue = it.get("issue").and_then(Json::as_str).unwrap_or("");
                    if list_row(ui, lang.tr(issue), Some(&what), i == d.list_index, 220.0).clicked() {
                        d.list_index = i;
                        go = Some(sheet.to_string());
                    }
                }
            });
        });
        if let Some(sheet) = go {
            go_to(app, &sheet, "A1");
        }
    }
    let count = lang.tr(if issues.len() == 1 { "{count} issue" } else { "{count} issues" }).replace("{count}", &issues.len().to_string());
    close_footer(ui, open, &count);
}

/// Formulas › Error Checking: the cells whose formula gives an error, and the circular
/// references; a click goes to the cell.
pub(super) fn error_checking(app: &mut SheetApp, ui: &mut egui::Ui, d: &mut Dialog, open: &mut bool) {
    let lang = app.ui.language;
    let r = d.result.clone().unwrap_or(Json::Null);
    let errors: Vec<Json> = r["errors"].as_array().cloned().unwrap_or_default();
    let circular: Vec<String> =
        r["circular"].as_array().map(|a| a.iter().filter_map(|c| c.as_str().map(str::to_string)).collect()).unwrap_or_default();
    // (sheet, cell, primary, secondary)
    let mut rows: Vec<(String, String, String, String)> = errors
        .iter()
        .map(|e| {
            let s = |k: &str| e.get(k).and_then(Json::as_str).unwrap_or("").to_string();
            let sheet = s("sheet");
            let cell = s("cell");
            (sheet.clone(), cell.clone(), format!("{sheet}!{cell}  {}", s("error")), s("formula"))
        })
        .collect();
    for c in &circular {
        let (sheet, cell) = c.rsplit_once('!').map(|(s, c)| (mkmatrix_engine::core::unquote_sheet(s), c.to_string())).unwrap_or_default();
        rows.push((sheet, cell, c.clone(), lang.tr("Circular reference").to_string()));
    }
    ui.spacing_mut().item_spacing.y = 8.0;
    heading(ui, lang.tr("Cells with errors"));
    if rows.is_empty() {
        empty_panel(ui, lang.tr("The error check is complete for the entire sheet."));
    } else {
        let mut go = None;
        panel(ui, LIST_H, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            egui::ScrollArea::vertical().id_salt("errorChecking.list").auto_shrink([false, false]).show_rows(ui, ROW_H, rows.len(), |ui, range| {
                for i in range {
                    let Some((sheet, cell, primary, secondary)) = rows.get(i) else { continue };
                    let r = list_row(ui, primary, Some(secondary), i == d.list_index, 200.0);
                    if r.clicked() {
                        d.list_index = i;
                        go = Some((sheet.clone(), cell.clone()));
                    }
                    r.on_hover_text(lang.tr("Go to the cell"));
                }
            });
        });
        if let Some((sheet, cell)) = go {
            go_to(app, &sheet, &cell);
        }
    }
    let count = lang.tr(if rows.len() == 1 { "{count} error" } else { "{count} errors" }).replace("{count}", &rows.len().to_string());
    close_footer(ui, open, &count);
}

/// Formulas › Evaluate Formula: the formula, then each step of its evaluation (what was
/// evaluated and its value), the result last.
pub(super) fn evaluate_formula(ui: &mut egui::Ui, d: &Dialog, open: &mut bool) {
    let lang = crate::i18n::current(ui.ctx());
    let t = Tokens::get(ui.ctx());
    let r = d.result.clone().unwrap_or(Json::Null);
    // Whole numbers without a trailing `.0`, as a cell shows them in General.
    let value = |v: &Json| match v {
        Json::String(s) => format!("\"{s}\""),
        Json::Null => String::new(),
        Json::Number(n) => match n.as_f64() {
            Some(f) if f.is_finite() && f.fract() == 0.0 && f.abs() < 1e15 => format!("{}", f as i64),
            _ => n.to_string(),
        },
        other => other.to_string(),
    };
    ui.spacing_mut().item_spacing.y = 8.0;
    let Some(formula) = r.get("formula").and_then(Json::as_str) else {
        empty_panel(ui, lang.tr("The cell has no formula."));
        close_footer(ui, open, "");
        return;
    };
    heading(ui, lang.tr("Evaluation:"));
    egui::Frame::new().fill(t.input_bg).stroke(Stroke::new(1.0, t.input_border)).inner_margin(egui::Margin::symmetric(10, 8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.add(egui::Label::new(egui::RichText::new(formula).font(egui::FontId::monospace(13.0)).color(t.text)).wrap());
    });
    let steps: Vec<Json> = r["steps"].as_array().cloned().unwrap_or_default();
    heading(ui, lang.tr("Steps"));
    panel(ui, LIST_H, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        egui::ScrollArea::vertical().id_salt("evaluateFormula.steps").auto_shrink([false, false]).show(ui, |ui| {
            for (i, s) in steps.iter().enumerate() {
                let expr = s.get("expression").and_then(Json::as_str).unwrap_or("");
                let v = value(s.get("value").unwrap_or(&Json::Null));
                let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
                let p = ui.painter();
                p.text(pos2(row.left() + 10.0, row.center().y), egui::Align2::LEFT_CENTER, format!("{}.", i + 1), theme::ui_font(12.0), t.text_dim);
                let val = p.text(pos2(row.right() - 10.0, row.center().y), egui::Align2::RIGHT_CENTER, &v, theme::ui_bold(13.0), t.accent);
                let clip = egui::Rect::from_min_max(pos2(row.left() + 34.0, row.top()), pos2(val.left() - 12.0, row.bottom()));
                p.with_clip_rect(clip).text(
                    pos2(clip.left(), row.center().y),
                    egui::Align2::LEFT_CENTER,
                    expr,
                    egui::FontId::monospace(12.5),
                    t.text,
                );
            }
        });
    });
    let result = value(r.get("result").unwrap_or(&Json::Null));
    close_footer(ui, open, &lang.tr("Result: {value}").replace("{value}", &result));
}

/// The Action Journal: every command run so far, newest first, filtered by a search box.
pub(super) fn journal(app: &SheetApp, ui: &mut egui::Ui, d: &mut Dialog, open: &mut bool) {
    let lang = app.ui.language;
    ui.spacing_mut().item_spacing.y = 8.0;
    let w = ui.available_width();
    search_field(ui, &mut d.search, lang.tr("Filter by command or parameter"), lang.tr("Action Journal"), w);
    let q = d.search.to_lowercase();
    let rows: Vec<(String, String)> = app
        .session
        .journal
        .iter()
        .rev()
        .map(|(id, p)| (id.clone(), if p.as_object().is_some_and(|o| o.is_empty()) { String::new() } else { p.to_string() }))
        .filter(|(id, p)| q.is_empty() || id.to_lowercase().contains(&q) || p.to_lowercase().contains(&q))
        .collect();
    if rows.is_empty() {
        empty_panel(ui, lang.tr("No actions yet."));
    } else {
        panel(ui, LIST_H, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            egui::ScrollArea::vertical().id_salt("journal.list").auto_shrink([false, false]).show_rows(ui, ROW_H, rows.len(), |ui, range| {
                for i in range {
                    let Some((id, p)) = rows.get(i) else { continue };
                    if list_row(ui, id, Some(p), i == d.list_index, 190.0).clicked() {
                        d.list_index = i;
                    }
                }
            });
        });
    }
    let count = lang.tr(if rows.len() == 1 { "{count} action" } else { "{count} actions" }).replace("{count}", &rows.len().to_string());
    close_footer(ui, open, &count);
}

/// Agent Control: how scripts and agents drive MK Matrix, the commands in a code card.
pub(super) fn agents(ui: &mut egui::Ui, open: &mut bool) {
    let lang = crate::i18n::current(ui.ctx());
    let t = Tokens::get(ui.ctx());
    ui.spacing_mut().item_spacing.y = 8.0;
    ui.add(
        egui::Label::new(
            egui::RichText::new(lang.tr("MK Matrix is fully drivable by agents. Every menu item, button and gesture is a command:"))
                .font(theme::ui_font(13.5))
                .color(t.text),
        )
        .wrap(),
    );
    egui::Frame::new().fill(t.input_bg).stroke(Stroke::new(1.0, t.input_border)).inner_margin(egui::Margin::symmetric(12, 10)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 6.0;
        for (cmd, what) in [
            ("mkmatrix --control 7979", "JSON-lines control channel"),
            ("mkmatrix-cli mcp --connect 7979", "MCP bridged to this window"),
            ("mkmatrix-cli mcp", "Headless MCP server"),
        ] {
            ui.label(egui::RichText::new(cmd).font(egui::FontId::monospace(13.0)).color(t.text));
            ui.label(egui::RichText::new(lang.tr(what)).font(theme::ui_font(11.5)).color(t.text_dim));
        }
    });
    let count = lang.tr("{count} engine commands").replace("{count}", &mkmatrix_engine::command_specs().len().to_string());
    hint(ui, lang.tr("See docs/mcp.md and docs/control-protocol.md."));
    close_footer(ui, open, &count);
}
