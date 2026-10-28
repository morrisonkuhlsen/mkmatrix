use super::*;

pub(super) fn insert_sheet(s: &mut Session, p: &Json) -> Result<Json> {
    // The sheets' indices change: a group of them would point at others.
    s.doc_mut()?.sheet_group.clear();
    if s.doc()?.wb.protected_structure {
        return Err(EngineError::Other("The workbook is protected and cannot be changed.".into()));
    }
    let name = match str_param(p, "name") {
        Some(n) => n.to_string(),
        None => s.doc()?.wb.next_sheet_name(),
    };
    s.doc()?.wb.check_sheet_name(&name, None).map_err(EngineError::Other)?;
    let d = s.doc_mut()?;
    let before = p.get("before").and_then(Json::as_u64).map(|v| v as usize);
    let idx = edit_doc(d, |cx| {
        // Excel inserts before the active sheet; the "+" button appends after it.
        let at = before.unwrap_or(cx.wb.active_sheet() + 1).min(cx.wb.sheets.len());
        cx.wb.sheets.insert(at, Arc::new(Sheet::new(name.clone())));
        // Names scoped to later sheets shift.
        for n in cx.wb.names.iter_mut() {
            if let Some(sc) = n.scope.as_mut()
                && *sc >= at
            {
                *sc += 1;
            }
        }
        cx.wb.set_active_sheet(at);
        *cx.sel = crate::Selection::default();
        cx.structural = true;
        Ok(at)
    })?;
    d.sheet_selections.insert(idx.min(d.sheet_selections.len()), crate::Selection::default());
    Ok(json!({"index": idx, "name": name}))
}

fn edit_doc<R>(d: &mut crate::DocState, f: impl FnOnce(&mut Ctx) -> Result<R>) -> Result<R> {
    crate::cmd::commit(d, f)
}

pub(super) fn delete_sheet(s: &mut Session, p: &Json) -> Result<Json> {
    // The sheets' indices change: a group of them would point at others.
    s.doc_mut()?.sheet_group.clear();
    let i = target_sheet(s, p)?;
    let d = s.doc()?;
    if d.wb.protected_structure {
        return Err(EngineError::Other("The workbook is protected and cannot be changed.".into()));
    }
    if d.wb.sheets.iter().filter(|s| s.visibility == Visibility::Visible).count() <= 1 {
        return Err(EngineError::Other("A workbook must contain at least one visible worksheet.".into()));
    }
    let name = d.wb.sheet(i).map(|s| s.name.clone()).unwrap_or_default();
    let order: Vec<String> = d.wb.sheets.iter().map(|s| s.name.clone()).collect();
    let d = s.doc_mut()?;
    edit_doc(d, |cx| {
        cx.wb.sheets.remove(i);
        for si in 0..cx.wb.sheets.len() {
            let keys: Vec<(CellRef, Arc<mkmatrix_model::Formula>)> =
                cx.wb.sheets.get(si).map(|s| s.cells.iter().filter_map(|(c, x)| x.formula.clone().map(|f| (c, f))).collect()).unwrap_or_default();
            let Some(sh) = cx.wb.sheet_mut(si) else { continue };
            for (c, f) in keys {
                if let Some(e) = f.expr() {
                    let ne = mkmatrix_formula::adjust::delete_sheet(e, &name, &order);
                    if let Some(cell) = sh.cells.get_mut(c) {
                        cell.formula = Some(Arc::new(mkmatrix_model::Formula::from_expr(ne)));
                    }
                }
            }
        }
        cx.wb.names.retain(|n| n.scope != Some(i));
        for n in cx.wb.names.iter_mut() {
            if let Some(sc) = n.scope.as_mut()
                && *sc > i
            {
                *sc -= 1;
            }
            // Names referring to the sheet become #REF!.
            if let Ok(e) = mkmatrix_formula::parse_name(&n.formula) {
                let ne = mkmatrix_formula::adjust::delete_sheet(e.clone(), &name, &order);
                if ne != e {
                    n.formula = mkmatrix_formula::print(&ne);
                }
            }
        }
        if cx.wb.active_sheet() >= cx.wb.sheets.len() || cx.wb.active_sheet() > i {
            cx.wb.set_active_sheet(cx.wb.active_sheet().saturating_sub(1).min(cx.wb.sheets.len().saturating_sub(1)));
        }
        *cx.sel = crate::Selection::default();
        cx.structural = true;
        Ok(())
    })?;
    if i < d.sheet_selections.len() {
        d.sheet_selections.remove(i);
    }
    ok()
}

