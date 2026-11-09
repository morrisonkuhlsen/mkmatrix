//! The Terminal task pane: Claude Code, Codex or a shell run beside the workbook, in its folder,
//! with the agents connected to this window (see [`crate::terminal::Agent`]): the window turns its
//! control channel on, and the agent reaches it through MK Matrix's MCP server, so what it does
//! shows in the window at once and can be undone. A program starts only from a click here.
//! Adapted from Pearl's terminal pane (MIT OR Apache-2.0, the same house).

use std::path::PathBuf;

use egui::{Align2, Color32, CursorIcon, Event, FontId, Id, Key, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};

use crate::SheetApp;
use crate::icons::Icon;
use crate::terminal::{Agent, Caret, Drawn, Launch, Palette, Program, Terminal};
use crate::theme::{self, Tokens};

/// The pane's id in `grid.pane`, and its width when it first opens.
pub const PANE: &str = "terminal";
pub const START_WIDTH: f32 = 560.0;
const FONT_SIZE: f32 = 13.0;
const PADDING: f32 = 6.0;

fn grid_id() -> Id {
    Id::unique("mkmatrix-terminal-grid")
}

#[derive(Default)]
pub struct TerminalPane {
    running: Option<Terminal>,
    /// Why the last start failed, or why the agent isn't connected.
    problem: Option<String>,
    /// The control port the program was told of.
    port: Option<u16>,
    /// Wheel movement not yet a whole line.
    scroll_rest: f32,
    /// The pointer went down on the screen and is selecting.
    selecting: bool,
}

fn font() -> FontId {
    FontId::monospace(FONT_SIZE)
}

fn palette(t: &Tokens) -> Palette {
    Palette::new(t.dark, t.text, t.input_bg, t.accent)
}

/// `pattern` translated, with `{name}` replaced by `value`.
fn fill(app: &SheetApp, pattern: &str, name: &str, value: &str) -> String {
    app.ui.language.tr(pattern).replace(&format!("{{{name}}}"), value)
}

impl SheetApp {
    fn terminal_shown(&self) -> bool {
        self.grid.pane.as_deref() == Some(PANE)
    }

    /// Whether the terminal has the keyboard.
    pub fn terminal_focused(&self, ctx: &egui::Context) -> bool {
        self.terminal_shown() && self.terminal.running.is_some() && ctx.memory(|m| m.has_focus(grid_id()))
    }

    /// The text on the terminal's screen, as tests read it.
    pub fn terminal_text(&self, ctx: &egui::Context) -> Option<String> {
        let t = Tokens::get(ctx);
        self.terminal.running.as_ref().map(|term| term.text(&palette(&t)))
    }

    /// The control port the running program was told of.
    pub fn terminal_port(&self) -> Option<u16> {
        self.terminal.port.filter(|_| self.terminal.running.is_some())
    }

    /// Sends text to the program as if typed, for tests.
    pub fn type_in_terminal(&self, text: &str) {
        if let Some(term) = &self.terminal.running {
            term.input(text.as_bytes().to_vec());
        }
    }

    /// Ends the program, and the terminal with it.
    pub fn end_terminal(&mut self) {
        self.terminal.running = None;
        self.terminal.port = None;
    }

    /// Where programs start: the workbook's folder, or the home folder for one with no file.
    fn terminal_folder(&self) -> PathBuf {
        let path = self.session.active().and_then(|d| d.path.clone()).map(PathBuf::from);
        let parent = path.as_deref().and_then(std::path::Path::parent).filter(|p| !p.as_os_str().is_empty()).map(PathBuf::from);
        let home = || std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
        let folder = parent.or_else(home).unwrap_or_else(|| PathBuf::from("."));
        std::path::absolute(&folder).unwrap_or(folder)
    }

    /// The control port agents reach this window on: the one it was started with, else one the
    /// host opens now on a free port (its token in a file only this user can read).
    fn agent_port(&mut self, ctx: &egui::Context) -> Result<u16, String> {
        if let Some(port) = self.control_port {
            return Ok(port);
        }
        let start = self.services.start_control.as_ref().ok_or_else(|| "this build has no control channel".to_string())?;
        let (port, rx) = start(ctx)?;
        self.control_rx = Some(rx);
        self.control_port = Some(port);
        Ok(port)
    }

