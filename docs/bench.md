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

## Against LibreOffice Calc

Measured 2026-11-02 on the same machine (linux x86_64, AMD Ryzen 7 5700G, 16 cores): MK Matrix at
commit `b18633d5` against LibreOffice Calc 26.8.1 running the same workload headless over UNO (same
values, formulas, edit, sort and save). Each is the median of five rounds, alternating the two.
Black-box only: nothing of LibreOffice's is read or copied.

Both saved files hold the same value in every cell, at both sizes. MK Matrix's steps finish with
everything recalculated. Calc computes formulas when read, so each of its timed steps reads a
dependent cell. Its sort is timed both ways, since it leaves the recalculation to the first read.
Calc's times include the UNO bridge and Python, which weigh most on filling values, because the rows
are built in Python inside the timed step. Its peak memory includes about 216 MB that an empty Calc
already holds.

| 100000 rows | MK Matrix | LibreOffice Calc |
|---|---|---|
| fill values | 190 ms | 472 ms |
| autofill formulas | 69 ms | 133 ms |
| autofill chained formulas | 101 ms | 183 ms |
| aggregate formulas | 6 ms | 13 ms |
| edit → recalc | 47 ms | 161 ms |
| sort | 156 ms | 270 ms |
| sort, then read the results | 156 ms | 421 ms |
| save xlsx | 699 ms | 1.47 s |
| open xlsx (fresh process) | 478 ms | 1.11 s |
| peak memory: build to save | 202 MB | 501 MB |
| peak memory: open | 187 MB | 406 MB |
| xlsx size | 5.9 MB | 6.3 MB |

| 1000000 rows | MK Matrix | LibreOffice Calc |
|---|---|---|
| fill values | 2.12 s | 4.91 s |
| autofill formulas | 763 ms | 1.35 s |
| autofill chained formulas | 1.52 s | 1.86 s |
| aggregate formulas | 62 ms | 89 ms |
| edit → recalc | 873 ms | 1.58 s |
| sort | 2.34 s | 3.13 s |
| sort, then read the results | 2.34 s | 4.69 s |
| save xlsx | 8.12 s | 16.31 s |
| open xlsx (fresh process) | 4.91 s | 9.72 s |
| peak memory: build to save | 2003 MB | 2744 MB |
| peak memory: open | 1640 MB | 1887 MB |
| xlsx size | 61.9 MB | 63.8 MB |

MK Matrix leads every row at both sizes. Two leads are narrow. Peak memory on open is about level
once the empty Calc is taken off (1640 MB against 1671 MB at a million rows). The chained autofill
overlaps at a million rows: MK Matrix ranged 1.35–1.87 s and Calc 1.82–1.89 s.

Each opens the other's file too. Calc opens MK Matrix's in about 970 ms at 100,000 rows and 8.7 s at a
million; MK Matrix opens Calc's in about 570 ms and, once #132 lets a 518 MB part in, 5.6 s.
