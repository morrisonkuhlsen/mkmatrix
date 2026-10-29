use super::kit::{ROW_H, button, hint, list_row, panel, rule};
use super::*;

use crate::theme::Tokens;

/// The list's height: the window keeps one size whatever the workbook holds.
const LIST_H: f32 = 8.0 * ROW_H + 2.0;

/// The Macros dialog, in Format Cells' style: MK Matrix's own recorded scripts and (if the file
/// carries one) the preserved VBA project in a framed list, each with what it is. Mirrors Excel's
/// Developer ▸ Macros list, but is honest that VBA is preserved and not executed.
pub(super) fn macros(app: &mut SheetApp, ui: &mut egui::Ui, d: &mut Dialog, open: &mut bool) {
    let lang = app.ui.language;
    let t = Tokens::get(ui.ctx());
    ui.spacing_mut().item_spacing.y = 8.0;
    ui.add(
        egui::Label::new(
            egui::RichText::new(
                lang.tr("Automation in this workbook. MK Matrix runs its own scripts; a file's VBA project is preserved but not executed."),
            )
            .font(theme::ui_font(13.0))
            .color(t.text_dim),
        )
        .wrap(),
    );
    let items: Vec<Json> = d.result.as_ref().and_then(Json::as_array).cloned().unwrap_or_default();
    // Scripts that came with a file stay disabled until the user trusts the workbook (#2178).
    if items.iter().any(|it| it.get("trusted").and_then(Json::as_bool) == Some(false)) {
        ui.horizontal(|ui| {
            hint(ui, lang.tr("Scripts in this workbook came from a file and are disabled."));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if button(ui, lang.tr("Trust Scripts"), false)
                    .on_hover_text(lang.tr("Let this workbook's scripts run for the rest of this session"))
                    .clicked()
                {
                    let _ = app.run("automate.trustScripts", json!({"trusted": true}));
                    d.result = app.session.run("view.macros", json!({})).ok();
                }
            });
        });
    }
    panel(ui, LIST_H, |ui| {
        if items.is_empty() {
            ui.add_space(LIST_H / 2.0 - 10.0);
            ui.vertical_centered(|ui| hint(ui, lang.tr("This workbook has no scripts or macros.")));
            return;
        }
        ui.spacing_mut().item_spacing.y = 0.0;
        egui::ScrollArea::vertical().id_salt("macros.list").auto_shrink([false, false]).show_rows(ui, ROW_H, items.len(), |ui, rows| {
            for i in rows {
                let Some(it) = items.get(i) else { continue };
                let name = it.get("name").and_then(Json::as_str).unwrap_or("(unnamed)");
                let what = if it.get("kind").and_then(Json::as_str) == Some("vba") {
                    lang.tr("VBA project (preserved, not run)").to_string()
                } else {
                    let steps = it.get("steps").and_then(Json::as_u64).unwrap_or(0);
                    lang.tr(if steps == 1 { "{count} step" } else { "{count} steps" }).replace("{count}", &steps.to_string())
                };
                if list_row(ui, name, Some(&what), i == d.list_index, 200.0).clicked() {
                    d.list_index = i;
                }
            }
        });
    });
    rule(ui);
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if button(ui, lang.tr("Close"), true).clicked() {
                *open = false;
            }
        });
    });
}
