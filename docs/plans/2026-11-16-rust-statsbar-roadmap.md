# rust-statsbar — Roadmap

> **For agentic workers:** this file is the map, not an executable plan. Each numbered plan below gets
> its own `docs/plans/YYYY-MM-DD-rust-statsbar-NN-<name>.md`, written with `writing-plans-strict`
> right before it is executed, because its paths and symbols must be ones that exist by then.
> Plan 01 is already written: `docs/plans/2026-11-16-rust-statsbar-01-core.md`.

**Goal:** a Linux counterpart of [StatsBar](https://github.com/Matrix-I/stats-bar) (macOS menu-bar
hardware inspector) with the same panels and behaviour wherever Linux exposes the data, written in
Rust, and tuned first for the lowest idle CPU and memory.

**Architecture:** three processes. `statsbard` is a small session daemon with no GUI toolkit: it polls,
draws the tray items and serves snapshots. `statsbar-panel` is a GTK4/libadwaita window that the daemon
spawns on demand and that exits when closed. `statsbar-fan-helper` is a root helper used only for fan
control. Pure arithmetic lives in `crates/core`, which is the only crate tests can pin without hardware.

**Tech stack:** Rust 1.99 (edition 2024) · `ksni` 0.3.7 *(vendored at `vendor/ksni` with `XAyatanaLabel`, see its `PATCHES.md`)* (StatusNotifierItem tray, `async-io` feature) ·
`zbus` 5.19.0 (NetworkManager, BlueZ, UPower, notifications; the major version `ksni` 0.3.7
uses, so only one copy is linked) · `gtk4` 0.11.5 + `libadwaita` 0.9.2 (panel only) · `rustix` 1.1.6 ·
`serde` 1.0.229 + `serde_json` 1.0.154 (IPC) · `toml` 1.2.1 (config) ·
`ureq` 3.5.2 (public IP). Names and versions were checked against crates.io on 2026-11-16; their APIs
were not, except the `ksni::Tray` method list. Plan 02a built and ran `ksni` and `zbus` together on
2026-11-18.

**Branch:** one `feature/<short-desc>` branch per plan in the `rust-statsbar` repository; never commit
on `main`. One exception, approved by the user: the Plan 02b label renderer shares the Plan 02a
branch `feature/tray-spikes`, as recorded in
[its plan](2026-11-18-rust-statsbar-02b-label-renderer.md).

---

## Confirmed Requirements

1. Rust, with StatsBar's functions and GUI reproduced as far as Linux allows. *(user)*
2. GUI form: asked which of three forms to use, the user answered with a criterion — lowest CPU and RAM.
   Chosen on that criterion: tray items plus an on-demand panel process, daemon without GTK.
   *(delegated)* See "Why this shape" below.
3. Beyond the six panels and the hub, v1 includes: temperature alerts, public IP with country flag,
   iPhone/Android battery, fan control. *(user)*
4. Panel toolkit: GTK4 + libadwaita, through `gtk4-rs` without relm4. *(user)* The first pick said
   relm4; the user dropped it on 2026-11-22, because one small window does not need a second runtime
   layer.
5. Hardware target: this machine only — Dell laptop, Intel UHD 620 (i915), `dell_smm` fan, Ubuntu 26.04,
   GNOME Shell 50.1 on Wayland. No AMD/NVIDIA code. *(user)*
6. The upstream repository belongs to the user; its code may be ported. *(user)*
7. CPU temperature alert at 90 °C, re-armed after cooling to 85 °C. iPhone alert as upstream: 39 °C,
   re-armed at 37 °C. Both editable in the config file. *(user)*
8. Fan Manual mode offers Low and High only, never Off. *(user)*
9. Memory pressure: Warning at PSI `some avg10 ≥ 10`, Critical at `full avg10 ≥ 10`. *(user)*
10. "Memory used" is `MemTotal − MemAvailable`, matching GNOME System Monitor and `free`. *(user)*
11. No update checking in v1; the app makes no network request except the public-IP lookup. *(user)*
12. Project and repository name `rust-statsbar`. *(user)* Crates and binaries keep the `statsbar` stem:
    `statsbar-core`, `statsbard`, `statsbar-panel`, `statsbar-fan-helper`. *(default; the user named
    only the project)*

