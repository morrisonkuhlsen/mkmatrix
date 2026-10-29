use super::kit::{ROW_H, button, form, heading, hint, label, list_row, option_row, panel, rule, segments};
use super::show::{form_fields, ok_cancel};
use super::*;

/// Data › What-If Analysis › Goal Seek, in Format Cells' style: the three cells on a form, the
/// footer under a rule.
pub(super) fn goal_seek(app: &mut SheetApp, ui: &mut egui::Ui, d: &mut Dialog, confirm: &mut bool) {
    let lang = app.ui.language;
    let get = |k: &str| d.values.get(k).and_then(Json::as_str).unwrap_or("").to_string();
    let (mut set, mut to, mut changing) = (get("set"), get("to"), get("changing"));
    form(ui, "goalSeek", |ui| {
        for (text, value) in [("Set cell:", &mut set), ("To value:", &mut to), ("By changing cell:", &mut changing)] {
            let id = label(ui, lang.tr(text));
            ui.add(egui::TextEdit::singleline(value).desired_width(200.0)).labelled_by(id);
            ui.end_row();
        }
    });
    d.values.insert("set".into(), json!(set.clone()));
    d.values.insert("to".into(), json!(to.clone()));
    d.values.insert("changing".into(), json!(changing.clone()));
    let mut open = true;
    rule(ui);
    ok_cancel(ui, confirm, &mut open);
    if *confirm {
        match app.run("data.goalSeek", json!({"set": set, "to": to.parse::<f64>().unwrap_or(0.0), "changing": changing})) {
            Ok(r) => app.toast = Some((format!("Goal Seeking found a solution: {}", r["value"]), crate::now_ms())),
            Err(e) => app.message = Some(("Goal Seek".into(), crate::clean_error(&e))),
        }
    }
    if !open {
        *confirm = true;
    }
}

/// The fields of one Analysis ToolPak tool's form (input and options; output is drawn apart).
fn toolpak_fields(tool: &str) -> Vec<Field> {
    use Field::*;
    let grouped = || Choice { key: "groupedBy", label: "Grouped By:", options: vec![("columns", "Columns"), ("rows", "Rows")] };
    let labels = || Check { key: "labels", label: "Labels in first row" };
    let range = || Text { key: "range", label: "Input Range:" };
    let two = || vec![Text { key: "range1", label: "Variable 1 Range:" }, Text { key: "range2", label: "Variable 2 Range:" }];
    match tool {
        "descriptive" => vec![
            range(),
            grouped(),
            labels(),
            Check { key: "summary", label: "Summary statistics" },
            Text { key: "confidence", label: "Confidence Level for Mean (%), blank for none:" },
            Text { key: "kthLargest", label: "Kth Largest (blank for none):" },
            Text { key: "kthSmallest", label: "Kth Smallest (blank for none):" },
        ],
        "histogram" => vec![
            range(),
            Text { key: "bins", label: "Bin Range (blank for automatic bins):" },
            labels(),
            Check { key: "pareto", label: "Pareto (sorted histogram)" },
            Check { key: "cumulative", label: "Cumulative Percentage" },
        ],
        "correlation" | "covariance" | "rankPercentile" => vec![range(), grouped(), labels()],
        "anovaSingle" => vec![range(), grouped(), labels(), Text { key: "alpha", label: "Alpha:" }],
        "regression" => vec![
            Text { key: "inputY", label: "Input Y Range:" },
            Text { key: "inputX", label: "Input X Range:" },
            labels(),
            Check { key: "constantZero", label: "Constant is Zero" },
            Text { key: "confidence", label: "Confidence Level (%):" },
            Check { key: "residuals", label: "Residuals" },
            Check { key: "standardizedResiduals", label: "Standardized Residuals" },
            Check { key: "normalProbability", label: "Normal Probability" },
        ],
        "movingAverage" => {
            vec![range(), labels(), Text { key: "interval", label: "Interval:" }, Check { key: "standardErrors", label: "Standard Errors" }]
        }
        "exponentialSmoothing" => {
            vec![range(), labels(), Text { key: "damping", label: "Damping factor:" }, Check { key: "standardErrors", label: "Standard Errors" }]
        }
        "fTest" => {
            let mut v = two();
            v.extend([labels(), Text { key: "alpha", label: "Alpha:" }]);
            v
        }
        _ => {
            let mut v = two();
            v.extend([
                Text { key: "hypothesizedDifference", label: "Hypothesized Mean Difference:" },
                labels(),
                Text { key: "alpha", label: "Alpha:" },
            ]);
            v
        }
    }
}

