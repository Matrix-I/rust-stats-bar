# 0001. A GNOME Shell extension makes a single left click open the panel

- **Status:** Accepted (user, 2026-11-26)
- **Date:** 2026-11-26   **Deciders:** user

## Context

The user wants a single left click on the CPU or Memory tray icon to open the details panel, with
no "Open details…" step. Commit 155e203 made `statsbard` answer `StatusNotifierItem.Activate` by
opening the page, but GNOME draws the icons through Ubuntu's AppIndicator extension
(`ubuntu-appindicators@ubuntu.com`, shell versions 45–50). That extension opens the menu on a single
primary click and calls `Activate` only on a double click
(`/usr/share/gnome-shell/extensions/ubuntu-appindicators@ubuntu.com/indicatorStatusIcon.js:381-446`).
Nothing the daemon publishes over D-Bus changes that: with `ItemIsMenu` or an `UnknownMethod` reply,
the extension opens the menu even sooner (`appIndicator.js:855-858`, `indicatorStatusIcon.js:439-444`).
Only code running inside GNOME Shell can change what a click does.

## Drivers

| Driver | Target or evidence |
|---|---|
| A single primary click on CPU or Memory opens that page | The user's request, 2026-11-26 (confirmed) |
| Battery, Fan, Network and the hub keep opening their menu on a left click | (confirmed), 2026-11-26 |
| Right click keeps the menu, including "Open details…" | No change asked; the menu carries the at-a-glance rows (roadmap risk table, `docs/plans/2026-11-16-rust-statsbar-roadmap.md:119`) |
| Lowest idle CPU and RAM | Roadmap Confirmed Requirement 2. The extension runs inside `gnome-shell`, so its idle cost is GNOME Shell's |
| Small and simple | (confirmed): the user picked the small extension that only changes the click, not one that draws its own items |
| Target is this machine only | Ubuntu 26.04, GNOME Shell 50.1 on Wayland (roadmap Confirmed Requirement 5) |
| The daemon works alone | Without the extension, nothing breaks: a single click opens the menu and a double click opens the panel, as today |
| Install and uninstall are scripted in this work | (confirmed), 2026-11-26: `scripts/install-extension.sh`, with `--uninstall` |
| Testable without logging out | `gnome-shell --devkit` exists on this machine (`gnome-shell --help`) |

## Constraints

- GNOME Shell 45+ extensions are ES modules with `metadata.json` and `extension.js` exporting an
  `Extension` subclass with `enable()`/`disable()`. Source: GNOME Shell extension docs
  (gjs.guide/extensions).
- On Wayland, GNOME Shell discovers a newly installed extension only after the session restarts
  (one logout and login). Source: same docs. `gnome-shell --devkit` runs a nested shell for tests.
- No `statsbard` dependency on GTK or GNOME Shell (roadmap Architecture).

## Options

### A. Keep the current design (double click)

A single click opens the menu, a double click opens the panel. Migration: none. Rollback: none.
Cost: none. It does not meet the first driver.

### B. Small extension that intercepts the primary click on our two icons (recommended)

An extension `statsbar@matrixi.github.io` in `extension/` of this repository, about 60 lines of JS.
On `enable()` it finds the AppIndicator buttons in `Main.panel.statusArea` whose keys start with
`appindicator-` and whose `_indicator.id` is `statsbar-cpu` or `statsbar-memory`. It watches
`Main.panel.statusArea` changes (the buttons come and go when the hub hides an item or the daemon
restarts). For a primary-button press on one of those buttons it calls the button's
`_indicator.open(x, y, time)` (the same call the double-click path makes, `indicatorStatusIcon.js:388`,
which also passes the XDG activation token, `appIndicator.js:843-852`) and stops the event, so the
menu does not open. It does that either by connecting `button-press-event` and returning
`Clutter.EVENT_STOP`, or by overriding `vfunc_button_press_event` on those instances. Task 1 of the
plan picks one in `gnome-shell --devkit` (see Open Items). `disable()` removes every handler.
Any other button, item or mouse button is left untouched.

