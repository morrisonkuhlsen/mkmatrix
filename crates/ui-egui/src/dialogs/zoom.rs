use super::show::ok_cancel;
use super::*;

/// Zoom's Magnification presets, as `preset` values and labels, in Excel's order.
const ZOOM_PRESETS: [(&str, &str); 5] = [("200", "200%"), ("100", "100%"), ("75", "75%"), ("50", "50%"), ("25", "25%")];

/// The `preset` that shows a zoom percentage: its Magnification entry, or Custom.
pub(super) fn zoom_preset(percent: f64) -> &'static str {
    ZOOM_PRESETS.iter().map(|(val, _)| *val).find(|val| val.parse::<f64>().ok() == Some(percent)).unwrap_or("custom")
}

/// View › Zoom, laid out as Excel's in Format Cells' style: one Magnification column of round
/// options from 200% to 25%, Fit selection and Custom with its percentage box. The current zoom
/// is pre-selected; typing a percentage selects Custom; Fit selection zooms so the selected cells
/// fill the grid.
pub(super) fn zoom(ui: &mut egui::Ui, d: &mut Dialog, confirm: &mut bool, open: &mut bool) {
    use super::kit::{heading, option_row, rule};
    let lang = crate::i18n::current(ui.ctx());
    let cur = d.values.get("preset").and_then(Json::as_str).unwrap_or("custom").to_string();
    let w = ui.available_width();
    heading(ui, lang.tr("Magnification"));
    ui.spacing_mut().item_spacing.y = 2.0;
    for (val, text) in ZOOM_PRESETS {
        if option_row(ui, text, cur == val, w).clicked() {
            d.set_value("preset", json!(val));
        }
    }
    if option_row(ui, lang.tr("Fit selection"), cur == "fit", w).clicked() {
        d.set_value("preset", json!("fit"));
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        if option_row(ui, &format!("{}:", lang.tr("Custom")), cur == "custom", 110.0).clicked() {
            d.set_value("preset", json!("custom"));
        }
        // Whole percentages, as Excel shows them; the engine clamps to 10..400 as well.
        let mut percent = d.values.get("percent").and_then(Json::as_f64).unwrap_or(100.0).round() as i64;
        if ui.add_sized(vec2(64.0, 24.0), egui::DragValue::new(&mut percent).range(10..=400).speed(1.0)).accessible_name(lang.tr("Custom")).changed()
        {
            d.set_value("percent", json!(percent));
        }
        ui.label("%");
    });
    rule(ui);
    ok_cancel(ui, confirm, open);
}
