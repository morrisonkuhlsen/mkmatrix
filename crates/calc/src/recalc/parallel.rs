//! Calculates what kernels can answer on all cores, ahead of the interpreter. Kernels are pure: they
//! read the workbook and nothing else, so a block of cells can run on any thread, in any order,
//! and give what one thread would. Formulas that read a formula still to calculate wait for the
//! next round, and what is left after the last round goes to the interpreter, on one thread.

use mkmatrix_core::{CellRef, Value};
use mkmatrix_model::{Sheet, Workbook};
use rayon::prelude::*;

use super::graph::Key;
use super::kernel::{Kernels, number};

/// Fewest formulas worth sending to other threads. `MKMATRIX_PARALLEL_MIN` overrides it, so the
/// command-sequence fuzz can run its small sheets through this path.
fn min_cells() -> usize {
    static MIN: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *MIN.get_or_init(|| std::env::var("MKMATRIX_PARALLEL_MIN").ok().and_then(|v| v.parse().ok()).unwrap_or(if cfg!(test) { 1 } else { 4096 }))
}
/// Formulas a thread takes at a time.
const CHUNK: usize = 4096;
/// Rounds at most: each one reaches a level further along chains of formulas.
const MAX_ROUNDS: usize = 4;

/// Whether kernels may read this sheet: no spill lies under its cells.
fn no_spills(sheet: &Sheet) -> bool {
    sheet.spill.is_empty() && sheet.spill_ranges.is_empty()
}

/// The kernel's answer for `k`, if it has one. `left` is what is still to calculate, sorted.
fn answer(sheet: &Sheet, si: usize, c: CellRef, left: &[Key], kernels: &mut Kernels) -> Option<f64> {
    let formula = sheet.cell(c)?.formula.as_ref()?;
    if formula.array.is_some() {
        return None;
    }
    let (tree, rows, cols) = formula.tree()?;
    let kernel = kernels.get(tree)?;
    kernel.run(rows, cols, |at| match sheet.cells.get(at) {
        None => Some(0.0),
        Some(cell) if cell.formula.is_none() => number(&cell.value),
        Some(_) if left.binary_search(&(si, at)).is_ok() => None,
        Some(cell) => number(&cell.value),
    })
}

/// Calculates and stores what kernels can answer among `dirty` (sorted); gives back the rest, still
/// sorted, and how many it calculated.
pub(super) fn run_kernels(wb: &mut Workbook, dirty: &[Key]) -> (Vec<Key>, usize) {
    let mut left = dirty.to_vec();
    if dirty.len() < min_cells() || wb.calc.iterative {
        return (left, 0);
    }
    let mut done = 0;
    for _round in 0..MAX_ROUNDS {
        let this_round = {
            let (wb, left) = (&*wb, left.as_slice());
            left.par_chunks(CHUNK)
                .map(|chunk| {
                    let mut kernels = Kernels::default();
                    let (mut answers, mut waiting) = (Vec::with_capacity(chunk.len()), Vec::new());
                    for &k in chunk {
                        let found = wb.sheet(k.0).filter(|s| no_spills(s)).and_then(|s| answer(s, k.0, k.1, left, &mut kernels));
                        match found {
                            Some(x) => answers.push((k, x)),
                            None => waiting.push(k),
                        }
                    }
                    (answers, waiting)
                })
                .collect::<Vec<_>>()
        };
        let mut calculated = 0;
        let mut waiting = Vec::with_capacity(left.len());
        for (answers, rest) in this_round {
            calculated += answers.len();
            waiting.extend(rest);
            for (k, x) in answers {
                let value = Value::number(x);
                let Some(sheet) = wb.sheet(k.0) else { continue };
                if sheet.cell(k.1).is_some_and(|c| c.value != value)
                    && let Some(cell) = wb.sheet_mut(k.0).and_then(|s| s.cells.get_mut(k.1))
                {
                    cell.value = value;
                }
            }
        }
        left = waiting;
        done += calculated;
        // A long chain moves a level per round: the interpreter's thread walks it faster.
        if calculated.saturating_mul(8) < left.len() || left.is_empty() {
            break;
        }
    }
    (left, done)
}

/// Sorts a pass's formulas in sheet order.
pub(super) fn sort_keys(keys: &mut [Key]) {
    keys.par_sort_unstable();
}
