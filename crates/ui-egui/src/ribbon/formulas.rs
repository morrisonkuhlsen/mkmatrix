use super::controls::{act, anchor_keytip, control_keytip, invoke, keytip, menu_header, menu_items, popup_command, sep};
use super::home::autosum_menu;
use super::*;

/// How many cells Formulas › Error Checking › Circular References lists.
const CIRCULAR_MENU_LIMIT: usize = 100;

/// The cell listed for each circular reference, for the Circular References submenu: each once, in the order
/// the calculation found them, as (sheet index, cell, "Sheet!A1").
fn circular_references(app: &SheetApp) -> Vec<(usize, String, String)> {
    let Some(d) = app.session.active() else { return Vec::new() };
    let mut seen = std::collections::HashSet::new();
    d.calc
        .circular
        .iter()
        .filter(|k| seen.insert(**k))
        .filter_map(|(si, c)| d.wb.sheet(*si).map(|sh| (*si, c.a1(), format!("{}!{}", mkmatrix_engine::formula::quote_sheet(&sh.name), c.a1()))))
        .take(CIRCULAR_MENU_LIMIT)
        .collect()
}

/// A function-category button and its menu of functions (`cat` is a `formulas.functions` category).
fn function_menu(app: &mut SheetApp, ui: &mut Ui, icon: Icon, label: &str, cat: &str, cid: &'static str) {
    let r = big_button(ui, icon, label, label, true);
    control_keytip(app, ui, &r, cid);
    let forced_c = invoke(app, cid);
    egui::Popup::menu(&r).open_memory(popup_command(forced_c, r.clicked())).show(|ui| {
        menu_header(app, ui, icon, label, false);
        ui.set_max_height(420.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            for f in mkmatrix_engine::cmd::formulas::function_list() {
                let Some(n) = f["name"].as_str() else { continue };
                let listed =
                    if cat == "LinearAlgebra" { mkmatrix_engine::cmd::formulas::is_linear_algebra(n) } else { f["category"].as_str() == Some(cat) };
                if !listed {
                    continue;
                }
                let locale = app.ui.language.formula_locale();
                let name = app.session.active().map(|d| crate::formula_locale::completion_name(n, locale, &d.wb, d.wb.active_sheet())).unwrap_or(n);
                let desc = crate::formula_locale::description(n, locale, app.ui.language).unwrap_or_default();
                if ui.button(name).on_hover_text(desc).clicked() {
                    app.run_or_alert("formulas.insertFunction", json!({"name": n}));
                    ui.close();
                }
            }
        });
    });
}

