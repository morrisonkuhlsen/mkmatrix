//! Functions that read a reference cell by cell instead of copying it into an array first: the
//! criteria functions COUNTIFS, SUMIFS and AVERAGEIFS, and the exact searches of MATCH, VLOOKUP
//! and XLOOKUP. Over a whole column of a big sheet the copy is most of the work (a million values
//! cloned into an array that is read once), so these walk the stored cells and skip the blanks
//! between them, which a criterion or a search can answer for all at once.
//!
//! Each one answers exactly what the function answers from the array. A case it doesn't cover
//! (a criterion or needle that is an array the function lifts over, a mode other than the exact
//! linear search, a block the host can't walk) returns `None`, and the caller copies the
//! reference and calls the function as before.

use mkmatrix_functions::{Criterion, ExactKey, ifs_criteria};

use super::*;

/// A reference argument read by walking it: its index, the area as written (to copy it when the
/// walk is declined) and the block that copy would read (`Evaluator::area_extent`).
#[derive(Clone, Copy)]
pub(super) struct Walked {
    pub(super) at: usize,
    pub(super) area: Area,
    pub(super) block: Area,
}

/// The row `walk_ifs_rows` is in, and what the rows before it added up to.
struct Row {
    at: Option<u32>,
    /// Each criterion's answer for the row's cell in its column.
    hits: Vec<bool>,
    /// The target's number or error in this row.
    target: Option<Result<f64, CellError>>,
    /// Rows meeting every criterion; numbers summed at them; the first error met there.
    matched: u64,
    sum: f64,
    numbers: u64,
    error: Option<CellError>,
    /// Rows met so far.
    rows: u64,
}

impl Row {
    /// Adds the current row, if it meets every criterion; `false` once an error ends the walk.
    fn settle(&mut self) -> bool {
        if self.at.is_none() || !self.hits.iter().all(|h| *h) {
            return true;
        }
        self.matched += 1;
        match self.target {
            Some(Ok(n)) => {
                self.sum += n;
                self.numbers += 1;
                true
            }
            Some(Err(e)) => {
                self.error = Some(e);
                false
            }
            None => true,
        }
    }
}

/// Whether argument `i` of `name` is a range these functions can walk instead of copying.
pub(super) fn walks(name: &str, i: usize) -> bool {
    match name {
        "SUMIFS" | "AVERAGEIFS" => i == 0 || !i.is_multiple_of(2),
        "COUNTIFS" => i.is_multiple_of(2),
        "COUNTIF" => i == 0,
        "MATCH" | "VLOOKUP" => i == 1,
        _ => false,
    }
}

impl<'h> Evaluator<'h> {
    /// The block a multi-cell reference argument would be copied from, once the formulas in it
    /// are calculated (in the order copying it would calculate them); `None` when it should be
    /// copied as usual.
    pub(super) fn walkable(&mut self, at: usize, area: &Area) -> Option<Walked> {
        if area.range.is_single() {
            return None;
        }
        let range = self.area_extent(area).ok()?;
        let block = Area { sheet: area.sheet, range };
        // Stops at the first stored cell: only the calculation of the block's formulas is wanted.
        self.host.visit_range(block.sheet, block.range, &mut |_, _| false).then_some(Walked { at, area: *area, block })
    }

    /// `name` over `args`, with the walked references in `walked` (their slots in `args` are
    /// placeholders); `None` when the walk doesn't cover the call.
    pub(super) fn walk_call(&mut self, name: &str, args: &[Arg], walked: &[Walked]) -> Option<Value> {
        match name {
            "COUNTIFS" | "SUMIFS" | "AVERAGEIFS" => self.walk_ifs(name, args, walked),
            "COUNTIF" => self.walk_countif(args, walked),
            "MATCH" => self.walk_match(args, walked),
            "VLOOKUP" => self.walk_vlookup(args, walked),
            _ => None,
        }
    }

