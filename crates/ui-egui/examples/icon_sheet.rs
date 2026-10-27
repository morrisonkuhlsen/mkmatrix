//! Renders every icon in a grid, at 16 and 32 points with its name, in both themes, to a PNG.
//!
//! `cargo run -p mkmatrix-ui-egui --example icon_sheet -- [--dark] out.png`

use mkmatrix_ui_egui::icons::{self, Icon};
use mkmatrix_ui_egui::theme::Tokens;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dark = args.iter().any(|a| a == "--dark");
    let out = args.iter().rfind(|a| !a.starts_with("--")).cloned().unwrap_or_else(|| "icons.png".into());
    let cols = 10usize;
    let cell = egui::vec2(132.0, 70.0);
    let rows = Icon::ALL.len().div_ceil(cols);
    let size = egui::vec2(cols as f32 * cell.x, rows as f32 * cell.y);
    let mut harness = egui_kittest::Harness::builder().with_size(size).with_pixels_per_point(2.0).wgpu().build_ui(move |ui| {
        if ui.ctx().cumulative_pass_nr() == 0 {
            mkmatrix_ui_egui::SheetApp::setup_context(ui.ctx(), dark);
        }
        let t = if dark { Tokens::dark() } else { Tokens::light() };
        let rect = ui.max_rect();
        let p = ui.painter();
        p.rect_filled(rect, 0.0, t.ribbon);
        for (i, icon) in Icon::ALL.iter().enumerate() {
            let at = rect.min + egui::vec2((i % cols) as f32 * cell.x, (i / cols) as f32 * cell.y);
            let small = egui::Rect::from_min_size(at + egui::vec2(8.0, 12.0), egui::vec2(16.0, 16.0));
            let big = egui::Rect::from_min_size(at + egui::vec2(34.0, 4.0), egui::vec2(32.0, 32.0));
            icons::paint(p, small, *icon, t.text);
            icons::paint(p, big, *icon, t.text);
            p.text(at + egui::vec2(8.0, 46.0), egui::Align2::LEFT_TOP, format!("{icon:?}"), egui::FontId::proportional(10.0), t.text_dim);
        }
    });
    harness.run_steps(3);
    let img = harness.render().expect("render");
    img.save(&out).expect("save");
    eprintln!("wrote {out}");
}
