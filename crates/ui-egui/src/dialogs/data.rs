#[cfg(test)]
use super::show::build_params;
use super::show::{form_fields, ok_cancel};
use super::*;

/// Initial values of the Data Validation form: the rule stored on the active cell, so OK without
/// edits keeps it; a plain list rule when the cell has none.
pub(super) fn validation_defaults(app: &SheetApp) -> Json {
    let mut v = json!({"type": "list", "operator": "between", "ignoreBlank": true, "dropdown": true, "errorStyle": "stop"});
    let Some(doc) = app.session.active() else { return v };
    let Some(dv) = doc.wb.active().and_then(|s| s.validations.iter().find(|d| d.ranges.iter().any(|r| r.contains(doc.selection.active)))) else {
        return v;
    };
    // The enums serialize as their variant names; the command takes them in camelCase.
    let camel = |x: Json| {
        let name = x.as_str().unwrap_or_default();
        let mut chars = name.chars();
        chars.next().map(|c| c.to_lowercase().chain(chars).collect::<String>()).unwrap_or_default()
    };
    v["type"] = json!(camel(json!(dv.kind)));
    v["operator"] = json!(camel(json!(dv.op)));
    v["errorStyle"] = json!(camel(json!(dv.error_style)));
    v["formula1"] = json!(dv.f1);
    if let Some(f2) = &dv.f2 {
        v["formula2"] = json!(f2);
    }
    v["ignoreBlank"] = json!(dv.allow_blank);
    v["dropdown"] = json!(dv.in_cell_dropdown);
    v["inputTitle"] = json!(dv.input_title);
    v["inputMessage"] = json!(dv.input_message);
    v["errorTitle"] = json!(dv.error_title);
    v["errorMessage"] = json!(dv.error_message);
    v
}

/// The range Remove Duplicates starts with. A block selection is used as given; with only the
/// cursor placed, the table's data rows or the current region, as the command picks when it gets
/// no range (a single-cell range would remove nothing).
pub(super) fn remove_duplicates_range(app: &SheetApp, sel: &str) -> String {
    let Some(d) = app.session.active() else { return sel.to_string() };
    let Some(sh) = d.wb.active() else { return sel.to_string() };
    if !d.selection.current().is_single() {
        return sel.to_string();
    }
    let range = match sh.table_at(d.selection.active) {
        Some(t) => t.data_range().map(|dr| mkmatrix_engine::core::RangeRef::new(t.range.start, dr.end)).unwrap_or(t.range),
        None => mkmatrix_engine::selection::current_region(sh, d.selection.active),
    };
    range.a1()
}

pub(super) fn abs(r: &mkmatrix_engine::core::RangeRef) -> String {
    let a = |c: mkmatrix_engine::core::CellRef| format!("${}${}", mkmatrix_engine::core::col_to_letters(c.col), c.row + 1);
    if r.is_single() { a(r.start) } else { format!("{}:{}", a(r.start), a(r.end)) }
}

/// The criterion controls in Excel's Settings tab depend on both Allow and Data.
/// `formula1` is the sole source for a list or custom rule; `formula2` is a
/// second bound only for Between and Not between.
pub(super) fn data_validation_fields(kind: &str, operator: &str) -> Vec<Field> {
    use Field::*;
    let mut fields = vec![Choice {
        key: "type",
        label: "Allow",
        options: vec![
            ("any", "Any value"),
            ("whole", "Whole number"),
            ("decimal", "Decimal"),
            ("list", "List"),
            ("date", "Date"),
            ("time", "Time"),
            ("textLength", "Text length"),
            ("custom", "Custom"),
        ],
    }];
    let two_bounds = matches!(operator, "between" | "notBetween");
    match kind {
        "list" => {
            fields.push(Text { key: "formula1", label: "Source:" });
            fields.push(Check { key: "ignoreBlank", label: "Ignore blank" });
            fields.push(Check { key: "dropdown", label: "In-cell dropdown" });
        }
        "custom" => {
            fields.push(Text { key: "formula1", label: "Formula:" });
            fields.push(Check { key: "ignoreBlank", label: "Ignore blank" });
        }
        "whole" | "decimal" | "date" | "time" | "textLength" => {
            fields.push(Choice {
                key: "operator",
                label: "Data",
                options: vec![
                    ("between", "between"),
                    ("notBetween", "not between"),
                    ("equal", "equal to"),
                    ("notEqual", "not equal to"),
                    ("greater", "greater than"),
                    ("less", "less than"),
                    ("greaterOrEqual", "greater than or equal to"),
                    ("lessOrEqual", "less than or equal to"),
                ],
            });
            let lower = matches!(operator, "greater" | "greaterOrEqual");
            let (first, second) = match kind {
                "date" => (
                    if two_bounds || lower {
                        "Start date:"
                    } else if matches!(operator, "less" | "lessOrEqual") {
                        "End date:"
                    } else {
                        "Date:"
                    },
                    "End date:",
                ),
                "time" => (
                    if two_bounds || lower {
                        "Start time:"
                    } else if matches!(operator, "less" | "lessOrEqual") {
                        "End time:"
                    } else {
                        "Time:"
                    },
                    "End time:",
                ),
                _ => (
                    if two_bounds || lower {
                        "Minimum:"
                    } else if matches!(operator, "less" | "lessOrEqual") {
                        "Maximum:"
                    } else {
                        "Value:"
                    },
                    "Maximum:",
                ),
            };
            fields.push(Text { key: "formula1", label: first });
            if two_bounds {
                fields.push(Text { key: "formula2", label: second });
            }
            fields.push(Check { key: "ignoreBlank", label: "Ignore blank" });
        }
        _ => {}
    }
    fields.push(Text { key: "inputMessage", label: "Input message:" });
    fields.push(Choice {
        key: "errorStyle",
        label: "Error style",
        options: vec![("stop", "Stop"), ("warning", "Warning"), ("information", "Information")],
    });
    fields.push(Text { key: "errorMessage", label: "Error message:" });
    fields
}

