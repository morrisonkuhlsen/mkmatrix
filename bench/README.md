# Benchmark limits

`limits.json` holds, per row count, the most each measure of `cargo xtask bench --check` may take:
seconds for steps, MB for memory and file size. The CI `bench` job checks the 100,000-row run.

The 2026-10-29 baseline (`docs/bench.md`) set them. Times allow about 4× that machine's numbers,
because CI runners are slower and noisier; memory and size allow about 1.5×, because they vary little
between machines. When a change makes a measure better for good (#93, #94, #95…), lower its limit in
the same PR, so the gain can't quietly be lost again.