/// The starting values of a tool's form: the selection as its input, a new sheet as output.
pub(super) fn toolpak_defaults(sel: &str, tool: &str) -> Map<String, Json> {
    let mut v = json!({
        "tool": tool, "range": sel, "inputY": "", "inputX": "", "range1": sel, "range2": "", "bins": "",
        "groupedBy": "columns", "labels": false, "summary": true, "confidence": "", "kthLargest": "", "kthSmallest": "",
        "interval": "3", "damping": "0.3", "alpha": "0.05", "hypothesizedDifference": "0",
        "outputTo": "newSheet", "output": "", "newSheet": "",
    });
    if tool == "regression" {
        v["confidence"] = json!("95");
        v["inputY"] = json!(sel);
    }
    v.as_object().cloned().unwrap_or_default()
}

/// Data › Data Analysis, in Format Cells' style: pick a tool from a framed list, then fill in its
/// form, with the output options under a heading of their own. OK runs `data.analysisToolPak`
/// (through the dialog's command); a report that would replace data asks first.
pub(super) fn analysis_toolpak(app: &mut SheetApp, ui: &mut egui::Ui, d: &mut Dialog, confirm: &mut bool, open: &mut bool) {
    use mkmatrix_engine::cmd::analysis::TOOLS;
    let lang = app.ui.language;
    if d.values.get("askOverwrite").and_then(Json::as_bool) == Some(true) {
        ui.add_space(4.0);
        ui.add(
            egui::Label::new(
                egui::RichText::new(lang.tr("The output range already holds data. Replace it with the report?")).font(theme::ui_font(13.5)),
            )
            .wrap(),
        );
        rule(ui);
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if button(ui, lang.tr("Replace"), true).clicked() {
                    d.values.remove("askOverwrite");
                    d.values.insert("overwrite".into(), json!(true));
                    *confirm = true;
                }
                if button(ui, lang.tr("Cancel"), false).clicked() {
                    d.values.remove("askOverwrite");
                }
            });
        });
        return;
    }
    let tool = d.values.get("tool").and_then(Json::as_str).unwrap_or("").to_string();
    let Some((_, label)) = TOOLS.iter().find(|(id, _)| *id == tool) else {
        heading(ui, lang.tr("Analysis Tools"));
        let pick = d.values.get("pick").and_then(Json::as_str).unwrap_or("").to_string();
        let mut chosen = false;
        let list_h = 10.0 * ROW_H + 2.0;
        panel(ui, list_h, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            egui::ScrollArea::vertical().id_salt("toolpak.list").auto_shrink([false, false]).show(ui, |ui| {
                for (id, label) in TOOLS {
                    let r = list_row(ui, lang.tr(label), None, pick == *id, ui.available_width());
                    if r.clicked() {
                        d.values.insert("pick".into(), json!(id));
                    }
                    if r.double_clicked() {
                        chosen = true;
                    }
                }
            });
        });
        let mut ok = false;
        rule(ui);
        ok_cancel(ui, &mut ok, open);
        let pick = d.values.get("pick").and_then(Json::as_str).unwrap_or("").to_string();
        if (ok || chosen) && TOOLS.iter().any(|(id, _)| *id == pick) {
            let sel = app.session.active().map(|doc| doc.selection.current().a1()).unwrap_or_default();
            d.values = toolpak_defaults(&sel, &pick);
        }
        return;
    };
    d.title = label.to_string();
    d.fields = toolpak_fields(&tool);
    heading(ui, lang.tr("Input"));
    form_fields(ui, d);
    ui.add_space(10.0);
    heading(ui, lang.tr("Output options"));
    let to = d.values.get("outputTo").and_then(Json::as_str).unwrap_or("newSheet").to_string();
    for (key, choice, text_label) in [("output", "range", "Output Range:"), ("newSheet", "newSheet", "New Worksheet Ply:")] {
        ui.horizontal(|ui| {
            if option_row(ui, lang.tr(text_label), to == choice, 190.0).clicked() {
                d.values.insert("outputTo".into(), json!(choice));
            }
            let mut t = d.values.get(key).and_then(Json::as_str).unwrap_or("").to_string();
            if ui.add(egui::TextEdit::singleline(&mut t).desired_width(ui.available_width())).accessible_name(lang.tr(text_label)).changed() {
                d.values.insert(key.into(), json!(t));
                d.values.insert("outputTo".into(), json!(choice));
            }
        });
    }
    rule(ui);
    ok_cancel(ui, confirm, open);
}