/// The Formulas tab: Insert Function, AutoSum, the function-category menus (names in the formula
/// language, descriptions in the interface language), defined names, auditing and calculation.
pub(super) fn formulas(app: &mut SheetApp, ui: &mut Ui) {
    // MK Matrix's own category leads: the matrix functions.
    function_menu(app, ui, Icon::Lambda, "Linear\nAlgebra", "LinearAlgebra", "formulas.linearAlgebra");
    let __k = big_button(ui, Icon::Function, "Insert\nFunction", "Insert Function (⇧F3)", false);
    control_keytip(app, ui, &__k, "dialog:insertFunction");
    if __k.clicked() {
        app.open_dialog("insertFunction", json!({}));
    }
    let (s, sa) = big_split_button(ui, Icon::Sum, "AutoSum", "AutoSum");
    control_keytip(app, ui, &sa, "formulas.autoSum");
    if s.clicked() {
        act(app, "formulas.autoSum", json!({}));
    }
    egui::Popup::menu(&sa).show(|ui| {
        menu_header(app, ui, Icon::Sum, "AutoSum", true);
        autosum_menu(app, ui);
    });
    // Each category its own icon, so the single row shows them as icons alone.
    for (icon, label, cat, cid) in [
        (Icon::Currency, "Financial", "Financial", "formulas.financial"),
        (Icon::Check, "Logical", "Logical", "formulas.logical"),
        (Icon::TextBox, "Text", "Text", "formulas.text"),
        (Icon::Calendar, "Date &\nTime", "DateTime", "formulas.dateTime"),
        (Icon::Find, "Lookup &\nReference", "Lookup", "formulas.lookup"),
        (Icon::Pi, "Math &\nTrig", "MathTrig", "formulas.mathTrig"),
        (Icon::More, "More\nFunctions", "Statistical", "formulas.moreFunctions"),
    ] {
        function_menu(app, ui, icon, label, cat, cid);
    }
    sep(ui, "Function Library");
    let __k = big_button(ui, Icon::Name, "Name\nManager", "Name Manager", false);
    control_keytip(app, ui, &__k, "dialog:nameManager");
    if __k.clicked() {
        app.open_dialog("nameManager", json!({}));
    }
    controls::stack(ui, |ui| {
        let __k = small_button(ui, Icon::Name, "Define Name", "Define Name", false);
        control_keytip(app, ui, &__k, "dialog:defineName");
        if __k.clicked() {
            app.open_dialog("defineName", json!({}));
        }
        let use_in = small_button(ui, Icon::Insert, "Use in Formula", "Use in Formula", true);
        anchor_keytip(app, ui, &use_in, &["dialog:pasteName"]);
        egui::Popup::menu(&use_in).show(|ui| {
            menu_header(app, ui, Icon::Insert, "Use in Formula", false);
            // The menu lists the names Paste Names lists; clicking one runs `formulas.useInFormula`, which puts it at the caret.
            // Listed when the menu opens, and again only after the workbook changes.
            let cache = egui::Id::unique("mkmatrix.use_in_formula_names");
            let key = app.session.active().map(|d| (d.uid, d.revision, d.wb.active_sheet()));
            let cached = ui.ctx().data(|m| m.get_temp::<(Option<(u64, u64, usize)>, std::sync::Arc<Vec<String>>)>(cache)).filter(|(k, _)| *k == key);
            let names = match cached {
                Some((_, names)) => names,
                None => {
                    let names: std::sync::Arc<Vec<String>> = std::sync::Arc::new(
                        app.session
                            .active()
                            .map(|d| mkmatrix_engine::cmd::formulas::pasteable_names(&d.wb).into_iter().map(|(n, _)| n).collect())
                            .unwrap_or_default(),
                    );
                    ui.ctx().data_mut(|m| m.insert_temp(cache, (key, names.clone())));
                    names
                }
            };
            for n in names.iter() {
                if ui.button(n).clicked() {
                    app.run_or_alert("formulas.useInFormula", json!({"name": n}));
                }
            }
            if !names.is_empty() {
                ui.separator();
            }
            if ui.button(app.ui.language.tr("Paste Names…")).clicked() {
                app.open_dialog("pasteName", json!({}));
            }
        });
        let __k = small_button(ui, Icon::Table, "Create from Selection", "Create from Selection", false);
        control_keytip(app, ui, &__k, "formulas.createFromSelection");
        if __k.clicked() {
            act(app, "formulas.createFromSelection", json!({}));
        }
    });
    sep(ui, "Defined Names");
    controls::stack(ui, |ui| {
        if small_button(ui, Icon::Trace, "Trace Precedents", "Trace Precedents", false).clicked()
            && let Ok(r) = app.run("formulas.tracePrecedents", json!({}))
        {
            app.grid.trace = crate::grid::Trace::new(app, r);
        }
        if small_button(ui, Icon::Trace, "Trace Dependents", "Trace Dependents", false).clicked()
            && let Ok(r) = app.run("formulas.traceDependents", json!({}))
        {
            app.grid.trace = crate::grid::Trace::new(app, r);
        }
        if small_button(ui, Icon::Close, "Remove Arrows", "Remove Arrows", false).clicked() {
            app.grid.trace = None;
        }
    });
    controls::stack(ui, |ui| {
        let __k = small_button(ui, Icon::Fx, "Show Formulas", "Show Formulas", false);
        control_keytip(app, ui, &__k, "formulas.showFormulas");
        if __k.clicked() {
            act(app, "formulas.showFormulas", json!({}));
        }
        let (ec, ec_arrow) = small_split_button(ui, Icon::Validation, "Error Checking", "Error Checking");
        control_keytip(app, ui, &ec, "dialog:errorChecking");
        if ec.clicked() {
            app.open_dialog("errorChecking", json!({}));
        }
        egui::Popup::menu(&ec_arrow).show(|ui| {
            menu_header(app, ui, Icon::Validation, "Error Checking", false);
            let lang = app.ui.language;
            if ui.button(format!("{}…", lang.tr("Error Checking"))).clicked() {
                app.open_dialog("errorChecking", json!({}));
            }
            let circular = circular_references(app);
            ui.add_enabled_ui(!circular.is_empty(), |ui| {
                ui.menu_button(lang.tr("Circular References"), |ui| {
                    egui::ScrollArea::vertical().id_salt("circular_references").max_height(400.0).show(ui, |ui| {
                        for (sheet, cell, label) in circular {
                            if ui.button(label).clicked() {
                                act(app, "selection.set", json!({"range": cell, "sheet": sheet}));
                                app.grid.ensure_visible = true;
                            }
                        }
                    });
                });
            });
        });
        let __k = small_button(ui, Icon::Calc, "Evaluate Formula", "Evaluate Formula", false);
        control_keytip(app, ui, &__k, "dialog:evaluateFormula");
        if __k.clicked() {
            app.open_dialog("evaluateFormula", json!({}));
        }
        // Beyond Excel: why a cell shows what it does, down to the data.
        if small_button(ui, Icon::Trace, "Explain Formula", "Explain Formula", false).clicked() {
            app.open_dialog("explainFormula", json!({}));
        }
    });
    if big_button(ui, Icon::Search, "Watch\nWindow", "Watch Window", false).clicked() {
        app.grid.pane = Some("watch".into());
    }
    sep(ui, "Formula Auditing");
    let co = big_button(ui, Icon::Calc, "Calculation\nOptions", "Calculation Options", true);
    for sequence in keytip_table::group_extension(app.keytips.prefix().unwrap_or_default(), &["formulas.calculationOptions"]) {
        keytip(app, ui, sequence, co.rect.center_bottom());
    }
    egui::Popup::menu(&co).show(|ui| {
        menu_header(app, ui, Icon::Calc, "Calculation Options", false);
        menu_items(
            app,
            ui,
            &[
                ("Automatic", "formulas.calculationOptions", json!({"mode": "automatic"})),
                ("Automatic Except for Data Tables", "formulas.calculationOptions", json!({"mode": "automaticExceptTables"})),
                ("Manual", "formulas.calculationOptions", json!({"mode": "manual"})),
            ],
        );
    });
    controls::stack(ui, |ui| {
        let __k = small_button(ui, Icon::Calc, "Calculate Now", "Calculate Now (F9)", false);
        control_keytip(app, ui, &__k, "formulas.calculateNow");
        if __k.clicked() {
            act(app, "formulas.calculateNow", json!({}));
        }
        let __k = small_button(ui, Icon::Sheet, "Calculate Sheet", "Calculate Sheet (⇧F9)", false);
        control_keytip(app, ui, &__k, "formulas.calculateSheet");
        if __k.clicked() {
            act(app, "formulas.calculateSheet", json!({}));
        }
    });
}
