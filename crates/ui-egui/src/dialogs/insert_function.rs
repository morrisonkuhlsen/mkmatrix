use super::kit::{ROW_H, footer, hint, list_row, nav_item, nav_rail, panel, search_field};
use super::*;

use egui::pos2;

use crate::theme::Tokens;

/// The categories, as the catalog names them, with the label they translate by and their icon.
/// `All` is every function.
const CATEGORIES: [(&str, &str, Icon); 14] = [
    ("All", "All", Icon::Function),
    ("Financial", "Financial", Icon::Currency),
    ("DateTime", "Date & Time", Icon::Calendar),
    ("MathTrig", "Math & Trig", Icon::Sum),
    ("Statistical", "Statistical", Icon::ChartBar),
    ("Lookup", "Lookup & Reference", Icon::Find),
    ("Database", "Database", Icon::Table),
    ("Text", "Text", Icon::TextBox),
    ("Logical", "Logical", Icon::Check),
    ("Information", "Information", Icon::Info),
    ("Engineering", "Engineering", Icon::Settings),
    ("Compatibility", "Compatibility", Icon::Book),
    ("Web", "Web", Icon::Link),
    ("Cube", "Cube", Icon::PivotTable),
];

/// The body's height: the list and the detail card below it, so the window never jumps.
const BODY_H: f32 = 420.0;
/// The category rail's width.
const NAV_W: f32 = 240.0;
/// The detail card's height.
const DETAIL_H: f32 = 92.0;
/// The function name's column in a row.
const NAME_W: f32 = 120.0;

/// Formula Builder: categories down the left (each with how many functions match the search), a
/// search box over the matching functions, and a card with the chosen one's signature and
/// description. Arrow keys walk the list, Enter (or a double-click) starts editing `=NAME(` (or
/// appends to the formula being typed).
pub(super) fn insert_function(app: &mut SheetApp, ui: &mut egui::Ui, d: &mut Dialog, confirm: &mut bool) {
    let language = app.ui.language;
    let t = Tokens::get(ui.ctx());
    let locale = language.formula_locale();
    let list = mkmatrix_engine::cmd::formulas::function_list();
    // The names and descriptions searched are the ones shown, so in the interface language.
    let q = d.search.to_lowercase();
    let matching: Vec<&Json> = list
        .iter()
        .filter(|f| {
            q.is_empty()
                || f["name"].as_str().is_some_and(|n| locale.function_name(n).to_lowercase().contains(&q))
                || f["description"].as_str().is_some_and(|n| language.function_description(n).unwrap_or(n).to_lowercase().contains(&q))
        })
        .collect();
    // An unknown category (an old "Most Recently Used") is every function.
    let category = d.values.get("category").and_then(Json::as_str).filter(|c| CATEGORIES.iter().any(|(id, ..)| id == c)).unwrap_or("All").to_string();
    let in_category = |f: &&Json, c: &str| c == "All" || f["category"].as_str() == Some(c);
    let mut hits: Vec<&Json> = matching.iter().copied().filter(|f| in_category(f, &category)).collect();
    // A name that starts with the search comes before one that only contains it.
    if !q.is_empty() {
        hits.sort_by_key(|f| !f["name"].as_str().is_some_and(|n| locale.function_name(n).to_lowercase().starts_with(&q)));
    }
    // A new search or category starts at the top of its list.
    let key = format!("{category}\u{1}{q}");
    if d.values.get("shown").and_then(Json::as_str) != Some(key.as_str()) {
        d.values.insert("shown".into(), json!(key));
        d.list_index = 0;
    }
    let last = hits.len().saturating_sub(1);
    let (down, up) = ui.input(|i| (i.key_pressed(Key::ArrowDown), i.key_pressed(Key::ArrowUp)));
    if down {
        d.list_index = (d.list_index + 1).min(last);
    }
    if up {
        d.list_index = d.list_index.saturating_sub(1);
    }
    d.list_index = d.list_index.min(last);
    let mut picked_category = None;
    ui.horizontal_top(|ui| {
        nav_rail(ui, NAV_W, BODY_H, &t, |ui| {
            egui::ScrollArea::vertical().id_salt("categories").max_height(BODY_H).auto_shrink([false, true]).show(ui, |ui| {
                for (id, label, icon) in CATEGORIES {
                    let count = matching.iter().filter(|f| in_category(f, id)).count();
                    if count == 0 && id != "All" && id != category {
                        continue;
                    }
                    if nav_item(ui, icon, language.tr(label), category == id, Some(count), &t).clicked() {
                        picked_category = Some(id);
                    }
                }
            });
        });
        ui.vertical(|ui| {
            ui.set_min_height(BODY_H);
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 8.0;
            let width = ui.available_width();
            search_field(ui, &mut d.search, language.tr("e.g. lookup, average, date"), language.tr("Search"), width).request_focus();
            let list_h = BODY_H - 32.0 - DETAIL_H - 16.0;
            panel(ui, list_h, |ui| {
                if hits.is_empty() {
                    ui.add_space(list_h / 2.0 - 10.0);
                    ui.vertical_centered(|ui| hint(ui, language.tr("No functions match your search.")));
                    return;
                }
                ui.spacing_mut().item_spacing.y = 0.0;
                egui::ScrollArea::vertical().id_salt(("functions", key.as_str())).auto_shrink([false, false]).show_rows(
                    ui,
                    ROW_H,
                    hits.len(),
                    |ui, rows| {
                        for i in rows {
                            let Some(f) = hits.get(i) else { continue };
                            let name = locale.function_name(f["name"].as_str().unwrap_or(""));
                            let about = f["description"].as_str().map(|n| language.function_description(n).unwrap_or(n));
                            let r = list_row(ui, name, about, i == d.list_index, NAME_W);
                            if (down || up) && i == d.list_index {
                                r.scroll_to_me(None);
                            }
                            if r.clicked() {
                                d.list_index = i;
                            }
                            if r.double_clicked() {
                                d.list_index = i;
                                *confirm = true;
                            }
                        }
                    },
                );
            });
            detail(ui, hits.get(d.list_index).copied(), locale, language, &t);
        });
    });
    if let Some(c) = picked_category {
        d.values.insert("category".into(), json!(c));
    }
    let mut open = true;
    let count = language.tr("{count} functions").replace("{count}", &hits.len().to_string());
    footer(ui, confirm, &mut open, |ui| hint(ui, &count));
    if *confirm && let Some(n) = hits.get(d.list_index).and_then(|f| f["name"].as_str()) {
        let n = app.session.active().map(|doc| crate::formula_locale::completion_name(n, locale, &doc.wb, doc.wb.active_sheet())).unwrap_or(n);
        let cur = app.editor.as_ref().map(|e| e.text.clone());
        let text = match cur {
            Some(t) if t.starts_with('=') => format!("{t}{n}("),
            _ => format!("={n}("),
        };
        app.begin_edit(Some(text), false);
    }
    if !open {
        *confirm = true;
    }
}

