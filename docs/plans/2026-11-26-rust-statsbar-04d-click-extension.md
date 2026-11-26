# Plan 04d — Single-Click Extension Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: subagent-driven-strict or executing-plans-strict. Tasks are `### Task N: <name>`; steps use `- [ ]`.

**Goal:** a single primary click on StatsBar's CPU or Memory tray icon opens its details page in
GNOME Shell 50, through a small GNOME Shell extension. Every other click keeps working as it does
now.

**Architecture:** [ADR 0001](../adr/0001-single-click-opens-panel-extension.md), option B. The
extension `statsbar@matrixi.github.io` lives in `extension/statsbar@matrixi.github.io/`.

- `clicks.js` and `hooks.js` are pure: no `gi://` or `resource://` imports, so `gjs` can test them with fakes. `hooks.js` holds the button bookkeeping (added in the whole-branch review fix round 1; `extension.js` is a thin shell over it).
- `extension.js` hooks the `button-press-event` of every AppIndicator button, both the ones already
  in `Main.panel.statusArea` and those added later, found through an `InjectionManager` override of
  `Main.panel.addToStatusArea`.
- On a primary press it reads `button._indicator.id` (at click time: the item's properties load
  after the button is added). For `statsbar-cpu` or `statsbar-memory` it closes the button's menu,
  calls `button._indicator.open(x, y, time)` (Ubuntu's double-click path, which sends `Activate`
  with the activation token) and stops the event.
- `scripts/install-extension.sh` installs and uninstalls it.
- The daemon, protocol and config do not change.
- End-to-end checks run in a headless GNOME Shell (`--headless --mode=ubuntu --unsafe-mode`) on a
  private bus, with clicks driven through a Clutter virtual pointer over `org.gnome.Shell.Eval`.
  `gnome-shell --devkit` cannot run here: `mutter-devkit` is not installed. No `xdotool` or
  `ydotool` either.

**Tech Stack:** GJS 1.88.0 (ES modules), GNOME Shell 50.1. `ubuntu-appindicators@ubuntu.com` lists
shell versions 45–50 and is enabled by the `ubuntu` session mode
(`/usr/share/gnome-shell/modes/ubuntu.json`), not by `enabled-extensions`. Bash for the scripts.
Rust code is unchanged.

**Branch:** `feature/panel-skeleton` (same branch as Plans 04b and 04c; the user is working on it).

## Confirmed Requirements

1. A single primary click on CPU or Memory opens that page (user, 2026-11-26).
2. The extension is the small one that only changes the click. It does not draw its own items
   (user, 2026-11-26).
3. Battery, Fan, Network and the hub keep opening their menu on a left click (user, 2026-11-26).
4. Install and uninstall are scripted in this plan: `scripts/install-extension.sh`, with
   `--uninstall` (user, 2026-11-26).
5. ADR 0001 accepted (user, 2026-11-26).
6. With another item's menu open, a left click on CPU or Memory follows stock GNOME: the first
   click only closes the open menu, the next one opens the page. No extra handling (user,
   2026-11-26).

## Decision Log

- Small click-intercepting extension; other icons keep their menu; scripted install; stock GNOME
  behavior while another menu is open. Ticket: not
  posted, because this project has no tracker and records its decisions in ADRs and plans
  ([ADR 0001](../adr/0001-single-click-opens-panel-extension.md)). PR line: "A GNOME Shell extension
  makes a single left click on CPU or Memory open its page."

## Scope Map

Invariant: a single primary press on the AppIndicator button of `statsbar-cpu` or
`statsbar-memory` opens that page and does not open the menu. Every other press, button and item
behaves as it does without the extension.

| Path | Decision |
|---|---|
| Primary press, CPU or Memory button | in (Tasks 2, 3) |
| Primary press, Battery/Fan/Network/hub or another app's item | in (Task 2: `ignored`, the event propagates) |
| Secondary (right) and middle press on any item | in (Task 2: `ignored`) |
| Double click on CPU or Memory (users were told about it) | in (Task 3 e2e step 3). Both presses are stopped, so `Activate` arrives twice. The second panel launch hands over to the first and exits (Plan 04b, "Second launch"), so the result is one window on that page, and the menu stays shut |
| Another item's menu open, then a primary press on CPU or Memory | in (Task 3 e2e step 6). Stock GNOME, as confirmed (Requirement 6): the open menu's modal grab takes the press (`popupMenu.js` `_onCapturedEvent`), so the first click only closes it and the next opens the page. The extension adds nothing for it |
| Its own menu open (opened by a right click), then a primary press on it | in (Task 2: the menu is closed before opening the page) |
| Buttons already in `statusArea` when the extension is enabled | in (Task 3: scan on `enable()`) |
| Buttons added later (item shown from the hub, daemon restart, `tray-pos` change re-adding the same button) | in (Task 3: `addToStatusArea` override, de-duplicated) |
| Destroyed buttons | in (Task 3: dropped from the handler map on `destroy`) |
| Disable, then enable again (screen lock and unlock do this to every extension) | in (Task 3 e2e step 8) |
| Item whose `_indicator` has our id but no `open()` | in (Task 2: `unsupported`; Task 3 logs once and propagates) |
| Legacy tray icons (`IndicatorStatusTrayIcon`, also `appindicator-` roles, no `_indicator`) | in (Task 2: `ignored`) |
| Touch tap (`vfunc_event` toggles the menu, `indicatorStatusIcon.js:412-416`) | unchanged: not a button press, and this laptop's target is the pointer |
| Install, uninstall, including uninstall before the Shell discovered the extension | in (Task 4) |
| Daemon `Activate` handling | unchanged (commit 155e203, `crates/daemon/tests/tray.rs`) |
| A `.desktop` entry or a general installer | out (follow-up: Plan 11 folds `install-extension.sh` into the install) |
| Publishing to extensions.gnome.org | out (follow-up: only if the target grows beyond this machine, ADR 0001) |

