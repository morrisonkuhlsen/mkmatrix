//! Home › Conditional Formatting: one panel in the dialogs' style instead of cascading text
//! menus. A header naming the selection; Highlight Cells and Top/Bottom rules as rows with a
//! glyph each; Data Bars, Color Scales and Icon Sets as tiles that show what they draw; then New
//! Rule, Clear Rules and Manage Rules. Every item runs the same command (or opens the same
//! dialog) the menus did.

use egui::{Color32, Painter, Rect, Response, Sense, Stroke, Ui, pos2, vec2};
use serde_json::{Value as Json, json};

use super::controls::{act, menu_header, menu_section};
use crate::SheetApp;
use crate::i18n::Language;
use crate::icons::{self, Icon};
use crate::theme::{self, Tokens};

/// The panel's width inside its margins.
const W: f32 = 336.0;
/// A rule row's height.
const ROW_H: f32 = 26.0;
/// A tile's size (data bars, colour scales, icon sets): six to a row.
const TILE: egui::Vec2 = vec2(50.0, 34.0);

/// What a rule row shows before its label.
#[derive(Clone, Copy)]
enum Glyph {
    Icon(Icon),
    Text(&'static str),
    /// Top or bottom, by count (`false`) or percent (`true`).
    Rank(bool, bool),
    /// Above or below the average.
    Average(bool),
}

/// What an item does on a click.
enum Action {
    Dialog(&'static str, Json),
    Command(&'static str, Json),
}

const DATA_BARS: [(&str, &str); 6] =
    [("Blue", "#638EC6"), ("Green", "#63BE7B"), ("Red", "#F8696B"), ("Orange", "#FFB628"), ("Light Blue", "#008AEF"), ("Purple", "#D6007B")];
const COLOR_SCALES: [(&str, &[&str]); 6] = [
    ("Green - Yellow - Red", &["#63BE7B", "#FFEB84", "#F8696B"]),
    ("Red - Yellow - Green", &["#F8696B", "#FFEB84", "#63BE7B"]),
    ("Green - White - Red", &["#63BE7B", "#FCFCFF", "#F8696B"]),
    ("Blue - White - Red", &["#5A8AC6", "#FCFCFF", "#F8696B"]),
    ("White - Green", &["#FCFCFF", "#63BE7B"]),
    ("White - Red", &["#FCFCFF", "#F8696B"]),
];
const ICON_SETS: [(&str, &str); 6] = [
    ("3 Arrows", "3Arrows"),
    ("3 Traffic Lights", "3TrafficLights1"),
    ("3 Symbols", "3Symbols"),
    ("4 Ratings", "4Rating"),
    ("5 Arrows", "5Arrows"),
    ("5 Quarters", "5Quarters"),
];

/// The Conditional Formatting panel (inside the ribbon button's popup).
pub(super) fn cf_menu(app: &mut SheetApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let lang = app.ui.language;
    ui.set_width(W);
    ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
    let mut chosen: Option<Action> = None;
    // The header: what the rules will apply to.
    menu_header(app, ui, Icon::CondFormat, "Conditional Formatting", true);
    menu_section(ui, lang.tr("Highlight Cells Rules"));
    let highlight: [(&str, Glyph, Action); 7] = [
        ("Greater Than…", Glyph::Text(">"), Action::Dialog("cfQuick", json!({"type": "cellIs", "operator": "greater", "title": "Greater Than…"}))),
        ("Less Than…", Glyph::Text("<"), Action::Dialog("cfQuick", json!({"type": "cellIs", "operator": "less", "title": "Less Than…"}))),
        ("Between…", Glyph::Text("a–b"), Action::Dialog("cfQuick", json!({"type": "cellIs", "operator": "between", "title": "Between…"}))),
        ("Equal To…", Glyph::Text("="), Action::Dialog("cfQuick", json!({"type": "cellIs", "operator": "equal", "title": "Equal To…"}))),
        ("Text that Contains…", Glyph::Text("Abc"), Action::Dialog("cfQuick", json!({"type": "containsText", "title": "Text that Contains"}))),
        ("A Date Occurring…", Glyph::Icon(Icon::Calendar), Action::Dialog("cfQuick", json!({"type": "timePeriod", "title": "A Date Occurring"}))),
        ("Duplicate Values…", Glyph::Icon(Icon::Duplicates), Action::Command("home.conditionalFormat", json!({"rule": {"type": "duplicate"}}))),
    ];
    rows(ui, lang, &t, highlight, &mut chosen);
    menu_section(ui, lang.tr("Top/Bottom Rules"));
    let top: [(&str, Glyph, Action); 6] = [
        ("Top 10 Items", Glyph::Rank(true, false), Action::Command("home.conditionalFormat", json!({"rule": {"type": "top10", "rank": 10}}))),
        (
            "Top 10%",
            Glyph::Rank(true, true),
            Action::Command("home.conditionalFormat", json!({"rule": {"type": "top10", "rank": 10, "percent": true}})),
        ),
        (
            "Bottom 10 Items",
            Glyph::Rank(false, false),
            Action::Command("home.conditionalFormat", json!({"rule": {"type": "top10", "rank": 10, "bottom": true}})),
        ),
        (
            "Bottom 10%",
            Glyph::Rank(false, true),
            Action::Command("home.conditionalFormat", json!({"rule": {"type": "top10", "rank": 10, "bottom": true, "percent": true}})),
        ),
        ("Above Average", Glyph::Average(true), Action::Command("home.conditionalFormat", json!({"rule": {"type": "aboveAverage"}}))),
        ("Below Average", Glyph::Average(false), Action::Command("home.conditionalFormat", json!({"rule": {"type": "aboveAverage", "below": true}}))),
    ];
    rows(ui, lang, &t, top, &mut chosen);
    menu_section(ui, lang.tr("Data Bars"));
    ui.horizontal(|ui| {
        for (label, c) in DATA_BARS {
            if tile(ui, lang.tr(label), &t, |p, r| paint_bars(p, r, hex(c))).clicked() {
                chosen = Some(Action::Command("home.conditionalFormat", json!({"rule": {"type": "dataBar", "color": c}})));
            }
        }
    });
    menu_section(ui, lang.tr("Color Scales"));
    ui.horizontal(|ui| {
        for (label, stops) in COLOR_SCALES {
            if tile(ui, lang.tr(label), &t, |p, r| paint_scale(p, r, stops)).clicked() {
                chosen = Some(Action::Command("home.conditionalFormat", json!({"rule": {"type": "colorScale", "colors": stops}})));
            }
        }
    });
    menu_section(ui, lang.tr("Icon Sets"));
    ui.horizontal(|ui| {
        for (label, set) in ICON_SETS {
            if tile(ui, lang.tr(label), &t, |p, r| paint_icons(p, r, set)).clicked() {
                chosen = Some(Action::Command("home.conditionalFormat", json!({"rule": {"type": "iconSet", "set": set}})));
            }
        }
    });
    rule(ui, &t);
    let manage: [(&str, Glyph, Action); 2] = [
        ("New Rule…", Glyph::Icon(Icon::Plus), Action::Dialog("cfQuick", json!({"type": "expression", "title": "New Formatting Rule"}))),
        ("Manage Rules…", Glyph::Icon(Icon::Settings), Action::Dialog("manageRules", json!({}))),
    ];
    rows(ui, lang, &t, manage, &mut chosen);
    menu_section(ui, lang.tr("Clear Rules"));
    let clear: [(&str, Glyph, Action); 2] = [
        ("Selected Cells", Glyph::Icon(Icon::Eraser), Action::Command("home.clearRules", json!({}))),
        ("Entire Sheet", Glyph::Icon(Icon::Eraser), Action::Command("home.clearRules", json!({"sheet": true}))),
    ];
    rows(ui, lang, &t, clear, &mut chosen);
    match chosen {
        Some(Action::Dialog(id, p)) => {
            app.open_dialog(id, p);
            ui.close();
        }
        Some(Action::Command(id, p)) => {
            act(app, id, p);
            ui.close();
        }
        None => {}
    }
}

/// A rule across the panel.
fn rule(ui: &mut Ui, t: &Tokens) {
    let (r, _) = ui.allocate_exact_size(vec2(W, 9.0), Sense::hover());
    ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, t.separator));
}

/// Items as rows, two to a line.
fn rows<const N: usize>(ui: &mut Ui, lang: Language, t: &Tokens, items: [(&str, Glyph, Action); N], chosen: &mut Option<Action>) {
    let w = (W - 6.0) / 2.0;
    let mut items = items.into_iter();
    while let Some(first) = items.next() {
        let second = items.next();
        ui.horizontal(|ui| {
            for (label, glyph, action) in std::iter::once(first).chain(second) {
                if item_row(ui, lang.tr(label), glyph, w, t).clicked() {
                    *chosen = Some(action);
                }
            }
        });
    }
}

/// A rule: its glyph in the accent, on no background, then its name.
fn item_row(ui: &mut Ui, label: &str, glyph: Glyph, w: f32, t: &Tokens) -> Response {
    let (r, resp) = ui.allocate_exact_size(vec2(w, ROW_H), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::MenuItem, true, label));
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(r, 4.0, t.hover);
    }
    let g = Rect::from_center_size(pos2(r.left() + 14.0, r.center().y), vec2(22.0, 18.0));
    let ink = t.accent;
    match glyph {
        Glyph::Icon(icon) => icons::paint(p, Rect::from_center_size(g.center(), vec2(14.0, 14.0)), icon, ink),
        Glyph::Text(s) => {
            p.text(g.center(), egui::Align2::CENTER_CENTER, s, theme::ui_bold(if s.chars().count() > 1 { 9.5 } else { 12.5 }), ink);
        }
        Glyph::Rank(top, pct) => {
            triangle(p, pos2(g.left() + 6.0, g.center().y), 3.5, if top { 90.0 } else { -90.0 }, ink);
            p.text(pos2(g.left() + 10.5, g.center().y), egui::Align2::LEFT_CENTER, if pct { "%" } else { "10" }, theme::ui_bold(8.5), ink);
        }
        Glyph::Average(above) => {
            p.hline(g.left() + 4.0..=g.right() - 4.0, g.center().y, Stroke::new(1.2, ink));
            let y = if above { g.center().y - 4.0 } else { g.center().y + 4.0 };
            triangle(p, pos2(g.center().x, y), 3.0, if above { 90.0 } else { -90.0 }, ink);
        }
    }
    let clip = Rect::from_min_max(pos2(g.right() + 7.0, r.top()), pos2(r.right() - 4.0, r.bottom()));
    p.with_clip_rect(clip).text(pos2(clip.left(), r.center().y), egui::Align2::LEFT_CENTER, label, theme::ui_font(12.5), t.text);
    resp
}