pub(super) fn rename_sheet(s: &mut Session, p: &Json) -> Result<Json> {
    if s.doc()?.wb.protected_structure {
        return Err(EngineError::Other("The workbook is protected and cannot be changed.".into()));
    }
    let i = target_sheet(s, p)?;
    let Some(name) = str_param(p, "name").map(|n| n.trim().to_string()) else { return Err(bad("sheet.rename", "missing `name`")) };
    s.doc()?.wb.check_sheet_name(&name, Some(i)).map_err(EngineError::Other)?;
    edit(s, |cx| {
        let old = cx.wb.sheet(i).map(|s| s.name.clone()).unwrap_or_default();
        cx.sheet_mut(i)?.name = name.clone();
        for si in 0..cx.wb.sheets.len() {
            let keys: Vec<(CellRef, Arc<mkmatrix_model::Formula>)> =
                cx.wb.sheets.get(si).map(|s| s.cells.iter().filter_map(|(c, x)| x.formula.clone().map(|f| (c, f))).collect()).unwrap_or_default();
            let Some(sh) = cx.wb.sheet_mut(si) else { continue };
            for (c, f) in keys {
                if let Some(e) = f.expr() {
                    let ne = mkmatrix_formula::adjust::rename_sheet(e, &old, &name);
                    let text = mkmatrix_formula::print(&ne);
                    if text != f.text
                        && let Some(cell) = sh.cells.get_mut(c)
                    {
                        cell.formula = Some(Arc::new(mkmatrix_model::Formula::from_expr(ne)));
                    }
                }
            }
            let fix = |t: &str| {
                mkmatrix_formula::parse(t)
                    .map(|e| mkmatrix_formula::print(&mkmatrix_formula::adjust::rename_sheet(e, &old, &name)))
                    .unwrap_or_else(|_| t.to_string())
            };
            for ch in sh.charts.iter_mut() {
                for se in ch.series.iter_mut() {
                    se.values = fix(&se.values);
                    se.categories = se.categories.as_deref().map(fix);
                    se.name = se.name.as_deref().map(fix);
                }
                // A PivotChart follows its PivotTable's sheet.
                if let Some((sheet, pivot)) = ch.pivot.as_deref().and_then(crate::pivot::parse_chart_link)
                    && sheet.eq_ignore_ascii_case(&old)
                {
                    ch.pivot = Some(crate::pivot::chart_link(&name, &pivot));
                }
            }
        }
        for n in cx.wb.names.iter_mut() {
            if let Ok(e) = mkmatrix_formula::parse_name(&n.formula) {
                n.formula = mkmatrix_formula::print(&mkmatrix_formula::adjust::rename_sheet(e, &old, &name));
            }
        }
        cx.structural = true;
        Ok(Json::Null)
    })
}

