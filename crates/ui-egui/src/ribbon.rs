//! Title bar (Quick Access Toolbar), ribbon tabs and ribbon groups, and keyboard shortcuts.

use egui::{Align2, Color32, Key, Modifiers, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use mkmatrix_engine::model::{HAlign, Style, VAlign};
use serde_json::json;

use crate::SheetApp;
use crate::icons::{self, Icon};
use crate::keytips::keytip_table;
use crate::theme::{self, Cat, Tokens};
use crate::widgets::{big_button, big_split_button, color_palette, icon_button, small_button, small_split_button, split_button, toggle_button};

mod automate;
mod contextual;
mod controls;
mod data;
mod formulas;
mod galleries;
mod home;
mod insert;
mod page_layout;
mod review;
mod shortcuts;
mod title_bar;
mod view;

use automate::automate;
use contextual::{chart_design, table_design};
pub(crate) use controls::{ComboResult, editable_combo};
use controls::{act, keytip};
use data::data;
use formulas::formulas;
pub use home::FONTS;
use home::home;
use insert::{draw, insert};
use page_layout::page_layout;
use review::review;
pub use shortcuts::shortcut;
pub use title_bar::{MAC_CONTENT_LEFT, MAC_TITLE_BAR, TITLE_LEFT_PAD, resize_edges, title_bar};
use title_bar::{TITLE_BAR_H, TITLE_CONTROL_H};
use view::view;

pub const TABS: &[&str] = &["Home", "Insert", "Draw", "Page Layout", "Formulas", "Data", "Review", "View", "Automate"];
const _: () = assert!(TITLE_CONTROL_H < TITLE_BAR_H, "title-bar controls keep a margin above and below");

/// MK Matrix's top bar: File (the Backstage view), the tabs and the Help menu over a single row of the active
/// tab's controls, one size throughout. A tab's click shows its controls in the row; a click on the
/// active tab, or a double click, folds the row away (as Minimal density does). A contextual tab
/// that is gone (the cell left the table) falls back to Home.
pub fn show(app: &mut SheetApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let compact = app.ui.density.compact();
    let menu_h = if compact { 26.0 } else { 30.0 };
    // The row of controls, and under it the groups' names (not in compact density).
    let row_h = if compact { 36.0 } else { 50.0 };
    // A contextual tab exists only while its object is selected (Excel hides Table Design once the
    // cell leaves the table); a remembered one that is gone would draw an empty row.
    if CONTEXTUAL_TABS.contains(&app.ui.ribbon_tab.as_str()) && !contextual_tabs(app).contains(&app.ui.ribbon_tab.as_str()) {
        app.ui.ribbon_tab = "Home".to_string();
    }
    let collapsed = app.ui.ribbon_collapsed;
    // Room between the tabs and the icons under them.
    let gap = if compact { 4.0 } else { 8.0 };
    let h = menu_h + if collapsed { 0.0 } else { gap + row_h + 2.0 };
    egui::Panel::top("ribbon")
        .exact_size(h)
        .show_separator_line(false)
        .frame(egui::Frame::NONE.fill(t.window).inner_margin(egui::Margin { left: 10, right: 10, top: 0, bottom: 0 }))
        .show(ui, |ui| {
            let bar = Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), menu_h));
            ui.scope_builder(egui::UiBuilder::new().max_rect(bar), |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    if app.chrome.file_tab {
                        file_button(app, ui, menu_h);
                    }
                    let mut tabs: Vec<&str> = TABS.to_vec();
                    let ctx_tabs = contextual_tabs(app);
                    tabs.extend(ctx_tabs.iter());
                    for tab in tabs {
                        tab_button(app, ui, tab, ctx_tabs.contains(&tab), menu_h);
                    }
                    menu(app, ui, "Help", menu_h);
                });
            });
            if collapsed {
                return;
            }
            // The active tab's controls in one row, scrolling sideways when the window is narrower
            // than they are. One scroll state per tab: a tab opened after scrolling another starts
            // at its left edge.
            let tab = app.ui.ribbon_tab.clone();
            let row = Rect::from_min_size(pos2(bar.left(), bar.bottom() + gap), vec2(bar.width(), row_h));
            ui.scope_builder(egui::UiBuilder::new().max_rect(row), |ui| {
                let mut area = egui::ScrollArea::horizontal()
                    .id_salt(("toolbar_scroll", tab.as_str()))
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
                // egui 0.37 keeps `ScrollArea`'s offset private; `scroll_offset` is the supported
                // way to move it. An arrow click records the requested offset for the next frame.
                // Expire it if this tab stops rendering, rather than replaying it much later.
                if let Some((frame, x)) = ui.ctx().data_mut(|d| d.remove_temp::<(u64, f32)>(RibbonOverflow::scroll_target_id(tab.as_str())))
                    && ui.ctx().cumulative_frame_nr().saturating_sub(frame) <= 1
                {
                    area = area.horizontal_scroll_offset(x);
                }
                let scroll = area.show(ui, |ui| {
                    ui.set_min_height(row_h);
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                        ui.set_height(controls::TOOL_ROW_H);
                        controls::set_single_row(ui, true);
                        controls::show_group_names(ui, !compact);
                        controls::group_begin(ui);
                        match tab.as_str() {
                            "Home" => home(app, ui),
                            "Insert" => insert(app, ui),
                            "Draw" => draw(app, ui),
                            "Page Layout" => page_layout(app, ui),
                            "Formulas" => formulas(app, ui),
                            "Data" => data(app, ui),
                            "Review" => review(app, ui),
                            "View" => view(app, ui),
                            "Automate" => automate(app, ui),
                            "Table Design" => table_design(app, ui),
                            "Chart Design" => chart_design(app, ui),
                            crate::paper_view::TAB => crate::paper_view::ribbon_tab(app, ui),
                            _ => home(app, ui),
                        }
                        controls::group_end(ui, last_group(&tab));
                        controls::set_single_row(ui, false);
                    });
                });
                scroll_arrows(ui, &t, tab.as_str(), scroll);
            });
        });
}

