# Benchmarks

Measured with `cargo xtask bench --rows N --write` (`crates/engine/examples/bench.rs`): a workbook
built in code, with N rows of 6 values, a formula column (`=B*C+D`), a chained running sum and three
aggregates (SUM, SUMIFS, XLOOKUP over whole columns). Then an edit at the top that recalculates
everything below it, a sort on one column, a save to XLSX, and a fresh process that opens the saved
file. Peak memory is the process's high-water mark (Linux only). CI checks the 100,000-row run
against `bench/limits.json`. Each section is replaced when it is measured again.

<!-- bench:100000 -->
## 100000 rows

Commit `a71d2b91` (2026-10-29) · linux x86_64, AMD Ryzen 7 5700G with Radeon Graphics, 16 cores

| 100000 rows | |
|---|---|
| fill values | 958 ms |
| autofill formulas | 484 ms |
| autofill chained formulas | 476 ms |
| aggregate formulas | 61 ms |
| edit → recalc | 231 ms |
| sort | 592 ms |
| save xlsx | 705 ms |
| open xlsx (fresh process) | 1.49 s |
| peak memory: build to save | 433 MB |
| peak memory: open | 221 MB |
| xlsx size | 5.9 MB |
<!-- /bench:100000 -->

<!-- bench:1000000 -->
## 1000000 rows

Commit `a71d2b91` (2026-10-29) · linux x86_64, AMD Ryzen 7 5700G with Radeon Graphics, 16 cores

| 1000000 rows | |
|---|---|
| fill values | 9.72 s |
| autofill formulas | 5.53 s |
| autofill chained formulas | 5.91 s |
| aggregate formulas | 764 ms |
| edit → recalc | 2.76 s |
| sort | 9.07 s |
| save xlsx | 8.49 s |
| open xlsx (fresh process) | 15.91 s |
| peak memory: build to save | 3646 MB |
| peak memory: open | 2230 MB |
| xlsx size | 61.9 MB |
<!-- /bench:1000000 -->
