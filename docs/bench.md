# Benchmarks

Measured with `cargo xtask bench --rows N --write` (`crates/engine/examples/bench.rs`): a workbook
built in code, with N rows of 6 values, a formula column (`=B*C+D`), a chained running sum and three
aggregates (SUM, SUMIFS, XLOOKUP over whole columns). Then an edit at the top that recalculates
everything below it, a sort on one column, a save to XLSX, and a fresh process that opens the saved
file. Peak memory is the process's high-water mark (Linux only). CI checks the 100,000-row run
against `bench/limits.json`. Each section is replaced when it is measured again.

<!-- bench:100000 -->
## 100000 rows

Commit `acdfdf83` (2026-11-02) · linux x86_64, AMD Ryzen 7 5700G with Radeon Graphics, 16 cores

| 100000 rows | |
|---|---|
| fill values | 182 ms |
| autofill formulas | 68 ms |
| autofill chained formulas | 93 ms |
| aggregate formulas | 5 ms |
| edit → recalc | 47 ms |
| sort | 148 ms |
| save xlsx | 694 ms |
| open xlsx (fresh process) | 472 ms |
| peak memory: build to save | 204 MB |
| peak memory: open | 188 MB |
| xlsx size | 5.9 MB |
<!-- /bench:100000 -->

<!-- bench:1000000 -->
## 1000000 rows

Commit `acdfdf83` (2026-11-02) · linux x86_64, AMD Ryzen 7 5700G with Radeon Graphics, 16 cores

| 1000000 rows | |
|---|---|
| fill values | 2.01 s |
| autofill formulas | 684 ms |
| autofill chained formulas | 1.34 s |
| aggregate formulas | 56 ms |
| edit → recalc | 781 ms |
| sort | 2.18 s |
| save xlsx | 8.28 s |
| open xlsx (fresh process) | 5.44 s |
| peak memory: build to save | 2000 MB |
| peak memory: open | 1639 MB |
| xlsx size | 61.9 MB |
<!-- /bench:1000000 -->
