use super::show::ok_cancel;
use super::*;

use egui::pos2;
use mkmatrix_engine::model::{BorderLine, BorderStyle, Color, Fill, HAlign, Style, Underline, VAlign, VertAlign};

use crate::theme::Tokens;

/// The sections, as `d.tab` names them (the English name `open_dialog` and tests pass), with the
/// key their label translates by (the Fill section's word differs from Alignment's Fill in some
/// languages) and their icon.
const SECTIONS: [(&str, &str, Icon); 6] = [
    ("Number", "Number", Icon::Percent),
    ("Alignment", "Alignment", Icon::AlignLeft),
    ("Font", "Font", Icon::FontColor),
    ("Border", "Border", Icon::Borders),
    ("Fill", "Format Cells|Fill", Icon::Fill),
    ("Protection", "Protection", Icon::Lock),
];

/// The content area's height: every section the same size, so the window never jumps.
const BODY_H: f32 = 360.0;
/// The navigation column's width.
const NAV_W: f32 = 168.0;
/// The form's label column.
const LABEL_W: f32 = 112.0;

/// Draws Format Cells: a navigation column of sections beside one form at a time, laid out on a
/// fixed grid (labels left, controls right), and a footer with a live preview of the active cell
/// and Cancel / OK. Edits change a style draft; OK applies it with `home.formatCells`.
pub(super) fn format_cells(app: &mut SheetApp, ui: &mut egui::Ui, d: &mut Dialog, confirm: &mut bool) {
    let lang = app.ui.language;
    let t = Tokens::get(ui.ctx());
    let mut st: Style = d.values.get("style").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
    let sample = app.session.active().and_then(|doc| doc.wb.active().map(|s| s.value(doc.selection.active))).unwrap_or_default();
    let theme = app.session.active().map(|doc| doc.wb.theme.clone()).unwrap_or_default();
    ui.horizontal_top(|ui| {
        // Navigation.
        ui.vertical(|ui| {
            ui.set_width(NAV_W);
            ui.set_min_height(BODY_H);
            ui.spacing_mut().item_spacing.y = 2.0;
            for (tab, key, icon) in SECTIONS {
                if nav_item(ui, icon, lang.tr(key), d.tab == tab, &t).clicked() {
                    d.tab = tab.to_string();
                }
            }
        });
        let (line, _) = ui.allocate_exact_size(vec2(1.0, BODY_H), egui::Sense::hover());
        ui.painter().rect_filled(line, 0.0, t.separator);
        ui.add_space(16.0);
        // The section's form.
        ui.vertical(|ui| {
            ui.set_min_height(BODY_H);
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 8.0;
            match d.tab.as_str() {
                "Number" => number(app, ui, &mut st, &sample, &t),
                "Alignment" => alignment(app, ui, &mut st),
                "Font" => font(app, ui, &mut st, &theme.colors, &t),
                "Border" => border(app, ui, &mut st, &theme, &t),
                "Fill" => fill(app, ui, &mut st, &theme.colors),
                "Protection" => protection(app, ui, &mut st),
                _ => {}
            }
        });
    });
    if let Ok(v) = serde_json::to_value(&st) {
        d.values.insert("style".into(), v);
    }
    // Footer: the active cell as it will look, then Cancel / OK.
    ui.add_space(6.0);
    let (rule, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().rect_filled(rule, 0.0, t.separator);
    ui.add_space(8.0);
    let mut open = true;
    ui.horizontal(|ui| {
        cell_preview(app, ui, &st, &sample, &theme, &t);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ok_cancel(ui, confirm, &mut open);
        });
    });
    if *confirm && let Some(style) = d.values.get("style").cloned() {
        app.run_or_alert("home.formatCells", json!({"style": style}));
    }
    if !open {
        app.dialog = None;
        *confirm = true; // closes
    }
}

// ------------------------------------------------------------------------------------ pieces