pub(super) fn move_sheet(s: &mut Session, p: &Json) -> Result<Json> {
    // The sheets' indices change: a group of them would point at others.
    s.doc_mut()?.sheet_group.clear();
    if s.doc()?.wb.protected_structure {
        return Err(EngineError::Other("The workbook is protected and cannot be changed.".into()));
    }
    let i = target_sheet(s, p)?;
    let copy = bool_param(p, "copy").unwrap_or(false);
    let to = p.get("to").and_then(Json::as_u64).map(|v| v as usize).ok_or_else(|| bad("sheet.move", "missing `to`"))?;
    edit(s, |cx| {
        let n = cx.wb.sheets.len();
        let Some(src) = cx.wb.sheets.get(i).cloned() else { return Err(bad("sheet.move", "no such sheet")) };
        if copy {
            let mut sh = (*src).clone();
            let base = sh.name.clone();
            // `Name (2)`: the name is shortened so the suffix fits in Excel's 31 characters.
            let name = (2..)
                .map(|k| {
                    let suffix = format!(" ({k})");
                    let keep = 31usize.saturating_sub(suffix.chars().count());
                    format!("{}{suffix}", base.chars().take(keep).collect::<String>().trim_end())
                })
                .find(|nm| cx.wb.sheet_index(nm).is_none())
                .unwrap_or(base.clone());
            sh.name = name;
            sh.tables.clear(); // table names must be unique
            sh.preserved = None; // kept file content (form controls, OLE objects…) stays with the original
            let at = to.min(n);
            // The copy gets its own copies of the sheet's names, referring to the copy.
            let copies: Vec<mkmatrix_model::DefinedName> = cx
                .wb
                .names
                .iter()
                .filter(|nm| nm.scope == Some(i))
                .map(|nm| {
                    let formula = mkmatrix_formula::parse_name(&nm.formula)
                        .map(|e| mkmatrix_formula::print(&mkmatrix_formula::adjust::rename_sheet(e, &base, &sh.name)))
                        .unwrap_or_else(|_| nm.formula.clone());
                    mkmatrix_model::DefinedName { scope: Some(at), formula, ..nm.clone() }
                })
                .collect();
            for nm in cx.wb.names.iter_mut() {
                if let Some(sc) = nm.scope.as_mut()
                    && *sc >= at
                {
                    *sc += 1;
                }
            }
            cx.wb.names.extend(copies);
            cx.wb.sheets.insert(at, Arc::new(sh));
            cx.wb.set_active_sheet(at);
        } else {
            let sh = cx.wb.sheets.remove(i);
            let at = to.min(n - 1);
            cx.wb.sheets.insert(at, sh);
            cx.wb.set_active_sheet(at);
            // Sheet-level names follow their sheet.
            for nm in cx.wb.names.iter_mut() {
                if let Some(sc) = nm.scope.as_mut() {
                    *sc = match *sc {
                        x if x == i => at,
                        x if i < x && x <= at => x - 1,
                        x if at <= x && x < i => x + 1,
                        x => x,
                    };
                }
            }
        }
        cx.structural = true;
        Ok(Json::Null)
    })
}

pub(super) fn tab_color(s: &mut Session, p: &Json) -> Result<Json> {
    let i = target_sheet(s, p)?;
    let c = crate::cmd::format::color_param(p.get("color"));
    edit(s, |cx| {
        cx.sheet_mut(i)?.tab_color = c.filter(|c| *c != mkmatrix_model::Color::Auto);
        Ok(Json::Null)
    })
}

pub(super) fn hide_sheet(s: &mut Session, p: &Json) -> Result<Json> {
    if s.doc()?.wb.protected_structure {
        return Err(EngineError::Other("The workbook is protected and cannot be changed.".into()));
    }
    let i = target_sheet(s, p)?;
    if s.doc()?.wb.sheets.iter().filter(|s| s.visibility == Visibility::Visible).count() <= 1 {
        return Err(EngineError::Other("A workbook must contain at least one visible worksheet.".into()));
    }
    edit(s, |cx| {
        cx.sheet_mut(i)?.visibility = Visibility::Hidden;
        if cx.wb.active_sheet() == i {
            cx.wb.set_active_sheet(cx.wb.sheets.iter().position(|s| s.visibility == Visibility::Visible).unwrap_or(0));
        }
        Ok(Json::Null)
    })
}

