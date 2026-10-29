//! The PivotTable Fields task pane: shown on the right when the active cell is in a pivot
//! report. A search box over the source fields, each with its type; ticking a field adds it (text
//! → Rows, numbers → Values). Fields are dragged from the list into Filters, Columns, Rows or
//! Values, between them and within one to reorder, and out of the areas to remove them; while a
//! field is dragged over an area the grid previews the report as the drop would leave it. Each
//! field in an area has a menu (move, summarize, show as, sort, group, remove). Everything runs
//! `pivot.*` commands, one undo step per drop.

use egui::{DragAndDrop, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::{Value as Json, json};

use crate::SheetApp;
use crate::dialogs::kit;
use crate::i18n::{self, Language};
use crate::icons::{self, Icon};
use crate::theme::{self, Tokens};

/// Whether the active cell is inside a pivot report (so the pane should show).
pub fn active_pivot(app: &SheetApp) -> Option<String> {
    let d = app.session.active()?;
    let sh = d.wb.active()?;
    let a = d.selection.active;
    sh.pivots.iter().find(|p| p.last_range.is_some_and(|r| r.contains(a)) || p.anchor == a).map(|p| p.name.clone())
}

/// Σ Values (two or more value fields) sits in Rows or Columns; it moves only between them.
const VALUES_FIELD: &str = "Σ Values";
/// The four areas: key, label, icon.
const AREAS: [(&str, &str, Icon); 4] = [
    ("filters", "Filters", Icon::Filter),
    ("columns", "Columns", Icon::TextColumns),
    ("rows", "Rows", Icon::SheetList),
    ("values", "Values", Icon::Sum),
];
/// A field list row's height.
const ROW_H: f32 = 26.0;
/// A field chip's height in an area.
const CHIP_H: f32 = 26.0;

/// A field being dragged: from the field list (`from: None`) or from an area, at its position
/// there among the real fields (Σ Values not counted, unless it is the one dragged).
#[derive(Clone, Debug, PartialEq)]
struct FieldDrag {
    field: String,
    from: Option<&'static str>,
    index: usize,
}

/// Where a drag would land: an area at a position, or out of the areas (removing the field).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Target {
    Area(&'static str, usize),
    Remove,
}

/// A field in an area as the pane shows it.
struct Entry {
    name: String,
    /// `false` for Σ Values, which isn't a source field.
    real: bool,
    /// A value field's function (`sum`, `count`…).
    func: Option<String>,
}

/// An area as laid out this frame: its drop zone and its chips (with whether each is real).
struct Zone {
    key: &'static str,
    rect: Rect,
    chips: Vec<(Rect, bool)>,
}

/// Draws the PivotTable Fields pane: add/remove fields, drag them between areas, sort/group/value settings, refresh, calculated field and report layout.
pub fn show(app: &mut SheetApp, ui: &mut Ui) {
    let Some(name) = active_pivot(app) else { return };
    let t = Tokens::get(ui.ctx());
    let lang = i18n::current(ui.ctx());
    let Some(key) = app.session.active().map(|d| (d.uid, d.revision, name.clone())) else { return };
    if app.grid.pivot_fields.as_ref().is_none_or(|(k, _)| *k != key) {
        let info = match app.session.run("pivot.fields", json!({"pivot": name})) {
            Ok(v) => v,
            Err(_) => return,
        };
        app.grid.pivot_fields = Some((key, info));
    }
    let Some((_, info)) = app.grid.pivot_fields.clone() else { return };
    let ctx = ui.ctx().clone();
    let drag = DragAndDrop::payload::<FieldDrag>(&ctx);
    let search_id = egui::Id::unique("mkmatrix.pivot_pane.search");
    let mut search: String = ctx.data(|d| d.get_temp(search_id)).unwrap_or_default();
    let mut zones: Vec<Zone> = Vec::new();
    let pane = egui::Panel::right("pivot_pane").default_size(300.0).resizable(true).frame(egui::Frame::NONE.fill(t.ribbon).inner_margin(12)).show(
        ui,
        |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            ui.label(egui::RichText::new(lang.tr("PivotTable Fields")).font(theme::ui_bold(15.0)));
            let fields = info["fields"].as_array().cloned().unwrap_or_default();
            let mut summary = lang.tr("{n} fields").replace("{n}", &fields.len().to_string());
            if let Some(n) = info["records"].as_u64() {
                summary = format!("{summary} · {}", lang.tr("{n} records").replace("{n}", &crate::credits::group(n)));
            }
            ui.label(egui::RichText::new(summary).font(theme::ui_font(12.0)).color(t.text_dim));
            kit::search_field(ui, &mut search, lang.tr("Search fields"), lang.tr("Search fields"), ui.available_width());
            let needle = search.trim().to_lowercase();
            let shown: Vec<&Json> =
                fields.iter().filter(|f| needle.is_empty() || f["name"].as_str().unwrap_or("").to_lowercase().contains(&needle)).collect();
            let list_h = (shown.len().max(1) as f32 * ROW_H + 2.0).clamp(ROW_H * 3.0 + 2.0, (ui.available_height() * 0.36).max(ROW_H * 4.0 + 2.0));
            kit::panel(ui, list_h, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                egui::ScrollArea::vertical().auto_shrink([false, false]).id_salt("pv_fields").show(ui, |ui| {
                    if shown.is_empty() {
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.add_space(10.0);
                            ui.label(egui::RichText::new(lang.tr("No field matches the search")).font(theme::ui_font(12.5)).color(t.text_dim));
                        });
                    }
                    for f in shown {
                        field_row(app, ui, &name, f, lang, &t);
                    }
                });
            });
            ui.add_space(4.0);
            ui.label(egui::RichText::new(lang.tr("Drag fields between areas below:")).font(theme::ui_font(12.0)).color(t.text_dim));
            let entries = |key: &str| area_entries(&info, key);
            ui.columns(2, |cols| {
                for (i, (key, label, icon)) in AREAS.iter().enumerate() {
                    let ui = &mut cols[i % 2];
                    zones.push(area_card(app, ui, &name, key, lang.tr(label), *icon, &entries(key), drag.as_deref(), lang, &t));
                }
            });
            ui.add_space(4.0);
            kit::rule(ui);
            ui.horizontal_wrapped(|ui| {
                if kit::button(ui, lang.tr("Refresh"), false).clicked() {
                    app.run_or_alert("pivot.refresh", json!({"pivot": name}));
                }
                if kit::button(ui, lang.tr("Calculated Field…"), false).clicked() {
                    app.run_or_alert("pivot.calculatedField", json!({"pivot": name}));
                }
                craft_ui::ComboBox::from_id_salt("pv_layout")
                    .selected_text(lang.tr("Report Layout"))
                    .show_ui(ui, |ui| {
                        for (l, v) in [("Compact", "compact"), ("Outline", "outline"), ("Tabular", "tabular")] {
                            if ui.button(lang.tr(l)).clicked() {
                                app.run_or_alert("pivot.layout", json!({"pivot": name, "layout": v}));
                            }
                        }
                    })
                    .response
                    .accessible_name(lang.tr("Report Layout"));
            });
        },
    );
    ctx.data_mut(|d| d.insert_temp(search_id, search));
    let Some(drag) = drag else { return };
    // The drag: where it would land, previewed on the grid, run on release.
    let Some(at) = ctx.pointer_latest_pos() else { return };
    let target = drop_target(&zones, pane.response.rect, at, &drag);
    let cmd = target.and_then(|tg| drop_command(&info, &name, &drag, tg));
    paint_drag(&ctx, &zones, target, cmd.is_some(), at, &drag, &t);
    if ctx.input(|i| i.pointer.any_released()) {
        DragAndDrop::clear_payload(&ctx);
        if let Some((id, params)) = cmd {
            app.run_or_alert(id, params);
        }
    } else if let Some(c) = cmd {
        app.preview_hover = Some(c);
    }
}

