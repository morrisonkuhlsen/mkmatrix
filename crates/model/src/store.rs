//! Copy-on-write sparse cell storage.
//!
//! Cells live in bands of 64 rows, each band behind an `Arc`. Cloning a store (an undo snapshot)
//! copies only the band pointers; editing a cell copies just its band's list of columns and the
//! column it is in.
//!
//! Inside a band, cells are packed by column, each column behind an `Arc` of its own: a column
//! holds a bitmap of its stored rows and its cells in row order, one array per band instead of a
//! tree node per cell. A cell's place in the array is the number of stored rows above it.

use std::collections::BTreeMap;
use std::sync::Arc;

use mkmatrix_core::{CellError, CellRef, RangeRef, Value};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::cell::Cell;

const BAND: u32 = 64;

/// One column's cells in one band.
#[derive(Clone, Debug, Default, PartialEq)]
struct Column {
    col: u32,
    /// Bit `i` is set when the band's row `i` holds a cell.
    rows: u64,
    /// The stored cells, top to bottom: as many as `rows` has bits set.
    cells: Vec<Cell>,
}

impl Column {
    /// Where the cell of band row `bit` is (or would go) in `cells`.
    fn slot(&self, bit: u32) -> usize {
        if self.rows == u64::MAX { bit as usize } else { (self.rows & ((1u64 << bit) - 1)).count_ones() as usize }
    }
    fn has(&self, bit: u32) -> bool {
        (self.rows >> bit) & 1 == 1
    }
    fn get(&self, bit: u32) -> Option<&Cell> {
        if self.has(bit) { self.cells.get(self.slot(bit)) } else { None }
    }
}

/// The columns holding cells, in column order; a column with no cells is removed.
#[derive(Clone, Debug, Default, PartialEq)]
struct Band {
    columns: Vec<Arc<Column>>,
}

impl Band {
    fn column(&self, col: u32) -> Option<&Column> {
        self.columns.binary_search_by_key(&col, |x| x.col).ok().and_then(|i| self.columns.get(i)).map(|x| &**x)
    }
    fn get(&self, row: u32, col: u32) -> Option<&Cell> {
        self.column(col)?.get(row % BAND)
    }
    /// The columns in `c0..=c1`.
    fn columns_in(&self, c0: u32, c1: u32) -> &[Arc<Column>] {
        let from = self.columns.partition_point(|x| x.col < c0);
        let to = self.columns.partition_point(|x| x.col <= c1);
        self.columns.get(from..to).unwrap_or_default()
    }
}

/// Cells of some bands inside a block, row-major.
struct Cells<'a, I> {
    bands: I,
    block: (u32, u32, u32, u32),
    /// The current band's columns inside the block, and its first row.
    columns: &'a [Arc<Column>],
    base: u32,
    /// The band rows inside the block still to visit, the current one, and the next column of it.
    rows: u64,
    bit: u32,
    next: usize,
}

impl<'a, I: Iterator<Item = (u32, &'a Band)>> Cells<'a, I> {
    /// `block` is first row, last row, first column, last column.
    fn new(bands: I, block: (u32, u32, u32, u32)) -> Self {
        Cells { bands, block, columns: &[], base: 0, rows: 0, bit: 0, next: 0 }
    }
}

impl<'a, I: Iterator<Item = (u32, &'a Band)>> Iterator for Cells<'a, I> {
    type Item = (CellRef, &'a Cell);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            while let Some(column) = self.columns.get(self.next) {
                self.next += 1;
                if let Some(cell) = column.get(self.bit) {
                    return Some((CellRef::new(self.base + self.bit, column.col), cell));
                }
            }
            if self.rows != 0 {
                self.bit = self.rows.trailing_zeros();
                self.rows &= self.rows - 1;
                self.next = 0;
                continue;
            }
            let (key, band) = self.bands.next()?;
            let (r0, r1, c0, c1) = self.block;
            self.base = key.saturating_mul(BAND);
            self.columns = band.columns_in(c0, c1);
            self.next = self.columns.len();
            self.rows = 0;
            if r0 <= self.base.saturating_add(BAND - 1) && r1 >= self.base {
                let top = r0.saturating_sub(self.base);
                let bottom = (r1 - self.base).min(BAND - 1);
                let inside = (u64::MAX << top) & (u64::MAX >> (BAND - 1 - bottom));
                self.rows = self.columns.iter().fold(0, |all, column| all | column.rows) & inside;
            }
        }
    }
}

