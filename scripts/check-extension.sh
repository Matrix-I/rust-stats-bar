#!/usr/bin/env bash
# Runs the GNOME Shell extension's unit tests. Needs gjs (Ubuntu package gjs); fails, never
# skips, without it, so a missing tool cannot pass the gate.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
command -v gjs >/dev/null || { echo "check-extension: gjs not found (apt install gjs)" >&2; exit 1; }
gjs -m "$root/extension/tests/clicks.test.js"
gjs -m "$root/extension/tests/hooks.test.js"
