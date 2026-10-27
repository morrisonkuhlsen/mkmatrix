//! Sample workbooks built in code (original content), for the start screen, demos, README
//! screenshots and tests.

use std::sync::Arc;

use mkmatrix_model::{Sheet, Workbook};
use serde_json::json;

use crate::{DocState, Session};

pub const SAMPLES: &[(&str, &str)] = &[
    ("regression", "Linear Regression by Matrices"),
    ("budget", "Household Budget"),
    ("sales", "Quarterly Sales Dashboard"),
    ("grades", "Gradebook"),
];

pub fn title(name: &str) -> String {
    SAMPLES.iter().find(|(n, _)| *n == name).map(|(_, t)| t.to_string()).unwrap_or_else(|| "Sample".into())
}

/// Builds a sample by replaying commands (so samples also exercise the command layer).
pub fn build(name: &str) -> Option<Workbook> {
    let mut s = Session::new();
    let mut wb = Workbook::new();
    if let Some(sh) = wb.sheet_mut(0) {
        sh.name = match name {
            "regression" => "Regression".into(),
            "budget" => "Budget".into(),
            "sales" => "Sales".into(),
            "grades" => "Grades".into(),
            _ => return None,
        };
    }
    s.add_document(DocState::new(wb, None, title(name), crate::Trust::Local));
    let ok = match name {
        "regression" => regression(&mut s),
        "budget" => budget(&mut s),
        "sales" => sales(&mut s),
        "grades" => grades(&mut s),
        _ => false,
    };
    if !ok {
        return None;
    }
    let mut wb = (*s.doc().ok()?.wb).clone();
    wb.set_active_sheet(0);
    if let Some(sh) = wb.sheet_mut(0) {
        sh.view_active = mkmatrix_core::CellRef::default();
    }
    Some(wb)
}

fn run(s: &mut Session, id: &str, p: serde_json::Value) -> bool {
    match s.execute(id, p) {
        Ok(_) => true,
        Err(e) => {
            log::warn!("sample: {id} failed: {e}");
            false
        }
    }
}