pub(super) fn unhide_sheet(s: &mut Session, p: &Json) -> Result<Json> {
    // The sheets' indices change: a group of them would point at others.
    s.doc_mut()?.sheet_group.clear();
    if s.doc()?.wb.protected_structure {
        return Err(EngineError::Other("The workbook is protected and cannot be changed.".into()));
    }
    if p.get("sheet").is_none() {
        let hidden: Vec<String> = s.doc()?.wb.sheets.iter().filter(|s| s.visibility != Visibility::Visible).map(|s| s.name.clone()).collect();
        if hidden.is_empty() {
            return Err(EngineError::Other("No sheets are hidden.".into()));
        }
        s.ui_requests.push(crate::UiRequest::Dialog("unhideSheet".into(), json!({"sheets": hidden})));
        return ok();
    }
    let i = target_sheet(s, p)?;
    edit(s, |cx| {
        cx.sheet_mut(i)?.visibility = Visibility::Visible;
        cx.wb.set_active_sheet(i);
        Ok(Json::Null)
    })
}

pub(super) fn activate(s: &mut Session, p: &Json) -> Result<Json> {
    let i = target_sheet(s, p)?;
    let d = s.doc_mut()?;
    // A sheet of the group becomes the active one and the group stays, unless every sheet is in
    // it (then, as in Excel, a click ungroups); any other sheet ungroups.
    if !d.sheet_group.contains(&i) || d.sheet_group.len() >= visible_sheets(d).len() {
        d.sheet_group.clear();
    }
    d.wb_switch_sheet(i);
    Ok(json!({"sheet": i, "name": d.wb.active().map(|s| s.name.clone())}))
}

pub(super) fn step_sheet(s: &mut Session, dir: isize) -> Result<Json> {
    let d = s.doc()?;
    let mut i = d.wb.active_sheet();
    while let Some(next) = i.checked_add_signed(dir) {
        let Some(sheet) = d.wb.sheet(next) else { break };
        if sheet.visibility == Visibility::Visible {
            s.doc_mut()?.wb_switch_sheet(next);
            return ok();
        }
        i = next;
    }
    // No visible sheet in this direction: keep the current sheet and its selection.
    ok()
}

// ------------------------------------------------------------------ sheets selected together

/// The visible sheets' indices.
fn visible_sheets(d: &crate::DocState) -> Vec<usize> {
    d.wb.sheets.iter().enumerate().filter(|(_, s)| s.visibility == Visibility::Visible).map(|(i, _)| i).collect()
}

/// The sheets selected: the group, or the active sheet alone.
fn selected_sheets(d: &crate::DocState) -> Json {
    if d.sheet_group.len() > 1 { json!(d.sheet_group) } else { json!([d.wb.active_sheet()]) }
}

/// Ctrl+click on a tab (`add`) toggles the sheet in the group and makes it active; Shift+click
/// (`extend`) selects every visible sheet from the active one to it, the active one staying
/// active; a plain click selects the sheet alone.
pub(super) fn select_sheet(s: &mut Session, p: &Json) -> Result<Json> {
    let i = target_sheet(s, p)?;
    let add = bool_param(p, "add").unwrap_or(false);
    let extend = bool_param(p, "extend").unwrap_or(false);
    let d = s.doc_mut()?;
    if d.wb.sheet(i).is_none_or(|sh| sh.visibility != Visibility::Visible) {
        return Err(bad("sheet", "a hidden sheet can't be selected"));
    }
    let active = d.wb.active_sheet();
    let visible = visible_sheets(d);
    if extend {
        let (from, to) = (active.min(i), active.max(i));
        d.sheet_group = visible.into_iter().filter(|x| (from..=to).contains(x)).collect();
    } else if add {
        let mut group = if d.sheet_group.len() > 1 { d.sheet_group.clone() } else { vec![active] };
        match group.iter().position(|&x| x == i) {
            // The last sheet of a group stays selected.
            Some(_) if group.len() == 1 => {}
            Some(at) => {
                group.remove(at);
                if i == active
                    && let Some(&next) = group.first()
                {
                    d.wb_switch_sheet(next);
                }
            }
            None => {
                group.push(i);
                group.sort_unstable();
                d.wb_switch_sheet(i);
            }
        }
        d.sheet_group = group;
    } else {
        d.sheet_group.clear();
        d.wb_switch_sheet(i);
    }
    if d.sheet_group.len() < 2 {
        d.sheet_group.clear();
    }
    Ok(json!({"sheets": selected_sheets(d)}))
}

