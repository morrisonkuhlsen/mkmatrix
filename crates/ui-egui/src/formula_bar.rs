//! The formula bar (Name Box, ✕ ✓ fx, formula field) and the shared cell editor widget.

use egui::{FontId, Key, Rect, Sense, TextEdit, pos2, vec2};
use serde_json::json;

use crate::SheetApp;
use crate::editor::formula_refs;
use crate::icons::{self, Icon};
use crate::theme::{self, Tokens};

/// Lines the formula bar (and the in-cell editor) grows to before it scrolls.
pub const MAX_LINES: usize = 5;
/// The bar's height at one line.
const ONE_LINE: f32 = 30.0;
/// What the bar adds around the field's rows: the panel's margins (3 + 3), the field's (3 + 3)
/// and two pixels for the rounding to the pixel grid.
const PAD: f32 = 14.0;

/// The bar's height for one line of a font whose rows are `row_h` tall: Excel's 30 px, unless
/// the rows need more (the Windows UI font's row is 17.28 px). Every line counted must fit in
/// the field, whatever the font; `n` lines take `one_line(row_h) + (n - 1) * row_h`.
pub fn one_line(row_h: f32) -> f32 {
    ONE_LINE.max(row_h + PAD)
}
/// How long the bar takes to grow or shrink to a new line count.
const EASE_SECONDS: f32 = 0.12;

/// Where the formula bar put its parts last frame (for tests).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarLayout {
    pub name_box: Rect,
    /// ✕ ✓ fx.
    pub buttons: Rect,
    pub field: Rect,
    pub height: f32,
    /// Laid-out lines of the content (wrapped to the field), and how many show without scrolling.
    pub lines: usize,
    pub visible_lines: usize,
    /// The caret while editing in the bar.
    pub caret: Option<Rect>,
}

impl Default for BarLayout {
    fn default() -> Self {
        BarLayout { name_box: Rect::NOTHING, buttons: Rect::NOTHING, field: Rect::NOTHING, height: 0.0, lines: 1, visible_lines: 1, caret: None }
    }
}

impl BarLayout {
    /// Returns the stable widget id used for the formula-bar layout state.
    pub fn id() -> egui::Id {
        egui::Id::unique("formula_bar.layout")
    }

    /// Last frame's layout.
    pub fn get(ctx: &egui::Context) -> BarLayout {
        ctx.data(|d| d.get_temp(Self::id())).unwrap_or_default()
    }
}

/// What the editor widget laid out: its line count and caret.
#[derive(Clone, Copy, Debug, Default)]
pub struct EditorInfo {
    pub lines: usize,
    pub caret: Option<Rect>,
}

/// Draws the formula bar: line-count-driven height, Name Box, the Insert Function button, the editable field, muted read-only spill text and the expansion chevron.
pub fn show(app: &mut SheetApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let font = theme::ui_font(13.0);
    let row_h = ui.ctx().fonts_mut(|f| f.row_height(&font));
    let last = BarLayout::get(ui.ctx());
    // Height follows the content's line count (last frame's), 1 to MAX_LINES; the chevron pins it
    // at MAX_LINES. It eases only when that target changes.
    let lines = if app.ui.formula_bar_expanded { MAX_LINES } else { last.lines.clamp(1, MAX_LINES) };
    // Compact: two points less margin above and below the bar's first line.
    let (squeeze, pad) = if app.ui.density.compact() { (4.0, 1) } else { (0.0, 3) };
    let target = one_line(row_h) - squeeze + (lines - 1) as f32 * row_h;
    let h = ui.ctx().animate_value_with_time(egui::Id::unique("formula_bar.height"), target, EASE_SECONDS);
    let mut layout = BarLayout { height: h, ..Default::default() };
    egui::Panel::top("formula_bar")
        .exact_size(h)
        // No line between the bar and the column headings.
        .show_separator_line(false)
        .frame(egui::Frame::NONE.fill(t.window).inner_margin(egui::Margin { left: 8, right: 8, top: pad, bottom: pad }))
        .show(ui, |ui| {
            // Centred: as the bar grows (eased), the Name Box and buttons glide to its middle.
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                // The Name Box is as tall as the formula field at one line.
                let box_h = one_line(row_h) - squeeze - 2.0 * f32::from(pad);
                layout.name_box = name_box(app, ui, &t, box_h);
                ui.add_space(4.0);
                contents(app, ui, &t, &font, row_h, false, &mut layout);
            });
        });
    ui.ctx().data_mut(|d| d.insert_temp(BarLayout::id(), layout));
}

/// The bar's working part, after the Name Box: Insert Function, the formula field (editable,
/// darker under the pointer and while typing, a spill child's formula muted) and the expand
/// chevron. Shared by the docked bar and the floating one (`floating`, whose chevron also
/// takes back a size dragged with its edges); records the field in `layout`.
#[allow(clippy::too_many_arguments)]
fn contents(app: &mut SheetApp, ui: &mut egui::Ui, t: &Tokens, font: &FontId, row_h: f32, floating: bool, layout: &mut BarLayout) {
    // No Cancel / Enter buttons: Esc and Enter do that.
    // Insert Function: the fx drawn large, the bar's one label-like button.
    let (fx_rect, fx) = ui.allocate_exact_size(vec2(34.0, 24.0), egui::Sense::click());
    if fx.hovered() {
        ui.painter().rect_filled(fx_rect, 0.0, t.hover);
    }
    icons::paint(ui.painter(), egui::Rect::from_center_size(fx_rect.center(), vec2(24.0, 24.0)), Icon::Fx, t.text);
    let fx = fx.on_hover_text(app.ui.language.tr("Insert Function"));
    fx.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Insert Function"));
    if fx.clicked() {
        app.open_dialog("insertFunction", json!({}));
    }
    layout.buttons = fx.rect;
    ui.add_space(4.0);
    // The formula field fills the rest, as tall as the bar.
    // Room at the end for the notation toggle and the chevron.
    let avail = ui.available_width() - 48.0;
    let field = Rect::from_min_size(ui.cursor().min, vec2(avail, ui.available_height()));
    // Darker under the pointer, and darker again while typing in it.
    let typing_here = app.editor.as_ref().is_some_and(|e| e.from_formula_bar);
    let fill = if typing_here {
        t.field_active
    } else if ui.rect_contains_pointer(field) {
        t.field_hover
    } else {
        t.window
    };
    ui.painter().rect_filled(field, 0.0, fill);
    // The shape the formula will have, or why its matrices don't fit, at the field's end.
    let verdict = app.editing_shape();
    let mut inner = field.shrink2(vec2(6.0, 3.0));
    if let Some(v) = &verdict {
        let (text, color) = match v {
            crate::shapes::Verdict::Fits((r, c)) => (format!("→ {r}×{c}"), t.accent),
            crate::shapes::Verdict::Mismatch(why) => (why.clone(), t.danger),
        };
        let galley = ui.painter().layout_no_wrap(text, theme::ui_font(12.0), color);
        let w = galley.size().x.min(field.width() * 0.6);
        let at = egui::pos2(field.right() - 8.0 - w, field.top() + (one_line(row_h).min(field.height()) - galley.size().y) / 2.0);
        ui.painter().with_clip_rect(field).galley(at, galley, color);
        inner.max.x -= w + 12.0;
    }
    // The formula reads from the field's top, whatever the row's centring.
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
    let id = egui::Id::unique("mkmatrix.formula_bar");
    // A spill child shows its anchor's formula muted and read-only.
    let read_only = active_spill_anchor(app).is_some();
    let info = if app.editor.is_some() {
        editor_widget(app, &mut child, id, font.clone(), true, inner.width(), inner.height())
    } else {
        // Show the active cell's content; clicking starts editing in the bar.
        let text = active_input(app);
        let color = if read_only { t.text_disabled } else { t.text };
        let galley = child.painter().layout(text, font.clone(), color, inner.width());
        let lines = galley.rows.len();
        // Each cell's view starts at its top: the offset is kept per cell, not per bar.
        let shown = app.session.active().map(|d| (d.uid, d.wb.active_sheet(), d.selection.active.row, d.selection.active.col));
        egui::ScrollArea::vertical().id_salt(id.with(("view", shown))).max_height(inner.height()).auto_shrink([false, true]).show(&mut child, |ui| {
            if ui.add(egui::Label::new(galley).sense(Sense::click())).clicked() && !read_only {
                app.begin_edit(None, true);
            }
        });
        if !read_only && ui.interact(field, id.with("click"), Sense::click()).clicked() {
            app.begin_edit(None, true);
        }
        EditorInfo { lines, caret: None }
    };
    layout.field = field;
    layout.lines = info.lines.max(1);
    layout.visible_lines = ((inner.height() / row_h).floor() as usize).max(1);
    layout.caret = info.caret;
    ui.allocate_space(vec2(avail, field.height()));
    // Mathematical notation: the formula drawn as maths in a panel under the field.
    let ink = if app.ui.math_notation { t.accent } else { t.text_dim };
    let math = crate::widgets::icon_button(ui, Icon::Sum, ink, app.ui.language.tr("Mathematical Notation"), vec2(22.0, 24.0));
    if math.clicked() {
        app.ui.math_notation = !app.ui.math_notation;
    }
    // The floating bar is "expanded" while the chevron or a dragged edge made it bigger than its
    // formula; the chevron then brings it back to the formula's size.
    let expanded = if floating {
        app.floating_bar_expanded || app.ui.floating_bar_lines.is_some() || app.ui.floating_bar_width.is_some()
    } else {
        app.ui.formula_bar_expanded
    };
    let chevron = if expanded { Icon::ChevronUp } else { Icon::Chevron };
    if crate::widgets::icon_button(ui, chevron, t.text_dim, "Expand formula bar", vec2(20.0, 24.0)).clicked() {
        if floating {
            toggle_floating_expand(app);
        } else {
            app.ui.formula_bar_expanded = !app.ui.formula_bar_expanded;
        }
    }
    if app.ui.math_notation && app.dialog.is_none() {
        math_panel(app, ui.ctx(), field, t);
    }
}

