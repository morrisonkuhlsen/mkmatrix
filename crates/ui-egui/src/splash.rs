//! The splash screen: a small window of its own, without decorations, before the main one, as
//! MK Sigma and MK Perth have. In two halves: on Morrison Kühlsen's ink, MK Matrix's mark, name,
//! motto and version, a bar that fills while it shows with the program's parts named in turn,
//! and the house's wordmark; beside them, a picture of linear algebra's identities on a dark
//! field (`assets/splash/`), melting into the ink at its edge. Without the picture, artwork drawn
//! in code takes its place: a bracketed matrix, a row of it meeting a column in the cell their
//! product fills. It closes by itself after a moment; a click or Esc closes it sooner.

use egui::epaint::{Mesh, Vertex};
use egui::{Align2, Color32, CornerRadius, FontId, Rect, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions, pos2, vec2};

use crate::i18n::Language;

/// The window's size, in points, as MK Sigma's and MK Perth's.
pub const SIZE: [f32; 2] = [816.0, 480.0];
/// The height the layout's measures are given for; they scale with the window's.
const LAYOUT_HEIGHT: f32 = 400.0;
/// Seconds it stays.
const HOLD: f64 = 5.0;
/// The Morrison Kühlsen wordmark, white on clear (a brand asset: `docs/brand/`).
const MORRISON_KUHLSEN: &[u8] = include_bytes!("../../../docs/brand/morrison-kuhlsen.png");
/// The picture beside the words (original artwork, CC0: see ATTRIBUTION.md).
const ART: &[u8] = include_bytes!("../../../assets/splash/splash.jpg");
/// The part of the picture shown, across: its left side, where the matrix and `Ax = λx` are.
const ART_LEFT: f32 = 0.03;
/// The largest image side decoded, against a corrupt header.
const MAX_SIDE: u32 = 4096;
/// Morrison Kühlsen's ink, and the text on it.
pub(crate) const INK: Color32 = Color32::from_rgb(0x14, 0x16, 0x1C);
pub(crate) const ON_INK: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
pub(crate) const ON_INK_SOFT: Color32 = Color32::from_rgba_premultiplied(184, 184, 184, 184);
/// MK Matrix's colour: teal.
pub const BRAND: Color32 = Color32::from_rgb(0x2E, 0xC4, 0xA0);
/// The artwork's lighter and deeper teals.
const TEAL_LIGHT: Color32 = Color32::from_rgb(0xB8, 0xF2, 0xE2);
const TEAL_DEEP: Color32 = Color32::from_rgb(0x12, 0x5E, 0x52);
pub const NAME: &str = "MK Matrix";
/// "By number, weight and measure" (Wisdom 11:20).
pub const MOTTO: &str = "Numero, pondere et mensura";

/// The program's parts, named in turn as the bar fills.
const COMPONENTS: [&str; 12] = [
    "the formula parser",
    "the calculation engine",
    "the function library",
    "matrix algebra",
    "dynamic arrays",
    "number formats",
    "the XLSX reader and writer",
    "charts",
    "PDF export",
    "the grid",
    "the control channel",
    "the MCP server for agents",
];

#[derive(Debug, Default)]
pub struct SplashScreen {
    language: Language,
    /// When it first showed, in egui's time.
    started: Option<f64>,
    closed: bool,
}

impl SplashScreen {
    /// A splash in the given language.
    pub fn new(language: Language) -> Self {
        Self { language, ..Self::default() }
    }

    /// Whether it asked its window to close.
    pub fn closed(&self) -> bool {
        self.closed
    }

    /// Draws it over the whole window; asks the window to close when its time is up.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        let started = *self.started.get_or_insert(now);
        let progress = ((now - started) / HOLD).clamp(0.0, 1.0) as f32;
        // The whole window: the root Ui comes inset by a margin.
        let rect = ctx.content_rect();
        let response = ui.allocate_rect(ui.max_rect(), Sense::click());
        paint(&ui.painter().with_clip_rect(rect), &ctx, rect, progress, self.language);
        let done = progress >= 1.0 || response.clicked() || ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if done && !self.closed {
            self.closed = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint();
    }
}

