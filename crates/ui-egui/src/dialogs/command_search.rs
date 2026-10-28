use super::kit::{ROW_H, footer, hint, list_row, panel, search_field};
use super::*;

use egui::pos2;

use crate::theme::Tokens;

/// The list's height: the window keeps one size whatever the search finds.
const LIST_H: f32 = 12.0 * ROW_H + 2.0;

/// Search Commands, in Format Cells' style: a search box over every enabled command whose name or
/// id matches (in the interface language and in English), each row its name, where it lives in
/// the menus and its shortcut. Arrow keys, Page Up/Down and Ctrl+Home/End walk the list; Enter or
/// a click runs the command.
pub(super) fn command_search(app: &mut SheetApp, ui: &mut egui::Ui, d: &mut Dialog, confirm: &mut bool) {
    let lang = app.ui.language;
    let t = Tokens::get(ui.ctx());
    let (down, up, home, end, pgup, pgdn) = ui.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, Key::ArrowDown),
            i.consume_key(egui::Modifiers::NONE, Key::ArrowUp),
            i.consume_key(egui::Modifiers::CTRL, Key::Home),
            i.consume_key(egui::Modifiers::CTRL, Key::End),
            i.consume_key(egui::Modifiers::NONE, Key::PageUp),
            i.consume_key(egui::Modifiers::NONE, Key::PageDown),
        )
    });
    ui.spacing_mut().item_spacing.y = 8.0;
    let width = ui.available_width();
    let r = search_field(ui, &mut d.search, lang.tr("Type a command, e.g. freeze, chart, bold…"), lang.tr("Search Commands"), width);
    if !d.focus_initialized {
        r.request_focus();
        d.focus_initialized = true;
    }

    let q = d.search.to_lowercase();
    // (id, name, menu path, shortcut), the names and paths in the interface language.
    let cmds: Vec<(String, String, String, Option<&'static str>)> = app
        .session
        .commands()
        .into_iter()
        .filter(|c| c.enabled)
        .map(|c| {
            (c.id.to_string(), lang.tr(c.label).to_string(), c.menu.iter().map(|m| lang.tr(m)).collect::<Vec<_>>().join(" › "), c.shortcut, c.label)
        })
        .filter(|(id, name, _, _, english)| {
            q.is_empty() || name.to_lowercase().contains(&q) || english.to_lowercase().contains(&q) || id.to_lowercase().contains(&q)
        })
        .map(|(id, name, menu, shortcut, _)| (id, name, menu, shortcut))
        .collect();

    let last = cmds.len().saturating_sub(1);
    let page_size = ((LIST_H / ROW_H).floor() as usize).saturating_sub(1).max(1);
    if down {
        d.list_index = (d.list_index + 1).min(last);
    }
    if up {
        d.list_index = d.list_index.saturating_sub(1);
    }
    if pgdn {
        d.list_index = (d.list_index + page_size).min(last);
    }
    if pgup {
        d.list_index = d.list_index.saturating_sub(page_size);
    }
    if home {
        d.list_index = 0;
    }
    if end {
        d.list_index = last;
    }
    d.list_index = d.list_index.min(last);
    let moved = down || up || home || end || pgup || pgdn;

    panel(ui, LIST_H, |ui| {
        if cmds.is_empty() {
            ui.add_space(LIST_H / 2.0 - 10.0);
            ui.vertical_centered(|ui| hint(ui, lang.tr("No commands match your search.")));
            return;
        }
        ui.spacing_mut().item_spacing.y = 0.0;
        egui::ScrollArea::vertical().id_salt("commandSearch.list").auto_shrink([false, false]).show_rows(ui, ROW_H, cmds.len(), |ui, rows| {
            for i in rows {
                let Some((id, name, menu, shortcut)) = cmds.get(i) else { continue };
                let on = i == d.list_index;
                let r = list_row(ui, name, Some(menu), on, 200.0);
                // The shortcut at the row's end, as a key cap.
                if let Some(keys) = shortcut {
                    let p = ui.painter();
                    let galley = p.layout_no_wrap((*keys).to_string(), theme::ui_font(11.5), t.text_dim);
                    let cap = egui::Rect::from_min_size(
                        pos2(r.rect.right() - galley.size().x - 18.0, r.rect.center().y - 9.0),
                        vec2(galley.size().x + 10.0, 18.0),
                    );
                    p.rect_filled(cap, 3.0, if on { t.input_bg } else { t.hover });
                    p.galley(pos2(cap.left() + 5.0, cap.center().y - galley.size().y / 2.0), galley, t.text_dim);
                }
                if moved && on && !ui.clip_rect().contains_rect(r.rect) {
                    r.scroll_to_me(Some(if down || end || pgdn { egui::Align::Max } else { egui::Align::Min }));
                }
                if r.clicked() {
                    d.list_index = i;
                    *confirm = true;
                }
                r.on_hover_text(id);
            }
        });
    });

    let mut open = true;
    let count = lang.tr("{count} commands").replace("{count}", &cmds.len().to_string());
    footer(ui, confirm, &mut open, |ui| hint(ui, &format!("{count}  ·  ↑ ↓  ·  Enter")));
    // Cancel: `confirm` without a command closes the dialog, and nothing runs.
    if !open {
        *confirm = true;
        return;
    }
    if *confirm && let Some((id, ..)) = cmds.get(d.list_index) {
        let id = id.clone();
        app.dialog = None;
        app.run_or_alert(&id, json!({}));
    }
}
