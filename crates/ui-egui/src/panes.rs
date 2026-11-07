//! Right-hand task panes: Comments, Watch Window, Selection Pane, Format Chart.

use egui::{Color32, Ui, vec2};
use serde_json::{Value as Json, json};

use crate::SheetApp;
use crate::theme::{self, Tokens};

pub fn show(app: &mut SheetApp, ui: &mut Ui) {
    // The Comments pane is engine state (`review.showComments`, so agents can drive it); the other
    // panes are UI state. One pane shows at a time: opening another one closes Comments.
    if app.grid.pane.as_deref() == Some("comments") {
        app.grid.pane = None;
        app.run_or_alert("review.showComments", json!({"on": true}));
    } else if app.grid.pane.is_some() && app.session.comments_pane {
        app.run_or_alert("review.showComments", json!({"on": false}));
    }
    // The History pane's preview lives only while it is open.
    if app.grid.pane.as_deref() != Some("history") {
        app.history_preview = None;
    }
    let Some(pane) = app.grid.pane.clone().or_else(|| app.session.comments_pane.then(|| "comments".to_string())) else { return };
    let t = Tokens::get(ui.ctx());
    // Capped at half the window so a pane can never cover the sheet, whatever its content asks for.
    let max = (ui.available_width() / 2.0).max(300.0);
    let panel = egui::Panel::right("task_pane").default_size(300.0).max_size(max).resizable(true);
    panel.frame(egui::Frame::NONE.fill(t.ribbon).inner_margin(10)).show(ui, |ui| {
        ui.horizontal(|ui| {
            let title = match pane.as_str() {
                "comments" => "Comments",
                "watch" => "Watch Window",
                "selection" => "Selection",
                "formatChart" => "Format Chart Area",
                "history" => "History",
                _ => "",
            };
            ui.label(egui::RichText::new(app.ui.language.tr(title)).font(theme::ui_bold(15.0)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if crate::widgets::icon_button(ui, crate::icons::Icon::Close, t.text_dim, "Close", vec2(22.0, 22.0)).clicked() {
                    if pane == "comments" {
                        app.run_or_alert("review.showComments", json!({"on": false}));
                    }
                    app.grid.pane = None;
                }
            });
        });
        ui.separator();
        // Comments keeps its search and filters fixed above its own scrolling list.
        if pane == "comments" {
            crate::comments_pane::show(app, ui);
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| match pane.as_str() {
            "watch" => watch(app, ui),
            "selection" => selection(app, ui),
            "formatChart" => format_chart(app, ui),
            "history" => history(app, ui),
            _ => {}
        });
    });
}

/// Adds the active cell, lists watch values and formulas, and removes watched entries.
fn watch(app: &mut SheetApp, ui: &mut Ui) {
    if ui.button("Add Watch (active cell)").clicked()
        && let Some(d) = app.session.active()
        && let Some(sh) = d.wb.active()
    {
        let r = format!("{}!{}", mkmatrix_engine::formula::quote_sheet(&sh.name), d.selection.active.a1());
        let _ = app.session.run("formulas.watchWindow", json!({"add": r}));
    }
    let list = app.session.run("formulas.watchWindow", json!({})).unwrap_or(Json::Null);
    egui::Grid::new("watch_grid").striped(true).num_columns(4).show(ui, |ui| {
        ui.strong("Cell");
        ui.strong("Value");
        ui.strong("Formula");
        ui.label("");
        ui.end_row();
        for w in list.as_array().cloned().unwrap_or_default() {
            let cell = w["cell"].as_str().unwrap_or("").to_string();
            ui.label(&cell);
            ui.label(w["value"].as_str().unwrap_or(""));
            ui.label(w["formula"].as_str().unwrap_or(""));
            if crate::widgets::icon_button(ui, crate::icons::Icon::Close, Tokens::get(ui.ctx()).text_dim, "Delete", vec2(18.0, 18.0)).clicked() {
                let _ = app.session.run("formulas.watchWindow", json!({"remove": cell}));
            }
            ui.end_row();
        }
    });
}