    /// Starts `program` in the workbook's folder, ending what ran before; an agent starts
    /// connected to this window. Only a click calls this.
    pub fn start_terminal(&mut self, ctx: &egui::Context, program: Program) {
        self.terminal.problem = None;
        self.end_terminal();
        let folder = self.terminal_folder();
        let missing = fill(self, "{program} isn't installed, or isn't on your PATH.", "program", program.name());
        let port = match self.agent_port(ctx) {
            Ok(port) => Some(port),
            Err(e) => {
                self.terminal.problem = Some(fill(self, "Can't connect the agent to this window: {error}", "error", &e));
                None
            }
        };
        let exe = std::env::current_exe().ok();
        let workbook = self.session.active().and_then(|d| d.path.clone()).map(PathBuf::from);
        let agent = match (&exe, port) {
            (Some(exe), Some(port)) => Some(Agent { exe, port, workbook: workbook.as_deref() }),
            _ => None,
        };
        let (cw, ch) = cell_size(ctx);
        let cell = (cw.round() as u16, ch.round() as u16);
        let launch = Launch { program, command: program.command(), folder: &folder, agent, missing: &missing };
        match Terminal::start(ctx, &launch, (80, 24), cell) {
            Ok(term) => {
                self.terminal.running = Some(term);
                self.terminal.port = agent.map(|a| a.port);
                self.grid.pane = Some(PANE.to_string());
                ctx.memory_mut(|m| m.request_focus(grid_id()));
            }
            Err(e) => self.terminal.problem = Some(fill(self, "Can't start the terminal: {error}", "error", &e.to_string())),
        }
    }

    /// Takes the keyboard's events for the terminal, before anything else in the window sees them.
    pub(crate) fn terminal_keys(&mut self, ctx: &egui::Context) {
        let t = Tokens::get(ctx);
        if let Some(term) = &mut self.terminal.running {
            term.poll(ctx, &palette(&t));
        }
        let focused = self.terminal_focused(ctx);
        let Some(term) = &mut self.terminal.running else { return };
        term.set_focused(focused);
        if !focused || term.ended.is_some() {
            return;
        }
        let mode = term.mode();
        let events = ctx.input_mut(|i| {
            let (mine, rest): (Vec<Event>, Vec<Event>) = std::mem::take(&mut i.events).into_iter().partition(|e| {
                matches!(e, Event::Key { .. } | Event::Text(_) | Event::Copy | Event::Cut | Event::Paste(_) | Event::PasteImage(_) | Event::Ime(_))
            });
            i.events = rest;
            mine
        });
        let mac = cfg!(target_os = "macos");
        let alt_held = ctx.input(|i| i.modifiers.alt && !i.modifiers.ctrl);
        for event in events {
            match event {
                Event::Key { key, pressed: true, modifiers, .. } => {
                    // Copying is Ctrl+Shift+C, as in other terminals: Ctrl+C belongs to the program.
                    if !mac && key == Key::C && modifiers.ctrl && modifiers.shift {
                        if let Some(text) = term.selected_text() {
                            ctx.copy_text(text);
                        }
                        continue;
                    }
                    if let Some(bytes) = crate::terminal::key_bytes(key, modifiers, mode) {
                        term.input(bytes);
                    }
                }
                // Alt with a letter came as a key already; on macOS, Option makes characters.
                Event::Text(text) if !(alt_held && !mac) => term.input(text.into_bytes()),
                Event::Ime(egui::ImeEvent::Commit(text)) => term.input(text.into_bytes()),
                Event::Copy => match term.selected_text() {
                    Some(text) => {
                        ctx.copy_text(text);
                        term.clear_selection();
                    }
                    None if !mac => term.input(vec![0x03]),
                    None => {}
                },
                Event::Cut if !mac => term.input(vec![0x18]),
                Event::Paste(text) => term.paste(&text),
                // An image is the program's to read from the clipboard, as Ctrl+V asks it to.
                Event::PasteImage(_) => term.input(vec![0x16]),
                _ => {}
            }
        }
    }
}

/// The pane's body, under the task pane's title: what runs and its screen, or what can be started.
pub fn show(app: &mut SheetApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    if app.terminal.running.is_some() {
        header(app, ui, &t);
        let rect = ui.available_rect_before_wrap();
        ui.allocate_rect(rect, Sense::hover());
        screen(app, ui, rect, &t);
    } else {
        start(app, ui, &t);
    }
}

