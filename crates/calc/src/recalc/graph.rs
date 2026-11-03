use super::*;
use mkmatrix_formula::Reference;
use mkmatrix_formula::adjust::shift_ref;
use std::sync::Arc;

/// A fast, non-cryptographic hasher for cell keys (FxHash-style multiply-rotate).
#[derive(Default, Clone, Copy)]
pub struct FxHasher(u64);

impl Hasher for FxHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.write_u64(*b as u64);
        }
    }
    fn write_u64(&mut self, i: u64) {
        self.0 = (self.0.rotate_left(5) ^ i).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    fn write_u32(&mut self, i: u32) {
        self.write_u64(i as u64);
    }
    fn write_usize(&mut self, i: usize) {
        self.write_u64(i as u64);
    }
}

pub(super) type HashMap<K, V> = std::collections::HashMap<K, V, BuildHasherDefault<FxHasher>>;

pub(super) type HashSet<K> = std::collections::HashSet<K, BuildHasherDefault<FxHasher>>;

pub type Key = (usize, CellRef);

/// The formulas that read one cell. Most cells are read by a single formula (a filled column's
/// `=B1*C1`, `=B2*C2`…), so that one is kept inline; a set is only allocated for the second.
#[derive(Clone, Debug)]
enum Readers {
    One(Key),
    Many(Box<HashSet<Key>>),
}

impl Readers {
    fn insert(&mut self, k: Key) {
        match self {
            Readers::One(only) if *only == k => {}
            Readers::One(only) => {
                let mut set = HashSet::default();
                set.insert(*only);
                set.insert(k);
                *self = Readers::Many(Box::new(set));
            }
            Readers::Many(set) => {
                set.insert(k);
            }
        }
    }
    /// Removes `k`; whether no reader is left.
    fn remove(&mut self, k: &Key) -> bool {
        match self {
            Readers::One(only) => only == k,
            Readers::Many(set) => {
                set.remove(k);
                match (set.len(), set.iter().next().copied()) {
                    (0, _) => true,
                    // Back to one reader: no set kept for it.
                    (1, Some(last)) => {
                        *self = Readers::One(last);
                        false
                    }
                    _ => false,
                }
            }
        }
    }
    fn for_each(&self, mut f: impl FnMut(Key)) {
        match self {
            Readers::One(k) => f(*k),
            Readers::Many(set) => set.iter().copied().for_each(f),
        }
    }
}

/// What a formula reads.
#[derive(Clone, Debug)]
enum Areas {
    List(Vec<Area>),
    /// The references of a tree shared with other formulas, moved by this one's offset: all on
    /// its own sheet (see [`crate::eval::plain_references`]). A filled column keeps one list.
    Moved(Arc<[Reference]>, i32, i32),
}

#[derive(Clone, Debug)]
pub(super) struct Node {
    areas: Areas,
    pub(super) dynamic: bool,
}

impl Node {
    pub(super) fn new(areas: Vec<Area>, dynamic: bool) -> Node {
        Node { areas: Areas::List(areas), dynamic }
    }
    pub(super) fn moved(refs: Arc<[Reference]>, rows: i64, cols: i64, dynamic: bool) -> Option<Node> {
        Some(Node { areas: Areas::Moved(refs, i32::try_from(rows).ok()?, i32::try_from(cols).ok()?), dynamic })
    }
    /// The areas read, for the node of a formula on `sheet`. A reference moved off the sheet reads
    /// nothing, as the `#REF!` it becomes.
    pub(super) fn areas(&self, sheet: usize) -> Vec<Area> {
        match &self.areas {
            Areas::List(areas) => areas.clone(),
            Areas::Moved(refs, rows, cols) => {
                refs.iter().filter_map(|r| shift_ref(r, i64::from(*rows), i64::from(*cols))).map(|r| Area { sheet, range: r.range() }).collect()
            }
        }
    }
}

/// Ranges wider than this many columns go to the per-sheet wide index instead of per-column ones.
const WIDE: u32 = 64;

/// Insertions an [`Intervals`] keeps aside (scanned by every query) before merging them in.
const PENDING: usize = 32;

