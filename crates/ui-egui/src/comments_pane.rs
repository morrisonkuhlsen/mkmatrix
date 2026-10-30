//! The Comments pane: every comment and note of the sheet (or of the whole workbook), each on a
//! card that names its cell, its row and its column's header, with the author, the date when the
//! file has one, the replies, a reply box and its actions (resolve, edit, copy, delete). Above the
//! cards, fixed while they scroll, a search box filters by text, reply, author, cell or column
//! header, accents and case ignored, and marks what it found; Enter or the arrows beside the count
//! go to the next card and select its cell, switching sheet when needed. The status segments show
//! all, open or resolved threads. Everything acts through `review.*` and `selection.set`.

use egui::text::{LayoutJob, TextFormat};
use egui::{FontId, Rect, Sense, Stroke, Ui, vec2};
use mkmatrix_engine::core::CellRef;
use mkmatrix_engine::model::{Comment, Visibility};
use serde_json::json;

use crate::SheetApp;
use crate::dialogs::kit;
use crate::i18n::{self, Language};
use crate::icons::Icon;
use crate::theme::{self, Tokens};
use crate::widgets;

/// The pane's own state, kept between frames.
#[derive(Clone, Default)]
struct State {
    search: String,
    /// `all`, `open` or `resolved` (empty: all).
    status: &'static str,
    /// Comments of every visible sheet, not only the active one.
    workbook: bool,
    /// The card to bring into view next frame: (sheet, cell).
    scroll_to: Option<(usize, CellRef)>,
    /// The card whose text is being edited, and the draft.
    editing: Option<(usize, CellRef, String)>,
}

/// A comment as the pane lists it.
struct Entry {
    sheet: usize,
    sheet_name: String,
    cell: CellRef,
    /// The header of the table or AutoFilter column the cell sits in.
    column: Option<String>,
    comment: Comment,
}

impl Entry {
    fn resolved(&self) -> bool {
        self.comment.threaded && self.comment.resolved
    }

    /// Whether the search (folded) finds anything on this card.
    fn matches(&self, q: &[char]) -> bool {
        let c = &self.comment;
        q.is_empty()
            || [c.text.as_str(), c.author.as_str(), &self.cell.a1(), self.column.as_deref().unwrap_or("")]
                .into_iter()
                .chain(c.replies.iter().flat_map(|(a, t)| [a.as_str(), t.as_str()]))
                .any(|s| !find_all(s, q).is_empty())
    }

    /// The thread as plain text, for the clipboard.
    fn as_text(&self) -> String {
        let c = &self.comment;
        let mut out = format!("{}: {}", c.author, c.text);
        for (a, t) in &c.replies {
            out.push_str(&format!("\n  {a}: {t}"));
        }
        out
    }
}

fn state_id() -> egui::Id {
    egui::Id::unique("comments_pane")
}

/// Set when an editor opens: it takes the focus on its first frame.
fn edit_focus_id() -> egui::Id {
    egui::Id::unique("comments_pane_edit_focus")
}

/// Every comment the pane can list: the active sheet's, or every visible sheet's in tab order;
/// row by row within a sheet.
fn entries(app: &SheetApp, workbook: bool) -> Vec<Entry> {
    let Some(d) = app.session.active() else { return vec![] };
    let active = d.wb.active_sheet();
    d.wb.sheets
        .iter()
        .enumerate()
        .filter(|(i, sh)| if workbook { sh.visibility == Visibility::Visible || *i == active } else { *i == active })
        .flat_map(|(i, sh)| {
            sh.comments.iter().map(move |(cell, c)| Entry {
                sheet: i,
                sheet_name: sh.name.clone(),
                cell: *cell,
                column: column_header(sh, *cell),
                comment: c.clone(),
            })
        })
        .collect()
}

