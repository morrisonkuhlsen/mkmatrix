use super::*;

/// Paper sizes the Paper setting offers: name and size in inches (portrait).
const PAPERS: [(&str, f32, f32); 7] = [
    ("Letter", 8.5, 11.0),
    ("Legal", 8.5, 14.0),
    ("A4", 8.27, 11.69),
    ("A3", 11.69, 16.54),
    ("A5", 5.83, 8.27),
    ("Executive", 7.25, 10.5),
    ("Tabloid", 11.0, 17.0),
];
/// The margin presets: `pageLayout.margins` preset, label and margins in inches (left, right,
/// top, bottom).
const MARGINS: [(&str, &str, [f32; 4]); 3] = [
    ("normal", "Normal Margins", [0.7, 0.7, 0.75, 0.75]),
    ("wide", "Wide Margins", [1.0, 1.0, 1.0, 1.0]),
    ("narrow", "Narrow Margins", [0.25, 0.25, 0.75, 0.75]),
];
/// The settings column's width.
const SETTINGS_W: f32 = 320.0;

/// File › Print, laid out as Excel's: Print and the printer on the left over the settings (what
/// to print, orientation, paper, margins, scaling), each a card showing its choice that opens the
/// others, and the preview filling the rest with the page navigation under it. The settings act
/// on the active sheet's Page Setup through `pageLayout.*`, so they are undoable and saved with
/// the workbook; the preview is `file.printPreview`, cached per document revision and options.
pub(super) fn print(app: &mut SheetApp, ui: &mut Ui, bs: &mut Backstage, out: &mut Outcome) {
    let t = Tokens::get(ui.ctx());
    let lang = app.ui.language;
    let Some((uid, revision)) = app.session.active().map(|d| (d.uid, d.revision)) else {
        dim(ui, lang.tr("No workbook is open."));
        return;
    };
    let mut params = json!({"sheets": if bs.print_all { "all" } else { "active" }});
    if bs.fit_to_page {
        params["fitToPage"] = json!(true);
    }
    let key = (uid, revision, bs.print_all, bs.fit_to_page);
    if bs.preview.as_ref().is_none_or(|(k, _)| *k != key) {
        let preview = app.session.run("file.printPreview", params.clone()).unwrap_or(Json::Null);
        bs.preview = Some((key, preview));
    }
    let pages: Vec<Json> = bs.preview.as_ref().and_then(|(_, p)| p.get("pageList")).and_then(Json::as_array).cloned().unwrap_or_default();
    let print = app.session.active().and_then(|d| d.wb.active().map(|s| s.print.clone()));
    // The preview is as tall as the window allows.
    let body_h = (ui.clip_rect().bottom() - ui.cursor().top() - 24.0).max(420.0);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(SETTINGS_W);
            ui.spacing_mut().item_spacing.y = 6.0;
            let can_print = app.services.print_native.is_none() || app.printing.ready();
            if ui.add_enabled_ui(can_print, |ui| print_button(ui, lang.tr("Print"), &t)).inner.clicked() && app.commit_edit(0, 0, false, false) {
                app.print_now(params.clone());
                out.close = app.services.print_native.is_none();
            }
            if let Some(discover) = &app.services.printers {
                ui.add_space(10.0);
                heading(ui, lang.tr("Printer"), &t);
                crate::printing::show(&mut app.printing, ui, discover, |label| lang.tr(label).to_owned());
            }
            ui.add_space(10.0);
            heading(ui, lang.tr("Settings"), &t);
            if let Some(pr) = &print {
                settings(app, ui, bs, pr, lang, &t);
            }
            ui.add_space(4.0);
            if ui.link(lang.tr("Page Setup…")).clicked() {
                app.open_dialog("pageSetup", json!({}));
            }
        });
        ui.add_space(28.0);
        ui.vertical(|ui| preview(app, ui, bs, &pages, body_h, &t));
    });
}

