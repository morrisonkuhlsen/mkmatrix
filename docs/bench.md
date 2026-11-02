# Benchmarks

Measured with `cargo xtask bench --rows N --write` (`crates/engine/examples/bench.rs`): a workbook
built in code, with N rows of 6 values, a formula column (`=B*C+D`), a chained running sum and three
aggregates (SUM, SUMIFS, XLOOKUP over whole columns). Then an edit at the top that recalculates
everything below it, a sort on one column, a save to XLSX, and a fresh process that opens the saved
file. Peak memory is the process's high-water mark (Linux only). CI checks the 100,000-row run
against `bench/limits.json`. Each section is replaced when it is measured again.

<!-- bench:100000 -->
## 100000 rows

Commit `c3ea46a1` (2026-11-02) · linux x86_64, AMD Ryzen 7 5700G with Radeon Graphics, 16 cores

| 100000 rows | |
|---|---|
| fill values | 196 ms |
| autofill formulas | 173 ms |
| autofill chained formulas | 144 ms |
| aggregate formulas | 6 ms |
| edit → recalc | 102 ms |
| sort | 340 ms |
| save xlsx | 693 ms |
| open xlsx (fresh process) | 1.33 s |
| peak memory: build to save | 269 MB |
| peak memory: open | 180 MB |
| xlsx size | 5.9 MB |
<!-- /bench:100000 -->

<!-- bench:1000000 -->
## 1000000 rows

Commit `c3ea46a1` (2026-11-02) · linux x86_64, AMD Ryzen 7 5700G with Radeon Graphics, 16 cores

| 1000000 rows | |
|---|---|
| fill values | 2.00 s |
| autofill formulas | 2.16 s |
| autofill chained formulas | 2.02 s |
| aggregate formulas | 58 ms |
| edit → recalc | 1.33 s |
| sort | 4.39 s |
| save xlsx | 8.85 s |
| open xlsx (fresh process) | 15.36 s |
| peak memory: build to save | 2507 MB |
| peak memory: open | 1875 MB |
| xlsx size | 61.9 MB |
<!-- /bench:1000000 -->