/// Data › Solver: the Solver Parameters form, in Format Cells' style: the objective and its goal,
/// the variable cells, the constraints in a framed list with a row to add one, and the solving
/// method, each under a muted heading; Close and Solve in the footer. Solve runs `data.solver`
/// through the dialog's command; the result opens Solver Results.
pub(super) fn solver(ui: &mut egui::Ui, d: &mut Dialog, confirm: &mut bool, open: &mut bool) {
    let lang = crate::i18n::current(ui.ctx());
    let text = |d: &Dialog, k: &str| d.values.get(k).map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())).unwrap_or_default();
    let edit = |ui: &mut egui::Ui, d: &mut Dialog, k: &str, width: f32, label: &str| {
        let mut t = text(d, k);
        if ui.add(egui::TextEdit::singleline(&mut t).desired_width(width)).accessible_name(lang.tr(label)).changed() {
            d.values.insert(k.into(), json!(t));
        }
    };
    let full = ui.available_width();
    ui.spacing_mut().item_spacing.y = 6.0;
    heading(ui, lang.tr("Set Objective:"));
    edit(ui, d, "objective", full, "Set Objective:");
    let goal = text(d, "goal");
    ui.horizontal(|ui| {
        hint(ui, lang.tr("To:"));
        if let Some(g) = segments(ui, &goal, &[("max", lang.tr("Max")), ("min", lang.tr("Min")), ("value", lang.tr("Value Of:"))], 86.0) {
            d.values.insert("goal".into(), json!(g));
        }
        ui.add_space(6.0);
        ui.add_enabled_ui(goal == "value", |ui| edit(ui, d, "value", ui.available_width(), "Value Of:"));
    });
    ui.add_space(4.0);
    heading(ui, lang.tr("By Changing Variable Cells:"));
    edit(ui, d, "variables", full, "By Changing Variable Cells:");
    ui.add_space(4.0);
    heading(ui, lang.tr("Subject to the Constraints:"));
    let list: Vec<String> = d
        .values
        .get("constraints")
        .and_then(Json::as_array)
        .map(|a| a.iter().map(|c| c.as_str().map(str::to_string).unwrap_or_else(|| c.to_string())).collect())
        .unwrap_or_default();
    let list_h = 4.0 * ROW_H + 2.0;
    panel(ui, list_h, |ui| {
        if list.is_empty() {
            ui.add_space(list_h / 2.0 - 10.0);
            ui.vertical_centered(|ui| hint(ui, lang.tr("No constraints")));
            return;
        }
        ui.spacing_mut().item_spacing.y = 0.0;
        egui::ScrollArea::vertical().id_salt("solver.constraints").auto_shrink([false, false]).show(ui, |ui| {
            for (i, c) in list.iter().enumerate() {
                if list_row(ui, c, None, d.list_index == i, ui.available_width()).clicked() {
                    d.list_index = i;
                }
            }
        });
    });
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        edit(ui, d, "newLhs", 120.0, "Constraint Cell Reference");
        let op = text(d, "newOp");
        craft_ui::ComboBox::from_id_salt("solverOp")
            .selected_text(op.clone())
            .width(60.0)
            .show_ui(ui, |ui| {
                for o in ["<=", "=", ">=", "int", "bin"] {
                    if ui.selectable_label(op == o, o).clicked() {
                        d.values.insert("newOp".into(), json!(o));
                    }
                }
            })
            .response
            .accessible_name(lang.tr("Constraint Operator"));
        if !matches!(op.as_str(), "int" | "bin") {
            edit(ui, d, "newRhs", 110.0, "Constraint Value");
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_enabled_ui(d.list_index < list.len(), |ui| {
                if button(ui, lang.tr("Delete"), false).clicked() {
                    let mut list = list.clone();
                    if d.list_index < list.len() {
                        list.remove(d.list_index);
                    }
                    d.values.insert("constraints".into(), json!(list));
                }
            });
            if button(ui, lang.tr("Add"), false).clicked() {
                let lhs = text(d, "newLhs");
                if !lhs.trim().is_empty() {
                    let rhs = if matches!(op.as_str(), "int" | "bin") { String::new() } else { format!(" {}", text(d, "newRhs").trim()) };
                    let mut list = list.clone();
                    list.push(format!("{} {op}{rhs}", lhs.trim()));
                    d.values.insert("constraints".into(), json!(list));
                    d.values.insert("newLhs".into(), json!(""));
                    d.values.insert("newRhs".into(), json!(""));
                }
            }
        });
    });
    let mut nonneg = d.values.get("nonNegative").and_then(Json::as_bool).unwrap_or(true);
    if ui.checkbox(&mut nonneg, lang.tr("Make Unconstrained Variables Non-Negative")).changed() {
        d.values.insert("nonNegative".into(), json!(nonneg));
    }
    ui.add_space(4.0);
    heading(ui, lang.tr("Select a Solving Method:"));
    let method = text(d, "method");
    let method_label = |m: &str| match m {
        "simplex" => "Simplex LP",
        "evolutionary" => "Evolutionary",
        _ => "GRG Nonlinear",
    };
    craft_ui::ComboBox::from_id_salt("solverMethod")
        .selected_text(lang.tr(method_label(&method)))
        .width(200.0)
        .show_ui(ui, |ui| {
            for m in ["grg", "simplex", "evolutionary"] {
                if ui.selectable_label(method == m, lang.tr(method_label(m))).clicked() {
                    d.values.insert("method".into(), json!(m));
                }
            }
        })
        .response
        .accessible_name(lang.tr("Select a Solving Method:"));
    let about = match method.as_str() {
        "simplex" => "Simplex LP: for linear models; integer and binary constraints by branch and bound.",
        "evolutionary" => "Evolutionary: a genetic search for models that aren't smooth; every variable cell needs bounds.",
        _ => "GRG Nonlinear: a local optimum of a smooth model, from the current values.",
    };
    ui.add(egui::Label::new(egui::RichText::new(lang.tr(about)).font(theme::ui_font(12.5)).color(theme::Tokens::get(ui.ctx()).text_dim)).wrap());
    rule(ui);
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if button(ui, lang.tr("Solve"), true).clicked() && !text(d, "objective").trim().is_empty() {
                *confirm = true;
            }
            if button(ui, lang.tr("Close"), false).clicked() {
                *open = false;
            }
        });
    });
}