## Impact Map

| Changed shared symbol | Caller | Before → after | Verdict |
|---|---|---|---|
| `Main.panel.addToStatusArea` (wrapped while the extension is enabled) | `ubuntu-appindicators` `addIconToPanel` (`indicatorStatusIcon.js:50`) and every other extension or Shell component adding a panel button | same arguments, return value and thrown errors. The wrapper calls the original first, then connects handlers only to `appindicator-` roles | unchanged (Task 3 code; e2e steps 1–8 run with Ubuntu's icons added through it) |
| `scripts/check.sh` | CI, the final gate | also runs `scripts/check-extension.sh` and `scripts/test-install-extension.sh` | intended (Tasks 2, 4) |
| `.github/workflows/ci.yml` | CI | also installs `gjs`, and also triggers on `extension/**` | intended (Task 2) |

## Existing Data & States

| Case | Intended behavior | Task/test |
|---|---|---|
| The Shell is already running with our icons when the extension is enabled | Scanned and hooked on `enable()` | Task 3 e2e step 8 (enable after the icons exist) |
| A session without the extension installed | As today: a single click opens the menu, a double click opens the page | Task 1 baseline run |
| An earlier install of the extension at the target path | Its files are overwritten. The running Shell keeps the old code until the next login, and the script says so | Task 4 test `reinstall overwrites` |
| Installed but not yet discovered (first install on Wayland, before logout) | `--uninstall` still removes the files and exits 0 | Task 4 test with `STUB_DISABLE_FAILS=1` |
| The user's real `statsbard` (pid 239811) and an unrelated `gnome-shell --headless` (pid 227241, Wayland display `hs-gnome-0`) are running | The e2e harness uses its own bus, runtime, config and data directories and the Wayland display `sb-e2e-0`, so it touches neither | Task 1 harness, checked by `pgrep` before and after each run |

## Global Constraints

- Global rules: `/home/thang-tran/.claude/CLAUDE.md`. Project rules: `README.md`.
- Verification: `./scripts/check.sh`, which by the end of this plan runs fmt, clippy, the Rust
  tests, core purity, `scripts/check-extension.sh` and `scripts/test-install-extension.sh`.
- Commits only when the user asks. Tasks end with "Ready to commit".
- `clicks.js` and `hooks.js` must not import `gi://` or `resource://` modules.
- The extension adds no timer, no polling and no D-Bus calls of its own (ADR 0001, idle cost).
- The e2e harness lives in `.sdd/e2e/` (git-ignored by `.sdd/.gitignore`) and never touches the real
  session. It runs everything under `dbus-run-session`, with `XDG_RUNTIME_DIR` (mode 0700),
  `XDG_CONFIG_HOME` and `XDG_DATA_HOME` under `.sdd/e2e/env/`. It never writes the real dconf
  database, `~/.local/share` or `/run/user/1000`. It never stops the user's `statsbard` or the other
  headless shell.
- Drive every click yourself through the harness; do not ask the user to click. The real session
  install in Task 5 is the exception: only the user can log out.

## Technical Defaults

- `gjs -m` with a hand-written assert harness that runs tests sequentially with top-level `await`
  and exits once at the end. Reason: no npm or JS test framework in the repo; GJS is the runtime the
  extension uses.
- `gjs` is installed in CI with `apt-get`, like `libgtk-4-dev`. Reason: CI runs `./scripts/check.sh`.
- `shell-version: ["50"]`. Reason: the target is this machine (roadmap Requirement 5).
- The install script calls `gnome-extensions enable`. If that fails (Wayland has not discovered a
  new extension yet), it prints how to finish, then exits 0. Reason: the first install always needs
  a logout.
- The handler is a `button-press-event` signal handler returning `Clutter.EVENT_STOP`. Plan review
  confirmed from the Clutter-18 typelib (RUN_LAST, boolean accumulator) that it runs before the
  vfunc; Task 1 confirms it on the running Shell.
- The e2e harness drives a headless shell instead of `--devkit`. Reason: `mutter-devkit` is not
  installed, and installing it was not asked for.

## Risks & Rollback

| Risk | Mitigation | Revert |
|---|---|---|
| The signal handler does not stop Ubuntu's handler on the real Shell | Task 1 settles it first. If it fails, stop and report (Deviation) | none needed, nothing built |
| `org.gnome.Shell.Eval` or the virtual pointer does not work in the headless shell | Task 1 builds and proves the harness first. If it fails, stop and report: the user decides about installing `mutter-dev-bin` for `--devkit` | none needed |
| Ubuntu renames `_indicator` or `open()` | Fails safe: `ignored` or `unsupported`, so the clicks behave as today | `--uninstall` |
| Another extension also wraps `addToStatusArea`, and the disable order makes `InjectionManager.clear()` restore over its wrapper | None enabled here wraps it (Task 1 records `gnome-extensions list --enabled` and checks it). Revisit if one is installed | `gnome-extensions disable statsbar@matrixi.github.io` |
| An extension bug throws inside `gnome-shell` | Handlers return early on missing fields and catch around the call. GNOME disables an extension that throws in `enable()` | `gnome-extensions disable statsbar@matrixi.github.io` |
| CI breaks because `gjs` is missing | Task 2 adds the install step | revert the workflow line |

### Task 1: E2E harness and the handler spike

**Type:** docs (spike; harness kept outside Git) · **Test level:** e2e (headless shell)
**Files:** create `.sdd/e2e/nested.sh`, `.sdd/e2e/press.sh`, `.sdd/e2e/press.js` and `.sdd/e2e/hub-click.sh`. These are
git-ignored, and Tasks 3 and 5 reuse them. Create `docs/spikes/2026-11-26-04d-click-spike.md`. The
spike extension lives only in `.sdd/e2e/spike/` and is deleted at the end.

- [x] Run `cargo build --workspace --release --locked` (the harness runs `target/release/statsbard`);
  expect success.
- [x] Write `.sdd/e2e/nested.sh` (mode 0755):

```bash
#!/usr/bin/env bash
# Throwaway e2e harness for Plan 04d, outside Git. Runs a headless GNOME Shell in the ubuntu
# session mode (which enables ubuntu-appindicators) on a private session bus, with private
# runtime, config and data directories, plus a statsbard that registers with it. Then it runs
# "$@" inside that environment and tears everything down.
# EXTENSIONS: space-separated extension directories to install and enable (default: none).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
env_dir="$root/.sdd/e2e/env"
rm -rf -- "$env_dir"
mkdir -p "$env_dir/run" "$env_dir/config/statsbar" "$env_dir/data/gnome-shell/extensions"
chmod 0700 "$env_dir/run"
printf '[items]\ncpu = true\nmemory = true\nbattery = true\nfan = true\nnetwork = true\n' \
    >"$env_dir/config/statsbar/config.toml"
uuids=()
for dir in ${EXTENSIONS:-}; do
    uuid="$(basename "$dir")"
    cp -r -- "$dir" "$env_dir/data/gnome-shell/extensions/$uuid"
    uuids+=("'$uuid'")
done
enabled="[$(IFS=,; echo "${uuids[*]:-}")]"

export XDG_RUNTIME_DIR="$env_dir/run" XDG_CONFIG_HOME="$env_dir/config" \
    XDG_DATA_HOME="$env_dir/data" E2E_ENABLED="$enabled" E2E_ROOT="$root"
exec dbus-run-session -- bash -c '
    set -euo pipefail
    gsettings set org.gnome.shell enabled-extensions "$E2E_ENABLED"
    env -u DISPLAY -u WAYLAND_DISPLAY XDG_SESSION_TYPE=wayland \
        gnome-shell --headless --virtual-monitor 1280x800 --wayland-display sb-e2e-0 \
        --mode=ubuntu --unsafe-mode >"$XDG_RUNTIME_DIR/shell.log" 2>&1 &
    shell=$!
    daemon=
    trap "kill \$daemon \$shell 2>/dev/null || true; wait 2>/dev/null || true" EXIT
    gdbus wait --session --timeout 30 org.gnome.Shell
    gdbus wait --session --timeout 30 org.kde.StatusNotifierWatcher
    # The daemon and its panels must reach the headless shell, never the real desktop.
    env -u DISPLAY -u XAUTHORITY WAYLAND_DISPLAY=sb-e2e-0 \
        "$E2E_ROOT/target/release/statsbard" >"$XDG_RUNTIME_DIR/daemon.log" 2>&1 &
    daemon=$!
    export E2E_DAEMON=$daemon
    for _ in $(seq 50); do
        [[ "$(gdbus call --session -d org.kde.StatusNotifierWatcher -o /StatusNotifierWatcher \
            -m org.freedesktop.DBus.Properties.Get org.kde.StatusNotifierWatcher \
            RegisteredStatusNotifierItems | grep -o "StatusNotifierItem-$daemon-" | wc -l)" == 6 ]] && break
        sleep 0.2
    done
    "$@"
' bash "$@"
```

- [x] Write `.sdd/e2e/press.js`. `press.sh` replaces `@ITEM@`, `@BUTTON@`, `@COUNT@` and `@KEEP@`
  before sending it to `org.gnome.Shell.Eval`. Eval runs `JSON.stringify(await eval(code))`
  (`shellDBus.js` in Shell 50), so it waits for the async body:

```js
// Evaluated in the headless shell by press.sh. Unless @KEEP@ is true, it first closes any open
// panel menu (an open menu's modal grab would take the press). Then it moves a virtual pointer
// to the item's button, presses @BUTTON@ @COUNT@ times, waits for the double-click window, and
// records the open AppIndicator menus and the shell's windows in globalThis.__statsbarE2e.
(async () => {
    const out = globalThis.__statsbarE2e = {done: false};
    try {
        const {default: Clutter} = await import('gi://Clutter');
        const {default: GLib} = await import('gi://GLib');
        const Main = await import('resource:///org/gnome/shell/ui/main.js');
        const settle = (ms) => new Promise((resolve) => GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms,
            () => { resolve(); return GLib.SOURCE_REMOVE; }));
        if (!@KEEP@ && Main.panel.menuManager.activeMenu) {
            Main.panel.menuManager.activeMenu.close();
            await settle(300);
        }
        const items = () => Object.entries(Main.panel.statusArea)
            .filter(([role, b]) => role.startsWith('appindicator-') && b?._indicator?.id);
        const found = items().find(([, b]) => b._indicator.id === '@ITEM@');
        if (!found)
            throw new Error('no button for @ITEM@');
        const button = found[1];
        const [x, y] = button.get_transformed_position();
        const [w, h] = button.get_transformed_size();
        const backend = global.stage.get_context?.()?.get_backend?.() ?? Clutter.get_default_backend();
        const pointer = backend.get_default_seat().create_virtual_device(Clutter.InputDeviceType.POINTER_DEVICE);
        const now = () => GLib.get_monotonic_time();
        pointer.notify_absolute_motion(now(), x + w / 2, y + h / 2);
        for (let i = 0; i < @COUNT@; i++) {
            pointer.notify_button(now(), @BUTTON@, Clutter.ButtonState.PRESSED);
            pointer.notify_button(now(), @BUTTON@, Clutter.ButtonState.RELEASED);
        }
        const {doubleClickTime} = Clutter.Settings.get_default();
        await settle(doubleClickTime + 300);
        out.menus = Object.fromEntries(items().map(([, b]) => [b._indicator.id, Boolean(b.menu?.isOpen)]));
        // A panel window here proves the panel reached the headless shell, not the real desktop.
        await settle(1000);
        out.windows = global.display.list_all_windows().map((w) => w.get_title());
    } catch (error) {
        out.error = String(error);
    }
    out.done = true;
})();
```

- [x] Write `.sdd/e2e/press.sh` (mode 0755), run inside `nested.sh`:

```bash
#!/usr/bin/env bash
# press.sh <item-id> <button 1|2|3> [count] [keep]: presses a button on a StatsBar tray item in
# the headless shell (keep=true leaves an open menu open first), then prints the open
# AppIndicator menus, the shell's window titles, and the daemon's statsbar-panel children.
# Eval's reply is JSON inside a GVariant string, so keys print escaped: \"statsbar-cpu\":true.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
code="$(sed -e "s/@ITEM@/$1/g" -e "s/@BUTTON@/$2/g" -e "s/@COUNT@/${3:-1}/g" \
    -e "s/@KEEP@/${4:-false}/g" "$here/press.js")"
eval_js() { gdbus call --session -d org.gnome.Shell -o /org/gnome/Shell -m org.gnome.Shell.Eval "$1"; }
eval_js "$code" >/dev/null
for _ in $(seq 50); do
    result="$(eval_js 'JSON.stringify(globalThis.__statsbarE2e)')"
    [[ "$result" == *'\"done\":true'* ]] && break
    sleep 0.1
done
echo "result: $result"
echo "panels: $(pgrep -P "$E2E_DAEMON" -a statsbar-panel | sed 's/^[0-9]* //' | paste -sd ';' -)"
```

- [x] Write `.sdd/e2e/hub-click.sh` (mode 0755), run inside `nested.sh`:

```bash
#!/usr/bin/env bash
# hub-click.sh <label>: clicks the hub menu row with this label (e.g. "Show CPU") over dbusmenu.
set -euo pipefail
hub="org.kde.StatusNotifierItem-$E2E_DAEMON-1"
id="$(gdbus call --session -d "$hub" -o /MenuBar -m com.canonical.dbusmenu.GetGroupProperties '[]' '["label"]' \
    | grep -oP "\(\K[0-9]+(?=, \{'label': <'$1'>)")"
[[ -n "$id" ]] || { echo "hub-click: no row '$1'" >&2; exit 1; }
gdbus call --session -d "$hub" -o /MenuBar -m com.canonical.dbusmenu.Event "$id" clicked '<int32 0>' 0 >/dev/null
```

- [x] Prove the harness without any extension. — deviated: harness readiness waits, parsing and helper fixes (checkpoint 1; [the e2e record](../spikes/2026-11-26-04d-click-extension-e2e.md)) Record `pgrep -a statsbard` and
  `pgrep -af 'gnome-shell --headless'` before. Then run:
  `.sdd/e2e/nested.sh bash -c '.sdd/e2e/press.sh statsbar-cpu 1; .sdd/e2e/press.sh statsbar-cpu 1 2'`.
  Expected baseline (ADR 0001 option A):
  - The single press prints `\"statsbar-cpu\":true` in the result, and panels is empty.
  - The double press prints panels `.../statsbar-panel --page cpu`, and the result's `windows`
    holds the panel's title `StatsBar`. That proves the panel is a client of the headless shell.

  Every later expectation uses the same escaped form, and expects `StatsBar` in `windows`
  whenever a panel is expected.

  Check `pgrep` again after the run: the user's daemon and the other headless shell are still
  there, with the same pids. If Eval returns `(false, ...)` or the menu never opens, stop and
  report (Risks row 2).
- [x] Write the spike extension `.sdd/e2e/spike/statsbar-spike@test/`. Its `metadata.json` is
  `{"uuid":"statsbar-spike@test","name":"spike","description":"spike","shell-version":["50"]}`. Its
  `extension.js`:

```js
import Clutter from 'gi://Clutter';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

export default class Spike extends Extension {
    enable() {
        this._ids = [];
        for (const [role, button] of Object.entries(Main.panel.statusArea)) {
            if (!role.startsWith('appindicator-') || !button)
                continue;
            const id = button.connect('button-press-event', (_actor, event) => {
                if (event.get_button() !== Clutter.BUTTON_PRIMARY || button._indicator?.id !== 'statsbar-cpu')
                    return Clutter.EVENT_PROPAGATE;
                button._indicator.open(...event.get_coords(), event.get_time()).catch(logError);
                return Clutter.EVENT_STOP;
            });
            this._ids.push([button, id]);
        }
    }

    disable() {
        for (const [button, id] of this._ids)
            button.disconnect(id);
        this._ids = [];
    }
}
```

  The spike scans only at `enable()`, but the icons register later. So the run disables and
  re-enables it once the items are up. Run:
  `EXTENSIONS=.sdd/e2e/spike/statsbar-spike@test .sdd/e2e/nested.sh bash -c 'gnome-extensions disable statsbar-spike@test; gnome-extensions enable statsbar-spike@test; .sdd/e2e/press.sh statsbar-cpu 1; .sdd/e2e/press.sh statsbar-battery 1; .sdd/e2e/press.sh statsbar-cpu 3'`.
  Pass when:
  - The CPU single press prints no open menu, and panels has one `--page cpu`.
  - Battery prints `"statsbar-battery":true`.
  - A right press on CPU prints `"statsbar-cpu":true`.
- [x] Record in `docs/spikes/2026-11-26-04d-click-spike.md`:
  - the commands and the printed lines;
  - `gnome-extensions list --enabled` inside the harness, and whether any of them wraps
    `addToStatusArea` (`grep -l addToStatusArea` over their directories, excluding
    `ubuntu-appindicators`, which only calls it);
  - the conclusion line, `Handler: signal works` or `Handler: signal fails`. On `fails`, stop the
    plan and report.

  RED: n/a — no executable behavior, checked by the recorded harness run.
- [x] Delete `.sdd/e2e/spike/` and `.sdd/e2e/env/`. Keep `nested.sh`, `press.sh`, `press.js` and `hub-click.sh`.
- [x] Ready to commit: `docs/spikes/2026-11-26-04d-click-spike.md`, message
  "docs: record the single-click handler spike"

### Task 2: Pure click logic with gjs tests and the check gate

**Type:** feature · **Test level:** unit
**Files:**
- Create `extension/statsbar@matrixi.github.io/clicks.js`, `extension/tests/clicks.test.js` and
  `scripts/check-extension.sh`.
- Modify `scripts/check.sh`: append one line after `./scripts/check-core-purity.sh`.
- Modify `.github/workflows/ci.yml`: the `paths` lists of `push` and `pull_request`, plus one
  install step before `./scripts/check.sh`.

**Interfaces:** Produces `PAGE_ITEM_IDS: Set<string>`, `isAppIndicatorRole(role) -> boolean` and
`openOnPrimaryPress(button, press, primaryButton, onError) -> 'opened' | 'ignored' | 'unsupported'`.
`press` is `{button: number, x: number, y: number, time: number}`.

- [x] Write the failing test `extension/tests/clicks.test.js`:

```js
// Unit tests for clicks.js, run by scripts/check-extension.sh with `gjs -m`. Tests run one after
// another (top-level await), and the process exits once at the end with the failure count, so a
// failing or throwing test can never leave exit code 0.
import System from 'system';
import {PAGE_ITEM_IDS, isAppIndicatorRole, openOnPrimaryPress} from '../statsbar@matrixi.github.io/clicks.js';

const PRIMARY = 1;
const MIDDLE = 2;
const SECONDARY = 3;
const tests = [];
const test = (name, body) => tests.push([name, body]);

function equal(actual, expected, what) {
    const a = JSON.stringify(actual);
    const e = JSON.stringify(expected);
    if (a !== e)
        throw new Error(`${what}: expected ${e}, got ${a}`);
}

/** A button as ubuntu-appindicators builds it: `_indicator` with `id` and `open`, and a menu. */
function fakeButton(id, {withOpen = true, openError = null} = {}) {
    const calls = {open: [], menuClosed: 0};
    const indicator = {id};
    if (withOpen) {
        indicator.open = async (x, y, time) => {
            calls.open.push([x, y, time]);
            if (openError)
                throw openError;
        };
    }
    const button = {_indicator: indicator, menu: {close: () => { calls.menuClosed += 1; }}};
    return {button, calls};
}

const press = (button) => ({button, x: 120, y: 8, time: 4242});
const noError = () => { throw new Error('onError called'); };

test('the page items are CPU and Memory only', () => {
    equal([...PAGE_ITEM_IDS].sort(), ['statsbar-cpu', 'statsbar-memory'], 'ids');
});

test('appindicator roles are recognised and others are not', () => {
    equal(isAppIndicatorRole('appindicator-:1.341@/StatusNotifierItem'), true, 'appindicator role');
    equal(isAppIndicatorRole('quickSettings'), false, 'shell role');
    equal(isAppIndicatorRole(undefined), false, 'no role');
});

for (const id of ['statsbar-cpu', 'statsbar-memory']) {
    test(`a primary press on ${id} closes the menu and opens the page at the press`, () => {
        const {button, calls} = fakeButton(id);
        equal(openOnPrimaryPress(button, press(PRIMARY), PRIMARY, noError), 'opened', 'outcome');
        equal(calls.open, [[120, 8, 4242]], 'open calls');
        equal(calls.menuClosed, 1, 'menu closes');
    });
}

for (const id of ['statsbar-hub', 'statsbar-battery', 'statsbar-fan', 'statsbar-network', 'org.other.App']) {
    test(`a primary press on ${id} is left to the menu`, () => {
        const {button, calls} = fakeButton(id);
        equal(openOnPrimaryPress(button, press(PRIMARY), PRIMARY, noError), 'ignored', 'outcome');
        equal(calls.open, [], 'open calls');
        equal(calls.menuClosed, 0, 'menu untouched');
    });
}

for (const [name, which] of [['secondary', SECONDARY], ['middle', MIDDLE]]) {
    test(`a ${name} press on CPU is left alone`, () => {
        const {button, calls} = fakeButton('statsbar-cpu');
        equal(openOnPrimaryPress(button, press(which), PRIMARY, noError), 'ignored', 'outcome');
        equal(calls.open, [], 'open calls');
        equal(calls.menuClosed, 0, 'menu untouched');
    });
}

test('a button without _indicator (legacy tray icon, or a renamed field) is ignored', () => {
    equal(openOnPrimaryPress({menu: {close: noError}}, press(PRIMARY), PRIMARY, noError), 'ignored', 'outcome');
});

test('our item without open() is unsupported and the menu is untouched', () => {
    const {button, calls} = fakeButton('statsbar-cpu', {withOpen: false});
    equal(openOnPrimaryPress(button, press(PRIMARY), PRIMARY, noError), 'unsupported', 'outcome');
    equal(calls.menuClosed, 0, 'menu untouched');
});

test('a failed open is handed to onError', async () => {
    const {button} = fakeButton('statsbar-memory', {openError: new Error('activate failed')});
    let reported;
    const reportedOnce = new Promise((resolve) => { reported = resolve; });
    equal(openOnPrimaryPress(button, press(PRIMARY), PRIMARY, reported), 'opened', 'outcome');
    const timeout = new Promise((_, reject) => setTimeout(() => reject(new Error('onError never called')), 2000));
    const error = await Promise.race([reportedOnce, timeout]);
    equal(error.message, 'activate failed', 'error');
});

let failures = 0;
for (const [name, body] of tests) {
    try {
        await body();
        print(`ok - ${name}`);
    } catch (error) {
        failures += 1;
        print(`FAIL - ${name}: ${error.message}`);
    }
}
print(failures === 0 ? `clicks: all ${tests.length} passed` : `clicks: ${failures} of ${tests.length} failed`);
System.exit(failures === 0 ? 0 : 1);
```

- [x] Write `scripts/check-extension.sh` (mode 0755):

```bash
#!/usr/bin/env bash
# Runs the GNOME Shell extension's unit tests. Needs gjs (Ubuntu package gjs); fails, never
# skips, without it, so a missing tool cannot pass the gate.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
command -v gjs >/dev/null || { echo "check-extension: gjs not found (apt install gjs)" >&2; exit 1; }
gjs -m "$root/extension/tests/clicks.test.js"
```

- [x] Run `./scripts/check-extension.sh`. Expect FAIL: gjs reports an import error for
  `../statsbar@matrixi.github.io/clicks.js` (file not found), exit code non-zero.
- [x] Write `extension/statsbar@matrixi.github.io/clicks.js`:

```js
// What a press on an AppIndicator panel button does. Pure: no gi:// or resource:// imports, so
// gjs can test it with fakes (extension/tests/clicks.test.js).
//
// The button fields read here (`_indicator`, its `id` and `open`) belong to
// ubuntu-appindicators@ubuntu.com (indicatorStatusIcon.js, appIndicator.js), not to a public API.
// When they are missing, every press is left to that extension, which opens the menu as before.

/** The StatusNotifierItem ids whose page a single primary click opens (statsbard's tray ids). */
export const PAGE_ITEM_IDS = new Set(['statsbar-cpu', 'statsbar-memory']);

const ROLE_PREFIX = 'appindicator-';

/** Whether a Main.panel.statusArea role is one ubuntu-appindicators added. */
export function isAppIndicatorRole(role) {
    return typeof role === 'string' && role.startsWith(ROLE_PREFIX);
}

/**
 * Opens the item's page for a primary press on a page item, as ubuntu-appindicators does on a
 * double click (`_indicator.open`, which sends Activate with the activation token).
 * Returns 'opened' (the caller stops the event), 'ignored' (let it through), or 'unsupported'
 * (a page item without `open`; let it through and log once).
 */
export function openOnPrimaryPress(button, press, primaryButton, onError) {
    if (press.button !== primaryButton)
        return 'ignored';
    const indicator = button?._indicator;
    if (!indicator || !PAGE_ITEM_IDS.has(indicator.id))
        return 'ignored';
    if (typeof indicator.open !== 'function')
        return 'unsupported';
    button.menu?.close?.();
    // `open` is async (appIndicator.js:843), so its failure arrives as a rejection.
    Promise.resolve(indicator.open(press.x, press.y, press.time)).catch(onError);
    return 'opened';
}
```

- [x] Run `./scripts/check-extension.sh`. Expect PASS: 14 `ok -` lines, then
  `clicks: all 14 passed`, exit 0. Then prove the gate can fail: temporarily change `'opened'` to
  `'opnd'` in the last `return` of `clicks.js` and run it again. Expect `FAIL -` lines,
  `clicks: 3 of 14 failed` and exit 1. Revert, and confirm the pass again.
- [x] Append `./scripts/check-extension.sh` as the last line of `scripts/check.sh`. In
  `.github/workflows/ci.yml`, add `- "extension/**"` to both `paths` lists, and add this before the
  `./scripts/check.sh` step:

```yaml
      # scripts/check-extension.sh runs the GNOME Shell extension's tests with gjs.
      - run: command -v gjs || (sudo apt-get update && sudo apt-get install -y --no-install-recommends gjs)
```

- [x] Run `./scripts/check.sh`. Expect exit 0, the Rust totals unchanged (384 passed, 0 failed,
  1 ignored), and `clicks: all 14 passed`.
- [x] Ready to commit: `extension/statsbar@matrixi.github.io/clicks.js extension/tests/clicks.test.js scripts/check-extension.sh scripts/check.sh .github/workflows/ci.yml`,
  message "feat(extension): decide what a press on a StatsBar tray button does"

### Task 3: The extension

**Type:** feature · **Test level:** e2e (headless shell through the Task 1 harness; the wiring has no
unit seam beyond Task 2)
**Files:** create `extension/statsbar@matrixi.github.io/metadata.json` and
`extension/statsbar@matrixi.github.io/extension.js`.
**Interfaces:** Consumes `isAppIndicatorRole` and `openOnPrimaryPress` (Task 2). Produces the
extension's `enable()` and `disable()`.

- [x] RED: run `EXTENSIONS=extension/statsbar@matrixi.github.io .sdd/e2e/nested.sh bash -c 'gnome-extensions info statsbar@matrixi.github.io; .sdd/e2e/press.sh statsbar-cpu 1'`.
  Expected: `info` fails (no `metadata.json`, so the Shell finds no extension), and the CPU press
  prints `"statsbar-cpu":true` with no panel. That is today's behavior.
- [x] Write `extension/statsbar@matrixi.github.io/metadata.json`:

```json
{
  "uuid": "statsbar@matrixi.github.io",
  "name": "StatsBar single click",
  "description": "A single left click on StatsBar's CPU or Memory icon opens its details page.",
  "shell-version": ["50"]
}
```

- [x] Write `extension/statsbar@matrixi.github.io/extension.js`:

```js
// Makes a single primary click on StatsBar's CPU or Memory icon open its page (ADR 0001).
// ubuntu-appindicators@ubuntu.com draws the icons and opens the menu on a single click; this
// connects a button-press-event handler, which runs before its vfunc, and stops the event for
// those two items only. No timers, no polling, no D-Bus of its own.
import Clutter from 'gi://Clutter';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Extension, InjectionManager} from 'resource:///org/gnome/shell/extensions/extension.js';

import {isAppIndicatorRole, openOnPrimaryPress} from './clicks.js';

export default class StatsBarSingleClick extends Extension {
    enable() {
        /** Each hooked button → its [press, destroy] handler ids. */
        this._handlers = new Map();
        this._warned = false;
        this._injections = new InjectionManager();
        const hook = (button) => this._hook(button);
        this._injections.overrideMethod(Main.panel, 'addToStatusArea', (original) =>
            function (role, indicator, ...rest) {
                const result = original.call(this, role, indicator, ...rest);
                if (isAppIndicatorRole(role) && indicator)
                    hook(indicator);
                return result;
            });
        for (const [role, button] of Object.entries(Main.panel.statusArea)) {
            if (isAppIndicatorRole(role) && button)
                this._hook(button);
        }
    }

    disable() {
        this._injections.clear();
        this._injections = null;
        for (const [button, ids] of this._handlers) {
            for (const id of ids)
                button.disconnect(id);
        }
        this._handlers = null;
    }

    _hook(button) {
        // addIconToPanel re-adds the same button when the tray position changes.
        if (!this._handlers || this._handlers.has(button))
            return;
        const press = button.connect('button-press-event', (_actor, event) => {
            const [x, y] = event.get_coords();
            let outcome;
            try {
                outcome = openOnPrimaryPress(button,
                    {button: event.get_button(), x, y, time: event.get_time()},
                    Clutter.BUTTON_PRIMARY,
                    (error) => logError(error, 'statsbar: opening the page failed'));
            } catch (error) {
                logError(error, 'statsbar: opening the page failed');
                return Clutter.EVENT_PROPAGATE;
            }
            if (outcome === 'unsupported' && !this._warned) {
                this._warned = true;
                console.warn('statsbar: the AppIndicator item has no open(); a single click opens the menu');
            }
            return outcome === 'opened' ? Clutter.EVENT_STOP : Clutter.EVENT_PROPAGATE;
        });
        const destroy = button.connect('destroy', () => this._handlers?.delete(button));
        this._handlers.set(button, [press, destroy]);
    }
}
```

- [x] GREEN, through the harness with the extension enabled from the start: — deviated: panel closed before steps 6a, 7, 8a, 8b; rollback on surviving buttons added (checkpoint 1; [the e2e record](../spikes/2026-11-26-04d-click-extension-e2e.md))
  `EXTENSIONS=extension/statsbar@matrixi.github.io .sdd/e2e/nested.sh bash -c '<steps>'`. For each
  step below, record the commands and the printed lines in the report.
  1. `press.sh statsbar-cpu 1` → no open menu; panels lists one `--page cpu`. This proves the
     `addToStatusArea` hook, since the icons register after `enable()`.
  2. `press.sh statsbar-memory 1` with the panel still open → no open menu; panels lists one
     panel (a second launch hands over and exits).
  3. Close the panel (`pkill -P "$E2E_DAEMON" statsbar-panel`), then `press.sh statsbar-cpu 1 2`
     (double click) → no open menu; panels lists one `--page cpu`.
  4. `press.sh statsbar-battery 1` → `"statsbar-battery":true`; no new panel.
  5. `press.sh statsbar-cpu 3` (right click) → `"statsbar-cpu":true`.
  6. With CPU's menu still open from step 5, `press.sh statsbar-memory 1 1 true` → no open menu
     and no panel page change: stock GNOME, the press only closes the open menu (Requirement 6).
     Then `press.sh statsbar-memory 1` → no open menu; panels lists `--page memory`.
  7. `hub-click.sh "Show CPU"` (hides it), then `hub-click.sh "Show CPU"` again (shows it), then
     `press.sh statsbar-cpu 1` → no open menu; panels lists `--page cpu` (a new button, hooked).
  8. `gnome-extensions disable statsbar@matrixi.github.io; press.sh statsbar-cpu 1` →
     `"statsbar-cpu":true`. Then `gnome-extensions enable statsbar@matrixi.github.io; press.sh statsbar-cpu 1`
     → no open menu, panel opens. This is the `enable()` scan over existing buttons, the
     lock/unlock path.

  Finally, `grep -i statsbar "$XDG_RUNTIME_DIR/shell.log"` shows no error, and the `pgrep` check from
  Task 1 shows the user's daemon and the other headless shell untouched.
- [x] Run `./scripts/check.sh`. Expect exit 0 and `clicks: all 14 passed`.
- [x] Ready to commit: `extension/statsbar@matrixi.github.io/metadata.json extension/statsbar@matrixi.github.io/extension.js`,
  message "feat(extension): open the CPU and Memory pages on a single click"

### Task 4: Install and uninstall script

**Type:** feature · **Test level:** integration (real file system in a temporary
`XDG_DATA_HOME`; `gnome-extensions` stubbed on `PATH`, because the real one talks to the user's
running Shell)
**Files:**
- Create `scripts/install-extension.sh` and `scripts/test-install-extension.sh` (both mode 0755).
- Modify `scripts/check.sh`: append `./scripts/test-install-extension.sh` after the line Task 2
  added.

**Interfaces:** `scripts/install-extension.sh [--uninstall]`. Exit 0 on success, 1 when installing
without `gnome-extensions`, 2 on an unknown argument. Uninstall works without it. (test added in review fix round 1 of checkpoint 2).

- [x] Write the failing test `scripts/test-install-extension.sh` — deviated: the shipped file adds a ninth case (uninstall without `gnome-extensions`, review fix round 1 of checkpoint 2); the file is authoritative, not this listing:

```bash
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
for file in metadata.json extension.js clicks.js; do
    cmp -s "$root/extension/$uuid/$file" "$target/$file" || fail "install copies $file"
    [[ "$(stat -c %a "$target/$file")" == "644" ]] || fail "$file is mode 644"
done
[[ "$(find "$target" -mindepth 1 | wc -l)" == "3" ]] || fail "install copies only the three extension files"
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
echo "install-extension: all passed"
```
- [x] Run `./scripts/test-install-extension.sh`. Expect FAIL: bash prints
  `.../scripts/install-extension.sh: No such file or directory`, exit 127.
- [x] Write `scripts/install-extension.sh` — deviated: the shipped script splits the missing-`gnome-extensions` uninstall into its own branch and message (review fix round 1 of checkpoint 2); the file is authoritative, not this listing:

```bash
#!/usr/bin/env bash
# Installs StatsBar's GNOME Shell extension for this user, or removes it with --uninstall
# (ADR 0001). On Wayland the Shell discovers a new extension, and loads changed code, only at the
# next login, so the first install cannot enable it yet: run this again after logging back in.
set -euo pipefail

uuid="statsbar@matrixi.github.io"
source_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/extension/$uuid"
target="${XDG_DATA_HOME:-$HOME/.local/share}/gnome-shell/extensions/$uuid"
files=(metadata.json extension.js clicks.js)

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
            # Fails ("doesn't exist") when the Shell never discovered it; the files go either way.
            if ! command -v gnome-extensions >/dev/null || ! gnome-extensions disable "$uuid"; then
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
```

- [x] Run `./scripts/test-install-extension.sh`. Expect PASS: eight `ok -` lines and — deviated: nine after review fix round 1 (checkpoint 2)
  `install-extension: all passed`, exit 0.
- [x] Append `./scripts/test-install-extension.sh` to `scripts/check.sh`, after
  `./scripts/check-extension.sh`. Run `./scripts/check.sh`. Expect exit 0.
- [x] Ready to commit: `scripts/install-extension.sh scripts/test-install-extension.sh scripts/check.sh`,
  message "feat(extension): install and uninstall the extension with a script"

### Task 5: Verification & Docs

**Type:** docs · **Test level:** e2e
**Files:**
- `README.md`: the Status paragraph at lines 6–8, and the plan list.
- `docs/plans/2026-11-16-rust-statsbar-roadmap.md`: the GNOME tray-click row in the risk table,
  line 119.
- This plan.

- [x] In `README.md`, replace the two lines

  ```
  CPU and Memory pages (statsbar-panel), opened by a left click on their icons or from "Open details…"
  in their menus; run with
  ```

  (whatever their exact wrapping is after Plan 04c) with

  ```
  CPU and Memory pages (statsbar-panel), opened by a single left click on their icons with the
  StatsBar GNOME Shell extension installed (`./scripts/install-extension.sh`, log out and back in,
  then run it again), by a double click without it, or from "Open details…" in their menus; run with
  ```

  Then add to the plan list:

  ```
  - [Plan 04d — single-click extension](docs/plans/2026-11-26-rust-statsbar-04d-click-extension.md):
    a GNOME Shell extension so a single left click on CPU or Memory opens its page
    ([ADR 0001](docs/adr/0001-single-click-opens-panel-extension.md)).
  ```

  RED: n/a — no executable behavior, checked by reading the rendered diff.
- [x] In the roadmap row at line 119, add to its middle cell: "; since
  [ADR 0001](../adr/0001-single-click-opens-panel-extension.md) a GNOME Shell extension makes a
  single left click on CPU or Memory open the page".
- [x] Run `./scripts/check.sh` and `cargo build --workspace --release --locked`.
  - `check.sh`: expect exit 0. Quote the Rust totals, `clicks: all 14 passed` and
    `install-extension: all passed`.
  - The build: expect success.
- [x] Re-run Task 3's GREEN steps 1, 4 and 5 through the harness on the final tree; expect the same
  lines.
- [ ] (user step — implementer denied by permission classifier; not self-verified) Real install on this session: run `./scripts/install-extension.sh`. Expect the logout hint
  (first install on Wayland). Report that the user must log out and back in, then run the script
  again. After they confirm, check that `gnome-extensions info statsbar@matrixi.github.io` shows
  `State: ACTIVE`, and the user confirms a single left click on CPU opens the page. If the user has
  not logged out yet, report the session check as unverified, with the harness runs from Task 3
  as the evidence.
- [x] Note for the PR: CI runs `check.sh`, including the gjs tests and the install-script tests. The
  extension's Shell behavior was verified only through the headless-shell harness and on this
  session.
- [x] Ready to commit: `README.md docs/plans/2026-11-16-rust-statsbar-roadmap.md docs/plans/2026-11-26-rust-statsbar-04d-click-extension.md docs/adr/0001-single-click-opens-panel-extension.md`,
  message "docs: record the single-click extension"
