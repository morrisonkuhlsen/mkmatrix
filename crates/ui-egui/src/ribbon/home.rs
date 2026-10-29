use super::cf_menu::cf_menu;
use super::controls::{
    ComboResult, Mark, RowExtra, act, active_style, anchor_keytip, control_keytip, invoke, keytip, list_combo, marked_combo, menu_items,
    popup_command, sep, stack, theme_colors, tool, wide,
};
use super::galleries::{cell_style_gallery, table_gallery};
use super::*;

/// The Accounting Number Format dropdown: label and the code Excel applies for that symbol.
const ACCOUNTING_SYMBOLS: &[(&str, &str)] = &[
    ("$ English (United States)", "_(\"$\"* #,##0.00_);_(\"$\"* \\(#,##0.00\\);_(\"$\"* \"-\"??_);_(@_)"),
    ("£ English (United Kingdom)", "_-[$£-809]* #,##0.00_-;-[$£-809]* #,##0.00_-;_-[$£-809]* \"-\"??_-;_-@_-"),
    ("€ Euro", "_-[$€-x-euro2] * #,##0.00_-;-[$€-x-euro2] * #,##0.00_-;_-[$€-x-euro2] * \"-\"??_-;_-@_-"),
    ("¥ Japanese", "_-[$¥-411]* #,##0_-;-[$¥-411]* #,##0_-;_-[$¥-411]* \"-\"_-;_-@_-"),
    ("¥ Chinese (PRC)", "_-[$¥-804]* #,##0.00_-;-[$¥-804]* #,##0.00_-;_-[$¥-804]* \"-\"??_-;_-@_-"),
    ("₹ English (India)", "_-[$₹-4009]* #,##0.00_-;-[$₹-4009]* #,##0.00_-;_-[$₹-4009]* \"-\"??_-;_-@_-"),
    ("CHF German (Switzerland)", "_-[$CHF-807]* #,##0.00_-;-[$CHF-807]* #,##0.00_-;_-[$CHF-807]* \"-\"??_-;_-@_-"),
    ("Rp Indonesian", "_-[$Rp-421]* #,##0.00_-;-[$Rp-421]* #,##0.00_-;_-[$Rp-421]* \"-\"??_-;_-@_-"),
    ("₩ Korean", "_-[$₩-412]* #,##0_-;-[$₩-412]* #,##0_-;_-[$₩-412]* \"-\"_-;_-@_-"),
    ("R$ Portuguese (Brazil)", "_-[$R$-416]* #,##0.00_-;-[$R$-416]* #,##0.00_-;_-[$R$-416]* \"-\"??_-;_-@_-"),
    ("₽ Russian", "_-[$₽-419]* #,##0.00_-;-[$₽-419]* #,##0.00_-;_-[$₽-419]* \"-\"??_-;_-@_-"),
];

