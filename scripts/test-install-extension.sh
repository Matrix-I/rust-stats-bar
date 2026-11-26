#!/usr/bin/env bash
# Tests scripts/install-extension.sh against a temporary data directory, with gnome-extensions
# replaced by a stub that records its arguments. Never touches the user's real extensions.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
uuid="statsbar@matrixi.github.io"
work="$(mktemp -d)"
trap 'rm -rf -- "$work"' EXIT
mkdir -p "$work/bin" "$work/data"
cat >"$work/bin/gnome-extensions" <<'STUB'
#!/usr/bin/env bash
echo "$*" >>"$STUB_LOG"
if [[ ( "$1" == "enable" && "${STUB_ENABLE_FAILS:-0}" == "1" ) ||
      ( "$1" == "disable" && "${STUB_DISABLE_FAILS:-0}" == "1" ) ]]; then
    echo "Extension “$2” doesn't exist" >&2
    exit 2
fi
exit 0
STUB
chmod 0755 "$work/bin/gnome-extensions"
export PATH="$work/bin:$PATH" XDG_DATA_HOME="$work/data" STUB_LOG="$work/calls"
target="$work/data/gnome-shell/extensions/$uuid"
fail() { echo "FAIL - $1" >&2; exit 1; }

"$root/scripts/install-extension.sh"
for file in metadata.json extension.js clicks.js hooks.js; do
    cmp -s "$root/extension/$uuid/$file" "$target/$file" || fail "install copies $file"
    [[ "$(stat -c %a "$target/$file")" == "644" ]] || fail "$file is mode 644"
done
[[ "$(find "$target" -mindepth 1 | wc -l)" == "4" ]] || fail "install copies only the four extension files"
grep -qx "enable $uuid" "$work/calls" || fail "install enables the extension"
echo "ok - install copies the files and enables the extension"

echo "stale" >"$target/extension.js"
output="$("$root/scripts/install-extension.sh" 2>&1)"
cmp -s "$root/extension/$uuid/extension.js" "$target/extension.js" || fail "reinstall overwrites"
[[ "$output" == *"takes effect at the next login"* ]] || fail "reinstall says when it takes effect: $output"
echo "ok - reinstall overwrites and says it takes effect at the next login"

: >"$work/calls"
output="$(STUB_ENABLE_FAILS=1 "$root/scripts/install-extension.sh" 2>&1)" || fail "an enable failure still exits 0"
[[ "$output" == *"log out and back in"* ]] || fail "an enable failure says to log out: $output"
echo "ok - an extension the Shell has not discovered yet gets the logout hint"

: >"$work/calls"
"$root/scripts/install-extension.sh" --uninstall
[[ ! -e "$target" ]] || fail "uninstall removes the directory"
grep -qx "disable $uuid" "$work/calls" || fail "uninstall disables the extension"
echo "ok - uninstall disables and removes it"

"$root/scripts/install-extension.sh" --uninstall || fail "uninstall twice still exits 0"
echo "ok - uninstall is idempotent"

"$root/scripts/install-extension.sh" 2>/dev/null
STUB_DISABLE_FAILS=1 "$root/scripts/install-extension.sh" --uninstall 2>/dev/null \
    || fail "uninstall of an undiscovered extension exits 0"
[[ ! -e "$target" ]] || fail "uninstall of an undiscovered extension removes the directory"
echo "ok - uninstall before the Shell discovered it still removes the files"

set +e
"$root/scripts/install-extension.sh" --bogus 2>/dev/null
status=$?
set -e
[[ "$status" == "2" ]] || fail "an unknown argument exits 2, got $status"
echo "ok - an unknown argument exits 2"

# A PATH with only the tools the script needs, and no gnome-extensions.
mkdir -p "$work/nogx"
for tool in bash dirname basename mkdir install rm cat; do
    ln -s "$(command -v "$tool")" "$work/nogx/$tool"
done
set +e
PATH="$work/nogx" "$root/scripts/install-extension.sh" 2>"$work/missing"
missing=$?
set -e
[[ "$missing" == "1" ]] || fail "a missing gnome-extensions exits 1, got $missing"
grep -q "gnome-extensions not found" "$work/missing" || fail "a missing gnome-extensions is named"
echo "ok - a missing gnome-extensions exits 1 and says so"

"$root/scripts/install-extension.sh" 2>/dev/null
[[ -d "$target" ]] || fail "uninstall without gnome-extensions: precondition, target missing"
set +e
PATH="$work/nogx" "$root/scripts/install-extension.sh" --uninstall 2>"$work/missing-uninstall"
status=$?
set -e
[[ "$status" == "0" ]] || fail "uninstall without gnome-extensions exits 0, got $status"
[[ ! -e "$target" ]] || fail "uninstall without gnome-extensions removes the directory"
grep -q "gnome-extensions not found; removing the files only" "$work/missing-uninstall" \
    || fail "uninstall without gnome-extensions names the missing tool"
echo "ok - uninstall without gnome-extensions still removes the files"
echo "install-extension: all passed"