The user settled Plan 04's questions on 2026-11-22. [Plan 04a](2026-11-22-rust-statsbar-04a-ipc.md)
lists all 17 decisions, those of Plan 04b included, under Confirmed Requirements.

## Open Questions

None. The repository lives at `~/Desktop/rust-statsbar`, with `origin` at
`https://github.com/Matrix-I/rust-stats-bar` (MIT licence).

## Global Constraints

From the user's global rules; a project `CLAUDE.md`, once the repo has one, wins on conflict.

- Test-first: happy path, edge cases, error paths. Untested code is reported incomplete, never done.
- Each test can fail without its change; assert observable behaviour. Mock only true boundaries. No
  sleeps, wall clock, ordering or network in tests.
- Never weaken, skip or delete a test to make it pass.
- Commit or push only when asked; plans therefore end tasks with `Ready to commit: …`, not `git commit`.
  Conventional Commits, imperative mood, one logical change per commit.
- Code, comments, names, commit messages and docs in English. Comments explain why.
- Every external call has a timeout: D-Bus calls, subprocesses, HTTP. Retries only when idempotent and
  bounded. Nothing unbounded.
- No host clock in logic or tests: timestamps are passed in, from a monotonic clock.
- Ask before adding or upgrading dependencies. A new dependency is checked for exact name, maintenance
  and licence, pinned, locked, and audited.
- Flag changes to authentication, authorization or privilege for review. Plan 10 is one.
- Final gate after the last edit: read the full diff; run `./scripts/check.sh`; self-review;
  `code-review-strict` for non-trivial, security or money changes; repeat if anything changed.
- **Project invariant (enforced by `scripts/check-core-purity.sh`):** `crates/core` does no I/O, reads
  no clock, spawns nothing, has no `unsafe` and no build script, and depends on nothing but `serde`.
- **Project invariant:** `statsbard` never links GTK and never starts a tokio runtime. Its threads are
  bounded and named in this list, measured by spike S2 (`docs/spikes/2026-11-17-s2-threads.md`): the
  main event loop, which also ticks the daemon's own `zbus` connection; `async-io`'s reactor thread;
  `ksni`'s single driver thread, shared by every tray item; and one worker that runs blocking calls
  (HTTP, `ideviceinfo`, `adb`) one at a time, each with its own timeout, so a slow call never stalls
  polling or the tray. Threads of the `blocking` pool that `zbus` uses while connecting are allowed;
  S2 found one (`blocking-1`) still present after 30 s, and Plan 11 measures whether it retires.
  *Reworded 2026-11-18 with the user's approval.*

## Technical Defaults

- Cargo workspace, one crate per process plus `core` and `readers` — keeps GTK out of the daemon's
  dependency graph by construction.
- Readers take a filesystem root (`&Path`, `/` in production) — tests run them against fixture trees
  captured from this machine, with serial numbers and MAC addresses replaced.
- Integration tests in `crates/<name>/tests/` rather than inline `mod tests` — the test file exists and
  fails before the module does, which is what test-first needs.
- IPC: newline-delimited JSON over a Unix socket at `$XDG_RUNTIME_DIR/statsbar.sock` — a few kilobytes
  per second at most, and debuggable with `socat`. First line each way is a protocol version. The
  socket is the user's only (mode 0600, peer uid checked), and the variable is required.
- Config: `~/.config/statsbar/config.toml`, every key optional — an older file keeps working after an
  upgrade. Written to a temporary file and renamed.
- Launch at login: `~/.config/autostart/statsbar.desktop` — the XDG convention.
- Notifications: `org.freedesktop.Notifications` through the `zbus` connection the daemon already has —
  no extra crate.
