#!/usr/bin/env bash
# Builds a minimal QieYue.app bundle for macOS.
#
# Usage:
#   ./packaging/macos/bundle.sh            # release build into build/macos/QieYue.app
#
# The bundle carries the generated .icns, so the Dock and Finder show the
# project icon instead of the generic executable icon. The window itself also
# sets the same icon at runtime (useful when run outside the bundle).
set -euo pipefail

if [ "$(uname -s)" != "Darwin" ]; then
  echo "错误：bundle.sh 只能在 macOS 上运行。" >&2
  exit 1
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/desktop/Cargo.toml" | head -n 1)"
if [ -z "$version" ]; then
  echo "错误：读不到 desktop/Cargo.toml 的版本号。" >&2
  exit 1
fi

echo "==> 编译 release…"
"$root/scripts/cargo.sh" build --release -p qieyue

app="$root/build/macos/QieYue.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"

cp "$root/target/release/qieyue" "$app/Contents/MacOS/qieyue"
cp "$root/assets/branding/qieyue.icns" "$app/Contents/Resources/qieyue.icns"
cp "$root/packaging/macos/Info.plist" "$app/Contents/Info.plist"
sed -i '' "s|<string>0.1.0</string>|<string>$version</string>|g" "$app/Contents/Info.plist"
chmod +x "$app/Contents/MacOS/qieyue"

# Ad-hoc signature so Gatekeeper does not refuse a locally built bundle. A
# distributed build should be signed and notarised with the project identity.
codesign --force --deep --sign - "$app" >/dev/null 2>&1 || true

echo "==> $app"
echo "    运行：open \"$app\""