    /// COUNTIFS, SUMIFS and AVERAGEIFS: a mask of the matching positions, built pair by pair from
    /// the stored cells (blanks take the criterion's answer for a blank), then a count or a sum of
    /// the target's numbers at those positions. Errors as in `criteria_mask` and `masked_numbers`.
    fn walk_ifs(&mut self, name: &str, args: &[Arg], walked: &[Walked]) -> Option<Value> {
        let block = |i: usize| walked.iter().find(|w| w.at == i).map(|w| w.block);
        let first_pair = if name == "COUNTIFS" { 0 } else { 1 };
        let rest = args.len().checked_sub(first_pair).filter(|n| *n > 0 && n.is_multiple_of(2))?;
        let pairs = rest / 2;
        let ranges: Vec<Area> = (0..pairs).map(|k| block(first_pair + 2 * k)).collect::<Option<_>>()?;
        let target = if name == "COUNTIFS" { None } else { Some(block(0)?) };
        let criteria: Vec<Value> = (0..pairs).map(|k| args.get(first_pair + 2 * k + 1).map(|a| a.value.clone())).collect::<Option<_>>()?;
        if criteria.iter().any(|v| matches!(v, Value::Array(_))) {
            return None;
        }
        let shape = target.or(ranges.first().copied())?.range;
        let (h, w) = (shape.height() as usize, shape.width() as usize);
        if ranges.iter().any(|r| r.range.height() != shape.height() || r.range.width() != shape.width()) {
            return Some(Value::Error(CellError::Value));
        }
        let criteria = match ifs_criteria(&criteria) {
            Ok(c) => c,
            Err(e) => return Some(Value::Error(e)),
        };
        if let Some(v) = self.walk_ifs_rows(name, &ranges, target, &criteria) {
            return Some(v);
        }
        let mut mask = vec![true; h.checked_mul(w)?];
        for (range, criterion) in ranges.iter().zip(&criteria) {
            let mut hit = vec![criterion.matches(&Value::Empty); mask.len()];
            let start = range.range.start;
            let walked = self.host.visit_range(range.sheet, range.range, &mut |c, v| {
                let i = c.row.saturating_sub(start.row) as usize * w + c.col.saturating_sub(start.col) as usize;
                if let Some(slot) = hit.get_mut(i) {
                    *slot = criterion.matches(v);
                }
                true
            });
            if !walked {
                return None;
            }
            for (m, hit) in mask.iter_mut().zip(hit) {
                *m &= hit;
            }
        }
        let Some(target) = target else {
            return Some(Value::number(mask.iter().filter(|m| **m).count() as f64));
        };
        let (mut sum, mut count, mut error) = (0.0, 0u64, None);
        let start = target.range.start;
        let walked = self.host.visit_range(target.sheet, target.range, &mut |c, v| {
            let i = c.row.saturating_sub(start.row) as usize * w + c.col.saturating_sub(start.col) as usize;
            if !mask.get(i).copied().unwrap_or(false) {
                return true;
            }
            match v {
                Value::Number(n) => {
                    sum += n;
                    count += 1;
                    true
                }
                Value::Error(e) => {
                    error = Some(*e);
                    false
                }
                _ => true,
            }
        });
        if !walked {
            return None;
        }
        Some(match error {
            Some(e) => Value::Error(e),
            None if name == "AVERAGEIFS" && count == 0 => Value::Error(CellError::Div0),
            None if name == "AVERAGEIFS" => Value::number(sum / count as f64),
            None => Value::number(sum),
        })
    }

    /// `walk_ifs` in one walk when every block is one column over the same rows (`SUMIFS(B:B,
    /// C:C,">50")`): the columns are read together and each row is settled when the walk leaves
    /// it, rather than building a mask over all rows. Rows with no stored cell take the
    /// criteria's answers for blanks. `None` when the blocks aren't like that.
    fn walk_ifs_rows(&mut self, name: &str, ranges: &[Area], target: Option<Area>, criteria: &[Criterion]) -> Option<Value> {
        let first = target.or(ranges.first().copied())?;
        let blocks = || ranges.iter().chain(target.iter());
        if blocks().any(|b| b.sheet != first.sheet || b.range.width() != 1 || b.range.start.row != first.range.start.row) {
            return None;
        }
        let c0 = blocks().map(|b| b.range.start.col).min()?;
        let c1 = blocks().map(|b| b.range.start.col).max()?;
        let span = RangeRef::new(CellRef::new(first.range.start.row, c0), CellRef::new(first.range.end.row, c1));
        let blank: Vec<bool> = criteria.iter().map(|c| c.matches(&Value::Empty)).collect();
        let mut row = Row { at: None, hits: blank.clone(), target: None, matched: 0, sum: 0.0, numbers: 0, error: None, rows: 0 };
        let walked = self.host.visit_range(first.sheet, span, &mut |c, v| {
            if row.at != Some(c.row) {
                if !row.settle() {
                    return false;
                }
                row.at = Some(c.row);
                row.hits.clone_from(&blank);
                row.target = None;
                row.rows += 1;
            }
            for ((range, criterion), hit) in ranges.iter().zip(criteria).zip(row.hits.iter_mut()) {
                if range.range.start.col == c.col {
                    *hit = criterion.matches(v);
                }
            }
            if target.is_some_and(|t| t.range.start.col == c.col) {
                row.target = match v {
                    Value::Number(n) => Some(Ok(*n)),
                    Value::Error(e) => Some(Err(*e)),
                    _ => None,
                };
            }
            true
        });
        if !walked {
            return None;
        }
        row.settle();
        Some(match row.error {
            Some(e) => Value::Error(e),
            None if name == "COUNTIFS" => {
                // Rows the walk never met hold only blanks.
                let unmet = if blank.iter().all(|b| *b) { u64::from(first.range.height()).saturating_sub(row.rows) } else { 0 };
                Value::number(row.matched.saturating_add(unmet) as f64)
            }
            None if name == "AVERAGEIFS" && row.numbers == 0 => Value::Error(CellError::Div0),
            None if name == "AVERAGEIFS" => Value::number(row.sum / row.numbers as f64),
            None => Value::number(row.sum),
        })
    }