/// A source field in the list: its box (ticked when it is in an area), its type, its name, and
/// the areas it is in. A click adds or removes it; dragging it takes it to an area.
fn field_row(app: &mut SheetApp, ui: &mut Ui, pivot: &str, f: &Json, lang: Language, t: &Tokens) {
    let fname = f["name"].as_str().unwrap_or("").to_string();
    let areas: Vec<&str> = f["areas"].as_array().map(|a| a.iter().filter_map(Json::as_str).collect()).unwrap_or_default();
    let on = !areas.is_empty();
    let numeric = f["numeric"].as_bool().unwrap_or(false);
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click_and_drag());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, on, &fname));
    let p = ui.painter();
    if resp.hovered() || resp.dragged() {
        p.rect_filled(r, 0.0, t.hover);
    }
    let b = Rect::from_center_size(pos2(r.left() + 17.0, r.center().y), vec2(14.0, 14.0));
    if on {
        p.rect_filled(b, 3.0, t.accent);
        let s = Stroke::new(1.8, t.on_accent);
        p.line_segment([b.center() + vec2(-3.5, 0.2), b.center() + vec2(-1.0, 2.8)], s);
        p.line_segment([b.center() + vec2(-1.0, 2.8), b.center() + vec2(3.8, -2.6)], s);
    } else {
        p.rect_filled(b, 3.0, t.input_bg);
        p.rect_stroke(b, 3.0, Stroke::new(1.0, if resp.hovered() { t.accent } else { t.input_border }), egui::StrokeKind::Inside);
    }
    // The type: a date, a number, a calculated field or text.
    let kind = Rect::from_min_size(pos2(b.right() + 8.0, r.center().y - 8.0), vec2(26.0, 16.0));
    if f["date"].as_bool().unwrap_or(false) {
        icons::paint(p, Rect::from_center_size(kind.center(), vec2(14.0, 14.0)), Icon::Calendar, t.text_dim);
    } else {
        let tag = if f["calculated"].as_bool().unwrap_or(false) {
            "fx"
        } else if numeric {
            "123"
        } else {
            "Abc"
        };
        p.text(kind.center(), egui::Align2::CENTER_CENTER, tag, theme::ui_font(10.0), t.text_dim);
    }
    // The right end: where it is.
    let right = if on {
        let names: Vec<&str> = areas.iter().map(|a| lang.tr(area_label(a))).collect();
        p.text(pos2(r.right() - 10.0, r.center().y), egui::Align2::RIGHT_CENTER, names.join(", "), theme::ui_font(11.0), t.text_dim).left()
    } else {
        r.right()
    };
    let clip = Rect::from_min_max(pos2(kind.right() + 6.0, r.top()), pos2(right - 8.0, r.bottom()));
    let font = if on { theme::ui_bold(12.5) } else { theme::ui_font(12.5) };
    p.with_clip_rect(clip).text(pos2(clip.left(), r.center().y), egui::Align2::LEFT_CENTER, &fname, font, t.text);
    if resp.drag_started() {
        DragAndDrop::set_payload(ui.ctx(), FieldDrag { field: fname.clone(), from: None, index: 0 });
    }
    if resp.clicked() {
        let res = if on {
            app.run("pivot.removeField", json!({"pivot": pivot, "field": fname}))
        } else {
            app.run("pivot.addField", json!({"pivot": pivot, "field": fname, "area": if numeric { "values" } else { "rows" }}))
        };
        if let Err(e) = res {
            app.message = Some(("PivotTable".into(), crate::clean_error(&e)));
        }
    }
    let samples: Vec<&str> = f["samples"].as_array().map(|a| a.iter().filter_map(Json::as_str).collect()).unwrap_or_default();
    if !samples.is_empty() && !resp.dragged() {
        resp.on_hover_text(format!("{}…", samples.join(", ")));
    }
}

