//! Automate › Claude Code: the Terminal pane offers Claude Code, Codex and a shell, and a program
//! started there is connected to this window: the window turns its control channel on and tells
//! the program the port. The test starts only the shell; Claude Code and Codex would spend the
//! user's credits.
#![cfg(unix)]
// Waiting for the program's output reads the clock: these tests run on the desktop, never on wasm.
#![allow(clippy::disallowed_methods)]

use std::time::{Duration, Instant};

use egui_kittest::kittest::Queryable as _;
use mkmatrix_engine::Session;
use serde_json::json;

#[test]
fn the_pane_offers_the_agents_and_connects_what_it_starts() {
    let mut s = Session::new();
    s.new_workbook();
    let mut app = crate::common::app(s);
    // The host's control channel, as the desktop app opens it: here a port and an empty queue.
    app.services.start_control = Some(Box::new(|_| {
        let (_tx, rx) = std::sync::mpsc::channel();
        Ok((40999, rx))
    }));
    let mut h = crate::common::harness_with(crate::common::builder().with_size(egui::vec2(1400.0, 800.0)), app, 3);
    h.state_mut().run("view.terminal", json!({"on": true})).unwrap();
    h.run_steps(3);
    assert!(h.query_by_label("Run an agent beside the workbook").is_some());
    for program in ["Claude Code", "Codex", "Shell"] {
        assert!(h.query_by_label(program).is_some(), "{program} can be started");
    }
    h.get_by_label("Shell").click();
    h.run_steps(3);
    assert_eq!(h.state().terminal_port(), Some(40999), "the shell was told the window's port");
    assert_eq!(h.state().control_port, Some(40999));
    assert!(h.state().control_rx.is_some(), "the window listens for the agent");
    assert!(h.query_by_label("Connected to this window on port 40999").is_some());
    h.state().type_in_terminal("echo port=$MKMATRIX_CONTROL_PORT\r");
    let started = Instant::now();
    loop {
        h.run_steps(1);
        let ctx = h.ctx.clone();
        let text = h.state().terminal_text(&ctx).unwrap_or_default();
        if text.contains("port=40999") {
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(20), "the screen says:\n{text}");
        std::thread::sleep(Duration::from_millis(20));
    }
    h.get_by_label("End the program").click();
    h.run_steps(2);
    assert_eq!(h.state().terminal_port(), None, "ended");
}