fn paint(painter: &egui::Painter, ctx: &egui::Context, rect: Rect, progress: f32, lang: Language) {
    // The layout is drawn for 400 points of height and scales with the window.
    let k = rect.height() / LAYOUT_HEIGHT;
    // Two halves: the words on Morrison Kühlsen's ink, the artwork beside them.
    let half = rect.left() + rect.width() / 2.0;
    let words = Rect::from_min_max(rect.min, pos2(half, rect.bottom()));
    let art = Rect::from_min_max(pos2(half, rect.top()), rect.max);
    painter.rect_filled(rect, 0.0, INK);
    match art_texture(ctx) {
        Some(texture) => picture(&painter.with_clip_rect(art), art, &texture),
        None => artwork(&painter.with_clip_rect(art), art, k),
    }

    let left = words.left() + 36.0 * k;
    let width = words.width() - 72.0 * k;
    // The mark, in teal, and the name.
    mark(painter, Rect::from_min_size(pos2(left, words.top() + 52.0 * k), vec2(46.0, 46.0) * k), BRAND);
    painter.text(pos2(left + 58.0 * k, words.top() + 75.0 * k), Align2::LEFT_CENTER, NAME, FontId::proportional(34.0 * k), ON_INK);
    painter.text(pos2(left, words.top() + 128.0 * k), Align2::LEFT_CENTER, MOTTO, FontId::proportional(15.0 * k), ON_INK_SOFT);
    let tagline = painter.layout(
        lang.tr("Spreadsheets of matrices: every cell a number, every block an algebra.").to_string(),
        FontId::proportional(12.5 * k),
        ON_INK_SOFT,
        width,
    );
    let below = words.top() + 152.0 * k + tagline.size().y;
    painter.galley(pos2(left, words.top() + 152.0 * k), tagline, ON_INK_SOFT);
    painter.text(
        pos2(left, below + 10.0 * k),
        Align2::LEFT_TOP,
        lang.tr("Version {version}").replace("{version}", env!("CARGO_PKG_VERSION")),
        FontId::proportional(12.5 * k),
        ON_INK_SOFT,
    );

    // The parts loading, over the progress bar.
    let bar = Rect::from_min_size(pos2(left, words.bottom() - 96.0 * k), vec2(width, 3.0 * k));
    let shown = ((progress * COMPONENTS.len() as f32) as usize).min(COMPONENTS.len().saturating_sub(1));
    let status = COMPONENTS.get(shown).map_or_else(String::new, |c| lang.tr("Loading {component}…").replace("{component}", lang.tr(c)));
    // A long part's name shrinks to the bar's width rather than run into the artwork.
    let size = 12.0 * k;
    let mut galley = painter.layout_no_wrap(status.clone(), FontId::proportional(size), ON_INK_SOFT);
    if galley.size().x > width {
        galley = painter.layout_no_wrap(status, FontId::proportional(size * width / galley.size().x), ON_INK_SOFT);
    }
    painter.galley(bar.left_top() - vec2(0.0, 10.0 * k + galley.size().y), galley, ON_INK_SOFT);
    painter.rect_filled(bar, 0.0, Color32::from_white_alpha(28));
    painter.rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * progress, bar.height())), 0.0, BRAND);

    // The house at the foot: its wordmark and line; the line alone without the wordmark.
    let foot = words.bottom() - 28.0 * k;
    if let Some(texture) = wordmark(ctx) {
        let logo_width = 104.0 * k;
        let size = vec2(logo_width, logo_width * texture.size_vec2().y / texture.size_vec2().x.max(1.0));
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        painter.image(texture.id(), Rect::from_min_size(pos2(left, foot - size.y), size), uv, ON_INK);
    }
    painter.text(
        pos2(left + width, foot - 4.0 * k),
        Align2::RIGHT_BOTTOM,
        lang.tr("A Morrison Kühlsen project"),
        FontId::proportional(11.0 * k),
        ON_INK_SOFT,
    );
}

/// MK Matrix's mark, drawn in code: a matrix's brackets around a 3×3 of cells, its diagonal lit.
pub fn mark(painter: &egui::Painter, rect: Rect, color: Color32) {
    let s = rect.width().min(rect.height());
    let r = Rect::from_center_size(rect.center(), vec2(s, s));
    let stroke = Stroke::new((s * 0.075).max(1.0), color);
    let serif = s * 0.16;
    for (x, dir) in [(r.left() + s * 0.06, 1.0), (r.right() - s * 0.06, -1.0)] {
        let (top, bottom) = (r.top() + s * 0.06, r.bottom() - s * 0.06);
        painter.line_segment([pos2(x, top), pos2(x, bottom)], stroke);
        painter.line_segment([pos2(x, top), pos2(x + dir * serif, top)], stroke);
        painter.line_segment([pos2(x, bottom), pos2(x + dir * serif, bottom)], stroke);
    }
    let cell = s * 0.17;
    let gap = s * 0.07;
    let start = r.center() - vec2(1.5 * cell + gap, 1.5 * cell + gap);
    for row in 0..3 {
        for col in 0..3 {
            let min = start + vec2(col as f32 * (cell + gap), row as f32 * (cell + gap));
            let fill = if row == col { color } else { color.gamma_multiply(0.4) };
            painter.rect_filled(Rect::from_min_size(min, vec2(cell, cell)), CornerRadius::same((cell * 0.2) as u8), fill);
        }
    }
}

