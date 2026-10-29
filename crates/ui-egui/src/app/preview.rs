//! The preview of the command under the pointer: what it would change on the active sheet, for
//! the grid to draw faded before the click.

use super::*;

impl SheetApp {
    /// The cells the command under the pointer would change, with their new text (empty when
    /// nothing is being previewed).
    #[doc(hidden)]
    pub fn preview_cells(&self) -> &[(mkmatrix_engine::core::CellRef, String)] {
        self.preview.as_ref().map_or(&[], |(_, cells)| cells.as_slice())
    }

    /// Computes (or keeps) the preview of the command under the pointer, for the grid to draw.
    pub(super) fn update_preview(&mut self) {
        let Some((id, params)) = self.preview_hover.clone() else {
            self.preview = None;
            return;
        };
        let Some(d) = self.session.active() else { return };
        let key = (id.to_string(), params.to_string(), d.uid, d.revision, d.wb.active_sheet());
        if self.preview.as_ref().is_some_and(|(k, _)| *k == key) {
            return;
        }
        let cells = self.session.preview(id, params).unwrap_or_default();
        self.preview = Some((key, cells));
    }
}