/// The formula being edited (or the active cell's) in canonical syntax, when there is one.
fn math_source(app: &SheetApp) -> Option<String> {
    let d = app.session.active()?;
    if let Some(ed) = &app.editor {
        let text = crate::formula_locale::canonical(&ed.text, ed.locale, &d.wb, ed.sheet, ed.cell).ok()?;
        return text.starts_with('=').then_some(text);
    }
    let sh = d.wb.active()?;
    let c = sh.cell(d.selection.active)?;
    c.formula.as_ref()?;
    Some(c.input_text())
}

/// The formula in mathematical notation, in a card just under the formula field.
fn math_panel(app: &SheetApp, ctx: &egui::Context, field: Rect, t: &Tokens) {
    let Some(node) = math_source(app).and_then(|f| crate::math_notation::from_formula(&f)) else { return };
    let size = 17.0;
    egui::Area::new(egui::Id::unique("mkmatrix.math_notation")).order(egui::Order::Foreground).fixed_pos(field.left_bottom() + vec2(0.0, 4.0)).show(
        ctx,
        |ui| {
            egui::Frame::new()
                .fill(t.window)
                .stroke(egui::Stroke::new(1.0, t.input_border))
                .corner_radius(6.0)
                .inner_margin(egui::Margin::symmetric(14, 10))
                .shadow(egui::epaint::Shadow { offset: [0, 3], blur: 12, spread: 0, color: egui::Color32::from_black_alpha(40) })
                .show(ui, |ui| {
                    let m = crate::math_notation::measure(&node, ui.painter(), size);
                    // A long formula is clipped to the field's width rather than running off.
                    let w = m.width.min(field.width().max(200.0) - 28.0);
                    let (r, resp) = ui.allocate_exact_size(vec2(w, m.height()), Sense::hover());
                    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, "Mathematical Notation"));
                    crate::math_notation::show(&node, &ui.painter().with_clip_rect(r), r.min, size, t.text);
                });
        },
    );
}

/// The floating formula bar: over the sheet, just above the active cell (below it when the cell is
/// near the top), as wide as a comfortable formula. F2 brings it up and edits in it, closing it
/// again when the edit ends; Ctrl pressed twice shows or hides it; Esc puts it away. It steps aside
/// while a dialog is open (the Formula Builder works on the same edit). Works with
/// `UiState::floating_formula_bar`.
///
/// The grip at its left end moves it: dragged, the bar stays where it is dropped
/// (`UiState::floating_bar_at`) instead of following the cell, and a double-click on the grip
/// sends it back. Its right edge sets the width and its bottom edge the rows it shows (the corner
/// both), as far as the grid has room; a double-click on an edge puts the size back.
///
/// Left to itself it follows the formula: wider as a line gets longer (up to most of the grid's
/// width, then it wraps), taller as lines are added (up to half the cells' height, then it
/// scrolls); the chevron shows the whole formula (five rows at least). It stays over the cells,
/// clear of the headings, with its controls on its first line. Its size eases, and it fades in.
pub fn floating(app: &mut SheetApp, ctx: &egui::Context) {
    floating_keys(app, ctx);
    // Alt+Shift+F lays the formula out on several lines, or back on one; taken before the field
    // sees it (with its letter, which some systems also send as text).
    if app.floating_bar_open
        && app.dialog.is_none()
        && ctx.input_mut(|i| {
            let hit = i.consume_key(egui::Modifiers::ALT | egui::Modifiers::SHIFT, Key::F);
            if hit {
                i.events.retain(|e| !matches!(e, egui::Event::Text(t) if t.eq_ignore_ascii_case("f")));
            }
            hit
        })
    {
        toggle_layout(app);
    }
    let Some(grid) = app.grid.rect else { return };
    // An edit in the bar keeps it up, and its end (Enter, Esc, a click away) puts it away.
    let typing = app.editor.as_ref().is_some_and(|e| e.from_formula_bar);
    let was_typing_id = egui::Id::unique("mkmatrix.floating_bar.typing");
    let was_typing = ctx.data(|d| d.get_temp::<bool>(was_typing_id)).unwrap_or(false);
    ctx.data_mut(|d| d.insert_temp(was_typing_id, typing));
    if typing {
        app.floating_bar_open = true;
    } else if was_typing && app.dialog.is_none() {
        app.floating_bar_open = false;
    }
    // A size made for one cell's formula (the chevron, a dragged edge) doesn't carry over: closed,
    // or moved to another cell, the bar is sized to the formula it shows.
    let at = app.session.active().map(|d| (d.uid, d.wb.active_sheet(), d.selection.active.row, d.selection.active.col));
    let shown_id = egui::Id::unique("mkmatrix.floating_bar.on");
    let before = ctx.data(|d| d.get_temp::<(bool, Option<(u64, usize, u32, u32)>)>(shown_id));
    if before.is_some_and(|(was_open, was_at)| was_open && (!app.floating_bar_open || was_at != at)) {
        reset_floating_size(app);
    }
    ctx.data_mut(|d| d.insert_temp(shown_id, (app.floating_bar_open, at)));
    if !app.floating_bar_open || app.dialog.is_some() {
        // Faded out while away, so it fades in when it comes back.
        ctx.animate_bool_with_time(egui::Id::unique(SHOWN), false, EASE_SECONDS);
        return;
    }
    // Each sheet keeps where the bar was dropped on it: another sheet brings its own place.
    let sheet_name = app.session.active().and_then(|d| d.wb.active()).map(|sh| sh.name.clone()).unwrap_or_default();
    let sheet_id = egui::Id::unique("mkmatrix.floating_bar.sheet");
    if ctx.data(|d| d.get_temp::<String>(sheet_id)).is_some_and(|was| was != sheet_name) {
        app.ui.floating_bar_at = app.ui.floating_bar_at_sheets.get(&sheet_name).copied();
    }
    ctx.data_mut(|d| d.insert_temp(sheet_id, sheet_name.clone()));
    // The cell being edited (the active one without an edit), the range being pointed at while a
    // formula is typed, and the cells' area (the grid without its headings), which the bar stays in.
    let Some((cell, pointed, area)) = app.session.active().and_then(|d| {
        let sh = d.wb.active()?;
        let si = d.wb.active_sheet();
        let geo = crate::grid::Geo::new(sh, grid, app.view().scroll);
        let editing = app.editor.as_ref().filter(|e| e.sheet == si).map_or(d.selection.active, |e| e.cell);
        let pointed = app.editor.as_ref().filter(|e| e.point.is_some()).and_then(|e| {
            let to = e.point_cell?;
            let from = e.point_anchor.filter(|(s, _)| *s == si).map_or(to, |(_, c)| c);
            let r = mkmatrix_engine::core::RangeRef::new(from, to);
            Some(geo.cell_rect(sh, r.start).union(geo.cell_rect(sh, r.end)))
        });
        Some((geo.cell_rect(sh, editing), pointed, geo.cells))
    }) else {
        return;
    };
    let t = Tokens::get(ctx);
    let font = theme::ui_font(13.0);
    let row_h = ctx.fonts_mut(|f| f.row_height(&font));
    let last = BarLayout::get(ctx);
    // The rows the cells' area has room for (the most the bottom edge and the chevron give), and
    // the rows the formula grows to by itself.
    let room = ((area.height() - 12.0 - FOOTER_H - one_line(row_h)) / row_h.max(1.0)).floor().max(0.0) as usize + 1;
    let auto = ((area.height() * 0.5 - one_line(row_h)) / row_h.max(1.0)).floor().max(0.0) as usize + 1;
    let auto = auto.max(MAX_LINES).min(room);
    let at_least = app.ui.floating_bar_lines.map_or(1, usize::from);
    // The chevron shows the whole formula (five rows at least), as far as there's room.
    let lines = if app.floating_bar_expanded { last.lines.max(MAX_LINES) } else { last.lines.min(auto) };
    let lines = lines.max(at_least).clamp(1, room);
    let widest = (area.width() - 16.0).max(FLOATING_MIN_W);
    let w = match app.ui.floating_bar_width {
        Some(w) => w,
        // As wide as the formula's longest line, between the comfortable width and most of the grid.
        None => (natural_width(app, ctx, &font) + FLOATING_CHROME).clamp(FLOATING_W, (area.width() * 0.85).max(FLOATING_W)),
    }
    .min(widest)
    .max(FLOATING_MIN_W);
    // Eased, except under the pointer: a dragged edge follows it exactly.
    let resizing = ctx.data(|d| d.get_temp::<bool>(egui::Id::unique(RESIZING))).unwrap_or(false);
    let ease = if resizing { 0.0 } else { EASE_SECONDS };
    let h = ctx.animate_value_with_time(egui::Id::unique("mkmatrix.floating_bar.h"), one_line(row_h) + (lines - 1) as f32 * row_h, ease);
    let w = ctx.animate_value_with_time(egui::Id::unique("mkmatrix.floating_bar.w"), w, ease);
    let shown = ctx.animate_bool_with_time(egui::Id::unique(SHOWN), true, EASE_SECONDS);
    // Where it was dropped, or above the cell with its left edge on the cell's; over the cells,
    // clear of the headings.
    let gap = 6.0;
    let (x, y) = match app.ui.floating_bar_at {
        Some([dx, dy]) => (grid.left() + dx, grid.top() + dy),
        None => {
            let above = cell.top() - gap - h - FOOTER_H - 4.0;
            (cell.left(), if above >= area.top() + 4.0 { above } else { cell.bottom() + gap })
        }
    };
    let x = x.clamp(area.left() + 4.0, (area.right() - w - 8.0).max(area.left() + 4.0));
    let y = y.clamp(area.top() + 4.0, (area.bottom() - h - FOOTER_H - 8.0).max(area.top() + 4.0));
    // Out of the way of the range being pointed at: below it, or else above it.
    let total = h + FOOTER_H + 4.0;
    let y = match pointed {
        Some(r) if Rect::from_min_size(pos2(x, y), vec2(w + 8.0, total)).intersects(r.expand(2.0)) => {
            if r.bottom() + gap + total <= area.bottom() - 8.0 {
                r.bottom() + gap
            } else if r.top() - gap - total >= area.top() + 4.0 {
                r.top() - gap - total
            } else {
                y
            }
        }
        _ => y,
    };
    // Fainter while it waits: away from the pointer, with nothing typed in it or a range being
    // dragged out on the sheet.
    let last_rect = ctx.data(|d| d.get_temp::<Rect>(egui::Id::unique("mkmatrix.floating_bar.rect")));
    let (hover, down) = ctx.input(|i| (i.pointer.hover_pos(), i.pointer.primary_down()));
    let over = hover.is_some_and(|p| last_rect.is_some_and(|r| r.expand(6.0).contains(p)));
    let waiting = !over && (!typing || (down && pointed.is_some()));
    let solid = ctx.animate_value_with_time(egui::Id::unique("mkmatrix.floating_bar.solid"), if waiting { FAINT } else { 1.0 }, EASE_SECONDS);
    // A bar dropped away from its cell points at it: a thin line to the cell, outlined.
    if app.ui.floating_bar_at.is_some() {
        let bar = Rect::from_min_size(pos2(x, y), vec2(w + 8.0, total));
        if !bar.expand(gap + 2.0).intersects(cell) {
            // From the bar's edge nearest the cell to the cell's edge nearest the bar.
            let clamp = |r: Rect, q: egui::Pos2| pos2(q.x.clamp(r.left(), r.right()), q.y.clamp(r.top(), r.bottom()));
            let from = clamp(bar.shrink(6.0), cell.center());
            let near = clamp(cell, from);
            let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::unique("mkmatrix.floating_bar.link"))).with_clip_rect(area);
            let ink = t.accent.gamma_multiply(0.7 * shown);
            p.line_segment([from, near], egui::Stroke::new(1.0, ink));
            p.rect_stroke(cell, 0.0, egui::Stroke::new(1.5, ink), egui::StrokeKind::Inside);
        }
    }
    let mut layout = BarLayout { height: h, ..Default::default() };
    let mut moved: Option<egui::Vec2> = None;
    let mut reset_at = false;
    egui::Area::new(egui::Id::unique("mkmatrix.floating_formula_bar")).order(egui::Order::Foreground).fixed_pos(pos2(x, y)).show(ctx, |ui| {
        ui.multiply_opacity(shown * solid);
        let frame = egui::Frame::new()
            .fill(t.window)
            .stroke(egui::Stroke::new(1.0, t.input_border))
            .corner_radius(6.0)
            .inner_margin(egui::Margin::symmetric(4, 2))
            .shadow(egui::epaint::Shadow { offset: [0, 3], blur: 12, spread: 0, color: egui::Color32::from_black_alpha(48) })
            .show(ui, |ui| {
                ui.set_width(w);
                ui.set_height(h + FOOTER_H);
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.allocate_ui_with_layout(vec2(w, h), egui::Layout::left_to_right(egui::Align::Min), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let name = crate::i18n::current(ui.ctx()).tr("Move the formula bar").to_owned();
                    // Its controls sit on the first line, however tall the bar gets.
                    let grip = grip(ui, h, one_line(row_h), &t, &name);
                    if grip.dragged() {
                        moved = Some(grip.drag_delta());
                    }
                    reset_at = grip.double_clicked();
                    contents(app, ui, &t, &font, row_h, true, &mut layout);
                });
                footer(app, ui, layout.field, &t);
            });
        resize_edges(app, ui, frame.response.rect, grid.min, w, lines, room, row_h, &t);
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::unique("mkmatrix.floating_bar.rect"), frame.response.rect));
    });
    if reset_at {
        app.ui.floating_bar_at = None;
        app.ui.floating_bar_at_sheets.remove(&sheet_name);
    } else if let Some(d) = moved {
        let at = [x + d.x - grid.left(), y + d.y - grid.top()];
        app.ui.floating_bar_at = Some(at);
        app.ui.floating_bar_at_sheets.insert(sheet_name.clone(), at);
    }
    ctx.data_mut(|d| d.insert_temp(BarLayout::id(), layout));
}

