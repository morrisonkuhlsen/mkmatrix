//! Where there are no threads (wasm), rows are read one after another by the caller.

use super::{El, RowState, Sheet};
use crate::read::Ctx;

pub(super) fn read_on_threads(_cx: &mut Ctx<'_>, _bytes: &[u8], _sheet: &mut Sheet, _st: &mut RowState) -> Option<El> {
    None
}
