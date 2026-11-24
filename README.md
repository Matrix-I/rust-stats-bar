# rust-statsbar

A Linux counterpart of [StatsBar](https://github.com/Matrix-I/stats-bar): hardware figures in the
panel, details one click away, built to cost as little CPU and memory as possible.

Status: CPU, Memory, Battery, Fan and Network in the tray, and the CPU and Memory pages' data on a
private socket for the coming panel (statsbard); run with `cargo run --release -p statsbard`.

## Development

```bash
./scripts/check.sh    # formatting, lints, tests and the purity gate
```

`crates/core` is the pure layer: no file, socket, clock, thread or `unsafe`, and no dependency other
than `serde`. `scripts/check-core-purity.sh` fails the build on a violation. Arithmetic goes there, with
an integration test in `crates/core/tests/` written first; anything that touches the machine goes in a
reader, which hands plain values to the core.

## Plans

- [Roadmap](docs/plans/2026-11-16-rust-statsbar-roadmap.md): requirements, architecture, risks and
  the list of plans 01–11.
- [Plan 01 — core](docs/plans/2026-11-16-rust-statsbar-01-core.md): the pure `crates/core` layer,
  step by step with tests.
- [Plan 02a — tray spikes](docs/plans/2026-11-17-rust-statsbar-02a-spikes.md): what the GNOME panel
  does with a label pixmap, and what the tray costs in threads and memory. Results in
  [docs/spikes/](docs/spikes/).
- [Plan 02b — font probe](docs/plans/2026-11-18-rust-statsbar-02b-font-probe.md): a system font
  against the Stats Mini layout in the panel.
- [Plan 02b — label renderer](docs/plans/2026-11-18-rust-statsbar-02b-label-renderer.md): the pure
  20×20 CPU and RAM label renderer in `crates/core`.
- [Plan 02b — daemon](docs/plans/2026-11-18-rust-statsbar-02b-daemon.md): config, scheduler,
  `/proc` readers, the hub with show/hide switches, launch at login, and the CPU and Memory items.
- [Plan 02b — end-to-end run](docs/spikes/2026-11-18-02b-daemon-e2e.md): the daemon checked in the
  GNOME panel.
- [Plan 03 — battery, fan and network](docs/plans/2026-11-19-rust-statsbar-03-battery-fan-network.md):
  the `BAT0`, `AC`, `dell_smm` and default-route readers, and their three tray items.
- [Plan 03 — end-to-end run](docs/spikes/2026-11-19-03-battery-fan-network-e2e.md): the new items
  checked in the GNOME panel; [spike S3](docs/spikes/2026-11-21-s3-panel-legibility.md) on why the
  pixmap labels are hard to read at 125 % scale.
- [Plan 03b — text labels](docs/plans/2026-11-21-rust-statsbar-03b-text-labels.md): a themed
  icon with the value as panel text, through a vendored `ksni` with `XAyatanaLabel`;
  [end-to-end run](docs/spikes/2026-11-21-03b-text-labels-e2e.md).
- [Plan 04a — daemon IPC](docs/plans/2026-11-22-rust-statsbar-04a-ipc.md): the panel's protocol
  and socket, with process lists and CPU details;
  [end-to-end run](docs/spikes/2026-11-22-04a-ipc-e2e.md).

## License

[MIT](LICENSE)