/// The floating bar's opacity while it waits (see [`floating`]).
const FAINT: f32 = 0.55;

/// The strip under the floating bar's field.
const FOOTER_H: f32 = 20.0;

/// The floating bar's footer, under the field (`field`): the cell being edited (with its sheet
/// when the edit is on another one), the formula's value as typed, and the layout button.
fn footer(app: &mut SheetApp, ui: &mut egui::Ui, field: Rect, t: &Tokens) {
    let (strip, _) = ui.allocate_exact_size(vec2(ui.available_width(), FOOTER_H), Sense::hover());
    let font = theme::ui_font(11.5);
    let lang = crate::i18n::current(ui.ctx());
    let painter = ui.painter().with_clip_rect(strip);
    let mid = strip.center().y;
    // The layout button at the right end, under the chevron.
    let pretty = app.editor.as_ref().is_some_and(|e| e.text.contains('\n'));
    let button = Rect::from_center_size(pos2(strip.right() - 14.0, mid), vec2(22.0, FOOTER_H - 2.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(button));
    let ink = if pretty { t.accent } else { t.text_dim };
    let tip = format!("{} (Alt+Shift+F)", lang.tr("Lay out the formula on several lines"));
    if crate::widgets::icon_button(&mut child, Icon::IndentInc, ink, &tip, vec2(20.0, FOOTER_H - 2.0)).clicked() {
        toggle_layout(app);
    }
    let left = field.left().max(strip.left()) + 6.0;
    let label = editing_label(app);
    let place = painter.text(pos2(left, mid), egui::Align2::LEFT_CENTER, &label, font.clone(), t.text_dim);
    // Both read out too (and found by tests).
    let read_out = |ui: &mut egui::Ui, r: Rect, what: &str, text: String| {
        ui.interact(r, egui::Id::unique(("mkmatrix.floating_bar.footer", what)), Sense::hover())
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, text.clone()));
    };
    read_out(ui, place, "cell", label);
    if let Some((text, error)) = live_value(app, ui.ctx()) {
        let color = if error { t.danger } else { t.text_dim };
        let galley = painter.layout_no_wrap(format!("= {text}"), font, color);
        let room = (button.left() - 8.0) - (place.right() + 16.0);
        if room > 24.0 {
            let w = galley.size().x.min(room);
            let at = Rect::from_min_max(pos2(button.left() - 8.0 - w, strip.top()), pos2(button.left() - 8.0, strip.bottom()));
            read_out(ui, at, "value", format!("= {text}"));
            painter.with_clip_rect(Rect::from_min_max(pos2(button.left() - 8.0 - w, strip.top()), pos2(button.left() - 8.0, strip.bottom()))).galley(
                pos2(button.left() - 8.0 - w, mid - galley.size().y / 2.0),
                galley,
                color,
            );
        }
    }
}

/// Where the bar edits: the cell as the Name Box writes it, after its sheet when that isn't the
/// active one (pointing can take the edit's references to another sheet).
fn editing_label(app: &SheetApp) -> String {
    let Some(d) = app.session.active() else { return String::new() };
    let (sheet, cell) = app.editor.as_ref().map_or((d.wb.active_sheet(), d.selection.active), |e| (e.sheet, e.cell));
    let at = if app.ui.numbered_columns { format!("({}, {})", u64::from(cell.row) + 1, u64::from(cell.col) + 1) } else { cell.a1() };
    match d.wb.sheet(sheet) {
        Some(sh) if sheet != d.wb.active_sheet() => format!("{}!{at}", mkmatrix_engine::core::quote_sheet(&sh.name)),
        _ => at,
    }
}

/// The value of the formula being typed (or of the cell's formula), in the cell's number format,
/// and whether it is an error. Worked out again only when the text or the workbook changes.
fn live_value(app: &SheetApp, ctx: &egui::Context) -> Option<(String, bool)> {
    let d = app.session.active()?;
    let (sheet, cell, text, locale) = match &app.editor {
        Some(e) => (e.sheet, e.cell, e.text.clone(), e.locale),
        None => {
            let sh = d.wb.active()?;
            let c = sh.cell(d.selection.active)?;
            c.formula.as_ref()?;
            (d.wb.active_sheet(), d.selection.active, c.input_text(), Default::default())
        }
    };
    if !text.starts_with('=') {
        return None;
    }
    let id = egui::Id::unique("mkmatrix.floating_bar.value");
    let key = (d.uid, d.revision, sheet, cell.row, cell.col, text.clone());
    if let Some((k, v)) = ctx.data(|m| m.get_temp::<((u64, u64, usize, u32, u32, String), Option<(String, bool)>)>(id))
        && k == key
    {
        return v;
    }
    let shown = (|| {
        let canon = crate::formula_locale::canonical(&text, locale, &d.wb, sheet, cell).ok()?;
        let v = mkmatrix_engine::calc::evaluate(&d.wb, sheet, cell, &canon);
        let sh = d.wb.sheet(sheet)?;
        let fmt = d.wb.styles.get(sh.style_id(cell)).num_fmt.as_str().to_string();
        let one = |v: &mkmatrix_engine::core::Value| mkmatrix_engine::display::format(v, &fmt, &d.wb).text;
        Some(match &v {
            mkmatrix_engine::core::Value::Error(e) => (e.as_str().to_string(), true),
            mkmatrix_engine::core::Value::Array(a) => {
                let first = a.data.first().map(one).unwrap_or_default();
                (format!("{first} … ({}×{})", a.rows, a.cols), false)
            }
            v => (one(v), false),
        })
    })();
    ctx.data_mut(|m| m.insert_temp(id, (key, shown.clone())));
    shown
}