/// Data Validation, in Format Cells' style: Settings, Input Message and Error Alert down the left
/// (as Excel's tabs), the chosen section's fields on the right, and the footer. Every field stays
/// in `d.fields`, so OK sends all of them whichever section is showing.
pub(super) fn data_validation_form(ui: &mut egui::Ui, d: &mut Dialog, confirm: &mut bool, open: &mut bool) {
    use super::kit::{nav_item, nav_rail, nav_width, rule};
    const SECTIONS: [(&str, Icon); 3] = [("Settings", Icon::Validation), ("Input Message", Icon::Comment), ("Error Alert", Icon::Info)];
    let lang = crate::i18n::current(ui.ctx());
    let t = crate::theme::Tokens::get(ui.ctx());
    let kind = d.values.get("type").and_then(Json::as_str).unwrap_or("any");
    let operator = d.values.get("operator").and_then(Json::as_str).unwrap_or("between");
    let all = data_validation_fields(kind, operator);
    if !SECTIONS.iter().any(|(s, _)| *s == d.tab) {
        d.tab = "Settings".into();
    }
    let section = |f: &Field| match f {
        Field::Text { key: "inputMessage", .. } => "Input Message",
        Field::Text { key: "errorMessage", .. } | Field::Choice { key: "errorStyle", .. } => "Error Alert",
        _ => "Settings",
    };
    ui.horizontal_top(|ui| {
        let names: Vec<&str> = SECTIONS.iter().map(|(s, _)| lang.tr(s)).collect();
        nav_rail(ui, nav_width(ui, &names, 150.0), 300.0, &t, |ui| {
            for (tab, icon) in SECTIONS {
                if nav_item(ui, icon, lang.tr(tab), d.tab == tab, None, &t).clicked() {
                    d.tab = tab.to_string();
                }
            }
        });
        ui.vertical(|ui| {
            ui.set_min_height(300.0);
            d.fields = all.iter().filter(|f| section(f) == d.tab).cloned().collect();
            form_fields(ui, d);
        });
    });
    d.fields = all;
    rule(ui);
    ok_cancel(ui, confirm, open);
}

/// Data › Advanced filter, in Format Cells' style: the action as round options, the ranges, and the
/// footer under a rule.
pub(super) fn advanced_filter_form(ui: &mut egui::Ui, d: &mut Dialog, confirm: &mut bool, open: &mut bool) {
    use Field::*;
    let copy = d.values.get("action").and_then(Json::as_str) == Some("copy");
    d.fields = vec![
        Choice {
            key: "action",
            label: "Action",
            options: vec![("inPlace", "Show matching rows in place"), ("copy", "Copy matching rows elsewhere")],
        },
        Text { key: "range", label: "List range:" },
        Text { key: "criteria", label: "Criteria range (optional):" },
    ];
    if copy {
        d.fields.push(Text { key: "copyTo", label: "Copy to:" });
    }
    d.fields.push(Check { key: "unique", label: "Unique rows only" });
    form_fields(ui, d);
    super::kit::rule(ui);
    // Constrain the button row: the shared right-to-left layout otherwise
    // consumes the window's remaining height instead of fitting the buttons.
    ui.allocate_ui(vec2(ui.available_width(), ui.spacing().interact_size.y + 8.0), |ui| ok_cancel(ui, confirm, open));
}

