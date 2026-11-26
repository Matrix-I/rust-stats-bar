# Plan 04d — single-click handler spike

Run on 2026-11-26 on Ubuntu 26.04.1 LTS, kernel 7.0.0-44, GNOME Shell 50.1, gjs 1.88.0, from
[the Plan 04d plan](../plans/2026-11-26-rust-statsbar-04d-click-extension.md) and
[ADR 0001](../adr/0001-single-click-opens-panel-extension.md). The release build of `statsbard`
and `statsbar-panel` came from `feature/panel-skeleton` at 0bd89b2, built with rustc 1.100.0 by
`cargo build --workspace --release --locked`.

The question: does a `button-press-event` handler connected to an AppIndicator panel button run
before `ubuntu-appindicators@ubuntu.com`'s `vfunc_button_press_event`, and can it stop the event
so a single primary click opens the page instead of the menu?

## Harness

A headless GNOME Shell (`--headless --virtual-monitor 1280x800 --mode=ubuntu --unsafe-mode`) on a
private session bus from `dbus-run-session`, with `XDG_RUNTIME_DIR` (mode 0700), `XDG_CONFIG_HOME`
and `XDG_DATA_HOME` under a throwaway directory, plus a `statsbard` that shows all five items.
Clicks were a Clutter virtual pointer driven over `org.gnome.Shell.Eval`; the result read
`Main.panel.statusArea` (which AppIndicator menus are open) and `global.display` (the shell's
window titles), and `pgrep -P` listed the daemon's `statsbar-panel` children. The harness is the
one described in [the extension end-to-end run](2026-11-26-04d-click-extension-e2e.md#how-it-was-run).

The user's `statsbard` (pid 239811) and another headless shell (pids 294877, 294881) ran
throughout; `pgrep` showed the same pids before and after every run.

Two timing waits were needed beyond the plan's scripts:

- An item registers with the watcher about a second before `ubuntu-appindicators` has built its
  button and loaded its dbusmenu layout, and a menu with no items does not open. The harness waits
  until all six buttons exist with menu items, and each press waits for its button's menu.
- `gnome-extensions enable` sent right after `gnome-extensions disable` was lost (state stayed
  `INACTIVE`). The run waits for `INACTIVE` before enabling, and for `ACTIVE` after.

## Baseline, no extension of ours

Inside the harness: one single left press on CPU, then a double left press on CPU.

```
result: (true, '"{\\"done\\":true,\\"menus\\":{\\"statsbar-hub\\":false,\\"statsbar-cpu\\":true,\\"statsbar-memory\\":false,\\"statsbar-battery\\":false,\\"statsbar-fan\\":false,\\"statsbar-network\\":false},\\"windows\\":[]}"')
panels:
result: (true, '"{\\"done\\":true,\\"menus\\":{\\"statsbar-hub\\":false,\\"statsbar-cpu\\":false,\\"statsbar-memory\\":false,\\"statsbar-battery\\":false,\\"statsbar-fan\\":false,\\"statsbar-network\\":false},\\"windows\\":[\\"Desktop Icons 1\\",\\"StatsBar\\"]}"')
panels: /home/…/target/release/statsbar-panel --page cpu
```

A single press opens CPU's menu and no panel; a double press opens the CPU page, and the panel
window `StatsBar` belongs to the headless shell. This is today's behavior (ADR 0001 option A).

## Spike extension

`statsbar-spike@test` scanned `Main.panel.statusArea` in `enable()` and connected a
`button-press-event` handler to each AppIndicator button. For a primary press on `statsbar-cpu` it
called `button._indicator.open(...)` and returned `Clutter.EVENT_STOP`; everything else
propagated. It was disabled and enabled again once the items were up.

Inside the harness, with the spike installed and enabled: `gnome-extensions disable`, wait for
`INACTIVE`, `gnome-extensions enable`, wait for `ACTIVE`. Then a single left press on CPU, a single
left press on Battery, and a right press on CPU.

```
result: (true, '"{\\"done\\":true,\\"menus\\":{\\"statsbar-network\\":false,\\"statsbar-fan\\":false,\\"statsbar-battery\\":false,\\"statsbar-memory\\":false,\\"statsbar-cpu\\":false,\\"statsbar-hub\\":false},\\"windows\\":[\\"StatsBar\\"]}"')
panels: /home/…/target/release/statsbar-panel --page cpu
result: (true, '"{\\"done\\":true,\\"menus\\":{\\"statsbar-network\\":false,\\"statsbar-fan\\":false,\\"statsbar-battery\\":true,\\"statsbar-memory\\":false,\\"statsbar-cpu\\":false,\\"statsbar-hub\\":false},\\"windows\\":[\\"StatsBar\\"]}"')
panels: /home/…/target/release/statsbar-panel --page cpu
result: (true, '"{\\"done\\":true,\\"menus\\":{\\"statsbar-network\\":false,\\"statsbar-fan\\":false,\\"statsbar-battery\\":false,\\"statsbar-memory\\":false,\\"statsbar-cpu\\":true,\\"statsbar-hub\\":false},\\"windows\\":[\\"StatsBar\\"]}"')
panels: /home/…/target/release/statsbar-panel --page cpu
```

- A single primary press on CPU opened no menu and one `--page cpu` panel.
- A single press on Battery still opened Battery's menu.
- A right press on CPU still opened CPU's menu.

The first spike run sent `enable` straight after `disable`; the spike stayed `INACTIVE`, so its
handler was never connected and CPU's menu opened. That run measured the lost `enable`, not the
handler.

## Other extensions

`gnome-extensions list --enabled` inside the harness (session mode `ubuntu`):

```
statsbar-spike@test
ding@rastersoft.com
snapd-prompting@canonical.com
snapd-search-provider@canonical.com
tiling-assistant@ubuntu.com
ubuntu-appindicators@ubuntu.com
ubuntu-dock@ubuntu.com
web-search-provider@ubuntu.com
```

`grep -rl addToStatusArea` over their directories, excluding `ubuntu-appindicators`, found only
`tiling-assistant@ubuntu.com/src/extension/layoutsManager.js`, which calls
`Main.panel.addToStatusArea('tiling-assistant@leleat-on-github', …)` for its own indicator and
does not wrap the method. None of them wraps `addToStatusArea`.

## Conclusion

Handler: signal works