/// MK Matrix's own sample: ordinary least squares worked as matrices, each step a block on the
/// sheet: the design matrix X and y, X'X, its inverse, X'y, then beta = (X'X)^-1 X'y, the fitted
/// values and the residuals, all as spilled MX. array formulas.
fn regression(s: &mut Session) -> bool {
    // Eight students: an intercept column, hours studied, hours slept; and the exam score.
    let x = json!([["const", "Hours", "Sleep"], [1, 2, 8], [1, 3, 6], [1, 4, 7], [1, 5, 5], [1, 6, 8], [1, 7, 6], [1, 8, 7], [1, 9, 8]]);
    let y = json!([["Score"], [73], [75], [79], [82], [88], [91], [95], [99]]);
    let mut ok = run(s, "range.setValues", json!({"range": "B2", "values": [["Linear Regression by Matrices"]]}));
    ok &= run(s, "home.cellStyle", json!({"range": "B2", "name": "Title"}));
    ok &= run(
        s,
        "range.setValues",
        json!({"range": "B3", "values": [["beta = (X'X)^-1 X'y, every step a block: X' transposes, @ multiplies, \\ solves. Change a score and watch it recalculate."]]}),
    );
    ok &= run(s, "home.cellStyle", json!({"range": "B3", "name": "Explanatory Text"}));
    // Labels over each block.
    for (cell, label) in [
        ("B5", "X (design matrix)"),
        ("F5", "y"),
        ("G5", "X beta"),
        ("H5", "y - X beta"),
        ("J5", "X'X"),
        ("J10", "(X'X)^-1"),
        ("O5", "X'y"),
        ("N10", "beta"),
        ("B16", "Fit"),
    ] {
        ok &= run(s, "range.setValues", json!({"range": cell, "values": [[label]]}));
        ok &= run(s, "home.bold", json!({"range": cell, "on": true}));
    }
    ok &= run(s, "range.setValues", json!({"range": "B6", "values": x}));
    ok &= run(s, "range.setValues", json!({"range": "F6", "values": y}));
    ok &= run(s, "range.setValues", json!({"range": "G6", "values": [["y-hat"]]}));
    ok &= run(s, "range.setValues", json!({"range": "H6", "values": [["e"]]}));
    // The matrices, as spilled array formulas.
    ok &= run(s, "cell.set", json!({"cell": "J6", "input": "=B7:D14'@B7:D14"}));
    ok &= run(s, "cell.set", json!({"cell": "J11", "input": "=MX.INV(J6:L8)"}));
    ok &= run(s, "cell.set", json!({"cell": "O6", "input": "=B7:D14'@F7:F14"}));
    ok &= run(s, "range.setValues", json!({"range": "M11", "values": [["Intercept"], ["Hours"], ["Sleep"]]}));
    ok &= run(s, "cell.set", json!({"cell": "N11", "input": "=J11:L13@O6:O8"}));
    ok &= run(s, "cell.set", json!({"cell": "G7", "input": "=B7:D14@N11:N13"}));
    // The same beta in one step: left division solves the least-squares problem directly.
    ok &= run(s, "range.setValues", json!({"range": "Q10", "values": [["X\\y"]]}));
    ok &= run(s, "home.bold", json!({"range": "Q10", "on": true}));
    ok &= run(s, "cell.set", json!({"cell": "Q11", "input": "=B7:D14\\F7:F14"}));
    ok &= run(s, "home.numberFormat", json!({"range": "Q11:Q13", "code": "0.000"}));
    ok &= run(s, "home.fillColor", json!({"range": "Q11:Q13", "color": "#B8F2E2"}));
    ok &= run(s, "home.borders", json!({"range": "Q11:Q13", "preset": "outside"}));
    ok &= run(s, "cell.set", json!({"cell": "H7", "input": "=F7:F14-G7:G14"}));
    // How good the fit is.
    ok &= run(s, "range.setValues", json!({"range": "B17", "values": [["R squared"], ["RSS"], ["det(X'X)"]]}));
    ok &= run(s, "cell.set", json!({"cell": "D17", "input": "=1-SUMSQ(H7:H14)/DEVSQ(F7:F14)"}));
    ok &= run(s, "cell.set", json!({"cell": "D18", "input": "=SUMSQ(H7:H14)"}));
    ok &= run(s, "cell.set", json!({"cell": "D19", "input": "=MX.DET(J6:L8)"}));
    // Each block shaded and boxed, the headers bold.
    for block in ["B7:D14", "F7:F14", "G7:H14", "J6:L8", "J11:L13", "O6:O8", "N11:N13"] {
        ok &= run(s, "home.fillColor", json!({"range": block, "color": "#DCF4EE"}));
        ok &= run(s, "home.borders", json!({"range": block, "preset": "outside"}));
    }
    ok &= run(s, "home.fillColor", json!({"range": "N11:N13", "color": "#B8F2E2"}));
    ok &= run(s, "home.bold", json!({"range": "B6:H6", "on": true}));
    ok &= run(s, "home.alignCenter", json!({"range": "B6:H6"}));
    ok &= run(s, "home.italic", json!({"range": "M11:M13", "on": true}));
    ok &= run(s, "home.numberFormat", json!({"range": "G7:H14", "code": "0.00"}));
    ok &= run(s, "home.numberFormat", json!({"range": "J11:L13", "code": "0.0000"}));
    ok &= run(s, "home.numberFormat", json!({"range": "N11:N13", "code": "0.000"}));
    ok &= run(s, "home.numberFormat", json!({"range": "D17", "code": "0.0000"}));
    ok &= run(s, "home.numberFormat", json!({"range": "D18", "code": "0.00"}));
    ok &= run(s, "home.columnWidth", json!({"cols": "A:A", "width": 20}));
    ok &= run(s, "home.columnWidth", json!({"cols": "B:D", "width": 64}));
    ok &= run(s, "home.columnWidth", json!({"cols": "E:E", "width": 20}));
    ok &= run(s, "home.columnWidth", json!({"cols": "F:H", "width": 90}));
    ok &= run(s, "home.columnWidth", json!({"cols": "I:I", "width": 20}));
    ok &= run(s, "home.columnWidth", json!({"cols": "J:L", "width": 74}));
    ok &= run(s, "home.columnWidth", json!({"cols": "M:M", "width": 70}));
    ok &= run(s, "home.columnWidth", json!({"cols": "N:O", "width": 74}));
    ok &= run(s, "review.newNote", json!({"cell": "N10", "text": "beta = (X'X)^-1 X'y: the inverse @ X'y. Q11 gets it in one step, X\\y."}));
    // The fit: actual and fitted scores against hours studied.
    ok &= run(
        s,
        "insert.chart",
        json!({"range": "F6:G14", "type": "line", "title": "Score: actual and fitted", "at": "B21", "width": 520, "height": 260}),
    );
    ok &= run(s, "selection.set", json!({"cell": "F7"}));
    ok
}

