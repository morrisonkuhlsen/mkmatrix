# Benchmarks

Measured with `cargo xtask bench --rows N --write` (`crates/engine/examples/bench.rs`): a workbook
built in code, with N rows of 6 values, a formula column (`=B*C+D`), a chained running sum and three
aggregates (SUM, SUMIFS, XLOOKUP over whole columns). Then an edit at the top that recalculates
everything below it, a sort on one column, a save to XLSX, and a fresh process that opens the saved
file. Peak memory is the process's high-water mark (Linux only). CI checks the 100,000-row run
against `bench/limits.json`. Each section is replaced when it is measured again.

<!-- bench:100000 -->
## 100000 rows

Commit `ac62d8a0` (2026-11-02) · linux x86_64, AMD Ryzen 7 5700G with Radeon Graphics, 16 cores

| 100000 rows | |
|---|---|
| fill values | 193 ms |
| autofill formulas | 71 ms |
| autofill chained formulas | 105 ms |
| aggregate formulas | 6 ms |
| edit → recalc | 52 ms |
| sort | 159 ms |
| save xlsx | 690 ms |
| open xlsx (fresh process) | 1.20 s |
| peak memory: build to save | 203 MB |
| peak memory: open | 156 MB |
| xlsx size | 5.9 MB |
<!-- /bench:100000 -->

<!-- bench:1000000 -->
## 1000000 rows

Commit `ac62d8a0` (2026-11-02) · linux x86_64, AMD Ryzen 7 5700G with Radeon Graphics, 16 cores

| 1000000 rows | |
|---|---|
| fill values | 2.05 s |
| autofill formulas | 722 ms |
| autofill chained formulas | 1.46 s |
| aggregate formulas | 59 ms |
| edit → recalc | 716 ms |
| sort | 2.14 s |
| save xlsx | 7.92 s |
| open xlsx (fresh process) | 12.36 s |
| peak memory: build to save | 2004 MB |
| peak memory: open | 1613 MB |
| xlsx size | 61.9 MB |
<!-- /bench:1000000 -->
