//! Repeat (Excel's F4): the last edit again, on the current selection.

use serde_json::{Value as Json, json};

use crate::cmd::has_doc;
use crate::{EngineError, Result, Session};

/// The parameters that say where a command acts. Repeat drops them, so the command acts on the
/// current selection, as Excel's F4 does.
const REPEAT_PLACE_KEYS: [&str; 7] = ["range", "cell", "cells", "rows", "cols", "sheet", "target"];

/// The last edit that Repeat runs again: the newest journalled command that changes the workbook,
/// apart from Undo, Redo and Repeat themselves and moving the selection.
fn repeatable(s: &Session) -> Option<(String, Json)> {
    s.journal.iter().rev().find_map(|(id, p)| {
        let spec = crate::cmd::find_command(id)?;
        let skip = !spec.undoable || id.starts_with("selection.") || matches!(id.as_str(), "edit.undo" | "edit.redo" | "edit.repeat");
        (!skip).then(|| (id.clone(), p.clone()))
    })
}

pub(super) fn can_repeat(s: &Session) -> std::result::Result<(), String> {
    has_doc(s)?;
    repeatable(s).map(|_| ()).ok_or_else(|| "nothing to repeat".into())
}

/// Repeat (F4 outside a cell being edited): the last edit again, on the current selection.
pub(super) fn repeat(s: &mut Session, _: &Json) -> Result<Json> {
    let (id, mut params) = repeatable(s).ok_or_else(|| EngineError::Other("nothing to repeat".into()))?;
    if let Some(m) = params.as_object_mut() {
        for key in REPEAT_PLACE_KEYS {
            m.remove(key);
        }
    }
    s.execute(&id, params)?;
    Ok(json!({"repeated": id}))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Session;

    fn fill(s: &Session, cell: &str) -> Option<String> {
        let d = s.active()?;
        let sh = d.wb.active()?;
        let at = mkmatrix_core::CellRef::parse(cell)?;
        let st = d.wb.styles.get(sh.style_id(at));
        (st.fill != Default::default()).then(|| format!("{:?}", st.fill))
    }

    #[test]
    fn repeat_runs_the_last_edit_on_the_current_selection_and_undoes_in_one_step() {
        let mut s = Session::new();
        s.new_workbook();
        assert!(s.execute("edit.repeat", json!({})).is_err(), "nothing to repeat yet");
        s.execute("home.fillColor", json!({"range": "A1", "color": "#FFFF00"})).unwrap();
        // Moving the selection is not an edit: Repeat looks past it.
        s.execute("selection.set", json!({"range": "C3:D4"})).unwrap();
        let r = s.execute("edit.repeat", json!({})).unwrap();
        assert_eq!(r["repeated"], "home.fillColor");
        assert_eq!(fill(&s, "C3"), fill(&s, "A1"));
        assert_eq!(fill(&s, "D4"), fill(&s, "A1"));
        assert!(fill(&s, "B2").is_none());
        // Again, somewhere else: Repeat repeats the edit, not itself.
        s.execute("selection.set", json!({"range": "F6"})).unwrap();
        s.execute("edit.repeat", json!({})).unwrap();
        assert_eq!(fill(&s, "F6"), fill(&s, "A1"));
        // One Undo takes back one repeat.
        s.execute("edit.undo", json!({})).unwrap();
        assert!(fill(&s, "F6").is_none());
        assert!(fill(&s, "C3").is_some() && fill(&s, "A1").is_some());
    }

    #[test]
    fn repeat_types_the_last_entry_into_the_active_cell() {
        let mut s = Session::new();
        s.new_workbook();
        s.execute("cell.set", json!({"cell": "A1", "input": "=1+1"})).unwrap();
        s.execute("selection.set", json!({"range": "B5"})).unwrap();
        s.execute("edit.repeat", json!({})).unwrap();
        let d = s.active().unwrap();
        let sh = d.wb.active().unwrap();
        assert_eq!(sh.value(mkmatrix_core::CellRef::parse("B5").unwrap()).display(), "2");
    }
}