/// One of the top bar's tabs: underlined while active, in the accent colour when contextual.
fn tab_button(app: &mut SheetApp, ui: &mut Ui, tab: &str, contextual: bool, menu_h: f32) {
    let t = Tokens::get(ui.ctx());
    let active = app.ui.ribbon_tab == tab;
    let collapsed = app.ui.ribbon_collapsed;
    let font = theme::ui_font(13.0);
    let label = app.ui.language.tr(tab).to_string();
    let tw = ui.painter().layout_no_wrap(label.clone(), font.clone(), t.text).size().x;
    let (r, resp) = ui.allocate_exact_size(vec2(tw + 18.0, menu_h - 4.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(r, 0.0, t.hover);
    }
    let color = if contextual {
        t.accent
    } else if active {
        t.text
    } else {
        t.text_dim
    };
    ui.painter().text(r.center(), Align2::CENTER_CENTER, label.clone(), if active { theme::ui_bold(13.0) } else { font }, color);
    if active && !collapsed {
        let ul = Rect::from_center_size(pos2(r.center().x, r.bottom() - 2.0), vec2(tw.clamp(20.0, 40.0), 3.0));
        ui.painter().rect_filled(ul, 1.5, t.accent);
    }
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, active, &label));
    if resp.double_clicked() {
        app.ui.ribbon_collapsed = !collapsed;
    } else if resp.clicked() {
        if active && !collapsed {
            app.ui.ribbon_collapsed = true;
        } else {
            app.ui.ribbon_tab = tab.to_string();
            app.ui.ribbon_collapsed = false;
        }
    }
    if app.keytips.prefix() == Some("")
        && let Some(sequence) = keytip_table::tab_sequence(tab)
    {
        keytip(app, ui, sequence, pos2(r.center().x, r.bottom() - 1.0));
    }
}