/// Lists the sheet's objects and dispatches select, bring-forward, send-backward and delete.
fn selection(app: &mut SheetApp, ui: &mut Ui) {
    let list = app.session.run("arrange.selectionPane", json!({})).unwrap_or(Json::Null);
    let items = list.as_array().cloned().unwrap_or_default();
    if items.is_empty() {
        ui.label(egui::RichText::new("There are no shapes, pictures or charts on this sheet.").italics());
    }
    for it in items.iter().rev() {
        let kind = it["kind"].as_str().unwrap_or("").to_string();
        let id = it["id"].as_u64().unwrap_or(0) as u32;
        ui.horizontal(|ui| {
            let sel = app.selected_chart == Some(id);
            if ui.selectable_label(sel, it["name"].as_str().unwrap_or("")).clicked() {
                app.selected_chart = Some(id);
            }
            if ui.small_button("▲").on_hover_text("Bring Forward").clicked() {
                app.run_or_alert("arrange.bringForward", json!({"kind": kind, "id": id}));
            }
            if ui.small_button("▼").on_hover_text("Send Backward").clicked() {
                app.run_or_alert("arrange.sendBackward", json!({"kind": kind, "id": id}));
            }
            if crate::widgets::icon_button(ui, crate::icons::Icon::Close, Tokens::get(ui.ctx()).text_dim, "Delete", vec2(18.0, 18.0)).clicked() {
                app.run_or_alert("object.delete", json!({"kind": kind, "id": id}));
            }
        });
    }
}

