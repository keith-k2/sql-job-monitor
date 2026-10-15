#!/usr/bin/env bash
# Builds the Linux installers from the same release binary used on Windows
# and macOS. Produces a portable .tar.gz always, and an AppImage when
# appimagetool is available (or downloaded into target/linux/tools).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
version="${VERSION:-$("$root/scripts/read-version.sh")}"
binary="${BINARY:-$root/target/release/sql-job-monitor}"
arch="$(uname -m)"
stage="$root/target/linux"
appdir="$stage/SQL Job Monitor.AppDir"
tarball="$root/dist/sql-job-monitor-${version}-linux-${arch}.tar.gz"
appimage="$root/dist/sql-job-monitor-${version}-linux-${arch}.AppImage"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --binary) binary="$2"; shift 2 ;;
    --version) version="$2"; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 1 ;;
  esac
done

tarball="$root/dist/sql-job-monitor-${version}-linux-${arch}.tar.gz"
appimage="$root/dist/sql-job-monitor-${version}-linux-${arch}.AppImage"

if [[ ! -x "$binary" ]]; then
  echo "missing release binary: $binary" >&2
  echo "build it on Linux with: cargo build --release" >&2
  exit 1
fi

rm -rf "$stage"
mkdir -p "$appdir/usr/bin" "$appdir/usr/share/applications" \
  "$appdir/usr/share/icons/hicolor/256x256/apps" "$root/dist"

cp "$binary" "$appdir/usr/bin/sql-job-monitor"
chmod +x "$appdir/usr/bin/sql-job-monitor"
cp "$root/packaging/linux/AppRun" "$appdir/AppRun"
chmod +x "$appdir/AppRun"
cp "$root/packaging/linux/sql-job-monitor.desktop" "$appdir/sql-job-monitor.desktop"
cp "$root/packaging/linux/sql-job-monitor.desktop" "$appdir/usr/share/applications/sql-job-monitor.desktop"
cp "$root/assets/app.png" "$appdir/sql-job-monitor.png"
cp "$root/assets/app.png" "$appdir/usr/share/icons/hicolor/256x256/apps/sql-job-monitor.png"

portable="$stage/sql-job-monitor-${version}-linux-${arch}"
mkdir -p "$portable"
cp "$appdir/usr/bin/sql-job-monitor" "$portable/"
cp "$appdir/AppRun" "$portable/"
cp "$root/packaging/linux/sql-job-monitor.desktop" "$portable/"
cp "$root/assets/app.png" "$portable/sql-job-monitor.png"
tar -C "$stage" -czf "$tarball" "$(basename "$portable")"

echo "$tarball"

appimagetool=""
if command -v appimagetool >/dev/null 2>&1; then
  appimagetool="$(command -v appimagetool)"
elif [[ -x "$stage/tools/appimagetool" ]]; then
  appimagetool="$stage/tools/appimagetool"
elif [[ -n "${APPIMAGETOOL:-}" && -x "${APPIMAGETOOL}" ]]; then
  appimagetool="$APPIMAGETOOL"
fi

if [[ -z "$appimagetool" ]]; then
  echo "appimagetool not found; skipped AppImage. Install it or set APPIMAGETOOL." >&2
  exit 0
fi

ARCH="$arch" "$appimagetool" "$appdir" "$appimage"
chmod +x "$appimage"
echo "$appimage"
