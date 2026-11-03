//! Reads a big sheet's rows on all cores. A row read depends only on read-only parts of the file,
//! so the part is cut between rows, blocks are read on any thread, and the rows go on the sheet in
//! file order, which is where what depends on earlier rows happens.
//!
//! This is only for well-formed sheets written in the usual way. Anything else, such as a row with
//! no number, a block that doesn't parse (a comment or CDATA cut in two gives that) or text in
//! another encoding, gives `None` and the caller reads the sheet from the start, one row at a time.

use rayon::prelude::*;

use super::{CellSource, El, ParsedRow, RowState, Shapes, Sheet, parse_row, place_row};
use crate::read::Ctx;
use crate::xml;

/// Bytes of a part below which threads aren't worth starting. `MKMATRIX_PARALLEL_MIN` in the
/// environment makes it 0, so tests of whole workbooks can run small sheets through this path.
fn min_bytes() -> usize {
    static MIN: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *MIN.get_or_init(|| if cfg!(test) || std::env::var_os("MKMATRIX_PARALLEL_MIN").is_some() { 0 } else { 8 * 1024 * 1024 })
}

/// Bytes a thread takes at a time.
const BLOCK: usize = if cfg!(test) { 256 } else { 512 * 1024 };

#[cfg(test)]
thread_local! {
    /// Makes this path decline every sheet, so tests can compare it with one thread.
    pub(crate) static DECLINE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// How many sheets this path read.
    pub(crate) static READ: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Where the next `<row` start tag is, at or after `from`.
pub(crate) fn next_row_tag(data: &[u8], from: usize) -> Option<usize> {
    let tag = b"<row";
    let mut at = from;
    while let Some(i) = data.get(at..)?.windows(tag.len()).position(|w| w == tag) {
        let pos = at + i;
        if matches!(data.get(pos + tag.len()), Some(b' ' | b'>' | b'/' | b'\t' | b'\r' | b'\n')) {
            return Some(pos);
        }
        at = pos + tag.len();
    }
    None
}

/// The rows of a sheet part: where they begin and where they end, when it looks as expected.
fn rows_span(data: &[u8]) -> Option<(usize, usize)> {
    let first = next_row_tag(data, 0)?;
    let open = b"<sheetData";
    data.get(..first)?.windows(open.len()).rposition(|w| w == open)?;
    let close = b"</sheetData>";
    let end = data.windows(close.len()).rposition(|w| w == close)?;
    (first < end).then_some((first, end))
}

/// Cuts `rows` between rows, in blocks of about [`BLOCK`] bytes.
fn blocks(rows: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut start = 0;
    while start < rows.len() {
        let end = next_row_tag(rows, start.saturating_add(BLOCK)).unwrap_or(rows.len());
        let Some(block) = rows.get(start..end) else { break };
        out.push(block);
        start = end;
    }
    out
}

/// Reads a block. `None` when it isn't a clean run of numbered rows.
fn read_block(src: &CellSource<'_>, block: &[u8], default_height: f32) -> Option<Vec<ParsedRow>> {
    let mut shapes = Shapes::default();
    let mut rows = Vec::new();
    let mut numbered = true;
    xml::parse_siblings(block, "row", &mut |row| {
        numbered &= row.attr_u32("r").is_some();
        rows.push(parse_row(src, &mut shapes, default_height, 0, &row));
        Ok(())
    })
    .ok()?;
    numbered.then_some(rows)
}

/// Reads the sheet's rows on all cores and gives the part's tree without them; `None` leaves the
/// sheet to the caller, whatever was placed on it so far.
pub(super) fn read_on_threads(cx: &mut Ctx<'_>, bytes: &[u8], sheet: &mut Sheet, st: &mut RowState) -> Option<El> {
    #[cfg(test)]
    if DECLINE.with(std::cell::Cell::get) {
        return None;
    }
    if bytes.len() < min_bytes() || !xml::is_utf8_as_is(bytes) {
        return None;
    }
    let (first, end) = rows_span(bytes)?;
    // The part without its rows.
    let mut outline = Vec::with_capacity(first + bytes.len() - end);
    outline.extend_from_slice(bytes.get(..first)?);
    outline.extend_from_slice(bytes.get(end..)?);
    let root = xml::parse(&outline).ok()?;
    let blocks = blocks(bytes.get(first..end)?);
    let warnings = cx.report.warnings.len();
    let height = sheet.default_row_height;
    // A block per thread at a time, so read rows don't pile up ahead of the sheet.
    let window = rayon::current_num_threads().max(1);
    for group in blocks.chunks(window) {
        let read: Vec<Option<Vec<ParsedRow>>> = {
            let src = CellSource::of(cx);
            group.par_iter().map(|block| read_block(&src, block, height)).collect()
        };
        for rows in read {
            let Some(rows) = rows else {
                cx.report.warnings.truncate(warnings);
                return None;
            };
            for row in rows {
                place_row(cx, sheet, st, row);
            }
        }
    }
    #[cfg(test)]
    READ.with(|n| n.set(n.get() + 1));
    Some(root)
}