/// A tile showing what a data bar, colour scale or icon set draws; its name on hover.
fn tile(ui: &mut Ui, label: &str, t: &Tokens, paint: impl FnOnce(&Painter, Rect)) -> Response {
    let (r, resp) = ui.allocate_exact_size(TILE, Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::MenuItem, true, label));
    let p = ui.painter();
    p.rect_filled(r, 4.0, t.input_bg);
    paint(p, r.shrink(5.0));
    let stroke = if resp.hovered() { Stroke::new(1.5, t.accent) } else { Stroke::new(1.0, t.input_border) };
    p.rect_stroke(r, 4.0, stroke, egui::StrokeKind::Inside);
    resp.on_hover_text(label)
}

/// Three bars of different lengths, each fading from `c` to white as a data bar does.
fn paint_bars(p: &Painter, r: Rect, c: Color32) {
    let h = r.height() / 3.0;
    for (i, share) in [0.92_f32, 0.6, 0.32].into_iter().enumerate() {
        let bar = Rect::from_min_size(pos2(r.left(), r.top() + i as f32 * h + 1.0), vec2(r.width() * share, h - 2.0));
        gradient(p, bar, &[c, lerp(c, Color32::WHITE, 0.85)]);
    }
}

/// The scale's colours across, as a strip.
fn paint_scale(p: &Painter, r: Rect, stops: &[&str]) {
    let colors: Vec<Color32> = stops.iter().map(|s| hex(s)).collect();
    gradient(p, r, &colors);
}