/// A whole band's cells, row-major.
fn band_cells(key: u32, band: &Band) -> impl Iterator<Item = (CellRef, &Cell)> {
    Cells::new(std::iter::once((key, band)), (0, u32::MAX, 0, u32::MAX))
}

/// How a store is saved: `{"bands":{"0":{"rows":{"4":{"0":cell}}}},"len":1}`.
#[derive(Serialize, Deserialize)]
struct SavedBand<C> {
    rows: BTreeMap<u32, BTreeMap<u32, C>>,
}

#[derive(Clone, Debug, Default)]
pub struct CellStore {
    bands: BTreeMap<u32, Arc<Band>>,
    len: usize,
    /// Cells a file placed outside the sheet grid: left out when read, reported by `rebuild`.
    dropped_on_read: usize,
}

impl PartialEq for CellStore {
    fn eq(&self, other: &CellStore) -> bool {
        self.len == other.len && self.bands == other.bands
    }
}

impl Serialize for CellStore {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Saved<'a> {
            bands: BTreeMap<u32, SavedBand<&'a Cell>>,
            len: usize,
        }
        let mut bands: BTreeMap<u32, SavedBand<&Cell>> = BTreeMap::new();
        for (c, cell) in self.iter() {
            bands.entry(c.row / BAND).or_insert_with(|| SavedBand { rows: BTreeMap::new() }).rows.entry(c.row).or_default().insert(c.col, cell);
        }
        Saved { bands, len: self.len }.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CellStore {
    /// Band keys and the count a file carries are derived state and aren't trusted: each cell goes
    /// where its row says (#1008).
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<CellStore, D::Error> {
        #[derive(Deserialize)]
        struct Saved {
            bands: BTreeMap<u32, SavedBand<Cell>>,
            #[serde(rename = "len")]
            _len: usize,
        }
        let saved = Saved::deserialize(deserializer)?;
        let mut store = CellStore::new();
        for band in saved.bands.into_values() {
            for (row, cols) in band.rows {
                for (col, cell) in cols {
                    let at = CellRef::new(row, col);
                    if at.is_valid() {
                        store.set(at, cell);
                    } else {
                        store.dropped_on_read += 1;
                    }
                }
            }
        }
        Ok(store)
    }
}

impl CellStore {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn get(&self, c: CellRef) -> Option<&Cell> {
        self.bands.get(&(c.row / BAND))?.get(c.row, c.col)
    }
    /// Mutable access (copies the band if shared).
    pub fn get_mut(&mut self, c: CellRef) -> Option<&mut Cell> {
        let band = self.bands.get_mut(&(c.row / BAND))?;
        band.get(c.row, c.col)?;
        let band = Arc::make_mut(band);
        let i = band.columns.binary_search_by_key(&c.col, |x| x.col).ok()?;
        let column = Arc::make_mut(band.columns.get_mut(i)?);
        let slot = column.slot(c.row % BAND);
        column.cells.get_mut(slot)
    }
    /// Stores a cell; a default (empty, unstyled) cell is removed instead. A constant (a cell with
    /// no formula) can't be `#SPILL!` or `#CALC!`: those describe a formula's result, aren't error
    /// literals of the file format, and a saved file keeps them as `#VALUE!`. A pasted value or a
    /// typed one becomes `#VALUE!` here too, so the workbook reads the same after a reload (#104).
    pub fn set(&mut self, c: CellRef, mut cell: Cell) {
        if cell.formula.is_none() && matches!(cell.value, Value::Error(CellError::Spill | CellError::Calc)) {
            cell.value = Value::Error(CellError::Value);
        }
        if cell.is_blank() {
            self.remove(c);
            return;
        }
        let band = Arc::make_mut(self.bands.entry(c.row / BAND).or_default());
        let i = match band.columns.binary_search_by_key(&c.col, |x| x.col) {
            Ok(i) => i,
            Err(i) => {
                band.columns.insert(i, Arc::new(Column { col: c.col, ..Column::default() }));
                i
            }
        };
        let Some(column) = band.columns.get_mut(i).map(Arc::make_mut) else { return };
        let bit = c.row % BAND;
        let slot = column.slot(bit);
        if column.has(bit) {
            if let Some(old) = column.cells.get_mut(slot) {
                *old = cell;
            }
        } else {
            column.cells.insert(slot, cell);
            column.rows |= 1u64 << bit;
            self.len += 1;
        }
    }
    /// Drops the cells outside the sheet grid, those left out when the store was read included;
    /// returns how many (#1008).
    pub fn rebuild(&mut self) -> usize {
        let outside: Vec<CellRef> = self.iter().filter(|(c, _)| !c.is_valid()).map(|(c, _)| c).collect();
        let dropped = std::mem::take(&mut self.dropped_on_read) + outside.len();
        for c in outside {
            self.remove(c);
        }
        dropped
    }

