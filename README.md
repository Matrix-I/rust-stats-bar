# rust-statsbar

A Linux counterpart of [StatsBar](https://github.com/Matrix-I/stats-bar): hardware figures in the
panel, details one click away, built to cost as little CPU and memory as possible.

Status: the pure core only. Nothing runs yet. See `docs/plans/` for the roadmap.

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

## License

[MIT](LICENSE)
