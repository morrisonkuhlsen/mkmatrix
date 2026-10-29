use super::*;

/// Built-in cell styles (our own vintage palette, [`crate::palette`]: inks on paper).
pub fn builtin_cell_style(name: &str, theme: &Theme) -> Option<Style> {
    use crate::palette::{self, CREAM, INK, tone};
    let mut s = Style::default();
    // Accent `i` (1..=6) at a tint.
    let accent = |i: u8, tint: i16| tone(palette::accent(usize::from(i).saturating_sub(1)), tint);
    let rgb = Color::Rgb;
    let ink = rgb(INK);
    let taupe = rgb(0x8C8279);
    match name {
        "Normal" => {}
        // Good, Bad and Neutral: sage, dusty rose and parchment, each with its own deep ink.
        "Good" => {
            s.fill = Fill::solid(rgb(0xDDE5CF));
            s.font.color = rgb(0x3F5A2A);
        }
        "Bad" => {
            s.fill = Fill::solid(rgb(0xF0D5D0));
            s.font.color = rgb(0x7A2E3A);
        }
        "Neutral" => {
            s.fill = Fill::solid(rgb(0xF3E6C4));
            s.font.color = rgb(0x7A5A1E);
        }
        "Calculation" => {
            s.fill = Fill::solid(rgb(0xF4EFE6));
            s.font.color = rgb(0xA0522D);
            s.font.bold = true;
            let l = BorderLine { style: BorderStyle::Thin, color: taupe };
            s.borders = Borders { left: l, right: l, top: l, bottom: l, ..Default::default() };
        }
        "Check Cell" => {
            s.fill = Fill::solid(taupe);
            s.font.color = rgb(CREAM);
            s.font.bold = true;
            let l = BorderLine { style: BorderStyle::Double, color: ink };
            s.borders = Borders { left: l, right: l, top: l, bottom: l, ..Default::default() };
        }
        "Explanatory Text" => {
            s.font.italic = true;
            s.font.color = rgb(0x7D746B);
        }
        "Input" => {
            s.fill = Fill::solid(rgb(0xF6E3CC));
            s.font.color = rgb(0x2F4A5C);
            let l = BorderLine { style: BorderStyle::Thin, color: taupe };
            s.borders = Borders { left: l, right: l, top: l, bottom: l, ..Default::default() };
        }
        "Linked Cell" => {
            s.font.color = rgb(0xA0522D);
            s.borders.bottom = BorderLine { style: BorderStyle::Double, color: accent(5, 0) };
        }
        "Note" => {
            s.fill = Fill::solid(rgb(0xFBF3D5));
            let l = BorderLine { style: BorderStyle::Thin, color: rgb(0xC9BFA8) };
            s.borders = Borders { left: l, right: l, top: l, bottom: l, ..Default::default() };
        }
        "Output" => {
            s.fill = Fill::solid(rgb(0xF4EFE6));
            s.font.color = ink;
            s.font.bold = true;
            let l = BorderLine { style: BorderStyle::Thin, color: ink };
            s.borders = Borders { left: l, right: l, top: l, bottom: l, ..Default::default() };
        }
        "Warning Text" => s.font.color = rgb(0xA3322B),
        // Titles and headings in the ink, ruled in petrol.
        "Title" => {
            s.font.size = 18.0;
            s.font.color = ink;
            s.font.name = theme.major_font.clone();
        }
        "Heading 1" => {
            s.font.size = 15.0;
            s.font.bold = true;
            s.font.color = ink;
            s.borders.bottom = BorderLine { style: BorderStyle::Thick, color: accent(1, 0) };
        }
        "Heading 2" => {
            s.font.size = 13.0;
            s.font.bold = true;
            s.font.color = ink;
            s.borders.bottom = BorderLine { style: BorderStyle::Thick, color: accent(1, 500) };
        }
        "Heading 3" => {
            s.font.bold = true;
            s.font.color = ink;
            s.borders.bottom = BorderLine { style: BorderStyle::Medium, color: accent(1, 400) };
        }
        "Heading 4" => {
            s.font.bold = true;
            s.font.color = ink;
        }
        "Total" => {
            s.font.bold = true;
            s.borders.top = BorderLine { style: BorderStyle::Thin, color: accent(1, 0) };
            s.borders.bottom = BorderLine { style: BorderStyle::Double, color: accent(1, 0) };
        }
        "Currency" => s.num_fmt = NumFmt::new(ACCOUNTING),
        "Comma" => s.num_fmt = NumFmt::new(COMMA),
        "Percent" => s.num_fmt = NumFmt::new("0%"),
        "Currency [0]" => s.num_fmt = NumFmt::new("_(\"$\"* #,##0_);_(\"$\"* \\(#,##0\\);_(\"$\"* \"-\"_);_(@_)"),
        "Comma [0]" => s.num_fmt = NumFmt::new("_(* #,##0_);_(* \\(#,##0\\);_(* \"-\"_);_(@_)"),
        n => {
            // AccentN, 20%/40%/60% - AccentN
            let (pct, rest) = match n.split_once("% - ") {
                Some((p, r)) => (p.parse::<i16>().ok(), r),
                None => (None, n),
            };
            let i: u8 = rest.strip_prefix("Accent")?.parse().ok()?;
            if !(1..=6).contains(&i) {
                return None;
            }
            // The text in cream or the ink, whichever reads on the fill.
            let tint = pct.map_or(0, |p| (1000 - p.saturating_mul(10)).clamp(0, 1000));
            let fill = accent(i, tint);
            s.fill = Fill::solid(fill);
            if let Color::Rgb(c) = fill {
                s.font.color = rgb(palette::on(c));
            }
        }
    }
    Some(s)
}