/// Row intervals `[r0, r1]`, each with a payload, answering "which intervals overlap rows
/// `a..=b`?" in O(log n + k).
///
/// The intervals are sorted by start with an implicit segment tree of their largest end, so a query
/// only descends into subtrees that hold a match. Removals mark entries dead (re-inserting one
/// revives it); insertions wait in a small sorted set. [`Intervals::settle`] merges them and drops
/// dead entries once either grows, so mutations cost O(log n) amortised.
#[derive(Clone, Debug)]
pub(super) struct Intervals<T> {
    /// Sorted, without duplicates.
    items: Vec<(u32, u32, T)>,
    dead: Vec<bool>,
    dead_count: usize,
    /// Segment tree over `items`: node `i` has children `2i` and `2i + 1`, leaves start at
    /// `max_end.len() / 2`. Each node holds the largest end below it (dead entries included).
    max_end: Vec<u32>,
    /// Inserted since the last merge.
    pending: BTreeSet<(u32, u32, T)>,
}

impl<T> Default for Intervals<T> {
    fn default() -> Self {
        Intervals { items: Vec::new(), dead: Vec::new(), dead_count: 0, max_end: Vec::new(), pending: BTreeSet::new() }
    }
}

impl<T: Copy + Ord> Intervals<T> {
    pub(super) fn is_empty(&self) -> bool {
        self.items.len() == self.dead_count && self.pending.is_empty()
    }

    pub(super) fn insert(&mut self, r0: u32, r1: u32, t: T) {
        let it = (r0, r1, t);
        if let Ok(i) = self.items.binary_search(&it) {
            if let Some(d) = self.dead.get_mut(i)
                && *d
            {
                *d = false;
                self.dead_count -= 1;
            }
            return;
        }
        self.pending.insert(it);
    }

    pub(super) fn remove(&mut self, r0: u32, r1: u32, t: T) {
        let it = (r0, r1, t);
        if let Ok(i) = self.items.binary_search(&it) {
            if let Some(d) = self.dead.get_mut(i)
                && !*d
            {
                *d = true;
                self.dead_count += 1;
            }
            return;
        }
        self.pending.remove(&it);
    }

    /// Merges pending insertions and drops dead entries when there are enough of them.
    pub(super) fn settle(&mut self) {
        if self.pending.len() <= PENDING && self.dead_count <= (self.items.len() / 2).max(PENDING) {
            return;
        }
        let mut all: Vec<(u32, u32, T)> = Vec::with_capacity(self.items.len() - self.dead_count + self.pending.len());
        all.extend(self.items.iter().zip(&self.dead).filter(|(_, d)| !**d).map(|(it, _)| *it));
        all.extend(std::mem::take(&mut self.pending));
        // Two sorted runs: the stable sort merges them in linear time.
        all.sort();
        all.dedup();
        let size = all.len().next_power_of_two();
        let mut max_end = vec![0; 2 * size];
        if let Some(leaves) = max_end.get_mut(size..) {
            for (slot, it) in leaves.iter_mut().zip(&all) {
                *slot = it.1;
            }
        }
        for i in (1..size).rev() {
            let m = max_end.get(2 * i).copied().unwrap_or(0).max(max_end.get(2 * i + 1).copied().unwrap_or(0));
            if let Some(slot) = max_end.get_mut(i) {
                *slot = m;
            }
        }
        self.dead = vec![false; all.len()];
        self.dead_count = 0;
        self.items = all;
        self.max_end = max_end;
    }

    /// Calls `f` for every interval overlapping rows `a..=b` (`r0 <= b && r1 >= a`). Returns the
    /// number of tree nodes visited.
    pub(super) fn query(&self, a: u32, b: u32, f: &mut dyn FnMut(&T)) -> usize {
        for it in &self.pending {
            if it.0 <= b && it.1 >= a {
                f(&it.2);
            }
        }
        // Entries before `p` start at or before row `b`.
        let p = self.items.partition_point(|it| it.0 <= b);
        if p == 0 {
            return 0;
        }
        self.walk(1, 0, self.max_end.len() / 2, p, a, f)
    }