/// A navigation row: icon and name, the selected one tinted with a bar of the accent.
fn nav_item(ui: &mut egui::Ui, icon: Icon, label: &str, selected: bool, t: &Tokens) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(NAV_W, 34.0), egui::Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, selected, label));
    let p = ui.painter();
    if selected {
        p.rect_filled(r, 0.0, t.accent_soft);
        p.rect_filled(egui::Rect::from_min_size(r.min, vec2(3.0, r.height())), 0.0, t.accent);
    } else if resp.hovered() {
        p.rect_filled(r, 0.0, t.hover);
    }
    let ink = if selected { t.accent } else { t.text };
    icons::paint(p, egui::Rect::from_center_size(pos2(r.left() + 22.0, r.center().y), vec2(16.0, 16.0)), icon, ink);
    let font = if selected { theme::ui_bold(13.0) } else { theme::ui_font(13.0) };
    p.text(pos2(r.left() + 42.0, r.center().y), egui::Align2::LEFT_CENTER, label, font, ink);
    resp
}

/// A section's heading: small, muted, with room above.
fn heading(ui: &mut egui::Ui, text: &str) {
    let t = Tokens::get(ui.ctx());
    ui.add_space(2.0);
    ui.label(egui::RichText::new(text).font(theme::ui_bold(11.5)).color(t.text_dim));
}

/// A form of labelled rows: labels in a column of their own, controls lined up beside them.
fn form(ui: &mut egui::Ui, id: &str, add: impl FnOnce(&mut egui::Ui)) {
    egui::Grid::new(id).num_columns(2).min_col_width(LABEL_W).spacing([12.0, 10.0]).show(ui, add);
}

/// A row's label; its id names the row's control for screen readers.
fn label(ui: &mut egui::Ui, text: &str) -> egui::Id {
    let t = Tokens::get(ui.ctx());
    ui.label(egui::RichText::new(text).font(theme::ui_font(13.0)).color(t.text_dim)).id
}

/// One choice of a segmented control: joined buttons of one height, the chosen one filled.
fn segment(ui: &mut egui::Ui, text: &str, on: bool, width: f32, a11y: &str) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(vec2(width, 28.0), egui::Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, a11y));
    let p = ui.painter();
    p.rect_filled(
        r,
        0.0,
        if on {
            t.accent_soft
        } else if resp.hovered() {
            t.hover
        } else {
            t.input_bg
        },
    );
    p.rect_stroke(r, 0.0, egui::Stroke::new(1.0, if on { t.accent } else { t.input_border }), egui::StrokeKind::Inside);
    p.text(r.center(), egui::Align2::CENTER_CENTER, text, theme::ui_font(12.5), if on { t.accent } else { t.text });
    resp
}

/// A drop-down of named choices bound to `value`.
fn choice<T: PartialEq + Copy>(ui: &mut egui::Ui, id: &str, label: egui::Id, value: &mut T, options: &[(T, &str)], lang: crate::i18n::Language) {
    let current = options.iter().find(|(v, _)| v == value).map_or("", |(_, n)| *n);
    craft_ui::ComboBox::from_id_salt(id)
        .width(220.0)
        .selected_text(lang.tr(current))
        .show_ui(ui, |ui| {
            for (v, n) in options {
                ui.selectable_value(value, *v, lang.tr(n));
            }
        })
        .response
        .labelled_by(label);
}

/// A colour swatch that opens the palette; `None` in the palette is `none_label`.
fn swatch(ui: &mut egui::Ui, id: &str, color: Option<egui::Color32>, colors: &[u32; 12], none_label: &str, t: &Tokens) -> Option<String> {
    let (r, resp) = ui.allocate_exact_size(vec2(64.0, 26.0), egui::Sense::click());
    let p = ui.painter();
    p.rect_filled(r, 0.0, t.input_bg);
    p.rect_stroke(r, 0.0, egui::Stroke::new(1.0, if resp.hovered() { t.accent } else { t.input_border }), egui::StrokeKind::Inside);
    let chip = egui::Rect::from_min_size(r.min + vec2(5.0, 5.0), vec2(36.0, 16.0));
    match color {
        Some(c) => {
            p.rect_filled(chip, 0.0, c);
        }
        None => {
            p.rect_stroke(chip, 0.0, egui::Stroke::new(1.0, t.text_dim), egui::StrokeKind::Inside);
            p.line_segment([chip.left_bottom(), chip.right_top()], egui::Stroke::new(1.0, t.danger));
        }
    }
    icons::paint(p, egui::Rect::from_center_size(pos2(r.right() - 12.0, r.center().y), vec2(9.0, 9.0)), Icon::Chevron, t.text_dim);
    let mut picked = None;
    egui::Popup::menu(&resp).id(egui::Id::unique(id)).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        if let Some(c) = crate::widgets::color_palette(ui, colors, none_label) {
            picked = Some(c);
            ui.close();
        }
    });
    picked
}