/// The Print button: wide, in the accent, the printer icon beside its name.
fn print_button(ui: &mut Ui, label: &str, t: &Tokens) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 52.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
    let p = ui.painter();
    let fill = if !ui.is_enabled() {
        t.text_disabled
    } else if resp.hovered() {
        t.accent.gamma_multiply(0.9)
    } else {
        t.accent
    };
    p.rect_filled(r, 4.0, fill);
    icons::paint(p, Rect::from_center_size(pos2(r.left() + 30.0, r.center().y), vec2(22.0, 22.0)), Icon::Print, t.on_accent);
    p.text(pos2(r.left() + 54.0, r.center().y), Align2::LEFT_CENTER, label, theme::ui_bold(15.0), t.on_accent);
    resp
}

/// A section's heading in the settings column: small, muted.
fn heading(ui: &mut Ui, text: &str, t: &Tokens) {
    ui.label(RichText::new(text).font(theme::ui_bold(12.0)).color(t.text_dim));
}

/// The settings cards: what to print, orientation, paper, margins and scaling.
fn settings(app: &mut SheetApp, ui: &mut Ui, bs: &mut Backstage, pr: &mkmatrix_engine::model::PrintSettings, lang: Language, t: &Tokens) {
    // What to print.
    let what = [
        (Icon::Sheet, lang.tr("Print Active Sheets").to_string(), lang.tr("Only print the active sheets").to_string()),
        (Icon::Book, lang.tr("Print Entire Workbook").to_string(), lang.tr("Print every sheet in the workbook").to_string()),
    ];
    if let Some(i) = setting(ui, &what, usize::from(bs.print_all), lang.tr("Print What"), t) {
        bs.print_all = i == 1;
        bs.preview_page = 0;
    }

    // Orientation.
    let landscape = pr.orientation == mkmatrix_engine::model::Orientation::Landscape;
    let orient = [
        (Icon::Portrait, lang.tr("Portrait Orientation").to_string(), String::new()),
        (Icon::Landscape, lang.tr("Landscape Orientation").to_string(), String::new()),
    ];
    if let Some(i) = setting(ui, &orient, usize::from(landscape), lang.tr("Orientation"), t)
        && (i == 1) != landscape
    {
        app.run_or_alert("pageLayout.orientation", json!({"orientation": if i == 1 { "landscape" } else { "portrait" }}));
    }

    // Paper.
    let mut papers: Vec<(Icon, String, String)> =
        PAPERS.iter().map(|(name, w, h)| (Icon::Paper, lang.tr(name).to_string(), size_text(*w, *h, lang))).collect();
    let current = PAPERS.iter().position(|(name, ..)| name.eq_ignore_ascii_case(&pr.paper)).unwrap_or_else(|| {
        // A size the list doesn't offer (from a file): shown as the current choice, by name.
        papers.push((Icon::Paper, pr.paper.clone(), String::new()));
        papers.len() - 1
    });
    if let Some(i) = setting(ui, &papers, current, lang.tr("Paper Size"), t)
        && i != current
        && let Some((name, ..)) = PAPERS.get(i)
    {
        app.run_or_alert("pageLayout.size", json!({"paper": name}));
    }

    // Margins.
    let m = [pr.margins[0], pr.margins[1], pr.margins[2], pr.margins[3]];
    let margins_text = |m: [f32; 4]| {
        lang.tr("Left: {left}  Right: {right}").replace("{left}", &length_text(m[0], lang)).replace("{right}", &length_text(m[1], lang))
    };
    let mut margins: Vec<(Icon, String, String)> =
        MARGINS.iter().map(|(_, label, m)| (Icon::Margins, lang.tr(label).to_string(), margins_text(*m))).collect();
    let current = MARGINS.iter().position(|(.., p)| p.iter().zip(m).all(|(a, b)| (a - b).abs() < 0.01)).unwrap_or_else(|| {
        margins.push((Icon::Margins, lang.tr("Custom Margins").to_string(), margins_text(m)));
        margins.len() - 1
    });
    if let Some(i) = setting(ui, &margins, current, lang.tr("Margins"), t)
        && i != current
        && let Some((preset, ..)) = MARGINS.get(i)
    {
        app.run_or_alert("pageLayout.margins", json!({"preset": preset}));
    }

    // Scaling.
    let mut scaling: Vec<(Icon, String, String)> = vec![
        (Icon::Zoom, lang.tr("No Scaling").to_string(), lang.tr("Print sheets at their actual size").to_string()),
        (Icon::Zoom, lang.tr("Fit Sheet on One Page").to_string(), lang.tr("Shrink the printout so that it fits on one page").to_string()),
        (Icon::Zoom, lang.tr("Fit All Columns on One Page").to_string(), lang.tr("Shrink the printout so that it is one page wide").to_string()),
        (Icon::Zoom, lang.tr("Fit All Rows on One Page").to_string(), lang.tr("Shrink the printout so that it is one page high").to_string()),
    ];
    let current = match pr.fit_to {
        Some((1, 1)) => 1,
        Some((1, 0)) => 2,
        Some((0, 1)) => 3,
        None if pr.scale == 100 => 0,
        _ => {
            let text = match pr.fit_to {
                Some((w, h)) => lang.tr("Fit to {wide} × {tall} pages").replace("{wide}", &w.to_string()).replace("{tall}", &h.to_string()),
                None => format!("{}%", pr.scale),
            };
            scaling.push((Icon::Zoom, lang.tr("Custom Scaling").to_string(), text));
            4
        }
    };
    if let Some(i) = setting(ui, &scaling, current, lang.tr("Scaling"), t)
        && i != current
    {
        let p = match i {
            1 => json!({"width": 1, "height": 1}),
            2 => json!({"width": 1, "height": 0}),
            3 => json!({"width": 0, "height": 1}),
            _ => json!({"scale": 100}),
        };
        app.run_or_alert("pageLayout.scaleToFit", p);
        bs.preview_page = 0;
    }
}

