#!/usr/bin/env bash
# Builds SQL Job Monitor.app and a drag-install DMG. Run on macOS after
# `cargo build --release` (or pass a binary with --binary).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
version="${VERSION:-0.1.0}"
binary="${BINARY:-$root/target/release/sql-job-monitor}"
stage="$root/target/macos"
app="$stage/SQL Job Monitor.app"
dmg="$root/target/macos/sql-job-monitor-${version}-macos.dmg"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --binary) binary="$2"; shift 2 ;;
    --version) version="$2"; dmg="$root/target/macos/sql-job-monitor-${version}-macos.dmg"; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 1 ;;
  esac
done

if [[ ! -x "$binary" ]]; then
  echo "missing release binary: $binary" >&2
  echo "build it with: cargo build --release" >&2
  exit 1
fi

rm -rf "$stage"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"

iconset="$stage/app.iconset"
mkdir -p "$iconset"
for size in 16 32 64 128 256 512; do
  sips -z "$size" "$size" "$root/assets/app.png" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" "$root/assets/app.png" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/app.icns"
rm -rf "$iconset"

sed "s/0.1.0/${version}/g" "$root/packaging/macos/Info.plist" > "$app/Contents/Info.plist"
cp "$binary" "$app/Contents/MacOS/sql-job-monitor"
chmod +x "$app/Contents/MacOS/sql-job-monitor"

volume="$stage/dmg"
mkdir -p "$volume"
cp -R "$app" "$volume/"
ln -s /Applications "$volume/Applications"

rm -f "$dmg"
hdiutil create -volname "SQL Job Monitor" -srcfolder "$volume" -ov -format UDZO "$dmg" >/dev/null
echo "$dmg"