/// Lays the formula being edited out with each long call's arguments on their own lines, or puts
/// a laid-out one back on one line (the cell keeps the formula either way). Without an edit, it
/// starts one in the bar first. The caret stays by the same character.
pub(crate) fn toggle_layout(app: &mut SheetApp) {
    if app.editor.is_none() {
        app.begin_edit(None, true);
    }
    let Some(ed) = app.editor.as_mut().filter(|e| e.is_formula()) else { return };
    let new = if ed.text.contains('\n') {
        mkmatrix_engine::formula::pretty::compact(&ed.text, ed.locale)
    } else {
        mkmatrix_engine::formula::pretty::pretty(&ed.text, ed.locale)
    };
    let Some(new) = new.filter(|n| *n != ed.text) else { return };
    let kept = ed.text.chars().take(ed.caret).filter(|c| !c.is_whitespace()).count();
    let mut seen = 0;
    let caret = new
        .chars()
        .position(|c| {
            if seen == kept {
                return true;
            }
            seen += usize::from(!c.is_whitespace());
            false
        })
        .unwrap_or(new.chars().count());
    ed.text = new;
    ed.caret = caret;
    ed.anchor = caret;
    ed.sync_caret = true;
    ed.request_focus = true;
    ed.point = None;
    ed.autocomplete.clear();
    ed.completion = None;
}

/// The floating bar's chevron (and Ctrl+Shift+U while it's up): the whole formula shows, or, on a
/// bar the chevron or a dragged edge made bigger than its formula, back to the formula's size.
/// Whether it is now expanded.
pub(crate) fn toggle_floating_expand(app: &mut SheetApp) -> bool {
    if app.floating_bar_expanded || app.ui.floating_bar_lines.is_some() || app.ui.floating_bar_width.is_some() {
        reset_floating_size(app);
        false
    } else {
        app.floating_bar_expanded = true;
        true
    }
}

/// Back to the formula's own size: no chevron, no dragged width or rows.
fn reset_floating_size(app: &mut SheetApp) {
    app.floating_bar_expanded = false;
    app.ui.floating_bar_width = None;
    app.ui.floating_bar_lines = None;
}

/// The floating formula bar's narrowest width.
const FLOATING_MIN_W: f32 = 240.0;
/// Its width while the formula fits: a comfortable formula.
const FLOATING_W: f32 = 560.0;
/// What the bar takes besides the formula's text: the grip, fx, the field's padding, the notation
/// toggle, the chevron, spacing and the frame.
const FLOATING_CHROME: f32 = 136.0;
/// Set while an edge of the floating bar is being dragged (its size then doesn't ease).
const RESIZING: &str = "mkmatrix.floating_bar.resizing";
/// The floating bar's fade in.
const SHOWN: &str = "mkmatrix.floating_bar.shown";

/// The width of the longest line of what the bar shows: the edit's text, or the active cell's.
fn natural_width(app: &SheetApp, ctx: &egui::Context, font: &FontId) -> f32 {
    let text = app.editor.as_ref().map_or_else(|| active_input(app), |e| e.text.clone());
    // Only the first few thousand characters: past that the bar is at its widest anyway.
    let text: String = text.chars().take(4000).collect();
    ctx.fonts_mut(|f| text.lines().map(|l| f.layout_no_wrap(l.to_owned(), font.clone(), egui::Color32::WHITE).size().x).fold(0.0, f32::max))
}

/// The floating bar's grip: two columns of dots at its left end, dragged to move the bar.
/// Its dots sit in the middle of the bar's first line (`first` tall).
fn grip(ui: &mut egui::Ui, h: f32, first: f32, t: &Tokens, name: &str) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(10.0, h), egui::Sense::click_and_drag());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
    let ink = if resp.hovered() || resp.dragged() { t.accent } else { t.text_dim };
    for i in 0..3 {
        for j in 0..2 {
            let c = pos2(r.center().x - 2.0 + 4.0 * j as f32, r.top() + first.min(h) / 2.0 - 4.0 + 4.0 * i as f32);
            ui.painter().circle_filled(c, 1.1, ink);
        }
    }
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(if resp.dragged() { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Grab });
    }
    resp.on_hover_text(name)
}

/// The floating bar's right edge (width), bottom edge (rows) and their corner (both), dragged
/// from the size at the press; a double-click on any of them puts the size back. Taking the
/// bottom edge or the corner pins the bar where it is (`grid` is the grid's top-left), so it
/// grows down under the pointer instead of up from the cell it sits above.
#[allow(clippy::too_many_arguments)]
fn resize_edges(app: &mut SheetApp, ui: &mut egui::Ui, bar: Rect, grid: egui::Pos2, w: f32, lines: usize, room: usize, row_h: f32, t: &Tokens) {
    let lang = crate::i18n::current(ui.ctx());
    let start_id = egui::Id::unique("mkmatrix.floating_bar.resize_start");
    let mut resizing = false;
    let edges = [
        (
            "e",
            Rect::from_min_max(pos2(bar.right() - 5.0, bar.top() + 4.0), pos2(bar.right() + 3.0, bar.bottom() - 8.0)),
            egui::CursorIcon::ResizeHorizontal,
        ),
        (
            "s",
            Rect::from_min_max(pos2(bar.left() + 8.0, bar.bottom() - 4.0), pos2(bar.right() - 8.0, bar.bottom() + 4.0)),
            egui::CursorIcon::ResizeVertical,
        ),
        (
            "se",
            Rect::from_min_max(pos2(bar.right() - 8.0, bar.bottom() - 8.0), pos2(bar.right() + 4.0, bar.bottom() + 4.0)),
            egui::CursorIcon::ResizeNwSe,
        ),
    ];
    for (side, r, cursor) in edges {
        let resp = ui.interact(r, egui::Id::unique(("mkmatrix.floating_bar.edge", side)), egui::Sense::click_and_drag());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, lang.tr("Resize the formula bar")));
        if resp.hovered() || resp.dragged() {
            ui.ctx().set_cursor_icon(cursor);
        }
        if resp.double_clicked() {
            app.ui.floating_bar_width = None;
            app.ui.floating_bar_lines = None;
            continue;
        }
        if resp.drag_started() {
            ui.ctx().data_mut(|d| d.insert_temp(start_id, (w, lines)));
            if side != "e" {
                app.ui.floating_bar_at = Some([bar.left() - grid.x, bar.top() - grid.y]);
            }
        }
        if resp.dragged() {
            resizing = true;
            let (w0, lines0) = ui.ctx().data(|d| d.get_temp::<(f32, usize)>(start_id)).unwrap_or((w, lines));
            let (Some(origin), Some(now)) = (ui.input(|i| i.pointer.press_origin()), ui.input(|i| i.pointer.latest_pos())) else { continue };
            let d = now - origin;
            if side != "s" {
                app.ui.floating_bar_width = Some((w0 + d.x).max(FLOATING_MIN_W).round());
            }
            if side != "e" {
                let rows = (lines0 as f32 + d.y / row_h.max(1.0)).round().clamp(1.0, room.min(u8::MAX as usize) as f32);
                app.ui.floating_bar_lines = Some(rows as u8);
            }
            // The edge being dragged, in the accent.
            ui.painter().rect_stroke(bar, 6.0, egui::Stroke::new(1.0, t.accent), egui::StrokeKind::Inside);
        }
    }
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::unique(RESIZING), resizing));
}

/// Ctrl pressed twice in a row (alone, each tap within 400 ms of the last) toggles the floating
/// bar; Esc puts it away when nothing is being typed. A tap is Ctrl going up with no other key or
/// click while it was down.
fn floating_keys(app: &mut SheetApp, ctx: &egui::Context) {
    // Read the input first: egui's memory can't be touched inside `input`.
    let (ctrl, other, esc) = ctx.input(|i| {
        let other = i.events.iter().any(|e| match e {
            egui::Event::Key { key, pressed: true, .. } => *key != egui::Key::Escape,
            egui::Event::PointerButton { pressed: true, .. } | egui::Event::Text(_) => true,
            _ => false,
        });
        (i.modifiers.ctrl || i.modifiers.command, other, i.key_pressed(egui::Key::Escape))
    });
    let id = egui::Id::unique("mkmatrix.floating_bar.ctrl");
    // (Ctrl down last frame, something else happened while it was down)
    let (was, dirty) = ctx.data(|d| d.get_temp::<(bool, bool)>(id)).unwrap_or_default();
    let dirty = (dirty && was) || (other && (ctrl || was));
    if was && !ctrl {
        if dirty {
            app.ctrl_tap = None;
        } else {
            let now = crate::now_ms();
            match app.ctrl_tap {
                Some(t0) if now - t0 <= 400.0 => {
                    app.floating_bar_open = !app.floating_bar_open;
                    app.ctrl_tap = None;
                }
                _ => app.ctrl_tap = Some(now),
            }
        }
    } else if other && !ctrl {
        app.ctrl_tap = None;
    }
    ctx.data_mut(|d| d.insert_temp(id, (ctrl, ctrl && dirty)));
    if esc && app.editor.is_none() {
        app.floating_bar_open = false;
    }
}

/// Resolve ownership on every frame; never cache a spill range in presentation state.
pub(crate) fn active_spill_anchor(app: &SheetApp) -> Option<mkmatrix_engine::core::CellRef> {
    let d = app.session.active()?;
    let sh = d.wb.active()?;
    let at = sh.merge_at(d.selection.active).map(|m| m.start).unwrap_or(d.selection.active);
    sh.spill_anchor(at)
}

fn active_input(app: &SheetApp) -> String {
    let Some(d) = app.session.active() else { return String::new() };
    let Some(sh) = d.wb.active() else { return String::new() };
    let a = sh.merge_at(d.selection.active).map(|m| m.start).unwrap_or(d.selection.active);
    if let Some(picture) = sh.cell_pictures.get(&a) {
        return if picture.alt.is_empty() { "Picture in cell".into() } else { format!("Picture: {}", picture.alt) };
    }
    if let Some(anchor) = sh.spill_anchor(a) {
        if mkmatrix_engine::display::formula_hidden(&d.wb, sh, anchor) {
            return String::new();
        }
        return sh
            .cell(anchor)
            .map(|c| crate::formula_locale::display(&c.input_text(), app.ui.language.formula_locale(), &d.wb, d.wb.active_sheet()))
            .unwrap_or_default();
    }
    if let Some(c) = sh.cell(a) {
        if c.formula.is_some() && d.wb.styles.get(c.style).protection.hidden && sh.is_protected() {
            return String::new();
        }
        return if let Some(f) = &c.formula {
            let shown = crate::formula_locale::display(&c.input_text(), app.ui.language.formula_locale(), &d.wb, d.wb.active_sheet());
            // A legacy array formula shows in braces (`{=A1:A2*2}`), as `Cell::bar_text` does.
            if f.array.is_some() { format!("{{{shown}}}") } else { shown }
        } else {
            mkmatrix_engine::display::edit_text(&d.wb, sh, a)
        };
    }
    String::new()
}