    fn walk(&self, node: usize, lo: usize, hi: usize, p: usize, a: u32, f: &mut dyn FnMut(&T)) -> usize {
        if lo >= p || self.max_end.get(node).is_none_or(|m| *m < a) {
            return 1;
        }
        if hi - lo <= 1 {
            if let (Some(it), Some(false)) = (self.items.get(lo), self.dead.get(lo))
                && it.1 >= a
            {
                f(&it.2);
            }
            return 1;
        }
        let mid = lo + (hi - lo) / 2;
        1 + self.walk(2 * node, lo, mid, p, a, f) + self.walk(2 * node + 1, mid, hi, p, a, f)
    }
}

/// Formulas in one column that read a single cell of another column (or their own) a fixed number
/// of rows away: a filled column's `=B1*C1`, `=B2*C2`… is one run per column it reads, however
/// long, instead of an entry per cell.
#[derive(Clone, Debug)]
struct AlignedRun {
    /// The readers' sheet and column.
    sheet: usize,
    col: u32,
    /// Reader's row minus the row it reads.
    delta: i64,
    /// The readers' rows: first → last of each stretch, none touching.
    rows: BTreeMap<u32, u32>,
}

impl AlignedRun {
    fn contains(&self, row: u32) -> bool {
        self.rows.range(..=row).next_back().is_some_and(|(_, last)| *last >= row)
    }
    fn insert(&mut self, row: u32) {
        let before = self.rows.range(..=row).next_back().map(|(a, b)| (*a, *b));
        if before.is_some_and(|(_, last)| last >= row) {
            return;
        }
        let after = row.checked_add(1).and_then(|next| self.rows.remove_entry(&next)).map(|(_, last)| last);
        let last = after.unwrap_or(row);
        match before {
            Some((first, end)) if end.checked_add(1) == Some(row) => {
                self.rows.insert(first, last);
            }
            _ => {
                self.rows.insert(row, last);
            }
        }
    }
    fn remove(&mut self, row: u32) {
        let Some((first, last)) = self.rows.range(..=row).next_back().map(|(a, b)| (*a, *b)) else { return };
        if last < row {
            return;
        }
        self.rows.remove(&first);
        if first < row {
            self.rows.insert(first, row - 1);
        }
        if row < last {
            self.rows.insert(row + 1, last);
        }
    }
    /// Reader of `row` in the column read, if any.
    fn reader_of(&self, row: u32) -> Option<CellRef> {
        let at = u32::try_from(i64::from(row) + self.delta).ok()?;
        self.contains(at).then(|| CellRef::new(at, self.col))
    }
    /// Readers of rows `r0..=r1` in the column read.
    fn readers_of(&self, r0: u32, r1: u32, out: &mut Vec<Key>) {
        let top = (i64::from(r0) + self.delta).max(0);
        let bottom = i64::from(r1) + self.delta;
        if bottom < 0 {
            return;
        }
        let (Ok(top), Ok(bottom)) = (u32::try_from(top), u32::try_from(bottom.min(i64::from(u32::MAX)))) else { return };
        // A stretch beginning above `top` may reach it.
        let first = self.rows.range(..=top).next_back().map_or(top, |(a, _)| *a);
        for (a, b) in self.rows.range(first..=bottom) {
            for at in (*a).max(top)..=(*b).min(bottom) {
                out.push((self.sheet, CellRef::new(at, self.col)));
            }
        }
    }
}

/// Most runs kept per column read. Past it a reader goes in the per-cell map, so that looking up a
/// cell stays cheap on a sheet whose formulas read one column from all sorts of distances.
const MAX_RUNS: usize = 16;

#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub(super) nodes: HashMap<Key, Node>,
    /// single cell → dependents
    cell_deps: HashMap<Key, Readers>,
    /// (sheet, col) read → the runs of single-cell readers aligned with it
    aligned: HashMap<(usize, u32), Vec<AlignedRun>>,
    /// (sheet, col) → row intervals of the ranges (up to `WIDE` columns) that cover the column
    col_deps: HashMap<(usize, u32), Intervals<Key>>,
    /// sheet → row intervals of wider ranges, with their columns: (first, last, dependent)
    wide_deps: HashMap<usize, Intervals<(u32, u32, Key)>>,
    pub(super) dynamic: HashSet<Key>,
    /// Range indexes changed since the last [`Graph::settle`].
    unsettled_cols: HashSet<(usize, u32)>,
    unsettled_wide: HashSet<usize>,
}