/// Draws the Home tab: clipboard/paste menu, font, alignment, number, style, cell and editing commands with keytips.
pub(super) fn home(app: &mut SheetApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let st = active_style(app);
    // Clipboard
    let paste = tool(ui, Icon::Paste, "Paste", "Paste", true);
    paste.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), "Paste"));
    for sequence in
        keytip_table::group_extension(app.keytips.prefix().unwrap_or_default(), &["edit.paste", "edit.pasteSpecial", "dialog:pasteSpecial"])
    {
        keytip(app, ui, sequence, paste.rect.center_bottom());
    }
    if paste.clicked() {
        app.paste_from_clipboard();
    }
    let paste_keys = matches!(app.keytips.prefix(), Some("HV" | "E"));
    // Keep a keyboard-opened menu anchored to the button when a pointer click
    // cancels keytips, so its items remain in place and can still be clicked.
    let anchor_id = paste.id.with("keytip_anchor");
    // Query response state before taking the egui context write lock.
    // Calling secondary_clicked() inside data_mut can deadlock.
    let paste_secondary_clicked = paste.secondary_clicked();
    let anchored = ui.ctx().data_mut(|data| {
        let anchored = if paste_keys {
            true
        } else if paste_secondary_clicked {
            false
        } else {
            data.get_temp::<bool>(anchor_id).unwrap_or(false)
        };
        data.insert_temp(anchor_id, anchored);
        anchored
    });
    let mut paste_popup = egui::Popup::context_menu(&paste);
    if anchored {
        paste_popup = paste_popup.anchor(&paste);
    }
    if paste_keys {
        paste_popup = paste_popup.open_memory(egui::SetOpenCommand::Bool(true));
    }
    paste_popup.show(|ui| {
        menu_items(
            app,
            ui,
            &[
                ("Paste", "edit.paste", json!({})),
                ("Paste Values", "edit.pasteSpecial", json!({"what": "values"})),
                ("Paste Formulas", "edit.pasteSpecial", json!({"what": "formulas"})),
                ("Paste Formatting", "edit.pasteSpecial", json!({"what": "formats"})),
                ("Transpose", "edit.pasteSpecial", json!({"what": "all", "transpose": true})),
                ("Paste Link", "edit.pasteSpecial", json!({"what": "all", "link": true})),
                ("-", "", json!(null)),
                ("Paste Special…", "dialog:pasteSpecial", json!({})),
            ],
        );
    });
    stack(ui, |ui| {
        let __k = icon_button(ui, Icon::Cut, t.text, "Cut", vec2(26.0, 23.0));
        control_keytip(app, ui, &__k, "edit.cut");
        if __k.clicked() {
            app.copy_to_clipboard(ui.ctx(), "edit.cut");
        }
        let __k = icon_button(ui, Icon::Copy, t.text, "Copy", vec2(26.0, 23.0));
        control_keytip(app, ui, &__k, "edit.copy");
        if __k.clicked() {
            app.copy_to_clipboard(ui.ctx(), "edit.copy");
        }
        let fp_on = app.session.format_painter.is_some();
        let fp = toggle_button(ui, Icon::Brush, fp_on, "Format Painter (double-click to keep it on)");
        fp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), fp_on, "Format Painter"));
        control_keytip(app, ui, &fp, "edit.formatPainter");
        if fp.double_clicked() {
            act(app, "edit.formatPainter", json!({"sticky": true}));
        } else if fp.clicked() {
            if fp_on {
                app.session.format_painter = None;
            } else {
                act(app, "edit.formatPainter", json!({}));
            }
        }
    });
    sep(ui, "Clipboard");
    // Font
    stack(ui, |ui| {
        ui.horizontal(|ui| {
            font_combo(app, ui, &st);
            size_combo(app, ui, &st);
            let __k = icon_button(ui, Icon::Plus, t.text, "Increase Font Size", vec2(22.0, 23.0));
            control_keytip(app, ui, &__k, "home.increaseFontSize");
            if __k.clicked() {
                act(app, "home.increaseFontSize", json!({}));
            }
            let __k = icon_button(ui, Icon::Minus, t.text, "Decrease Font Size", vec2(22.0, 23.0));
            control_keytip(app, ui, &__k, "home.decreaseFontSize");
            if __k.clicked() {
                act(app, "home.decreaseFontSize", json!({}));
            }
        });
        ui.horizontal(|ui| {
            let __k = toggle_button(ui, Icon::Bold, st.font.bold, "Bold");
            control_keytip(app, ui, &__k, "home.bold");
            if __k.clicked() {
                act(app, "home.bold", json!({}));
            }
            let __k = toggle_button(ui, Icon::Italic, st.font.italic, "Italic");
            control_keytip(app, ui, &__k, "home.italic");
            if __k.clicked() {
                act(app, "home.italic", json!({}));
            }
            let (u, ua) = split_button(ui, Icon::Underline, None, "Underline");
            control_keytip(app, ui, &ua, "home.underline");
            if u {
                act(app, "home.underline", json!({}));
            }
            egui::Popup::menu(&ua).show(|ui| {
                menu_items(
                    app,
                    ui,
                    &[
                        ("Underline", "home.underline", json!({"style": "single"})),
                        ("Double Underline", "home.underline", json!({"style": "double"})),
                    ],
                );
            });
            let __k = toggle_button(ui, Icon::Strike, st.font.strike, "Strikethrough");
            control_keytip(app, ui, &__k, "home.strikethrough");
            if __k.clicked() {
                act(app, "home.strikethrough", json!({}));
            }
            let (b, ba) = split_button(ui, Icon::Borders, None, "Borders");
            control_keytip(app, ui, &ba, "home.borders");
            if b {
                act(app, "home.borders", json!({"preset": app.grid.last_border.clone()}));
            }
            ba.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), "Border presets"));
            egui::Popup::menu(&ba).show(|ui| {
                for (label, preset) in [
                    ("Bottom Border", "bottom"),
                    ("Top Border", "top"),
                    ("Left Border", "left"),
                    ("Right Border", "right"),
                    ("No Border", "none"),
                    ("All Borders", "all"),
                    ("Outside Borders", "outside"),
                    ("Thick Outside Borders", "thickOutside"),
                    ("Bottom Double Border", "doubleBottom"),
                    ("Thick Bottom Border", "thickBottom"),
                    ("Top and Bottom Border", "topBottom"),
                    ("Top and Thick Bottom Border", "topThickBottom"),
                    ("Top and Double Bottom Border", "topDoubleBottom"),
                    ("Inside Borders", "inside"),
                ] {
                    if crate::border_preview::preset_button(ui, label, preset).clicked() {
                        app.grid.last_border = preset.to_string();
                        act(app, "home.borders", json!({"preset": preset}));
                    }
                }
                ui.separator();
                if ui.button("More Borders…").clicked() {
                    app.open_dialog("formatCells", json!({"tab": "Border"}));
                }
            });
            let fill = app.grid.last_fill.clone();
            let fill_c = mkmatrix_engine::model::Color::from_hex(&fill).and_then(|c| c.resolve(&Default::default())).map(theme::color32);
            let (f, fa) = split_button(ui, Icon::Fill, fill_c, "Fill Color");
            control_keytip(app, ui, &fa, "home.fillColor");
            if f {
                act(app, "home.fillColor", json!({"color": fill}));
            }
            let forced = invoke(app, "home.fillColor");
            egui::Popup::menu(&fa)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .open_memory(popup_command(forced, fa.clicked()))
                .show(|ui| {
                    if let Some(c) = color_palette(ui, &theme_colors(app), "No Fill") {
                        if c != "none" {
                            app.grid.last_fill = c.clone();
                        }
                        act(app, "home.fillColor", json!({"color": c}));
                        ui.close();
                    }
                });
            let fc = app.grid.last_font_color.clone();
            let fc_c = mkmatrix_engine::model::Color::from_hex(&fc).and_then(|c| c.resolve(&Default::default())).map(theme::color32);
            let (c, ca) = split_button(ui, Icon::FontColor, fc_c, "Font Color");
            control_keytip(app, ui, &ca, "home.fontColor");
            if c {
                act(app, "home.fontColor", json!({"color": fc}));
            }
            let forced = invoke(app, "home.fontColor");
            egui::Popup::menu(&ca)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .open_memory(popup_command(forced, ca.clicked()))
                .show(|ui| {
                    if let Some(c) = color_palette(ui, &theme_colors(app), "Automatic") {
                        if c != "none" {
                            app.grid.last_font_color = c.clone();
                        }
                        act(app, "home.fontColor", json!({"color": if c == "none" { "auto".to_string() } else { c }}));
                        ui.close();
                    }
                });
        });
    });
    sep(ui, "Font");
    // Alignment
    stack(ui, |ui| {
        ui.horizontal(|ui| {
            for (icon, v, id, tip) in [
                (Icon::AlignTop, VAlign::Top, "home.alignTop", "Top Align"),
                (Icon::AlignMiddle, VAlign::Center, "home.alignMiddle", "Middle Align"),
                (Icon::AlignBottom, VAlign::Bottom, "home.alignBottom", "Bottom Align"),
            ] {
                let __k = toggle_button(ui, icon, st.align.v == v, tip);
                control_keytip(app, ui, &__k, id);
                if __k.clicked() {
                    act(app, id, json!({}));
                }
            }
            let o = small_button(ui, Icon::Orientation, "", "Orientation", true);
            control_keytip(app, ui, &o, "home.orientation");
            let forced_o = invoke(app, "home.orientation");
            egui::Popup::menu(&o).open_memory(popup_command(forced_o, o.clicked())).show(|ui| {
                menu_items(
                    app,
                    ui,
                    &[
                        ("Angle Counterclockwise", "home.orientation", json!({"angle": "counterclockwise"})),
                        ("Angle Clockwise", "home.orientation", json!({"angle": "clockwise"})),
                        ("Vertical Text", "home.orientation", json!({"angle": "vertical"})),
                        ("Rotate Text Up", "home.orientation", json!({"angle": "up"})),
                        ("Rotate Text Down", "home.orientation", json!({"angle": "down"})),
                        ("Horizontal", "home.orientation", json!({"angle": 0})),
                        ("-", "", json!(null)),
                        ("Format Cell Alignment…", "dialog:formatCells", json!({"tab": "Alignment"})),
                    ],
                );
            });
            let wrap = toggle_button(ui, Icon::Wrap, st.align.wrap, "Wrap Text");
            control_keytip(app, ui, &wrap, "home.wrapText");
            if wrap.clicked() {
                act(app, "home.wrapText", json!({}));
            }
        });
        ui.horizontal(|ui| {
            for (icon, h, id, tip) in [
                (Icon::AlignLeft, HAlign::Left, "home.alignLeft", "Align Left"),
                (Icon::AlignCenter, HAlign::Center, "home.alignCenter", "Center"),
                (Icon::AlignRight, HAlign::Right, "home.alignRight", "Align Right"),
            ] {
                let response = toggle_button(ui, icon, st.align.h == h, tip);
                for sequence in keytip_table::control_badges(app.keytips.prefix().unwrap_or_default(), id) {
                    keytip(app, ui, sequence, response.rect.center_bottom());
                }
                if response.clicked() {
                    act(app, id, json!({}));
                }
            }
            let __k = icon_button(ui, Icon::IndentDec, t.text, "Decrease Indent", vec2(24.0, 23.0));
            control_keytip(app, ui, &__k, "home.decreaseIndent");
            if __k.clicked() {
                act(app, "home.decreaseIndent", json!({}));
            }
            let __k = icon_button(ui, Icon::IndentInc, t.text, "Increase Indent", vec2(24.0, 23.0));
            control_keytip(app, ui, &__k, "home.increaseIndent");
            if __k.clicked() {
                act(app, "home.increaseIndent", json!({}));
            }
            let (m, ma) = split_button(ui, Icon::Merge, None, "Merge & Center");
            control_keytip(app, ui, &ma, "home.mergeCenter");
            if m {
                act(app, "home.mergeCenter", json!({}));
            }
            egui::Popup::menu(&ma).show(|ui| {
                menu_items(
                    app,
                    ui,
                    &[
                        ("Merge & Center", "home.mergeCenter", json!({})),
                        ("Merge Across", "home.mergeAcross", json!({})),
                        ("Merge Cells", "home.mergeCells", json!({})),
                        ("Unmerge Cells", "home.unmergeCells", json!({})),
                    ],
                );
            });
        });
    });
    sep(ui, "Alignment");
    // Number
    stack(ui, |ui| {
        number_combo(app, ui, &st);
        ui.horizontal(|ui| {
            let (c, ca) = split_button(ui, Icon::Currency, None, "Accounting Number Format");
            control_keytip(app, ui, &ca, "home.accounting");
            if c {
                act(app, "home.accounting", json!({}));
            }
            egui::Popup::menu(&ca).show(|ui| {
                let mut items: Vec<_> = ACCOUNTING_SYMBOLS.iter().map(|(label, code)| (*label, "home.numberFormat", json!({"code": code}))).collect();
                items.push(("-", "", json!(null)));
                items.push(("More Accounting Formats…", "dialog:formatCells", json!({"tab": "Number"})));
                menu_items(app, ui, &items);
            });
            let __k = icon_button(ui, Icon::Percent, t.text, "Percent Style", vec2(24.0, 23.0));
            control_keytip(app, ui, &__k, "home.percent");
            if __k.clicked() {
                act(app, "home.percent", json!({}));
            }
            let __k = icon_button(ui, Icon::Comma, t.text, "Comma Style", vec2(24.0, 23.0));
            control_keytip(app, ui, &__k, "home.comma");
            if __k.clicked() {
                act(app, "home.comma", json!({}));
            }
            let __k = icon_button(ui, Icon::DecInc, t.text, "Increase Decimal", vec2(26.0, 23.0));
            control_keytip(app, ui, &__k, "home.increaseDecimal");
            if __k.clicked() {
                act(app, "home.increaseDecimal", json!({}));
            }
            let __k = icon_button(ui, Icon::DecDec, t.text, "Decrease Decimal", vec2(26.0, 23.0));
            control_keytip(app, ui, &__k, "home.decreaseDecimal");
            if __k.clicked() {
                act(app, "home.decreaseDecimal", json!({}));
            }
        });
    });
    sep(ui, "Number");
    // Styles
    let cf = tool(ui, Icon::CondFormat, "Conditional\nFormatting", "Conditional Formatting", true);
    anchor_keytip(app, ui, &cf, &["home.conditionalFormat", "home.manageRules", "home.clearRules"]);
    egui::Popup::menu(&cf).show(|ui| cf_menu(app, ui));
    let ft = tool(ui, Icon::FormatTable, "Format\nas Table", "Format as Table", true);
    anchor_keytip(app, ui, &ft, &["table.style"]);
    egui::Popup::menu(&ft).show(|ui| table_gallery(app, ui, "home.formatAsTable"));
    let cs = tool(ui, Icon::CellStyles, "Cell\nStyles", "Cell Styles", true);
    anchor_keytip(app, ui, &cs, &["home.newCellStyle"]);
    egui::Popup::menu(&cs).show(|ui| cell_style_gallery(app, ui));
    sep(ui, "Styles");
    // Cells
    stack(ui, |ui| {
        let ins = small_button(ui, Icon::Insert, wide(ui, "Insert"), "Insert", true);
        for sequence in keytip_table::group_extension(
            app.keytips.prefix().unwrap_or_default(),
            &["dialog:insertCells", "home.insertRows", "home.insertColumns", "home.insertSheet"],
        ) {
            keytip(app, ui, sequence, ins.rect.center_bottom());
        }
        egui::Popup::menu(&ins).show(|ui| {
            menu_items(
                app,
                ui,
                &[
                    ("Insert Cells…", "dialog:insertCells", json!({})),
                    ("Insert Sheet Rows", "home.insertRows", json!({})),
                    ("Insert Sheet Columns", "home.insertColumns", json!({})),
                    ("Insert Sheet", "home.insertSheet", json!({})),
                ],
            );
        });
        let del = small_button(ui, Icon::Delete, wide(ui, "Delete"), "Delete", true);
        for sequence in keytip_table::group_extension(
            app.keytips.prefix().unwrap_or_default(),
            &["dialog:deleteCells", "home.deleteRows", "home.deleteColumns", "home.deleteSheet"],
        ) {
            keytip(app, ui, sequence, del.rect.center_bottom());
        }
        egui::Popup::menu(&del).show(|ui| {
            menu_items(
                app,
                ui,
                &[
                    ("Delete Cells…", "dialog:deleteCells", json!({})),
                    ("Delete Sheet Rows", "home.deleteRows", json!({})),
                    ("Delete Sheet Columns", "home.deleteColumns", json!({})),
                    ("Delete Sheet", "home.deleteSheet", json!({})),
                ],
            );
        });
        let fmt = small_button(ui, Icon::Format, wide(ui, "Format"), "Format", true);
        for sequence in keytip_table::group_extension(
            app.keytips.prefix().unwrap_or_default(),
            &[
                "dialog:rowHeight",
                "home.autofitRowHeight",
                "dialog:columnWidth",
                "home.autofitColumnWidth",
                "dialog:defaultWidth",
                "home.hideRows",
                "home.hideColumns",
                "home.unhideRows",
                "home.unhideColumns",
                "sheet.hide",
                "sheet.unhide",
                "dialog:renameSheet",
                "dialog:moveSheet",
                "dialog:protectSheet",
                "home.lockCell",
                "dialog:formatCells",
            ],
        ) {
            keytip(app, ui, sequence, fmt.rect.center_bottom());
        }
        let format_keys = matches!(app.keytips.prefix(), Some("HO" | "OC"));
        let mut format_popup = egui::Popup::menu(&fmt);
        if format_keys {
            format_popup = format_popup.open_memory(egui::SetOpenCommand::Bool(true));
        }
        let protect_label = if app.active_sheet_protected() { "Unprotect Sheet…" } else { "Protect Sheet…" };
        format_popup.show(|ui| {
            menu_items(
                app,
                ui,
                &[
                    ("Row Height…", "dialog:rowHeight", json!({})),
                    ("AutoFit Row Height", "home.autofitRowHeight", json!({})),
                    ("Column Width…", "dialog:columnWidth", json!({})),
                    ("AutoFit Column Width", "home.autofitColumnWidth", json!({})),
                    ("Default Width…", "dialog:defaultWidth", json!({})),
                    ("-", "", json!(null)),
                    ("Hide Rows", "home.hideRows", json!({})),
                    ("Hide Columns", "home.hideColumns", json!({})),
                    ("Unhide Rows", "home.unhideRows", json!({})),
                    ("Unhide Columns", "home.unhideColumns", json!({})),
                    ("Hide Sheet", "sheet.hide", json!({})),
                    ("Unhide Sheet…", "sheet.unhide", json!({})),
                    ("-", "", json!(null)),
                    ("Rename Sheet", "dialog:renameSheet", json!({})),
                    ("Move or Copy Sheet…", "dialog:moveSheet", json!({})),
                    ("-", "", json!(null)),
                    (protect_label, "dialog:protectSheet", json!({})),
                    ("Lock Cell", "home.lockCell", json!({})),
                    ("Format Cells…", "dialog:formatCells", json!({})),
                ],
            );
        });
    });
    sep(ui, "Cells");
    // Editing
    stack(ui, |ui| {
        ui.horizontal(|ui| {
            let (s, sa) = split_button(ui, Icon::Sum, None, "AutoSum");
            control_keytip(app, ui, &sa, "formulas.autoSum");
            if s {
                act(app, "formulas.autoSum", json!({}));
            }
            egui::Popup::menu(&sa).show(|ui| autosum_menu(app, ui));
        });
        let fill = small_button(ui, Icon::FillDown, "", "Fill", true);
        for sequence in keytip_table::group_extension(
            app.keytips.prefix().unwrap_or_default(),
            &["edit.fillDown", "edit.fillRight", "edit.fillUp", "edit.fillLeft", "dialog:series", "edit.flashFill"],
        ) {
            keytip(app, ui, sequence, fill.rect.center_bottom());
        }
        egui::Popup::menu(&fill).show(|ui| {
            menu_items(
                app,
                ui,
                &[
                    ("Down", "edit.fillDown", json!({})),
                    ("Right", "edit.fillRight", json!({})),
                    ("Up", "edit.fillUp", json!({})),
                    ("Left", "edit.fillLeft", json!({})),
                    ("Series…", "dialog:series", json!({})),
                    ("Flash Fill", "edit.flashFill", json!({})),
                ],
            );
        });
        let clear = small_button(ui, Icon::Eraser, "", "Clear", true);
        for sequence in keytip_table::group_extension(
            app.keytips.prefix().unwrap_or_default(),
            &["edit.clearAll", "edit.clearFormats", "edit.clearContents", "edit.clearComments", "edit.clearHyperlinks"],
        ) {
            keytip(app, ui, sequence, clear.rect.center_bottom());
        }
        egui::Popup::menu(&clear).show(|ui| {
            menu_items(
                app,
                ui,
                &[
                    ("Clear All", "edit.clearAll", json!({})),
                    ("Clear Formats", "edit.clearFormats", json!({})),
                    ("Clear Contents", "edit.clearContents", json!({})),
                    ("Clear Comments and Notes", "edit.clearComments", json!({})),
                    ("Clear Hyperlinks", "edit.clearHyperlinks", json!({})),
                ],
            );
        });
    });
    let sf = tool(ui, Icon::SortFilter, "Sort &\nFilter", "Sort & Filter", true);
    for sequence in keytip_table::group_extension(
        app.keytips.prefix().unwrap_or_default(),
        &["data.sortAscending", "data.sortDescending", "dialog:sort", "data.filter", "data.clearFilter", "data.reapply"],
    ) {
        keytip(app, ui, sequence, sf.rect.center_bottom());
    }
    egui::Popup::menu(&sf).show(|ui| {
        menu_items(
            app,
            ui,
            &[
                ("Sort A to Z", "data.sortAscending", json!({})),
                ("Sort Z to A", "data.sortDescending", json!({})),
                ("Custom Sort…", "dialog:sort", json!({})),
                ("-", "", json!(null)),
                ("Filter", "data.filter", json!({})),
                ("Clear", "data.clearFilter", json!({})),
                ("Reapply", "data.reapply", json!({})),
            ],
        );
    });
    let fs = tool(ui, Icon::Find, "Find &\nSelect", "Find & Select", true);
    for sequence in keytip_table::group_extension(
        app.keytips.prefix().unwrap_or_default(),
        &["dialog:find", "dialog:goTo", "dialog:goToSpecial", "edit.goToSpecial"],
    ) {
        keytip(app, ui, sequence, fs.rect.center_bottom());
    }
    egui::Popup::menu(&fs).show(|ui| {
        menu_items(
            app,
            ui,
            &[
                ("Find…", "dialog:find", json!({})),
                ("Replace…", "dialog:find", json!({"replace": true})),
                ("Go To…", "dialog:goTo", json!({})),
                ("Go To Special…", "dialog:goToSpecial", json!({})),
                ("-", "", json!(null)),
                ("Formulas", "edit.goToSpecial", json!({"kind": "formulas"})),
                ("Notes", "edit.goToSpecial", json!({"kind": "notes"})),
                ("Conditional Formatting", "edit.goToSpecial", json!({"kind": "conditionalFormats"})),
                ("Constants", "edit.goToSpecial", json!({"kind": "constants"})),
                ("Data Validation", "edit.goToSpecial", json!({"kind": "dataValidation"})),
            ],
        );
    });
}