/// Draws the Name Box and its name dropdown; Enter jumps to a reference or known name (or defines a new name) and is consumed so the grid does not move down.
pub(crate) fn name_box(app: &mut SheetApp, ui: &mut egui::Ui, t: &Tokens, height: f32) -> Rect {
    let id = egui::Id::unique("mkmatrix.name_box");
    let shown = app.name_box_text();
    let mut text = app.name_box.clone().unwrap_or(shown.clone());
    // Centred in the bar's height, as the row's buttons are.
    let top = ui.cursor().min.y + ((ui.available_height() - height) / 2.0).max(0.0);
    let rect = Rect::from_min_size(pos2(ui.cursor().min.x, top), vec2(118.0, height));
    // Darker under the pointer, and darker again while typing in it, as the formula field is.
    let fill = if ui.memory(|m| m.has_focus(id)) {
        t.field_active
    } else if ui.rect_contains_pointer(rect) {
        t.field_hover
    } else {
        t.window
    };
    ui.painter().rect_filled(rect, 0.0, fill);
    let text_top = rect.top() + ((height - 18.0) / 2.0).max(0.0);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(pos2(rect.left() + 6.0, text_top), vec2(96.0, 18.0))));
    // The box has no visible label of its own: screen readers and the strict accessibility check
    // need a name (egui 0.37 makes unnamed text inputs a test failure).
    let accessible_name = app.ui.language.tr("Name Box").to_string();
    let resp = child
        .add(TextEdit::singleline(&mut text).id(id).frame(egui::Frame::NONE).font(theme::ui_font(13.0)).desired_width(96.0))
        .accessible_name(accessible_name);
    if resp.gained_focus() {
        app.name_box = Some(shown.clone());
    }
    if resp.changed() {
        app.name_box = Some(text.clone());
    }
    if resp.lost_focus() {
        if ui.input(|i| i.key_pressed(Key::Enter)) {
            // The Enter that commits the Name Box is used up here. Left in the frame's input, the
            // grid (drawn after the formula bar, and focused again now the box let go) would read
            // it as its own Enter and move the active cell one row down from the jump target (#247).
            ui.input_mut(|i| i.events.retain(|e| !matches!(e, egui::Event::Key { key: Key::Enter, .. })));
            // `X(3,2)` goes to that place in matrix X; `(21, 18)` to that row and column.
            let in_matrix = app.session.active().and_then(|d| crate::matrices::resolve(&d.wb, d.wb.active_sheet(), &text)).map(|c| c.a1());
            let target = in_matrix.or_else(|| crate::app::row_col_to_a1(text.trim())).unwrap_or_else(|| text.trim().to_string());
            if !target.is_empty() && target != shown {
                // A new name for the selection, or a place to go.
                let is_ref = mkmatrix_engine::core::RangeRef::parse(target.split('!').next_back().unwrap_or("")).is_some();
                let known = app.session.active().is_some_and(|d| d.wb.name(&target, d.wb.active_sheet()).is_some() || d.wb.table(&target).is_some());
                let r = if is_ref || known {
                    app.run("edit.goTo", json!({"reference": target}))
                } else {
                    app.run("formulas.defineName", json!({"name": target}))
                };
                if let Err(e) = r {
                    app.message = Some(("MK Matrix".into(), crate::clean_error(&e)));
                }
                app.grid.ensure_visible = true;
            }
        }
        app.name_box = None;
    }
    ui.allocate_space(vec2(118.0, height));
    // Dropdown of names.
    let names: Vec<String> =
        app.session.active().map(|d| d.wb.names.iter().filter(|n| !n.hidden).map(|n| n.name.clone()).collect()).unwrap_or_default();
    let mb = Rect::from_min_size(pos2(rect.right() - 16.0, rect.center().y - 8.0), vec2(14.0, 16.0));
    icons::paint(ui.painter(), mb, Icon::Chevron, t.text_dim);
    let r = ui.interact(mb, id.with("drop"), Sense::click());
    egui::Popup::menu(&r).show(|ui| {
        if names.is_empty() {
            ui.label(egui::RichText::new("No names defined").italics());
        }
        for n in names {
            if ui.button(&n).clicked() {
                let _ = app.run("edit.goTo", json!({"reference": n}));
                app.grid.ensure_visible = true;
            }
        }
    });
    rect
}

/// Backspace and Delete take back the AutoComplete proposal (Excel's selected rest), not a typed character.
fn dismiss_completion(i: &mut egui::InputState, completion: &mut Option<String>) {
    if completion.is_some() && (i.consume_key(egui::Modifiers::NONE, Key::Backspace) || i.consume_key(egui::Modifiers::NONE, Key::Delete)) {
        *completion = None;
    }
}

/// Column AutoComplete proposes only after typing: deleting characters (Backspace, Delete) brings no proposal back, as in Excel.
fn entry_shrank(chars_before: usize, text: &str) -> bool {
    text.chars().count() < chars_before
}

/// The editor's text field, the same in the cell and in the formula bar. Enter is the editor's
/// own key (it commits, and Alt+Enter breaks the line), so the field itself does nothing with it:
/// a message box over the editor takes Enter to close, and the field must not add a line then.
fn show_edit<'t>(
    edit: TextEdit<'t>,
    ui: &mut egui::Ui,
    id: egui::Id,
    font: FontId,
    width: f32,
    layouter: &'t mut dyn FnMut(&egui::Ui, &dyn egui::TextBuffer, f32) -> std::sync::Arc<egui::Galley>,
) -> egui::text_edit::TextEditOutput {
    edit.id(id)
        .frame(egui::Frame::NONE)
        .font(font)
        .desired_width(width)
        .desired_rows(1)
        .layouter(layouter)
        .lock_focus(true)
        .return_key(None)
        .event_filter(egui::EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true })
        .show(ui)
}

