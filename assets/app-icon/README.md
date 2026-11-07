# MK Matrix app icon

MK Matrix's mark on no background (transparent): **[aᵢⱼ]**, a matrix as mathematics writes one,
square brackets around its general element *a* with the subscripts *i* and *j* (the row and the
column), in MK Matrix terracotta. It says what the app is: every cell a number, aᵢⱼ. The letters
are monoline strokes drawn by hand for the mark and slanted 12° as italics; no font is used. The
splash screen and the File view draw the same mark in code (`crates/ui-egui/src/splash.rs`). It
is original geometry written by hand for this repository.

At 24 px and under the subscripts can't be read, so those sizes use `mkmatrix-small.svg`: the
brackets around the *a* alone, larger and heavier (the code draws it under 32 points).

To change it, edit `mkmatrix.svg` (and `mkmatrix-small.svg`) and run `packaging/icons.sh` (it
needs `resvg`; the `.icns` comes from `iconutil` on macOS and `cargo xtask icns` elsewhere).

## Palette

| Colour | Hex | Used for |
|---|---|---|
| MK Matrix terracotta (app colour, `splash::BRAND`) | `#e08a5f` | The brackets and letters |
| — | transparent | The background: no tile, so the mark sits on the taskbar, Dock or desktop |

## Geometry

A 512 × 512 viewBox; the mark is scaled 1.25× about the centre so it fills the canvas without a
tile. Windows and Linux use it as is. The macOS icons (`.icns`, `mkmatrix-macos-512.png`) sit on Apple's grid: an 824/1024
body with a transparent margin.

## Files

| File | Use |
|---|---|
| `mkmatrix.svg` | Canonical artwork; also `hicolor/scalable` |
| `mkmatrix-small.svg` | The mark at 24 px and under (the a alone) |
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

Edit `mkmatrix.svg` (and `mkmatrix-small.svg`), then run `packaging/icons.sh`. Every shape is
a stroked path, ellipse or circle, so any renderer that antialiases gives the same PNGs. It needs
`resvg`; it packs the `.ico` with `cargo xtask ico` and the `.icns` with `iconutil` on macOS or
`cargo xtask icns` elsewhere. Keep `splash::mark` in step with the SVGs.