fn area_label(key: &str) -> &'static str {
    AREAS.iter().find(|(k, ..)| *k == key).map_or("", |(_, l, _)| l)
}

/// The fields in an area, in order, with Σ Values where it sits.
fn area_entries(info: &Json, key: &str) -> Vec<Entry> {
    let list = info["pivot"][key].as_array().or(info[key].as_array());
    let mut v: Vec<Entry> = list
        .map(|a| {
            a.iter()
                .filter_map(|x| {
                    let name = x["name"].as_str().or(x["field"].as_str()).or(x["sourceCol"].as_str()).or(x.as_str())?;
                    Some(Entry { name: name.to_string(), real: true, func: x["func"].as_str().map(str::to_string) })
                })
                .collect()
        })
        .unwrap_or_default();
    let values_key = if info["valuesOnRows"].as_bool().unwrap_or(false) { "rows" } else { "columns" };
    let values = info["values"].as_array().map_or(0, Vec::len);
    if key == values_key && values > 1 {
        let at = info["valuesPosition"].as_u64().map_or(v.len(), |p| p as usize).min(v.len());
        v.insert(at, Entry { name: VALUES_FIELD.to_string(), real: false, func: None });
    }
    v
}

/// One area: its name over a framed drop zone holding its fields as chips.
#[allow(clippy::too_many_arguments)]
fn area_card(
    app: &mut SheetApp,
    ui: &mut Ui,
    pivot: &str,
    key: &'static str,
    label: &str,
    icon: Icon,
    entries: &[Entry],
    drag: Option<&FieldDrag>,
    lang: Language,
    t: &Tokens,
) -> Zone {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
        icons::paint(ui.painter(), r, icon, t.text_dim);
        ui.label(egui::RichText::new(label).font(theme::ui_bold(12.0)).color(t.text));
    });
    let h = (entries.len() as f32 * (CHIP_H + 4.0) + 8.0).max(76.0);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, t.input_bg);
    ui.painter().rect_stroke(rect, 0.0, Stroke::new(1.0, t.input_border), egui::StrokeKind::Inside);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink(4.0)).layout(egui::Layout::top_down(egui::Align::Min)));
    child.spacing_mut().item_spacing.y = 4.0;
    if entries.is_empty() {
        child.painter().text(rect.center(), egui::Align2::CENTER_CENTER, lang.tr("Drop fields here"), theme::ui_font(11.5), t.text_disabled);
    }
    let mut chips = Vec::new();
    let mut real_index = 0;
    for e in entries {
        let dragged = drag.is_some_and(|d| d.from == Some(key) && d.field == e.name);
        let resp = chip(&mut child, e, dragged, t);
        chips.push((resp.rect, e.real));
        if resp.drag_started() {
            DragAndDrop::set_payload(child.ctx(), FieldDrag { field: e.name.clone(), from: Some(key), index: real_index });
        }
        egui::Popup::menu(&resp).show(|ui| field_menu(app, ui, pivot, key, &e.name, !e.real, lang));
        if e.real {
            real_index += 1;
        }
    }
    ui.add_space(6.0);
    Zone { key, rect, chips }
}