    /// COUNTIF: the stored cells that match, and the blanks between them when the criterion
    /// matches a blank.
    fn walk_countif(&mut self, args: &[Arg], walked: &[Walked]) -> Option<Value> {
        let [w] = walked else { return None };
        let criterion = args.get(1).map(|a| a.value.clone()).filter(|v| w.at == 0 && !matches!(v, Value::Array(_)))?;
        let criterion = match ifs_criteria(&[criterion]) {
            Ok(mut c) => c.pop()?,
            Err(e) => return Some(Value::Error(e)),
        };
        let (mut matched, mut stored) = (0u64, 0u64);
        let walked = self.host.visit_range(w.block.sheet, w.block.range, &mut |_, v| {
            stored += 1;
            matched += u64::from(criterion.matches(v));
            true
        });
        if !walked {
            return None;
        }
        let cells = u64::from(w.block.range.height()) * u64::from(w.block.range.width());
        let blanks = if criterion.matches(&Value::Empty) { cells.saturating_sub(stored) } else { 0 };
        Some(Value::number(matched.saturating_add(blanks) as f64))
    }

    /// MATCH with match type 0 over one row or column.
    fn walk_match(&mut self, args: &[Arg], walked: &[Walked]) -> Option<Value> {
        let [w] = walked else { return None };
        if w.at != 1 || !matches!(args.get(2).map(|a| &a.value), Some(Value::Number(n)) if *n == 0.0) {
            return None;
        }
        let key = ExactKey::new(&args.first()?.value, true)?;
        let range = w.block.range;
        if range.height() != 1 && range.width() != 1 {
            return Some(Value::Error(CellError::NA));
        }
        let found = self.find_exact(w.block, &key, false)?;
        Some(found.map_or(Value::Error(CellError::NA), |p| Value::Number(f64::from(p) + 1.0)))
    }

    /// VLOOKUP with an exact match (`FALSE` or 0 as its fourth argument).
    fn walk_vlookup(&mut self, args: &[Arg], walked: &[Walked]) -> Option<Value> {
        let [w] = walked else { return None };
        if w.at != 1 || !matches!(args.get(3).map(|a| &a.value), Some(Value::Bool(false))) {
            return None;
        }
        let key = ExactKey::new(&args.first()?.value, true)?;
        let Some(Value::Number(idx)) = args.get(2).map(|a| &a.value) else { return None };
        if idx.is_nan() || *idx < 1.0 {
            return Some(Value::Error(CellError::Value));
        }
        let idx = (idx.trunc() as u64).saturating_sub(1);
        let table = w.block.range;
        if idx >= u64::from(table.width()) {
            return Some(Value::Error(CellError::Ref));
        }
        let keys = RangeRef::new(table.start, CellRef::new(table.end.row, table.start.col));
        let found = self.find_exact(Area { sheet: w.block.sheet, range: keys }, &key, false)?;
        let Some(p) = found else { return Some(Value::Error(CellError::NA)) };
        // `idx` is below the table's width, a `u32`.
        let col = table.start.col.saturating_add(u32::try_from(idx).ok()?);
        match self.host.cell_value(w.block.sheet, CellRef::new(table.start.row.saturating_add(p), col)) {
            Value::Empty => Some(Value::Number(0.0)),
            v => Some(v),
        }
    }

    /// XMATCH's answer for XLOOKUP's arguments (needle, lookup block, then the match and search
    /// modes when given) when the search is linear and exact: match mode 0 (or 2, with
    /// wildcards) and search mode 1 or -1. `None` for the other modes.
    pub(super) fn walk_xmatch(&mut self, xargs: &[Arg], block: Area) -> Option<Value> {
        let mode = |i: usize, default: f64| match xargs.get(i).map(|a| &a.value) {
            None => Some(default),
            Some(Value::Number(n)) => Some(*n),
            Some(_) => None,
        };
        let (match_mode, search_mode) = (mode(2, 0.0)?, mode(3, 1.0)?);
        if !(match_mode == 0.0 || match_mode == 2.0) || !(search_mode == 1.0 || search_mode == -1.0) {
            return None;
        }
        let key = ExactKey::new(&xargs.first()?.value, match_mode == 2.0)?;
        let found = self.find_exact(block, &key, search_mode == -1.0)?;
        Some(found.map_or(Value::Error(CellError::NA), |p| Value::Number(f64::from(p) + 1.0)))
    }

    /// The 0-based position of `key` along a one-row or one-column block: the first match, or
    /// the last when `reverse`. `Some(None)` when it isn't there, `None` when the host can't walk
    /// the block.
    pub(super) fn find_exact(&mut self, block: Area, key: &ExactKey, reverse: bool) -> Option<Option<u32>> {
        let range = block.range;
        let vertical = range.width() == 1;
        let mut found = None;
        let walked = self.host.visit_range(block.sheet, range, &mut |c, v| {
            if !key.matches(v) {
                return true;
            }
            found = Some(if vertical { c.row.saturating_sub(range.start.row) } else { c.col.saturating_sub(range.start.col) });
            reverse
        });
        walked.then_some(found)
    }
}
