//! MK Matrix's own palette for the Format as Table and Cell Styles galleries: vintage inks on
//! paper. Six muted accents (petrol, oxblood, olive, ochre, terracotta, plum), a sepia ink for
//! text and neutral styles, and a warm paper that light tints fade towards instead of white.
//! Fixed colours, not the workbook theme's, so the galleries keep their look under any theme and
//! files show the same colours in other spreadsheet apps.

use mkmatrix_model::Color;

/// What light tints fade towards: warm paper.
pub const PAPER: u32 = 0xFBF8F2;
/// Text on the accents and on dark fills: cream.
pub const CREAM: u32 = 0xFBF7EE;
/// Sepia ink: text, headings and the neutral styles.
pub const INK: u32 = 0x3B3530;
/// What dark tints deepen towards.
const DEEP: u32 = 0x1F1B18;
/// The accents, Accent 1 to 6: petrol, oxblood, olive, ochre, terracotta, plum.
pub const ACCENTS: [u32; 6] = [0x2F5D62, 0x7A2E3A, 0x6B7A4B, 0xB8892E, 0xB5654A, 0x5E4B6B];

/// Accent `i` (0-based); past the end, the ink.
pub fn accent(i: usize) -> u32 {
    ACCENTS.get(i).copied().unwrap_or(INK)
}

/// `base` lightened towards paper (`tint` > 0) or deepened towards ink (`tint` < 0), in
/// thousandths as theme tints are.
pub fn tone(base: u32, tint: i16) -> Color {
    let t = f32::from(tint.clamp(-1000, 1000)) / 1000.0;
    let rgb = if t >= 0.0 { mix(base, PAPER, t) } else { mix(base, DEEP, -t) };
    Color::Rgb(rgb)
}

/// The text colour that reads on `fill`: cream on a dark fill, the ink on a light one.
pub fn on(fill: u32) -> u32 {
    let ch = |shift: u32| ((fill >> shift) & 0xFF) as f32 / 255.0;
    let luma = 0.2126 * ch(16) + 0.7152 * ch(8) + 0.0722 * ch(0);
    if luma < 0.5 { CREAM } else { INK }
}

/// The colour `t` (0..=1) of the way from `a` to `b`.
fn mix(a: u32, b: u32, t: f32) -> u32 {
    let ch = |c: u32, shift: u32| ((c >> shift) & 0xFF) as f32;
    let one = |shift: u32| (ch(a, shift) + (ch(b, shift) - ch(a, shift)) * t).round().clamp(0.0, 255.0) as u32;
    (one(16) << 16) | (one(8) << 8) | one(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tones_run_from_deep_ink_through_the_colour_to_paper() {
        assert_eq!(tone(0x2F5D62, 0), Color::Rgb(0x2F5D62));
        assert_eq!(tone(0x2F5D62, 1000), Color::Rgb(PAPER));
        assert_eq!(tone(0x2F5D62, -1000), Color::Rgb(DEEP));
        assert_eq!(tone(0x000000, 500), Color::Rgb(0x7E7C79));
        assert_eq!(tone(INK, i16::MAX), Color::Rgb(PAPER), "tints past 1000 clamp");
        assert_eq!(accent(9), INK);
        assert_eq!(on(0x2F5D62), CREAM, "cream on petrol");
        assert_eq!(on(0xE3CFA0), INK, "ink on a light ochre");
    }
}
