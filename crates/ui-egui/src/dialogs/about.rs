use super::kit::{button, nav_item, nav_rail, rule};
use super::*;

use egui::pos2;

use crate::theme::Tokens;

/// The sections, as `d.tab` names them, with their icon.
const SECTIONS: [(&str, Icon); 3] = [("About", Icon::Info), ("Contributors", Icon::Name), ("Models", Icon::Script)];
/// The body's height: every section the same size, so the window never jumps.
const BODY_H: f32 = 400.0;
/// The navigation rail's width.
const NAV_W: f32 = 168.0;

/// Help ▸ About MK Matrix, in Format Cells' style: About, Contributors and Models down the left,
/// one at a time on the right, and Close in the footer. The credits are compiled in
/// (`crate::credits`, docs/contributors.md).
pub(super) fn about(ui: &mut egui::Ui, d: &mut Dialog, open: &mut bool) {
    let lang = crate::i18n::current(ui.ctx());
    let t = Tokens::get(ui.ctx());
    ui.horizontal_top(|ui| {
        let names: Vec<&str> = SECTIONS.iter().map(|(s, _)| lang.tr(s)).collect();
        nav_rail(ui, super::kit::nav_width(ui, &names, NAV_W), BODY_H, &t, |ui| {
            for (tab, icon) in SECTIONS {
                if nav_item(ui, icon, lang.tr(tab), d.tab == tab, None, &t).clicked() {
                    d.tab = tab.to_string();
                }
            }
        });
        ui.vertical(|ui| {
            ui.set_min_height(BODY_H);
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 8.0;
            match d.tab.as_str() {
                "Contributors" => crate::credits::contributors_ui(ui, BODY_H),
                "Models" => crate::credits::models_ui(ui, BODY_H),
                _ => about_page(ui, &t),
            }
        });
    });
    rule(ui);
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if button(ui, lang.tr("Close"), true).clicked() {
                *open = false;
            }
        });
    });
}

/// The app, its version and house, and the credits in three figures.
fn about_page(ui: &mut egui::Ui, t: &Tokens) {
    let lang = crate::i18n::current(ui.ctx());
    ui.add_space(6.0);
    ui.label(egui::RichText::new("MK Matrix").font(theme::ui_bold(26.0)).color(t.text));
    ui.label(
        egui::RichText::new(lang.tr("Version {version}").replace("{version}", env!("CARGO_PKG_VERSION")))
            .font(theme::ui_font(13.0))
            .color(t.text_dim),
    );
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(lang.tr("A spreadsheet of matrices: open-source and Rust-native, forked from GridCraft.")).font(theme::ui_font(13.5)),
    );
    ui.label(egui::RichText::new(lang.tr("A Morrison Kühlsen project.")).font(theme::ui_font(13.5)).color(t.text_dim));
    ui.hyperlink_to("morrisonkuhlsen.com", "https://morrisonkuhlsen.com");
    ui.add_space(14.0);
    let (people, commits, models) = crate::credits::totals();
    let figures = [(people, lang.tr("contributors")), (commits, lang.tr("commits")), (models, lang.tr("AI models"))];
    let gap = 10.0;
    let w = (ui.available_width() - 2.0 * gap) / 3.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for (n, what) in figures {
            let (r, _) = ui.allocate_exact_size(vec2(w, 72.0), egui::Sense::hover());
            let p = ui.painter();
            p.rect_filled(r, 4.0, t.input_bg);
            p.rect_filled(egui::Rect::from_min_size(r.min, vec2(3.0, r.height())), 0.0, t.accent);
            p.text(pos2(r.left() + 16.0, r.top() + 14.0), egui::Align2::LEFT_TOP, crate::credits::group(n), theme::ui_bold(22.0), t.accent);
            p.text(pos2(r.left() + 16.0, r.bottom() - 14.0), egui::Align2::LEFT_BOTTOM, what, theme::ui_font(12.0), t.text_dim);
        }
    });
}