- Release profile: `opt-level = "s"`, fat LTO, one codegen unit, `panic = "abort"`, stripped.
- Tray label: a themed Yaru icon with the value as `XAyatanaLabel` text (Plan 03b).

## Risks & Rollback

| Risk | Mitigation | Revert |
|---|---|---|
| GNOME AppIndicator can render wide tray labels too small to read | [The 02b probe](../spikes/2026-11-18-02b-system-font.md) measured 5 px for the wide system-font label and 11 px for the square Stats Mini percentage. At 125 % scale the user found every 20×20 pixmap label hard to read ([spike S3](../spikes/2026-11-21-s3-panel-legibility.md)); Plan 03b shows the value as panel text ([end-to-end run](../spikes/2026-11-21-03b-text-labels-e2e.md)); with all five items shown the panel ran out of width at 125 % and GNOME ellipsized the labels, so hide items through the hub ([same run](../spikes/2026-11-21-03b-text-labels-e2e.md)) | A level icon with the figure in the menu remains a fallback |
| Tray and D-Bus threads count towards the daemon's budget | [Spike S2](../spikes/2026-11-17-s2-threads.md) measured the main loop driving `zbus`, one shared `ksni` driver, the `async-io` reactor, and one `blocking-1` thread still present after 30 s. Additional items cost about 68 kB Pss each | Settled by spike S2: the invariant lists the threads S2 measured. |
| On GNOME a tray click opens a menu, never a popover, and Wayland does not let the panel place itself under the icon | Designed around: the menu carries the at-a-glance rows, "Open details…" opens the panel where the compositor puts it | — |
| Ubuntu removes or changes the AppIndicator extension | The panel is also launchable from a `.desktop` entry, so the app degrades to a window | — |
| GPU usage comes from `/proc/<pid>/fdinfo`, which shows only this user's GPU clients; scanning every process is expensive | Stated in the UI as "your processes"; pids with a DRM fd are cached and rescanned every 30 s; nothing is read while the item is hidden and the panel closed | — |
| Fan control writes to hardware: a stuck manual state can overheat the laptop; `dell_smm` has three levels, and the BIOS may fight a manual setting | Manual never offers Off. Helper restores Auto on start, on exit, when its client disconnects, and after 10 s without a ping. Max and Manual last for the session only. Hardware steps are run with the user present | `statsbar-panel --uninstall-fan-helper` removes the unit, binary and socket; a reboot restores BIOS control regardless |
| The fan helper runs as root | Socket owned by the installing uid, mode 0600, peer uid checked with `SO_PEERCRED`; a strict line parser; writes only to two sysfs paths resolved at start; `code-review-strict` and a security review before merge | As above |
| Public IP lookup sends requests to `api.ipify.org`, `api6.ipify.org` and `ipinfo.io` | Config switch; 5 s timeout on the worker thread; fetched on a default-route change and at most every 10 min | Set `public_ip = false` |
| Crate APIs differ from what a plan assumes | Each plan reads docs.rs for the pinned version before any code is written into it | — |
| The memory and CPU budgets below are targets, not measurements | Measured by a script in Plan 11; a miss is reported with the numbers, not waved through | — |
| Persisted state: config file, autostart entry, helper install | All three are additive and removable; config keys are only ever added | Delete the two files; uninstall the helper |

---

## Why this shape (CPU and RAM first)

| Choice | Effect |
|---|---|
| Daemon without GTK | Idle cost is a D-Bus connection and a few open files, not a toolkit and a GL context |
| Panel is a separate process that exits on close | Zero panel memory except while you are looking at it |
| The tray menu carries the summary rows | Most glances never start the panel at all |
| One timer for all readers | One wake-up per tick, not one per metric |
| Hidden item ⇒ its reader is not scheduled | A metric you turned off costs nothing |
| Faster cadence only for the page that is open | 1 s polling is paid for one metric, briefly |
| Process lists, GPU scan, ping, phones only on demand | The expensive readers never run in the background |
| D-Bus signals for NetworkManager, BlueZ, UPower | No polling at all for state that changes rarely |
| Files kept open and re-read with `pread` | No open/close pair per sample |
| Tray label resent only when its text changes | An idle machine redraws almost nothing |