/// One settings card: the current choice's icon, name and description, and a chevron; a click
/// opens the list of `options` (icon, name, description) under it. Returns the one picked.
fn setting(ui: &mut Ui, options: &[(Icon, String, String)], current: usize, name: &str, t: &Tokens) -> Option<usize> {
    let (icon, title, sub) = options.get(current).or(options.first())?.clone();
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, 52.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, format!("{name}: {title}")));
    card(ui.painter(), r, icon, &title, &sub, resp.hovered(), false, t);
    icons::paint(ui.painter(), Rect::from_center_size(pos2(r.right() - 18.0, r.center().y), vec2(12.0, 12.0)), Icon::Chevron, t.text_dim);
    let mut picked = None;
    egui::Popup::menu(&resp).width(w).show(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (i, (icon, title, sub)) in options.iter().enumerate() {
            let (r, resp) = ui.allocate_exact_size(vec2(w - 12.0, 52.0), Sense::click());
            resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, i == current, title));
            card(ui.painter(), r, *icon, title, sub, resp.hovered(), i == current, t);
            if resp.clicked() {
                picked = Some(i);
                ui.close();
            }
        }
    });
    picked
}

/// Paints a settings card or one of its choices: icon, name and (muted) description.
#[allow(clippy::too_many_arguments)]
fn card(p: &egui::Painter, r: Rect, icon: Icon, title: &str, sub: &str, hovered: bool, selected: bool, t: &Tokens) {
    let fill = if selected {
        t.accent_soft
    } else if hovered {
        t.hover
    } else {
        t.input_bg
    };
    p.rect_filled(r, 4.0, fill);
    p.rect_stroke(r, 4.0, Stroke::new(1.0, if selected { t.accent } else { t.input_border }), StrokeKind::Inside);
    let ink = if selected { t.accent } else { t.text };
    icons::paint(p, Rect::from_center_size(pos2(r.left() + 24.0, r.center().y), vec2(20.0, 20.0)), icon, ink);
    let clip = Rect::from_min_max(r.min, pos2(r.right() - 32.0, r.bottom()));
    let x = r.left() + 48.0;
    if sub.is_empty() {
        p.with_clip_rect(clip).text(pos2(x, r.center().y), Align2::LEFT_CENTER, title, theme::ui_font(13.5), ink);
    } else {
        p.with_clip_rect(clip).text(pos2(x, r.center().y - 9.0), Align2::LEFT_CENTER, title, theme::ui_font(13.5), ink);
        p.with_clip_rect(clip).text(pos2(x, r.center().y + 10.0), Align2::LEFT_CENTER, sub, theme::ui_font(11.5), t.text_dim);
    }
}

