//! The fixed performance scenarios behind `cargo xtask bench` (release build). The workbook is
//! built in code (no fixtures): `rows` × 6 values, a formula column, a chained running sum and
//! a few aggregates; then an edit that recalculates everything below it, a sort and a save.
//!
//! `bench run <rows> <out.xlsx>` prints one JSON object with each step's seconds and the
//! process's peak memory; `bench open <file.xlsx>` opens the saved file in a fresh process and
//! prints its time and peak memory (so the open isn't measured on memory the build left behind).
//! Peak memory is read from `/proc/self/status` (Linux); elsewhere it is `null`.

#![allow(clippy::disallowed_methods)] // a native-only benchmark: wasm never runs it

use std::time::Instant;

use mkmatrix_engine::Session;
use serde_json::{Value as Json, json};

/// The process's peak resident memory in MB, where the OS says (Linux).
fn peak_rss_mb() -> Option<f64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let kb: f64 = status.lines().find(|l| l.starts_with("VmHWM:"))?.split_whitespace().nth(1)?.parse().ok()?;
    Some((kb / 1024.0).round())
}

fn step(s: &mut Session, id: &str, params: Json) -> Result<Json, String> {
    s.execute(id, params).map_err(|e| format!("{id}: {e}"))
}

/// Times `f`, recording it under `name`.
fn timed(steps: &mut Vec<Json>, name: &str, f: impl FnOnce() -> Result<(), String>) -> Result<(), String> {
    let t = Instant::now();
    f()?;
    steps.push(json!({"name": name, "seconds": t.elapsed().as_secs_f64()}));
    Ok(())
}

fn run(rows: usize, out: &str) -> Result<Json, String> {
    if rows == 0 {
        return Err("rows must be at least 1".into());
    }
    let mut s = Session::new();
    s.new_workbook();
    let mut steps = Vec::new();
    timed(&mut steps, "fill values", || {
        let mut r = 0;
        while r < rows {
            let n = 10_000.min(rows - r);
            let vals: Vec<Json> = (0..n)
                .map(|i| json!([format!("Item {}", r + i), (r + i) as f64, ((r + i) % 97) as f64, ((r + i) % 13) as f64 * 1.5, 1.0, 2.0]))
                .collect();
            step(&mut s, "range.setValues", json!({"range": format!("A{}", r + 1), "values": vals}))?;
            r += n;
        }
        Ok(())
    })?;
    timed(&mut steps, "autofill formulas", || {
        step(&mut s, "cell.set", json!({"cell": "G1", "input": "=B1*C1+D1"}))?;
        step(&mut s, "edit.autoFill", json!({"source": "G1", "target": format!("G1:G{rows}")})).map(|_| ())
    })?;
    timed(&mut steps, "autofill chained formulas", || {
        step(&mut s, "cell.set", json!({"cell": "H1", "input": "=G1"}))?;
        if rows > 1 {
            step(&mut s, "cell.set", json!({"cell": "H2", "input": "=H1+G2"}))?;
            step(&mut s, "edit.autoFill", json!({"source": "H2", "target": format!("H2:H{rows}")}))?;
        }
        Ok(())
    })?;
    timed(&mut steps, "aggregate formulas", || {
        step(&mut s, "cell.set", json!({"cell": "J1", "input": format!("=SUM(G1:G{rows})")}))?;
        step(&mut s, "cell.set", json!({"cell": "J2", "input": "=SUMIFS(B:B,C:C,\">50\")"}))?;
        step(&mut s, "cell.set", json!({"cell": "J3", "input": format!("=XLOOKUP(\"Item {}\",A:A,B:B)", rows / 2)})).map(|_| ())
    })?;
    timed(&mut steps, "edit → recalc", || step(&mut s, "cell.set", json!({"cell": "B1", "input": "5"})).map(|_| ()))?;
    let recalculated = s.doc().map(|d| d.calc.last_recalc_cells).unwrap_or(0);
    timed(&mut steps, "sort", || step(&mut s, "data.sortDescending", json!({"range": format!("A1:F{rows}"), "column": "C"})).map(|_| ()))?;
    timed(&mut steps, "save xlsx", || step(&mut s, "file.saveAs", json!({"path": out})).map(|_| ()))?;
    let bytes = std::fs::metadata(out).map(|m| m.len()).unwrap_or(0);
    Ok(json!({"rows": rows, "steps": steps, "recalculated_cells": recalculated, "xlsx_bytes": bytes, "peak_rss_mb": peak_rss_mb()}))
}

fn open(path: &str) -> Result<Json, String> {
    let t = Instant::now();
    let mut s = Session::new();
    step(&mut s, "file.open", json!({"path": path}))?;
    Ok(json!({"seconds": t.elapsed().as_secs_f64(), "peak_rss_mb": peak_rss_mb()}))
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["run", rows, out] => rows.parse().map_err(|_| format!("not a row count: {rows}")).and_then(|r| run(r, out)),
        ["open", path] => open(path),
        _ => Err("usage: bench run <rows> <out.xlsx> | bench open <file.xlsx>".into()),
    };
    match result {
        Ok(v) => {
            println!("{v}");
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("bench: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
