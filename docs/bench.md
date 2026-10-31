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

## Against LibreOffice Calc

Measured 2026-10-30 on the same machine (linux x86_64, AMD Ryzen 7 5700G, 16 cores): MK Matrix at
commit `cde620a8` with `cargo xtask bench`, against LibreOffice Calc 26.8.1 running the same
workload headless over UNO (same values, formulas, edit, sort and save; each timed step reads a
dependent cell, since Calc computes formulas on demand). Black-box only: nothing of LibreOffice's
is read or copied. Both arrive at the same J1, J2 and J3. Calc's times include the UNO bridge
between processes; its peak memory includes about 218 MB that an empty Calc already holds.
Calc's 100,000-row figures are the median of three runs; the rest are single runs.

| 100000 rows | MK Matrix | LibreOffice Calc |
|---|---|---|
| fill values | 985 ms | 490 ms |
| autofill formulas | 514 ms | 133 ms |
| autofill chained formulas | 496 ms | 184 ms |
| aggregate formulas | 58 ms | 12 ms |
| edit → recalc | 276 ms | 157 ms |
| sort | 682 ms | 268 ms |
| save xlsx | 716 ms | 1.50 s |
| open xlsx (fresh process) | 1.55 s | 1.08 s |
| peak memory: build to save | 433 MB | 533 MB |
| peak memory: open | 220 MB | 407 MB |
| xlsx size | 5.9 MB | 6.3 MB |

| 1000000 rows | MK Matrix | LibreOffice Calc |
|---|---|---|
| fill values | 9.89 s | 4.66 s |
| autofill formulas | 5.20 s | 1.36 s |
| autofill chained formulas | 5.63 s | 1.84 s |
| aggregate formulas | 780 ms | 90 ms |
| edit → recalc | 2.70 s | 1.49 s |
| sort | 8.13 s | 2.98 s |
| save xlsx | 8.18 s | 16.44 s |
| open xlsx (fresh process) | 16.16 s | 9.39 s |
| peak memory: build to save | 3647 MB | 2622 MB |
| peak memory: open | 2232 MB | 1888 MB |
| xlsx size | 61.9 MB | 63.8 MB |

Where MK Matrix trails: formula autofill (about 4×), aggregates (5–9×), sort (2.5–3×), filling
values (2×) and, at a million rows, opening (1.7×) and memory (1.4×). Where it leads: saving XLSX
(2×) and smaller files.