/// What runs, whether it is connected to this window, and the button that ends it.
fn header(app: &mut SheetApp, ui: &mut Ui, t: &Tokens) {
    let Some(term) = &app.terminal.running else { return };
    let title = term.title.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| app.ui.language.tr(term.program.name()).to_string());
    let connected = app.terminal.port.map(|port| fill(app, "Connected to this window on port {port}", "port", &port.to_string()));
    let end = app.ui.language.tr("End the program").to_string();
    let mut ended = false;
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if crate::widgets::icon_button(ui, Icon::Delete, t.text_dim, &end, vec2(22.0, 22.0)).clicked() {
                ended = true;
            }
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                crate::icons::paint(ui.painter(), r, Icon::Terminal, t.text);
                ui.add(egui::Label::new(egui::RichText::new(title).font(theme::ui_bold(13.0)).color(t.text)).truncate());
            });
        });
    });
    if let Some(connected) = connected {
        ui.label(egui::RichText::new(connected).font(theme::ui_font(11.5)).color(t.text_dim));
    }
    if let Some(problem) = &app.terminal.problem {
        ui.colored_label(t.danger, problem);
    }
    ui.add_space(4.0);
    if ended {
        app.end_terminal();
    }
}

/// With nothing running: what can be started, and where.
fn start(app: &mut SheetApp, ui: &mut Ui, t: &Tokens) {
    let ctx = ui.ctx().clone();
    let folder = app.terminal_folder();
    let lang = app.ui.language;
    ui.add_space(6.0);
    ui.label(egui::RichText::new(lang.tr("Run an agent beside the workbook")).font(theme::ui_bold(13.0)).color(t.text));
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(
            lang.tr("Claude Code and Codex start connected to this window: they read and change the open workbook through MK Matrix's tools, and every change can be undone."),
        )
        .font(theme::ui_font(12.5))
        .color(t.text_dim),
    );
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(fill(app, "Starts in {folder}", "folder", &folder.display().to_string())).font(theme::ui_font(12.0)).color(t.text_dim),
    );
    ui.add_space(10.0);
    for program in [Program::Claude, Program::Codex, Program::Shell] {
        let name = lang.tr(program.name());
        let button = egui::Button::new(egui::RichText::new(name).font(theme::ui_font(13.0))).min_size(vec2(170.0, 28.0));
        let tip = fill(app, "Start {program} in the workbook's folder", "program", name);
        if ui.add(button).on_hover_text(tip).clicked() {
            app.start_terminal(&ctx, program);
        }
        ui.add_space(4.0);
    }
    ui.add_space(10.0);
    ui.label(
        egui::RichText::new(lang.tr("The programs are your own: MK Matrix doesn't install them or see what they send."))
            .font(theme::ui_font(12.0))
            .color(t.text_dim),
    );
    if let Some(problem) = &app.terminal.problem {
        ui.add_space(8.0);
        ui.colored_label(t.danger, problem);
    }
}