impl Graph {
    pub fn len(&self) -> usize {
        self.nodes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Index entries of a formula's precedents, without duplicates: single cells, column
    /// intervals and wide ranges.
    fn entries(areas: &[Area]) -> (Vec<Key>, Vec<(usize, u32, u32, u32)>, Vec<(usize, RangeRef)>) {
        let (mut cells, mut cols, mut wide) = (Vec::new(), Vec::new(), Vec::new());
        for a in areas {
            if a.range.is_single() {
                cells.push((a.sheet, a.range.start));
            } else if a.range.width() > WIDE {
                wide.push((a.sheet, a.range));
            } else {
                for c in a.range.start.col..=a.range.end.col {
                    cols.push((a.sheet, c, a.range.start.row, a.range.end.row));
                }
            }
        }
        cells.sort_unstable();
        cells.dedup();
        cols.sort_unstable();
        cols.dedup();
        wide.sort_unstable_by_key(|(s, r)| (*s, r.start, r.end));
        wide.dedup();
        (cells, cols, wide)
    }

    pub(super) fn insert(&mut self, k: Key, node: Node) {
        self.remove(k);
        let (cells, cols, wide) = Self::entries(&node.areas(k.0));
        for c in cells {
            if self.add_to_run(c, k) {
                continue;
            }
            match self.cell_deps.get_mut(&c) {
                Some(readers) => readers.insert(k),
                None => {
                    self.cell_deps.insert(c, Readers::One(k));
                }
            }
        }
        for (s, c, r0, r1) in cols {
            self.col_deps.entry((s, c)).or_default().insert(r0, r1, k);
            self.unsettled_cols.insert((s, c));
        }
        for (s, r) in wide {
            self.wide_deps.entry(s).or_default().insert(r.start.row, r.end.row, (r.start.col, r.end.col, k));
            self.unsettled_wide.insert(s);
        }
        if node.dynamic {
            self.dynamic.insert(k);
        }
        self.nodes.insert(k, node);
    }

    /// Adds `k` to a run for the cell it reads; false when the column has as many runs as it may.
    fn add_to_run(&mut self, read: Key, k: Key) -> bool {
        let delta = i64::from(k.1.row) - i64::from(read.1.row);
        let runs = self.aligned.entry((read.0, read.1.col)).or_default();
        if let Some(run) = runs.iter_mut().find(|r| r.delta == delta && r.col == k.1.col && r.sheet == k.0) {
            run.insert(k.1.row);
            return true;
        }
        if runs.len() >= MAX_RUNS {
            return false;
        }
        let mut run = AlignedRun { sheet: k.0, col: k.1.col, delta, rows: BTreeMap::new() };
        run.insert(k.1.row);
        runs.push(run);
        true
    }

    /// Takes `k` out of the run for the cell it reads; false when it isn't in one.
    fn remove_from_run(&mut self, read: Key, k: Key) -> bool {
        let delta = i64::from(k.1.row) - i64::from(read.1.row);
        let Some(runs) = self.aligned.get_mut(&(read.0, read.1.col)) else { return false };
        let Some(i) = runs.iter().position(|r| r.delta == delta && r.col == k.1.col && r.sheet == k.0 && r.contains(k.1.row)) else { return false };
        if let Some(run) = runs.get_mut(i) {
            run.remove(k.1.row);
            if run.rows.is_empty() {
                runs.swap_remove(i);
            }
        }
        if runs.is_empty() {
            self.aligned.remove(&(read.0, read.1.col));
        }
        true
    }

    pub(super) fn remove(&mut self, k: Key) {
        let Some(old) = self.nodes.remove(&k) else { return };
        let (cells, cols, wide) = Self::entries(&old.areas(k.0));
        for c in cells {
            if self.remove_from_run(c, k) {
                continue;
            }
            if let Some(v) = self.cell_deps.get_mut(&c)
                && v.remove(&k)
            {
                self.cell_deps.remove(&c);
            }
        }
        for (s, c, r0, r1) in cols {
            if let Some(v) = self.col_deps.get_mut(&(s, c)) {
                v.remove(r0, r1, k);
                self.unsettled_cols.insert((s, c));
            }
        }
        for (s, r) in wide {
            if let Some(v) = self.wide_deps.get_mut(&s) {
                v.remove(r.start.row, r.end.row, (r.start.col, r.end.col, k));
                self.unsettled_wide.insert(s);
            }
        }
        self.dynamic.remove(&k);
    }

    /// Tidies the range indexes changed by `insert`/`remove` (queries stay correct without it,
    /// just slower).
    pub(super) fn settle(&mut self) {
        for key in std::mem::take(&mut self.unsettled_cols) {
            if let Some(v) = self.col_deps.get_mut(&key) {
                v.settle();
                if v.is_empty() {
                    self.col_deps.remove(&key);
                }
            }
        }
        for key in std::mem::take(&mut self.unsettled_wide) {
            if let Some(v) = self.wide_deps.get_mut(&key) {
                v.settle();
                if v.is_empty() {
                    self.wide_deps.remove(&key);
                }
            }
        }
    }

    /// Formulas that read cell `c` of `sheet`.
    pub fn dependents(&self, sheet: usize, c: CellRef, out: &mut Vec<Key>) {
        if let Some(v) = self.cell_deps.get(&(sheet, c)) {
            v.for_each(|k| out.push(k));
        }
        if let Some(runs) = self.aligned.get(&(sheet, c.col)) {
            out.extend(runs.iter().filter_map(|run| Some((run.sheet, run.reader_of(c.row)?))));
        }
        if let Some(v) = self.col_deps.get(&(sheet, c.col)) {
            v.query(c.row, c.row, &mut |k| out.push(*k));
        }
        if let Some(v) = self.wide_deps.get(&sheet) {
            v.query(c.row, c.row, &mut |(c0, c1, k)| {
                if *c0 <= c.col && c.col <= *c1 {
                    out.push(*k);
                }
            });
        }
    }

    /// Formulas that read any cell of `range` (a formula can be listed more than once).
    pub fn dependents_of_range(&self, sheet: usize, range: RangeRef, out: &mut Vec<Key>) {
        if range.count() <= 4096 {
            for c in range.iter() {
                if let Some(v) = self.cell_deps.get(&(sheet, c)) {
                    v.for_each(|k| out.push(k));
                }
            }
        } else {
            for ((s, c), v) in &self.cell_deps {
                if *s == sheet && range.contains(*c) {
                    v.for_each(|k| out.push(k));
                }
            }
        }
        let (r0, r1) = (range.start.row, range.end.row);
        if (range.width() as usize) <= self.aligned.len() {
            for col in range.start.col..=range.end.col {
                for run in self.aligned.get(&(sheet, col)).into_iter().flatten() {
                    run.readers_of(r0, r1, out);
                }
            }
        } else {
            for ((s, col), runs) in &self.aligned {
                if *s == sheet && range.start.col <= *col && *col <= range.end.col {
                    for run in runs {
                        run.readers_of(r0, r1, out);
                    }
                }
            }
        }
        if (range.width() as usize) <= self.col_deps.len() {
            for col in range.start.col..=range.end.col {
                if let Some(v) = self.col_deps.get(&(sheet, col)) {
                    v.query(r0, r1, &mut |k| out.push(*k));
                }
            }
        } else {
            for ((s, col), v) in &self.col_deps {
                if *s == sheet && range.start.col <= *col && *col <= range.end.col {
                    v.query(r0, r1, &mut |k| out.push(*k));
                }
            }
        }
        if let Some(v) = self.wide_deps.get(&sheet) {
            v.query(r0, r1, &mut |(c0, c1, k)| {
                if *c0 <= range.end.col && range.start.col <= *c1 {
                    out.push(*k);
                }
            });
        }
    }

    /// Precedent areas of a formula cell (for Trace Precedents).
    pub fn precedents_of(&self, k: Key) -> Vec<Area> {
        self.nodes.get(&k).map(|n| n.areas(k.0)).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The graph checked step by step against a plain list of who reads what.
    #[test]
    fn finds_the_readers_a_plain_list_does() {
        let mut state = 0xA076_1D64_78BD_642Fu64;
        let mut next = move |n: u32| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state % u64::from(n)) as u32
        };
        let area = |sheet: usize, range: RangeRef| Area { sheet, range };
        for round in 0..60 {
            let (rows, cols) = [(30, 4), (300, 40), (5, 3)][round % 3];
            let mut g = Graph::default();
            let mut model: BTreeMap<Key, Vec<Area>> = BTreeMap::new();
            for step in 0..800 {
                let k = (next(2) as usize, CellRef::new(next(rows), next(cols)));
                if next(4) == 0 {
                    g.remove(k);
                    model.remove(&k);
                } else {
                    let mut areas = Vec::new();
                    for _ in 0..next(4) {
                        let at = CellRef::new(next(rows), next(cols));
                        let (sheet, kind) = (next(2) as usize, next(6));
                        // Filled columns: aligned with the reader, most of the time.
                        let at = if kind < 3 { CellRef::new(k.1.row, at.col) } else { at };
                        let end = CellRef::new(at.row + next(9), at.col + next(3));
                        areas.push(area(sheet, if kind == 5 { RangeRef::new(at, end) } else { RangeRef::cell(at) }));
                    }
                    g.insert(k, Node::new(areas.clone(), false));
                    model.insert(k, areas);
                }
                g.settle();
                if step % 20 != 0 {
                    continue;
                }
                let sorted = |mut v: Vec<Key>| {
                    v.sort_unstable();
                    v.dedup();
                    v
                };
                for _ in 0..12 {
                    let (sheet, at) = (next(2) as usize, CellRef::new(next(rows + 9), next(cols + 3)));
                    let mut got = Vec::new();
                    g.dependents(sheet, at, &mut got);
                    let want = model.iter().filter(|(_, a)| a.iter().any(|x| x.sheet == sheet && x.range.contains(at))).map(|(k, _)| *k).collect();
                    assert_eq!(sorted(got), sorted(want), "readers of {at:?} on sheet {sheet}");
                    let range = RangeRef::new(at, CellRef::new(next(rows + 9), next(cols + 3)));
                    let mut got = Vec::new();
                    g.dependents_of_range(sheet, range, &mut got);
                    let want =
                        model.iter().filter(|(_, a)| a.iter().any(|x| x.sheet == sheet && x.range.intersects(&range))).map(|(k, _)| *k).collect();
                    assert_eq!(sorted(got), sorted(want), "readers of {range:?} on sheet {sheet}");
                }
            }
            // Everything removed leaves nothing behind.
            for k in model.keys() {
                g.remove(*k);
            }
            g.settle();
            assert!(g.aligned.is_empty() && g.cell_deps.is_empty() && g.nodes.is_empty());
        }
    }

