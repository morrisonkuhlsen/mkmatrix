use super::controls::control_keytip;
use super::*;

pub(super) fn automate(app: &mut SheetApp, ui: &mut Ui) {
    if big_button(ui, Icon::Script, "Macros", "Macros and scripts in this workbook (Alt+F8)", false).clicked() {
        app.open_dialog("macros", json!({}));
    }
    let __k = big_button(ui, Icon::Script, "Command\nPalette", "Search and run any command", false);
    control_keytip(app, ui, &__k, "commandSearch");
    if __k.clicked() {
        app.open_dialog("commandSearch", json!({}));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let __k = big_button(ui, Icon::Terminal, "Claude\nCode", "Run Claude Code, Codex or a shell in a terminal connected to this workbook", false);
        control_keytip(app, ui, &__k, "terminal");
        if __k.clicked() {
            app.run_or_alert("view.terminal", json!({"on": true}));
        }
    }
    let __k = big_button(ui, Icon::Script, "Agent\nControl", "How agents drive MK Matrix (MCP / control channel)", false);
    control_keytip(app, ui, &__k, "agents");
    if __k.clicked() {
        app.open_dialog("agents", json!({}));
    }
    let __k = big_button(ui, Icon::Script, "Action\nJournal", "Every command run in this session (replayable)", false);
    control_keytip(app, ui, &__k, "journal");
    if __k.clicked() {
        app.open_dialog("journal", json!({}));
    }
    let __k = big_button(ui, Icon::Script, "Check for\nUpdates…", "Look for a newer MK Matrix release", false);
    control_keytip(app, ui, &__k, "checkForUpdates");
    if __k.clicked() {
        app.run_or_alert("app.checkForUpdates", json!({}));
    }
    let __k = big_button(ui, Icon::Script, "About\nMK Matrix", "Version, contributors and the AI models that helped", false);
    control_keytip(app, ui, &__k, "about");
    if __k.clicked() {
        app.open_dialog("about", json!({}));
    }
}