/// Splits `text` as Text to Columns will with these delimiters: on any of them, runs of them
/// counting as one when `consecutive` is on.
fn split_preview(text: &str, delims: &[char], consecutive: bool) -> Vec<String> {
    if delims.is_empty() {
        return vec![text.to_string()];
    }
    let parts = text.split(|c| delims.contains(&c));
    if consecutive { parts.filter(|p| !p.is_empty()).map(str::to_string).collect() } else { parts.map(str::to_string).collect() }
}

/// Data › Text to Columns, in Format Cells' style: the delimiters in two columns with Other and
/// its character on one line, the destination, and a preview of the first rows split as OK will
/// split them. The values are the dialog's `fields`, so the control channel sets the same.
pub(super) fn text_to_columns(app: &SheetApp, ui: &mut egui::Ui, d: &mut Dialog, confirm: &mut bool, open: &mut bool) {
    use super::kit::{heading, panel, rule};
    let lang = app.ui.language;
    let t = crate::theme::Tokens::get(ui.ctx());
    let flag = |d: &Dialog, k: &str| d.values.get(k).and_then(Json::as_bool).unwrap_or(false);
    let text = |d: &Dialog, k: &str| d.values.get(k).and_then(Json::as_str).unwrap_or("").to_string();
    let check = |ui: &mut egui::Ui, d: &mut Dialog, key: &str, label: &str| {
        let mut on = flag(d, key);
        if ui.checkbox(&mut on, lang.tr(label)).changed() {
            d.values.insert(key.into(), json!(on));
        }
    };
    let col_w = (ui.available_width() - 12.0) / 2.0;
    ui.spacing_mut().item_spacing.y = 6.0;
    heading(ui, lang.tr("Delimiters"));
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        for column in [[("tab", "Tab"), ("semicolon", "Semicolon")], [("comma", "Comma"), ("space", "Space")]] {
            ui.vertical(|ui| {
                ui.set_width(col_w);
                for (key, label) in column {
                    check(ui, d, key, label);
                }
            });
        }
    });
    ui.horizontal(|ui| {
        let id = ui.label(egui::RichText::new(lang.tr("Other:")).font(theme::ui_font(13.0))).id;
        let mut other = text(d, "other");
        let r = ui.add(egui::TextEdit::singleline(&mut other).char_limit(1).desired_width(32.0)).labelled_by(id);
        if r.changed() {
            d.values.insert("other".into(), json!(other));
        }
    });
    check(ui, d, "treatConsecutive", "Treat consecutive delimiters as one");
    ui.add_space(4.0);
    heading(ui, lang.tr("Destination:"));
    let mut dest = text(d, "destination");
    if ui.add(egui::TextEdit::singleline(&mut dest).desired_width(ui.available_width())).accessible_name(lang.tr("Destination:")).changed() {
        d.values.insert("destination".into(), json!(dest));
    }

    // The preview: the selection's first rows, split.
    ui.add_space(4.0);
    heading(ui, lang.tr("Data preview"));
    let mut delims: Vec<char> =
        [("tab", '\t'), ("semicolon", ';'), ("comma", ','), ("space", ' ')].into_iter().filter(|(k, _)| flag(d, k)).map(|(_, c)| c).collect();
    delims.extend(text(d, "other").chars().next());
    let consecutive = flag(d, "treatConsecutive");
    let rows: Vec<Vec<String>> = app
        .session
        .active()
        .and_then(|doc| {
            let sh = doc.wb.active()?;
            let r = doc.selection.current();
            Some(
                (r.start.row..=r.end.row.min(r.start.row.saturating_add(5)))
                    .map(|row| {
                        split_preview(
                            &mkmatrix_engine::display::cell_text(&doc.wb, sh, mkmatrix_engine::core::CellRef::new(row, r.start.col)),
                            &delims,
                            consecutive,
                        )
                    })
                    .collect(),
            )
        })
        .unwrap_or_default();
    let preview_h = 6.0 * 22.0 + 10.0;
    panel(ui, preview_h, |ui| {
        egui::ScrollArea::both().id_salt("textToColumns.preview").auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("textToColumns.grid").spacing([0.0, 0.0]).min_row_height(22.0).show(ui, |ui| {
                for row in &rows {
                    for cell in row.iter().take(16) {
                        let (r, _) = ui.allocate_exact_size(vec2(84.0, 22.0), egui::Sense::hover());
                        let p = ui.painter();
                        p.rect_stroke(r, 0.0, egui::Stroke::new(1.0, t.separator), egui::StrokeKind::Inside);
                        p.with_clip_rect(r.shrink(1.0)).text(
                            egui::pos2(r.left() + 6.0, r.center().y),
                            egui::Align2::LEFT_CENTER,
                            cell,
                            theme::ui_font(12.5),
                            t.text,
                        );
                    }
                    ui.end_row();
                }
            });
        });
    });
    rule(ui);
    ok_cancel(ui, confirm, open);
}