/// Select All Sheets: every visible sheet, the active one staying active.
pub(super) fn select_all_sheets(s: &mut Session, _: &Json) -> Result<Json> {
    let d = s.doc_mut()?;
    let visible = visible_sheets(d);
    d.sheet_group = if visible.len() > 1 { visible } else { vec![] };
    Ok(json!({"sheets": selected_sheets(d)}))
}

/// Ungroup Sheets: the active sheet alone.
pub(super) fn ungroup_sheets(s: &mut Session, _: &Json) -> Result<Json> {
    let d = s.doc_mut()?;
    d.sheet_group.clear();
    Ok(json!({"sheets": selected_sheets(d)}))
}

/// How a command runs over the sheets selected together.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum GroupFanOut {
    /// An edit to cells: run on each sheet in turn, with the active sheet's selection.
    Cells(Vec<usize>),
    /// A command on sheets themselves (Delete, Hide, Tab Color): run once per sheet, by index.
    Sheets(Vec<usize>),
}

/// Whether `id` with `p` goes to every sheet selected together, and how. Only a top-level call
/// that names no sheet of its own (no `sheet`, no `Sheet!` reference) does; reading, moving the
/// selection and the sheet's structure (insert, move, rename) stay with the active sheet.
pub(crate) fn fan_out(s: &Session, id: &str, p: &Json) -> Option<GroupFanOut> {
    let d = s.active()?;
    if d.sheet_group.len() < 2 || p.get("sheet").is_some() {
        return None;
    }
    let names_sheet = ["range", "cell", "cells", "target"].iter().any(|k| p.get(*k).and_then(Json::as_str).is_some_and(|r| r.contains('!')));
    if names_sheet {
        return None;
    }
    let group = d.sheet_group.clone();
    match id {
        "home.deleteSheet" | "sheet.hide" | "sheet.tabColor" => Some(GroupFanOut::Sheets(group)),
        "home.insertSheet" | "sheet.rename" | "sheet.move" | "sheet.unhide" => None,
        _ if id.starts_with("cell.") || id.starts_with("range.") || id.starts_with("home.") => Some(GroupFanOut::Cells(group)),
        "edit.clearAll"
        | "edit.clearContents"
        | "edit.clearFormats"
        | "edit.clearComments"
        | "edit.clearHyperlinks"
        | "edit.paste"
        | "edit.pasteSpecial" => Some(GroupFanOut::Cells(group)),
        _ if id.starts_with("edit.fill") => Some(GroupFanOut::Cells(group)),
        _ => None,
    }
}

#[cfg(test)]
mod group_tests {
    use serde_json::json;

    use crate::Session;

    /// A workbook of four sheets, the first active.
    fn four() -> Session {
        let mut s = Session::new();
        s.new_workbook();
        for name in ["Two", "Three", "Four"] {
            s.execute("home.insertSheet", json!({"name": name})).unwrap();
        }
        s.execute("sheet.activate", json!({"sheet": 0})).unwrap();
        s
    }

    fn group(s: &Session) -> Vec<usize> {
        s.active().unwrap().sheet_group.clone()
    }

    fn text(s: &Session, sheet: usize, cell: &str) -> String {
        let d = s.active().unwrap();
        d.wb.sheet(sheet).unwrap().value(mkmatrix_core::CellRef::parse(cell).unwrap()).display()
    }

