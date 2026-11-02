# Benchmarks

Measured with `cargo xtask bench --rows N --write` (`crates/engine/examples/bench.rs`): a workbook
built in code, with N rows of 6 values, a formula column (`=B*C+D`), a chained running sum and three
aggregates (SUM, SUMIFS, XLOOKUP over whole columns). Then an edit at the top that recalculates
everything below it, a sort on one column, a save to XLSX, and a fresh process that opens the saved
file. Peak memory is the process's high-water mark (Linux only). CI checks the 100,000-row run
against `bench/limits.json`. Each section is replaced when it is measured again.

<!-- bench:100000 -->
## 100000 rows

Commit `bf00d063` (2026-11-02) · linux x86_64, AMD Ryzen 7 5700G with Radeon Graphics, 16 cores

| 100000 rows | |
|---|---|
| fill values | 191 ms |
| autofill formulas | 242 ms |
| autofill chained formulas | 198 ms |
| aggregate formulas | 7 ms |
| edit → recalc | 102 ms |
| sort | 282 ms |
| save xlsx | 685 ms |
| open xlsx (fresh process) | 1.36 s |
| peak memory: build to save | 387 MB |
| peak memory: open | 192 MB |
| xlsx size | 5.9 MB |
<!-- /bench:100000 -->

<!-- bench:1000000 -->
## 1000000 rows

Commit `bf00d063` (2026-11-02) · linux x86_64, AMD Ryzen 7 5700G with Radeon Graphics, 16 cores

| 1000000 rows | |
|---|---|
| fill values | 2.01 s |
| autofill formulas | 3.14 s |
| autofill chained formulas | 2.97 s |
| aggregate formulas | 98 ms |
| edit → recalc | 1.42 s |
| sort | 4.33 s |
| save xlsx | 8.01 s |
| open xlsx (fresh process) | 14.94 s |
| peak memory: build to save | 2899 MB |
| peak memory: open | 1967 MB |
| xlsx size | 61.9 MB |
<!-- /bench:1000000 -->
