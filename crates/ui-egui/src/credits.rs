//! The About window's credits: every contributor (GitHub login always; display and real names only
//! with recorded consent) and the AI models that helped. The tables are baked into the binary by
//! build.rs from contributors/contributors.json (craftrules standards/contributors.md); nothing is
//! read at run time.

use std::cmp::Ordering;

use egui::RichText;

use crate::icons::Icon;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Contributor {
    pub login: &'static str,
    pub display_name: Option<&'static str>,
    pub real_name: Option<&'static str>,
    pub prs: u64,
    pub commits: u64,
    pub lines_added: u64,
    pub lines_deleted: u64,
    pub binary_added: u64,
    pub binary_deleted: u64,
    /// ISO 8601 UTC; empty for someone credited only through a merged PR.
    pub first_commit: &'static str,
    pub last_commit: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Model {
    pub company: &'static str,
    pub model: &'static str,
    pub version: &'static str,
    pub commits: u64,
    pub lines_added: u64,
    pub lines_deleted: u64,
}

include!(concat!(env!("OUT_DIR"), "/credits.rs"));

/// Which name to show. Display and real names fall back to the `@username` when not given.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NameMode {
    #[default]
    Username,
    DisplayName,
    RealName,
}

impl NameMode {
    pub const ALL: [NameMode; 3] = [NameMode::Username, NameMode::DisplayName, NameMode::RealName];

    pub fn label(self) -> &'static str {
        match self {
            NameMode::Username => "Username",
            NameMode::DisplayName => "Display name",
            NameMode::RealName => "Real name",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Prs,
    Commits,
    LinesAdded,
    LinesDeleted,
    LinesDelta,
    BinaryAdded,
    BinaryDeleted,
    #[default]
    FirstCommit,
    LastCommit,
}

impl SortKey {
    pub const ALL: [SortKey; 10] = [
        SortKey::Name,
        SortKey::Prs,
        SortKey::Commits,
        SortKey::LinesAdded,
        SortKey::LinesDeleted,
        SortKey::LinesDelta,
        SortKey::BinaryAdded,
        SortKey::BinaryDeleted,
        SortKey::FirstCommit,
        SortKey::LastCommit,
    ];

    /// Menu label and table header.
    pub fn label(self) -> (&'static str, &'static str) {
        match self {
            SortKey::Name => ("Name (A–Z)", "Name"),
            SortKey::Prs => ("Merged PRs", "PRs"),
            SortKey::Commits => ("Commits", "Commits"),
            SortKey::LinesAdded => ("Lines added", "+LOC"),
            SortKey::LinesDeleted => ("Lines deleted", "−LOC"),
            SortKey::LinesDelta => ("Line delta", "ΔLOC"),
            SortKey::BinaryAdded => ("Binary assets added", "+Bin"),
            SortKey::BinaryDeleted => ("Binary assets removed", "−Bin"),
            SortKey::FirstCommit => ("First commit", "First"),
            SortKey::LastCommit => ("Last commit", "Last"),
        }
    }

    /// Names and dates read naturally oldest/A first; counts read biggest first.
    pub fn default_ascending(self) -> bool {
        matches!(self, SortKey::Name | SortKey::FirstCommit | SortKey::LastCommit)
    }
}

impl Contributor {
    pub fn name(&self, mode: NameMode) -> String {
        let chosen = match mode {
            NameMode::Username => None,
            NameMode::DisplayName => self.display_name,
            NameMode::RealName => self.real_name,
        };
        chosen.map_or_else(|| format!("@{}", self.login), str::to_string)
    }

    pub fn lines_delta(&self) -> i128 {
        i128::from(self.lines_added) - i128::from(self.lines_deleted)
    }