**Budget, to be measured in Plan 11:** daemon ≤ 15 MB PSS and ≤ 0.5 % of one core averaged over five
idle minutes with all six items visible; no panel process while the panel is closed.

**Cadence** (upstream's CPU reader checks for an open panel first: 1 s with it open, else 5 s in Low
Power Mode, else 2 s). The first matching row wins:

| State | Interval |
|---|---|
| That metric's page open, its item visible or hidden, under any profile | 1 s |
| Item hidden and its page closed | not polled |
| `power-saver` profile active | 5 s |
| Item visible, page closed | 2 s |

## Layout

```
Cargo.toml                    workspace, lints, release profile
crates/core/                  pure: parsers, arithmetic, formatting, protocol types
crates/readers/               sysfs, procfs and D-Bus readers → core types
crates/daemon/                statsbard: scheduler, tray, alerts, IPC server, config
crates/panel/                 statsbar-panel: GTK4/libadwaita window
crates/fan-helper/            statsbar-fan-helper: root, sysfs writes only
packaging/                    .desktop, autostart, systemd unit, polkit action
scripts/check.sh              fmt + clippy + tests + purity gate
scripts/check-core-purity.sh
docs/plans/
```

## Feature map

Linux sources were verified to exist on this machine on 2026-11-16 unless marked otherwise.

| Panel | Upstream shows | Linux source | Gap |
|---|---|---|---|
| Battery | Charge, health, capacities, cycles, voltage, power, time left, serial | `/sys/class/power_supply/BAT0/` — `capacity`, `charge_now`, `charge_full`, `charge_full_design`, `cycle_count`, `voltage_now`, `current_now`, `status`, `serial_number`, `manufacturer`, `model_name` | No rated cycle life, so no "cycles against rated life" bar |
| | Temperature | — | `BAT0` exposes no temperature here; the row is omitted |
| | Adapter | `/sys/class/power_supply/AC`, `ucsi-source-psy-USBC000:001` | |
| | SMC power rails | — | RAPL counters are root-only; omitted |
| | Fans against their floor and ceiling | hwmon `dell_smm`: `fan1_input`, `fan1_min` (0), `fan1_max` (5300), `fan1_target` | One fan |
| CPU | Per-core usage, user/system/idle | `/proc/stat` | iowait counts as idle |
| | Per-core temperature | hwmon `coretemp`: package + Core 0–3; logical → core via `topology/core_id` | |
| | Frequency | `cpufreq/scaling_cur_freq` | No efficiency/performance split on this CPU |
| | Thermal pressure | package temperature: Fair from 75 °C, Serious from 90 °C, Critical from 100 °C | Derived, not a kernel state; `temp1_max` and `temp1_crit` are both 100 °C here |
| | Low Power Mode | power-profiles-daemon active profile | |
| | Load average ÷ cores, uptime, chip name | `/proc/loadavg`, `/proc/uptime`, `/proc/cpuinfo` | |
| | Top processes, quit one | `/proc/<pid>/stat` deltas; `SIGTERM` after a confirmation, own processes only, never GNOME Shell, systemd, the GNOME session or StatsBar | No app icons |
| GPU | Three utilisation rings, history chart | `/proc/<pid>/fdinfo` `drm-engine-render`, `-video`, `-copy` deltas per `drm-client-id` | Own processes only; the three rings are render / video / copy, not device / render / tiler |
| | Model, frequency | PCI id via `/usr/share/misc/pci.ids` (not yet checked for presence); `gt_act_freq_mhz` | No shader-core count |
| | Power draw | — | Root-only; omitted |
| | Driver-mapped memory | fdinfo `drm-total-system0` | |
| Memory | App / Wired / Compressed / Cached / Free | `/proc/meminfo` → App / Kernel / Cached / Free; used = `MemTotal − MemAvailable`; plus swap and zswap | Compressed is a figure, not a bar segment |
| | Pressure | `/proc/pressure/memory`: Warning at `some avg10 ≥ 10`, Critical at `full avg10 ≥ 10`; the ring fills to (kernel + zswap) ÷ total | Without PSI the level shows "—", never Normal |
| | Top processes | `/proc/<pid>/statm`: RSS − shared | Not PSS, which is too slow and unreadable for other users' processes |
| Network | Interface, MAC, IPv4/6, DNS | NetworkManager over D-Bus | |
| | Wi-Fi name, signal, band, channel, rate | NetworkManager; dBm from `/proc/net/wireless` | |
| | Throughput, session totals | `/sys/class/net/<if>/statistics/` | |
| | Latency, jitter | unprivileged ICMP datagram socket (`ping_group_range` allows it here) | |
| | Public IP, country flag | `api.ipify.org`, `api6.ipify.org`, `ipinfo.io/country`, as upstream | |
| | Per-process traffic | — | Needs root or eBPF; omitted |
| Bluetooth | Connected accessories and battery | BlueZ `Device1` + `Battery1` over D-Bus | One level per device; no left/right/case |
| Hub | Overview rows, show/hide switches, launch at login | Tray item menu; config file; autostart entry | |
| | Attached displays | Mutter `DisplayConfig.GetCurrentState`; EDID for the diagonal | GNOME only |
| Phones | iPhone/iPad battery | `ideviceinfo` (apt `libimobiledevice-utils` 1.4.0; not installed) | Install hint shown when missing, as upstream |
| | Android battery | `adb shell dumpsys battery` (apt `adb`; not installed) | Same |
| Alerts | Hot iPhone notification | Desktop notification: iPhone 39 °C, CPU package 90 °C | No laptop-battery alert: no sensor |
| Fan control | Auto · Max · Manual through a root helper | `dell_smm` `pwm1`, `pwm1_enable` (root-writable) | Manual is Low or High, not a continuous RPM range |
| Updates | Sparkle | — | Not in v1 *(user)* |

