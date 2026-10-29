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
        egui::ScrollArea::vertical().show(ui, |ui| match pane.as_str() {
            "comments" => comments(app, ui),
            "watch" => watch(app, ui),
            "selection" => selection(app, ui),
            "formatChart" => format_chart(app, ui),
            "history" => history(app, ui),
            _ => {}
        });
    });
}

/// Lists comments and replies, links each to its cell, and runs delete/resolve/reopen/reply commands.
fn comments(app: &mut SheetApp, ui: &mut Ui) {
    let Some(d) = app.session.active() else { return };
    let Some(sh) = d.wb.active() else { return };
    let list: Vec<(String, mkmatrix_engine::model::Comment)> = sh.comments.iter().map(|(c, m)| (c.a1(), m.clone())).collect();
    if list.is_empty() {
        ui.label(egui::RichText::new("No comments on this sheet yet. Select a cell and choose New Comment.").italics());
    }
    let t = crate::theme::Tokens::get(ui.ctx());
    for (cell, c) in list {
        egui::Frame::NONE
            .fill(if c.resolved { t.window } else { t.menu_bg })
            .corner_radius(6.0)
            .inner_margin(8.0)
            .stroke(egui::Stroke::new(1.0, t.separator))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    if ui.link(egui::RichText::new(&cell).strong()).clicked() {
                        let _ = app.run("selection.set", json!({"cell": cell}));
                        app.grid.ensure_visible = true;
                    }
                    ui.label(egui::RichText::new(&c.author).color(t.text_dim));
                    if c.resolved {
                        ui.label(egui::RichText::new("Resolved").small().color(t.accent));
                    }
                });
                ui.label(&c.text);
                for (a, txt) in &c.replies {
                    ui.label(egui::RichText::new(format!("↳ {a}: {txt}")).color(t.text));
                }
                let key = egui::Id::unique(("reply", &cell));
                let mut draft: String = ui.ctx().data_mut(|m| m.get_temp(key)).unwrap_or_default();
                // Buttons first, right to left, so the reply field gets exactly the width they leave:
                // a field sized by a guess wider than the row pushes the resizable pane wider every frame.
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Delete").clicked() {
                            app.run_or_alert("review.deleteComment", json!({"cell": cell}));
                        }
                        if ui.small_button(if c.resolved { "Reopen" } else { "Resolve" }).clicked() {
                            app.run_or_alert("review.resolveComment", json!({"cell": cell, "resolved": !c.resolved}));
                        }
                        let send = ui.small_button("Send").clicked();
                        let r = ui.add(egui::TextEdit::singleline(&mut draft).hint_text("Reply…").desired_width(ui.available_width()));
                        if (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) || send) && !draft.is_empty() {
                            app.run_or_alert("review.replyComment", json!({"cell": cell, "text": draft}));
                            draft.clear();
                        }
                    });
                });
                ui.ctx().data_mut(|m| m.insert_temp(key, draft));
            });
        ui.add_space(6.0);
    }
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