    #[test]
    fn ctrl_and_shift_click_select_sheets_and_a_plain_click_ungroups() {
        let mut s = four();
        s.execute("sheet.select", json!({"sheet": 2, "add": true})).unwrap();
        assert_eq!(group(&s), vec![0, 2]);
        assert_eq!(s.active().unwrap().wb.active_sheet(), 2, "Ctrl+click makes the sheet active");
        s.execute("sheet.select", json!({"sheet": 2, "add": true})).unwrap();
        assert!(group(&s).is_empty(), "toggled off, one sheet is left: no group");
        assert_eq!(s.active().unwrap().wb.active_sheet(), 0);
        s.execute("sheet.select", json!({"sheet": 2, "extend": true})).unwrap();
        assert_eq!(group(&s), vec![0, 1, 2]);
        assert_eq!(s.active().unwrap().wb.active_sheet(), 0, "Shift+click keeps the active sheet");
        // A sheet of the group becomes active and the group stays.
        s.execute("sheet.activate", json!({"sheet": 1})).unwrap();
        assert_eq!(group(&s), vec![0, 1, 2]);
        // One outside it ungroups.
        s.execute("sheet.activate", json!({"sheet": 3})).unwrap();
        assert!(group(&s).is_empty());
        s.execute("sheet.selectAll", json!({})).unwrap();
        assert_eq!(group(&s), vec![0, 1, 2, 3]);
        // With every sheet in the group, a click ungroups, as in Excel.
        s.execute("sheet.activate", json!({"sheet": 1})).unwrap();
        assert!(group(&s).is_empty());
        s.execute("sheet.selectAll", json!({})).unwrap();
        s.execute("sheet.ungroup", json!({})).unwrap();
        assert!(group(&s).is_empty());
    }

    #[test]
    fn an_edit_goes_to_every_sheet_of_the_group_and_undoes_in_one_step() {
        let mut s = four();
        s.execute("sheet.select", json!({"sheet": 2, "extend": true})).unwrap();
        s.execute("selection.set", json!({"range": "B2"})).unwrap();
        s.execute("cell.set", json!({"input": "=6*7"})).unwrap();
        for sheet in 0..3 {
            assert_eq!(text(&s, sheet, "B2"), "42", "sheet {sheet}");
        }
        assert_eq!(text(&s, 3, "B2"), "", "a sheet outside the group is left alone");
        assert_eq!(s.active().unwrap().wb.active_sheet(), 0, "the active sheet stays active");
        s.execute("edit.undo", json!({})).unwrap();
        for sheet in 0..3 {
            assert_eq!(text(&s, sheet, "B2"), "", "one Undo takes it back on sheet {sheet}");
        }
        // A call that names its sheet goes to that sheet alone.
        s.execute("cell.set", json!({"cell": "Four!C3", "input": "x"})).unwrap();
        assert_eq!(text(&s, 3, "C3"), "x");
        assert_eq!(text(&s, 0, "C3"), "");
    }

    #[test]
    fn delete_hide_and_tab_color_act_on_the_whole_group() {
        let mut s = four();
        s.execute("sheet.select", json!({"sheet": 3, "add": true})).unwrap();
        s.execute("sheet.tabColor", json!({"color": "#FF0000"})).unwrap();
        let d = s.active().unwrap();
        assert!(d.wb.sheet(0).unwrap().tab_color.is_some() && d.wb.sheet(3).unwrap().tab_color.is_some());
        assert!(d.wb.sheet(1).unwrap().tab_color.is_none());
        s.execute("home.deleteSheet", json!({})).unwrap();
        let names: Vec<String> = s.active().unwrap().wb.sheets.iter().map(|s| s.name.clone()).collect();
        assert_eq!(names, ["Two", "Three"]);
        assert!(group(&s).is_empty());
        // A group of every visible sheet can't be deleted or hidden.
        s.execute("sheet.selectAll", json!({})).unwrap();
        assert!(s.execute("home.deleteSheet", json!({})).is_err());
        assert!(s.execute("sheet.hide", json!({})).is_err());
        assert_eq!(s.active().unwrap().wb.sheets.len(), 2);
    }
}