## Plans

Each plan is one branch (one exception, under **Branch** above) and ends with its own verification
task. "Evidence" is what its end-to-end check must show.

### Plan 01 — Core pure layer *(written)*
- **Scope:** workspace, verification scripts, purity gate; CPU load, memory buckets, throughput, usage
  history, alert policy, formatting.
- **Evidence:** `./scripts/check.sh` green with 56 tests; the purity gate goes red on a planted
  `use std::fs;`, a dev-dependency and a `build.rs`; live memory "used" matches `free`.

### Plan 02a — Tray spikes *(written: `docs/plans/2026-11-17-rust-statsbar-02a-spikes.md`)*
- **Scope:** spike S1 (does a wide label pixmap render legibly in the GNOME panel, light and dark?)
  and spike S2 (threads and memory of `ksni` items plus a main-thread `zbus` connection). Results in
  `docs/spikes/`.
- **Evidence:** [S1 panel observations](../spikes/2026-11-17-s1-tray-label.md) and
  [S2 thread and memory measurements](../spikes/2026-11-17-s2-threads.md). The
  [02b probe](../spikes/2026-11-18-02b-system-font.md) compared a system font and the Stats Mini
  arrangement. The thread invariant was reworded from S2's measurements on 2026-11-18.

### Plan 02b — Daemon skeleton, first vertical slice *(written: `docs/plans/2026-11-18-rust-statsbar-02b-daemon.md`)*
- **Scope:** the config file, the scheduler, the worker thread (moved to Plan 05), the label renderer, the hub item with
  show/hide switches, launch at login, and CPU and Memory as tray items with summary menus. Written
  after Plan 02a, against its results.