#[cfg(test)]
mod text_to_columns_tests {
    use super::split_preview;

    #[test]
    fn the_preview_splits_as_the_command_does() {
        assert_eq!(split_preview("a;b,,c", &[';', ','], false), ["a", "b", "", "c"]);
        assert_eq!(split_preview("a;b,,c", &[';', ','], true), ["a", "b", "c"]);
        assert_eq!(split_preview("a b", &[], false), ["a b"]);
    }
}

/// Sort, in Format Cells' style: Add Level and My list has headers above a table of levels (Sort
/// by / Then by, the column, what to sort on and the order as two joined buttons, a cross to drop
/// the level), then the footer; OK runs `data.sort`.
pub(super) fn sort(app: &mut SheetApp, ui: &mut egui::Ui, d: &mut Dialog, confirm: &mut bool) {
    use super::kit::{button, footer, rule, segments};
    let lang = app.ui.language;
    let t = crate::theme::Tokens::get(ui.ctx());
    let mut header = d.values.get("header").and_then(Json::as_bool).unwrap_or(true);
    let mut levels: Vec<Json> = d.values.get("levels").and_then(Json::as_array).cloned().unwrap_or_default();
    // Columns of the region around the active cell, labelled with their header (or first value).
    let columns: Vec<(String, String)> = app
        .session
        .active()
        .and_then(|d| {
            let sh = d.wb.active()?;
            let region = mkmatrix_engine::selection::current_region(sh, d.selection.active);
            Some(
                (region.start.col..=region.end.col)
                    .map(|c| {
                        let letter = mkmatrix_engine::core::col_to_letters(c);
                        // The region starts at the header row when one is present, so the
                        // label always comes from start.row: header text with headers on,
                        // first data value with them off.
                        let label_row = region.start.row;
                        let text = mkmatrix_engine::display::cell_text(&d.wb, sh, mkmatrix_engine::core::CellRef::new(label_row, c));
                        let text: String = text.chars().take(24).collect();
                        (letter.clone(), if text.is_empty() { letter } else { format!("{letter}: {text}") })
                    })
                    .collect(),
            )
        })
        .unwrap_or_default();

    let mut add = false;
    ui.horizontal(|ui| {
        add = button(ui, lang.tr("+ Add Level"), false).clicked();
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.checkbox(&mut header, lang.tr("My list has headers"));
        });
    });
    d.values.insert("header".into(), json!(header));
    rule(ui);

    let mut remove = None;
    // The only level can't be removed.
    let removable = levels.len() > 1;
    egui::Grid::new("sort_levels").num_columns(5).spacing([10.0, 8.0]).min_col_width(0.0).min_row_height(28.0).show(ui, |ui| {
        // Muted headings, as `kit::heading` draws them (which can't sit in a grid).
        let heading = |ui: &mut egui::Ui, text: &str| ui.label(egui::RichText::new(text).font(theme::ui_bold(11.5)).color(t.text_dim));
        ui.label("");
        heading(ui, lang.tr("Column"));
        heading(ui, lang.tr("Sort On"));
        heading(ui, lang.tr("Order"));
        ui.label("");
        ui.end_row();
        for (i, lv) in levels.iter_mut().enumerate() {
            let label = ui.add_sized(
                vec2(64.0, 28.0),
                egui::Label::new(
                    egui::RichText::new(lang.tr(if i == 0 { "Sort by" } else { "Then by" })).font(theme::ui_font(13.0)).color(t.text_dim),
                ),
            );
            let mut col = lv["column"].as_str().unwrap_or("A").to_string();
            if columns.is_empty() {
                ui.add(egui::TextEdit::singleline(&mut col).desired_width(170.0)).labelled_by(label.id);
            } else {
                let shown = columns.iter().find(|(l, _)| *l == col).map(|(_, text)| text.clone()).unwrap_or_else(|| col.clone());
                craft_ui::ComboBox::from_id_salt(("sort_col", i))
                    .selected_text(shown)
                    .width(170.0)
                    .show_ui(ui, |ui| {
                        for (letter, label) in &columns {
                            ui.selectable_value(&mut col, letter.clone(), label.as_str());
                        }
                    })
                    .response
                    .labelled_by(label.id);
            }
            lv["column"] = json!(col.to_ascii_uppercase());
            let mut by = lv["by"].as_str().unwrap_or("values").to_string();
            let ons = [("values", "Cell Values"), ("cellColor", "Cell Color"), ("fontColor", "Font Color")];
            craft_ui::ComboBox::from_id_salt(("by", i))
                .selected_text(lang.tr(ons.iter().find(|(v, _)| *v == by).map_or("Cell Values", |(_, text)| text)))
                .width(120.0)
                .show_ui(ui, |ui| {
                    for (v, text) in ons {
                        ui.selectable_value(&mut by, v.into(), lang.tr(text));
                    }
                })
                .response
                .accessible_name(lang.tr("Sort On"));
            lv["by"] = json!(by);
            let order = if lv["order"].as_str() == Some("desc") { "desc" } else { "asc" };
            let picked = ui.push_id(("order", i), |ui| segments(ui, order, &[("asc", lang.tr("A to Z")), ("desc", lang.tr("Z to A"))], 64.0)).inner;
            lv["order"] = json!(picked.unwrap_or(order));
            if ui
                .add_enabled_ui(removable, |ui| crate::widgets::icon_button(ui, Icon::Close, t.text_dim, "Remove level", vec2(18.0, 18.0)))
                .inner
                .clicked()
            {
                remove = Some(i);
            }
            ui.end_row();
        }
    });
    if let Some(i) = remove
        && levels.len() > 1
    {
        levels.remove(i);
    }
    if add {
        let used: Vec<String> = levels.iter().filter_map(|l| l.get("column").and_then(Json::as_str).map(str::to_string)).collect();
        let next = columns
            .iter()
            .map(|(l, _)| l)
            .find(|l| !used.contains(l))
            .cloned()
            .or_else(|| columns.last().map(|(l, _)| l.clone()))
            .unwrap_or_else(|| "B".into());
        levels.push(json!({"column": next, "order": "asc"}));
    }
    d.values.insert("levels".into(), Json::Array(levels.clone()));
    let mut open = true;
    footer(ui, confirm, &mut open, |_| {});
    if *confirm {
        app.run_or_alert("data.sort", json!({"header": header, "keys": levels}));
    }
    if !open {
        *confirm = true;
    }
}