- Migration: install the extension with `scripts/install-extension.sh` (copy to
  `~/.local/share/gnome-shell/extensions/<uuid>/`, then `gnome-extensions enable <uuid>`), then log
  out once. The daemon does not change.
- Rollback: `scripts/install-extension.sh --uninstall`, or `gnome-extensions disable <uuid>`. Behavior
  returns to option A at once.
- Cost: no new process, no polling. One signal handler per matching button and one
  `statusArea` watch inside `gnome-shell`.
- Risk: it reads two private fields of another extension (`_indicator`, and `open()`). The id is
  read at click time, because the item's properties load after its button is added
  (`statusNotifierWatcher.js:120-122`). If Ubuntu renames `_indicator`, nothing matches and clicks
  fall back to option A. If our item has no `open()`, the extension logs once and does the same.

### C. Extension that draws its own CPU and Memory items

The extension draws its own panel buttons, labels and menus. It reads the figures from `statsbard`
(the IPC socket, or the item's D-Bus properties). The daemon stops registering those SNI items
while the extension runs. Migration: a new data path into GJS, coexistence logic in the daemon,
menus rebuilt in JS. Rollback: the daemon registers the items again when the extension is absent.
Cost: much more JS running inside `gnome-shell`, and a second implementation of labels and menus. It
does not depend on Ubuntu's extension internals. The user declined it (Drivers: small and simple).

## Comparison

| Driver | A. keep | B. intercept click | C. own items |
|---|---|---|---|
| Single click opens the page | ❌ double click only (`indicatorStatusIcon.js:381-446`) | ✅ if the event stops before the extension's handler (Open Items) | ✅ |
| Other icons keep their menu | ✅ | ✅ matches only `statsbar-cpu` and `statsbar-memory` | ✅ by design |
| Right-click menu kept | ✅ | ✅ only the primary button is intercepted | ⚠️ rebuilt in JS |
| Idle CPU / RAM | none | negligible: event handlers only, no timers | higher: labels updated in the shell |
| Size | 0 | about 60 lines of JS and one script | hundreds of lines, plus daemon changes |
| Daemon works alone | ✅ | ✅ unchanged | ⚠️ needs coexistence logic |
| Fragility | none | depends on `_indicator` and `open()` in `ubuntu-appindicators` 45–50; falls back to A | depends only on the public Shell API |
| Testable without logout | n/a | ✅ `gnome-shell --devkit` | ✅ same |

## Decision

**B**. It is the only small option that meets the first driver. The user picked "small"
and "keep the menu on the other icons". Its one weakness, the private fields, fails safe back to
today's behavior.

## Consequences

- Positive: a single click opens the CPU or Memory page. The daemon, protocol and config do not
  change.
- Negative: a JS component in a Rust repository. `./scripts/check.sh` does not cover it, so it is
  verified in `gnome-shell --devkit` and on the session. One logout is needed after the first
  install. An Ubuntu update to `ubuntu-appindicators` can silently bring back the double click.
- Follow-up: Plan 04c's README line "opened by a left click" is only true once the extension is
  installed. The plan for this ADR rewords it. Plan 11 folds `install-extension.sh` into the
  general install.
- Revisit when: `ubuntu-appindicators` changes `_indicator` or `open()`, or calls `Activate` on a
  single click (then remove the extension). Also revisit if the target grows beyond this machine
  (then extensions.gnome.org review and option C come back).

## Open Items

- Spike (first task of the plan, timeboxed, in `gnome-shell --devkit`): does a `button-press-event`
  handler that returns `Clutter.EVENT_STOP` run before `IndicatorStatusIcon.vfunc_button_press_event`
  and keep the menu shut? If not, override `vfunc_button_press_event` on the matching instances.