- **Core additions (pure, tested):**
  `schedule::due(now_nanos, power_saver, &[ReaderState]) -> Vec<ReaderId>`;
  `label::render(metric: Metric, percent: u8) -> Option<[u8; 1600]>` (a 20×20 ARGB32 pixmap, `None`
  above 100; done in [the label-renderer plan](2026-11-18-rust-statsbar-02b-label-renderer.md)).
- **Daemon:** `config::parse(text: &str) -> Result<Config, ConfigError>`.
- **Readers:** `CpuReader::open(root)?.read() -> io::Result<Vec<CoreTicks>>`,
  `MemoryReader::open(root)?.read() -> io::Result<MemInfo>`; each keeps its file open between reads.
- **Tests:** fixture trees for both readers, including a missing file and a truncated one; scheduler
  table covering hidden, visible, page-open and power-saver; label tests: CPU at 100 % pinned
  to the GNOME probe's 20×20 frame, RAM sharing CPU's percentage pixels under its own label, every
  value 0–100 distinct, each single digit drawn, and values above 100 rejected.
- **Evidence:** [the end-to-end run](../spikes/2026-11-18-02b-daemon-e2e.md). The user confirmed
  live CPU and Memory labels in the panel, within one sample of `top` and `free`; no screenshot, as
  the portal failed. Unticking *Show CPU* removed the item, and `strace -y` showed `/proc/stat`
  closed with no read after it. `ps -T` showed 4 threads, as the invariant allows, at Pss 2657 kB.
  Power Saver stretched reads from 2 s to 5 s. One GNOME click was lost once and not reproduced;
  its likely cause (a menu layout change at startup) was fixed after the run and rechecked over
  D-Bus only.
  Written: [the daemon plan](2026-11-18-rust-statsbar-02b-daemon.md). The worker thread moves
  to Plan 05; config parsing lives in the daemon crate, since `toml` cannot enter `core`.

### Plan 03 — Battery, fans (read-only), network label *(written: `docs/plans/2026-11-19-rust-statsbar-03-battery-fan-network.md`)*
- **Scope:** `BAT0`, `AC`, `dell_smm` readers; the default-route interface and its counters; three
  more tray items with summary menus. Battery shows `BAT` over the charge percent, or `CHG` only
  while the kernel status is `Charging`. Fan shows `FAN` over its share of `fan1_min`–`fan1_max`,
  clamped at 100 %. Network draws upload over download in the 20×20 pixmap, each at most three
  characters of number and one unit letter.
- **Core additions:** `battery::BatteryFigures::new(&BatteryInfo, Option<bool>)` (health, watts,
  time remaining from charge and current, dropped above 24 h); `fan::range_fraction(&FanSpeed)`;
  `network::default_route_interface(&str)`; `label::rate_text(f64)` and `label::render_rates`.
- **Tests:** discharging, charging, full, zero current (no time estimate), a missing `cycle_count`;
  no default route.
- **Evidence:** [the end-to-end run](../spikes/2026-11-19-03-battery-fan-network-e2e.md). Polled
  at the daemon's ticks, the title rates averaged within +2.0 % of `rx_bytes` over 10 ticks of a
  mirror download; a first attempt over an unaligned 10 s window was off by 6.2 %, from tick
  misalignment while the rate swung. After unplugging, `Time left` appeared 1 s after
  `Discharging`. Hiding each item closed its files, with no read until it was shown again. Power Saver stretched battery reads to 5 s. 4 threads at
  Pss 2997 kB. `CHG` was seen only in tests, since the charge sat above the 75 % start threshold.
  `BAT0/uevent` reads averaged 13–15 ms with a 119 ms peak, on the main loop. The user found every
  pixmap label hard to read at 125 % scale; [spike S3](../spikes/2026-11-21-s3-panel-legibility.md)
  traced it to the shell, and the user chose a themed icon with the number as `XAyatanaLabel` text,
  which `ksni` 0.3.7 cannot publish.