/// The header of the column `c` sits in: a table column's name, or the AutoFilter's header cell.
fn column_header(sh: &mkmatrix_engine::model::sheet::Sheet, c: CellRef) -> Option<String> {
    let table = sh.tables.iter().find(|t| t.header_row && t.range.contains(c) && c.row > t.range.start.row);
    if let Some(name) = table.and_then(|t| t.columns.get(c.col.checked_sub(t.range.start.col)? as usize)).map(|col| col.name.clone()) {
        return Some(name);
    }
    let af = sh.autofilter.as_ref().filter(|a| a.range.contains(c) && c.row > a.range.start.row)?;
    let header = sh.value(CellRef::new(af.range.start.row, c.col)).display();
    (!header.trim().is_empty()).then_some(header)
}

/// `c` in lower case without its accent, one char for one, so a match found in folded text is at
/// the same chars in the original.
fn fold(c: char) -> char {
    let l = c.to_lowercase().next().unwrap_or(c);
    match l {
        'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        'ý' | 'ÿ' => 'y',
        _ => l,
    }
}

/// The byte ranges of `s` where the folded query `q` occurs.
fn find_all(s: &str, q: &[char]) -> Vec<std::ops::Range<usize>> {
    if q.is_empty() {
        return vec![];
    }
    let chars: Vec<(usize, char)> = s.char_indices().collect();
    let folded: Vec<char> = chars.iter().map(|(_, c)| fold(*c)).collect();
    let mut out = vec![];
    let mut i = 0;
    while i + q.len() <= folded.len() {
        if folded.get(i..i + q.len()) == Some(q) {
            let start = chars.get(i).map_or(s.len(), |c| c.0);
            let end = chars.get(i + q.len()).map_or(s.len(), |c| c.0);
            out.push(start..end);
            i += q.len();
        } else {
            i += 1;
        }
    }
    out
}

/// `text` in `font` and `color`, with what the search found on the accent's tint.
fn highlighted(text: &str, q: &[char], font: FontId, color: egui::Color32, t: &Tokens) -> LayoutJob {
    let mut job = LayoutJob::default();
    let plain = TextFormat { font_id: font.clone(), color, ..Default::default() };
    let found = TextFormat { font_id: font, color: t.text, background: t.accent_soft, underline: Stroke::new(1.0, t.accent), ..Default::default() };
    let mut at = 0;
    for r in find_all(text, q) {
        job.append(text.get(at..r.start).unwrap_or(""), 0.0, plain.clone());
        job.append(text.get(r.clone()).unwrap_or(""), 0.0, found.clone());
        at = r.end;
    }
    job.append(text.get(at..).unwrap_or(""), 0.0, plain);
    job
}

/// The author (already laid out), then the date and a tag (text, colour) in smaller type, on one
/// baseline.
fn byline(mut job: LayoutJob, date: Option<&String>, tag: Option<(&str, egui::Color32)>, t: &Tokens) -> LayoutJob {
    let small = |color| TextFormat { font_id: theme::ui_font(11.5), color, ..Default::default() };
    if let Some(d) = date {
        job.append(&short_date(d), 8.0, small(t.text_dim));
    }
    if let Some((tag, color)) = tag {
        job.append(tag, 8.0, small(color));
    }
    job
}

/// An ISO 8601 date as the cards show it: `2024-05-01 14:30`.
fn short_date(iso: &str) -> String {
    iso.get(..16).unwrap_or(iso).replace('T', " ")
}

/// Selects the comment's cell (on its sheet) and scrolls the grid to it.
fn go_to(app: &mut SheetApp, sheet: usize, cell: CellRef) {
    app.run_or_alert("selection.set", json!({"sheet": sheet, "cell": cell.a1()}));
    app.grid.ensure_visible = true;
}

