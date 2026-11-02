//! Where there are no threads (wasm), the interpreter's thread calculates everything.

use mkmatrix_model::Workbook;

use super::graph::Key;

pub(super) fn run_kernels(_wb: &mut Workbook, dirty: &[Key]) -> (Vec<Key>, usize) {
    (dirty.to_vec(), 0)
}

/// Sorts a pass's formulas in sheet order.
pub(super) fn sort_keys(keys: &mut [Key]) {
    keys.sort_unstable();
}