/// A field in an area: grip, the value's function for a value field, the name, a menu chevron.
/// Screen readers read it as "Name ▾".
fn chip(ui: &mut Ui, e: &Entry, dragged: bool, t: &Tokens) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), CHIP_H), Sense::click_and_drag());
    let a11y = format!("{} ▾", e.name);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &a11y));
    let p = ui.painter();
    let fade = |c: egui::Color32| if dragged { c.gamma_multiply(0.35) } else { c };
    let fill = if resp.hovered() && !dragged { t.hover } else { t.ribbon };
    p.rect_filled(r, 3.0, fade(fill));
    p.rect_stroke(r, 3.0, Stroke::new(1.0, fade(if resp.hovered() { t.accent } else { t.input_border })), egui::StrokeKind::Inside);
    // The grip.
    for i in 0..3 {
        for j in 0..2 {
            p.circle_filled(pos2(r.left() + 7.0 + 3.0 * j as f32, r.center().y - 3.0 + 3.0 * i as f32), 0.9, fade(t.text_dim));
        }
    }
    let mut x = r.left() + 16.0;
    if let Some(glyph) = e.func.as_deref().map(func_glyph) {
        let g = p.text(pos2(x, r.center().y), egui::Align2::LEFT_CENTER, glyph, theme::ui_bold(12.0), fade(t.accent));
        x = g.right() + 5.0;
    } else if !e.real {
        let g = p.text(pos2(x, r.center().y), egui::Align2::LEFT_CENTER, "Σ", theme::ui_bold(12.0), fade(t.accent));
        x = g.right() + 5.0;
    }
    let name = e.name.strip_prefix("Σ ").unwrap_or(&e.name);
    let chev = Rect::from_center_size(pos2(r.right() - 11.0, r.center().y), vec2(10.0, 10.0));
    icons::paint(p, chev, Icon::Chevron, fade(t.text_dim));
    let clip = Rect::from_min_max(pos2(x, r.top()), pos2(chev.left() - 4.0, r.bottom()));
    p.with_clip_rect(clip).text(pos2(x, r.center().y), egui::Align2::LEFT_CENTER, name, theme::ui_font(12.0), fade(t.text));
    if resp.hovered() && !dragged {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
    resp
}