/// Draws the chart format pane: title, type, legend, elements/axis titles, series colors, palettes/layouts and size.
fn format_chart(app: &mut SheetApp, ui: &mut Ui) {
    let Some(id) = app.selected_chart else {
        ui.label(egui::RichText::new("Select a chart to format it.").italics());
        return;
    };
    let Some(chart) = app.session.active().and_then(|d| d.wb.active().and_then(|s| s.charts.iter().find(|c| c.id == id).cloned())) else {
        ui.label("The chart no longer exists.");
        return;
    };
    let theme = app.session.active().map(|d| d.wb.theme.clone()).unwrap_or_default();
    ui.collapsing("Chart Title", |ui| {
        let field = egui::Id::unique(("mkmatrix.chart_title", id));
        if let Some(title) = crate::widgets::committed_text(ui, field, chart.title.as_deref().unwrap_or(""), None) {
            app.run_or_alert("chart.set", json!({"chart": id, "title": title}));
        }
    });
    ui.collapsing("Chart Type", |ui| {
        ui.horizontal_wrapped(|ui| {
            for (l, k, s) in [
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
            ] {
                let cur = chart.kind == mkmatrix_engine::cmd::insert::chart_kind(k, s);
                if ui.selectable_label(cur, l).clicked() {
                    app.run_or_alert("chart.set", json!({"chart": id, "type": k, "subtype": s}));
                }
            }
        });
    });
    ui.collapsing("Legend", |ui| {
        ui.horizontal(|ui| {
            for (l, v) in [("None", "none"), ("Bottom", "bottom"), ("Top", "top"), ("Left", "left"), ("Right", "right")] {
                let cur = format!("{:?}", chart.legend).eq_ignore_ascii_case(v);
                if ui.selectable_label(cur, l).clicked() {
                    app.run_or_alert("chart.set", json!({"chart": id, "legend": v}));
                }
            }
        });
    });
    ui.collapsing("Elements", |ui| {
        let mut dl = chart.data_labels;
        if ui.checkbox(&mut dl, "Data Labels").changed() {
            app.run_or_alert("chart.set", json!({"chart": id, "dataLabels": dl}));
        }
        let mut gl = chart.gridlines;
        if ui.checkbox(&mut gl, "Gridlines").changed() {
            app.run_or_alert("chart.set", json!({"chart": id, "gridlines": gl}));
        }
        ui.horizontal(|ui| {
            ui.label("Horizontal axis title:");
            let field = egui::Id::unique(("mkmatrix.chart_x_title", id));
            if let Some(xt) = crate::widgets::committed_text(ui, field, chart.x_title.as_deref().unwrap_or(""), None) {
                app.run_or_alert("chart.set", json!({"chart": id, "xTitle": if xt.is_empty() { Json::Null } else { json!(xt) }}));
            }
        });
        ui.horizontal(|ui| {
            ui.label("Vertical axis title:");
            let field = egui::Id::unique(("mkmatrix.chart_y_title", id));
            if let Some(yt) = crate::widgets::committed_text(ui, field, chart.y_title.as_deref().unwrap_or(""), None) {
                app.run_or_alert("chart.set", json!({"chart": id, "yTitle": if yt.is_empty() { Json::Null } else { json!(yt) }}));
            }
        });
    });
    ui.collapsing("Series", |ui| {
        let data = app.session.active().map(|d| mkmatrix_chart::resolve(&d.wb, d.wb.active_sheet(), &chart));
        for (i, s) in chart.series.iter().enumerate() {
            let name = data.as_ref().and_then(|d| d.series.get(i)).map(|x| x.name.clone()).unwrap_or_else(|| format!("Series {}", i + 1));
            let col =
                data.as_ref().and_then(|d| d.series.get(i)).map(|x| Color32::from_rgb(x.color[0], x.color[1], x.color[2])).unwrap_or(Color32::GRAY);
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), egui::Sense::hover());
                ui.painter().rect_filled(r, 2.0, col);
                ui.label(&name);
                ui.label(egui::RichText::new(&s.values).small().color(crate::theme::Tokens::get(ui.ctx()).text_dim));
                ui.menu_button("Fill…", |ui| {
                    if let Some(c) = crate::widgets::color_palette_fixed(ui, &theme.colors, "Automatic") {
                        app.run_or_alert(
                            "chart.formatSelection",
                            json!({"chart": id, "series": i, "color": if c == "none" { Json::Null } else { json!(c) }}),
                        );
                        ui.close();
                    }
                });
            });
        }
    });
    ui.collapsing("Colors & Layout", |ui| {
        ui.horizontal_wrapped(|ui| {
            for p in ["colorful", "monochrome", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6"] {
                if ui.button(p).clicked() {
                    app.run_or_alert("chart.changeColors", json!({"chart": id, "palette": p}));
                }
            }
        });
        ui.horizontal(|ui| {
            for n in 1..=6 {
                if ui.button(format!("Layout {n}")).clicked() {
                    app.run_or_alert("chart.quickLayout", json!({"chart": id, "layout": n}));
                }
            }
        });
    });
    ui.collapsing("Size", |ui| {
        // Keep the value being typed or dragged between frames; reloading it from the chart every frame threw the edit away.
        let draft = egui::Id::unique(("mkmatrix.chart_size_draft", id));
        let (mut w, mut h) = ui.data_mut(|d| d.get_temp::<(f32, f32)>(draft)).unwrap_or((chart.anchor.width, chart.anchor.height));
        let a = ui.add(egui::DragValue::new(&mut w).prefix("Width ").range(40.0..=4000.0));
        let b = ui.add(egui::DragValue::new(&mut h).prefix("Height ").range(40.0..=4000.0));
        if a.drag_stopped() || a.lost_focus() || b.drag_stopped() || b.lost_focus() {
            ui.data_mut(|d| d.remove::<(f32, f32)>(draft));
            if !ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                app.run_or_alert("chart.set", json!({"chart": id, "width": w, "height": h}));
            }
        } else if a.dragged() || a.has_focus() || b.dragged() || b.has_focus() {
            ui.data_mut(|d| d.insert_temp(draft, (w, h)));
        } else {
            ui.data_mut(|d| d.remove::<(f32, f32)>(draft));
        }
    });
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
