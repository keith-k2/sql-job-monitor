#!/usr/bin/env bash
# Single source of truth: the version field in Cargo.toml.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
awk -F'"' '/^version = / { print $2; exit }' "$root/Cargo.toml"