fn screen(app: &mut SheetApp, ui: &mut Ui, rect: Rect, t: &Tokens) {
    let ctx = ui.ctx().clone();
    let pal = palette(t);
    ui.painter().rect_filled(rect, 0.0, pal.background());
    let (cw, ch) = cell_size(&ctx);
    let grid = rect.shrink(PADDING);
    let columns = ((grid.width() / cw).floor() as usize).max(2);
    let lines = ((grid.height() / ch).floor() as usize).max(1);
    let lang = app.ui.language;
    let ended = app.terminal.running.as_ref().and_then(|term| term.ended);
    let Some(term) = &mut app.terminal.running else { return };
    term.resize(columns, lines, (cw.round() as u16, ch.round() as u16));

    let id = grid_id();
    let resp = ui.interact(rect, id, Sense::click_and_drag());
    if resp.clicked() || resp.drag_started() {
        resp.request_focus();
    }
    if resp.has_focus() {
        ui.memory_mut(|m| m.set_focus_lock_filter(id, egui::EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true }));
    }
    let cell_at = |p: Pos2| -> ((usize, usize), bool) {
        let x = ((p.x - grid.left()) / cw).max(0.0);
        let y = ((p.y - grid.top()) / ch).max(0.0);
        let column = (x.floor() as usize).min(columns - 1);
        ((column, (y.floor() as usize).min(lines - 1)), x.fract() >= 0.5)
    };

    // The pointer: selecting, opening links with Ctrl, and the wheel.
    let hovered_link = resp.hover_pos().and_then(|p| term.link_at(cell_at(p).0));
    let command = ui.input(|i| i.modifiers.command);
    if let Some(link) = &hovered_link {
        if command {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
        }
        resp.clone().on_hover_text(lang.tr("Ctrl+click to open {url}").replace("{url}", link));
    } else if resp.hovered() {
        ctx.set_cursor_icon(CursorIcon::Text);
    }
    if resp.clicked()
        && command
        && let Some(link) = hovered_link.clone()
    {
        ctx.open_url(egui::OpenUrl::new_tab(link));
    } else if let Some(p) = resp.interact_pointer_pos() {
        let (at, right) = cell_at(p);
        if resp.drag_started() {
            term.select_from(at, right, 1);
            app.terminal.selecting = true;
        } else if resp.dragged() && app.terminal.selecting {
            term.select_to(at, right);
        } else if resp.triple_clicked() {
            term.select_from(at, right, 3);
        } else if resp.double_clicked() {
            term.select_from(at, right, 2);
        } else if resp.clicked() {
            term.clear_selection();
        }
    }
    if resp.drag_stopped() {
        app.terminal.selecting = false;
    }
    if resp.hovered() {
        // In lines; positive moves the content down, showing what's above.
        let delta = ui.input(|i| {
            i.events
                .iter()
                .map(|e| match e {
                    Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta, .. } => delta.y / ch,
                    Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta, .. } => delta.y,
                    Event::MouseWheel { unit: egui::MouseWheelUnit::Page, delta, .. } => delta.y * lines as f32,
                    _ => 0.0,
                })
                .sum::<f32>()
        });
        if delta != 0.0 {
            app.terminal.scroll_rest += delta;
            let whole = app.terminal.scroll_rest.trunc();
            app.terminal.scroll_rest -= whole;
            let at = resp.hover_pos().map_or((0, 0), |p| cell_at(p).0);
            term.scroll(whole as i32, at);
        }
    }

    let screen = term.snapshot(&pal);
    let painter = ui.painter_at(rect);
    let origin = grid.min;
    let place = |column: usize, line: usize| pos2(origin.x + column as f32 * cw, origin.y + line as f32 * ch);
    for cell in &screen.cells {
        paint_cell(&painter, cell, place(cell.column, cell.line), (cw, ch), hovered_link.is_some());
    }
    if let Some((column, line, shape)) = screen.caret {
        let at = place(column, line);
        let r = Rect::from_min_size(at, vec2(cw, ch));
        match shape {
            Caret::Block => {
                painter.rect_filled(r, 0.0, t.accent);
                if let Some(cell) = screen.cells.iter().find(|c| c.line == line && c.column == column && !c.text.is_empty())
                    && ctx.fonts_mut(|f| cell.text.chars().all(|c| f.has_glyph(&font(), c)))
                {
                    painter.text(at, Align2::LEFT_TOP, &cell.text, font(), pal.background());
                }
            }
            Caret::Hollow => {
                painter.rect_stroke(r.shrink(0.5), 0.0, Stroke::new(1.0, t.accent), egui::StrokeKind::Inside);
            }
            Caret::Beam => {
                painter.rect_filled(Rect::from_min_size(at, vec2(2.0, ch)), 0.0, t.accent);
            }
            Caret::Underline => {
                painter.rect_filled(Rect::from_min_size(pos2(at.x, at.y + ch - 2.0), vec2(cw, 2.0)), 0.0, t.accent);
            }
        }
    }
    if screen.scrolled > 0 {
        let note = lang.tr("{lines} lines back").replace("{lines}", &screen.scrolled.to_string());
        painter.text(pos2(rect.right() - 10.0, rect.top() + 6.0), Align2::RIGHT_TOP, note, theme::ui_font(11.5), t.text_dim);
    }
    if let Some(code) = ended {
        let note = match code {
            Some(code) if code != 0 => lang.tr("The program ended with code {code}").replace("{code}", &code.to_string()),
            _ => lang.tr("The program ended").to_string(),
        };
        let bar = Rect::from_min_max(pos2(rect.left(), rect.bottom() - 26.0), rect.max);
        painter.rect_filled(bar, 0.0, t.notice_info_bg);
        painter.text(pos2(bar.left() + 10.0, bar.center().y), Align2::LEFT_CENTER, note, theme::ui_font(12.5), t.text);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, lang.tr("Terminal")));
}

/// A cell's width and height in the terminal's font.
fn cell_size(ctx: &egui::Context) -> (f32, f32) {
    let f = font();
    let (w, h) = ctx.fonts_mut(|fonts| (fonts.glyph_width(&f, 'M'), fonts.row_height(&f)));
    (w.max(4.0), h.max(8.0))
}

