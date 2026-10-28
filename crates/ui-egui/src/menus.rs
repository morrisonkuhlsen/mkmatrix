//! The menu bar above MK Matrix's single-row toolbar: File, Edit, View, Insert, Format, Tools,
//! Data and Help, each a list of the commands the classic ribbon's tabs hold. The same tree as
//! the macOS menu bar (`apps/mkmatrix/src/native_menu.rs`); items run through [`run`].

use serde_json::{Value, json};

use crate::SheetApp;

/// (label, command or `dialog:name[:arg]` / `url:…`). `-` is a separator.
pub type Entry = (&'static str, &'static str);

/// The menus, in order.
pub fn tree() -> Vec<(&'static str, Vec<Entry>)> {
    vec![
        (
            "File",
            vec![
                ("New Workbook", "file.new"),
                ("New from Sample…", "dialog:start"),
                ("Open…", "file.open"),
                ("-", ""),
                ("Save", "file.save"),
                ("Save As…", "file.saveAs"),
                ("Export as PDF…", "file.exportPdf"),
                ("Export as CSV…", "dialog:saveCopy:csv"),
                ("-", ""),
                ("Page Setup…", "dialog:pageSetup"),
                ("Print…", "file.print"),
                ("-", ""),
                ("Properties…", "dialog:backstage:info"),
                ("Options…", "dialog:backstage:options"),
                ("Close", "file.close"),
            ],
        ),
        (
            "Edit",
            vec![
                ("Undo", "edit.undo"),
                ("Redo", "edit.redo"),
                ("-", ""),
                ("Cut", "edit.cut"),
                ("Copy", "edit.copy"),
                ("Paste", "edit.paste"),
                ("Paste Special…", "dialog:pasteSpecial"),
                ("-", ""),
                ("Fill Down", "edit.fillDown"),
                ("Fill Right", "edit.fillRight"),
                ("Series…", "dialog:series"),
                ("Flash Fill", "edit.flashFill"),
                ("-", ""),
                ("Clear All", "edit.clearAll"),
                ("Clear Formats", "edit.clearFormats"),
                ("Clear Contents", "edit.clearContents"),
                ("-", ""),
                ("Find…", "dialog:find"),
                ("Replace…", "dialog:find:replace"),
                ("Go To…", "dialog:goTo"),
            ],
        ),
        (
            "View",
            vec![
                ("Normal", "view.normal"),
                ("Page Layout", "view.pageLayout"),
                ("Page Break Preview", "view.pageBreakPreview"),
                ("-", ""),
                ("Formula Bar", "view.formulaBar"),
                ("Gridlines", "view.gridlines"),
                ("Headings", "view.headings"),
                ("Numbered Columns", "view.numberedColumns"),
                ("Matrix Brackets", "view.matrixBrackets"),
                ("Calculation by Hand", "view.byHand"),
                ("-", ""),
                ("Zoom…", "dialog:zoom"),
                ("Zoom to Selection", "view.zoomToSelection"),
                ("Freeze Panes", "view.freezePanes"),
                ("Unfreeze Panes", "view.unfreezePanes"),
                ("-", ""),
                ("Dark Mode", "view.darkMode"),
                ("Classic Ribbon", "view.classicRibbon"),
            ],
        ),
        (
            "Insert",
            vec![
                ("Cells…", "dialog:insertCells"),
                ("Rows", "home.insertRows"),
                ("Columns", "home.insertColumns"),
                ("Sheet", "home.insertSheet"),
                ("-", ""),
                ("Chart", "insert.recommendedCharts"),
                ("Sparklines…", "dialog:sparkline"),
                ("PivotTable", "insert.pivotTable"),
                ("Table", "insert.table"),
                ("-", ""),
                ("Function…", "dialog:insertFunction"),
                ("Name…", "dialog:defineName"),
                ("New Comment", "dialog:comment"),
                ("New Note", "dialog:note"),
                ("Picture…", "dialog:insertPicture"),
                ("Text Box", "insert.textBox"),
                ("Link…", "dialog:insertLink"),
                ("Checkbox", "insert.checkbox"),
            ],
        ),
        (
            "Format",
            vec![
                ("Cells…", "dialog:formatCells"),
                ("Row Height…", "dialog:rowHeight"),
                ("AutoFit Row Height", "home.autofitRowHeight"),
                ("Column Width…", "dialog:columnWidth"),
                ("AutoFit Column Width", "home.autofitColumnWidth"),
                ("-", ""),
                ("Hide Rows", "home.hideRows"),
                ("Unhide Rows", "home.unhideRows"),
                ("Hide Columns", "home.hideColumns"),
                ("Unhide Columns", "home.unhideColumns"),
                ("-", ""),
                ("Rename Sheet…", "dialog:renameSheet"),
                ("Hide Sheet", "sheet.hide"),
                ("Unhide Sheet…", "sheet.unhide"),
                ("-", ""),
                ("Conditional Formatting…", "dialog:manageRules"),
                ("Format as Table", "home.formatAsTable"),
            ],
        ),
        (
            "Data",
            vec![
                ("Sort…", "dialog:sort"),
                ("Sort A to Z", "data.sortAscending"),
                ("Sort Z to A", "data.sortDescending"),
                ("Filter", "data.filter"),
                ("Clear Filter", "data.clearFilter"),
                ("-", ""),
                ("Text to Columns…", "dialog:textToColumns"),
                ("Remove Duplicates…", "dialog:removeDuplicates"),
                ("Data Validation…", "dialog:dataValidation"),
                ("-", ""),
                ("Group", "data.group"),
                ("Ungroup", "data.ungroup"),
                ("Subtotal…", "dialog:subtotal"),
                ("-", ""),
                ("Calculate Now", "formulas.calculateNow"),
            ],
        ),
        (
            "Tools",
            vec![
                ("Spelling…", "dialog:spelling"),
                ("Workbook Statistics", "dialog:statistics"),
                ("-", ""),
                ("Protect Sheet…", "dialog:protectSheet"),
                ("Protect Workbook", "review.protectWorkbook"),
                ("-", ""),
                ("Goal Seek…", "dialog:goalSeek"),
                ("Error Checking…", "dialog:errorChecking"),
                ("Evaluate Formula…", "dialog:evaluateFormula"),
                ("-", ""),
                ("Record Actions", "automate.recordActions"),
                ("Command Palette…", "dialog:commandSearch"),
                ("Agent Control (MCP)…", "dialog:agents"),
            ],
        ),
        (
            "Help",
            vec![
                ("About MK Matrix", "dialog:about:About"),
                ("Check for Updates…", "app.checkForUpdates"),
                ("Morrison Kühlsen", "url:https://morrisonkuhlsen.com"),
            ],
        ),
    ]
}