    /// One line with everything we know, for tooltips.
    pub fn summary(&self) -> String {
        format!(
            "@{}: {} PRs, {} commits, +{} / −{} lines (Δ {}), +{} / −{} binary assets, {} – {}",
            self.login,
            self.prs,
            self.commits,
            group(self.lines_added),
            group(self.lines_deleted),
            signed(self.lines_delta()),
            self.binary_added,
            self.binary_deleted,
            day(self.first_commit),
            day(self.last_commit),
        )
    }
}

/// Alphabetical key: case-insensitive, ignoring the `@` of usernames.
fn name_key(name: &str) -> String {
    name.trim_start_matches('@').to_lowercase()
}

fn compare(a: &Contributor, b: &Contributor, mode: NameMode, key: SortKey) -> Ordering {
    match key {
        SortKey::Name => name_key(&a.name(mode)).cmp(&name_key(&b.name(mode))),
        SortKey::Prs => a.prs.cmp(&b.prs),
        SortKey::Commits => a.commits.cmp(&b.commits),
        SortKey::LinesAdded => a.lines_added.cmp(&b.lines_added),
        SortKey::LinesDeleted => a.lines_deleted.cmp(&b.lines_deleted),
        SortKey::LinesDelta => a.lines_delta().cmp(&b.lines_delta()),
        SortKey::BinaryAdded => a.binary_added.cmp(&b.binary_added),
        SortKey::BinaryDeleted => a.binary_deleted.cmp(&b.binary_deleted),
        // A PR-only contributor (no commit date) sorts after everyone with a date.
        SortKey::FirstCommit => dated(a.first_commit).cmp(&dated(b.first_commit)),
        SortKey::LastCommit => dated(a.last_commit).cmp(&dated(b.last_commit)),
    }
}

fn dated(d: &str) -> (bool, &str) {
    (d.is_empty(), d)
}

/// Contributors in display order. Ties fall back to the name (A–Z) so the order is stable.
pub fn sorted(list: &[Contributor], mode: NameMode, key: SortKey, ascending: bool) -> Vec<&Contributor> {
    let mut v: Vec<&Contributor> = list.iter().collect();
    v.sort_by(|a, b| {
        let o = compare(a, b, mode, key);
        let o = if ascending { o } else { o.reverse() };
        o.then_with(|| compare(a, b, mode, SortKey::Name)).then_with(|| a.login.cmp(b.login))
    });
    v
}

pub(crate) fn group(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

fn signed(n: i128) -> String {
    let g = group(u64::try_from(n.unsigned_abs()).unwrap_or(u64::MAX));
    if n < 0 { format!("−{g}") } else { format!("+{g}") }
}

fn day(iso: &str) -> &str {
    if iso.is_empty() { "—" } else { iso.get(..10).unwrap_or(iso) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct View {
    names: NameMode,
    key: SortKey,
    ascending: bool,
    table: bool,
}

impl Default for View {
    fn default() -> Self {
        View { names: NameMode::Username, key: SortKey::FirstCommit, ascending: true, table: false }
    }
}

/// The credits in three figures: contributors, commits, and AI models credited.
pub fn totals() -> (u64, u64, u64) {
    (CONTRIBUTORS.len() as u64, TOTAL_COMMITS, MODELS.len() as u64)
}

/// About ▸ Contributors, in Format Cells' style: which name to show, the sort and the view as a
/// form of segments, then the list in a panel as a grab bag or a table.
pub fn contributors_ui(ui: &mut egui::Ui) {
    use crate::dialogs::kit::{form, heading, hint, label, panel, segments};
    let lang = crate::i18n::current(ui.ctx());
    let id = egui::Id::unique("credits_view");
    let mut v = ui.data_mut(|d| d.get_temp::<View>(id)).unwrap_or_default();
    heading(ui, lang.tr("Contributors"));
    form(ui, "credits.form", |ui| {
        label(ui, lang.tr("Show"));
        let names: Vec<(&str, &str)> = NameMode::ALL.iter().map(|m| (m.label(), lang.tr(m.label()))).collect();
        if let Some(picked) = segments(ui, v.names.label(), &names, 112.0) {
            v.names = NameMode::ALL.into_iter().find(|m| m.label() == picked).unwrap_or(v.names);
        }
        ui.end_row();
        label(ui, lang.tr("Sort"));
        ui.horizontal(|ui| {
            craft_ui::ComboBox::from_id_salt("credits_sort").width(180.0).selected_text(lang.tr(v.key.label().0)).show_ui(ui, |ui| {
                for k in SortKey::ALL {
                    if ui.selectable_label(v.key == k, lang.tr(k.label().0)).clicked() {
                        v.key = k;
                        v.ascending = k.default_ascending();
                    }
                }
            });
            let order = if v.ascending { Icon::SortAsc } else { Icon::SortDesc };
            if crate::widgets::icon_button(ui, order, crate::theme::Tokens::get(ui.ctx()).text, "Reverse the order", egui::vec2(28.0, 28.0)).clicked()
            {
                v.ascending = !v.ascending;
            }
        });
        ui.end_row();
        label(ui, lang.tr("View"));
        if let Some(view) = segments(ui, if v.table { "table" } else { "bag" }, &[("bag", lang.tr("Grab bag")), ("table", lang.tr("Table"))], 112.0) {
            v.table = view == "table";
        }
        ui.end_row();
    });
    let list = sorted(CONTRIBUTORS, v.names, v.key, v.ascending);
    hint(
        ui,
        &lang
            .tr("{count} contributors · {commits} commits")
            .replace("{count}", &group(list.len() as u64))
            .replace("{commits}", &group(TOTAL_COMMITS)),
    );
    let height = ui.available_height().max(120.0);
    panel(ui, height, |ui| {
        egui::Frame::NONE.inner_margin(10.0).show(ui, |ui| {
            egui::ScrollArea::both().id_salt("credits.list").auto_shrink([false, false]).show(ui, |ui| {
                if list.is_empty() {
                    hint(ui, lang.tr("No contributor data was built into this copy."));
                } else if v.table {
                    table(ui, &list, &mut v);
                } else {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(10.0, 6.0);
                        for c in &list {
                            ui.hyperlink_to(c.name(v.names), format!("https://github.com/{}", c.login)).on_hover_text(c.summary());
                        }
                    });
                }
            });
        });
    });
    ui.data_mut(|d| d.insert_temp(id, v));
}

fn table(ui: &mut egui::Ui, list: &[&Contributor], v: &mut View) {
    egui::Grid::new("credits_table").striped(true).num_columns(SortKey::ALL.len()).show(ui, |ui| {
        for k in SortKey::ALL {
            let arrow = if v.key == k { if v.ascending { " ▲" } else { " ▼" } } else { "" };
            if ui.button(RichText::new(format!("{}{arrow}", k.label().1)).strong()).on_hover_text(k.label().0).clicked() {
                if v.key == k {
                    v.ascending = !v.ascending;
                } else {
                    v.key = k;
                    v.ascending = k.default_ascending();
                }
            }
        }
        ui.end_row();
        for c in list {
            ui.hyperlink_to(c.name(v.names), format!("https://github.com/{}", c.login)).on_hover_text(c.summary());
            ui.label(group(c.prs));
            ui.label(group(c.commits));
            ui.label(group(c.lines_added));
            ui.label(group(c.lines_deleted));
            ui.label(signed(c.lines_delta()));
            ui.label(group(c.binary_added));
            ui.label(group(c.binary_deleted));
            ui.label(day(c.first_commit));
            ui.label(day(c.last_commit));
            ui.end_row();
        }
    });
}

/// About ▸ Models: the AI models credited in Co-Authored-By trailers, each a row with its share of
/// the commits drawn as a bar in the accent.
pub fn models_ui(ui: &mut egui::Ui) {
    use crate::dialogs::kit::{heading, hint, panel};
    let lang = crate::i18n::current(ui.ctx());
    let t = crate::theme::Tokens::get(ui.ctx());
    heading(ui, lang.tr("AI models"));
    if MODELS.is_empty() {
        hint(ui, lang.tr("No model credits were built into this copy."));
        return;
    }
    let assisted: u64 = MODELS.iter().map(|m| m.commits).max().unwrap_or(0).max(1);
    let all = TOTAL_COMMITS.max(assisted) as f32;
    let height = ui.available_height().max(120.0);
    panel(ui, height, |ui| {
        egui::ScrollArea::vertical().id_salt("credits.models").auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for m in MODELS {
                let (r, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 52.0), egui::Sense::hover());
                let p = ui.painter();
                if resp.hovered() {
                    p.rect_filled(r, 0.0, t.hover);
                }
                let name = if m.version.is_empty() { m.model.to_string() } else { format!("{} {}", m.model, m.version) };
                p.text(egui::pos2(r.left() + 12.0, r.top() + 10.0), egui::Align2::LEFT_TOP, &name, crate::theme::ui_bold(13.5), t.text);
                p.text(egui::pos2(r.left() + 12.0, r.bottom() - 10.0), egui::Align2::LEFT_BOTTOM, m.company, crate::theme::ui_font(12.0), t.text_dim);
                let share = m.commits as f32 / all;
                let lines = format!("+{} / −{}", group(m.lines_added), group(m.lines_deleted));
                let commits = lang.tr("{count} commits").replace("{count}", &group(m.commits));
                p.text(
                    egui::pos2(r.right() - 12.0, r.top() + 10.0),
                    egui::Align2::RIGHT_TOP,
                    format!("{commits} · {:.0}%", 100.0 * share),
                    crate::theme::ui_font(12.5),
                    t.text,
                );
                p.text(egui::pos2(r.right() - 12.0, r.bottom() - 10.0), egui::Align2::RIGHT_BOTTOM, lines, crate::theme::ui_font(11.5), t.text_dim);
                // The share of all commits, as a bar between the name and the figures.
                let track =
                    egui::Rect::from_min_max(egui::pos2(r.left() + 190.0, r.center().y - 2.0), egui::pos2(r.right() - 190.0, r.center().y + 2.0));
                if track.width() > 20.0 {
                    p.rect_filled(track, 2.0, t.separator);
                    p.rect_filled(
                        egui::Rect::from_min_size(track.min, egui::vec2(track.width() * share.clamp(0.0, 1.0), track.height())),
                        2.0,
                        t.accent,
                    );
                }
                p.line_segment([r.left_bottom(), r.right_bottom()], egui::Stroke::new(1.0, t.separator));
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(
        login: &'static str,
        display: Option<&'static str>,
        real: Option<&'static str>,
        added: u64,
        deleted: u64,
        first: &'static str,
    ) -> Contributor {
        Contributor {
            login,
            display_name: display,
            real_name: real,
            prs: 0,
            commits: 1,
            lines_added: added,
            lines_deleted: deleted,
            binary_added: 0,
            binary_deleted: 0,
            first_commit: first,
            last_commit: first,
        }
    }

    fn people() -> [Contributor; 4] {
        [
            c("zed", None, None, 5, 50, "2026-01-02T00:00:00Z"),
            c("Bob", Some("Ann"), None, 10, 0, "2026-01-01T00:00:00Z"),
            c("alice", None, None, 7, 1, ""),
            c("carol", None, Some("Dana"), 1, 0, "2026-01-03T00:00:00Z"),
        ]
    }

    fn logins(v: &[&Contributor]) -> Vec<&'static str> {
        v.iter().map(|c| c.login).collect()
    }

    #[test]
    fn names_fall_back_to_the_username() {
        let p = c("bob", Some("Bobby"), None, 0, 0, "");
        assert_eq!(p.name(NameMode::Username), "@bob");
        assert_eq!(p.name(NameMode::DisplayName), "Bobby");
        assert_eq!(p.name(NameMode::RealName), "@bob");
    }

    #[test]
    fn alphabetical_ignores_at_and_case() {
        let all = people();
        let v = sorted(&all, NameMode::Username, SortKey::Name, true);
        assert_eq!(logins(&v), ["alice", "Bob", "carol", "zed"]);
        let v = sorted(&all, NameMode::DisplayName, SortKey::Name, true);
        assert_eq!(logins(&v), ["alice", "Bob", "carol", "zed"]); // "Ann" sorts with the @usernames
        let v = sorted(&all, NameMode::RealName, SortKey::Name, false);
        assert_eq!(logins(&v), ["zed", "carol", "Bob", "alice"]); // "Dana" between bob and zed
    }

    #[test]
    fn delta_and_dates() {
        let all = people();
        let v = sorted(&all, NameMode::Username, SortKey::LinesDelta, false);
        assert_eq!(logins(&v), ["Bob", "alice", "carol", "zed"]);
        let v = sorted(&all, NameMode::Username, SortKey::FirstCommit, true);
        assert_eq!(logins(&v), ["Bob", "zed", "carol", "alice"]); // undated last
    }

    #[test]
    fn formatting() {
        assert_eq!(group(1234567), "1,234,567");
        assert_eq!(group(12), "12");
        assert_eq!(signed(-1000), "−1,000");
        assert_eq!(day("2026-10-07T18:45:23Z"), "2026-10-07");
        assert_eq!(day(""), "—");
    }
}
