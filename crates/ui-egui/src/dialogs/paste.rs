use super::*;

/// Paste Special as Excel lays it out, in Format Cells' style: Paste and Operation each in two
/// columns of round options under a muted heading, Skip blanks and Transpose side by side, and
/// Paste Link in the footer, pasting at once. The options come from the dialog's `fields`, so the
/// letter shortcuts and the control channel see the same.
pub(super) fn paste_special(app: &mut SheetApp, ui: &mut egui::Ui, d: &mut Dialog, confirm: &mut bool, open: &mut bool) {
    use super::kit::{button, footer, heading, option_row};
    let lang = app.ui.language;
    let choices: Vec<(&'static str, &'static str, Vec<(&'static str, &'static str)>)> = d
        .fields
        .iter()
        .filter_map(|f| match f {
            Field::Choice { key, label, options } => Some((*key, *label, options.clone())),
            _ => None,
        })
        .collect();
    let gap = 8.0;
    let col_w = ((ui.available_width() - gap) / 2.0).max(120.0);
    // Two columns side by side, each filled top to bottom.
    let columns = |ui: &mut egui::Ui, add: &mut dyn FnMut(&mut egui::Ui, usize)| {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for c in 0..2 {
                ui.vertical(|ui| {
                    ui.set_width(col_w);
                    ui.spacing_mut().item_spacing.y = 2.0;
                    add(ui, c);
                });
            }
        });
    };
    for (key, label, options) in choices {
        heading(ui, lang.tr(label));
        let cur = d.values.get(key).and_then(Json::as_str).unwrap_or("").to_string();
        // Column by column, as Excel fills them.
        let rows = options.len().div_ceil(2);
        let mut picked = None;
        columns(ui, &mut |ui, c| {
            for (value, text) in options.iter().skip(c * rows).take(rows) {
                if option_row(ui, lang.tr(text), cur == *value, col_w).clicked() {
                    picked = Some(*value);
                }
            }
        });
        if let Some(value) = picked {
            d.set_value(key, json!(value));
        }
        ui.add_space(8.0);
    }
    // In the same columns as the options above, the boxes under their round marks.
    columns(ui, &mut |ui, c| {
        let (key, label) = [("skipBlanks", "Skip blanks"), ("transpose", "Transpose (E)")][c];
        let mut b = d.values.get(key).and_then(Json::as_bool).unwrap_or(false);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            if ui.checkbox(&mut b, lang.tr(label)).changed() {
                d.values.insert(key.into(), json!(b));
            }
        });
    });
    let mut link = false;
    footer(ui, confirm, open, |ui| {
        // Excel's Paste Link pastes references to the copied cells straight away.
        link = button(ui, lang.tr("Paste Link"), false).on_hover_text(lang.tr("Paste formulas that refer to the copied cells")).clicked();
    });
    if link {
        d.values.insert("what".into(), json!("all"));
        d.values.insert("operation".into(), json!("none"));
        d.values.insert("link".into(), json!(true));
        *confirm = true;
    }
}

/// Paste Special's legacy mnemonics select options; Enter still performs the paste.
pub(super) fn paste_special_shortcuts(d: &mut Dialog, ctx: &egui::Context) {
    if ctx.text_edit_focused() {
        return;
    }
    let right_alt = ctx.input(|i| {
        i.key_down(Key::AltRight)
            || i.events.iter().any(|event| {
                matches!(event, egui::Event::Key { key: Key::AltRight, .. })
                    || matches!(event, egui::Event::Key { physical_key: Some(Key::AltRight), .. })
            })
    });
    let mac = ctx.os() == egui::os::OperatingSystem::Mac;
    let allowed = |m: egui::Modifiers| !m.ctrl && !m.command && !m.mac_cmd && !(m.alt && (mac || right_alt));
    let mnemonic = |key| match key {
        Key::T => Some('t'),
        Key::V => Some('v'),
        Key::F => Some('f'),
        Key::C => Some('c'),
        Key::E => Some('e'),
        _ => None,
    };
    let mut text_to_consume = Vec::new();
    for event in ctx.input(|i| i.events.clone()) {
        let egui::Event::Key { key, pressed: true, repeat, modifiers, .. } = event else { continue };
        if !allowed(modifiers) {
            continue;
        }
        let Some(letter) = mnemonic(key) else { continue };
        text_to_consume.push(letter);
        if repeat {
            continue;
        }
        if key == Key::E {
            let transpose = d.values.get("transpose").and_then(Json::as_bool).unwrap_or(false);
            d.values.insert("transpose".into(), json!(!transpose));
        } else {
            let what = match key {
                Key::T => "formats",
                Key::V => "values",
                Key::F => "formulas",
                _ => "comments",
            };
            d.values.insert("what".into(), json!(what));
        }
    }
    // Browsers can deliver Text before or after Key. Act only on Key, then remove
    // its matching text so one physical press cannot toggle Transpose twice.
    ctx.input_mut(|i| {
        i.events.retain(|event| match event {
            egui::Event::Key { key, modifiers, .. } => !(allowed(*modifiers) && mnemonic(*key).is_some()),
            egui::Event::Text(text) if text.len() == 1 => {
                let letter = text.chars().next().unwrap_or_default().to_ascii_lowercase();
                if let Some(index) = text_to_consume.iter().position(|&c| c == letter) {
                    text_to_consume.remove(index);
                    false
                } else {
                    true
                }
            }
            _ => true,
        });
    });
}