### Plan 03b — Tray text labels *(written: `docs/plans/2026-11-21-rust-statsbar-03b-text-labels.md`)*
- **Scope:** every metric item shows a themed Yaru icon with its value as `XAyatanaLabel` text,
  drawn by GNOME in its own panel font; the 20×20 pixmap labels of Plans 02b and 03 are removed.
  `ksni` 0.3.7 is vendored at `vendor/ksni` and patched (see its `PATCHES.md`) to publish
  `XAyatanaLabel`, `XAyatanaLabelGuide` and the `XAyatanaNewLabel` signal. Percentage items show
  `37%` with the guide `100%`; Network shows only the download rate, such as `↓1.5M`, with the
  guide `↓999M`. Battery floors its icon to tens and adds `-charging` only while charging; Fan
  uses `weather-windy-symbolic`.
- **Core changes:** `label::render`, `label::render_rates`, `label::Metric` and `label::SIZE` are
  removed; `label::rate_text` stays.
- **Tests:** the icon and label mapping, including the battery floor and the charging variant; a
  D-Bus test on a private `dbus-daemon` that reads `XAyatanaLabel` and `XAyatanaLabelGuide`.
- **Evidence:** [the end-to-end run](../spikes/2026-11-21-03b-text-labels-e2e.md). Properties
  matched `BAT0`, `free -m` and the title rates; `XAyatanaNewLabel` fired 7 times in 10 s. With CPU
  and Memory shown the labels read crisply (`34%`, `43%`), but with all five items shown GNOME
  ellipsized every label at 125 % scale. The user accepted that as a gap ("Ghi gap, làm tiếp").
  The extension ignores `XAyatanaLabelGuide`. 4 threads at Pss 2789 kB. The charging icon was seen
  only in tests, and the run used the user's real `~/.config`.

### Plan 04a — Daemon IPC *(written: `docs/plans/2026-11-22-rust-statsbar-04a-ipc.md`)*
- **Scope:** socket server with a version handshake, subscribe, unsubscribe and refresh; snapshots of
  the CPU and Memory pages; process lists; CPU details (temperatures, frequency, load, uptime);
  memory pressure; a hidden item's page still read while it is watched; the cadence fix; one daemon
  at a time. Carries decisions 3–10, 12, 14–16 as far as they concern the daemon.
- **Core additions:** `ipc::{Hello, Request, Event}`; `process::{rank_by_cpu, rank_by_memory,
  can_quit}`; `system::Thermal::from_celsius`; `memory::Pressure::from_psi` with the thresholds in
  requirement 9.
- **Tests:** exact protocol lines, unknown fields ignored, version mismatch reported; ranking ties,
  vanished and reused pids; PSI and Thermal at, just below and above each threshold; the socket's
  bounds and uid check; the real binary on a private bus.
- **Evidence:** [the end-to-end run](../spikes/2026-11-22-04a-ipc-e2e.md). A socket client's
  figures matched `top`, `free`, coretemp and `/proc` within one sample, and a watched page came
  each second, also under power saver and with its item hidden, whose files closed with the last
  watcher; 4 threads at Pss 3285 kB idle, with Pressure seen only as Normal and the 5 s closed
  cadence read off `syscr`, since ptrace was refused.

### Plan 04b — Panel skeleton (CPU and Memory pages) *(written: `docs/plans/2026-11-24-rust-statsbar-04b-panel.md`)*
- **Needs first (done):** `libgtk-4-dev libadwaita-1-dev` installed on the laptop on 2026-11-24 and
  in CI; Rust 1.92 as the panel's floor.
- **Scope:** decisions 2, 11, 13 and 17, and the panel half of 3, 5, 8, 15 and 16: the GTK window
  (reused, not duplicated); rings, per-core bars, memory bar and process lists; quit with a
  confirmation and SIGTERM, guarded again by pid and start time; "Open details…" at the top of the
  CPU and Memory menus, with menu rows reserved from the start; Retry when the daemon is down; a
  Refresh footer; "—" for missing PSI. Measure the Cairo renderer against GL and keep the lighter.
