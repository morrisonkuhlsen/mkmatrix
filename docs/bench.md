# Benchmarks

Measured with `cargo xtask bench --rows N --write` (`crates/engine/examples/bench.rs`): a workbook
built in code, with N rows of 6 values, a formula column (`=B*C+D`), a chained running sum and three
aggregates (SUM, SUMIFS, XLOOKUP over whole columns). Then an edit at the top that recalculates
everything below it, a sort on one column, a save to XLSX, and a fresh process that opens the saved
file. Peak memory is the process's high-water mark (Linux only). CI checks the 100,000-row run
against `bench/limits.json`. Each section is replaced when it is measured again.

<!-- bench:100000 -->
## 100000 rows

Commit `ec114080` (2026-11-02) · linux x86_64, AMD Ryzen 7 5700G with Radeon Graphics, 16 cores

| 100000 rows | |
|---|---|
| fill values | 251 ms |
| autofill formulas | 322 ms |
| autofill chained formulas | 288 ms |
| aggregate formulas | 58 ms |
| edit → recalc | 235 ms |
| sort | 604 ms |
| save xlsx | 704 ms |
| open xlsx (fresh process) | 1.51 s |
| peak memory: build to save | 433 MB |
| peak memory: open | 220 MB |
| xlsx size | 5.9 MB |
<!-- /bench:100000 -->

<!-- bench:1000000 -->
## 1000000 rows

Commit `ec114080` (2026-11-02) · linux x86_64, AMD Ryzen 7 5700G with Radeon Graphics, 16 cores

| 1000000 rows | |
|---|---|
| fill values | 2.53 s |
| autofill formulas | 3.82 s |
| autofill chained formulas | 4.16 s |
| aggregate formulas | 768 ms |
| edit → recalc | 2.67 s |
| sort | 8.65 s |
| save xlsx | 8.34 s |
| open xlsx (fresh process) | 16.33 s |
| peak memory: build to save | 3647 MB |
| peak memory: open | 2231 MB |
| xlsx size | 61.9 MB |
<!-- /bench:1000000 -->