/// The chosen function's card: its category, its signature with the name in the accent, and what
/// it does.
fn detail(ui: &mut egui::Ui, f: Option<&Json>, locale: mkmatrix_engine::formula::FormulaLocale, language: crate::i18n::Language, t: &Tokens) {
    panel(ui, DETAIL_H, |ui| {
        let Some(f) = f else { return };
        let name = f["name"].as_str().unwrap_or("");
        let category = language.tr(function_category_label(f["category"].as_str().unwrap_or("")));
        let signature = crate::formula_locale::signature(name, locale, language).unwrap_or_default();
        let shown = locale.function_name(name).to_string();
        let rect = ui.max_rect().shrink2(vec2(12.0, 10.0));
        let p = ui.painter();
        let top = p.text(rect.left_top(), egui::Align2::LEFT_TOP, category.to_uppercase(), theme::ui_bold(10.5), t.text_dim);
        // The name in the accent, the arguments after it in the text colour.
        let args = signature.strip_prefix(shown.as_str()).unwrap_or(&signature);
        let at = pos2(rect.left(), top.bottom() + 6.0);
        let name_rect = p.text(at, egui::Align2::LEFT_TOP, &shown, egui::FontId::monospace(14.0), t.accent);
        p.with_clip_rect(rect).text(pos2(name_rect.right(), at.y), egui::Align2::LEFT_TOP, args, egui::FontId::monospace(14.0), t.text);
        let about = crate::formula_locale::description(name, locale, language).unwrap_or_default();
        let galley = p.layout(about, theme::ui_font(12.5), t.text_dim, rect.width());
        p.with_clip_rect(rect).galley(pos2(rect.left(), name_rect.bottom() + 6.0), galley, t.text_dim);
    });
}

/// Excel's name for a function category, given the catalog's id (`MathTrig`).
fn function_category_label(id: &str) -> &str {
    match id {
        "DateTime" => "Date & Time",
        "Lookup" => "Lookup & Reference",
        "MathTrig" => "Math & Trig",
        "AddIn" => "Add-in and Automation",
        // Financial, Logical, Text, Statistical, Engineering, Database, Information, …
        other => other,
    }
}
