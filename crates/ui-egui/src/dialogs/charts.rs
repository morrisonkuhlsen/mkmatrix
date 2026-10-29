use super::show::ok_cancel;
use super::*;

/// Every chart type the All Charts tab offers: `(type, subtype, label)` for `insert.chart`.
const ALL_CHARTS: [(&str, &str, &str); 21] = [
    ("column", "", "Clustered Column"),
    ("column", "stacked", "Stacked Column"),
    ("column", "stacked100", "100% Stacked Column"),
    ("bar", "", "Clustered Bar"),
    ("bar", "stacked", "Stacked Bar"),
    ("bar", "stacked100", "100% Stacked Bar"),
    ("line", "", "Line"),
    ("line", "markers", "Line with Markers"),
    ("line", "stacked", "Stacked Line"),
    ("pie", "", "Pie"),
    ("doughnut", "", "Doughnut"),
    ("area", "", "Area"),
    ("area", "stacked", "Stacked Area"),
    ("area", "stacked100", "100% Stacked Area"),
    ("scatter", "", "Scatter"),
    ("scatter", "lines", "Scatter with Lines"),
    ("radar", "", "Radar"),
    ("histogram", "", "Histogram"),
    ("waterfall", "", "Waterfall"),
    ("funnel", "", "Funnel"),
    ("combo", "", "Combo"),
];

/// The chart types listed on the dialog's current tab.
fn chart_choices(d: &Dialog) -> Vec<(String, String, String)> {
    if d.tab == "All Charts" {
        return ALL_CHARTS.iter().map(|(t, s, l)| (t.to_string(), s.to_string(), l.to_string())).collect();
    }
    let field = |x: &Json, k: &str| x.get(k).and_then(Json::as_str).unwrap_or("").to_string();
    d.result
        .as_ref()
        .and_then(Json::as_array)
        .map(|a| a.iter().map(|x| (field(x, "type"), field(x, "subtype"), field(x, "label"))).collect())
        .unwrap_or_default()
}

/// Selects entry `i` of the current tab: OK inserts it.
pub(super) fn select_chart(d: &mut Dialog, i: usize) {
    let Some((t, sub, _)) = chart_choices(d).into_iter().nth(i) else { return };
    d.list_index = i;
    d.values.insert("type".into(), json!(t));
    d.values.insert("subtype".into(), json!(sub));
}

fn chart_icon(t: &str) -> Icon {
    match t {
        "column" | "bar" | "histogram" | "waterfall" | "funnel" | "combo" => Icon::ChartBar,
        "line" | "area" | "radar" => Icon::ChartLine,
        "pie" | "doughnut" => Icon::ChartPie,
        "scatter" | "bubble" => Icon::ChartScatter,
        _ => Icon::Chart,
    }
}

/// Insert › Recommended Charts, in Format Cells' style: Recommended and All Charts as joined
/// buttons, the chart types in a framed list (icon and name, the chosen one tinted), and a preview
/// of the chosen one drawn from the data on a card beside it. A double-click inserts at once.
pub(super) fn recommended_charts(app: &mut SheetApp, ui: &mut egui::Ui, d: &mut Dialog, confirm: &mut bool, open: &mut bool) {
    use super::kit::{ROW_H, heading, panel, rule, segments};
    let lang = app.ui.language;
    let t = crate::theme::Tokens::get(ui.ctx());
    if let Some(tab) = segments(ui, &d.tab, &[("Recommended Charts", lang.tr("Recommended Charts")), ("All Charts", lang.tr("All Charts"))], 160.0)
        && d.tab != tab
    {
        d.tab = tab.to_string();
        select_chart(d, 0);
    }
    ui.add_space(10.0);
    let choices = chart_choices(d);
    const BODY_H: f32 = 300.0;
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(210.0);
            panel(ui, BODY_H, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                egui::ScrollArea::vertical().id_salt("chart_choices").auto_shrink([false, false]).show(ui, |ui| {
                    for (i, (kind, _, label)) in choices.iter().enumerate() {
                        let on = i == d.list_index;
                        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
                        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, label.as_str()));
                        let p = ui.painter();
                        if on {
                            p.rect_filled(r, 0.0, t.accent_soft);
                            p.rect_filled(egui::Rect::from_min_size(r.min, vec2(2.0, r.height())), 0.0, t.accent);
                        } else if resp.hovered() {
                            p.rect_filled(r, 0.0, t.hover);
                        }
                        let ink = if on { t.accent } else { t.text };
                        icons::paint(p, egui::Rect::from_center_size(egui::pos2(r.left() + 20.0, r.center().y), vec2(16.0, 16.0)), chart_icon(kind), ink);
                        let font = if on { theme::ui_bold(13.0) } else { theme::ui_font(13.0) };
                        p.with_clip_rect(r).text(egui::pos2(r.left() + 38.0, r.center().y), egui::Align2::LEFT_CENTER, lang.tr(label), font, ink);
                        if resp.clicked() {
                            select_chart(d, i);
                        }
                        if resp.double_clicked() {
                            select_chart(d, i);
                            *confirm = true;
                        }
                    }
                });
            });
        });
        ui.add_space(12.0);
        ui.vertical(|ui| {
            let label = choices.get(d.list_index).map(|c| c.2.clone()).unwrap_or_default();
            heading(ui, lang.tr(&label));
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), BODY_H - 26.0), Sense::hover());
            ui.painter().rect_filled(rect, 0.0, Color32::WHITE);
            ui.painter().rect_stroke(rect, 0.0, Stroke::new(1.0, t.input_border), StrokeKind::Inside);
            let params = json!({"type": d.values.get("type").cloned().unwrap_or(Json::Null), "subtype": d.values.get("subtype").cloned().unwrap_or(Json::Null)});
            if let Some(doc) = app.session.active()
                && let Ok((sheet, chart)) = mkmatrix_engine::cmd::insert::build_chart(&app.session, &params)
            {
                crate::chartview::paint_chart(ui.painter(), rect.shrink(6.0), &doc.wb, sheet, &chart);
            }
        });
    });
    rule(ui);
    ok_cancel(ui, confirm, open);
    if *confirm {
        *confirm = false;
        let params =
            json!({"type": d.values.get("type").cloned().unwrap_or(Json::Null), "subtype": d.values.get("subtype").cloned().unwrap_or(Json::Null)});
        match app.run("insert.chart", params) {
            Ok(r) => {
                // The new chart is selected, as after inserting any object.
                app.selected_chart = r.get("chart").and_then(Json::as_u64).and_then(|id| u32::try_from(id).ok());
                *open = false;
            }
            Err(e) => app.message = Some((d.title.clone(), crate::clean_error(&e))),
        }
    }
}