/// The active cell drawn with the draft style: its fill, borders, font, colour and alignment.
fn cell_preview(
    app: &SheetApp,
    ui: &mut egui::Ui,
    st: &Style,
    sample: &mkmatrix_engine::core::Value,
    theme: &mkmatrix_engine::model::Theme,
    t: &Tokens,
) {
    let (r, resp) = ui.allocate_exact_size(vec2(220.0, 34.0), egui::Sense::hover());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, "Cell preview"));
    let p = ui.painter();
    let bg = match st.fill.pattern {
        mkmatrix_engine::model::PatternType::Solid => st.fill.fg.resolve(theme).map(theme::color32).unwrap_or(t.grid_bg),
        _ => t.grid_bg,
    };
    p.rect_filled(r, 0.0, bg);
    p.rect_stroke(r, 0.0, egui::Stroke::new(1.0, t.gridline), egui::StrokeKind::Inside);
    let text = app.session.active().map(|doc| mkmatrix_engine::display::format(sample, st.num_fmt.as_str(), &doc.wb).text).unwrap_or_default();
    let text = if text.is_empty() { "123.45".to_string() } else { text };
    let size = st.font.size.clamp(8.0, 18.0);
    let family = match (st.font.bold, st.font.italic) {
        (true, _) => theme::CELL_BOLD,
        (false, true) => theme::CELL_ITALIC,
        _ => theme::CELL,
    };
    let font = egui::FontId::new(size, egui::FontFamily::Name(family.into()));
    // Automatic ink as the sheet draws it: the theme's on its own colour, black or white on a fill.
    let ink = st.font.color.resolve(theme).map(theme::color32).unwrap_or_else(|| {
        if bg == t.grid_bg {
            t.cell_text
        } else if 0.299 * f32::from(bg.r()) + 0.587 * f32::from(bg.g()) + 0.114 * f32::from(bg.b()) < 150.0 {
            egui::Color32::WHITE
        } else {
            egui::Color32::BLACK
        }
    });
    let (x, align) = match st.align.h {
        HAlign::Center | HAlign::CenterAcross => (r.center().x, egui::Align2::CENTER_CENTER),
        HAlign::Right => (r.right() - 6.0, egui::Align2::RIGHT_CENTER),
        HAlign::General if matches!(sample, mkmatrix_engine::core::Value::Number(_)) => (r.right() - 6.0, egui::Align2::RIGHT_CENTER),
        _ => (r.left() + 6.0, egui::Align2::LEFT_CENTER),
    };
    let at = p.with_clip_rect(r.shrink(1.0)).text(pos2(x, r.center().y), align, &text, font, ink);
    if st.font.strike {
        p.line_segment([pos2(at.left(), at.center().y), pos2(at.right(), at.center().y)], egui::Stroke::new(1.0, ink));
    }
    if st.font.underline != Underline::None {
        p.line_segment([pos2(at.left(), at.bottom() - 2.0), pos2(at.right(), at.bottom() - 2.0)], egui::Stroke::new(1.0, ink));
    }
    let edges = [
        (st.borders.top, [r.left_top(), r.right_top()]),
        (st.borders.bottom, [r.left_bottom(), r.right_bottom()]),
        (st.borders.left, [r.left_top(), r.left_bottom()]),
        (st.borders.right, [r.right_top(), r.right_bottom()]),
    ];
    for (line, pts) in edges {
        if !line.is_none() {
            let c = line.color.resolve(theme).map(theme::color32).unwrap_or(t.cell_text);
            p.line_segment(pts, egui::Stroke::new(if matches!(line.style, BorderStyle::Thick) { 2.5 } else { 1.0 }, c));
        }
    }
}

