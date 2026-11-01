#!/usr/bin/env bash
# Regenerates the committed branding assets from the two SVG sources.
#
#   assets/branding/icon.svg   app icon (full-bleed, used by Linux and the
#                              in-app mark)
#   assets/branding/logo.svg   horizontal lockup (README / website)
#
# Requires rsvg-convert. macOS icons additionally use sips/iconutil and
# ImageMagick for the padded Dock artwork. Run from anywhere.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root/assets/branding"

if ! command -v rsvg-convert >/dev/null 2>&1; then
  echo "错误：需要 rsvg-convert（librsvg）。" >&2
  exit 1
fi

for size in 32 64 128 256 512 1024; do
  rsvg-convert -w "$size" -h "$size" icon.svg -o "icon-$size.png"
done
rsvg-convert -w 760 -h 200 logo.svg -o logo.png
rsvg-convert -w 1520 -h 400 logo.svg -o logo@2x.png

# macOS expects the artwork inset inside the icon canvas. A full-bleed tile
# reads as oversized in the Dock; 832/1024 keeps the standard outer margin.
padded="icon-macos-1024.png"
rsvg-convert -w 832 -h 832 icon.svg -o /tmp/qieyue-icon-832.png
if command -v magick >/dev/null 2>&1; then
  magick /tmp/qieyue-icon-832.png -background none -gravity center \
    -extent 1024x1024 "$padded"
elif command -v convert >/dev/null 2>&1; then
  convert /tmp/qieyue-icon-832.png -background none -gravity center \
    -extent 1024x1024 "$padded"
else
  cp icon-1024.png "$padded"
  echo "警告：没有 ImageMagick，macOS 图标未加留白。" >&2
fi

if [ "$(uname -s)" = "Darwin" ] && command -v iconutil >/dev/null 2>&1; then
  rm -rf /tmp/qieyue.iconset
  mkdir -p /tmp/qieyue.iconset
  emit() { sips -z "$1" "$1" "$padded" --out "/tmp/qieyue.iconset/$2" >/dev/null; }
  emit 16 icon_16x16.png
  emit 32 icon_16x16@2x.png
  emit 32 icon_32x32.png
  emit 64 icon_32x32@2x.png
  emit 128 icon_128x128.png
  emit 256 icon_128x128@2x.png
  emit 256 icon_256x256.png
  emit 512 icon_256x256@2x.png
  emit 512 icon_512x512.png
  cp "$padded" /tmp/qieyue.iconset/icon_512x512@2x.png
  iconutil -c icns /tmp/qieyue.iconset -o qieyue.icns
  echo "==> assets/branding/qieyue.icns"
fi

# Tauri reads icons from `desktop/icons/` and the in-app mark from the
# frontend's asset directory.
mkdir -p "$root/desktop/icons" "$root/desktop/ui/assets"
cp icon-32.png "$root/desktop/icons/32x32.png"
cp icon-128.png "$root/desktop/icons/128x128.png"
cp icon-256.png "$root/desktop/icons/128x128@2x.png"
cp icon-512.png "$root/desktop/icons/icon.png"
if [ -f qieyue.icns ]; then
  cp qieyue.icns "$root/desktop/icons/icon.icns"
fi
cp icon-128.png "$root/desktop/ui/assets/logo.png"

echo "==> 品牌资源已更新"
