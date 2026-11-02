# Benchmarks

Measured with `cargo xtask bench --rows N --write` (`crates/engine/examples/bench.rs`): a workbook
built in code, with N rows of 6 values, a formula column (`=B*C+D`), a chained running sum and three
aggregates (SUM, SUMIFS, XLOOKUP over whole columns). Then an edit at the top that recalculates
everything below it, a sort on one column, a save to XLSX, and a fresh process that opens the saved
file. Peak memory is the process's high-water mark (Linux only). CI checks the 100,000-row run
against `bench/limits.json`. Each section is replaced when it is measured again.

<!-- bench:100000 -->
## 100000 rows

Commit `f2bf540b` (2026-11-02) · linux x86_64, AMD Ryzen 7 5700G with Radeon Graphics, 16 cores

| 100000 rows | |
|---|---|
| fill values | 184 ms |
| autofill formulas | 100 ms |
| autofill chained formulas | 106 ms |
| aggregate formulas | 5 ms |
| edit → recalc | 54 ms |
| sort | 255 ms |
| save xlsx | 688 ms |
| open xlsx (fresh process) | 1.27 s |
| peak memory: build to save | 207 MB |
| peak memory: open | 160 MB |
| xlsx size | 5.9 MB |
<!-- /bench:100000 -->

<!-- bench:1000000 -->
## 1000000 rows

Commit `f2bf540b` (2026-11-02) · linux x86_64, AMD Ryzen 7 5700G with Radeon Graphics, 16 cores

| 1000000 rows | |
|---|---|
| fill values | 2.09 s |
| autofill formulas | 1.48 s |
| autofill chained formulas | 1.59 s |
| aggregate formulas | 59 ms |
| edit → recalc | 924 ms |
| sort | 3.85 s |
| save xlsx | 8.04 s |
| open xlsx (fresh process) | 13.86 s |
| peak memory: build to save | 1952 MB |
| peak memory: open | 1762 MB |
| xlsx size | 61.9 MB |
<!-- /bench:1000000 -->