#[cfg(test)]
mod data_validation_tests {
    use super::*;

    #[test]
    fn list_uses_one_source_and_does_not_submit_a_second_list() {
        let fields = data_validation_fields("list", "between");
        assert!(fields.iter().any(|f| matches!(f, Field::Text { key: "formula1", label: "Source:" })));
        assert!(!fields.iter().any(|f| matches!(f, Field::Text { key: "formula2", .. } | Field::Choice { key: "operator", .. })));

        let dialog = Dialog::form(
            "dataValidation",
            "Data Validation",
            "data.validation",
            fields,
            json!({"type": "list", "operator": "between", "formula1": "ABC,DEF", "formula2": "GHI,JKL", "dropdown": true}),
        );
        let params = build_params(&dialog);
        assert_eq!(params["formula1"], "ABC,DEF");
        assert!(params.get("formula2").is_none());
        assert!(params.get("operator").is_none());
    }

    #[test]
    fn only_between_operators_show_and_submit_two_bounds() {
        let two = data_validation_fields("whole", "between");
        assert!(two.iter().any(|f| matches!(f, Field::Text { key: "formula2", label: "Maximum:" })));
        let one = data_validation_fields("date", "lessOrEqual");
        assert!(one.iter().any(|f| matches!(f, Field::Text { key: "formula1", label: "End date:" })));
        assert!(!one.iter().any(|f| matches!(f, Field::Text { key: "formula2", .. })));

        let dialog = Dialog::form(
            "dataValidation",
            "Data Validation",
            "data.validation",
            one,
            json!({"type": "date", "operator": "lessOrEqual", "formula1": "2026-10-12", "formula2": "2030-01-01"}),
        );
        let params = build_params(&dialog);
        assert_eq!(params["operator"], "lessOrEqual");
        assert_eq!(params["formula1"], "2026-10-12");
        assert!(params.get("formula2").is_none());
    }
}