/// The text editor used in the cell and in the formula bar (they share `app.editor`). It shows
/// `max_height` of text and scrolls beyond that, keeping the caret in view.
pub fn editor_widget(app: &mut SheetApp, ui: &mut egui::Ui, id: egui::Id, font: FontId, multiline: bool, width: f32, max_height: f32) -> EditorInfo {
    let Some(mut ed) = app.editor.take() else { return EditorInfo::default() };
    let mine = ed.from_formula_bar == (id == egui::Id::unique("mkmatrix.formula_bar"));
    // Keys the editor handles itself (before the TextEdit sees them), only where it has focus.
    let focused = ui.ctx().memory(|m| m.focused());
    let has_focus = focused == Some(id) || (mine && (ed.request_focus || focused.is_none()));
    let mut action: Option<(i64, i64, bool, bool)> = None;
    let mut arrow = false;
    let mut cancel = false;
    // A message box over the editor (a data validation alert, a formula error) gets Enter and Esc;
    // the editor used to take them, so the first Esc cancelled the edit behind the message.
    // So does a dialog opened from the editor (Paste Name): its OK takes Enter.
    if has_focus && mine && app.message.is_none() && app.dialog.is_none() {
        let (enter, tab, esc, alt_enter, cmd_enter, ctrl_shift_enter, shift) = ui.input_mut(|i| {
            let m = i.modifiers;
            let alt_enter =
                i.consume_key(egui::Modifiers::ALT, Key::Enter) || i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::ALT, Key::Enter);
            let ctrl_shift_enter = i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, Key::Enter);
            let cmd_enter = i.consume_key(egui::Modifiers::COMMAND, Key::Enter);
            let enter = i.consume_key(egui::Modifiers::NONE, Key::Enter) || i.consume_key(egui::Modifiers::SHIFT, Key::Enter);
            let tab = i.consume_key(egui::Modifiers::NONE, Key::Tab) || i.consume_key(egui::Modifiers::SHIFT, Key::Tab);
            let esc = i.consume_key(egui::Modifiers::NONE, Key::Escape);
            dismiss_completion(i, &mut ed.completion);
            (enter, tab, esc, alt_enter, cmd_enter, ctrl_shift_enter, m.shift)
        });
        if alt_enter {
            let b = ed.text.char_indices().nth(ed.caret).map(|(i, _)| i).unwrap_or(ed.text.len());
            ed.text.insert(b, '\n');
            ed.caret += 1;
            ed.caret_to_end = false;
            ed.sync_caret = true;
        }
        if !ed.autocomplete.is_empty() && tab {
            ed.accept_autocomplete();
        } else if enter {
            action = Some((if shift { -1 } else { 1 }, 0, false, false));
        } else if tab {
            action = Some((0, if shift { -1 } else { 1 }, false, false));
        }
        if cmd_enter {
            action = Some((0, 0, false, true));
        }
        if ctrl_shift_enter {
            action = Some((0, 0, true, false));
        }
        if esc {
            if !ed.autocomplete.is_empty() {
                ed.autocomplete.clear();
            } else if ed.completion.is_some() {
                ed.completion = None;
            } else {
                cancel = true;
            }
        }
        // Arrow keys: autocomplete navigation, point mode, or (Enter mode) commit and move.
        let arrows = ui.input(|i| {
            (
                i.key_pressed(Key::ArrowUp),
                i.key_pressed(Key::ArrowDown),
                i.key_pressed(Key::ArrowLeft),
                i.key_pressed(Key::ArrowRight),
                i.modifiers.shift,
            )
        });
        let (up, down, left, right, ashift) = arrows;
        if (up || down) && !ed.autocomplete.is_empty() {
            ui.input_mut(|i| {
                i.consume_key(egui::Modifiers::NONE, Key::ArrowUp);
                i.consume_key(egui::Modifiers::NONE, Key::ArrowDown);
            });
            let n = ed.autocomplete.len();
            ed.ac_index = if down { (ed.ac_index + 1) % n } else { (ed.ac_index + n - 1) % n };
        } else if ed.enter_mode && (up || down || left || right) && !ed.from_formula_bar {
            let (dr, dc) = if up {
                (-1, 0)
            } else if down {
                (1, 0)
            } else if left {
                (0, -1)
            } else {
                (0, 1)
            };
            // Left/Right follow the screen on a right-to-left sheet.
            let rtl = app.session.active().and_then(|d| d.wb.active()).is_some_and(|s| s.right_to_left);
            let dc = if rtl { -dc } else { dc };
            if ed.can_point() {
                ui.input_mut(|i| {
                    for k in [Key::ArrowUp, Key::ArrowDown, Key::ArrowLeft, Key::ArrowRight] {
                        i.consume_key(egui::Modifiers::NONE, k);
                        i.consume_key(egui::Modifiers::SHIFT, k);
                    }
                });
                crate::grid::point_move(app, &mut ed, dr, dc, ashift);
            } else {
                action = Some((dr, dc, false, false));
                arrow = true;
            }
        }
        // F4 cycles $ anchors of the reference before the caret.
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::F4)) {
            cycle_anchor(&mut ed);
        }
        // F3 opens Paste Name when the workbook has names; OK puts the name at the caret.
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::F3)) {
            app.open_dialog("pasteName", json!({"shortcut": true}));
        }
        // Ctrl+Shift+A (Control+Shift+A on a Mac) inserts the argument names. Consumed anywhere,
        // as in Excel, so it never falls through to the text's select-all.
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, Key::A)) {
            let (text, caret) = (ed.text.clone(), ed.caret);
            if ed.insert_argument_names(app.ui.language)
                && let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), id)
            {
                // The text edit records an undo point only once typing pauses: mark the text
                // before the shortcut so one Undo takes back just the inserted names.
                let mut undoer = state.undoer();
                let cursor = state.cursor.char_range().unwrap_or(egui::text::CCursorRange::one(egui::text::CCursor::new(caret)));
                undoer.add_undo(&(cursor, text));
                state.set_undoer(undoer);
                state.store(ui.ctx(), id);
            }
        }
        // With the formula bar floating, F2 takes the edit from the bar into the cell and back.
        if app.ui.floating_formula_bar && app.session.prefs.formula_bar && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::F2)) {
            ed.from_formula_bar = !ed.from_formula_bar;
            ed.request_focus = true;
            ed.sync_caret = true;
            if ed.from_formula_bar {
                app.floating_bar_open = true;
            }
        }
        // Ctrl+Shift+U (Control+Shift+U on a Mac) expands or collapses the formula bar while editing too.
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, Key::U)) {
            app.run_or_alert("view.expandFormulaBar", json!({}));
        }
    }
    // The text widget.
    let refs = formula_refs(&ed.text);
    // The bracket at the caret (just before or after it) and its partner, and any bracket left
    // without one.
    let (matched, unmatched) = if ed.is_formula() { brackets(&ed.text, ed.caret) } else { (None, Vec::new()) };
    let t = Tokens::get(ui.ctx());
    let on_sheet = id == egui::Id::unique("mkmatrix.cell_editor");
    let cell_text_color = if on_sheet { t.cell_text } else { t.text };
    let font2 = font.clone();
    let mut layouter = move |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap_width: f32| {
        let s = buf.as_str();
        let mut job = egui::text::LayoutJob::default();
        let mut pos = 0usize;
        // (start, end, colour, background): references in their colours, the matched brackets
        // in the accent on a tint, an unmatched one in the danger colour.
        let mut spans: Vec<(usize, usize, egui::Color32, egui::Color32)> =
            refs.iter().filter(|r| r.1 <= s.len()).map(|r| (r.0, r.1, t.ref_color(r.4, on_sheet), egui::Color32::TRANSPARENT)).collect();
        for at in matched.iter().flat_map(|(a, b)| [*a, *b]) {
            spans.push((at, at + 1, t.accent, t.accent.gamma_multiply(0.18)));
        }
        for at in &unmatched {
            spans.push((*at, at + 1, t.danger, egui::Color32::TRANSPARENT));
        }
        spans.sort_by_key(|x| (x.0, x.1));
        for (a, b, color, bg) in spans {
            if a < pos || b > s.len() || !s.is_char_boundary(a) || !s.is_char_boundary(b) {
                continue;
            }
            job.append(s.get(pos..a).unwrap_or(""), 0.0, egui::TextFormat::simple(font2.clone(), cell_text_color));
            let mut format = egui::TextFormat::simple(font2.clone(), color);
            format.background = bg;
            job.append(s.get(a..b).unwrap_or(""), 0.0, format);
            pos = b;
        }
        job.append(s.get(pos..).unwrap_or(""), 0.0, egui::TextFormat::simple(font2.clone(), cell_text_color));
        job.wrap.max_width = if multiline { wrap_width } else { f32::INFINITY };
        ui.fonts_mut(|f| f.layout_job(job))
    };
    let chars_before = ed.text.chars().count();
    // A dialog opened over the edit (Paste Name) takes the keyboard until it closes, and the
    // editor takes it back afterwards.
    let dialog_open = app.dialog.is_some();
    if dialog_open && mine {
        ed.request_focus = true;
        ed.keep_selection = true;
    }
    let synced = mine && !dialog_open && (ed.request_focus || ed.sync_caret);
    let edit = if multiline || ed.text.contains('\n') { TextEdit::multiline(&mut ed.text) } else { TextEdit::singleline(&mut ed.text) };
    let edit = edit.interactive(!dialog_open);
    let caret_index = ed.caret;
    // The in-cell editor grows with its text and never scrolls, so the field keeps its whole width
    // for the text (a scroll area would take the bar's width off it and wrap the text narrower
    // than the cell does). The formula bar shows its lines and scrolls past them.
    let mut output = if on_sheet {
        show_edit(edit, ui, id, font, width, &mut layouter)
    } else {
        egui::ScrollArea::vertical()
            .id_salt(id.with("scroll"))
            .max_height(max_height.max(1.0))
            .auto_shrink([false, true])
            .show(ui, |ui| {
                let output = show_edit(edit, ui, id, font, width, &mut layouter);
                // A caret moved by our own keys (Alt+Enter, F4, point mode) isn't the TextEdit's
                // doing, so it doesn't scroll to it itself.
                if synced {
                    let caret = output.galley.pos_from_cursor(egui::text::CCursor::new(caret_index)).translate(output.galley_pos.to_vec2());
                    ui.scroll_to_rect(caret.expand(2.0), None);
                }
                output
            })
            .inner
    };
    // Both editors are text inputs without a visible label next to them: name the one being shown.
    let accessible_name = (if on_sheet { app.ui.language.tr("Cell Editor") } else { app.ui.language.tr("Formula Bar") }).to_string();
    let resp = output.response.response.clone().accessible_name(accessible_name);
    if synced {
        if ed.request_focus {
            resp.request_focus();
        }
        ed.request_focus = false;
        // Closing a dialog that changed nothing (Cancel) gives the selection back as it was.
        let range = if ed.keep_selection && !ed.sync_caret {
            egui::text::CCursorRange::two(egui::text::CCursor::new(ed.anchor), egui::text::CCursor::new(ed.caret))
        } else {
            ed.anchor = ed.caret;
            egui::text::CCursorRange::one(egui::text::CCursor::new(ed.caret))
        };
        ed.sync_caret = false;
        ed.keep_selection = false;
        output.state.cursor.set_char_range(Some(range));
        output.state.store(ui.ctx(), id);
    } else if resp.clicked() || resp.gained_focus() {
        // Clicking into the other editor moves editing there.
        ed.from_formula_bar = id == egui::Id::unique("mkmatrix.formula_bar");
        ed.enter_mode = false;
        app.session.mode = mkmatrix_engine::Mode::Edit;
    }
    if !synced
        && !dialog_open
        && let Some(r) = output.cursor_range
    {
        ed.caret = r.primary.index.into();
        ed.anchor = r.secondary.index.into();
    }
    if resp.changed() {
        ed.completion = None;
        if app.ui.autocomplete
            && !ed.is_formula()
            && !entry_shrank(chars_before, &ed.text)
            && ed.caret == ed.text.chars().count()
            && let Some(sh) = app.session.active().and_then(|d| d.wb.sheet(ed.sheet))
        {
            ed.completion = crate::editor::column_completion(sh, ed.cell, &ed.text);
        }
        ed.point = None;
        ed.point_anchor = None;
        // Typing ends the pointing: the next arrow starts again from the edited cell, as in Excel.
        ed.point_cell = None;
        let names: Vec<String> = function_names()
            .iter()
            .map(|name| {
                app.session.active().map(|d| crate::formula_locale::completion_name(name, ed.locale, &d.wb, ed.sheet)).unwrap_or(name).to_string()
            })
            .collect();
        ed.update_autocomplete(&names);
        app.session.mode = if ed.can_point() {
            mkmatrix_engine::Mode::Point
        } else if ed.enter_mode {
            mkmatrix_engine::Mode::Enter
        } else {
            mkmatrix_engine::Mode::Edit
        };
    }
    // Column AutoComplete: the rest of the suggested entry, shown selected after the caret.
    if let Some(full) = &ed.completion
        && let Some(rest) = crate::editor::completion_rest(full, &ed.text)
    {
        let pos = resp.rect.left_top() + egui::vec2(output.galley.rect.width(), 0.0);
        let g = ui.painter().layout_no_wrap(
            rest.to_string(),
            output.galley.job.sections.first().map(|s| s.format.font_id.clone()).unwrap_or_default(),
            t.on_accent,
        );
        let r = egui::Rect::from_min_size(pos, g.size());
        ui.painter().rect_filled(r, 0.0, t.accent);
        ui.painter().galley(pos, g, t.on_accent);
    }
    // Autocomplete list and argument hint under the editor. Decide visibility from the area the
    // editor was allotted, not from its response: egui 0.37 gives every text field a
    // `spacing.interact_size.y` floor, so a one-line cell editor's response can reach below a short
    // cell and touch the visible quadrant even while the cell itself is hidden under a frozen row.
    if mine && ui.max_rect().intersect(ui.clip_rect()).is_positive() && (!ed.autocomplete.is_empty() || ed.current_function().is_some()) {
        let below = resp.rect.left_bottom() + vec2(0.0, 4.0);
        let language = app.ui.language;
        // Areas have their own clip rect: keep cell helpers inside the worksheet.
        let bounds = if ed.from_formula_bar { ui.ctx().content_rect() } else { ui.clip_rect() };
        egui::Area::new(id.with("ac")).fixed_pos(below).constrain_to(bounds).order(egui::Order::Tooltip).show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).inner_margin(4.0).show(ui, |ui| {
                if !ed.autocomplete.is_empty() {
                    let mut pick = None;
                    for (i, n) in ed.autocomplete.iter().enumerate() {
                        let sel = i == ed.ac_index;
                        let r = ui.add(
                            egui::Button::selectable(sel, egui::RichText::new(format!("ƒ  {n}")).font(theme::ui_font(12.5)))
                                .min_size(vec2(220.0, 20.0)),
                        );
                        if r.clicked() {
                            pick = Some(i);
                        }
                        if sel && let Some(desc) = crate::formula_locale::description(n, ed.locale, language) {
                            r.on_hover_text(desc);
                        }
                    }
                    if let Some(desc) = ed.autocomplete.get(ed.ac_index).and_then(|n| crate::formula_locale::description(n, ed.locale, language)) {
                        ui.separator();
                        ui.add(egui::Label::new(egui::RichText::new(desc).small()).wrap());
                    }
                    if let Some(i) = pick {
                        ed.ac_index = i;
                        ed.accept_autocomplete();
                        ed.request_focus = true;
                    }
                } else if let Some((name, arg)) = ed.current_function()
                    && let Some(sig) = crate::formula_locale::signature(&name, ed.locale, language)
                {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let (head, rest) = sig.split_once('(').unwrap_or((&sig, ""));
                        ui.label(egui::RichText::new(format!("{head}(")).font(theme::ui_font(12.0)));
                        let inner = rest.trim_end_matches(')');
                        let parts: Vec<&str> = inner.split(&format!("{} ", ed.locale.list_separator())).collect();
                        for (i, part) in parts.iter().enumerate() {
                            let is_cur = i == arg.min(parts.len().saturating_sub(1)) || (part.contains("...") && arg >= i);
                            let txt = egui::RichText::new(*part).font(if is_cur { theme::ui_bold(12.0) } else { theme::ui_font(12.0) });
                            ui.label(txt);
                            if i + 1 < parts.len() {
                                ui.label(egui::RichText::new(format!("{} ", ed.locale.list_separator())).font(theme::ui_font(12.0)));
                            }
                        }
                        ui.label(egui::RichText::new(")").font(theme::ui_font(12.0)));
                    });
                    // The description too, so values an argument takes (DATEDIF's units, #363) show
                    // while it's typed; wrapped to the hint's width.
                    if let Some(desc) = crate::formula_locale::description(&name, ed.locale, language) {
                        ui.set_max_width(ui.min_rect().width().max(220.0));
                        ui.separator();
                        ui.add(egui::Label::new(egui::RichText::new(desc).small()).wrap());
                    }
                }
            });
        });
    }
    let lines = output.galley.rows.len().max(1);
    let caret = Some(output.galley.pos_from_cursor(egui::text::CCursor::new(ed.caret)).translate(output.galley_pos.to_vec2()));
    app.editor = Some(ed);
    if cancel {
        app.cancel_edit();
    } else if let Some((dr, dc, array, fill)) = action {
        if arrow {
            app.commit_edit_arrow(dr, dc);
        } else {
            app.commit_edit(dr, dc, array, fill);
        }
    }
    EditorInfo { lines, caret }
}

