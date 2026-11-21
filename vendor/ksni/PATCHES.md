# Local changes to ksni 0.3.7

Copied from crates.io `ksni` 0.3.7 (upstream commit 2397cc2ce535ba4e7ed77fecd94731c5239923e7,
Unlicense) and loaded by the workspace's `[patch.crates-io]`. Why: spike S3
(`docs/spikes/2026-11-21-s3-panel-legibility.md`) needs the Ayatana `XAyatanaLabel` property,
and ksni's property list is fixed.

Removed from the registry copy: `.cargo-ok`, `.github/`, `.config/`, `.gitignore`,
`.gitmodules`, `Cargo.lock`. Kept unchanged: the published `Cargo.toml`, `Cargo.toml.orig`,
`.cargo_vcs_info.json`, `UNLICENSE`, `README.md`, `CHANGELOG.md`, `examples/`, `tests/`.

Changed:

- `src/lib.rs`: `Tray::x_ayatana_label` and `Tray::x_ayatana_label_guide`, both empty by
  default.
- `src/service.rs`: both methods join `def_properties_monitor!`, and `update_properties` emits
  `XAyatanaNewLabel(label, guide)` when either changes.
- `src/dbus_interface.rs`: the `XAyatanaLabel` and `XAyatanaLabelGuide` properties and the
  `XAyatanaNewLabel` signal.

Tested by `crates/daemon/tests/ayatana_label.rs`. On a ksni upgrade, reapply this or drop the
copy if upstream publishes the property.