    #[test]
    fn a_filled_column_is_one_run_per_column_it_reads() {
        let mut g = Graph::default();
        for row in 0..1000 {
            let read = |col| Area { sheet: 0, range: RangeRef::cell(CellRef::new(row, col)) };
            g.insert((0, CellRef::new(row, 6)), Node::new(vec![read(1), read(2), read(3)], false));
        }
        assert!(g.cell_deps.is_empty());
        assert_eq!(g.aligned.len(), 3);
        assert!(g.aligned.values().all(|runs| runs.len() == 1 && runs.iter().all(|r| r.rows.len() == 1)));
        // A formula taken out of the middle splits the stretch.
        g.remove((0, CellRef::new(500, 6)));
        assert!(g.aligned.values().all(|runs| runs.iter().all(|r| r.rows.len() == 2)));
        let mut got = Vec::new();
        g.dependents(0, CellRef::new(500, 2), &mut got);
        assert!(got.is_empty());
        g.dependents(0, CellRef::new(501, 2), &mut got);
        assert_eq!(got, [(0, CellRef::new(501, 6))]);
    }

    #[test]
    fn a_column_read_from_many_distances_keeps_its_runs_bounded() {
        let mut g = Graph::default();
        for row in 0..1000u32 {
            let area = Area { sheet: 0, range: RangeRef::cell(CellRef::new(0, 1)) };
            g.insert((0, CellRef::new(row, 6)), Node::new(vec![area], false));
        }
        assert!(g.aligned.values().all(|runs| runs.len() <= MAX_RUNS));
        let mut got = Vec::new();
        g.dependents(0, CellRef::new(0, 1), &mut got);
        assert_eq!(got.len(), 1000);
    }
}