/// Draws the pane's body under its title: the search and filters, then the cards.
pub fn show(app: &mut SheetApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let lang = i18n::current(ui.ctx());
    let mut st: State = ui.ctx().data_mut(|d| d.get_temp(state_id())).unwrap_or_default();
    let here = app.session.active().map(|d| (d.wb.active_sheet(), d.selection.active));

    let all = entries(app, st.workbook);
    let q: Vec<char> = st.search.trim().chars().map(fold).collect();
    let status = if st.status.is_empty() { "all" } else { st.status };
    let open = all.iter().filter(|e| !e.resolved()).count();
    let shown: Vec<&Entry> = all
        .iter()
        .filter(|e| match status {
            "open" => !e.resolved(),
            "resolved" => e.resolved(),
            _ => true,
        })
        .filter(|e| e.matches(&q))
        .collect();

    // Search, then where to look and which threads, fixed above the cards.
    let width = ui.available_width();
    let search = kit::search_field(ui, &mut st.search, lang.tr("Search comments"), lang.tr("Search comments"), width);
    let mut step: Option<bool> = None;
    if search.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        step = Some(!ui.input(|i| i.modifiers.shift));
        search.request_focus();
    }
    if search.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        st.search.clear();
    }
    ui.add_space(6.0);
    let all_label = format!("{} ({})", lang.tr("All"), all.len());
    let open_label = format!("{} ({open})", lang.tr("Unresolved"));
    let resolved_label = format!("{} ({})", lang.tr("Resolved"), all.len() - open);
    let options = [("all", all_label.as_str()), ("open", open_label.as_str()), ("resolved", resolved_label.as_str())];
    if let Some(s) = kit::segments(ui, status, &options, (width / 3.0).floor()) {
        st.status = s;
    }
    ui.add_space(4.0);
    let scope = [("sheet", lang.tr("This sheet")), ("workbook", lang.tr("Workbook"))];
    if let Some(s) = kit::segments(ui, if st.workbook { "workbook" } else { "sheet" }, &scope, (width / 2.0).floor()) {
        st.workbook = s == "workbook";
    }
    ui.add_space(4.0);

    // The count, and the arrows that step through the cards shown.
    ui.horizontal(|ui| {
        let count = if q.is_empty() {
            match shown.len() {
                1 => lang.tr("1 comment").to_string(),
                n => lang.tr("{n} comments").replace("{n}", &n.to_string()),
            }
        } else {
            lang.tr("{n} of {m} comments").replace("{n}", &shown.len().to_string()).replace("{m}", &all.len().to_string())
        };
        ui.label(egui::RichText::new(count).font(theme::ui_font(12.0)).color(t.text_dim));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if shown.is_empty() {
                return;
            }
            if widgets::icon_button(ui, Icon::Right, t.text, "Next comment", vec2(22.0, 22.0)).clicked() {
                step = Some(true);
            }
            if widgets::icon_button(ui, Icon::Left, t.text, "Previous comment", vec2(22.0, 22.0)).clicked() {
                step = Some(false);
            }
        });
    });
    kit::rule(ui);

    if let (Some(forward), Some(here)) = (step, here) {
        let at = |e: &&Entry| (e.sheet, e.cell);
        let target = if forward {
            shown.iter().find(|e| at(e) > here).or_else(|| shown.first())
        } else {
            shown.iter().rev().find(|e| at(e) < here).or_else(|| shown.last())
        };
        if let Some(e) = target {
            st.scroll_to = Some((e.sheet, e.cell));
            go_to(app, e.sheet, e.cell);
        }
    }

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        if all.is_empty() {
            let empty = if st.workbook { "No comments in this workbook yet." } else { "No comments on this sheet yet." };
            kit::hint(ui, lang.tr(empty));
            kit::hint(ui, lang.tr("Select a cell and choose New Comment."));
        } else if shown.is_empty() {
            kit::hint(ui, lang.tr("No comment matches the search."));
        }
        let active_sheet = here.map(|h| h.0);
        for e in &shown {
            let current = here == Some((e.sheet, e.cell));
            let r = card(app, ui, e, &q, current, st.workbook && Some(e.sheet) != active_sheet, &mut st.editing, lang, &t);
            if st.scroll_to == Some((e.sheet, e.cell)) {
                ui.scroll_to_rect(r, Some(egui::Align::Center));
                st.scroll_to = None;
            }
            ui.add_space(8.0);
        }
    });
    ui.ctx().data_mut(|d| d.insert_temp(state_id(), st));
}