/// Chart Design › Select Data, in Format Cells' style: the chart's data range, its series in a
/// framed list (each kept or removed, the removed ones struck through), and the category labels,
/// each under a muted heading; OK runs `chart.selectData` when something changed.
pub(super) fn select_data(app: &mut SheetApp, ui: &mut egui::Ui, d: &mut Dialog, confirm: &mut bool, open: &mut bool) {
    use super::kit::{ROW_H, button, heading, panel, rule};
    let lang = app.ui.language;
    let t = crate::theme::Tokens::get(ui.ctx());
    let text = |d: &Dialog, k: &str| d.values.get(k).and_then(Json::as_str).unwrap_or("").to_string();
    let edit = |ui: &mut egui::Ui, d: &mut Dialog, key: &str, label: &str| {
        heading(ui, lang.tr(label));
        let mut v = text(d, key);
        if ui.add(egui::TextEdit::singleline(&mut v).desired_width(ui.available_width())).accessible_name(lang.tr(label)).changed() {
            d.values.insert(key.into(), json!(v));
        }
    };
    ui.spacing_mut().item_spacing.y = 6.0;
    edit(ui, d, "range", "Chart data range:");
    ui.add_space(6.0);
    heading(ui, lang.tr("Legend Entries (Series)"));
    let names: Vec<String> =
        d.values.get("names").and_then(Json::as_array).map(|a| a.iter().map(|n| n.as_str().unwrap_or("").to_string()).collect()).unwrap_or_default();
    let mut remove: Vec<u64> =
        d.values.get("remove").and_then(Json::as_array).map(|a| a.iter().filter_map(Json::as_u64).collect()).unwrap_or_default();
    let kept = names.len().saturating_sub(remove.len());
    let list_h = 5.0 * (ROW_H + 4.0) + 2.0;
    panel(ui, list_h, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        egui::ScrollArea::vertical().id_salt("series").auto_shrink([false, false]).show(ui, |ui| {
            for (i, name) in names.iter().enumerate() {
                let i = i as u64;
                let removed = remove.contains(&i);
                let label = if name.is_empty() { format!("Series{}", i + 1) } else { name.clone() };
                ui.allocate_ui_with_layout(vec2(ui.available_width(), ROW_H + 4.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.add_space(10.0);
                    let text = egui::RichText::new(label).font(theme::ui_font(13.0));
                    ui.label(if removed { text.strikethrough().color(t.text_disabled) } else { text.color(t.text) });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(4.0);
                        if removed {
                            if button(ui, lang.tr("Keep"), false).clicked() {
                                remove.retain(|x| *x != i);
                            }
                        } else {
                            ui.add_enabled_ui(kept > 1, |ui| {
                                if button(ui, lang.tr("Remove"), false).clicked() {
                                    remove.push(i);
                                }
                            });
                        }
                    });
                });
            }
        });
    });
    d.values.insert("remove".into(), json!(remove));
    ui.add_space(6.0);
    edit(ui, d, "categories", "Horizontal (Category) Axis Labels:");
    rule(ui);
    ok_cancel(ui, confirm, open);
    if *confirm {
        *confirm = false;
        let mut params = serde_json::Map::new();
        params.insert("chart".into(), d.values.get("chart").cloned().unwrap_or(Json::Null));
        let range_changed = text(d, "range") != text(d, "range0");
        if range_changed {
            params.insert("range".into(), json!(text(d, "range")));
        } else if !remove.is_empty() {
            // A new range rebuilds the series; removals only apply to the current ones.
            params.insert("removeSeries".into(), json!(remove));
        }
        if text(d, "categories") != text(d, "categories0") || (range_changed && !text(d, "categories").is_empty()) {
            params.insert("categories".into(), json!(text(d, "categories")));
        }
        if params.len() == 1 {
            *open = false;
            return;
        }
        match app.run("chart.selectData", Json::Object(params)) {
            Ok(_) => *open = false,
            Err(e) => app.message = Some((d.title.clone(), crate::clean_error(&e))),
        }
    }
}
