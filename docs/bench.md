# Benchmarks

Measured with `cargo xtask bench --rows N --write` (`crates/engine/examples/bench.rs`): a workbook
built in code, with N rows of 6 values, a formula column (`=B*C+D`), a chained running sum and three
aggregates (SUM, SUMIFS, XLOOKUP over whole columns). Then an edit at the top that recalculates
everything below it, a sort on one column, a save to XLSX, and a fresh process that opens the saved
file. Peak memory is the process's high-water mark (Linux only). CI checks the 100,000-row run
against `bench/limits.json`. Each section is replaced when it is measured again.

<!-- bench:100000 -->
## 100000 rows

Commit `30613e58` (2026-10-29) · linux x86_64, AMD Ryzen 7 5700G with Radeon Graphics, 16 cores

| 100000 rows | |
|---|---|
| fill values | 919 ms |
| autofill formulas | 528 ms |
| autofill chained formulas | 474 ms |
| aggregate formulas | 56 ms |
| edit → recalc | 218 ms |
| sort | 689 ms |
| save xlsx | 628 ms |
| open xlsx (fresh process) | 1.39 s |
| peak memory: build to save | 577 MB |
| peak memory: open | 343 MB |
| xlsx size | 5.9 MB |
<!-- /bench:100000 -->

<!-- bench:1000000 -->
## 1000000 rows

Commit `30613e58` (2026-10-29) · linux x86_64, AMD Ryzen 7 5700G with Radeon Graphics, 16 cores

| 1000000 rows | |
|---|---|
| fill values | 9.37 s |
| autofill formulas | 5.73 s |
| autofill chained formulas | 5.83 s |
| aggregate formulas | 675 ms |
| edit → recalc | 2.78 s |
| sort | 9.48 s |
| save xlsx | 7.53 s |
| open xlsx (fresh process) | 15.39 s |
| peak memory: build to save | 4857 MB |
| peak memory: open | 3455 MB |
| xlsx size | 61.9 MB |
<!-- /bench:1000000 -->
