//! The pieces dialogs are built from, in Format Cells' language: a navigation rail of sections
//! (icon and name, the chosen one tinted with a bar of the accent), small muted headings, forms
//! of labelled rows on a fixed grid, segmented controls, lists of flat rows, and a footer under a
//! rule with the default button in the accent.

use super::show::ok_cancel;
use super::*;

use egui::{Rect, pos2};

use crate::theme::Tokens;

/// The form's label column.
pub(crate) const LABEL_W: f32 = 112.0;
/// A list row's height.
pub(crate) const ROW_H: f32 = 28.0;

/// A navigation row: icon and name, the selected one tinted with a bar of the accent. As wide as
/// the column it is in; `count` (a number of hits, say) sits muted at its right end.
pub(crate) fn nav_item(ui: &mut egui::Ui, icon: Icon, label: &str, selected: bool, count: Option<usize>, t: &Tokens) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), egui::Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, selected, label));
    let p = ui.painter();
    if selected {
        p.rect_filled(r, 0.0, t.accent_soft);
        p.rect_filled(Rect::from_min_size(r.min, vec2(3.0, r.height())), 0.0, t.accent);
    } else if resp.hovered() {
        p.rect_filled(r, 0.0, t.hover);
    }
    let dim = count == Some(0) && !selected;
    let ink = if selected {
        t.accent
    } else if dim {
        t.text_disabled
    } else {
        t.text
    };
    icons::paint(p, Rect::from_center_size(pos2(r.left() + 22.0, r.center().y), vec2(16.0, 16.0)), icon, ink);
    let font = if selected { theme::ui_bold(13.0) } else { theme::ui_font(13.0) };
    let right = match count {
        Some(n) => {
            let at = p.text(
                pos2(r.right() - 10.0, r.center().y),
                egui::Align2::RIGHT_CENTER,
                n.to_string(),
                theme::ui_font(11.5),
                if selected { t.accent } else { t.text_dim },
            );
            at.left() - 6.0
        }
        None => r.right() - 6.0,
    };
    let clip = Rect::from_min_max(r.min, pos2(right, r.max.y));
    p.with_clip_rect(clip).text(pos2(r.left() + 42.0, r.center().y), egui::Align2::LEFT_CENTER, label, font, ink);
    resp
}

/// A navigation rail `width` wide and `height` tall beside a hairline, then the gap before the
/// content.
pub(crate) fn nav_rail(ui: &mut egui::Ui, width: f32, height: f32, t: &Tokens, add: impl FnOnce(&mut egui::Ui)) {
    ui.vertical(|ui| {
        ui.set_width(width);
        ui.set_min_height(height);
        ui.spacing_mut().item_spacing.y = 2.0;
        add(ui);
    });
    let (line, _) = ui.allocate_exact_size(vec2(1.0, height), egui::Sense::hover());
    ui.painter().rect_filled(line, 0.0, t.separator);
    ui.add_space(16.0);
}

/// The width a navigation rail needs so none of `labels` is cut: the widest name (in the bold
/// the chosen one takes) beside the icon, and never less than `min`.
pub(crate) fn nav_width(ui: &egui::Ui, labels: &[&str], min: f32) -> f32 {
    let widest =
        labels.iter().map(|l| ui.painter().layout_no_wrap((*l).to_string(), theme::ui_bold(13.0), Color32::WHITE).size().x).fold(0.0, f32::max);
    (42.0 + widest + 14.0).ceil().max(min)
}

/// A section's heading: small, muted, with room above.
pub(crate) fn heading(ui: &mut egui::Ui, text: &str) {
    let t = Tokens::get(ui.ctx());
    ui.add_space(2.0);
    ui.label(egui::RichText::new(text).font(theme::ui_bold(11.5)).color(t.text_dim));
}

/// A form of labelled rows: labels in a column of their own, controls lined up beside them.
pub(crate) fn form(ui: &mut egui::Ui, id: &str, add: impl FnOnce(&mut egui::Ui)) {
    egui::Grid::new(id).num_columns(2).min_col_width(LABEL_W).spacing([12.0, 10.0]).show(ui, add);
}

/// A row's label; its id names the row's control for screen readers.
pub(crate) fn label(ui: &mut egui::Ui, text: &str) -> egui::Id {
    let t = Tokens::get(ui.ctx());
    ui.label(egui::RichText::new(text).font(theme::ui_font(13.0)).color(t.text_dim)).id
}

/// One choice of a segmented control: joined buttons of one height, the chosen one filled.
pub(crate) fn segment(ui: &mut egui::Ui, text: &str, on: bool, width: f32, a11y: &str) -> egui::Response {
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

/// A segmented control over `options` (value, label): returns the value clicked.
pub(crate) fn segments<'a>(ui: &mut egui::Ui, current: &str, options: &[(&'a str, &str)], width: f32) -> Option<&'a str> {
    let mut picked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (value, text) in options {
            if segment(ui, text, current == *value, width, text).clicked() {
                picked = Some(*value);
            }
        }
    });
    picked
}

