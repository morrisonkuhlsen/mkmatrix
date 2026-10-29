use super::kit::{ROW_H, button, hint, panel, rule};
use super::*;

use egui::pos2;

use crate::theme::Tokens;

/// The rules list's height.
const LIST_H: f32 = 10.0 * ROW_H + 2.0;
/// The Rule column's width.
const RULE_W: f32 = 180.0;

/// Conditional Formatting Rules Manager, in Format Cells' style: the sheet's rules in a framed
/// table (Rule, Applies to) under muted headings, in the order they are applied, each with
/// buttons to move it up or down or delete it; Close in the footer.
pub(super) fn manage_rules(app: &mut SheetApp, ui: &mut egui::Ui, open: &mut bool) {
    let lang = app.ui.language;
    let t = Tokens::get(ui.ctx());
    let list = app.session.run("home.manageRules", json!({})).unwrap_or(Json::Null);
    let rules = list["rules"].as_array().cloned().unwrap_or_default();
    ui.spacing_mut().item_spacing.y = 8.0;
    let (head, _) = ui.allocate_exact_size(vec2(ui.available_width(), 16.0), egui::Sense::hover());
    for (x, text) in [(head.left() + 10.0, "Rule"), (head.left() + 20.0 + RULE_W, "Applies to")] {
        ui.painter().text(pos2(x, head.center().y), egui::Align2::LEFT_CENTER, lang.tr(text), theme::ui_bold(11.5), t.text_dim);
    }
    let mut action = None;
    panel(ui, LIST_H, |ui| {
        if rules.is_empty() {
            ui.add_space(LIST_H / 2.0 - 10.0);
            ui.vertical_centered(|ui| hint(ui, lang.tr("There are no conditional formatting rules on this sheet.")));
            return;
        }
        ui.spacing_mut().item_spacing.y = 0.0;
        egui::ScrollArea::vertical().id_salt("manageRules.list").auto_shrink([false, false]).show(ui, |ui| {
            for (i, r) in rules.iter().enumerate() {
                let kind = r["rule"].as_object().and_then(|o| o.keys().next().cloned()).unwrap_or_default();
                let kind = lang.tr(rule_name(&kind));
                let ranges = r["ranges"].as_array().map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default();
                let (row, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), egui::Sense::hover());
                if resp.hovered() {
                    ui.painter().rect_filled(row, 0.0, t.hover);
                }
                let p = ui.painter();
                let rule_clip = egui::Rect::from_min_max(row.min, pos2(row.left() + 10.0 + RULE_W, row.bottom()));
                p.with_clip_rect(rule_clip).text(
                    pos2(row.left() + 10.0, row.center().y),
                    egui::Align2::LEFT_CENTER,
                    format!("{}. {kind}", i + 1),
                    theme::ui_font(13.0),
                    t.text,
                );
                let buttons_w = 3.0 * 24.0;
                let ranges_clip =
                    egui::Rect::from_min_max(pos2(rule_clip.right() + 10.0, row.top()), pos2(row.right() - buttons_w - 8.0, row.bottom()));
                p.with_clip_rect(ranges_clip).text(
                    pos2(ranges_clip.left(), row.center().y),
                    egui::Align2::LEFT_CENTER,
                    &ranges,
                    theme::ui_font(12.5),
                    t.text_dim,
                );
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(egui::Rect::from_min_max(pos2(row.right() - buttons_w - 4.0, row.top()), pos2(row.right() - 4.0, row.bottom())))
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                );
                child.spacing_mut().item_spacing.x = 6.0;
                for (icon, name, key) in
                    [(Icon::ChevronUp, "Move Up", "moveUp"), (Icon::Chevron, "Move Down", "moveDown"), (Icon::Close, "Delete Rule", "delete")]
                {
                    if crate::widgets::icon_button(&mut child, icon, t.text_dim, lang.tr(name), vec2(18.0, 18.0)).clicked() {
                        action = Some((key, i));
                    }
                }
            }
        });
    });
    if let Some((key, i)) = action {
        app.run_or_alert("home.manageRules", json!({ key: i }));
    }
    rule(ui);
    ui.horizontal(|ui| {
        hint(ui, &lang.tr("{count} rules").replace("{count}", &rules.len().to_string()));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if button(ui, lang.tr("Close"), true).clicked() {
                *open = false;
            }
        });
    });
}

/// A rule's kind (its variant, as the engine serializes it) in Excel's words.
fn rule_name(kind: &str) -> &str {
    match kind {
        "CellIs" => "Cell Value",
        "Expression" => "Formula",
        "ContainsText" => "Cell Contains",
        "NotContainsText" => "Does Not Contain",
        "BeginsWith" => "Begins With",
        "EndsWith" => "Ends With",
        "Blanks" => "Blanks",
        "NoBlanks" => "No Blanks",
        "Errors" => "Errors",
        "NoErrors" => "No Errors",
        "Duplicate" => "Duplicate Values",
        "Unique" => "Unique Values",
        "Top10" => "Top/Bottom",
        "AboveAverage" => "Above/Below Average",
        "TimePeriod" => "A Date Occurring",
        "ColorScale" => "Color Scale",
        "DataBar" => "Data Bar",
        "IconSet" => "Icon Set",
        other => other,
    }
}