pub const CELL_STYLE_NAMES: &[&str] = &[
    "Normal",
    "Bad",
    "Good",
    "Neutral",
    "Calculation",
    "Check Cell",
    "Explanatory Text",
    "Input",
    "Linked Cell",
    "Note",
    "Output",
    "Warning Text",
    "Heading 1",
    "Heading 2",
    "Heading 3",
    "Heading 4",
    "Title",
    "Total",
    "20% - Accent1",
    "20% - Accent2",
    "20% - Accent3",
    "20% - Accent4",
    "20% - Accent5",
    "20% - Accent6",
    "40% - Accent1",
    "40% - Accent2",
    "40% - Accent3",
    "40% - Accent4",
    "40% - Accent5",
    "40% - Accent6",
    "60% - Accent1",
    "60% - Accent2",
    "60% - Accent3",
    "60% - Accent4",
    "60% - Accent5",
    "60% - Accent6",
    "Accent1",
    "Accent2",
    "Accent3",
    "Accent4",
    "Accent5",
    "Accent6",
    "Comma",
    "Comma [0]",
    "Currency",
    "Currency [0]",
    "Percent",
];

pub(super) fn cell_style(s: &mut Session, p: &Json) -> Result<Json> {
    let name = str_param(p, "name").unwrap_or("Normal").to_string();
    let d = s.doc()?;
    let style =
        d.wb.cell_styles
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(&name))
            .map(|(_, s)| s.clone())
            .or_else(|| builtin_cell_style(&name, &d.wb.theme));
    let Some(style) = style else { return Err(bad("home.cellStyle", format!("unknown cell style `{name}`"))) };
    // Number-format styles only change the number format.
    let only_fmt = matches!(name.as_str(), "Comma" | "Comma [0]" | "Currency" | "Currency [0]" | "Percent");
    apply_style(s, p, move |st| {
        if only_fmt {
            st.num_fmt = style.num_fmt.clone();
        } else {
            let keep_fmt = st.num_fmt.clone();
            *st = style.clone();
            if style.num_fmt.as_str() == "General" {
                st.num_fmt = keep_fmt;
            }
        }
    })
}