fn cycle_anchor(ed: &mut crate::editor::EditState) {
    let caret_b = ed.text.char_indices().nth(ed.caret).map(|(i, _)| i).unwrap_or(ed.text.len());
    for (a, b, ..) in formula_refs(&ed.text) {
        if a <= caret_b && caret_b <= b {
            let Some(r) = ed.text.get(a..b) else { return };
            let cycled: String = r
                .split(':')
                .map(|part| {
                    let p = part.split('!').next_back().unwrap_or(part);
                    let prefix = &part[..part.len() - p.len()];
                    let bare = p.replace('$', "");
                    let letters: String = bare.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
                    let digits = &bare[letters.len()..];
                    let state = (p.starts_with('$'), p[letters.len().min(p.len())..].contains('$') || p.trim_start_matches('$').contains('$'));
                    let next = match state {
                        (false, false) => format!("${letters}${digits}"),
                        (true, true) => format!("{letters}${digits}"),
                        (false, true) => format!("${letters}{digits}"),
                        (true, false) => format!("{letters}{digits}"),
                    };
                    format!("{prefix}{next}")
                })
                .collect::<Vec<_>>()
                .join(":");
            let (Some(x), Some(y)) = (ed.text.get(..a), ed.text.get(b..)) else { return };
            ed.text = format!("{x}{cycled}{y}");
            ed.caret = ed.text.get(..a + cycled.len()).map(|s| s.chars().count()).unwrap_or(ed.caret);
            ed.caret_to_end = false;
            ed.request_focus = true;
            ed.sync_caret = true;
            return;
        }
    }
}

/// The brackets of formula text, outside text in quotes, quoted sheet names and structured
/// references: the pair at char index `caret` (the bracket just before it, else the one after it)
/// as byte offsets, and the byte offsets of closing brackets with no partner (an opening one
/// is only not closed yet). A `'` after an operand is MK Matrix's transpose, not a sheet's quote.
fn brackets(text: &str, caret: usize) -> (Option<(usize, usize)>, Vec<usize>) {
    let mut open: Vec<(usize, char)> = Vec::new();
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    let mut unmatched = Vec::new();
    let (mut quote, mut sheet, mut square) = (false, false, 0usize);
    // The last character outside quotes and spaces: after an operand's end a `'` transposes.
    let mut last = ' ';
    for (b, c) in text.char_indices() {
        let operand_end = last.is_alphanumeric() || matches!(last, ')' | ']' | '}' | '%' | '\'' | '$');
        if !quote && !sheet && !c.is_whitespace() && c != '"' && !(c == '\'' && !operand_end) {
            last = c;
        }
        match c {
            '"' if !sheet && square == 0 => quote = !quote,
            '\'' if !quote && square == 0 && (sheet || !operand_end) => {
                sheet = !sheet;
                if !sheet {
                    last = 'a';
                }
            }
            _ if quote || sheet => {}
            '[' => square += 1,
            ']' => square = square.saturating_sub(1),
            _ if square > 0 => {}
            '(' | '{' => open.push((b, c)),
            ')' | '}' => {
                let want = if c == ')' { '(' } else { '{' };
                match open.last() {
                    Some(&(o, oc)) if oc == want => {
                        open.pop();
                        pairs.push((o, b));
                    }
                    _ => unmatched.push(b),
                }
            }
            _ => {}
        }
    }
    let byte = |i: usize| text.char_indices().nth(i).map(|(b, _)| b);
    let before = caret.checked_sub(1).and_then(byte);
    let after = byte(caret);
    let pair_at = |at: Option<usize>| at.and_then(|at| pairs.iter().copied().find(|(a, b)| *a == at || *b == at));
    (pair_at(before).or_else(|| pair_at(after)), unmatched)
}

fn function_names() -> Vec<String> {
    static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    NAMES
        .get_or_init(|| mkmatrix_engine::cmd::formulas::function_list().iter().filter_map(|f| f["name"].as_str().map(str::to_string)).collect())
        .clone()
}