// ---------------------------------------------------------------------------------- sections

fn number(app: &SheetApp, ui: &mut egui::Ui, st: &mut Style, sample: &mkmatrix_engine::core::Value, t: &Tokens) {
    let lang = app.ui.language;
    let mut code = st.num_fmt.as_str().to_string();
    heading(ui, lang.tr("Category"));
    ui.horizontal_top(|ui| {
        // The categories, one column of rows.
        ui.vertical(|ui| {
            ui.set_width(150.0);
            ui.spacing_mut().item_spacing.y = 0.0;
            for name in
                ["General", "Number", "Currency", "Accounting", "Short Date", "Long Date", "Time", "Percentage", "Fraction", "Scientific", "Text"]
            {
                let c = mkmatrix_engine::cmd::format::format_code_for(name).to_string();
                let on = code == c;
                let label = lang.tr(name);
                let (r, resp) = ui.allocate_exact_size(vec2(150.0, 26.0), egui::Sense::click());
                resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, label));
                let p = ui.painter();
                if on {
                    p.rect_filled(r, 0.0, t.accent_soft);
                } else if resp.hovered() {
                    p.rect_filled(r, 0.0, t.hover);
                }
                p.text(
                    pos2(r.left() + 10.0, r.center().y),
                    egui::Align2::LEFT_CENTER,
                    label,
                    theme::ui_font(13.0),
                    if on { t.accent } else { t.text },
                );
                if resp.clicked() {
                    code = c;
                }
            }
        });
        ui.add_space(14.0);
        ui.vertical(|ui| {
            let _ = label(ui, lang.tr("Sample"));
            let preview = app.session.active().map(|doc| mkmatrix_engine::display::format(sample, &code, &doc.wb).text).unwrap_or_default();
            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), egui::Sense::hover());
            ui.painter().rect_filled(r, 0.0, t.input_bg);
            ui.painter().text(pos2(r.left() + 10.0, r.center().y), egui::Align2::LEFT_CENTER, preview, theme::ui_font(16.0), t.text);
            ui.add_space(6.0);
            let l = ui.label(egui::RichText::new(lang.tr("Format code")).font(theme::ui_font(13.0)).color(t.text_dim));
            ui.add(egui::TextEdit::singleline(&mut code).font(egui::FontId::monospace(13.0)).desired_width(f32::INFINITY)).labelled_by(l.id);
            ui.add_space(6.0);
            let _ = label(ui, lang.tr("Quick formats"));
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                for c in ["0", "0.00", "#,##0", "#,##0.00", "0%", "0.00%", "0.00E+00", "# ?/?", "d-mmm-yy", "h:mm:ss", "@"] {
                    let (r, resp) = ui.allocate_exact_size(
                        vec2(ui.painter().layout_no_wrap(c.into(), egui::FontId::monospace(12.0), t.text).size().x + 16.0, 24.0),
                        egui::Sense::click(),
                    );
                    let on = code == c;
                    ui.painter().rect_filled(
                        r,
                        0.0,
                        if on {
                            t.accent_soft
                        } else if resp.hovered() {
                            t.hover
                        } else {
                            t.input_bg
                        },
                    );
                    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, c, egui::FontId::monospace(12.0), if on { t.accent } else { t.text });
                    if resp.clicked() {
                        code = c.to_string();
                    }
                }
            });
        });
    });
    st.num_fmt = mkmatrix_engine::model::NumFmt::new(&code);
}