/// File, in the accent colour: opens the Backstage view.
fn file_button(app: &mut SheetApp, ui: &mut Ui, menu_h: f32) {
    let t = Tokens::get(ui.ctx());
    let font = theme::ui_font(13.0);
    let label = app.ui.language.tr("File").to_string();
    let tw = ui.painter().layout_no_wrap(label.clone(), font.clone(), t.text).size().x;
    let (r, resp) = ui.allocate_exact_size(vec2(tw + 18.0, menu_h - 4.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(r, 0.0, t.hover);
    }
    ui.painter().text(r.center(), Align2::CENTER_CENTER, label.clone(), font, t.accent);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
    if resp.clicked() {
        app.open_backstage(crate::backstage::Page::Home);
    }
    if app.keytips.prefix() == Some("")
        && let Some(sequence) = keytip_table::tab_sequence("File")
    {
        keytip(app, ui, sequence, pos2(r.center().x, r.bottom() - 1.0));
    }
}

/// One of the top bar's menus (Help): the items `crate::menus::tree()` lists under `title`.
fn menu(app: &mut SheetApp, ui: &mut Ui, title: &str, menu_h: f32) {
    let t = Tokens::get(ui.ctx());
    let lang = app.ui.language;
    let Some((_, items)) = crate::menus::tree().into_iter().find(|(m, _)| *m == title) else { return };
    let font = theme::ui_font(13.0);
    let label = lang.tr(title).to_string();
    let tw = ui.painter().layout_no_wrap(label.clone(), font.clone(), t.text).size().x;
    let (r, resp) = ui.allocate_exact_size(vec2(tw + 18.0, menu_h - 4.0), Sense::click());
    let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp));
    if resp.hovered() || open {
        ui.painter().rect_filled(r, 0.0, t.hover);
    }
    ui.painter().text(r.center(), Align2::CENTER_CENTER, label.clone(), font, t.text);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
    if app.keytips.prefix() == Some("")
        && let Some(sequence) = keytip_table::tab_sequence(title)
    {
        keytip(app, ui, sequence, pos2(r.center().x, r.bottom() - 1.0));
    }
    egui::Popup::menu(&resp).show(|ui| {
        ui.set_min_width(230.0);
        for (item, id) in &items {
            if *item == "-" {
                ui.separator();
                continue;
            }
            let button = egui::Button::new(lang.tr(item)).frame_when_inactive(false).min_size(vec2(230.0, 24.0));
            if ui.add(button).clicked() {
                let ctx = ui.ctx().clone();
                crate::menus::run(app, &ctx, id);
                ui.close();
            }
        }
    });
}

/// The name of a tab's last group, which no separator closes.
fn last_group(tab: &str) -> &'static str {
    match tab {
        "Insert" => "Text",
        "Draw" => "Convert",
        "Page Layout" => "Sheet Options",
        "Formulas" => "Calculation",
        "Data" => "Solver",
        "Review" => "Protect",
        "View" => "Interface",
        "Automate" => "Automate",
        "Table Design" => "Table Styles",
        "Chart Design" => "Chart",
        crate::paper_view::TAB => "",
        _ => "Editing",
    }
}

/// How much of the active tab's groups the ribbon card shows, recorded each frame for the scroll
/// arrows and for tests.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RibbonOverflow {
    /// Width of all the tab's groups.
    pub content: f32,
    /// Width of the card's content area.
    pub visible: f32,
    /// How far the groups are scrolled.
    pub offset: f32,
    /// Height of the tallest group (a column of small buttons) and of the content area.
    pub content_height: f32,
    pub visible_height: f32,
}

impl RibbonOverflow {
    /// Returns the stable id for the ribbon overflow state.
    fn id() -> egui::Id {
        egui::Id::unique("ribbon_overflow")
    }

    /// Where an arrow click asks the next frame's `ScrollArea` to scroll to.
    fn scroll_target_id(tab: &str) -> egui::Id {
        egui::Id::unique(("ribbon_scroll_target", tab))
    }