/// Whether the interface language writes decimals with a comma.
fn decimal_comma(lang: Language) -> bool {
    !matches!(lang, Language::En | Language::Zh | Language::Ja | Language::Ko)
}

/// A length given in inches, as the interface language measures paper: inches in English,
/// centimetres elsewhere.
fn length_text(inches: f32, lang: Language) -> String {
    let (value, unit) = if lang == Language::En { (inches, "\"") } else { (inches * 2.54, " cm") };
    let mut text = format!("{:.2}", value);
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    if decimal_comma(lang) {
        text = text.replace('.', ",");
    }
    format!("{text}{unit}")
}

/// A paper size: width × height.
fn size_text(w: f32, h: f32, lang: Language) -> String {
    format!("{} × {}", length_text(w, lang), length_text(h, lang))
}

/// The preview: the page, as large as `height` allows, on a soft shadow; under it the sheet and
/// range, and the page navigation.
fn preview(app: &SheetApp, ui: &mut Ui, bs: &mut Backstage, pages: &[Json], height: f32, t: &Tokens) {
    let lang = app.ui.language;
    let avail_w = ui.available_width();
    if pages.is_empty() {
        let (r, _) = ui.allocate_exact_size(vec2(avail_w, height - 48.0), Sense::hover());
        ui.painter().text(r.center(), Align2::CENTER_CENTER, lang.tr("Nothing to print."), theme::ui_font(14.0), t.text_dim);
        return;
    }
    let last = pages.len().saturating_sub(1);
    bs.preview_page = bs.preview_page.min(last);
    let Some(page) = pages.get(bs.preview_page) else { return };
    // Paper proportions from the preview (points); hostile sizes fall back to Letter.
    let w = page.get("width").and_then(Json::as_f64).filter(|w| w.is_finite() && *w > 1.0).unwrap_or(612.0) as f32;
    let h = page.get("height").and_then(Json::as_f64).filter(|h| h.is_finite() && *h > 1.0).unwrap_or(792.0) as f32;
    let area_h = height - 48.0;
    let scale = (area_h / h).min(avail_w / w).max(0.05);
    let (area, _) = ui.allocate_exact_size(vec2(avail_w, area_h), Sense::hover());
    use egui::emath::GuiRounding as _;
    let rect = Rect::from_center_size(area.center(), vec2(w * scale, h * scale)).round_to_pixels(ui.pixels_per_point());
    let p = ui.painter();
    for (grow, alpha) in [(6.0, 0.05), (3.0, 0.08), (1.0, 0.12)] {
        p.rect_filled(rect.expand(grow).translate(vec2(0.0, 2.0)), 2.0 + grow, Color32::from_black_alpha((alpha * 255.0) as u8));
    }
    p.rect_filled(rect, 1.0, t.grid_bg);
    if let Some(d) = app.session.active() {
        paint_page(p, rect, w, page, &d.wb, t);
    }
    // Navigation, centred under the page: ‹ Page 1 of 2 › and the sheet and range.
    ui.add_space(10.0);
    let sheet = page.get("sheet").and_then(Json::as_str).unwrap_or("");
    let range = page.get("range").and_then(Json::as_str).unwrap_or("");
    let label = lang.tr("Page {page} of {pages}").replace("{page}", &(bs.preview_page + 1).to_string()).replace("{pages}", &pages.len().to_string());
    let label_w = ui.painter().layout_no_wrap(label.clone(), theme::ui_font(13.0), t.text).size().x;
    let row_w = label_w + 2.0 * 28.0 + 2.0 * 10.0;
    ui.horizontal(|ui| {
        ui.add_space(((avail_w - row_w) / 2.0).max(0.0));
        ui.spacing_mut().item_spacing.x = 10.0;
        let ink = |on: bool| if on { t.text } else { t.text_disabled };
        if ui
            .add_enabled_ui(bs.preview_page > 0, |ui| {
                crate::widgets::icon_button(ui, Icon::Left, ink(bs.preview_page > 0), lang.tr("Previous page"), vec2(28.0, 28.0))
            })
            .inner
            .clicked()
        {
            bs.preview_page = bs.preview_page.saturating_sub(1);
        }
        ui.label(RichText::new(label).font(theme::ui_font(13.0)).color(t.text));
        if ui
            .add_enabled_ui(bs.preview_page < last, |ui| {
                crate::widgets::icon_button(ui, Icon::Right, ink(bs.preview_page < last), lang.tr("Next page"), vec2(28.0, 28.0))
            })
            .inner
            .clicked()
        {
            bs.preview_page = (bs.preview_page + 1).min(last);
        }
    });
    ui.vertical_centered(|ui| dim(ui, &format!("{sheet} · {range}")));
}