/// Draws the chart format pane in the dialogs' style (`dialogs::kit`): a muted heading per section
/// (title, type, legend, elements, series, colours and layout, size), each under a rule; the types
/// as a list with the chart's own marked, the legend, elements and layouts as segmented controls,
/// fields in labelled rows.
fn format_chart(app: &mut SheetApp, ui: &mut Ui) {
    use crate::dialogs::kit;
    let lang = app.ui.language;
    let Some(id) = app.selected_chart else {
        kit::hint(ui, lang.tr("Select a chart to format it."));
        return;
    };
    let Some(chart) = app.session.active().and_then(|d| d.wb.active().and_then(|s| s.charts.iter().find(|c| c.id == id).cloned())) else {
        kit::hint(ui, lang.tr("The chart no longer exists."));
        return;
    };
    let theme = app.session.active().map(|d| d.wb.theme.clone()).unwrap_or_default();
    let t = Tokens::get(ui.ctx());
    let width = ui.available_width();
    // Nothing may ask for more than the pane has: a control wider than it widens the pane, which
    // widens the control, frame after frame. A text field draws its margins outside its width;
    // a form's controls sit beside a label column and its gap.
    const FIELD_MARGIN: f32 = 8.0;
    let beside = (width - kit::LABEL_W - 12.0).max(40.0);
    ui.spacing_mut().item_spacing.y = 6.0;

    kit::heading(ui, lang.tr("Chart Title"));
    let field = egui::Id::unique(("mkmatrix.chart_title", id));
    let title_name = lang.tr("Chart Title");
    if let Some(title) =
        crate::widgets::committed_text_named(ui, field, chart.title.as_deref().unwrap_or(""), Some(width - FIELD_MARGIN), Some(title_name))
    {
        app.run_or_alert("chart.set", json!({"chart": id, "title": title}));
    }

    kit::rule(ui);
    kit::heading(ui, lang.tr("Chart Type"));
    const TYPES: [(&str, &str, &str); 15] = [
        ("Column", "column", ""),
        ("Stacked Column", "column", "stacked"),
        ("100% Stacked Column", "column", "stacked100"),
        ("Bar", "bar", ""),
        ("Stacked Bar", "bar", "stacked"),
        ("100% Stacked Bar", "bar", "stacked100"),
        ("Line", "line", "markers"),
        ("Stacked Line", "line", "stacked"),
        ("Area", "area", ""),
        ("Stacked Area", "area", "stacked"),
        ("100% Stacked Area", "area", "stacked100"),
        ("Pie", "pie", ""),
        ("Doughnut", "doughnut", ""),
        ("Scatter", "scatter", ""),
        ("Radar", "radar", ""),
    ];
    // Every type shows at once (the pane scrolls as a whole).
    kit::panel(ui, TYPES.len() as f32 * kit::ROW_H + 2.0, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (label, k, s) in TYPES {
            let cur = chart.kind == mkmatrix_engine::cmd::insert::chart_kind(k, s);
            if kit::list_row(ui, lang.tr(label), None, cur, width).clicked() && !cur {
                app.run_or_alert("chart.set", json!({"chart": id, "type": k, "subtype": s}));
            }
        }
    });

    kit::rule(ui);
    kit::heading(ui, lang.tr("Legend"));
    let legend = format!("{:?}", chart.legend).to_ascii_lowercase();
    let options =
        [("none", lang.tr("None")), ("bottom", lang.tr("Bottom")), ("top", lang.tr("Top")), ("left", lang.tr("Left")), ("right", lang.tr("Right"))];
    if let Some(v) = kit::segments(ui, &legend, &options, (width / options.len() as f32).floor()) {
        app.run_or_alert("chart.set", json!({"chart": id, "legend": v}));
    }

    kit::rule(ui);
    kit::heading(ui, lang.tr("Elements"));
    let show_none = [("on", lang.tr("Show")), ("off", lang.tr("None"))];
    let seg_w = (beside / 2.0).floor().min(90.0);
    kit::form(ui, "mkmatrix.chart_elements", |ui| {
        for (label, key, on) in [("Data Labels", "dataLabels", chart.data_labels), ("Gridlines", "gridlines", chart.gridlines)] {
            kit::label(ui, lang.tr(label));
            if let Some(v) = kit::segments(ui, if on { "on" } else { "off" }, &show_none, seg_w) {
                app.run_or_alert("chart.set", json!({"chart": id, key: v == "on"}));
            }
            ui.end_row();
        }
        for (label, key, current, salt) in [
            ("Horizontal axis title", "xTitle", chart.x_title.as_deref(), "mkmatrix.chart_x_title"),
            ("Vertical axis title", "yTitle", chart.y_title.as_deref(), "mkmatrix.chart_y_title"),
        ] {
            kit::label(ui, lang.tr(label));
            let field = egui::Id::unique((salt, id));
            if let Some(text) =
                crate::widgets::committed_text_named(ui, field, current.unwrap_or(""), Some(beside - FIELD_MARGIN), Some(lang.tr(label)))
            {
                app.run_or_alert("chart.set", json!({"chart": id, key: if text.is_empty() { Json::Null } else { json!(text) }}));
            }
            ui.end_row();
        }
    });

    kit::rule(ui);
    kit::heading(ui, lang.tr("Series"));
    let data = app.session.active().map(|d| mkmatrix_chart::resolve(&d.wb, d.wb.active_sheet(), &chart));
    for (i, s) in chart.series.iter().enumerate() {
        let name = data.as_ref().and_then(|d| d.series.get(i)).map(|x| x.name.clone()).unwrap_or_else(|| format!("Series {}", i + 1));
        let col = data.as_ref().and_then(|d| d.series.get(i)).map(|x| Color32::from_rgb(x.color[0], x.color[1], x.color[2])).unwrap_or(t.text_dim);
        ui.horizontal(|ui| {
            // The series' own colour: a data-driven swatch, as palettes are.
            let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), egui::Sense::hover());
            ui.painter().rect_filled(r, 2.0, col);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let fill = kit::button(ui, lang.tr("Fill…"), false);
                egui::Popup::menu(&fill).show(|ui| {
                    if let Some(c) = crate::widgets::color_palette_fixed(ui, &theme.colors, "Automatic") {
                        app.run_or_alert(
                            "chart.formatSelection",
                            json!({"chart": id, "series": i, "color": if c == "none" { Json::Null } else { json!(c) }}),
                        );
                        ui.close();
                    }
                });
                // Name and range in what is left, cut short rather than pushing the pane wider.
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.add(egui::Label::new(egui::RichText::new(&name).font(theme::ui_font(13.0)).color(t.text)).truncate());
                    ui.add(egui::Label::new(egui::RichText::new(&s.values).font(theme::ui_font(11.5)).color(t.text_dim)).truncate());
                });
            });
        });
    }

    kit::rule(ui);
    kit::heading(ui, lang.tr("Colors & Layout"));
    ui.horizontal_wrapped(|ui| {
        for (p, label) in [
            ("colorful", "Colorful"),
            ("monochrome", "Monochrome"),
            ("accent1", "Accent 1"),
            ("accent2", "Accent 2"),
            ("accent3", "Accent 3"),
            ("accent4", "Accent 4"),
            ("accent5", "Accent 5"),
            ("accent6", "Accent 6"),
        ] {
            if kit::button(ui, lang.tr(label), false).clicked() {
                app.run_or_alert("chart.changeColors", json!({"chart": id, "palette": p}));
            }
        }
    });
    kit::form(ui, "mkmatrix.chart_layout", |ui| {
        kit::label(ui, lang.tr("Quick Layout"));
        let layouts = [("1", "1"), ("2", "2"), ("3", "3"), ("4", "4"), ("5", "5"), ("6", "6")];
        if let Some(n) = kit::segments(ui, "", &layouts, (beside / layouts.len() as f32).floor().min(32.0)) {
            app.run_or_alert("chart.quickLayout", json!({"chart": id, "layout": n.parse::<u32>().unwrap_or(1)}));
        }
        ui.end_row();
    });

    kit::rule(ui);
    kit::heading(ui, lang.tr("Size"));
    // Keep the value being typed or dragged between frames; reloading it from the chart every frame threw the edit away.
    let draft = egui::Id::unique(("mkmatrix.chart_size_draft", id));
    let (mut w, mut h) = ui.data_mut(|d| d.get_temp::<(f32, f32)>(draft)).unwrap_or((chart.anchor.width, chart.anchor.height));
    let mut sizes = Vec::new();
    kit::form(ui, "mkmatrix.chart_size", |ui| {
        kit::label(ui, lang.tr("Width"));
        sizes.push(ui.add(egui::DragValue::new(&mut w).suffix(" pt").range(40.0..=4000.0)));
        ui.end_row();
        kit::label(ui, lang.tr("Height"));
        sizes.push(ui.add(egui::DragValue::new(&mut h).suffix(" pt").range(40.0..=4000.0)));
        ui.end_row();
    });
    if sizes.iter().any(|r| r.drag_stopped() || r.lost_focus()) {
        ui.data_mut(|d| d.remove::<(f32, f32)>(draft));
        if !ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            app.run_or_alert("chart.set", json!({"chart": id, "width": w, "height": h}));
        }
    } else if sizes.iter().any(|r| r.dragged() || r.has_focus()) {
        ui.data_mut(|d| d.insert_temp(draft, (w, h)));
    } else {
        ui.data_mut(|d| d.remove::<(f32, f32)>(draft));
    }
}

