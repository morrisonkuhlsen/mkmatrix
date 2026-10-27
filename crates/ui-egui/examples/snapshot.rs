//! Renders the whole MK Matrix window offscreen (wgpu, no window) to a PNG.
//!
//! `cargo run --release -p mkmatrix-ui-egui --example snapshot -- [--sample sales] [--in file.xlsx]
//!  [--size 1440x900] [--scale 2] [--cmd 'id={json}']... [--tab Insert] [--dark] [--language en|es|zh|ja|ko|ru|pt|de|cs|fr|tr]
//!  [--dialog 'name={json}'] [--hf-edit header|footer] [--splash] [--backstage] [--density normal|compact|minimal] [--ui 'id={json}']... out.png`
//!
//! `--splash` renders the splash screen alone, at its own size.

use mkmatrix_engine::Session;
use mkmatrix_ui_egui::SheetApp;
use serde_json::json;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut sample = None;
    let mut input = None;
    let mut size = (1440.0f32, 900.0f32);
    let mut scale = 2.0f32;
    let mut cmds: Vec<String> = vec![];
    let mut tab = None;
    let mut dark = false;
    let mut pane: Option<String> = None;
    let mut chart: Option<u32> = None;
    let mut dialog: Option<String> = None;
    let mut language: Option<String> = None;
    let mut hf_edit: Option<String> = None;
    let mut splash = false;
    let mut backstage = false;
    let mut density: Option<String> = None;
    let mut ui_cmds: Vec<String> = vec![];
    let mut out = "snapshot.png".to_string();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--sample" => {
                i += 1;
                sample = args.get(i).cloned();
            }
            "--in" => {
                i += 1;
                input = args.get(i).cloned();
            }
            "--size" => {
                i += 1;
                if let Some((w, h)) = args.get(i).and_then(|s| s.split_once('x')) {
                    size = (w.parse().unwrap_or(1440.0), h.parse().unwrap_or(900.0));
                }
            }
            "--scale" => {
                i += 1;
                scale = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(2.0);
            }
            "--cmd" => {
                i += 1;
                if let Some(c) = args.get(i) {
                    cmds.push(c.clone());
                }
            }
            "--tab" => {
                i += 1;
                tab = args.get(i).cloned();
            }
            "--dark" => dark = true,
            "--pane" => {
                i += 1;
                pane = args.get(i).cloned();
            }
            "--chart" => {
                i += 1;
                chart = args.get(i).and_then(|s| s.parse().ok());
            }
            "--dialog" => {
                i += 1;
                dialog = args.get(i).cloned();
            }
            "--hf-edit" => {
                i += 1;
                hf_edit = args.get(i).cloned();
            }
            "--language" => {
                i += 1;
                language = args.get(i).cloned();
            }
            "--splash" => splash = true,
            "--backstage" => backstage = true,
            "--ui" => {
                i += 1;
                if let Some(c) = args.get(i) {
                    ui_cmds.push(c.clone());
                }
            }
            "--density" => {
                i += 1;
                density = args.get(i).cloned();
            }
            other => out = other.to_string(),
        }
        i += 1;
    }
    if splash {
        let language = language.as_deref().and_then(mkmatrix_ui_egui::i18n::Language::parse).unwrap_or_default();
        let [w, h] = mkmatrix_ui_egui::splash::SIZE;
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(w, h))
            .with_pixels_per_point(scale)
            .with_step_dt(0.4)
            .with_max_steps(4)
            .wgpu()
            .build_ui_state(|ui, s: &mut mkmatrix_ui_egui::splash::SplashScreen| s.ui(ui), mkmatrix_ui_egui::splash::SplashScreen::new(language));
        // Part-way through, so the bar and a part's name show.
        harness.run_steps(3);
        let img = harness.render().expect("render");
        img.save(&out).expect("save png");
        eprintln!("wrote {out} ({}x{})", img.width(), img.height());
        return;
    }
    // Catalog the installed fonts first (the app scans in the background from its first frame), so
    // the workbook's fonts are drawn in their own faces, not a substitute.
    let _ = mkmatrix_ui_egui::fonts::catalog();
    let mut session = Session::new();
    if let Some(p) = &input {
        session.execute("file.open", json!({"path": p})).expect("open");
    } else if let Some(s) = &sample {
        session.execute("file.new", json!({"sample": s})).expect("sample");
    } else {
        session.new_workbook();
    }
    for c in &cmds {
        let (id, p) = c.split_once('=').map(|(a, b)| (a.to_string(), serde_json::from_str(b).unwrap_or(json!({})))).unwrap_or((c.clone(), json!({})));
        if let Err(e) = session.execute(&id, p) {
            eprintln!("{id}: {e}");
        }
    }
    let mut harness =
        egui_kittest::Harness::builder().with_size(egui::vec2(size.0, size.1)).with_pixels_per_point(scale).with_max_steps(12).wgpu().build_ui_state(
            move |ui, app: &mut SheetApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            {
                let mut app = SheetApp::new(session, Default::default());
                app.ui.dark = dark;
                if let Some(code) = &language
                    && let Some(l) = mkmatrix_ui_egui::i18n::Language::parse(code)
                {
                    app.ui.language = l;
                }
                if let Some(t) = tab {
                    app.ui.ribbon_tab = t;
                }
                for c in &ui_cmds {
                    let (id, p) = c
                        .split_once('=')
                        .map(|(a, b)| (a.to_string(), serde_json::from_str(b).unwrap_or(json!({}))))
                        .unwrap_or((c.clone(), json!({})));
                    if let Err(e) = app.run(&id, p) {
                        eprintln!("{id}: {e}");
                    }
                }
                if let Some(d) = &density {
                    let _ = app.run("view.density", json!({"mode": d}));
                }
                if backstage {
                    app.open_backstage(mkmatrix_ui_egui::backstage::Page::Home);
                }
                app.grid.pane = pane;
                app.selected_chart = chart;
                if let Some(d) = &dialog {
                    let (name, p) =
                        d.split_once('=').map(|(a, b)| (a, serde_json::from_str(b).unwrap_or(json!({})))).unwrap_or((d.as_str(), json!({})));
                    app.open_dialog(name, p);
                }
                // Page Layout view: edit the first page's centre header (or footer) section.
                let active = app.session.active().and_then(|d| d.wb.active().cloned().map(|s| (d.wb.active_sheet(), s)));
                if let (Some(part), Some((si, sh))) = (&hf_edit, active) {
                    let zone = mkmatrix_engine::paper::HfZone { footer: part == "footer", section: 1 };
                    mkmatrix_ui_egui::paper_view::begin(&mut app, si, &sh, (0, 0), zone);
                }
                app
            },
        );
    // The workbook's installed fonts bind once the font scan is done; frames past the first few
    // apply them, so the snapshot shows them as the app does.
    harness.run_steps(14);
    let img = harness.render().expect("render");
    img.save(&out).expect("save png");
    eprintln!("wrote {out} ({}x{})", img.width(), img.height());
}
