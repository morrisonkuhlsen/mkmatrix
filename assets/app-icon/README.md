# MK Matrix app icon

MK Matrix's mark on a full-bleed tile: a matrix's square brackets around a 3×3 of rounded cells,
the diagonal lit and the rest at 40 %, in MK Matrix teal on Morrison Kühlsen's ink. The splash
screen draws the same mark in code (`crates/ui-egui/src/splash.rs`). It is original geometry
written by hand for this repository.

To change it, edit `mkmatrix.svg` and run `packaging/icons.sh` (it needs `resvg`; on Linux,
`rsvg-convert` renders the same PNGs, and the `.icns` can be assembled from PNG entries).

## Palette

| Colour | Hex | Used for |
|---|---|---|
| Ink | `#14161c` | The tile (Morrison Kühlsen's ink, as in MK Sigma and MK Perth) |
| MK Matrix teal (app colour) | `#2ec4a0` | The brackets and cells |

## Geometry

A 512 × 512 viewBox, clipped to a rounded square with `rx=112`. Windows and Linux use the full
tile. The macOS icons (`.icns`, `mkmatrix-macos-512.png`) sit on Apple's grid: an 824/1024
body with a transparent margin.

## Files

| File | Use |
|---|---|
| `mkmatrix.svg` | Canonical artwork; also `hicolor/scalable` |
| `mkmatrix-1024.png` | 1024 px render of the full tile |
| `mkmatrix-macos-512.png` | Runtime Dock icon on macOS (for `apps/mkmatrix` to embed) |
| `mkmatrix.icns` | macOS bundle icon (`CFBundleIconFile`) |
| `mkmatrix.ico` | Windows exe icon, 16–256 px (embedded by `apps/mkmatrix/build.rs`) |
| `hicolor/<size>/apps/com.morrisonkuhlsen.mkmatrix.png` | Linux icon theme, 16–512 px; the 256 px one is also the runtime window icon on Windows and Linux |
| `hicolor/scalable/apps/com.morrisonkuhlsen.mkmatrix.svg` | Linux scalable icon |
| `LICENSE.txt` | MIT OR Apache-2.0 |

The app ID is `com.morrisonkuhlsen.mkmatrix`. It's used for the hicolor icon names, the Wayland app
ID and `packaging/linux/com.morrisonkuhlsen.mkmatrix.desktop`.

## Regenerate

Edit `mkmatrix.svg`, then run `packaging/icons.sh`. It needs `resvg`, and `iconutil` for the
`.icns` (macOS only). It packs the `.ico` with `cargo xtask ico`.