    /// Last frame's overflow.
    pub fn get(ctx: &egui::Context) -> RibbonOverflow {
        ctx.data(|d| d.get_temp(Self::id())).unwrap_or_default()
    }

    /// Whether the groups are wider than the card.
    pub fn overflows(&self) -> bool {
        self.content > self.visible + 0.5
    }
}

/// When the tab's groups are wider than the card, arrows at its edges scroll them (horizontal wheel or
/// trackpad input over the card still does too).
fn scroll_arrows(ui: &mut Ui, t: &Tokens, tab: &str, scroll: egui::scroll_area::ScrollAreaOutput<()>) {
    let inner = scroll.inner_rect;
    let max = (scroll.content_size.x - inner.width()).max(0.0);
    let overflow = RibbonOverflow {
        content: scroll.content_size.x,
        visible: inner.width(),
        offset: scroll.state.clamped_offset().x,
        content_height: scroll.content_size.y,
        visible_height: inner.height(),
    };
    ui.ctx().data_mut(|d| d.insert_temp(RibbonOverflow::id(), overflow));
    if !overflow.overflows() {
        return;
    }
    let step = (inner.width() * 0.6).max(60.0);
    let mut target = None;
    for (left, show) in [(true, overflow.offset > 0.5), (false, overflow.offset < max - 0.5)] {
        if !show {
            continue;
        }
        let r = if left {
            Rect::from_min_size(inner.left_top(), vec2(18.0, inner.height()))
        } else {
            Rect::from_min_size(pos2(inner.right() - 18.0, inner.top()), vec2(18.0, inner.height()))
        };
        let resp = ui.interact(r, ui.scope_id().with(("ribbon_scroll_arrow", left)), Sense::click());
        ui.painter().rect_filled(r, 4.0, if resp.hovered() { t.hover } else { t.ribbon });
        ui.painter().rect_stroke(r, 4.0, Stroke::new(1.0, t.ribbon_border), StrokeKind::Inside);
        icons::paint(ui.painter(), Rect::from_center_size(r.center(), vec2(14.0, 14.0)), if left { Icon::Left } else { Icon::Right }, t.text);
        let name = if left { "Scroll ribbon left" } else { "Scroll ribbon right" };
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
        if resp.on_hover_text(crate::i18n::current(ui.ctx()).tr(name)).clicked() {
            target = Some(if left { overflow.offset - step } else { overflow.offset + step });
        }
    }
    if let Some(x) = target {
        let frame = ui.ctx().cumulative_frame_nr();
        ui.ctx().data_mut(|d| d.insert_temp(RibbonOverflow::scroll_target_id(tab), (frame, x.clamp(0.0, max))));
        ui.ctx().request_repaint();
    }
}

/// System / Light / Dark, as in the View tab's Display theme box (`view.theme`).
fn theme_choices(app: &mut SheetApp, ui: &mut Ui) {
    let lang = app.ui.language;
    let mode = app.ui.theme_mode();
    for (value, label) in [("system", "System"), ("light", "Light"), ("dark", "Dark")] {
        if ui.radio(mode == value, lang.tr(label)).clicked() {
            act(app, "view.theme", json!({"mode": value}));
            ui.close();
        }
    }
}

const CONTEXTUAL_TABS: [&str; 3] = ["Table Design", "Chart Design", crate::paper_view::TAB];

fn contextual_tabs(app: &SheetApp) -> Vec<&'static str> {
    let mut v = Vec::new();
    if let Some(d) = app.session.active()
        && let Some(sh) = d.wb.active()
    {
        if sh.table_at(d.selection.active).is_some() {
            v.push("Table Design");
        }
        if app.selected_chart.is_some_and(|id| sh.charts.iter().any(|c| c.id == id)) {
            v.push("Chart Design");
        }
        if app.grid.hf_edit.is_some() {
            v.push(crate::paper_view::TAB);
        }
    }
    v
}