/// Most rows and columns one preview page draws (a page holds far fewer at any readable scale).
const PREVIEW_ROWS: u32 = 200;

const PREVIEW_COLS: u32 = 60;

/// Draws one printed page in miniature: the cells' displayed text at the page's print scale,
/// inside the sheet's margins. `paper_w` is the page width in points.
fn paint_page(p: &egui::Painter, rect: Rect, paper_w: f32, page: &Json, wb: &mkmatrix_engine::model::Workbook, t: &Tokens) {
    let Some(range) = page.get("range").and_then(Json::as_str).and_then(mkmatrix_engine::core::RangeRef::parse) else { return };
    let Some(sh) = page.get("sheet").and_then(Json::as_str).and_then(|n| wb.sheet_index(n)).and_then(|i| wb.sheets.get(i)) else { return };
    // Preview pixels per point, and per sheet pixel (sheet geometry is 96 dpi: 0.75 pt per pixel).
    let per_pt = rect.width() / paper_w;
    let print_scale =
        page.get("scale").and_then(Json::as_f64).filter(|s| s.is_finite() && *s > 0.0).map_or(1.0, |s| (s / 100.0) as f32).clamp(0.1, 4.0);
    let per_px = per_pt * 0.75 * print_scale;
    let margin =
        |i: usize, default: f32| sh.print.margins.get(i).copied().filter(|m| m.is_finite()).unwrap_or(default).clamp(0.0, 3.0) * 72.0 * per_pt;
    let body = Rect::from_min_max(
        pos2(rect.left() + margin(0, 0.7), rect.top() + margin(2, 0.75)),
        pos2(rect.right() - margin(1, 0.7), rect.bottom() - margin(3, 0.75)),
    );
    if !body.is_positive() {
        return;
    }
    let font = theme::ui_font((11.0 * per_pt * print_scale).clamp(3.0, 14.0));
    let last_row = range.end.row.min(range.start.row.saturating_add(PREVIEW_ROWS));
    let last_col = range.end.col.min(range.start.col.saturating_add(PREVIEW_COLS));
    let mut y = body.top();
    for row in range.start.row..=last_row {
        let h = sh.row_height(row) * per_px;
        if h <= 0.0 {
            continue;
        }
        if y >= body.bottom() {
            break;
        }
        let mut x = body.left();
        for col in range.start.col..=last_col {
            let w = sh.col_width(col) * per_px;
            if w <= 0.0 {
                continue;
            }
            if x >= body.right() {
                break;
            }
            let cell = mkmatrix_engine::core::CellRef::new(row, col);
            let text = mkmatrix_engine::display::cell_text(wb, sh, cell);
            if !text.is_empty() {
                let numeric = sh.value(cell).as_f64().is_some();
                // Text runs on over empty neighbours, as on the sheet; numbers stay in their cell.
                let mut span = w;
                if !numeric {
                    let mut next = col.saturating_add(1);
                    while next <= last_col && x + span < body.right() && sh.value(mkmatrix_engine::core::CellRef::new(row, next)).is_empty() {
                        span += sh.col_width(next) * per_px;
                        next = next.saturating_add(1);
                    }
                }
                let clip = Rect::from_min_size(pos2(x, y), vec2(span, h)).intersect(body);
                let (anchor, at) =
                    if numeric { (Align2::RIGHT_CENTER, pos2(x + w - 1.0, y + h / 2.0)) } else { (Align2::LEFT_CENTER, pos2(x + 1.0, y + h / 2.0)) };
                p.with_clip_rect(clip).text(at, anchor, text, font.clone(), t.cell_text);
            }
            x += w;
        }
        y += h;
    }
}