fn sales(s: &mut Session) -> bool {
    let rows = json!([
        ["Region", "Q1", "Q2", "Q3", "Q4", "Total", "Growth", "Trend"],
        ["North", 48200, 51340, 56010, 61870],
        ["South", 39150, 37980, 42400, 45120],
        ["East", 55300, 58910, 57230, 63480],
        ["West", 61020, 64400, 70150, 76390],
        ["Central", 28740, 31260, 33980, 36510],
        ["Online", 72410, 81630, 95220, 112840],
    ]);
    let mut ok = run(s, "range.setValues", json!({"range": "B4", "values": rows}));
    ok &= run(s, "range.setValues", json!({"range": "B2", "values": [["Quarterly Sales Dashboard"]]}));
    ok &= run(s, "home.cellStyle", json!({"range": "B2", "name": "Title"}));
    ok &= run(s, "range.setValues", json!({"range": "B3", "values": [["Fiscal year 2026 · all figures in USD"]]}));
    ok &= run(s, "home.cellStyle", json!({"range": "B3", "name": "Explanatory Text"}));
    for r in 5..=10 {
        ok &= run(s, "cell.set", json!({"cell": format!("G{r}"), "input": format!("=SUM(C{r}:F{r})")}));
        ok &= run(s, "cell.set", json!({"cell": format!("H{r}"), "input": format!("=F{r}/C{r}-1")}));
        ok &= run(s, "insert.sparkline", json!({"range": format!("C{r}:F{r}"), "location": format!("I{r}"), "type": "line", "markers": true}));
    }
    ok &= run(s, "insert.table", json!({"range": "B4:I10", "style": "TableStyleMedium2", "name": "Sales"}));
    ok &= run(s, "table.totalRow", json!({"table": "Sales", "on": true}));
    for col in ["Q1", "Q2", "Q3", "Q4", "Total"] {
        ok &= run(s, "table.totalFunction", json!({"table": "Sales", "column": col, "function": "sum"}));
    }
    ok &= run(s, "table.totalFunction", json!({"table": "Sales", "column": "Growth", "function": "average"}));
    ok &= run(s, "table.totalFunction", json!({"table": "Sales", "column": "Trend", "function": "none"}));
    ok &= run(s, "home.numberFormat", json!({"range": "C5:G11", "code": "\"$\"#,##0"}));
    ok &= run(s, "home.numberFormat", json!({"range": "H5:H11", "code": "0.0%"}));
    ok &= run(s, "home.conditionalFormat", json!({"range": "G5:G10", "rule": {"type": "dataBar", "color": "#5B9BD5"}}));
    ok &= run(s, "home.conditionalFormat", json!({"range": "H5:H10", "rule": {"type": "iconSet", "set": "3Arrows"}}));
    ok &= run(s, "home.columnWidth", json!({"cols": "A:A", "width": 20}));
    ok &= run(s, "home.columnWidth", json!({"cols": "B:B", "width": 132}));
    ok &= run(s, "home.columnWidth", json!({"cols": "C:G", "width": 84}));
    ok &= run(s, "home.columnWidth", json!({"cols": "H:I", "width": 76}));
    ok &= run(s, "selection.set", json!({"range": "B4:F10"}));
    ok &= run(s, "insert.chart", json!({"type": "column", "title": "Revenue by Region", "at": "K3", "width": 460, "height": 270}));
    ok &= run(s, "selection.set", json!({"range": "B4:B10"}));
    // Pie of totals.
    ok &=
        run(s, "insert.chart", json!({"range": "B4:B10", "type": "pie", "title": "Share of Annual Total", "at": "K19", "width": 460, "height": 260}));
    if let Ok(d) = s.doc_mut() {
        let wb = Arc::make_mut(&mut d.wb);
        if let Some(sh) = wb.sheet_mut(0)
            && let Some(c) = sh.charts.last_mut()
        {
            c.series = vec![mkmatrix_model::Series {
                name: Some("Sales!$G$4".into()),
                categories: Some("Sales!$B$5:$B$10".into()),
                values: "Sales!$G$5:$G$10".into(),
                bubble_sizes: None,
                color: None,
                secondary: false,
                kind: None,
                quartiles: Default::default(),
                smooth: false,
                trendlines: Vec::new(),
            }];
        }
    }
    ok &= run(s, "range.setValues", json!({"range": "B14", "values": [["Best region"], ["Average quarter"], ["Online share"], ["Year-over-year"]]}));
    ok &= run(s, "cell.set", json!({"cell": "C14", "input": "=INDEX(Sales[Region],MATCH(MAX(Sales[Total]),Sales[Total],0))"}));
    ok &= run(s, "cell.set", json!({"cell": "C15", "input": "=AVERAGE(C5:F10)"}));
    ok &= run(s, "cell.set", json!({"cell": "C16", "input": "=G10/SUM(G5:G10)"}));
    ok &= run(s, "cell.set", json!({"cell": "C17", "input": "=SUM(F5:F10)/SUM(C5:C10)-1"}));
    ok &= run(s, "home.numberFormat", json!({"range": "C15", "code": "\"$\"#,##0"}));
    ok &= run(s, "home.numberFormat", json!({"range": "C16:C17", "code": "0.0%"}));
    ok &= run(s, "home.bold", json!({"range": "B14:B17", "on": true}));
    ok &= run(s, "home.borders", json!({"range": "B14:C17", "preset": "outside"}));
    ok &= run(s, "home.fillColor", json!({"range": "B14:C17", "color": "#F2F7FC"}));
    ok &= run(s, "review.newNote", json!({"cell": "H4", "text": "Q4 vs Q1 growth for each region."}));
    ok &= run(s, "selection.set", json!({"cell": "B2"}));
    ok
}