    pub fn remove(&mut self, c: CellRef) -> Option<Cell> {
        let key = c.row / BAND;
        let shared = self.bands.get_mut(&key)?;
        shared.get(c.row, c.col)?;
        let band = Arc::make_mut(shared);
        let i = band.columns.binary_search_by_key(&c.col, |x| x.col).ok()?;
        let column = Arc::make_mut(band.columns.get_mut(i)?);
        let bit = c.row % BAND;
        let slot = column.slot(bit);
        let old = column.cells.remove(slot);
        column.rows &= !(1u64 << bit);
        if column.cells.is_empty() {
            band.columns.remove(i);
        }
        if band.columns.is_empty() {
            self.bands.remove(&key);
        }
        self.len -= 1;
        Some(old)
    }
    /// Cells in `r`, row-major. Efficient for sparse data: skips empty bands and columns.
    pub fn iter_range(&self, r: RangeRef) -> impl Iterator<Item = (CellRef, &Cell)> + '_ {
        let (b0, b1) = (r.start.row / BAND, r.end.row / BAND);
        Cells::new(self.bands.range(b0..=b1).map(|(k, b)| (*k, &**b)), (r.start.row, r.end.row, r.start.col, r.end.col))
    }
    pub fn iter(&self) -> impl Iterator<Item = (CellRef, &Cell)> + '_ {
        Cells::new(self.bands.iter().map(|(k, b)| (*k, &**b)), (0, u32::MAX, 0, u32::MAX))
    }
    /// Cells of one row in `c0..=c1`.
    pub fn row(&self, row: u32, c0: u32, c1: u32) -> impl Iterator<Item = (u32, &Cell)> + '_ {
        let columns: &[Arc<Column>] = self.bands.get(&(row / BAND)).map_or(&[], |b| b.columns_in(c0, c1));
        columns.iter().filter_map(move |column| Some((column.col, column.get(row % BAND)?)))
    }
    /// Smallest range containing every stored cell, `None` when empty.
    pub fn used_range(&self) -> Option<RangeRef> {
        let stored = |band: &Band| band.columns.iter().fold(0u64, |all, column| all | column.rows);
        let (first_key, first) = self.bands.iter().next()?;
        let (last_key, last) = self.bands.iter().next_back()?;
        let r0 = first_key.saturating_mul(BAND).saturating_add(stored(first).trailing_zeros());
        let r1 = last_key.saturating_mul(BAND).saturating_add((BAND - 1).saturating_sub(stored(last).leading_zeros()));
        let c0 = self.bands.values().filter_map(|b| b.columns.first()).map(|x| x.col).min()?;
        let c1 = self.bands.values().filter_map(|b| b.columns.last()).map(|x| x.col).max()?;
        Some(RangeRef::new(CellRef::new(r0, c0), CellRef::new(r1, c1)))
    }
    /// Whether `c` holds a value or a formula (a cell with only a style does not).
    pub fn has(&self, c: CellRef) -> bool {
        self.get(c).is_some_and(Cell::has_value)
    }
    /// Last row (inclusive, capped at `cap`) of the maximal run of contiguous rows at or after
    /// `from.row` that store a cell in `from.col` with a non-empty *value* in `from.col`. `None`
    /// when `from` itself doesn't qualify.
    ///
    /// Deliberately narrower than `has`/`get` (which also count a formula with no cached value
    /// yet, for Ctrl+arrow navigation): this matches `Sheet::value_ref`, whose callers test the
    /// `Value` it returns with `!v.is_empty()` regardless of whether a formula produced it — a
    /// formula cell with an empty cached value (`=""`, or not yet recalculated) is "empty" to
    /// them. A stored cell with no value and no formula (for example a formatted spill child) does
    /// fall through to `spill` in `value_ref`, so `last_contiguous_nonempty` asks `value_ref`
    /// about the row where this run stops.
    ///
    /// Looks each band's column up once, however long the run.
    pub fn contiguous_value_run(&self, from: CellRef, cap: u32) -> Option<u32> {
        let mut last = None;
        let mut expect = from.row;
        while expect <= cap {
            let key = expect / BAND;
            let Some(column) = self.bands.get(&key).and_then(|b| b.column(from.col)) else { break };
            let base = key.saturating_mul(BAND);
            let bottom = cap.min(base.saturating_add(BAND - 1));
            while expect <= bottom {
                if column.get(expect - base).is_none_or(|c| c.value.is_empty()) {
                    return last;
                }
                last = Some(expect);
                let Some(next) = expect.checked_add(1) else { return last };
                expect = next;
            }
        }
        last
    }
    /// Positions whose cells differ between `self` and `other` (stored on one side only, or not
    /// equal). Bands both stores share are skipped, so comparing an edited copy with the store
    /// it was cloned from only visits the bands the edit touched.
    pub fn diff(&self, other: &CellStore) -> Vec<CellRef> {
        let empty = Band::default();
        let mut keys: Vec<u32> = self.bands.keys().chain(other.bands.keys()).copied().collect();
        keys.sort_unstable();
        keys.dedup();
        let mut out = Vec::new();
        for k in keys {
            let (a, b) = (self.bands.get(&k), other.bands.get(&k));
            if let (Some(a), Some(b)) = (a, b)
                && Arc::ptr_eq(a, b)
            {
                continue;
            }
            let (a, b): (&Band, &Band) = (a.map_or(&empty, |x| x), b.map_or(&empty, |x| x));
            for (c, cell) in band_cells(k, a) {
                if b.get(c.row, c.col) != Some(cell) {
                    out.push(c);
                }
            }
            for (c, _) in band_cells(k, b) {
                if a.get(c.row, c.col).is_none() {
                    out.push(c);
                }
            }
        }
        out
    }
    /// Approximate heap storage kept alive by this snapshot's bands that `newer` no longer
    /// shares. This is for bounding Undo history, not for reporting process memory: allocator
    /// overhead varies by platform. Shared formulas and values are counted only when the newer
    /// band no longer holds the same Arc.
    pub fn retained_bytes_against(&self, newer: &CellStore) -> usize {
        use std::mem::size_of;

        let mut bytes = 0usize;
        for (key, band) in &self.bands {
            let next = newer.bands.get(key);
            if next.is_some_and(|b| Arc::ptr_eq(b, band)) {
                continue;
            }
            bytes = bytes.saturating_add(size_of::<Band>()).saturating_add(band.columns.capacity().saturating_mul(size_of::<Arc<Column>>()));
            // A column the newer band still shares costs this snapshot only its pointer.
            let unshared = |column: &Column| !next.and_then(|b| b.column(column.col)).is_some_and(|x| std::ptr::eq(x, column));
            for column in band.columns.iter().filter(|x| unshared(x)) {
                // A cloned column duplicates the array of cells even when their formula/value
                // Arcs are still shared with the next snapshot.
                bytes = bytes.saturating_add(size_of::<Column>()).saturating_add(column.cells.capacity().saturating_mul(size_of::<Cell>()));
            }
            for (at, cell) in band_cells(*key, band).filter(|(at, _)| band.column(at.col).is_some_and(unshared)) {
                let next_cell = next.and_then(|b| b.get(at.row, at.col));
                if let Some(formula) = &cell.formula
                    && !next_cell.and_then(|c| c.formula.as_ref()).is_some_and(|f| Arc::ptr_eq(f, formula))
                {
                    bytes = bytes.saturating_add(size_of::<crate::cell::Formula>() + formula.text_bytes());
                    if let Some(expr) = &formula.expr {
                        expr.walk(&mut |_| bytes = bytes.saturating_add(size_of::<mkmatrix_formula::Expr>()));
                    }
                }
                match &cell.value {
                    mkmatrix_core::Value::Text(text)
                        if !next_cell.and_then(|c| c.value.as_text()).is_some_and(|t| std::ptr::eq(t, text.as_ref())) =>
                    {
                        bytes = bytes.saturating_add(text.len());
                    }
                    mkmatrix_core::Value::Array(array)
                        if !next_cell.is_some_and(|c| matches!(&c.value, mkmatrix_core::Value::Array(a) if Arc::ptr_eq(a, array))) =>
                    {
                        bytes = bytes
                            .saturating_add(size_of::<mkmatrix_core::Array>() + array.data.len().saturating_mul(size_of::<mkmatrix_core::Value>()));
                    }
                    _ => {}
                }
            }
        }
        bytes
    }
    /// Removes every cell in `r` and returns them.
    pub fn take_range(&mut self, r: RangeRef) -> Vec<(CellRef, Cell)> {
        let keys: Vec<CellRef> = self.iter_range(r).map(|(c, _)| c).collect();
        keys.into_iter().filter_map(|c| self.remove(c).map(|cell| (c, cell))).collect()
    }
    /// Rows strictly greater than or equal to `from` moved by `delta` (positive = down). Rows that
    /// would land before `from`... callers delete the vacated block first.
    pub fn shift_rows(&mut self, from: u32, delta: i64) {
        let moved: Vec<(CellRef, Cell)> = {
            let keys: Vec<CellRef> = self.iter().filter(|(c, _)| c.row >= from).map(|(c, _)| c).collect();
            keys.into_iter().filter_map(|c| self.remove(c).map(|cell| (c, cell))).collect()
        };
        for (c, cell) in moved {
            let nr = c.row as i64 + delta;
            if (0..mkmatrix_core::MAX_ROWS as i64).contains(&nr) {
                self.set(CellRef::new(nr as u32, c.col), cell);
            }
        }
    }
    pub fn shift_cols(&mut self, from: u32, delta: i64) {
        let keys: Vec<CellRef> = self.iter().filter(|(c, _)| c.col >= from).map(|(c, _)| c).collect();
        let moved: Vec<(CellRef, Cell)> = keys.into_iter().filter_map(|c| self.remove(c).map(|cell| (c, cell))).collect();
        for (c, cell) in moved {
            let nc = c.col as i64 + delta;
            if (0..mkmatrix_core::MAX_COLS as i64).contains(&nc) {
                self.set(CellRef::new(c.row, nc as u32), cell);
            }
        }
    }
    /// Shifts cells within rows `r0..=r1` and columns `>= from` horizontally (Insert/Delete Cells
    /// with shift right/left).
    pub fn shift_cols_in_rows(&mut self, r0: u32, r1: u32, from: u32, delta: i64) {
        let range = RangeRef::new(CellRef::new(r0, from), CellRef::new(r1, mkmatrix_core::MAX_COLS - 1));
        let moved = self.take_range(range);
        for (c, cell) in moved {
            let nc = c.col as i64 + delta;
            if (0..mkmatrix_core::MAX_COLS as i64).contains(&nc) {
                self.set(CellRef::new(c.row, nc as u32), cell);
            }
        }
    }
    /// Shifts cells within columns `c0..=c1` and rows `>= from` vertically.
    pub fn shift_rows_in_cols(&mut self, c0: u32, c1: u32, from: u32, delta: i64) {
        let range = RangeRef::new(CellRef::new(from, c0), CellRef::new(mkmatrix_core::MAX_ROWS - 1, c1));
        let moved = self.take_range(range);
        for (c, cell) in moved {
            let nr = c.row as i64 + delta;
            if (0..mkmatrix_core::MAX_ROWS as i64).contains(&nr) {
                self.set(CellRef::new(nr as u32, c.col), cell);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mkmatrix_core::Value;

    fn c(r: u32, col: u32) -> CellRef {
        CellRef::new(r, col)
    }

    #[test]
    fn set_get_remove() {
        let mut s = CellStore::new();
        s.set(c(0, 0), Cell::value(1.0.into()));
        s.set(c(100, 5), Cell::value("x".into()));
        assert_eq!(s.len(), 2);
        assert_eq!(s.get(c(100, 5)).map(|x| x.value.clone()), Some(Value::from("x")));
        assert_eq!(s.used_range().unwrap().a1(), "A1:F101");
        s.set(c(0, 0), Cell::default());
        assert_eq!(s.len(), 1);
        assert!(s.remove(c(100, 5)).is_some());
        assert!(s.is_empty());
        assert!(s.used_range().is_none());
    }

    #[test]
    fn cow_snapshots() {
        let mut s = CellStore::new();
        for r in 0..1000 {
            s.set(c(r, 0), Cell::value((r as f64).into()));
        }
        let snap = s.clone();
        s.set(c(5, 0), Cell::value(99.0.into()));
        assert_eq!(snap.get(c(5, 0)).unwrap().value, Value::Number(5.0));
        assert_eq!(s.get(c(5, 0)).unwrap().value, Value::Number(99.0));
    }

    #[test]
    fn diff_visits_changed_bands() {
        let mut s = CellStore::new();
        for r in 0..1000 {
            s.set(c(r, 0), Cell::value((r as f64).into()));
        }
        let snap = s.clone();
        assert!(s.diff(&snap).is_empty());
        s.set(c(5, 0), Cell::value(99.0.into()));
        s.set(c(700, 3), Cell::value("new".into()));
        s.remove(c(900, 0));
        assert_eq!(snap.diff(&s), [c(5, 0), c(700, 3), c(900, 0)]);
    }

    #[test]
    fn range_iteration_order() {
        let mut s = CellStore::new();
        s.set(c(70, 2), Cell::value(3.0.into()));
        s.set(c(1, 1), Cell::value(1.0.into()));
        s.set(c(1, 3), Cell::value(2.0.into()));
        s.set(c(500, 0), Cell::value(4.0.into()));
        let v: Vec<f64> = s.iter_range(RangeRef::parse("A1:D100").unwrap()).map(|(_, x)| x.value.as_f64().unwrap()).collect();
        assert_eq!(v, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn used_range_follows_cells_added_and_removed() {
        let mut s = CellStore::new();
        assert_eq!(s.used_range(), None);
        s.set(c(5, 3), Cell::value(Value::Number(1.0)));
        s.set(c(70, 1), Cell::value(Value::Number(2.0)));
        assert_eq!(s.used_range(), Some(RangeRef::new(c(5, 1), c(70, 3))));
        // A snapshot keeps its own bounds while the store changes under it.
        let snapshot = s.clone();
        s.set(c(6, 9), Cell::value(Value::Number(3.0)));
        assert_eq!(s.used_range(), Some(RangeRef::new(c(5, 1), c(70, 9))));
        s.remove(c(6, 9));
        s.remove(c(70, 1));
        assert_eq!(s.used_range(), Some(RangeRef::new(c(5, 3), c(5, 3))));
        assert_eq!(snapshot.used_range(), Some(RangeRef::new(c(5, 1), c(70, 3))));
        // Equality ignores whether the bounds were worked out.
        let mut fresh = CellStore::new();
        fresh.set(c(5, 3), Cell::value(Value::Number(1.0)));
        assert_eq!(s, fresh);
        // A store read back from JSON works its bounds out again.
        let json = serde_json::to_string(&snapshot).unwrap();
        let back: CellStore = serde_json::from_str(&json).unwrap();
        assert_eq!(back.used_range(), Some(RangeRef::new(c(5, 1), c(70, 3))));
    }

    /// A store checked step by step against the plainest model of what it promises.
    #[test]
    fn answers_as_a_plain_map_does() {
        use crate::style::StyleId;
        use std::collections::BTreeMap;

        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = move |n: u32| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state % u64::from(n)) as u32
        };
        for round in 0..60 {
            // Dense blocks, tall columns, wide rows; a band's edges are always in reach.
            let (rows, cols) = [(70, 8), (400, 3), (5, 300), (2000, 2000)][round % 4];
            let mut s = CellStore::new();
            let mut m: BTreeMap<(u32, u32), Cell> = BTreeMap::new();
            let mut snapshots: Vec<(CellStore, BTreeMap<(u32, u32), Cell>)> = Vec::new();
            for _ in 0..1500 {
                let at = c(next(rows), next(cols));
                match next(12) {
                    0..=5 => {
                        let cell = Cell::value(Value::Number(f64::from(next(1000))));
                        s.set(at, cell.clone());
                        m.insert((at.row, at.col), cell);
                    }
                    6 => {
                        let cell = Cell { value: Value::Empty, formula: None, style: StyleId(1 + next(3)) };
                        s.set(at, cell.clone());
                        m.insert((at.row, at.col), cell);
                    }
                    7 => {
                        s.set(at, Cell::default());
                        m.remove(&(at.row, at.col));
                    }
                    8 => assert_eq!(s.remove(at), m.remove(&(at.row, at.col))),
                    // A stored cell emptied in place stays stored.
                    9 => {
                        if let Some(cell) = s.get_mut(at) {
                            cell.value = Value::Empty;
                        }
                        if let Some(cell) = m.get_mut(&(at.row, at.col)) {
                            cell.value = Value::Empty;
                        }
                    }
                    10 => {
                        if snapshots.len() < 8 {
                            snapshots.push((s.clone(), m.clone()));
                        }
                    }
                    _ => assert_eq!(s.get(at), m.get(&(at.row, at.col))),
                }
                assert_eq!(s.len(), m.len());
            }
            let sees = |s: &CellStore, m: &BTreeMap<(u32, u32), Cell>| {
                assert_eq!(s.len(), m.len());
                assert!(s.iter().map(|(at, cell)| ((at.row, at.col), cell)).eq(m.iter().map(|(k, v)| (*k, v))), "iter, round {round}");
                let used = m.keys().fold(None::<(u32, u32, u32, u32)>, |u, &(r, col)| {
                    Some(u.map_or((r, r, col, col), |(a, b, x, y)| (a.min(r), b.max(r), x.min(col), y.max(col))))
                });
                assert_eq!(s.used_range().map(|u| (u.start.row, u.end.row, u.start.col, u.end.col)), used);
            };
            sees(&s, &m);
            // The snapshots haven't moved, and comparing with each finds what changed.
            for (snap, was) in &snapshots {
                sees(snap, was);
                let mut found = snap.diff(&s);
                found.sort_unstable();
                let mut expect: Vec<CellRef> = was.keys().chain(m.keys()).filter(|k| was.get(k) != m.get(k)).map(|&(r, col)| c(r, col)).collect();
                expect.sort_unstable();
                expect.dedup();
                assert_eq!(found, expect);
            }
            for _ in 0..40 {
                let r = RangeRef::new(c(next(rows + 70), next(cols + 3)), c(next(rows + 70), next(cols + 3)));
                let got: Vec<_> = s.iter_range(r).map(|(at, cell)| (at, cell.clone())).collect();
                let want: Vec<_> = m.iter().filter(|(k, _)| r.contains(c(k.0, k.1))).map(|(k, v)| (c(k.0, k.1), v.clone())).collect();
                assert_eq!(got, want, "iter_range {r:?}");
                let row = next(rows);
                let (c0, c1) = (r.start.col, r.end.col);
                let got: Vec<_> = s.row(row, c0, c1).map(|(col, cell)| (col, cell.clone())).collect();
                let want: Vec<_> = m.iter().filter(|(k, _)| k.0 == row && k.1 >= c0 && k.1 <= c1).map(|(k, v)| (k.1, v.clone())).collect();
                assert_eq!(got, want, "row {row}");
                let (from, cap) = (c(next(rows), next(cols)), next(rows + 70));
                let filled = |r: u32| m.get(&(r, from.col)).is_some_and(|x| !x.value.is_empty());
                let want = (from.row..=cap).take_while(|r| filled(*r)).last();
                assert_eq!(s.contiguous_value_run(from, cap), want, "run from {from:?} to {cap}");
            }
            // A save and a read give the same store, but for cells emptied in place: reading goes
            // through `set`, which doesn't store a blank cell.
            let back: CellStore = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
            let mut kept = CellStore::new();
            for (at, cell) in s.iter() {
                kept.set(at, cell.clone());
            }
            assert_eq!(back, kept);
        }
    }

    #[test]
    fn the_saved_form_is_unchanged() {
        let mut s = CellStore::new();
        s.set(c(70, 2), Cell::value(1.0.into()));
        s.set(c(4, 0), Cell::value(2.0.into()));
        s.set(c(4, 1), Cell::value(3.0.into()));
        let cell = |v: &str| format!(r#"{{"value":{v},"formula":null,"style":0}}"#);
        let value = serde_json::to_value(&s).unwrap();
        let number = |n: f64| serde_json::to_value(Value::Number(n)).unwrap().to_string();
        let expect = format!(
            r#"{{"bands":{{"0":{{"rows":{{"4":{{"0":{},"1":{}}}}}}},"1":{{"rows":{{"70":{{"2":{}}}}}}}}},"len":3}}"#,
            cell(&number(2.0)),
            cell(&number(3.0)),
            cell(&number(1.0)),
        );
        assert_eq!(value, serde_json::from_str::<serde_json::Value>(&expect).unwrap());
        assert_eq!(serde_json::from_str::<CellStore>(&expect).unwrap(), s);
    }

    #[test]
    fn a_file_cannot_place_cells_or_forge_the_count() {
        let cell = serde_json::to_string(&Cell { style: crate::style::StyleId(7), ..Cell::default() }).unwrap();
        let json = format!(r#"{{"bands":{{"9":{{"rows":{{"4":{{"0":{cell}}},"1048576":{{"0":{cell}}},"5":{{"16384":{cell}}}}}}}}},"len":99}}"#);
        let mut s: CellStore = serde_json::from_str(&json).unwrap();
        assert_eq!(s.len(), 1);
        assert!(s.get(c(4, 0)).is_some());
        assert_eq!(s.rebuild(), 2);
        assert_eq!(s.rebuild(), 0);
        assert!(s.remove(c(4, 0)).is_some());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn a_snapshot_shares_the_columns_an_edit_does_not_touch() {
        let mut s = CellStore::new();
        for r in 0..64 {
            for col in 0..6 {
                s.set(c(r, col), Cell::value(1.0.into()));
            }
        }
        let snap = s.clone();
        let whole = snap.retained_bytes_against(&CellStore::new());
        // A new column and an edit in another: the snapshot keeps one column of its own.
        s.set(c(3, 7), Cell::value(2.0.into()));
        s.set(c(3, 2), Cell::value(2.0.into()));
        let kept = snap.retained_bytes_against(&s);
        assert!(kept * 3 < whole, "{kept} of {whole}");
        assert_eq!(snap.get(c(3, 2)).unwrap().value, Value::Number(1.0));
        assert_eq!(s.get(c(3, 2)).unwrap().value, Value::Number(2.0));
        assert_eq!(snap.diff(&s), [c(3, 2), c(3, 7)]);
    }

    #[test]
    fn shifting() {
        let mut s = CellStore::new();
        s.set(c(2, 0), Cell::value(1.0.into()));
        s.set(c(5, 0), Cell::value(2.0.into()));
        s.shift_rows(3, 2);
        assert!(s.get(c(7, 0)).is_some());
        assert!(s.get(c(2, 0)).is_some());
        s.shift_cols(0, 1);
        assert!(s.get(c(7, 1)).is_some());
    }
}