/// The set's icons in a row.
fn paint_icons(p: &Painter, r: Rect, set: &str) {
    let (green, yellow, red, grey) = (hex("#63BE7B"), hex("#FFB628"), hex("#F8696B"), Color32::from_gray(110));
    let n = if set.starts_with('5') {
        5
    } else if set.starts_with('4') {
        4
    } else {
        3
    };
    let step = r.width() / n as f32;
    let at = |i: usize| pos2(r.left() + step * (i as f32 + 0.5), r.center().y);
    let s = (step * 0.42).min(r.height() * 0.4);
    match set {
        "3Arrows" => {
            for (i, (angle, c)) in [(90.0, green), (0.0, yellow), (-90.0, red)].into_iter().enumerate() {
                triangle(p, at(i), s, angle, c);
            }
        }
        "5Arrows" => {
            let cs = [green, lerp(green, yellow, 0.5), yellow, lerp(yellow, red, 0.5), red];
            for (i, c) in cs.into_iter().enumerate() {
                triangle(p, at(i), s, 90.0 - 45.0 * i as f32, c);
            }
        }
        "3TrafficLights1" => {
            for (i, c) in [green, yellow, red].into_iter().enumerate() {
                p.circle_filled(at(i), s, c);
            }
        }
        "3Symbols" => {
            for (i, c) in [green, yellow, red].into_iter().enumerate() {
                let o = at(i);
                p.circle_filled(o, s, c);
                let w = Stroke::new(1.3, Color32::WHITE);
                let k = s * 0.5;
                match i {
                    0 => {
                        p.line(vec![o + vec2(-k, 0.0), o + vec2(-k * 0.2, k * 0.7), o + vec2(k, -k * 0.6)], w);
                    }
                    1 => {
                        p.line_segment([o + vec2(0.0, -k), o + vec2(0.0, k * 0.3)], w);
                        p.circle_filled(o + vec2(0.0, k * 0.8), 0.8, Color32::WHITE);
                    }
                    _ => {
                        p.line_segment([o + vec2(-k, -k), o + vec2(k, k)], w);
                        p.line_segment([o + vec2(-k, k), o + vec2(k, -k)], w);
                    }
                }
            }
        }
        "4Rating" => {
            // A signal-bars glyph per level, from all four bars inked down to one.
            for i in 0..4 {
                let o = at(i);
                for b in 0..4 {
                    let bh = s * 0.45 * (b + 1) as f32;
                    let x = o.x - s * 0.9 + b as f32 * s * 0.5;
                    let bar = Rect::from_min_max(pos2(x, o.y + s - bh), pos2(x + s * 0.35, o.y + s));
                    p.rect_filled(bar, 0.0, if b <= 3 - i { hex("#2F5D62") } else { Color32::from_gray(200) });
                }
            }
        }
        _ => {
            // 5 Quarters: a circle filled from empty to full.
            for i in 0..5 {
                let o = at(i);
                p.circle_stroke(o, s, Stroke::new(1.0, grey));
                let share = 1.0 - i as f32 / 4.0;
                if share > 0.0 {
                    let steps = (24.0 * share).ceil() as usize;
                    let mut pts = vec![o];
                    for k in 0..=steps {
                        let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * share * k as f32 / steps as f32;
                        pts.push(o + vec2(a.cos(), a.sin()) * s);
                    }
                    p.add(egui::Shape::convex_polygon(pts, grey, Stroke::NONE));
                }
            }
        }
    }
}