fn budget(s: &mut Session) -> bool {
    let rows = json!([
        ["Category", "Budget", "Actual", "Difference", "% Used"],
        ["Rent", 1850, 1850],
        ["Groceries", 620, 684.35],
        ["Utilities", 240, 212.8],
        ["Transport", 180, 203.5],
        ["Internet & phone", 95, 95],
        ["Insurance", 210, 210],
        ["Dining out", 200, 265.9],
        ["Entertainment", 120, 88.25],
        ["Savings", 600, 600],
        ["Art supplies", 150, 172.4],
    ]);
    let mut ok = run(s, "range.setValues", json!({"range": "B2", "values": [["Monthly Budget — October 2026"]]}));
    ok &= run(s, "home.cellStyle", json!({"range": "B2", "name": "Title"}));
    ok &= run(s, "range.setValues", json!({"range": "B4", "values": rows}));
    for r in 5..=14 {
        ok &= run(s, "cell.set", json!({"cell": format!("E{r}"), "input": format!("=C{r}-D{r}")}));
        ok &= run(s, "cell.set", json!({"cell": format!("F{r}"), "input": format!("=D{r}/C{r}")}));
    }
    ok &= run(s, "range.setValues", json!({"range": "B15", "values": [["Total"]]}));
    for c in ["C", "D", "E"] {
        ok &= run(s, "cell.set", json!({"cell": format!("{c}15"), "input": format!("=SUM({c}5:{c}14)")}));
    }
    ok &= run(s, "cell.set", json!({"cell": "F15", "input": "=D15/C15"}));
    ok &= run(s, "home.cellStyle", json!({"range": "B4:F4", "name": "Heading 3"}));
    ok &= run(s, "home.cellStyle", json!({"range": "B15:F15", "name": "Total"}));
    ok &= run(s, "home.numberFormat", json!({"range": "C5:E15", "format": "Accounting"}));
    ok &= run(s, "home.numberFormat", json!({"range": "F5:F15", "code": "0%"}));
    ok &= run(
        s,
        "home.conditionalFormat",
        json!({"range": "E5:E14", "rule": {"type": "cellIs", "operator": "less", "value": 0, "preset": "lightRedFillDarkRedText"}}),
    );
    ok &= run(s, "home.conditionalFormat", json!({"range": "F5:F14", "rule": {"type": "colorScale", "colors": ["#63BE7B", "#FFEB84", "#F8696B"]}}));
    ok &= run(s, "home.columnWidth", json!({"cols": "B:B", "width": 130}));
    ok &= run(s, "home.columnWidth", json!({"cols": "C:E", "width": 90}));
    ok &= run(s, "selection.set", json!({"range": "B4:D14"}));
    ok &= run(s, "insert.chart", json!({"type": "bar", "title": "Budget vs Actual", "at": "H3", "width": 440, "height": 300}));
    ok &= run(s, "view.freezePanes", json!({"cell": "A5"}));
    ok &= run(s, "selection.set", json!({"cell": "B2"}));
    ok
}

