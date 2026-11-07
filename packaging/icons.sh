#!/usr/bin/env bash
# Regenerate every app icon from assets/app-icon/mkmatrix.svg (and mkmatrix-small.svg, the mark
# with the a alone, for renders of 24 px and under, where the subscripts can't be read).
#
# Needs: resvg (brew install resvg / cargo install resvg). The .icns is written by iconutil on
# macOS and by `cargo xtask icns` elsewhere. The outputs are committed, so builds and packaging
# never need these tools.
#
#   packaging/icons.sh
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIR="$ROOT/assets/app-icon"
SVG="$DIR/mkmatrix.svg"
SMALL_SVG="$DIR/mkmatrix-small.svg"
ID="com.morrisonkuhlsen.mkmatrix"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

command -v resvg >/dev/null || { echo "error: resvg not found (brew install resvg)" >&2; exit 1; }

# The artwork is the mark on a transparent 512 canvas. Windows and Linux use it as is. macOS icons sit on
# Apple's grid: an 824/1024 body with a transparent margin, so widen the viewBox to 512*1024/824.
MAC="$TMP/mac.svg"
SMALL_MAC="$TMP/mac-small.svg"
for pair in "$SVG:$MAC" "$SMALL_SVG:$SMALL_MAC"; do
  src="${pair%%:*}" dst="${pair#*:}"
  sed 's/viewBox="0 0 512 512"/viewBox="-62.136 -62.136 636.272 636.272"/' "$src" >"$dst"
  grep -q 'viewBox="-62.136' "$dst" || { echo "error: expected viewBox=\"0 0 512 512\" in $src" >&2; exit 1; }
done

# Renders of 24 px and under take the small mark.
render() {
  local svg="$1" size="$2" out="$3"
  if [ "$size" -le 24 ]; then
    case "$svg" in
      "$SVG") svg="$SMALL_SVG" ;;
      "$MAC") svg="$SMALL_MAC" ;;
    esac
  fi
  resvg -w "$size" -h "$size" "$svg" "$out" </dev/null
}

render "$SVG" 1024 "$DIR/mkmatrix-1024.png"
# Runtime window/Dock icon on macOS (for apps/mkmatrix to embed); other platforms use hicolor 256.
render "$MAC" 512 "$DIR/mkmatrix-macos-512.png"

# Linux hicolor theme.
for s in 16 24 32 48 64 128 256 512; do
  mkdir -p "$DIR/hicolor/${s}x${s}/apps"
  render "$SVG" "$s" "$DIR/hicolor/${s}x${s}/apps/$ID.png"
done
mkdir -p "$DIR/hicolor/scalable/apps"
cp "$SVG" "$DIR/hicolor/scalable/apps/$ID.svg"

# Windows .ico.
ICO_PNGS=()
for s in 16 20 24 32 40 48 64 128 256; do
  render "$SVG" "$s" "$TMP/ico-$s.png"
  ICO_PNGS+=("$TMP/ico-$s.png")
done
(cd "$ROOT" && cargo run -q -p xtask --no-default-features -- ico "$DIR/mkmatrix.ico" "${ICO_PNGS[@]}")

# macOS .icns.
SET="$TMP/mkmatrix.iconset"
mkdir -p "$SET"
for s in 16 32 128 256 512; do
  render "$MAC" "$s" "$SET/icon_${s}x${s}.png"
  render "$MAC" $((s * 2)) "$SET/icon_${s}x${s}@2x.png"
done
if command -v iconutil >/dev/null; then
  iconutil -c icns -o "$DIR/mkmatrix.icns" "$SET"
else
  (cd "$ROOT" && cargo run -q -p xtask --no-default-features -- icns "$DIR/mkmatrix.icns" "$SET")
fi
echo "icons written to $DIR"