/// A value field's function as one glyph.
fn func_glyph(func: &str) -> &'static str {
    match func {
        "sum" => "Σ",
        "count" | "countNums" => "#",
        "average" => "x̄",
        "max" => "↑",
        "min" => "↓",
        "product" => "Π",
        "stdDev" | "stdDevp" | "var" | "varp" => "σ",
        _ => "ƒ",
    }
}

/// A field's menu: move it, summarize or show a value, sort or group an axis field, remove it.
fn field_menu(app: &mut SheetApp, ui: &mut Ui, pivot: &str, key: &str, f: &str, is_values: bool, lang: Language) {
    for (to, l) in
        [("rows", "Move to Row Labels"), ("columns", "Move to Column Labels"), ("filters", "Move to Report Filter"), ("values", "Move to Values")]
    {
        if to != key && (!is_values || matches!(to, "rows" | "columns")) && ui.button(lang.tr(l)).clicked() {
            app.run_or_alert("pivot.moveField", json!({"pivot": pivot, "field": f, "from": key, "to": to}));
        }
    }
    if is_values {
        return;
    }
    ui.separator();
    if key == "values" {
        for (func, l) in [
            ("sum", "Summarize by Sum"),
            ("count", "Summarize by Count"),
            ("average", "Summarize by Average"),
            ("max", "Summarize by Max"),
            ("min", "Summarize by Min"),
        ] {
            if ui.button(lang.tr(l)).clicked() {
                app.run_or_alert("pivot.valueSettings", json!({"pivot": pivot, "field": f, "func": func}));
            }
        }
        for (sa, l) in [
            ("normal", "Show as No Calculation"),
            ("percentOfGrandTotal", "Show as % of Grand Total"),
            ("runningTotal", "Show as Running Total"),
            ("rank", "Show as Rank"),
        ] {
            if ui.button(lang.tr(l)).clicked() {
                app.run_or_alert("pivot.valueSettings", json!({"pivot": pivot, "field": f, "showAs": sa}));
            }
        }
    } else {
        for (o, l) in [("asc", "Sort A to Z"), ("desc", "Sort Z to A")] {
            if ui.button(lang.tr(l)).clicked() {
                app.run_or_alert("pivot.sort", json!({"pivot": pivot, "field": f, "order": o}));
            }
        }
        for (g, l) in [("years", "Group by Years"), ("quarters", "Group by Quarters"), ("months", "Group by Months"), ("none", "Ungroup")] {
            if ui.button(lang.tr(l)).clicked() {
                app.run_or_alert("pivot.group", json!({"pivot": pivot, "field": f, "by": g}));
            }
        }
    }
    ui.separator();
    if ui.button(lang.tr("Remove Field")).clicked() {
        app.run_or_alert("pivot.removeField", json!({"pivot": pivot, "field": f, "area": key}));
    }
}

/// Where a drag at `at` would land: the area under the pointer, at the gap nearest to it; out of
/// the pane (a field taken from an area) removes it.
fn drop_target(zones: &[Zone], pane: Rect, at: egui::Pos2, drag: &FieldDrag) -> Option<Target> {
    if let Some(z) = zones.iter().find(|z| z.rect.contains(at)) {
        let shown = z.chips.iter().filter(|(r, _)| r.center().y < at.y).count();
        let real = z.chips.iter().take(shown).filter(|(_, real)| *real).count();
        return Some(Target::Area(z.key, real));
    }
    (drag.from.is_some() && drag.field != VALUES_FIELD && !pane.contains(at)).then_some(Target::Remove)
}

