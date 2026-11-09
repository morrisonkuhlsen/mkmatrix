//! The Terminal pane's emulator with a real pseudoterminal: a shell started for an agent knows the
//! window's control port, and a program that isn't there says so. No test starts Claude Code or
//! Codex: they would spend the user's credits.
#![cfg(unix)]
// Waiting for the program's output reads the clock: these tests run on the desktop, never on wasm.
#![allow(clippy::disallowed_methods)]
#![allow(clippy::unwrap_used)]

use std::path::Path;
use std::time::{Duration, Instant};

use egui::Color32;
use mkmatrix_ui_egui::terminal::{Agent, Launch, Palette, Program, Terminal};

fn palette() -> Palette {
    Palette::new(false, Color32::BLACK, Color32::WHITE, Color32::BLUE)
}

fn wait_for(terminal: &mut Terminal, ctx: &egui::Context, what: &str) -> String {
    let started = Instant::now();
    loop {
        terminal.poll(ctx, &palette());
        let text = terminal.text(&palette());
        if text.contains(what) {
            return text;
        }
        assert!(started.elapsed() < Duration::from_secs(20), "waiting for {what:?}, the screen says:\n{text}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn folder() -> std::path::PathBuf {
    let folder = std::env::temp_dir().join(format!("mkmatrix-terminal-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

#[test]
fn a_shell_started_for_an_agent_knows_the_windows_port() {
    let folder = folder();
    let ctx = egui::Context::default();
    let agent = Agent { exe: Path::new("/usr/bin/true"), port: 40123, workbook: None };
    let launch = Launch { program: Program::Shell, command: None, folder: &folder, agent: Some(agent), missing: "missing" };
    let mut terminal = Terminal::start(&ctx, &launch, (80, 24), (8, 16)).unwrap();
    terminal.input(b"echo port=$MKMATRIX_CONTROL_PORT\r".to_vec());
    wait_for(&mut terminal, &ctx, "port=40123");
}

#[test]
fn a_program_that_isnt_there_says_so() {
    let folder = folder();
    let ctx = egui::Context::default();
    let launch = Launch {
        program: Program::Codex,
        command: Some("mkmatrix-no-such-agent"),
        folder: &folder,
        agent: None,
        missing: "mkmatrix-no-such-agent isn't installed",
    };
    let mut terminal = Terminal::start(&ctx, &launch, (80, 24), (8, 16)).unwrap();
    wait_for(&mut terminal, &ctx, "isn't installed");
}
