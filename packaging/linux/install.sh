#!/usr/bin/env bash
# Installs qieyue into the current user's XDG prefix on Linux.
#
# Usage:
#   ./packaging/linux/install.sh                 # ~/.local
#   PREFIX=/usr/local sudo ./packaging/linux/install.sh
set -euo pipefail

if [ "$(uname -s)" != "Linux" ]; then
  echo "错误：install.sh 只能在 Linux 上运行。" >&2
  exit 1
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
prefix="${PREFIX:-$HOME/.local}"

echo "==> 编译 release…"
"$root/scripts/cargo.sh" build --release -p qieyue

install -Dm755 "$root/target/release/qieyue" "$prefix/bin/qieyue"
install -Dm644 "$root/assets/branding/icon-512.png" \
  "$prefix/share/icons/hicolor/512x512/apps/qieyue.png"
install -Dm644 "$root/packaging/linux/qieyue.desktop" \
  "$prefix/share/applications/qieyue.desktop"

command -v update-desktop-database >/dev/null 2>&1 \
  && update-desktop-database "$prefix/share/applications" >/dev/null 2>&1 || true
command -v gtk-update-icon-cache >/dev/null 2>&1 \
  && gtk-update-icon-cache -f "$prefix/share/icons/hicolor" >/dev/null 2>&1 || true

echo "==> 已安装到 $prefix/bin/qieyue"