/// Solver Results, in Format Cells' style: the outcome, keeping or restoring the variable cells as
/// round options, and the reports to write.
pub(super) fn solver_results(app: &mut SheetApp, ui: &mut egui::Ui, d: &mut Dialog, open: &mut bool) {
    let lang = app.ui.language;
    let message = d.values.get("message").and_then(Json::as_str).unwrap_or("").to_string();
    ui.add(egui::Label::new(egui::RichText::new(message).font(theme::ui_font(13.5))).wrap());
    let changed = d.values.get("changed").and_then(Json::as_bool).unwrap_or(false);
    let restore = d.values.get("restore").and_then(Json::as_bool).unwrap_or(false);
    let w = ui.available_width();
    if changed {
        ui.add_space(8.0);
        ui.spacing_mut().item_spacing.y = 2.0;
        if option_row(ui, lang.tr("Keep Solver Solution"), !restore, w).clicked() {
            d.values.insert("restore".into(), json!(false));
        }
        if option_row(ui, lang.tr("Restore Original Values"), restore, w).clicked() {
            d.values.insert("restore".into(), json!(true));
        }
    }
    // Reports describe the solution whether it is kept or not.
    let solved = d.values.get("values").and_then(Json::as_object).is_some_and(|v| !v.is_empty());
    if solved {
        ui.add_space(10.0);
        heading(ui, lang.tr("Reports:"));
        ui.spacing_mut().item_spacing.y = 4.0;
        for (key, label) in [("answerReport", "Answer"), ("limitsReport", "Limits")] {
            let mut on = d.values.get(key).and_then(Json::as_bool).unwrap_or(false);
            if ui.checkbox(&mut on, lang.tr(label)).changed() {
                d.values.insert(key.into(), json!(on));
            }
        }
    }
    rule(ui);
    let mut confirm = false;
    ok_cancel(ui, &mut confirm, open);
    if confirm {
        // The solution (and the saved model) is one undo step.
        if changed && restore {
            app.run_or_alert("edit.undo", json!({}));
        }
        let reports: Vec<&str> = [("answerReport", "answer"), ("limitsReport", "limits")]
            .into_iter()
            .filter(|(key, _)| d.values.get(*key).and_then(Json::as_bool) == Some(true))
            .map(|(_, kind)| kind)
            .collect();
        if solved && !reports.is_empty() {
            let field = |k: &str| d.values.get(k).cloned().unwrap_or(Json::Null);
            let params = json!({"reports": reports, "model": field("model"), "values": field("values"), "original": field("original"), "message": field("message")});
            app.run_or_alert("data.solverReport", params);
        }
        *open = false;
    }
}