/// The AutoSum menu (Home › Editing and Formulas › Function Library): a function, or the Insert
/// Function dialog.
pub(super) fn autosum_menu(app: &mut SheetApp, ui: &mut Ui) {
    for f in ["SUM", "AVERAGE", "COUNT", "MAX", "MIN"] {
        if ui.button(f).clicked() {
            act(app, "formulas.autoSum", json!({"function": f}));
        }
    }
    ui.separator();
    if ui.button("More Functions…").clicked() {
        app.open_dialog("insertFunction", json!({}));
    }
}

fn font_combo(app: &mut SheetApp, ui: &mut Ui, st: &Style) {
    let name = st.font.name.clone();
    let resp = ui.add_sized([150.0, 24.0], egui::Button::new(egui::RichText::new(&name).font(theme::ui_font(12.5))));
    control_keytip(app, ui, &resp, "home.fontName");
    let forced = invoke(app, "home.fontName");
    // The standard names, then every installed family; each name drawn in its own font (or the
    // kind of font that stands in for it), a missing one marked as such.
    let families = crate::fonts::families();
    let lang = app.ui.language;
    let missing = lang.tr("not installed");
    let items: Vec<(&str, String)> = families.iter().map(|f| (f.as_str(), f.clone())).collect();
    let extras: Vec<RowExtra> = families
        .iter()
        .map(|f| RowExtra {
            value: f.as_str(),
            mark: Mark::None,
            note: if crate::fonts::installed_now(f) == Some(false) { missing.to_string() } else { String::new() },
            font: Some(theme::cell_family(f, false, false)),
        })
        .collect();
    let picked = marked_combo(ui, &resp, "font", &name, &items, forced, &extras);
    // Bind the installed families in view, so their names show in their own faces.
    let shown = super::controls::take_shown_fonts(ui.ctx());
    if !shown.is_empty() {
        app.preview_fonts(shown);
    }
    if let ComboResult::Picked(value) = picked {
        act(app, "home.fontName", json!({"name": value}));
    }
}

