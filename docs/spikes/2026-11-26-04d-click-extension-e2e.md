# Plan 04d — single-click extension end-to-end run

Run on 2026-11-26 on Ubuntu 26.04.1 LTS, kernel 7.0.0-44, GNOME Shell 50.1, gjs 1.88.0, from
[the Plan 04d plan](../plans/2026-11-26-rust-statsbar-04d-click-extension.md) and
[ADR 0001](../adr/0001-single-click-opens-panel-extension.md). The run used the release build of
`statsbard` and `statsbar-panel` from `feature/panel-skeleton` (on 0bd89b2, with this plan's
extension in the working tree), built with rustc 1.100.0 by
`cargo build --workspace --release --locked`. The extension was the four files `metadata.json`,
`extension.js`, `clicks.js` and `hooks.js` from `extension/statsbar@matrixi.github.io/`.

## How it was run

Everything ran in a headless GNOME Shell, never in the logged-in session:

- `dbus-run-session` gave a private session bus. `XDG_RUNTIME_DIR` (mode 0700), `XDG_CONFIG_HOME`,
  `XDG_DATA_HOME`, `XDG_CACHE_HOME` and `XDG_STATE_HOME` pointed into a throwaway directory, which
  was deleted afterwards.
- `gnome-shell --headless --virtual-monitor 1280x800 --mode=ubuntu --unsafe-mode`. The `ubuntu`
  session mode enables `ubuntu-appindicators@ubuntu.com`, which draws the tray icons.
  `--unsafe-mode` allows `org.gnome.Shell.Eval`.
- The extension was copied into the private `XDG_DATA_HOME`. For the main run it was listed in
  `org.gnome.shell enabled-extensions` from the start. For the rollback run it was copied without
  enabling, and was enabled once the icons were up, so that it came after ubuntu-appindicators.
- `statsbard` ran with `WAYLAND_DISPLAY` set to the headless shell, with all five items on. The run
  waited until the watcher listed its six items, and until the Shell had built six buttons with
  their dbusmenu items loaded.
- A press: an `org.gnome.Shell.Eval` script made a Clutter virtual pointer
  (`create_virtual_device(POINTER_DEVICE)`). It moved the pointer to the centre of the item's
  panel button and sent button press and release events, once or twice. It then waited the
  double-click time plus 300 ms and read which AppIndicator menus were open
  (`Main.panel.statusArea`). After a further second it read the shell's window titles
  (`global.display.list_all_windows()`). `pgrep -P <daemon>` listed the `statsbar-panel` children
  and their `--page`.
- Hub menu clicks were dbusmenu `Event <id> clicked` calls, with the id taken from `GetLayout`.
- `gnome-extensions disable` and `enable` each waited for the state to read `INACTIVE` or `ACTIVE`.
  An `enable` sent right after a `disable` is otherwise lost.

A panel window titled `StatsBar` in the headless shell's window list shows the panel reached the
headless shell, not the desktop. The user's own `statsbard` and an unrelated headless shell kept
the same pids through every run.

## Results

Main run, with the extension enabled before the icons registered. "No menu" means every StatsBar
AppIndicator menu read closed.

| # | Action | Expected | Seen |
|---|---|---|---|
| 1 | Single left click on CPU | CPU page, no menu (hook via `addToStatusArea`) | No menu; `StatsBar` window; panel `--page cpu` |
| 2 | Single left click on Memory, panel open | No menu; still one panel (the second launch hands over) | No menu; one panel |
| 3 | Panel closed, double left click on CPU | CPU page, no menu | No menu; panel `--page cpu` |
| 4 | Single left click on Battery | Battery's menu, no new panel | Battery menu open; panel unchanged |
| 5 | Right click on CPU | CPU's menu | CPU menu open |
| 6a | Panel closed, CPU menu kept open, single left click on Memory | The click only closes the open menu (stock GNOME) | No menu; no window; no panel |
| 6b | Single left click on Memory | Memory page | No menu; panel `--page memory` |
| 7 | Hub *Show CPU* off and on again, then single left click on CPU | CPU page (the new button is hooked) | No menu; panel `--page cpu` |
| 8a | Extension disabled, single left click on CPU | CPU's menu, no panel | CPU menu open; no panel |
| 8b | Extension enabled again, single left click on CPU | CPU page (the `enable()` scan) | No menu; panel `--page cpu` |

The Shell log had no line mentioning `statsbar`.

In this run the extension was enabled before ubuntu-appindicators. So disabling it in 8a made the
Shell also disable and re-enable ubuntu-appindicators, which rebuilt the buttons. 8a therefore
shows that `addToStatusArea` was restored, not that the old buttons lost their handlers. The
rollback run covers that.

Rollback run, with the extension enabled after the icons were up:

| # | Action | Expected | Seen |
|---|---|---|---|
| a | Single left click on CPU | CPU page | No menu; panel `--page cpu` |
| b | CPU button tagged in Eval, panel closed, extension disabled, single left click on CPU | Same button (tag present); CPU's menu; no panel | Tag present; CPU menu open; no panel |
| c | Extension enabled, a counter wrapped around the CPU item's `open()`, single left click on CPU | One `open()` call, one panel, no menu | Count 1; panel `--page cpu`; no menu |

The extension order read back from the Shell put `statsbar@matrixi.github.io` last, after
`ubuntu-appindicators@ubuntu.com`. So in b the buttons survived the disable, and the menu opening
shows the handler was disconnected. In c, a count of 1 shows that the re-enable left exactly one
handler.

## Commands

```bash
cargo build --workspace --release --locked
dbus-run-session -- bash -c '…'      # everything below ran inside this
gsettings set org.gnome.shell enabled-extensions "['statsbar@matrixi.github.io']"   # main run only
env -u DISPLAY -u WAYLAND_DISPLAY XDG_SESSION_TYPE=wayland gnome-shell --headless \
    --virtual-monitor 1280x800 --wayland-display sb-e2e-0 --mode=ubuntu --unsafe-mode &
env -u DISPLAY -u XAUTHORITY WAYLAND_DISPLAY=sb-e2e-0 target/release/statsbard &
gdbus call --session -d org.gnome.Shell -o /org/gnome/Shell -m org.gnome.Shell.Eval "<press script>"
gdbus call --session -d org.kde.StatusNotifierItem-<pid>-1 -o /MenuBar \
    -m com.canonical.dbusmenu.GetLayout 0 -1 '["label"]'
gdbus call --session -d org.kde.StatusNotifierItem-<pid>-1 -o /MenuBar \
    -m com.canonical.dbusmenu.Event <id> clicked '<int32 0>' 0
gnome-extensions disable|enable statsbar@matrixi.github.io; gnome-extensions info statsbar@matrixi.github.io
pgrep -P <daemon> -a statsbar-panel; pkill -P <daemon> statsbar-panel
```

## Not seen

1. **The logged-in session.** The run used a headless shell with a virtual pointer. A click with a
   real mouse on the real panel, the install by `scripts/install-extension.sh`, and loading at the
   next login were not covered here.
2. **Which page a handed-over panel shows.** In step 2 the panel was already open. The window title
   is always `StatsBar`, so the run saw that one panel stayed, not that it switched to Memory.
3. **An item without `open()`, or with `supportsActivation === false`.** The installed
   ubuntu-appindicators has both, and statsbard answers `Activate`. These paths are covered only by
   the gjs unit tests in `extension/tests/`.
