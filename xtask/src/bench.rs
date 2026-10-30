//! `cargo xtask bench [--rows N] [--check] [--write]`: the fixed performance scenarios of
//! `crates/engine/examples/bench.rs`, built in release and run in two processes (build, edit,
//! sort and save; then a fresh open of the saved file), as a table.
//!
//! `--check` fails when a measure exceeds its limit in `bench/limits.json` (per row count):
//! generous limits, so a slower CI machine passes and only a real regression fails. `--write`
//! records the run in `docs/bench.md`, one section per row count, with the commit and machine.

use std::path::{Path, PathBuf};

use serde_json::Value as Json;

/// One measured number: its name, value and unit.
struct Measure {
    name: String,
    value: f64,
    unit: &'static str,
}

pub fn run(root: &Path, args: &[&str]) -> Result<(), String> {
    let mut rows: usize = 100_000;
    let (mut check, mut write) = (false, false);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match *a {
            "--rows" => rows = it.next().and_then(|n| n.replace('_', "").parse().ok()).ok_or("--rows takes a number")?,
            "--check" => check = true,
            "--write" => write = true,
            other => return Err(format!("bench: unknown option `{other}` (--rows N, --check, --write)")),
        }
    }
    let exe = build()?;
    let file = std::env::temp_dir().join(format!("mkmatrix-bench-{}-{rows}.xlsx", std::process::id()));
    let result = measure(&exe, rows, &file);
    let _ = std::fs::remove_file(&file);
    let measures = result?;
    println!("{}", table(rows, &measures));
    if write {
        record(root, rows, &measures)?;
    }
    if check {
        return check_limits(root, rows, &measures);
    }
    Ok(())
}

/// Builds the bench example in release; its path.
fn build() -> Result<PathBuf, String> {
    let mut c = super::cargo();
    c.args(["build", "--release", "-p", "mkmatrix-engine", "--example", "bench"]);
    super::run(c, "cargo build --release -p mkmatrix-engine --example bench")?;
    let target = super::metadata()?["target_directory"].as_str().map(PathBuf::from).ok_or("cargo metadata: no target_directory")?;
    Ok(target.join("release").join("examples").join(format!("bench{}", std::env::consts::EXE_SUFFIX)))
}