/// The artwork, drawn in code: on a teal dusk over a sheet's faint grid, a bracketed 7×7 matrix
/// whose cells shade as a correlation matrix does (brightest on the diagonal), one of its rows
/// and one of its columns outlined, meeting in the cell their product fills.
pub(crate) fn artwork(painter: &egui::Painter, art: Rect, k: f32) {
    // The dusk: deep teal at the top left, brighter at the bottom right.
    let mut mesh = Mesh::default();
    for (pos, color) in [
        (art.left_top(), Color32::from_rgb(0x0C, 0x1F, 0x22)),
        (art.right_top(), Color32::from_rgb(0x10, 0x33, 0x34)),
        (art.right_bottom(), Color32::from_rgb(0x1D, 0x7A, 0x6A)),
        (art.left_bottom(), Color32::from_rgb(0x13, 0x45, 0x42)),
    ] {
        mesh.vertices.push(Vertex { pos, uv: egui::epaint::WHITE_UV, color });
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(mesh);
    // The sheet's faint grid.
    let line = Stroke::new(1.0, Color32::from_white_alpha(9));
    let (col_w, row_h) = (34.0 * k, 17.0 * k);
    let mut x = art.left() + col_w / 2.0;
    while x < art.right() {
        painter.vline(x, art.y_range(), line);
        x += col_w;
    }
    let mut y = art.top() + row_h / 2.0;
    while y < art.bottom() {
        painter.hline(art.x_range(), y, line);
        y += row_h;
    }

    // The matrix: 7×7 cells, centred a little high.
    const N: usize = 7;
    let cell = 34.0 * k;
    let gap = 6.0 * k;
    let side = N as f32 * cell + (N - 1) as f32 * gap;
    let origin = pos2(art.center().x - side / 2.0, art.center().y - side / 2.0 - 8.0 * k);
    let at = |row: usize, col: usize| Rect::from_min_size(origin + vec2(col as f32 * (cell + gap), row as f32 * (cell + gap)), vec2(cell, cell));
    // The row and column that meet, and the cell they fill.
    let (pick_row, pick_col) = (2, 4);
    for row in 0..N {
        for col in 0..N {
            // A correlation-like shade: 1 on the diagonal, fading with the distance from it.
            let distance = row.abs_diff(col) as f32;
            let strength = 0.88_f32.powf(distance * distance);
            let fill = lerp(TEAL_DEEP, BRAND, strength).gamma_multiply(0.55 + 0.45 * strength);
            painter.rect_filled(at(row, col), CornerRadius::same((4.0 * k) as u8), fill);
        }
    }
    let outline = Stroke::new(1.6 * k, TEAL_LIGHT.gamma_multiply(0.8));
    let pad = 3.0 * k;
    let row_band = at(pick_row, 0).union(at(pick_row, N - 1)).expand(pad);
    let col_band = at(0, pick_col).union(at(N - 1, pick_col)).expand(pad);
    painter.rect_stroke(row_band, CornerRadius::same((6.0 * k) as u8), outline, StrokeKind::Outside);
    painter.rect_stroke(col_band, CornerRadius::same((6.0 * k) as u8), outline, StrokeKind::Outside);
    let product = at(pick_row, pick_col);
    painter.rect_filled(product, CornerRadius::same((4.0 * k) as u8), TEAL_LIGHT);
    painter.text(product.center(), Align2::CENTER_CENTER, "Σ", FontId::proportional(18.0 * k), TEAL_DEEP);

    // The brackets.
    let stroke = Stroke::new(2.4 * k, TEAL_LIGHT.gamma_multiply(0.9));
    let (top, bottom) = (origin.y - 12.0 * k, origin.y + side + 12.0 * k);
    let serif = 14.0 * k;
    for (x, dir) in [(origin.x - 16.0 * k, 1.0), (origin.x + side + 16.0 * k, -1.0)] {
        painter.line_segment([pos2(x, top), pos2(x, bottom)], stroke);
        painter.line_segment([pos2(x, top), pos2(x + dir * serif, top)], stroke);
        painter.line_segment([pos2(x, bottom), pos2(x + dir * serif, bottom)], stroke);
    }
    // Below it, the product that cell holds, as a formula would say it.
    painter.text(
        pos2(art.center().x, bottom + 26.0 * k),
        Align2::CENTER_CENTER,
        "c(3,5) = Σ a(3,k) · b(k,5)",
        FontId::proportional(15.0 * k),
        TEAL_LIGHT.gamma_multiply(0.85),
    );
}

/// A colour between `a` (t = 0) and `b` (t = 1).
fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}