pub const FONTS: &[&str] = &[
    "Aptos",
    "Aptos Narrow",
    "Arial",
    "Calibri",
    "Cambria",
    "Candara",
    "Consolas",
    "Courier New",
    "Garamond",
    "Georgia",
    "Helvetica",
    "Helvetica Neue",
    "Segoe UI",
    "Tahoma",
    "Times New Roman",
    "Trebuchet MS",
    "Verdana",
];

/// Excel's size box: type a size and press Enter to apply exactly that number (the command refuses
/// sizes outside 1 to 409), Escape or clicking away reverts; the arrow opens the standard sizes.
fn size_combo(app: &mut SheetApp, ui: &mut Ui, st: &Style) {
    let t = Tokens::get(ui.ctx());
    let size = st.font.size;
    let current = if size.fract() == 0.0 { format!("{}", size as i32) } else { format!("{size}") };
    let field = egui::Id::unique("mkmatrix.font_size_box");
    let draft = field.with("draft");
    let mut text = ui.data_mut(|d| d.get_temp::<String>(draft)).unwrap_or_else(|| current.clone());
    // One frame like the Font name box beside it (same fill, border, height and corners), with the
    // text field and the dropdown arrow inside it, as in Excel.
    let (frame, _) = ui.allocate_exact_size(vec2(52.0, 24.0), egui::Sense::hover());
    // Painted once the field's focus and hover are known, but beneath the field and the arrow.
    let bg = ui.painter().add(egui::Shape::Noop);
    let text_rect = Rect::from_min_max(frame.min, egui::pos2(frame.right() - 16.0, frame.bottom()));
    let arrow_rect = Rect::from_min_max(egui::pos2(frame.right() - 16.0, frame.top() + 1.0), egui::pos2(frame.right() - 1.0, frame.bottom() - 1.0));
    let mut field_ui =
        ui.new_child(egui::UiBuilder::new().max_rect(text_rect.shrink2(vec2(6.0, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
    let out = egui::TextEdit::singleline(&mut text)
        .id(field)
        .font(theme::ui_font(12.5))
        .frame(egui::Frame::NONE)
        .desired_width(text_rect.width() - 12.0)
        .show(&mut field_ui);
    let resp = out.response;
    ui.ctx().accesskit_node_builder(resp.id, |node| node.set_label(app.ui.language.tr("Font Size")));
    control_keytip(app, ui, &resp, "home.fontSize");
    if invoke(app, "home.fontSize") {
        ui.memory_mut(|m| m.request_focus(field));
    }
    if resp.gained_focus() {
        // Typing replaces the size, as in Excel, whether the box was clicked or reached by keytip.
        let mut state = out.state;
        state.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(text.chars().count()))));
        state.store(ui.ctx(), field);
    }
    if resp.lost_focus() {
        ui.data_mut(|d| d.remove::<String>(draft));
        // Consume the Enter so the grid behind does not also move the active cell down.
        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter))
            && let Some(v) = text.trim().parse::<f64>().ok().filter(|v| v.is_finite())
        {
            act(app, "home.fontSize", json!({"size": v}));
        }
    } else if resp.has_focus() {
        ui.data_mut(|d| d.insert_temp(draft, text));
    } else {
        ui.data_mut(|d| d.remove::<String>(draft));
    }
    let mut arrow_ui = ui.new_child(egui::UiBuilder::new().max_rect(arrow_rect));
    let arrow = icon_button(&mut arrow_ui, Icon::Chevron, t.text_dim, "Font Size", arrow_rect.size());
    let visuals = ui.style().visuals.clone();
    let looks = if resp.has_focus() || ui.rect_contains_pointer(frame) { &visuals.widgets.hovered } else { &visuals.widgets.inactive };
    let stroke = if resp.has_focus() { visuals.selection.stroke } else { looks.bg_stroke };
    ui.painter().set(bg, egui::epaint::RectShape::new(frame, looks.corner_radius, looks.weak_bg_fill, stroke, egui::StrokeKind::Inside));
    arrow.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), "Font Size"));
    let sizes = [8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 22.0, 24.0, 26.0, 28.0, 36.0, 48.0, 72.0];
    let labels: Vec<String> = sizes.iter().map(|s| format!("{}", *s as i32)).collect();
    let items: Vec<(&str, String)> = labels.iter().map(|s| (s.as_str(), s.clone())).collect();
    if let ComboResult::Picked(value) = list_combo(ui, &arrow, frame, "size", &current, &items)
        && let Ok(v) = value.parse::<f32>()
    {
        act(app, "home.fontSize", json!({"size": v}));
    }
}