/// Runs the example and parses the JSON it prints.
fn invoke(exe: &Path, args: &[&str]) -> Result<Json, String> {
    let out = std::process::Command::new(exe).args(args).output().map_err(|e| format!("{}: {e}", exe.display()))?;
    if !out.status.success() {
        return Err(format!("bench {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim()));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("bench {}: bad JSON: {e}", args.join(" ")))
}

fn measure(exe: &Path, rows: usize, file: &Path) -> Result<Vec<Measure>, String> {
    let path = file.to_str().ok_or("temporary path isn't UTF-8")?;
    let built = invoke(exe, &["run", &rows.to_string(), path])?;
    let opened = invoke(exe, &["open", path])?;
    let mut m: Vec<Measure> = built["steps"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| Some(Measure { name: s["name"].as_str()?.to_string(), value: s["seconds"].as_f64()?, unit: "s" }))
        .collect();
    if let Some(s) = opened["seconds"].as_f64() {
        m.push(Measure { name: "open xlsx (fresh process)".into(), value: s, unit: "s" });
    }
    if let Some(mb) = built["peak_rss_mb"].as_f64() {
        m.push(Measure { name: "peak memory: build to save".into(), value: mb, unit: "MB" });
    }
    if let Some(mb) = opened["peak_rss_mb"].as_f64() {
        m.push(Measure { name: "peak memory: open".into(), value: mb, unit: "MB" });
    }
    if let Some(b) = built["xlsx_bytes"].as_f64() {
        m.push(Measure { name: "xlsx size".into(), value: (b / 1_048_576.0 * 10.0).round() / 10.0, unit: "MB" });
    }
    Ok(m)
}

fn fmt(m: &Measure) -> String {
    match m.unit {
        "s" if m.value < 1.0 => format!("{:.0} ms", m.value * 1000.0),
        "s" => format!("{:.2} s", m.value),
        unit => format!("{} {unit}", m.value),
    }
}

/// A Markdown table of the run.
fn table(rows: usize, measures: &[Measure]) -> String {
    let mut out = format!("| {rows} rows | |\n|---|---|\n");
    for m in measures {
        out.push_str(&format!("| {} | {} |\n", m.name, fmt(m)));
    }
    out
}

/// Fails on any measure over its limit in `bench/limits.json` (`{"<rows>": {"<name>": max}}`,
/// seconds or MB). A measure without a limit, or a row count without limits, is reported, not failed.
fn check_limits(root: &Path, rows: usize, measures: &[Measure]) -> Result<(), String> {
    let path = root.join("bench/limits.json");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let limits: Json = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let Some(limits) = limits.get(rows.to_string()) else {
        println!("bench: no limits for {rows} rows in bench/limits.json; nothing checked");
        return Ok(());
    };
    let mut over = Vec::new();
    for m in measures {
        match limits.get(&m.name).and_then(Json::as_f64) {
            Some(max) if m.value > max => over.push(format!("{}: {} (limit {max} {})", m.name, fmt(m), m.unit)),
            Some(_) => {}
            None => println!("bench: no limit for `{}`", m.name),
        }
    }
    if over.is_empty() {
        println!("bench: every measure is within bench/limits.json");
        Ok(())
    } else {
        Err(format!("bench: over the limit:\n  {}", over.join("\n  ")))
    }
}

/// Writes the run into `docs/bench.md`, replacing the section for this row count.
fn record(root: &Path, rows: usize, measures: &[Measure]) -> Result<(), String> {
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .current_dir(root)
            .args(args)
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default()
    };
    let commit = git(&["rev-parse", "--short", "HEAD"]);
    let date = git(&["log", "-1", "--format=%cs"]);
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    let cpu = std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|c| c.lines().find(|l| l.starts_with("model name")).and_then(|l| l.split(':').nth(1)).map(|s| s.trim().to_string()))
        .unwrap_or_else(|| "unknown CPU".into());
    let machine = format!("{} {}, {cpu}, {cores} cores", std::env::consts::OS, std::env::consts::ARCH);
    let section =
        format!("<!-- bench:{rows} -->\n## {rows} rows\n\nCommit `{commit}` ({date}) · {machine}\n\n{}<!-- /bench:{rows} -->", table(rows, measures));
    let path = root.join("docs/bench.md");
    let old = std::fs::read_to_string(&path).unwrap_or_else(|_| HEADER.to_string());
    let (open, close) = (format!("<!-- bench:{rows} -->"), format!("<!-- /bench:{rows} -->"));
    let new = match (old.find(&open), old.find(&close)) {
        (Some(a), Some(b)) if b > a => format!("{}{section}{}", &old[..a], &old[b + close.len()..]),
        _ => format!("{}\n\n{section}\n", old.trim_end()),
    };
    std::fs::write(&path, new).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("bench: recorded in docs/bench.md");
    Ok(())
}

const HEADER: &str = "# Benchmarks

Measured with `cargo xtask bench --rows N --write` (`crates/engine/examples/bench.rs`): a workbook
built in code, with N rows of 6 values, a formula column (`=B*C+D`), a chained running sum and three
aggregates (SUM, SUMIFS, XLOOKUP over whole columns). Then an edit at the top that recalculates
everything below it, a sort on one column, a save to XLSX, and a fresh process that opens the saved
file. Peak memory is the process's high-water mark (Linux only). CI checks the 100,000-row run
against `bench/limits.json`. Each section is replaced when it is measured again.";

#[cfg(test)]
mod tests {
    use super::*;

    fn m(name: &str, value: f64, unit: &'static str) -> Measure {
        Measure { name: name.into(), value, unit }
    }

    #[test]
    fn the_table_shows_milliseconds_below_a_second() {
        let t = table(10, &[m("sort", 0.25, "s"), m("open xlsx (fresh process)", 2.5, "s"), m("peak memory: open", 300.0, "MB")]);
        assert!(t.contains("| sort | 250 ms |"), "{t}");
        assert!(t.contains("| open xlsx (fresh process) | 2.50 s |"), "{t}");
        assert!(t.contains("| peak memory: open | 300 MB |"), "{t}");
    }

    #[test]
    fn limits_fail_only_what_is_over() {
        let dir = std::env::temp_dir().join(format!("xtask-bench-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("bench")).unwrap();
        std::fs::write(dir.join("bench/limits.json"), r#"{"10": {"sort": 1.0, "save xlsx": 0.5}}"#).unwrap();
        assert!(check_limits(&dir, 10, &[m("sort", 0.9, "s"), m("save xlsx", 0.4, "s"), m("new", 9.0, "s")]).is_ok());
        let err = check_limits(&dir, 10, &[m("sort", 1.5, "s")]).unwrap_err();
        assert!(err.contains("sort: 1.50 s (limit 1"), "{err}");
        // No limits for this row count: nothing to check.
        assert!(check_limits(&dir, 99, &[m("sort", 1e9, "s")]).is_ok());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn recording_replaces_the_section_for_the_same_row_count() {
        let dir = std::env::temp_dir().join(format!("xtask-bench-record-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("docs")).unwrap();
        record(&dir, 10, &[m("sort", 1.0, "s")]).unwrap();
        record(&dir, 20, &[m("sort", 2.0, "s")]).unwrap();
        record(&dir, 10, &[m("sort", 3.0, "s")]).unwrap();
        let text = std::fs::read_to_string(dir.join("docs/bench.md")).unwrap();
        assert_eq!(text.matches("<!-- bench:10 -->").count(), 1);
        assert!(text.contains("| sort | 3.00 s |") && text.contains("| sort | 2.00 s |") && !text.contains("| sort | 1.00 s |"), "{text}");
        assert!(text.starts_with("# Benchmarks"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