/// One comment's card; returns where it was drawn.
#[allow(clippy::too_many_arguments)]
fn card(
    app: &mut SheetApp,
    ui: &mut Ui,
    e: &Entry,
    q: &[char],
    current: bool,
    other_sheet: bool,
    editing: &mut Option<(usize, CellRef, String)>,
    lang: Language,
    t: &Tokens,
) -> Rect {
    let c = &e.comment;
    let ink = if e.resolved() { t.text_dim } else { t.text };
    let a1 = e.cell.a1();
    let place = if other_sheet { format!("{}!{a1}", mkmatrix_engine::formula::quote_sheet(&e.sheet_name)) } else { a1.clone() };
    let params = |extra: serde_json::Value| {
        let mut p = json!({"sheet": e.sheet, "cell": a1});
        if let (Some(p), Some(extra)) = (p.as_object_mut(), extra.as_object()) {
            p.extend(extra.clone());
        }
        p
    };
    let frame = egui::Frame::NONE
        .fill(t.input_bg)
        .stroke(Stroke::new(1.0, if current { t.accent } else { t.input_border }))
        .inner_margin(egui::Margin { left: 12, right: 8, top: 8, bottom: 8 });
    let resp = frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        // Where: the cell, then its row and column header, a click away; the actions on the right.
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let tip = if c.threaded { "Delete comment" } else { "Delete note" };
                if widgets::icon_button(ui, Icon::Close, t.text_dim, tip, vec2(22.0, 22.0)).clicked() {
                    app.run_or_alert("review.deleteComment", params(json!({})));
                }
                if widgets::icon_button(ui, Icon::Copy, t.text_dim, "Copy text", vec2(22.0, 22.0)).clicked() {
                    ui.ctx().copy_text(e.as_text());
                }
                if widgets::icon_button(ui, Icon::Pen, t.text_dim, "Edit text", vec2(22.0, 22.0)).clicked() {
                    *editing = Some((e.sheet, e.cell, c.text.clone()));
                    ui.ctx().data_mut(|d| d.insert_temp(edit_focus_id(), true));
                }
                if c.threaded {
                    let (color, tip) = if c.resolved { (t.accent, "Reopen thread") } else { (t.text_dim, "Resolve thread") };
                    if widgets::icon_button(ui, Icon::Check, color, tip, vec2(22.0, 22.0)).clicked() {
                        app.run_or_alert("review.resolveComment", params(json!({"resolved": !c.resolved})));
                    }
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let cell = ui
                        .add(egui::Label::new(highlighted(&place, q, theme::ui_bold(13.5), t.accent, t)).sense(Sense::click()).truncate())
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text(lang.tr("Go to the cell"));
                    let mut row = lang.tr("Row {n}").replace("{n}", &(e.cell.row + 1).to_string());
                    if let Some(h) = &e.column {
                        row = format!("{row} · {h}");
                    }
                    let row = ui
                        .add(egui::Label::new(highlighted(&row, q, theme::ui_font(12.0), t.text_dim, t)).sense(Sense::click()).truncate())
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    if cell.clicked() || row.clicked() {
                        go_to(app, e.sheet, e.cell);
                    }
                });
            });
        });
        ui.add_space(2.0);
        // Who, when, and what kind.
        let kind = if !c.threaded {
            Some((lang.tr("Note"), t.text_dim))
        } else if c.resolved {
            Some((lang.tr("Resolved thread"), t.accent))
        } else {
            None
        };
        ui.label(byline(highlighted(&c.author, q, theme::ui_bold(12.5), ink, t), c.dates.first(), kind, t));
        // The text, or its editor.
        if let Some((_, _, draft)) = editing.as_mut().filter(|(s, at, _)| (*s, *at) == (e.sheet, e.cell)) {
            let r = ui.add(egui::TextEdit::multiline(draft).font(theme::ui_font(13.0)).desired_rows(2).desired_width(ui.available_width()));
            // Focused as it opens with the text selected: typing replaces it, an arrow keeps it.
            if ui.ctx().data_mut(|d| d.remove_temp::<bool>(edit_focus_id())).is_some() {
                r.request_focus();
                if let Some(mut s) = egui::TextEdit::load_state(ui.ctx(), r.id) {
                    let all = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(draft.chars().count()));
                    s.cursor.set_char_range(Some(all));
                    s.store(ui.ctx(), r.id);
                }
            }
            let mut done = None;
            ui.horizontal(|ui| {
                if kit::button(ui, lang.tr("Save"), true).clicked() {
                    done = Some(true);
                }
                if kit::button(ui, lang.tr("Cancel"), false).clicked() || r.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    done = Some(false);
                }
            });
            if let Some(save) = done {
                if save && !draft.trim().is_empty() {
                    let id = if c.threaded { "review.newComment" } else { "review.newNote" };
                    app.run_or_alert(id, params(json!({"text": draft.trim()})));
                }
                *editing = None;
            }
        } else {
            ui.label(highlighted(&c.text, q, theme::ui_font(13.0), ink, t));
        }
        // The replies, indented behind a rule.
        for (i, (a, txt)) in c.replies.iter().enumerate() {
            ui.add_space(4.0);
            let r = egui::Frame::NONE.inner_margin(egui::Margin { left: 10, ..Default::default() }).show(ui, |ui| {
                ui.label(byline(highlighted(a, q, theme::ui_bold(12.0), ink, t), c.dates.get(i + 1), None, t));
                ui.label(highlighted(txt, q, theme::ui_font(12.5), ink, t));
            });
            ui.painter().rect_filled(Rect::from_min_size(r.response.rect.min, vec2(2.0, r.response.rect.height())), 0.0, t.separator);
        }
        // A reply, on an open thread.
        if c.threaded && !c.resolved {
            ui.add_space(6.0);
            let key = egui::Id::unique(("comment_reply", e.sheet, e.cell));
            let mut draft: String = ui.ctx().data_mut(|m| m.get_temp(key)).unwrap_or_default();
            let mut send = false;
            // The button first, right to left, so the field takes exactly the width it leaves: a
            // field sized by a guess wider than the row would push the resizable pane wider.
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    send = widgets::icon_button(ui, Icon::Right, t.accent, "Send reply", vec2(26.0, 26.0)).clicked();
                    let r = reply_field(ui, &mut draft, lang.tr("Reply…"), ui.available_width());
                    send |= r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                });
            });
            if send && !draft.trim().is_empty() {
                app.run_or_alert("review.replyComment", params(json!({"text": draft.trim()})));
                draft.clear();
            }
            ui.ctx().data_mut(|m| m.insert_temp(key, draft));
        }
    });
    let rect = resp.response.rect;
    if current {
        ui.painter().rect_filled(Rect::from_min_size(rect.min, vec2(3.0, rect.height())), 0.0, t.accent);
    }
    rect
}