fn alignment(app: &SheetApp, ui: &mut egui::Ui, st: &mut Style) {
    let lang = app.ui.language;
    heading(ui, lang.tr("Text alignment"));
    form(ui, "fc_alignment", |ui| {
        let l = label(ui, lang.tr("Horizontal"));
        choice(
            ui,
            "fc_h",
            l,
            &mut st.align.h,
            &[
                (HAlign::General, "General"),
                (HAlign::Left, "Left"),
                (HAlign::Center, "Center"),
                (HAlign::Right, "Right"),
                (HAlign::Fill, "Fill"),
                (HAlign::Justify, "Justify"),
                (HAlign::CenterAcross, "Center Across Selection"),
                (HAlign::Distributed, "Distributed"),
            ],
            lang,
        );
        ui.end_row();
        let l = label(ui, lang.tr("Vertical"));
        choice(
            ui,
            "fc_v",
            l,
            &mut st.align.v,
            &[
                (VAlign::Top, "Top"),
                (VAlign::Center, "Center"),
                (VAlign::Bottom, "Bottom"),
                (VAlign::Justify, "Justify"),
                (VAlign::Distributed, "Distributed"),
            ],
            lang,
        );
        ui.end_row();
        let l = label(ui, lang.tr("Indent"));
        let mut indent = st.align.indent as f32;
        ui.add(egui::Slider::new(&mut indent, 0.0..=15.0).step_by(1.0)).labelled_by(l);
        st.align.indent = indent as u8;
        ui.end_row();
        let l = label(ui, lang.tr("Orientation (degrees)"));
        let mut rot = if st.align.rotation == 255 { 0.0 } else { st.align.rotation as f32 };
        ui.add(egui::Slider::new(&mut rot, -90.0..=90.0).step_by(1.0).suffix("°")).labelled_by(l);
        st.align.rotation = rot as i16;
        ui.end_row();
    });
    ui.add_space(6.0);
    heading(ui, lang.tr("Text control"));
    form(ui, "fc_control", |ui| {
        let _ = label(ui, "");
        ui.vertical(|ui| {
            ui.checkbox(&mut st.align.wrap, lang.tr("Wrap text"));
            ui.checkbox(&mut st.align.shrink, lang.tr("Shrink to fit"));
        });
        ui.end_row();
    });
}

fn font(app: &SheetApp, ui: &mut egui::Ui, st: &mut Style, colors: &[u32; 12], t: &Tokens) {
    let lang = app.ui.language;
    heading(ui, lang.tr("Typeface"));
    form(ui, "fc_font", |ui| {
        let l = label(ui, lang.tr("Font"));
        craft_ui::ComboBox::from_id_salt("fc_family")
            .width(220.0)
            .selected_text(st.font.name.clone())
            .show_ui(ui, |ui| {
                for f in crate::fonts::families() {
                    ui.selectable_value(&mut st.font.name, f.clone(), f.clone());
                }
            })
            .response
            .labelled_by(l);
        ui.end_row();
        let l = label(ui, lang.tr("Size"));
        ui.add(egui::DragValue::new(&mut st.font.size).range(1.0..=409.0).speed(0.5).suffix(" pt")).labelled_by(l);
        ui.end_row();
        let _ = label(ui, lang.tr("Style"));
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for (on, text, name) in
                [(&mut st.font.bold, "B", "Bold"), (&mut st.font.italic, "I", "Italic"), (&mut st.font.strike, "S", "Strikethrough")]
            {
                if segment(ui, text, *on, 36.0, lang.tr(name)).on_hover_text(lang.tr(name)).clicked() {
                    *on = !*on;
                }
            }
        });
        ui.end_row();
        let l = label(ui, lang.tr("Underline"));
        choice(
            ui,
            "fc_underline",
            l,
            &mut st.font.underline,
            &[
                (Underline::None, "None"),
                (Underline::Single, "Single"),
                (Underline::Double, "Double"),
                (Underline::SingleAccounting, "Single Accounting"),
                (Underline::DoubleAccounting, "Double Accounting"),
            ],
            lang,
        );
        ui.end_row();
        let _ = label(ui, lang.tr("Effect"));
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for (v, name) in [(VertAlign::Baseline, "Normal"), (VertAlign::Superscript, "Superscript"), (VertAlign::Subscript, "Subscript")] {
                if segment(ui, lang.tr(name), st.font.vert == v, 92.0, lang.tr(name)).clicked() {
                    st.font.vert = v;
                }
            }
        });
        ui.end_row();
        let _ = label(ui, lang.tr("Color"));
        let current = st.font.color.resolve(&Default::default()).map(theme::color32);
        if let Some(c) = swatch(ui, "fc_font_color", current, colors, lang.tr("Automatic"), t) {
            st.font.color = if c == "none" { Color::Auto } else { Color::from_hex(&c).unwrap_or_default() };
        }
        ui.end_row();
    });
}