fn function_info(name: &str) -> Option<serde_json::Value> {
    static LIST: std::sync::OnceLock<Vec<serde_json::Value>> = std::sync::OnceLock::new();
    LIST.get_or_init(mkmatrix_engine::cmd::formulas::function_list)
        .iter()
        .find(|f| f["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(name)))
        .cloned()
}

pub fn function_signature(name: &str) -> Option<String> {
    function_info(name).and_then(|f| f["signature"].as_str().map(str::to_string))
}

pub fn function_description(name: &str) -> Option<String> {
    function_info(name).and_then(|f| f["description"].as_str().map(str::to_string))
}

#[cfg(test)]
mod spill_tests {
    use super::*;
    use mkmatrix_engine::Session;

    fn app() -> SheetApp {
        let mut s = Session::new();
        s.new_workbook();
        SheetApp::new(s, Default::default())
    }

    fn set(app: &mut SheetApp, cell: &str, input: &str) {
        app.run("cell.set", json!({"cell": cell, "input": input})).unwrap();
    }

    fn select(app: &mut SheetApp, cell: &str) {
        app.cancel_edit();
        app.run("selection.set", json!({"cell": cell})).unwrap();
    }

    fn check(app: &mut SheetApp, cell: &str, text: &str, read_only: bool) {
        select(app, cell);
        assert_eq!(active_input(app), text);
        assert_eq!(active_spill_anchor(app).is_some(), read_only);
        app.begin_edit(None, true);
        assert_eq!(app.editor.is_none(), read_only);
        app.cancel_edit();
        if read_only {
            app.begin_edit(Some("=SUM(1,2)".into()), true);
            assert!(app.editor.is_none());
        }
    }

    #[test]
    fn selection_restores_editing_and_keeps_spill_formulas_only_at_anchor() {
        let mut app = app();
        set(&mut app, "A1", "=SEQUENCE(5,1,10,10)");
        set(&mut app, "D1", "=SUM(1,2)");
        set(&mut app, "D2", "hello");
        for cell in ["A2", "A3", "A4", "A5"] {
            check(&mut app, cell, "=SEQUENCE(5,1,10,10)", true);
        }
        check(&mut app, "A1", "=SEQUENCE(5,1,10,10)", false);
        check(&mut app, "D1", "=SUM(1,2)", false);
        check(&mut app, "D2", "hello", false);
        check(&mut app, "E1", "", false);
        let sh = app.session.active().unwrap().wb.active().unwrap();
        assert!(sh.cell(mkmatrix_engine::core::CellRef::parse("A2").unwrap()).and_then(|c| c.formula.as_ref()).is_none());
        // Intentional grid replacement retains the existing entry behavior.
        select(&mut app, "A2");
        app.begin_edit(Some("replacement".into()), false);
        assert!(app.editor.is_some());
    }

    #[test]
    fn spill_ownership_follows_recalculation_and_obstructions() {
        let mut app = app();
        set(&mut app, "D1", "2");
        set(&mut app, "A1", "=SEQUENCE(D1,2)");
        check(&mut app, "B2", "=SEQUENCE(D1,2)", true);
        set(&mut app, "D1", "4");
        check(&mut app, "B4", "=SEQUENCE(D1,2)", true);
        set(&mut app, "D1", "1");
        check(&mut app, "B4", "", false);
        check(&mut app, "B1", "=SEQUENCE(D1,2)", true);
        set(&mut app, "A1", "");
        check(&mut app, "B1", "", false);
        set(&mut app, "B2", "blocked");
        set(&mut app, "A1", "=SEQUENCE(3,2)");
        check(&mut app, "A2", "", false);
        check(&mut app, "B2", "blocked", false);
        set(&mut app, "B2", "");
        check(&mut app, "B2", "=SEQUENCE(3,2)", true);
    }

    #[test]
    fn formatted_children_and_other_dynamic_array_functions() {
        let mut app = app();
        app.run("home.bold", json!({"range": "A2"})).unwrap();
        set(&mut app, "D1", "3");
        set(&mut app, "D2", "1");
        set(&mut app, "D3", "3");
        for formula in ["=SORT(D1:D3)", "=UNIQUE(D1:D3)", "=FILTER(D1:D3,D1:D3>0)"] {
            set(&mut app, "A1", formula);
            check(&mut app, "A2", formula, true);
        }
        app.run("home.insertSheet", json!({"name": "Other"})).unwrap();
        set(&mut app, "A1", "=SORT(Sheet1!D1:D3)");
        check(&mut app, "A2", "=SORT(Sheet1!D1:D3)", true);
        app.run("sheet.activate", json!({"sheet": "Sheet1"})).unwrap();
        check(&mut app, "A2", "=FILTER(D1:D3,D1:D3>0)", true);
    }
    #[test]
    /// A disabled spill formula paints disabled and does not edit; clicking an active formula starts formula-bar editing in both light and dark themes, with accessibility checks.
    fn formula_bar_click_and_theme_presentation() {
        use egui_kittest::kittest::Queryable as _;
        for dark in [false, true] {
            let mut app = app();
            set(&mut app, "A1", "=SEQUENCE(3)");
            select(&mut app, "A2");
            let mut fonts_ready = false;
            let mut h = egui_kittest::Harness::builder().with_size(vec2(900.0, 100.0)).build_ui_state(
                move |ui, app: &mut SheetApp| {
                    if !fonts_ready {
                        ui.ctx().set_fonts(theme::font_definitions());
                        fonts_ready = true;
                        return;
                    }
                    ui.ctx().set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
                    show(app, ui);
                },
                app,
            );
            h.run_steps(3);
            // `run_steps` does not enforce the 0.37 strict policy; run it explicitly so the
            // formula bar's accessible names are actually tested.
            h.check_accessibility();
            let disabled = Tokens::get(&h.ctx).text_disabled;
            assert!(h.output().shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t)
                if t.galley.job.text == "=SEQUENCE(3)" && t.galley.job.sections.iter().all(|section| section.format.color == disabled))));
            h.get_by_label("=SEQUENCE(3)").click();
            h.run_steps(3);
            assert!(h.state().editor.is_none());
            select(h.state_mut(), "A1");
            h.run_steps(3);
            h.get_by_label("=SEQUENCE(3)").click();
            h.run_steps(3);
            h.check_accessibility();
            assert!(h.state().editor.as_ref().is_some_and(|e| e.from_formula_bar));
        }
    }

    #[test]
    /// The Name Box and the formula field are equally tall at one line, and each turns darker under
    /// the pointer and darker again while being typed in.
    fn name_box_and_field_share_height_and_darken_on_hover_and_typing() {
        let has_fill = |h: &egui_kittest::Harness<'_, SheetApp>, rect: Rect, fill: egui::Color32| {
            h.output().shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == rect && r.fill == fill))
        };
        for dark in [false, true] {
            let mut app = app();
            set(&mut app, "A1", "=1+2");
            select(&mut app, "A1");
            let mut fonts_ready = false;
            let mut h = egui_kittest::Harness::builder().with_size(vec2(900.0, 100.0)).build_ui_state(
                move |ui, app: &mut SheetApp| {
                    if !fonts_ready {
                        ui.ctx().set_fonts(theme::font_definitions());
                        fonts_ready = true;
                        return;
                    }
                    ui.ctx().set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
                    show(app, ui);
                },
                app,
            );
            h.run_steps(3);
            let t = Tokens::get(&h.ctx);
            let bar = BarLayout::get(&h.ctx);
            assert_eq!(bar.name_box.height(), bar.field.height(), "dark={dark}");
            assert!(has_fill(&h, bar.name_box, t.window) && has_fill(&h, bar.field, t.window));
            assert!(t.field_hover != t.window && t.field_active != t.field_hover);

            h.hover_at(bar.field.center());
            h.run_steps(2);
            assert!(has_fill(&h, bar.field, t.field_hover), "dark={dark}: field hover");
            h.hover_at(bar.name_box.center());
            h.run_steps(2);
            assert!(has_fill(&h, bar.name_box, t.field_hover), "dark={dark}: name box hover");

            h.state_mut().begin_edit(None, true);
            h.hover_at(egui::pos2(450.0, 95.0));
            h.run_steps(3);
            assert!(has_fill(&h, bar.field, t.field_active), "dark={dark}: typing in the field");

            h.state_mut().editor = None;
            h.ctx.memory_mut(|m| m.request_focus(egui::Id::unique("mkmatrix.name_box")));
            h.run_steps(3);
            assert!(has_fill(&h, bar.name_box, t.field_active), "dark={dark}: typing in the name box");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brackets_pair_up_outside_text_sheet_names_and_table_columns() {
        let f = "=IF(SUM(A1:A3)>0,\"(\",'My (sheet)'!B1)+T[(x)]";
        // The caret after SUM's ")" (char 14) pairs it with SUM's "(".
        let sum_open = f.find("SUM(").unwrap() + 3;
        let sum_close = f.find(")>").unwrap();
        assert_eq!(brackets(f, sum_close + 1).0, Some((sum_open, sum_close)));
        // Before IF's "(" (the caret right after "=IF") it pairs with the last ")".
        let if_close = f.rfind(")+").unwrap();
        assert_eq!(brackets(f, 3).0, Some((3, if_close)));
        assert!(brackets(f, 0).1.is_empty(), "nothing unmatched");
        // A closer too many is flagged; an opener still being typed isn't.
        assert_eq!(brackets("=1+2)", 0).1, vec![4]);
        assert!(brackets("=SUM(1,2", 0).1.is_empty());
        // A' transposes: the next brackets still pair.
        let t = "=(A'*B)'+(1)";
        assert_eq!(brackets(t, t.len()).0, Some((9, 11)));
        assert!(brackets(t, 0).1.is_empty());
    }

    fn press(key: Key) -> egui::RawInput {
        let mut raw = egui::RawInput::default();
        raw.events.push(egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE });
        raw
    }

    #[test]
    fn backspace_and_delete_dismiss_completion() {
        let ctx = egui::Context::default();
        for key in [Key::Backspace, Key::Delete] {
            let mut completion = Some("SomeText".to_string());
            let mut out = ctx.run_ui(press(key), |ui| ui.input_mut(|i| dismiss_completion(i, &mut completion)));
            out.textures_delta.clear(); // nothing consumes the atlas in a headless test
            assert_eq!(completion, None, "{key:?} must drop the proposal");
        }
        let mut completion = Some("SomeText".to_string());
        let mut out = ctx.run_ui(press(Key::A), |ui| ui.input_mut(|i| dismiss_completion(i, &mut completion)));
        out.textures_delta.clear();
        assert!(completion.is_some());
    }

    #[test]
    fn only_typing_proposes_completion() {
        assert!(!entry_shrank(4, "SomeT"));
        // Backspacing "Some" to "Som" must not bring "SomeText" back.
        assert!(entry_shrank(4, "Som"));
        assert!(!entry_shrank(4, "Same"));
    }

    #[test]
    fn one_line_fits_a_row_of_any_font() {
        // macOS and Linux UI fonts: rows of 15 to 16 px keep Excel's 30 px bar.
        assert_eq!(one_line(15.6), 30.0);
        assert_eq!(one_line(16.0), 30.0);
        // The Windows UI font's 17.28 px rows need more: the panel's and the field's margins take
        // 12 px, and rounding to the pixel grid up to another one, so 30 px held only four rows
        // of five (a tester's finding). Every counted line must fit in what is left.
        for row_h in [17.0_f32, 17.28, 18.0, 20.0] {
            for lines in 1..=MAX_LINES {
                let bar = one_line(row_h) + (lines - 1) as f32 * row_h;
                let inner = bar - 13.0;
                assert!((inner / row_h).floor() as usize >= lines, "{lines} lines of {row_h} px in a {bar} px bar");
            }
        }
    }
}