/// The reply box: the search box's look without the magnifier.
fn reply_field(ui: &mut Ui, text: &mut String, hint: &str, width: f32) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let (r, _) = ui.allocate_exact_size(vec2(width, 28.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0, t.ribbon);
    let inner = r.shrink2(vec2(8.0, 0.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
    let resp = child
        .add(egui::TextEdit::singleline(text).frame(egui::Frame::NONE).font(theme::ui_font(13.0)).hint_text(hint).desired_width(inner.width()))
        .accessible_name(hint.to_string());
    let border = if resp.has_focus() { t.accent } else { t.input_border };
    ui.painter().rect_stroke(r, 0.0, Stroke::new(1.0, border), egui::StrokeKind::Inside);
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(s: &str) -> Vec<char> {
        s.chars().map(fold).collect()
    }

    #[test]
    fn the_search_ignores_case_and_accents() {
        assert_eq!(find_all("Revisão do Orçamento", &q("revisao")), vec![0..8]);
        assert_eq!(find_all("Revisão do Orçamento", &q("ORÇA")), vec![12..17]);
        assert_eq!(find_all("aaa", &q("a")).len(), 3);
        assert!(find_all("abc", &q("")).is_empty());
        assert!(find_all("ab", &q("abc")).is_empty());
    }
}
