#!/usr/bin/env bash
# Installs StatsBar's GNOME Shell extension for this user, or removes it with --uninstall
# (ADR 0001). On Wayland the Shell discovers a new extension, and loads changed code, only at the
# next login, so the first install cannot enable it yet: run this again after logging back in.
set -euo pipefail

uuid="statsbar@matrixi.github.io"
source_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/extension/$uuid"
target="${XDG_DATA_HOME:-$HOME/.local/share}/gnome-shell/extensions/$uuid"
files=(metadata.json extension.js clicks.js hooks.js)

case "${1:-}" in
    "")
        if ! command -v gnome-extensions >/dev/null; then
            echo "install-extension: gnome-extensions not found (it comes with gnome-shell)" >&2
            exit 1
        fi
        reinstall=false
        [[ -d "$target" ]] && reinstall=true
        mkdir -p -- "$target"
        for file in "${files[@]}"; do
            install -m 0644 -- "$source_dir/$file" "$target/$file"
        done
        if ! gnome-extensions enable "$uuid"; then
            echo "install-extension: installed to $target, but GNOME Shell has not loaded it yet." >&2
            echo "install-extension: log out and back in, then run this script again to enable it." >&2
        elif $reinstall; then
            echo "install-extension: updated $target; the new code takes effect at the next login." >&2
        fi
        ;;
    --uninstall)
        if [[ -d "$target" ]]; then
            if ! command -v gnome-extensions >/dev/null; then
                echo "install-extension: gnome-extensions not found; removing the files only." >&2
            # Fails ("doesn't exist") when the Shell never discovered it; the files go either way.
            elif ! gnome-extensions disable "$uuid"; then
                echo "install-extension: GNOME Shell had not loaded it; removing the files only." >&2
            fi
            rm -rf -- "$target"
        fi
        ;;
    *)
        echo "usage: $0 [--uninstall]" >&2
        exit 2
        ;;
esac