fn paint_cell(painter: &egui::Painter, cell: &Drawn, at: Pos2, (cw, ch): (f32, f32), link_hovered: bool) {
    let width = if cell.wide { cw * 2.0 } else { cw };
    let r = Rect::from_min_size(at, vec2(width, ch));
    if let Some(bg) = cell.bg {
        painter.rect_filled(r, 0.0, bg);
    }
    if let Some(c) = cell.text.chars().next()
        && cell.text.chars().count() == 1
        && box_drawing(painter, c, r, cell.fg)
    {
        return;
    }
    // A program writes any character; one the fonts can't draw is a hollow box, as terminals show it.
    if !cell.text.is_empty() && !painter.ctx().fonts_mut(|f| cell.text.chars().all(|c| f.has_glyph(&font(), c))) {
        painter.rect_stroke(r.shrink2(vec2(1.5, 2.5)), 0.0, Stroke::new(1.0, cell.fg), egui::StrokeKind::Inside);
        return;
    }
    if !cell.text.is_empty() {
        painter.text(at, Align2::LEFT_TOP, &cell.text, font(), cell.fg);
        if cell.bold {
            // The monospace font has no bold: the same letters, a hair to the right.
            painter.text(at + vec2(0.6, 0.0), Align2::LEFT_TOP, &cell.text, font(), cell.fg);
        }
    }
    if cell.underline || (cell.link && link_hovered) {
        painter.hline(r.x_range(), r.bottom() - 1.5, Stroke::new(1.0, cell.fg));
    }
    if cell.strike {
        painter.hline(r.x_range(), r.center().y, Stroke::new(1.0, cell.fg));
    }
}

/// Lines and blocks drawn to the cell's edges, so boxes join up whatever the font has.
fn box_drawing(painter: &egui::Painter, c: char, r: Rect, colour: Color32) -> bool {
    let (l, rt, t, b) = (r.left(), r.right(), r.top(), r.bottom());
    let (x, y) = (r.center().x.round() + 0.5, r.center().y.round() + 0.5);
    let thin = Stroke::new(1.0, colour);
    let heavy = Stroke::new(2.0, colour);
    // Which arms each line has: left, right, up, down.
    let arms = match c {
        '─' | '╌' | '┄' => Some((true, true, false, false, thin)),
        '━' => Some((true, true, false, false, heavy)),
        '│' | '╎' | '┆' => Some((false, false, true, true, thin)),
        '┃' => Some((false, false, true, true, heavy)),
        '┌' => Some((false, true, false, true, thin)),
        '┐' => Some((true, false, false, true, thin)),
        '└' => Some((false, true, true, false, thin)),
        '┘' => Some((true, false, true, false, thin)),
        '├' => Some((false, true, true, true, thin)),
        '┤' => Some((true, false, true, true, thin)),
        '┬' => Some((true, true, false, true, thin)),
        '┴' => Some((true, true, true, false, thin)),
        '┼' => Some((true, true, true, true, thin)),
        _ => None,
    };
    if let Some((left, right, up, down, stroke)) = arms {
        if left {
            painter.hline(l..=x, y, stroke);
        }
        if right {
            painter.hline(x..=rt, y, stroke);
        }
        if up {
            painter.vline(x, t..=y, stroke);
        }
        if down {
            painter.vline(x, y..=b, stroke);
        }
        return true;
    }
    // Rounded corners: an arc from one edge's middle to the other's.
    let corner = match c {
        '╭' => Some((pos2(rt, y), pos2(x, b))),
        '╮' => Some((pos2(l, y), pos2(x, b))),
        '╯' => Some((pos2(l, y), pos2(x, t))),
        '╰' => Some((pos2(rt, y), pos2(x, t))),
        _ => None,
    };
    if let Some((from, to)) = corner {
        let control = pos2(to.x, from.y);
        let points: Vec<Pos2> = (0..=8)
            .map(|i| {
                let s = i as f32 / 8.0;
                let a = from.lerp(control, s);
                let b = control.lerp(to, s);
                a.lerp(b, s)
            })
            .collect();
        painter.add(egui::Shape::line(points, thin));
        return true;
    }
    let block = match c {
        '█' => Some(r),
        '▀' => Some(Rect::from_min_max(r.min, pos2(rt, r.center().y))),
        '▄' => Some(Rect::from_min_max(pos2(l, r.center().y), r.max)),
        '▌' => Some(Rect::from_min_max(r.min, pos2(r.center().x, b))),
        '▐' => Some(Rect::from_min_max(pos2(r.center().x, t), r.max)),
        _ => None,
    };
    if let Some(block) = block {
        painter.rect_filled(block, 0.0, colour);
        return true;
    }
    false
}