/// The picture filling `art` (cropped, never stretched), its left edge fading into the ink.
fn picture(painter: &egui::Painter, art: Rect, texture: &TextureHandle) {
    let [w, h] = texture.size_vec2().into();
    // The share of the picture's width that fills `art` at the picture's full height.
    let across = ((art.width() / art.height()) / (w / h.max(1.0))).clamp(0.05, 1.0);
    let left = ART_LEFT.min(1.0 - across);
    let uv = Rect::from_min_max(pos2(left, 0.0), pos2(left + across, 1.0));
    painter.image(texture.id(), art, uv, Color32::WHITE);
    // The seam: the ink, fading out over the picture's first fifth.
    let seam = Rect::from_min_size(art.min, vec2(art.width() * 0.2, art.height()));
    let clear = Color32::from_rgba_premultiplied(0, 0, 0, 0);
    let mut mesh = Mesh::default();
    for (pos, color) in [(seam.left_top(), INK), (seam.right_top(), clear), (seam.right_bottom(), clear), (seam.left_bottom(), INK)] {
        mesh.vertices.push(Vertex { pos, uv: egui::epaint::WHITE_UV, color });
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(mesh);
}

/// The splash picture as a texture, loaded once per context.
fn art_texture(ctx: &egui::Context) -> Option<TextureHandle> {
    let id = egui::Id::unique("splash-art");
    if let Some(texture) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return Some(texture);
    }
    let image = decode(ART, image::ImageFormat::Jpeg).ok()?;
    let texture = ctx.load_texture("splash-art", image, TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, texture.clone()));
    Some(texture)
}

/// The Morrison Kühlsen wordmark, white on clear, as a texture, loaded once per context.
pub(crate) fn wordmark(ctx: &egui::Context) -> Option<TextureHandle> {
    let id = egui::Id::unique("splash-morrison-kuhlsen");
    if let Some(texture) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return Some(texture);
    }
    let image = decode_png(MORRISON_KUHLSEN).ok()?;
    let texture = ctx.load_texture("splash-morrison-kuhlsen", image, TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, texture.clone()));
    Some(texture)
}

/// A PNG as an egui image (RGBA), refusing absurd sizes.
fn decode_png(bytes: &[u8]) -> Result<egui::ColorImage, String> {
    decode(bytes, image::ImageFormat::Png)
}

/// An image in `format` as an egui image (RGBA), refusing absurd sizes.
fn decode(bytes: &[u8], format: image::ImageFormat) -> Result<egui::ColorImage, String> {
    let reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
    let (w, h) = reader.into_dimensions().map_err(|e| e.to_string())?;
    if w > MAX_SIDE || h > MAX_SIDE {
        return Err(format!("{w}×{h} is too large"));
    }
    let rgba = image::load_from_memory_with_format(bytes, format).map_err(|e| e.to_string())?.to_rgba8();
    let size = [usize::try_from(rgba.width()).map_err(|e| e.to_string())?, usize::try_from(rgba.height()).map_err(|e| e.to_string())?];
    Ok(egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wordmark_decodes() {
        let logo = decode_png(MORRISON_KUHLSEN).unwrap();
        assert!(logo.size[0] > logo.size[1] * 2, "a wide wordmark");
    }

    #[test]
    fn the_picture_decodes() {
        let art = decode(ART, image::ImageFormat::Jpeg).unwrap();
        assert!(art.size[0] > art.size[1], "a wide picture");
    }

    #[test]
    fn hostile_images_never_panic() {
        // A truncated JPEG may decode in part or fail; either way, no panic.
        let _ = decode(ART.get(..ART.len() / 3).unwrap(), image::ImageFormat::Jpeg);
        assert!(decode_png(&[]).is_err());
        for cut in [2, 60, MORRISON_KUHLSEN.len() / 2] {
            let _ = decode_png(MORRISON_KUHLSEN.get(..cut).unwrap());
        }
        let mut flipped = MORRISON_KUHLSEN.to_vec();
        for i in (0..flipped.len()).step_by(97) {
            if let Some(b) = flipped.get_mut(i) {
                *b ^= 0xff;
            }
            let _ = decode_png(&flipped);
        }
    }

    #[test]
    fn it_closes_on_esc_and_after_its_time() {
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(SIZE[0], SIZE[1]))
            .build_ui_state(|ui, s: &mut SplashScreen| s.ui(ui), SplashScreen::default());
        harness.run_steps(2);
        assert!(!harness.state().closed());
        harness.key_press(egui::Key::Escape);
        harness.run_steps(1);
        assert!(harness.state().closed());
        let mut waiting = egui_kittest::Harness::builder()
            .with_size(egui::vec2(SIZE[0], SIZE[1]))
            .with_step_dt(1.0)
            .build_ui_state(|ui, s: &mut SplashScreen| s.ui(ui), SplashScreen::default());
        waiting.run_steps(7);
        assert!(waiting.state().closed(), "gone after {HOLD} s");
    }

    #[test]
    fn every_part_reads_in_portuguese() {
        for c in COMPONENTS {
            assert_ne!(Language::PtBr.tr(c), c, "{c}");
        }
    }
}