/// A filled triangle pointing at `angle` degrees (90: up), centred on `c`.
fn triangle(p: &Painter, c: egui::Pos2, s: f32, angle: f32, color: Color32) {
    let a = angle.to_radians();
    let dir = vec2(a.cos(), -a.sin());
    let side = vec2(-dir.y, dir.x);
    let pts = vec![c + dir * s, c - dir * s * 0.7 + side * s * 0.85, c - dir * s * 0.7 - side * s * 0.85];
    p.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
}

/// `r` filled left to right through `stops`.
fn gradient(p: &Painter, r: Rect, stops: &[Color32]) {
    let mut mesh = egui::Mesh::default();
    let n = stops.len().max(2) - 1;
    for (i, c) in stops.iter().enumerate() {
        let x = r.left() + r.width() * i as f32 / n as f32;
        mesh.colored_vertex(pos2(x, r.top()), *c);
        mesh.colored_vertex(pos2(x, r.bottom()), *c);
        if i > 0 {
            let k = (i as u32) * 2;
            mesh.add_triangle(k - 2, k - 1, k);
            mesh.add_triangle(k - 1, k + 1, k);
        }
    }
    p.add(egui::Shape::mesh(mesh));
}

fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let ch = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(ch(a.r(), b.r()), ch(a.g(), b.g()), ch(a.b(), b.b()))
}

/// `#RRGGBB` as a colour (grey when it isn't one).
fn hex(s: &str) -> Color32 {
    u32::from_str_radix(s.trim_start_matches('#'), 16).map(|v| Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)).unwrap_or(Color32::GRAY)
}