/// History (beyond Excel): the workbook's timeline. A slider and the list of steps, oldest first;
/// choosing a point shows on the grid, faded, how each cell that differs stood then (nothing
/// changes), and Go Back Here goes there with undo or redo, so it can be left again.
fn history(app: &mut SheetApp, ui: &mut Ui) {
    let lang = app.ui.language;
    let t = Tokens::get(ui.ctx());
    // The timeline is read again only when the workbook changes (it diffs the latest steps).
    let Some(rev) = app.session.active().map(|d| (d.uid, d.revision)) else { return };
    let cache_id = egui::Id::unique("mkmatrix.history.cache");
    let h = match ui.data(|d| d.get_temp::<((u64, u64), Json)>(cache_id)) {
        Some((k, h)) if k == rev => h,
        _ => {
            let Ok(h) = app.session.run("edit.history", json!({})) else { return };
            ui.data_mut(|d| d.insert_temp(cache_id, (rev, h.clone())));
            h
        }
    };
    // Each step as what it did and where: "Enter Cell · B1".
    let steps: Vec<String> = h["steps"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|s| {
                    let label = lang.tr(s["label"].as_str().unwrap_or("")).to_string();
                    match s["at"].as_str() {
                        Some(at) if !at.is_empty() => format!("{label} · {at}"),
                        _ => label,
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    let now = h["position"].as_u64().unwrap_or(0) as usize;
    let last = steps.len();
    let id = egui::Id::unique("mkmatrix.history.target");
    let Some(key) = app.session.active().map(|d| (d.uid, d.revision, d.wb.active_sheet())) else { return };
    // The chosen point, back to now whenever the workbook changes under it.
    let (mut target, seen) = ui.data(|d| d.get_temp::<(usize, (u64, u64, usize))>(id)).unwrap_or((now, key));
    if seen != key {
        target = now;
    }
    target = target.min(last);
    ui.spacing_mut().item_spacing.y = 6.0;
    if last == 0 {
        ui.label(egui::RichText::new(lang.tr("Nothing has changed in this workbook yet.")).color(t.text_dim));
        return;
    }
    ui.spacing_mut().slider_width = ui.available_width() - 8.0;
    ui.add(egui::Slider::new(&mut target, 0..=last).show_value(false)).on_hover_text(lang.tr("Drag to see how the sheet stood at each step"));
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(lang.tr("Start")).font(theme::ui_font(11.5)).color(t.text_dim));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(lang.tr("Latest")).font(theme::ui_font(11.5)).color(t.text_dim));
        });
    });
    let differs = target != now;
    let go =
        ui.add_enabled(
            differs,
            egui::Button::new(egui::RichText::new(lang.tr("Go Back Here")).color(if differs { t.on_accent } else { t.text_disabled }))
                .fill(if differs { t.accent } else { t.input_bg }),
        );
    if go.clicked() {
        let (cmd, n) = if target < now { ("edit.undo", now - target) } else { ("edit.redo", target - now) };
        app.run_or_alert(cmd, json!({"steps": n}));
    }
    ui.add_space(4.0);
    // Point 0 is the workbook as it was opened; point i is after step i.
    for p in (0..=last).rev() {
        let label = if p == 0 { lang.tr("Opened").to_string() } else { steps.get(p - 1).cloned().unwrap_or_default() };
        let (row, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), egui::Sense::click());
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, p == target, &label));
        let painter = ui.painter();
        if p == target {
            painter.rect_filled(row, 0.0, t.accent_soft);
            painter.rect_filled(egui::Rect::from_min_size(row.min, vec2(2.0, row.height())), 0.0, t.accent);
        } else if resp.hovered() {
            painter.rect_filled(row, 0.0, t.hover);
        }
        // Steps past now can be redone: drawn muted.
        let ink = if p > now {
            t.text_disabled
        } else if p == target {
            t.accent
        } else {
            t.text
        };
        painter.circle_filled(
            egui::pos2(row.left() + 12.0, row.center().y),
            if p == now { 4.5 } else { 3.0 },
            if p == now { t.accent } else { t.text_dim },
        );
        painter.text(egui::pos2(row.left() + 26.0, row.center().y), egui::Align2::LEFT_CENTER, &label, theme::ui_font(12.5), ink);
        if p == now {
            painter.text(egui::pos2(row.right() - 8.0, row.center().y), egui::Align2::RIGHT_CENTER, lang.tr("now"), theme::ui_font(11.0), t.text_dim);
        }
        if resp.clicked() {
            target = p;
        }
    }
    ui.data_mut(|d| d.insert_temp(id, (target, key)));
    // The grid shows the chosen point, faded, where it differs from now.
    if target == now {
        app.history_preview = None;
    } else {
        let pkey = (target, key.0, key.1, key.2);
        if app.history_preview.as_ref().is_none_or(|(k, _)| *k != pkey) {
            let cells = app
                .session
                .run("edit.historyDiff", json!({"position": target}))
                .ok()
                .and_then(|r| r["cells"].as_array().cloned())
                .unwrap_or_default()
                .iter()
                .filter_map(|c| Some((mkmatrix_engine::core::CellRef::parse(c["cell"].as_str()?)?, c["text"].as_str()?.to_string())))
                .collect();
            app.history_preview = Some((pkey, cells));
        }
    }
}
