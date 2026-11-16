# rust-statsbar

A Linux counterpart of [StatsBar](https://github.com/Matrix-I/stats-bar): hardware figures in the
panel, details one click away, built to cost as little CPU and memory as possible.

**Status:** planning only. No code exists yet; nothing builds or runs.

## Planned design

| Process | Role |
|---|---|
| `statsbard` | Session daemon without a GUI toolkit: polls the machine, draws the tray items, serves snapshots. |
| `statsbar-panel` | GTK4 + libadwaita window, spawned on demand by the daemon, exits when closed. |
| `statsbar-fan-helper` | Small root helper, used only for fan control. |

Pure arithmetic (CPU load, memory buckets, throughput, history, alerts, formatting) lives in
`crates/core`, which does no I/O and is tested without hardware.

Target: this machine only — Dell laptop, Intel UHD 620, `dell_smm` fan, Ubuntu 26.04, GNOME on Wayland.

## Plans

- [Roadmap](docs/plans/2026-11-16-rust-statsbar-roadmap.md): requirements, architecture, risks and
  the list of plans 01–11.
- [Plan 01 — core](docs/plans/2026-11-16-rust-statsbar-01-core.md): the pure `crates/core` layer,
  step by step with tests.

## License

[MIT](LICENSE)