fn number_combo(app: &mut SheetApp, ui: &mut Ui, st: &Style) {
    let code = st.num_fmt.as_str().to_string();
    let kind = mkmatrix_engine::display::number_format(&code).kind();
    let current = format!("{kind:?}");
    let lang = app.ui.language;
    let sample = app.session.active().and_then(|d| d.wb.active().map(|sh| sh.value(d.selection.active))).unwrap_or_default();
    let resp = ui.add_sized([150.0, 24.0], egui::Button::new(egui::RichText::new(lang.tr(&current)).font(theme::ui_font(12.5))));
    control_keytip(app, ui, &resp, "home.numberFormat");
    let forced = invoke(app, "home.numberFormat");
    // Each format with its symbol, as Excel's list shows them, and the active cell in it.
    let formats: [(&str, Mark); 11] = [
        ("General", Mark::Text("123")),
        ("Number", Mark::Text("12")),
        ("Currency", Mark::Icon(Icon::Currency)),
        ("Accounting", Mark::Icon(Icon::Comma)),
        ("Short Date", Mark::Icon(Icon::Calendar)),
        ("Long Date", Mark::Icon(Icon::Calendar)),
        ("Time", Mark::Icon(Icon::Clock)),
        ("Percentage", Mark::Icon(Icon::Percent)),
        ("Fraction", Mark::Text("½")),
        ("Scientific", Mark::Text("10²")),
        ("Text", Mark::Text("ABC")),
    ];
    let mut items: Vec<(&str, String)> = Vec::new();
    let mut extras: Vec<RowExtra> = Vec::new();
    for (n, mark) in formats {
        let code = mkmatrix_engine::cmd::format::format_code_for(n);
        let preview = app.session.active().map(|d| mkmatrix_engine::display::format(&sample, code, &d.wb).text).unwrap_or_default();
        items.push((n, lang.tr(n).to_string()));
        extras.push(RowExtra { value: n, mark, note: preview.chars().take(28).collect(), font: None });
    }
    items.push(("__more", lang.tr("More Number Formats…").to_string()));
    extras.push(RowExtra { value: "__more", mark: Mark::Icon(Icon::More), note: String::new(), font: None });
    if let ComboResult::Picked(value) = marked_combo(ui, &resp, "number", &current, &items, forced, &extras) {
        match value.as_str() {
            "__more" => app.open_dialog("formatCells", json!({"tab": "Number"})),
            n => act(app, "home.numberFormat", json!({"format": n})),
        }
    }
}