/// Runs a menu item: opens its dialog, follows its link, uses the clipboard, or runs its command.
pub fn run(app: &mut SheetApp, ctx: &egui::Context, id: &str) {
    if let Some(url) = id.strip_prefix("url:") {
        if let Some(open) = &app.services.open_url {
            open(url);
        }
    } else if let Some(rest) = id.strip_prefix("dialog:") {
        let (name, arg) = rest.split_once(':').unwrap_or((rest, ""));
        let params: Value = match (name, arg) {
            ("saveCopy", fmt) => json!({"format": fmt}),
            ("find", "replace") => json!({"replace": true}),
            ("note", _) => json!({"threaded": false}),
            ("comment", _) => json!({"threaded": true}),
            ("about", tab) => json!({"tab": tab}),
            _ => json!({}),
        };
        match (name, arg) {
            ("backstage", page) => app.run_or_alert("ui.backstage", json!({"page": page})),
            _ => app.open_dialog(if name == "note" { "comment" } else { name }, params),
        }
    } else if id == "edit.copy" || id == "edit.cut" {
        app.copy_to_clipboard(ctx, id);
    } else if id == "edit.paste" {
        app.paste_from_clipboard();
    } else {
        app.run_or_alert(id, json!({}));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_item_names_a_command_or_a_dialog() {
        for (_, items) in super::tree() {
            for (label, id) in items {
                if label == "-" || id.starts_with("dialog:") || id.starts_with("url:") || id.starts_with("view.") || id.starts_with("app.") {
                    continue;
                }
                assert!(mkmatrix_engine::find_command(id).is_some(), "{label}: {id}");
            }
        }
    }
}
