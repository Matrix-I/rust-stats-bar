# Plan 03b — text labels end-to-end run

Run on 2026-11-21 (about 22:55–22:58 +07) on Ubuntu 26.04.1 LTS, GNOME Shell 50.1, Wayland, with
Ubuntu AppIndicators (`gnome-shell-ubuntu-extensions` 50.26.04.8ubuntu). The built-in 1920×1080
display ran at a logical scale of 1.25, with Mutter's `scale-monitor-framebuffer` on. The run used
the release build of `statsbard` from the working tree of `feature/tray-text-labels` (base
d993c66, Tasks 1–4 staged and uncommitted), PID 360042, from
[the Plan 03b plan](../plans/2026-11-21-rust-statsbar-03b-text-labels.md).

Unlike Plan 03, the daemon ran with the user's own `~/.config` (no `XDG_CONFIG_HOME` override) and
without `strace`. Clicks were sent as dbusmenu `Event` calls over `busctl --user`, not made in the
GNOME UI. Screenshots came from the Desktop Screenshot portal, at 1920×1080 device pixels; the
panel crops are kept outside Git.

## Results

| Check | Expected | Observed |
|---|---|---|
| Gates | `./scripts/check.sh` passes | Exit 0: 211 passed, 0 failed, 1 ignored (the child half of `ayatana_label`) |
| Audit | `cargo audit` finds no vulnerability | Exit 0, no vulnerability; 3 allowed "unmaintained" warnings, see below |
| Build and source | Release build; `ksni` comes from `vendor/ksni` | `cargo build --release -p statsbard` succeeded; `cargo tree -p statsbard -i ksni` printed `ksni v0.3.7 (vendor/ksni)` |
| Properties | Each item publishes its icon, label, guide and title | Six items; see the table below |
| Battery label | Matches `BAT0` | `BAT0/capacity` 80, status `Not charging`: icon `battery-level-80-symbolic`, label `80%` |
| Fan label | Share of `fan1_min`–`fan1_max`, clamped at 100 % | `fan1_input` 6021 RPM, `fan1_min` 0, `fan1_max` 5300: label `100%` |
| `XAyatanaNewLabel` | Emitted when a label changes | 7 signals in 10 s, each with label and guide; only CPU and Network changed |
| CPU value | Follows the load | Labels 20–37 % across ticks; `top -bn2 -d1` gave 24.9 us + 3.0 sy, about 28 % busy, within the sample-to-sample swing |
| Memory value | Matches `free -m` | 4850 of 15288 MB is 31.7 %; label `32%` |
| Network value | Download rate of the title, banded by `rate_text` | Label `↓3K` with title `Network ↓ 3.1 KB/s ↑ 15.5 KB/s` |
| Panel, all five items | Every label legible | **Not met.** GNOME ellipsized every label to one character (`8…`, `3…`, `5…`, `1…`), and the network label overlapped the clock; see below |
| Panel, CPU and Memory only | Labels legible next to their icons | `34%` and `43%`, crisp next to their icons, and nothing touched the clock |
| Hide | Item vanishes; `false` in the config | *Show Network* unticked: network item gone, hub, cpu, memory, battery and fan remained; `network = false` |
| Show again | Item back with its icon and label | Reticked: back with `network-transmit-receive-symbolic` and `↓4K` |
| Threads | The four of Plan 03 | `ps -T -p 360042`: 4 threads, `statsbard` ×2, `async-io`, `blocking-1` |
| Memory cost | About Plan 03's | Pss 2789 kB (Plan 03: 2997 kB) |
| Charging icon | `battery-level-N0-charging-symbolic` while charging | **Not seen live.** The battery was `Not charging` at 80 %, above the 75 % start threshold; covered only by `the_battery_icon_floors_to_tens_and_has_the_bolt_only_while_charging` |

## Properties

From `busctl --user get-property <name> /StatusNotifierItem org.kde.StatusNotifierItem Id
IconName XAyatanaLabel XAyatanaLabelGuide Title`:

| Id | IconName | XAyatanaLabel | XAyatanaLabelGuide | Title |
|---|---|---|---|---|
| `statsbar-hub` | `utilities-system-monitor` | | | StatsBar |
| `statsbar-cpu` | `computer-chip-symbolic` | `30%` | `100%` | CPU 30 % |
| `statsbar-memory` | `drive-harddisk-solidstate-symbolic` | `32%` | `100%` | Memory 32 % |
| `statsbar-battery` | `battery-level-80-symbolic` | `80%` | `100%` | Battery 80 % |
| `statsbar-fan` | `weather-windy-symbolic` | `100%` | `100%` | Fan 100 % |
| `statsbar-network` | `network-transmit-receive-symbolic` | `↓6K` | `↓999M` | Network ↓ 6.3 KB/s ↑ 17.9 KB/s |

The hub publishes an empty label and an empty guide.

## Commands

```bash
./scripts/check.sh
cargo audit
cargo build --release -p statsbard
cargo tree -p statsbard -i ksni
timeout 10 dbus-monitor --session "type='signal',member='XAyatanaNewLabel'"
top -bn2 -d1
free -m
ps -T -p 360042
grep Pss /proc/360042/smaps_rollup
```

The signal capture showed, for example, (`37%`, `100%`), (`↓4K`, `↓999M`), (`20%`, `100%`),
(`↓3K`, `↓999M`), (`30%`, `100%`), (`20%`, `100%`) and (`↓8K`, `↓999M`) as label and guide.

## Panel

All five items shown, GNOME ellipsized every label to one character. The right side of the panel
also held the hub, three other apps' tray icons, a screen-sharing indicator, the input source and
the system status area. With Fan, Battery and Network hidden, CPU and Memory showed full labels.
The truncation is therefore the panel running out of width, not the patch.

The extension's `indicatorStatusIcon.js` puts the label in an `St.Bin` holding an `St.Label`, with
no width reservation. `XAyatanaLabelGuide` is only listed in `appIndicator.js` and never read, so
the guide has no effect in GNOME and each label's width follows its text.

**The user's verdict (2026-11-21, in chat, after seeing these findings):** "Ghi gap, làm tiếp" —
accept the truncation with all five items shown as a recorded gap, and continue. The user did not
separately state that the untruncated labels are legible; the evidence for that is the crop with
two items shown.

## Gaps

1. **Truncation with all five items shown (accepted by the user).** At 125 % on a 1920×1080 panel
   this crowded, GNOME ellipsizes the labels. Hiding items through the hub is the mitigation
   available today.
2. **The guide is ignored.** The Ubuntu extension 50.26 never reads `XAyatanaLabelGuide`, so a
   label's width follows its text. What happens to neighbouring items when the width changes, for
   example `9%` to `100%`, was not observed separately.
3. **The charging icon was not seen live**, only in tests.
4. **The run used the user's real `~/.config`.** The hide and show clicks wrote
   `~/.config/statsbar/config.toml`. Fan, Battery and Network were also hidden and reshown for the
   panel comparison; the config ended with all five items `true`. Reticking *Show Network* gave the
   item the menu id 26 instead of 11, since `ksni` renumbers after a layout change.
5. **`cargo audit` warns about three unmaintained crates:** RUSTSEC-2026-0370 (`zvariant` 5.15.1),
   RUSTSEC-2026-0371 (`zbus_names` 4.3.5) and RUSTSEC-2026-0372 (`zvariant_derive` 5.15.1). No
   fixed upgrade exists, and `Cargo.lock` on `main` gives the same warnings, so this plan did not
   cause them.
6. **Clicks were not made in the GNOME UI.** They were dbusmenu `Event` calls, as in Plan 03.