#[cfg(test)]
mod accounting_symbol_tests {
    use super::*;
    use mkmatrix_engine::core::{DateSystem, Value};
    use mkmatrix_engine::numfmt::{NumberFormat, format_value};

    #[test]
    fn accounting_dropdown_offers_the_issue_currencies_with_excel_codes() {
        // #1063: label, Excel's [$symbol-LCID] tag, and 1234.5 as Excel shows it (fill and the
        // `_-` padding aside). The yen and won have no minor unit, so their codes show no decimals.
        for (label, tag, shown) in [
            ("Rp Indonesian", "[$Rp-421]", "Rp1,234.50"),
            ("₹ English (India)", "[$₹-4009]", "₹1,234.50"),
            ("₩ Korean", "[$₩-412]", "₩1,235"),
            ("¥ Chinese (PRC)", "[$¥-804]", "¥1,234.50"),
            ("¥ Japanese", "[$¥-411]", "¥1,235"),
        ] {
            let code = ACCOUNTING_SYMBOLS.iter().find(|(l, _)| *l == label).map(|(_, c)| *c);
            let Some(code) = code else { panic!("{label} is not in the Accounting dropdown") };
            assert!(code.split(';').take(3).all(|section| section.contains(tag)), "{label}: {code}");
            let out = format_value(&Value::Number(1234.5), &NumberFormat::parse(code), DateSystem::D1900);
            assert_eq!(out.text.trim(), shown, "{label}");
            let symbol = tag.trim_start_matches("[$").split('-').next().unwrap_or_default();
            assert_eq!(out.fill.map(|(c, at)| (c, out.text[..at].trim_start())), Some((' ', symbol)), "{label}");
        }
    }
}