- **Evidence:** panel figures match `top` and `free` within one sample; closing the panel leaves no
  `statsbar-panel` process and the daemon back at its closed-page cadence.
  [The end-to-end run](../spikes/2026-11-24-04b-panel-e2e.md): figures matched `free`, `top` and
  coretemp within one sample, one window took a second launch's page, quit and its failure toast
  were seen on stand-ins, Retry and "Open details…" reconnected after a daemon restart, and closing
  left no panel and the daemon's reads back from about 2370 to 6–31 a second. Cairo kept at Pss
  about 23 MB against 50–127 MB for GL; the daemon at Pss 2919 kB on a fresh start.

### Plan 05 — Battery and Network pages
- **Scope:** battery detail page; NetworkManager reader; Wi-Fi details; DNS; ping; public IP; the
  worker thread (moved from 02b), with the public IP lookup as its first caller.
- **Note:** the NetworkManager reader replaces `/proc/net/route` for the tray's interface, which
  closes Plan 03's gap: an IPv6-only default route reads as `No active interface` until then.
- **Tests:** NetworkManager property maps → `NetworkInfo` (wired, Wi-Fi, disconnected, no IPv6);
  frequency → band and channel; `ping::stats(&[f64]) -> (mean, jitter)`; public-IP replies that are
  empty, oversized or not an address.
- **Evidence:** values match `nmcli device show`; unplugging the network clears the public IP; a
  public-IP lookup held for its full timeout does not delay the tray's CPU label.

### Plan 06 — GPU
- **Scope:** fdinfo reader with the pid cache; three rings and a 60-sample history chart.
- **Core additions:** `gpu::parse_fdinfo`, `gpu::EngineUsage::between`.
- **Tests:** two fds sharing one `drm-client-id` counted once; a client that exits between samples; an
  engine counter that goes backwards.
- **Evidence:** the render ring rises while `glmark2` or a video plays and returns to idle after.

### Plan 07 — Bluetooth and displays
- **Scope:** BlueZ object manager with signals; Mutter display state; EDID diagonal.
- **Tests:** BlueZ property maps (connected with battery, connected without, adapter off); EDID blobs
  → name and diagonal.
- **Evidence:** connecting a headset adds its row without a restart.

### Plan 08 — Temperature alerts
- **Scope:** wire `HotAlertPolicy` (Plan 01) to the CPU package sensor (90 °C, margin 5) and to phones
  (39 °C, margin 2); notifications; config keys for both thresholds and an on/off switch.
- **Evidence:** with the threshold set below the current temperature, exactly one notification arrives.

### Plan 09 — Phones
- **Scope:** `ideviceinfo` and `adb` readers on the worker thread with a timeout; install hints;
  USB-presence check so neither tool is spawned with nothing plugged in.
- **Tests:** tool output → battery figures; tool missing; tool hangs; malformed output.
- **Evidence:** a plugged-in, trusted phone appears on the Battery page. Needs the user's phone.

### Plan 10 — Fan control
- **Security-sensitive:** `code-review-strict` plus a security review.
- **Scope:** the helper, its systemd unit and polkit install, the Auto · Max · Manual control, with
  Manual limited to Low and High.
- **Core additions:** `fan::Command` and `fan::Reply` (`version`, `ping`, `level`, `max`, `auto`),
  after upstream's line protocol; `fan::Level` with no Off variant, so Off cannot be requested.
- **Tests:** every malformed line rejected, including a request for level 0; peer-uid check; watchdog
  and disconnect both restore Auto, against a fixture sysfs tree.
- **Evidence:** Max raises `fan1_input`; killing the panel returns `pwm1_enable` to automatic within
  10 s. Run with the user present.

### Plan 11 — Packaging and budget verification
- **Scope:** install and uninstall, `.desktop` entries, README, `scripts/measure-idle.sh`.
- **Evidence:** the budget table above, filled in with measured numbers.