/// The command a drop runs (and the preview shows), or `None` when it would change nothing or
/// isn't allowed (Σ Values outside Rows and Columns).
fn drop_command(info: &Json, pivot: &str, drag: &FieldDrag, target: Target) -> Option<(&'static str, Json)> {
    let f = drag.field.as_str();
    match target {
        Target::Remove => Some(("pivot.removeField", json!({"pivot": pivot, "field": f, "area": drag.from?}))),
        Target::Area(to, pos) => {
            if f == VALUES_FIELD && !matches!(to, "rows" | "columns") {
                return None;
            }
            match drag.from {
                Some(from) if from == to && f == VALUES_FIELD => {
                    // Σ Values isn't one of the real fields its position counts.
                    (pos != drag.index).then(|| ("pivot.moveField", json!({"pivot": pivot, "field": f, "from": from, "to": to, "position": pos})))
                }
                Some(from) if from == to => {
                    // Reorder: the gaps on either side of the field leave it where it is.
                    if pos == drag.index || pos == drag.index + 1 {
                        return None;
                    }
                    let pos = if pos > drag.index { pos - 1 } else { pos };
                    Some(("pivot.moveField", json!({"pivot": pivot, "field": f, "from": from, "to": to, "position": pos})))
                }
                Some(from) => Some(("pivot.moveField", json!({"pivot": pivot, "field": f, "from": from, "to": to, "position": pos}))),
                None => {
                    // From the list: an axis field already in Rows, Columns or Filters moves; a
                    // value field can be added again.
                    let areas: Vec<&str> = info["fields"]
                        .as_array()
                        .and_then(|l| l.iter().find(|x| x["name"].as_str() == Some(f)))
                        .and_then(|x| x["areas"].as_array())
                        .map(|a| a.iter().filter_map(Json::as_str).collect())
                        .unwrap_or_default();
                    if to != "values"
                        && let Some(from) = areas.iter().copied().find(|a| *a != "values")
                    {
                        if from == to {
                            return None;
                        }
                        return Some(("pivot.moveField", json!({"pivot": pivot, "field": f, "from": from, "to": to, "position": pos})));
                    }
                    Some(("pivot.addField", json!({"pivot": pivot, "field": f, "area": to, "position": pos})))
                }
            }
        }
    }
}

/// The drag's feedback: the target area outlined in the accent with a line where the field would
/// go, and the field itself under the pointer (struck through when the drop would remove it).
fn paint_drag(ctx: &egui::Context, zones: &[Zone], target: Option<Target>, allowed: bool, at: egui::Pos2, drag: &FieldDrag, t: &Tokens) {
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("mkmatrix.pivot_pane.drop")));
    if let Some(Target::Area(key, pos)) = target
        && let Some(z) = zones.iter().find(|z| z.key == key)
    {
        let ink = if allowed { t.accent } else { t.text_disabled };
        p.rect_filled(z.rect, 0.0, t.accent_soft.gamma_multiply(if allowed { 0.5 } else { 0.0 }));
        p.rect_stroke(z.rect, 0.0, Stroke::new(1.5, ink), egui::StrokeKind::Inside);
        if allowed {
            // The gap: before the pos-th real chip, or after the last one.
            let mut seen = 0;
            let mut y = None;
            for (r, real) in &z.chips {
                if seen == pos && (*real || drag.field == VALUES_FIELD) {
                    y = Some(r.top() - 2.0);
                    break;
                }
                if *real {
                    seen += 1;
                }
            }
            let y = y.unwrap_or_else(|| z.chips.last().map_or(z.rect.top() + 6.0, |(r, _)| r.bottom() + 2.0));
            p.line_segment([pos2(z.rect.left() + 6.0, y), pos2(z.rect.right() - 6.0, y)], Stroke::new(2.0, t.accent));
        }
    }
    let ghost = egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("mkmatrix.pivot_pane.ghost"));
    let gp = ctx.layer_painter(ghost);
    let name = drag.field.as_str();
    let galley = gp.layout_no_wrap(name.to_string(), theme::ui_font(12.0), t.text);
    let r = Rect::from_min_size(at + vec2(12.0, 8.0), vec2(galley.size().x + 20.0, CHIP_H));
    gp.rect_filled(r.translate(vec2(0.0, 2.0)), 3.0, t.shadow);
    gp.rect_filled(r, 3.0, t.window);
    let remove = target == Some(Target::Remove);
    gp.rect_stroke(r, 3.0, Stroke::new(1.0, if remove { t.danger } else { t.accent }), egui::StrokeKind::Inside);
    let text_pos = pos2(r.left() + 10.0, r.center().y - galley.size().y / 2.0);
    let w = galley.size().x;
    gp.galley(text_pos, galley, t.text);
    if remove {
        gp.line_segment([pos2(r.left() + 8.0, r.center().y), pos2(r.left() + 12.0 + w, r.center().y)], Stroke::new(1.2, t.danger));
    }
    ctx.set_cursor_icon(if allowed { egui::CursorIcon::Grabbing } else { egui::CursorIcon::NotAllowed });
}