/// The search box: a magnifier, then the text, on the input colour with a border that takes the
/// accent while typing. `width` is the box's whole width.
pub(crate) fn search_field(ui: &mut egui::Ui, text: &mut String, hint: &str, name: &str, width: f32) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let (r, _) = ui.allocate_exact_size(vec2(width, 32.0), egui::Sense::hover());
    ui.painter().rect_filled(r, 0.0, t.input_bg);
    icons::paint(ui.painter(), Rect::from_center_size(pos2(r.left() + 16.0, r.center().y), vec2(15.0, 15.0)), Icon::Search, t.text_dim);
    let inner = Rect::from_min_max(pos2(r.left() + 32.0, r.top()), pos2(r.right() - 8.0, r.bottom()));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
    let resp = child
        .add(egui::TextEdit::singleline(text).frame(egui::Frame::NONE).font(theme::ui_font(13.5)).hint_text(hint).desired_width(inner.width()))
        .accessible_name(name.to_string());
    let border = if resp.has_focus() { t.accent } else { t.input_border };
    ui.painter().rect_stroke(r, 0.0, egui::Stroke::new(1.0, border), egui::StrokeKind::Inside);
    resp
}

/// A flat list row: `primary` text, then `secondary` muted beside it (clipped), the chosen row
/// tinted with the accent. Screen readers read it by `primary`.
pub(crate) fn list_row(ui: &mut egui::Ui, primary: &str, secondary: Option<&str>, selected: bool, primary_w: f32) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), egui::Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, selected, primary));
    let p = ui.painter();
    if selected {
        p.rect_filled(r, 0.0, t.accent_soft);
        p.rect_filled(Rect::from_min_size(r.min, vec2(2.0, r.height())), 0.0, t.accent);
    } else if resp.hovered() {
        p.rect_filled(r, 0.0, t.hover);
    }
    let ink = if selected { t.accent } else { t.text };
    let font = if selected { theme::ui_bold(13.0) } else { theme::ui_font(13.0) };
    let name_clip = Rect::from_min_max(r.min, pos2((r.left() + 10.0 + primary_w).min(r.right()), r.max.y));
    p.with_clip_rect(name_clip).text(pos2(r.left() + 10.0, r.center().y), egui::Align2::LEFT_CENTER, primary, font, ink);
    if let Some(s) = secondary {
        let clip = Rect::from_min_max(pos2(name_clip.right() + 8.0, r.top()), pos2(r.right() - 8.0, r.bottom()));
        p.with_clip_rect(clip).text(pos2(clip.left(), r.center().y), egui::Align2::LEFT_CENTER, s, theme::ui_font(12.5), t.text_dim);
    }
    resp
}

/// A choice among several shown at once: a round mark (filled with the accent when chosen) and
/// its text, as wide as `width`.
pub(crate) fn option_row(ui: &mut egui::Ui, text: &str, on: bool, width: f32) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(vec2(width, 30.0), egui::Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::RadioButton, true, on, text));
    let p = ui.painter();
    if on {
        p.rect_filled(r, 0.0, t.accent_soft);
    } else if resp.hovered() {
        p.rect_filled(r, 0.0, t.hover);
    }
    let c = pos2(r.left() + 16.0, r.center().y);
    p.circle(c, 6.5, t.input_bg, egui::Stroke::new(1.0, if on { t.accent } else { t.input_border }));
    if on {
        p.circle_filled(c, 3.5, t.accent);
    }
    p.text(pos2(r.left() + 32.0, r.center().y), egui::Align2::LEFT_CENTER, text, theme::ui_font(13.0), if on { t.accent } else { t.text });
    resp
}

/// A framed panel on the input colour (a list, a detail card), `height` tall and as wide as the
/// space left.
pub(crate) fn panel<R>(ui: &mut egui::Ui, height: f32, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let t = Tokens::get(ui.ctx());
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), egui::Sense::hover());
    ui.painter().rect_filled(r, 0.0, t.input_bg);
    ui.painter().rect_stroke(r, 0.0, egui::Stroke::new(1.0, t.input_border), egui::StrokeKind::Inside);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r.shrink(1.0)).layout(egui::Layout::top_down(egui::Align::Min)));
    child.set_clip_rect(r.shrink(1.0));
    add(&mut child)
}

/// A button of the footer's size; the default one in the accent.
pub(crate) fn button(ui: &mut egui::Ui, text: &str, primary: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let label = egui::RichText::new(text).font(theme::ui_font(13.0));
    let b = if primary { egui::Button::new(label.color(t.on_accent)).fill(t.accent) } else { egui::Button::new(label) };
    let w = ui.painter().layout_no_wrap(text.to_string(), theme::ui_font(13.0), t.text).size().x + 28.0;
    ui.add_sized(vec2(w.max(84.0), 28.0), b)
}

/// The footer: a rule across, then `left` (a hint, a count, a button of its own) and Cancel / OK
/// on the right.
pub(crate) fn footer(ui: &mut egui::Ui, confirm: &mut bool, open: &mut bool, left: impl FnOnce(&mut egui::Ui)) {
    rule(ui);
    ui.horizontal(|ui| {
        left(ui);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ok_cancel(ui, confirm, open);
        });
    });
}

/// The hairline above a footer, with room on both sides.
pub(crate) fn rule(ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    ui.add_space(6.0);
    let (rule, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().rect_filled(rule, 0.0, t.separator);
    ui.add_space(8.0);
}

/// Muted text, for counts and empty states.
pub(crate) fn hint(ui: &mut egui::Ui, text: &str) {
    let t = Tokens::get(ui.ctx());
    ui.label(egui::RichText::new(text).font(theme::ui_font(12.5)).color(t.text_dim));
}