fn grades(s: &mut Session) -> bool {
    let rows = json!([
        ["Student", "Essay", "Midterm", "Project", "Final", "Average", "Grade"],
        ["Amara Okafor", 92, 88, 95, 91],
        ["Bruno Silva", 78, 82, 74, 80],
        ["Chen Wei", 85, 91, 89, 94],
        ["Dana Kowalski", 67, 72, 80, 70],
        ["Elif Yılmaz", 98, 95, 97, 99],
        ["Farid Haddad", 74, 69, 71, 77],
        ["Grace Mensah", 88, 84, 90, 86],
        ["Hiro Tanaka", 59, 64, 70, 62],
    ]);
    let mut ok = run(s, "range.setValues", json!({"range": "A1", "values": rows}));
    for r in 2..=9 {
        ok &= run(s, "cell.set", json!({"cell": format!("F{r}"), "input": format!("=AVERAGE(B{r}:E{r})")}));
        ok &= run(
            s,
            "cell.set",
            json!({"cell": format!("G{r}"), "input": format!("=IFS(F{r}>=90,\"A\",F{r}>=80,\"B\",F{r}>=70,\"C\",F{r}>=60,\"D\",TRUE,\"F\")")}),
        );
    }
    ok &= run(s, "home.numberFormat", json!({"range": "F2:F9", "code": "0.0"}));
    ok &= run(s, "insert.table", json!({"range": "A1:G9", "style": "TableStyleLight9", "name": "Grades"}));
    ok &= run(s, "home.columnWidth", json!({"cols": "A:A", "width": 120}));
    ok &= run(
        s,
        "home.conditionalFormat",
        json!({"range": "G2:G9", "rule": {"type": "cellIs", "operator": "equal", "value": "A", "preset": "greenFillDarkGreenText"}}),
    );
    ok &= run(s, "home.conditionalFormat", json!({"range": "F2:F9", "rule": {"type": "dataBar", "color": "#63BE7B"}}));
    ok
}

/// A blank sheet (for callers that need one without a session).
pub fn blank_sheet(name: &str) -> Sheet {
    Sheet::new(name)
}