fn border(app: &SheetApp, ui: &mut egui::Ui, st: &mut Style, theme: &mkmatrix_engine::model::Theme, t: &Tokens) {
    let lang = app.ui.language;
    let thin = BorderLine { style: BorderStyle::Thin, color: Color::Auto };
    heading(ui, lang.tr("Presets"));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        if segment(ui, lang.tr("None"), false, 92.0, lang.tr("None")).clicked() {
            st.borders = Default::default();
        }
        if segment(ui, lang.tr("Outline"), false, 92.0, lang.tr("Outline")).clicked() {
            st.borders.top = thin;
            st.borders.bottom = thin;
            st.borders.left = thin;
            st.borders.right = thin;
        }
    });
    ui.add_space(4.0);
    heading(ui, lang.tr("Lines"));
    form(ui, "fc_border", |ui| {
        for (n, slot) in [
            ("Top", &mut st.borders.top),
            ("Bottom", &mut st.borders.bottom),
            ("Left", &mut st.borders.left),
            ("Right", &mut st.borders.right),
            ("Diagonal ↘", &mut st.borders.diag_down),
            ("Diagonal ↗", &mut st.borders.diag_up),
        ] {
            let name = lang.tr(n);
            let _ = label(ui, name);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for (s, a11y) in [
                    (BorderStyle::None, "none"),
                    (BorderStyle::Thin, "thin"),
                    (BorderStyle::Medium, "medium"),
                    (BorderStyle::Thick, "thick"),
                    (BorderStyle::Dashed, "dashed"),
                    (BorderStyle::Dotted, "dotted"),
                    (BorderStyle::Double, "double"),
                ] {
                    // Each choice draws its line; its name is for tooltips and screen readers.
                    let a11y_label = format!("{name} {}", lang.tr(a11y));
                    let resp = segment(ui, "", slot.style == s, 38.0, &a11y_label).on_hover_text(lang.tr(a11y));
                    let r = resp.rect.shrink2(vec2(8.0, 0.0));
                    let ink = if slot.style == s { t.accent } else { t.text };
                    if s == BorderStyle::None {
                        ui.painter().extend(egui::Shape::dotted_line(
                            &[pos2(r.left(), r.center().y), pos2(r.right(), r.center().y)],
                            t.text_dim,
                            4.0,
                            0.6,
                        ));
                    } else {
                        crate::grid::paint_border(ui.painter(), [pos2(r.left(), r.center().y), pos2(r.right(), r.center().y)], s, ink);
                    }
                    if resp.clicked() {
                        slot.style = s;
                    }
                }
            });
            ui.end_row();
        }
    });
    crate::border_preview::sample(ui, &st.borders, theme);
}

fn fill(app: &SheetApp, ui: &mut egui::Ui, st: &mut Style, colors: &[u32; 12]) {
    let lang = app.ui.language;
    heading(ui, lang.tr("Background Color"));
    if let Some(c) = crate::widgets::color_palette(ui, colors, lang.tr("No Color")) {
        st.fill = if c == "none" { Default::default() } else { Fill::solid(Color::from_hex(&c).unwrap_or_default()) };
    }
}

fn protection(app: &SheetApp, ui: &mut egui::Ui, st: &mut Style) {
    let lang = app.ui.language;
    let t = Tokens::get(ui.ctx());
    heading(ui, lang.tr("Protection"));
    for (value, name, note) in [
        (&mut st.protection.locked, "Locked", "The cell can't be changed while the sheet is protected."),
        (&mut st.protection.hidden, "Hidden", "The cell's formula isn't shown while the sheet is protected."),
    ] {
        ui.checkbox(value, lang.tr(name));
        ui.label(egui::RichText::new(lang.tr(note)).font(theme::ui_font(12.0)).color(t.text_dim));
        ui.add_space(4.0);
    }
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(lang.tr("Locking cells or hiding formulas has no effect until you protect the worksheet (Review › Protect Sheet)."))
            .font(theme::ui_font(12.0))
            .color(t.text_dim),
    );
}
