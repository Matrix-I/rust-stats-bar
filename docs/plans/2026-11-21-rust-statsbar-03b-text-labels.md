# Plan 03b — Tray text labels

> **For agentic workers:** REQUIRED SUB-SKILL: subagent-driven-strict or executing-plans-strict.
> Tasks are `### Task N: <name>`; steps use `- [ ]`.

**Goal:** every metric item shows a themed Yaru icon with its value as text, for example
`computer-chip-symbolic` + `37%`. GNOME draws the text in its own panel font, as
[spike S3](../spikes/2026-11-21-s3-panel-legibility.md) chose. The 20×20 pixmap labels of Plans 02b
and 03 are removed.

**Architecture:** the text travels as the Ayatana `XAyatanaLabel` property of
`org.kde.StatusNotifierItem`. Ubuntu's AppIndicator extension reads it and shows an `St.Label`
next to the icon. `ksni` 0.3.7 has a fixed property list, so the workspace vendors it at
`vendor/ksni` and patches it with two `Tray` methods, two properties and the `XAyatanaNewLabel`
signal. `[patch.crates-io]` points the daemon's existing `ksni = "=0.3.7"` at the patched copy.
`statsbard` changes as follows:

- `app` returns what each label needs without `label::Metric`.
- `tray` maps it to an icon name and a label text.
- `run` passes both to the item.

`statsbar-core` keeps only `label::rate_text`.

**Tech stack:** Rust edition 2024, workspace floor 1.85, `statsbard` 1.87, `rustc 1.100.0`.
`ksni` 0.3.7 stays at the same version; only its source changes from the registry to
`vendor/ksni` (Unlicense). No new crate.

**Branch:** `feature/tray-text-labels`, cut from `main` (d993c66). Plan 03 is already on `main`
(55792e2, a30a747, d993c66). The local `feature/battery-fan-network` is stale and is not the base.

## Confirmed Requirements

Items 1–5: the user accepted every recommendation on 2026-11-21 ("Nhận cả 5").

1. **Publishing the label:** vendor and patch `ksni` 0.3.7 and load it through
   `[patch.crates-io]`. The patch adds `x_ayatana_label()` and `x_ayatana_label_guide()` to
   `Tray`, and emits `XAyatanaNewLabel` when either changes. The daemon's own StatusNotifierItem on
   `zbus` was rejected. The thread invariant is unchanged.
2. **The pixmap labels are removed:** `label::render`, `label::render_rates`, `label::Metric`,
   `label::SIZE` and their tests. The target is GNOME only (roadmap requirement 5). `rate_text`
   stays, because the network label and title use it.
3. **Icons:**

   | Item | Icon |
   |---|---|
   | CPU | `computer-chip-symbolic` |
   | Memory | `drive-harddisk-solidstate-symbolic` |
   | Network | `network-transmit-receive-symbolic` |
   | Battery | `battery-level-N0-symbolic`, or `battery-level-N0-charging-symbolic` while charging |
   | Fan | a Yaru icon present on this machine *(delegated)*: `weather-windy-symbolic`, because Yaru has no fan icon (searched 2026-11-21) |

4. **Label text:**
   - CPU, Memory, Battery and Fan show a bare percentage: `37%`. Fan keeps its share of the
     `fan1_min`–`fan1_max` range, as in Plan 03.
   - Network shows only the download rate: `↓1.5M`. The upload rate stays in the title and the
     menu.
5. **`XAyatanaLabelGuide` is published:** `100%` for percentage items, `↓999M` for Network.
6. **Before the first value** (answered by the user on 2026-11-21, "Icon không chữ"): an item
   shows its icon with an empty label. Battery shows `battery-missing-symbolic` until it knows
   its charge. A failed read changes nothing, as in Plan 03: the item keeps its last look, so a
   battery that is never read keeps `battery-missing-symbolic`. Fan without a range keeps its
   icon with no text.

## Global Constraints

From the roadmap, which repeats the user's global rules (this repo has no `CLAUDE.md`):

- Test-first: happy path, edge cases, error paths. Untested code is reported incomplete, never done.
- Each test can fail without its change; assert observable behaviour. Mock only true boundaries. No
  sleeps, wall clock, ordering or network in tests.
- Never weaken, skip or delete a test to make it pass. Tests of code this plan removes are deleted
  with that code. Each task says which tests are deleted and why.
- Commit or push only when asked: tasks end with `Ready to commit: …`, not `git commit`.
  Conventional Commits, imperative mood, one logical change per commit.
- Code, comments, names, commit messages and docs in English. Comments explain why.
- Every external call has a timeout. This includes the D-Bus calls and child processes in the new
  test.
- No host clock in logic or tests.
- Ask before adding or upgrading dependencies. Requirement 1 approves the vendored `ksni`. Nothing
  else is added.
- **Project invariant:** `crates/core` does no I/O, reads no clock, spawns nothing, has no `unsafe`
  and no build script, and depends on nothing but `serde`. Enforced by
  `scripts/check-core-purity.sh`.
- **Project invariant:** `statsbard` never links GTK and never starts a tokio runtime. Its threads
  are the main loop, `async-io`'s reactor, `ksni`'s one driver thread and the allowed `zbus`
  `blocking` pool threads. The patch adds no thread.
- Verification entry point: `./scripts/check.sh`. Final gate: read the full diff, run
  `./scripts/check.sh`, self-review, run `code-review-strict` (this change touches a dependency's
  code), and repeat if anything changed.
- The new D-Bus test needs `/usr/bin/dbus-daemon` (present on 2026-11-21). If it is missing, the
  test fails and says so; it never skips.

## Technical Defaults

- **`vendor/ksni`, listed in `[workspace] exclude`, loaded through `[patch.crates-io]`.** This is
  Cargo's standard override, and the daemon's manifest stays as it is. Excluding the crate keeps
  workspace lints and `clippy --workspace` off upstream code.
- **`scripts/check.sh` runs `cargo fmt --check` instead of `cargo fmt --all --check`.** `--all`
  also checks local path dependencies, and upstream `ksni` is not rustfmt-clean
  (`cargo fmt --check` exits 1 on the registry copy). Without `--all`, cargo checks every
  workspace member and nothing else. Both behaviours were checked in a scratch workspace on
  2026-11-21: an excluded path dependency failed with `--all` and passed without it, and a
  misformatted member still failed.
- **Vendored files:** the registry copy without the files that do not apply to a vendored crate:
  `.cargo-ok`, `.github/`, `.config/`, `.gitignore`, `.gitmodules` and `Cargo.lock`. `examples/`
  and `tests/` stay, because the published `Cargo.toml` declares targets in them.
- **Label text has no space (`37%`), as in the S3 probe.** Titles and menu rows keep `37 %`.
- **Battery icon rule:** the charge is floored to a multiple of 10, as GNOME Shell 50.1's own
  indicator does (`10 * Math.floor(Percentage / 10)` in `ui/status/system.js`). The `-plugged-in`
  and `-charged` variants are not used, since requirement 3 names only `-charging`.
- **D-Bus test:** a private `dbus-daemon`, plus the test binary re-run as the item's process with
  `DBUS_SESSION_BUS_ADDRESS` set. The test cannot point `ksni` at a bus in-process:
  `std::env::set_var` is `unsafe` in edition 2024, and the workspace forbids `unsafe`. The desktop
  panel never sees the test item.

## Risks & Rollback

| Risk | Mitigation | Revert |
|---|---|---|
| The vendored `ksni` drifts from upstream, or a later upgrade loses the patch | `vendor/ksni/PATCHES.md` lists every changed line. A `ksni` upgrade must reapply it or drop it if upstream adds the property | Remove `[patch.crates-io]`, `exclude` and `vendor/ksni`; revert Tasks 2–4 to get the pixmaps back |
| Ubuntu's extension ignores `XAyatanaLabelGuide` (its `appIndicator.js` only lists the property), so the label's width changes with its text and moves the items to its right | Accepted: requirement 5 publishes the guide for hosts that read it. Task 5's end-to-end check records what happens between `9%` and `100%` | — |
| A host without Ayatana labels (KDE) shows the icon only | Out of scope: GNOME only (roadmap requirement 5). The value is still in the title and menu | — |
| `scripts/check.sh` formats fewer files | Members are still checked, as shown in Technical Defaults. Only the vendored copy drops out | Restore `--all` once `vendor/ksni` is gone |
| The S3 probe `spikes/tray/src/bin/sharp.rs` uses the removed renderer | Task 4 deletes it with its `statsbar-core` dependency. The S3 record says how to rebuild it from a30a747 | `git checkout a30a747 -- spikes/tray` |
| No persisted data, config key, IPC or item id changes | Item ids stay `statsbar-cpu` … `statsbar-network`, so GNOME keeps their order | — |
| `CHG` was never seen live in Plan 03 (charge above the 75 % start threshold); the charging icon may stay unseen too | Unit test in Task 3; Task 5 reports it as a gap if the laptop is not charging | — |

---

### Task 1: Patch `ksni` to publish `XAyatanaLabel`

**Test level:** integration, against a real private `dbus-daemon`.

**Files:**
- Create: `vendor/ksni/` (copy of the registry crate), `vendor/ksni/PATCHES.md`
- Modify: `vendor/ksni/src/lib.rs` (after `fn tool_tip`, lines 205–210),
  `vendor/ksni/src/service.rs` (`update_properties` after the `tool_tip_changed` block,
  lines 303–305; `def_properties_monitor!` list, lines 635–650),
  `vendor/ksni/src/dbus_interface.rs` (after the `tool_tip` property, lines 360–366; after the
  `new_status` signal, line 385)
- Modify: `Cargo.toml` (root; `[workspace]` and a new `[patch.crates-io]`), `Cargo.lock`,
  `scripts/check.sh` (the `cargo fmt` line), `.github/workflows/ci.yml` (a step before
  `./scripts/check.sh`)
- Test: `crates/daemon/tests/ayatana_label.rs`

**Interfaces:**
- Produces, on `ksni::Tray`: `fn x_ayatana_label(&self) -> String` and
  `fn x_ayatana_label_guide(&self) -> String`, both empty by default.
- On D-Bus: properties `XAyatanaLabel` (`s`) and `XAyatanaLabelGuide` (`s`), and the signal
  `XAyatanaNewLabel(s label, s guide)`, emitted from `Handle::update` when either value changes.

- [x] **Step 1: Branch.**

  ```bash
  git switch main
  git switch -c feature/tray-text-labels
  ```

- [x] **Step 2: Write the failing test** `crates/daemon/tests/ayatana_label.rs`. The code is rustfmt-clean. In a scratch copy on 2026-11-21 it passed, and it failed, on a missing or unexpected signal, when the patch's two checks were joined by a short-circuiting `||`:

  ```rust
  //! The vendored `ksni` patch: an item publishes `XAyatanaLabel` and `XAyatanaLabelGuide`, and
  //! signals `XAyatanaNewLabel` once per update that changes either, never for one that changes
  //! neither. It runs on a private `dbus-daemon`, so the desktop's panel never sees the test item.

  use std::future::Future;
  use std::io::{BufRead, BufReader, Read, Write};
  use std::process::{Child, ChildStdin, Command, Stdio};
  use std::sync::mpsc;
  use std::time::Duration;

  use futures_lite::{StreamExt, future};
  use ksni::TrayMethods;
  use zbus::names::InterfaceName;
  use zbus::{MatchRule, MessageStream};

  const SNI_PATH: &str = "/StatusNotifierItem";
  const SNI_INTERFACE: &str = "org.kde.StatusNotifierItem";
  /// `ksni` names each item `org.kde.StatusNotifierItem-<pid>-<n>`.
  const ITEM_PREFIX: &str = "org.kde.StatusNotifierItem-";
  /// Generous for a local bus; it only has to tell "slow" from "hung".
  const TIMEOUT: Duration = Duration::from_secs(5);
  /// Lines a child may print before the one awaited (libtest's own); bounds every wait.
  const MAX_SKIPPED_LINES: usize = 16;
  /// Set only in the re-run that plays the item; see `label_item_child`.
  const CHILD_ENV: &str = "STATSBAR_TEST_LABEL_CHILD";
  /// Printed by the item once it is on the bus. libtest may print `test … ` on the same line
  /// when tests run on one thread, so the parent looks for it with `contains`.
  const READY: &str = "statsbar-label-item-ready";
  /// Printed by the item after each command is applied.
  const APPLIED: &str = "statsbar-label-item-applied";

  struct LabelTray {
      label: String,
      guide: String,
  }

  impl ksni::Tray for LabelTray {
      fn id(&self) -> String {
          "statsbar-test-label".to_owned()
      }

      fn x_ayatana_label(&self) -> String {
          self.label.clone()
      }

      fn x_ayatana_label_guide(&self) -> String {
          self.guide.clone()
      }
  }

  /// Kills the process on drop, so a failed assertion never leaks a bus or an item.
  struct Reaped(Child);

  impl Drop for Reaped {
      fn drop(&mut self) {
          // Either may fail only because the process has already exited, which is the goal.
          let _ = self.0.kill();
          let _ = self.0.wait();
      }
  }

  /// A child's stdout lines, read on a thread so every wait below has a timeout.
  fn lines(out: impl Read + Send + 'static) -> mpsc::Receiver<String> {
      let (sender, receiver) = mpsc::channel();
      std::thread::spawn(move || {
          for line in BufReader::new(out).lines().map_while(Result::ok) {
              if sender.send(line).is_err() {
                  break;
              }
          }
      });
      receiver
  }

  /// The first line that `wanted` accepts, skipping at most `MAX_SKIPPED_LINES` others.
  fn wait_for(lines: &mpsc::Receiver<String>, wanted: impl Fn(&str) -> bool, what: &str) -> String {
      for _ in 0..=MAX_SKIPPED_LINES {
          match lines.recv_timeout(TIMEOUT) {
              Ok(line) if wanted(&line) => return line,
              Ok(_) => {}
              Err(error) => panic!("{what}: {error}"),
          }
      }
      panic!("{what}: more than {MAX_SKIPPED_LINES} other lines")
  }

  async fn within<T>(what: &str, call: impl Future<Output = T>) -> T {
      future::or(call, async {
          async_io::Timer::after(TIMEOUT).await;
          panic!("{what} timed out after {TIMEOUT:?}")
      })
      .await
  }

  async fn text_property(properties: &zbus::fdo::PropertiesProxy<'_>, name: &str) -> String {
      let interface = InterfaceName::from_static_str(SNI_INTERFACE).expect("valid interface");
      let value = within(name, properties.get(interface, name))
          .await
          .unwrap_or_else(|error| panic!("{name}: {error}"));
      String::try_from(value).unwrap_or_else(|error| panic!("{name}: {error}"))
  }

  /// The item's half of `the_item_publishes_and_signals_its_label`. It does nothing unless that
  /// test starts it with `CHILD_ENV` and a private bus address. Each stdin line is one update:
  /// `label <text>`, `guide <text>`, `both <label> <guide>`, or `same` (an update that changes
  /// nothing). It exits at the end of stdin.
  #[test]
  #[ignore = "the item half of the_item_publishes_and_signals_its_label"]
  fn label_item_child() {
      if std::env::var_os(CHILD_ENV).is_none() {
          return;
      }
      async_io::block_on(async {
          let item = LabelTray {
              label: "37%".to_owned(),
              guide: "100%".to_owned(),
          };
          let handle = item
              .assume_sni_available(true)
              .spawn()
              .await
              .expect("spawn on the private bus");
          println!("{READY}");
          for line in std::io::stdin().lock().lines() {
              let line = line.expect("read a command");
              let words: Vec<String> = line.split(' ').map(str::to_owned).collect();
              handle
                  .update(move |tray: &mut LabelTray| match words.as_slice() {
                      [command, label] if command == "label" => tray.label = label.clone(),
                      [command, guide] if command == "guide" => tray.guide = guide.clone(),
                      [command, label, guide] if command == "both" => {
                          tray.label = label.clone();
                          tray.guide = guide.clone();
                      }
                      [command] if command == "same" => {}
                      other => panic!("unknown command {other:?}"),
                  })
                  .await
                  .expect("the item is still running");
              println!("{APPLIED}");
          }
          handle.shutdown().await;
      });
  }

  /// Sends one command and waits until the item has applied it, so its signal (if any) is on
  /// the bus before the next command.
  fn send(item: &mut ChildStdin, applied: &mpsc::Receiver<String>, command: &str) {
      writeln!(item, "{command}").unwrap_or_else(|error| panic!("{command}: {error}"));
      wait_for(applied, |line| line.contains(APPLIED), command);
  }

  #[test]
  fn the_item_publishes_and_signals_its_label() {
      let mut bus = Reaped(
          Command::new("dbus-daemon")
              .args(["--session", "--nofork", "--print-address=1"])
              .stdout(Stdio::piped())
              .spawn()
              .expect("run /usr/bin/dbus-daemon (Ubuntu package dbus-daemon)"),
      );
      let bus_lines = lines(bus.0.stdout.take().expect("piped stdout"));
      let address = wait_for(&bus_lines, |line| line.starts_with("unix:"), "bus address");

      let mut item = Reaped(
          Command::new(std::env::current_exe().expect("test binary path"))
              .args(["--exact", "label_item_child", "--ignored", "--nocapture"])
              .env(CHILD_ENV, "1")
              .env("DBUS_SESSION_BUS_ADDRESS", &address)
              .stdin(Stdio::piped())
              .stdout(Stdio::piped())
              .spawn()
              .expect("start the item"),
      );
      let item_lines = lines(item.0.stdout.take().expect("piped stdout"));
      let mut commands = item.0.stdin.take().expect("piped stdin");
      wait_for(&item_lines, |line| line.contains(READY), "item ready");

      async_io::block_on(async {
          let connection = within("connect", async {
              zbus::connection::Builder::address(address.as_str())
                  .expect("valid address")
                  .build()
                  .await
          })
          .await
          .expect("connect to the private bus");
          let dbus = within("DBus proxy", zbus::fdo::DBusProxy::new(&connection))
              .await
              .expect("DBus proxy");
          let names = within("ListNames", dbus.list_names())
              .await
              .expect("list names");
          let item_name = names
              .iter()
              .map(|name| name.to_string())
              .find(|name| name.starts_with(ITEM_PREFIX))
              .expect("the item owns its name");
          let properties = within("properties proxy", async {
              zbus::fdo::PropertiesProxy::builder(&connection)
                  .destination(item_name.as_str())
                  .expect("valid name")
                  .path(SNI_PATH)
                  .expect("valid path")
                  .build()
                  .await
          })
          .await
          .expect("properties proxy");
          let rule = MatchRule::builder()
              .msg_type(zbus::message::Type::Signal)
              .interface(SNI_INTERFACE)
              .expect("valid interface")
              .member("XAyatanaNewLabel")
              .expect("valid member")
              .build();
          // Subscribed before the first command, so no signal can be missed.
          let mut signals = within(
              "AddMatch",
              MessageStream::for_match_rule(rule, &connection, None),
          )
          .await
          .expect("subscribe");
          let mut next_signal = async || -> (String, String) {
              let message = within("XAyatanaNewLabel", signals.next())
                  .await
                  .expect("the stream is open")
                  .expect("a signal");
              message.body().deserialize().expect("an (ss) body")
          };

          assert_eq!(text_property(&properties, "XAyatanaLabel").await, "37%");
          assert_eq!(
              text_property(&properties, "XAyatanaLabelGuide").await,
              "100%"
          );

          send(&mut commands, &item_lines, "label 38%");
          assert_eq!(next_signal().await, ("38%".to_owned(), "100%".to_owned()));
          assert_eq!(text_property(&properties, "XAyatanaLabel").await, "38%");

          send(&mut commands, &item_lines, "guide ↓999M");
          assert_eq!(next_signal().await, ("38%".to_owned(), "↓999M".to_owned()));
          assert_eq!(
              text_property(&properties, "XAyatanaLabelGuide").await,
              "↓999M"
          );

          // Both at once, then nothing: if the label check short-circuited the guide's, the
          // guide's stale hash would send a second signal on the update that changes nothing.
          send(&mut commands, &item_lines, "both 39% 100%");
          assert_eq!(next_signal().await, ("39%".to_owned(), "100%".to_owned()));
          send(&mut commands, &item_lines, "same");
          send(&mut commands, &item_lines, "label 40%");
          assert_eq!(
              next_signal().await,
              ("40%".to_owned(), "100%".to_owned()),
              "the update that changed nothing sent no signal"
          );
      });
      // Closing stdin ends the item's loop; `Reaped` then reaps both processes.
      drop(commands);
  }
  ```

- [x] **Step 3: Run it and expect FAIL.**

  Run: `cargo test -p statsbard --test ayatana_label`
  Expected: compile error E0407, ``method `x_ayatana_label` is not a member of trait
  `ksni::Tray` ``. The registry `ksni` has no such method.

- [x] **Step 4: Vendor `ksni` 0.3.7.**

  ```bash
  mkdir -p vendor
  cp -r ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/ksni-0.3.7 vendor/ksni
  rm -r vendor/ksni/.cargo-ok vendor/ksni/.github vendor/ksni/.config \
        vendor/ksni/.gitignore vendor/ksni/.gitmodules vendor/ksni/Cargo.lock
  ```

  In the root `Cargo.toml`, below `members = [...]` in `[workspace]`:

  ```toml
  # The patched ksni (vendor/ksni/PATCHES.md) is a path dependency, not a member, so upstream's
  # code keeps upstream's formatting and is not held to this workspace's lints.
  exclude = ["vendor/ksni"]
  ```

  And after the `[workspace.lints.clippy]` table:

  ```toml
  # ksni 0.3.7 cannot publish XAyatanaLabel; this copy adds it (spike S3, Plan 03b).
  [patch.crates-io]
  ksni = { path = "vendor/ksni" }
  ```

  In `scripts/check.sh`, replace `cargo fmt --all --check` with:

  ```bash
  # Without --all, cargo fmt checks the workspace members only. --all would also check local
  # path dependencies, such as the vendored ksni, which keeps upstream's formatting.
  cargo fmt --check
  ```

  In `.github/workflows/ci.yml`, before `- run: ./scripts/check.sh`:

  ```yaml
      # tests/ayatana_label.rs runs a private session bus; it fails, never skips, without one.
      - run: command -v dbus-daemon || (sudo apt-get update && sudo apt-get install -y --no-install-recommends dbus-daemon)
  ```

- [x] **Step 5: Patch `vendor/ksni/src/lib.rs`.** Insert after `fn tool_tip` (after line 210):

  ```rust
      /// Text shown next to the icon: the Ayatana `XAyatanaLabel` property, which Ubuntu's
      /// AppIndicator extension draws in the panel font. Empty means no label.
      ///
      /// Added by statsbar's vendored patch; see `PATCHES.md`.
      fn x_ayatana_label(&self) -> String {
          Default::default()
      }

      /// The widest text the label is expected to take (`XAyatanaLabelGuide`), so a host can
      /// reserve its width.
      ///
      /// Added by statsbar's vendored patch; see `PATCHES.md`.
      fn x_ayatana_label_guide(&self) -> String {
          Default::default()
      }
  ```

- [x] **Step 6: Patch `vendor/ksni/src/service.rs`.** In `def_properties_monitor!`, change
  `text_direction: crate::TextDirection` to:

  ```rust
      text_direction: crate::TextDirection,
      x_ayatana_label: String,
      x_ayatana_label_guide: String
  ```

  In `update_properties`, after the `if self.tool_tip_changed() { … }` block:

  ```rust
          // Both hashes must be refreshed on every update, so neither check may short-circuit
          // the other.
          let label_changed = self.x_ayatana_label_changed();
          let guide_changed = self.x_ayatana_label_guide_changed();
          if label_changed || guide_changed {
              StatusNotifierItem::<T>::x_ayatana_new_label(
                  sni_obj.signal_emitter(),
                  &self.tray.x_ayatana_label(),
                  &self.tray.x_ayatana_label_guide(),
              )
              .await?;
          }
  ```

- [x] **Step 7: Patch `vendor/ksni/src/dbus_interface.rs`.** After the `tool_tip` property:

  ```rust
      #[zbus(property, name = "XAyatanaLabel")]
      async fn x_ayatana_label(&self) -> zbus::fdo::Result<String> {
          Ok(self
              .0
              .lock_state(false, async |state| state.get_x_ayatana_label())
              .await)
      }

      #[zbus(property, name = "XAyatanaLabelGuide")]
      async fn x_ayatana_label_guide(&self) -> zbus::fdo::Result<String> {
          Ok(self
              .0
              .lock_state(false, async |state| state.get_x_ayatana_label_guide())
              .await)
      }
  ```

  After the `new_status` signal:

  ```rust
      #[zbus(signal, name = "XAyatanaNewLabel")]
      pub async fn x_ayatana_new_label(
          ctxt: &SignalEmitter<'_>,
          label: &str,
          guide: &str,
      ) -> zbus::Result<()>;
  ```

- [x] **Step 8: Write `vendor/ksni/PATCHES.md`:**

  ```markdown
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
  ```

- [x] **Step 9: Run it and expect PASS.** — deviated: `cargo audit` exits 0 but lists 3 allowed "unmaintained" warnings (RUSTSEC-2026-0370/0371/0372, zvariant, zbus_names, zvariant_derive), identical on main; see checkpoint 1

  Run: `cargo test -p statsbard --test ayatana_label`
  Expected: `the_item_publishes_and_signals_its_label ... ok` and `label_item_child ... ignored`.

  Run: `RUST_TEST_THREADS=1 cargo test -p statsbard --test ayatana_label`
  Expected: the same. With one test thread, libtest prints `test label_item_child ... ` on the
  same line as the marker, and the `contains` match still finds it.

  Run: `grep -A2 'name = "ksni"' Cargo.lock`
  Expected: `version = "0.3.7"` with no `source =` line, so the build uses the vendored copy.

  Run: `./scripts/check.sh`
  Expected: exit 0.

  Run: `cargo audit`
  Expected: no advisory. The crate set is unchanged; only `ksni`'s source moved.

- [x] **Step 10: Commit checkpoint.**

  Ready to commit: `Cargo.toml Cargo.lock scripts/check.sh .github/workflows/ci.yml vendor/ksni crates/daemon/tests/ayatana_label.rs docs/plans/2026-11-21-rust-statsbar-03b-text-labels.md`
  — `"build: vendor ksni 0.3.7 with the XAyatanaLabel property"`

### Task 2: Give the app's label results without pixmap words

**Test level:** unit (`App` has no I/O).

**Files:**
- Modify: `crates/daemon/src/app.rs`. Remove the `Metric` import (line 7) and `RateLabel`
  (lines 32–33). Add `BatteryLabel`. Change the `battery_label` and `network_label` fields
  (lines 56, 61–62), `record_battery` (lines 194–212) and `record_network` (lines 222–241).
- Modify: `crates/daemon/src/run.rs`, the Battery and Network arms of `sample` (lines 330–352),
  to keep the build green until Task 3.
- Test: `crates/daemon/tests/app.rs`. Change the `Metric` import (line 4), the `texts` helper
  (lines 54–56) and the battery and network tests (lines 194–222, 248–289, 322–333).

**Interfaces:**
- Produces: `pub struct app::BatteryLabel { pub percent: u8, pub charging: bool }`, which is
  `Debug, Clone, Copy, PartialEq, Eq`.
- Produces: `App::record_battery(&mut self, u64, &BatteryInfo, Option<bool>) -> Option<BatteryLabel>`.
- Produces: `App::record_network(&mut self, u64, Option<&Sample>) -> Option<String>`. The `String`
  is the download text `rate_text` gives, returned when it or the title changed.

- [x] **Step 1: Write the failing tests.** In `crates/daemon/tests/app.rs`:
  - Replace `use statsbar_core::label::Metric;` with nothing.
  - Change `use statsbard::app::{App, READERS};` to `use statsbard::app::{App, BatteryLabel, READERS};`.
  - Replace the `texts` helper:

  ```rust
  fn download(text: &str) -> Option<String> {
      Some(text.to_owned())
  }

  fn battery_label(percent: u8, charging: bool) -> Option<BatteryLabel> {
      Some(BatteryLabel { percent, charging })
  }
  ```

  Replace `the_battery_label_says_chg_only_while_charging` with:

  ```rust
  #[test]
  fn the_battery_label_has_the_bolt_only_while_charging() {
      let mut app = only(&[Battery]);
      use ChargeStatus::{Charging, Discharging, NotCharging};
      assert_eq!(
          app.record_battery(0, &battery(Discharging, 76), Some(false)),
          battery_label(76, false)
      );
      assert_eq!(
          app.record_battery(2 * SECOND, &battery(Discharging, 76), Some(false)),
          None
      );
      assert_eq!(
          app.record_battery(4 * SECOND, &battery(Charging, 76), Some(true)),
          battery_label(76, true)
      );
      assert_eq!(
          app.record_battery(6 * SECOND, &battery(NotCharging, 76), Some(true)),
          battery_label(76, false)
      );
      assert_eq!(
          app.record_battery(8 * SECOND, &battery(NotCharging, 76), Some(true)),
          None,
          "Not charging and Discharging look the same, so the status alone redraws nothing"
      );
      assert_eq!(
          app.record_battery(10 * SECOND, &battery(Discharging, 76), Some(false)),
          None
      );
      assert_eq!(
          app.record_battery(12 * SECOND, &battery(NotCharging, 77), Some(true)),
          battery_label(77, false)
      );
      assert_eq!(
          app.battery().map(|figures| figures.adapter_online),
          Some(Some(true))
      );
  }
  ```

  In `hiding_the_battery_drops_its_figures_and_label`, change `Some((Metric::Bat, 76)),` to
  `battery_label(76, false),`.

  Replace `the_network_label_follows_its_rate_texts` with:

  ```rust
  #[test]
  fn the_network_label_follows_its_download_text() {
      let mut app = only(&[Network]);
      assert_eq!(app.record_network(0, Some(&wlo1(0, 0))), download("0B"));
      assert_eq!(
          app.record_network(2 * SECOND, Some(&wlo1(24_000, 2_000))),
          download("12K")
      );
      assert_eq!(
          app.record_network(4 * SECOND, Some(&wlo1(48_000, 4_000))),
          None
      );
      assert_eq!(app.rows(Network)[0], "Interface wlo1");
  }

  #[test]
  fn an_upload_change_redraws_the_title_but_keeps_the_download_label() {
      let mut app = only(&[Network]);
      app.record_network(0, Some(&wlo1(0, 0)));
      app.record_network(2 * SECOND, Some(&wlo1(24_000, 2_000)));
      // Upload goes from 1.0 to 2.0 KB/s; download stays at 12 KB/s.
      assert_eq!(
          app.record_network(4 * SECOND, Some(&wlo1(48_000, 6_000))),
          download("12K")
      );
      assert_eq!(
          summary::network_title(&app.network().expect("sampled").reading),
          "Network ↓ 12.0 KB/s ↑ 2.0 KB/s"
      );
  }
  ```

  In `a_rate_change_too_small_for_the_label_still_redraws_the_title`, change
  `texts("1K", "12K")` to `download("12K")`. In
  `no_default_route_reads_as_no_interface_and_zero_rates`, change `texts("0B", "0B")` to
  `download("0B")`.

- [x] **Step 2: Run them and expect FAIL.**

  Run: `cargo test -p statsbard --test app`
  Expected: compile error E0432, ``unresolved import `statsbard::app::BatteryLabel` ``.

- [x] **Step 3: Implement.** In `crates/daemon/src/app.rs`:
  - Change `use statsbar_core::label::{self, Metric};` to `use statsbar_core::label;`.
  - Replace the `RateLabel` alias and its doc comment with:

  ```rust
  /// What the battery's tray label shows: the charge, and whether its icon has the charging bolt.
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub struct BatteryLabel {
      pub percent: u8,
      /// Only the kernel's `Charging` status shows the bolt.
      pub charging: bool,
  }
  ```

  The fields:

  ```rust
      battery_label: Option<BatteryLabel>,
  ```

  ```rust
      /// The label's download text and the item's title, as last drawn.
      network_label: Option<(String, String)>,
  ```

  `record_battery`:

  ```rust
      /// Takes a battery sample. Returns the new label when its percent or its charging bolt
      /// changed.
      pub fn record_battery(
          &mut self,
          now_nanos: u64,
          info: &BatteryInfo,
          adapter_online: Option<bool>,
      ) -> Option<BatteryLabel> {
          self.mark_read(ReaderId::Battery, now_nanos);
          let figures = BatteryFigures::new(info, adapter_online);
          let label = BatteryLabel {
              percent: figures.percent,
              charging: figures.status == ChargeStatus::Charging,
          };
          self.battery = Some(figures);
          changed(&mut self.battery_label, label)
      }
  ```

  `record_network`:

  ```rust
      /// Takes a network sample, `None` when there is no default route. Returns the label's
      /// download text when it or the item's title changed. The title carries both rates, finer
      /// than the label, and it must stay as current as the hub row that repeats it.
      pub fn record_network(&mut self, now_nanos: u64, sample: Option<&Sample>) -> Option<String> {
          self.mark_read(ReaderId::Network, now_nanos);
          let reading = self.throughput.update(sample, now_nanos);
          self.network = Some(NetworkStatus {
              interface: sample.map(|sample| sample.interface.clone()),
              reading,
          });
          changed(
              &mut self.network_label,
              (
                  label::rate_text(reading.download_rate),
                  summary::network_title(&reading),
              ),
          )
          .map(|(download, _)| download)
      }
  ```

  In `crates/daemon/src/run.rs`, change the Battery arm's closure to:

  ```rust
              .map(|reading| {
                  app.record_battery(now, &reading.info, reading.adapter_online)
                      .map(|label| {
                          // Interim until Task 3 replaces the pixmaps.
                          let word = if label.charging { Metric::Chg } else { Metric::Bat };
                          percent_look(id, word, label.percent)
                      })
              }),
  ```

  The Network arm compiles unchanged: it only uses the result through `?`.

- [x] **Step 4: Run them and expect PASS.**

  Run: `cargo test -p statsbard`
  Expected: all pass, including the new
  `an_upload_change_redraws_the_title_but_keeps_the_download_label`.

  Run: `./scripts/check.sh`
  Expected: exit 0.

- [x] **Step 5: Commit checkpoint.**

  Ready to commit: `crates/daemon/src/app.rs crates/daemon/src/run.rs crates/daemon/tests/app.rs`
  — `"refactor(daemon): describe tray labels without pixmap words"`

### Task 3: Show a themed icon and a text label on each metric item

**Test level:** unit (the `Tray` methods are called directly, as in the existing tray tests). The
D-Bus transport is covered by Task 1.

**Files:**
- Modify: `crates/daemon/src/tray.rs`, lines 1–117 (module doc, imports, `FALLBACK_ICON`,
  `icon`, `rates_icon`, `MetricItem`, its `Tray` impl). `Hub` and below stay unchanged.
- Modify: `crates/daemon/src/run.rs`. Change the imports (lines 13–14, 23), `Look`,
  `percent_look` and `rates_look` (lines 56–71), the `sample` arms (lines 312–352) and
  `refresh_item` (lines 369–389).
- Test: `crates/daemon/tests/tray.rs`. Change the imports (lines 3, 8), replace
  `a_metric_item_shows_a_themed_icon_until_its_first_value`, `an_icon_above_100_is_refused` and
  `the_rates_icon_is_the_two_line_label` (lines 119–137, 163–168), and add four tests.

**Interfaces:**
- Consumes: `app::BatteryLabel` (Task 2), `Tray::x_ayatana_label` and
  `Tray::x_ayatana_label_guide` (Task 1).
- Produces in `tray`:
  - `pub fn metric_icon(ReaderId) -> &'static str`
  - `pub fn battery_icon(BatteryLabel) -> String`
  - `pub fn percent_label(u8) -> String`
  - `pub fn download_label(&str) -> String`
  - `pub fn label_guide(ReaderId) -> &'static str`
  - `MetricItem { reader, icon: String, label: String, title, rows }`
- Removes: `tray::icon`, `tray::rates_icon`, `MetricItem::icon: Option<Icon>`.

- [x] **Step 1: Write the failing tests.** In `crates/daemon/tests/tray.rs`:
  - Delete `use statsbar_core::label::{self, Metric};`.
  - Change `use statsbard::tray::{self, FALLBACK_ICON, Hub, MetricItem};` to
    `use statsbard::tray::{self, Hub, MetricItem};`.
  - Add `use statsbard::app::BatteryLabel;`.
  - Delete `an_icon_above_100_is_refused` and `the_rates_icon_is_the_two_line_label`. They test
    `tray::icon` and `tray::rates_icon`, which this task removes, and the label functions below
    take no out-of-range input.
  - Replace `a_metric_item_shows_a_themed_icon_until_its_first_value` with:

  ```rust
  #[test]
  fn a_metric_item_shows_its_icon_alone_until_its_first_value() {
      let mut item = MetricItem::new(ReaderId::Cpu);
      assert_eq!(item.icon_name(), "computer-chip-symbolic");
      assert_eq!(item.x_ayatana_label(), "");
      assert_eq!(item.x_ayatana_label_guide(), "100%");
      assert!(item.icon_pixmap().is_empty());

      item.label = tray::percent_label(42);
      assert_eq!(item.x_ayatana_label(), "42%");
      assert!(item.icon_pixmap().is_empty(), "the shell draws the text");
  }

  #[test]
  fn every_metric_has_its_yaru_icon() {
      let icons: Vec<&str> = READERS.map(tray::metric_icon).to_vec();
      assert_eq!(
          icons,
          [
              "computer-chip-symbolic",
              "drive-harddisk-solidstate-symbolic",
              "battery-missing-symbolic",
              "weather-windy-symbolic",
              "network-transmit-receive-symbolic",
          ]
      );
      let icons: Vec<String> = READERS.map(|id| MetricItem::new(id).icon_name()).to_vec();
      assert_eq!(icons, READERS.map(|id| tray::metric_icon(id).to_owned()).to_vec());
  }

  #[test]
  fn the_battery_icon_floors_to_tens_and_has_the_bolt_only_while_charging() {
      for (percent, charging, icon) in [
          (76, false, "battery-level-70-symbolic"),
          (76, true, "battery-level-70-charging-symbolic"),
          (9, false, "battery-level-0-symbolic"),
          (0, true, "battery-level-0-charging-symbolic"),
          (99, false, "battery-level-90-symbolic"),
          (100, false, "battery-level-100-symbolic"),
          (100, true, "battery-level-100-charging-symbolic"),
          (255, false, "battery-level-100-symbolic"),
      ] {
          assert_eq!(
              tray::battery_icon(BatteryLabel { percent, charging }),
              icon,
              "{percent} {charging}"
          );
      }
  }

  #[test]
  fn labels_are_a_bare_percent_or_the_download_rate() {
      assert_eq!(tray::percent_label(0), "0%");
      assert_eq!(tray::percent_label(37), "37%");
      assert_eq!(tray::percent_label(100), "100%");
      assert_eq!(tray::download_label("1.5M"), "↓1.5M");
      assert_eq!(tray::download_label("0B"), "↓0B");
  }

  #[test]
  fn each_guide_is_its_widest_label() {
      for id in READERS {
          let expected = if id == ReaderId::Network { "↓999M" } else { "100%" };
          assert_eq!(tray::label_guide(id), expected, "{id:?}");
          assert_eq!(MetricItem::new(id).x_ayatana_label_guide(), expected, "{id:?}");
      }
  }
  ```

- [x] **Step 2: Run them and expect FAIL.**

  Run: `cargo test -p statsbard --test tray`
  Expected: compile errors E0425, ``cannot find function `metric_icon` in module `tray` `` (and
  `battery_icon`, `percent_label`, `download_label`, `label_guide`), plus E0609
  ``no field `label` on type `MetricItem` ``.

- [x] **Step 3: Implement `tray.rs`.** Replace lines 1–117 (up to `pub struct Hub`) with:

  ```rust
  //! The `ksni` tray items: one per metric, plus the hub. `ksni`'s driver thread calls their `Tray`
  //! methods and menu callbacks; the main loop changes their fields through `Handle::update`, whose
  //! closure runs on the main thread under `ksni`'s lock. Menu clicks become `Command`s for the main
  //! loop; nothing here reads or writes files.
  //!
  //! A metric item is a themed icon with its value as `XAyatanaLabel` text. The shell draws that
  //! text in its panel font, which stays legible at fractional scales where a pixmap blurs
  //! (spike S3).

  use async_channel::Sender;
  use ksni::menu::{CheckmarkItem, StandardItem};
  use ksni::{Category, MenuItem};
  use statsbar_core::schedule::ReaderId;

  use crate::app::{BatteryLabel, Command, READERS};
  use crate::config::Items;
  use crate::summary::{self, Overview};

  /// The hub's icon.
  pub const FALLBACK_ICON: &str = "utilities-system-monitor";

  /// The icon an item shows next to its label. Only the battery's changes after its first value
  /// (`battery_icon`). Every name was found in Yaru on 2026-11-21.
  pub fn metric_icon(reader: ReaderId) -> &'static str {
      match reader {
          ReaderId::Cpu => "computer-chip-symbolic",
          ReaderId::Memory => "drive-harddisk-solidstate-symbolic",
          // Until the first sample says how full the battery is.
          ReaderId::Battery => "battery-missing-symbolic",
          // Yaru has no fan icon; moving air is the nearest.
          ReaderId::Fan => "weather-windy-symbolic",
          ReaderId::Network => "network-transmit-receive-symbolic",
      }
  }

  /// The icon GNOME Shell's own battery indicator would pick: the charge floored to a multiple of
  /// 10, with the bolt while charging.
  pub fn battery_icon(label: BatteryLabel) -> String {
      let level = label.percent.min(100) / 10 * 10;
      if label.charging {
          format!("battery-level-{level}-charging-symbolic")
      } else {
          format!("battery-level-{level}-symbolic")
      }
  }

  /// The text next to a percentage item's icon. No space, unlike the titles, to keep the panel
  /// narrow.
  pub fn percent_label(percent: u8) -> String {
      format!("{percent}%")
  }

  /// The text next to the network icon: the download rate, banded by `label::rate_text`.
  pub fn download_label(download_text: &str) -> String {
      format!("↓{download_text}")
  }

  /// The widest label each item takes, published as `XAyatanaLabelGuide`. Ubuntu's AppIndicator
  /// extension 50.26 does not read it (only listed in its `appIndicator.js`, checked 2026-11-21);
  /// it is there for hosts that do.
  pub fn label_guide(reader: ReaderId) -> &'static str {
      match reader {
          ReaderId::Network => "↓999M",
          ReaderId::Cpu | ReaderId::Memory | ReaderId::Battery | ReaderId::Fan => "100%",
      }
  }

  fn row<T>(text: &str) -> MenuItem<T> {
      StandardItem {
          label: text.to_owned(),
          enabled: false,
          ..Default::default()
      }
      .into()
  }

  pub struct MetricItem {
      pub reader: ReaderId,
      /// A themed icon name.
      pub icon: String,
      /// Empty until the first value, so the panel shows the icon alone.
      pub label: String,
      pub title: String,
      pub rows: Vec<String>,
  }

  impl MetricItem {
      pub fn new(reader: ReaderId) -> MetricItem {
          MetricItem {
              reader,
              icon: metric_icon(reader).to_owned(),
              label: String::new(),
              title: name(reader).to_owned(),
              rows: Vec::new(),
          }
      }
  }

  /// The name in titles, the hub's rows and its switches.
  pub fn name(reader: ReaderId) -> &'static str {
      match reader {
          ReaderId::Cpu => "CPU",
          ReaderId::Memory => "Memory",
          ReaderId::Battery => "Battery",
          ReaderId::Fan => "Fan",
          ReaderId::Network => "Network",
      }
  }

  impl ksni::Tray for MetricItem {
      fn id(&self) -> String {
          match self.reader {
              ReaderId::Cpu => "statsbar-cpu",
              ReaderId::Memory => "statsbar-memory",
              ReaderId::Battery => "statsbar-battery",
              ReaderId::Fan => "statsbar-fan",
              ReaderId::Network => "statsbar-network",
          }
          .to_owned()
      }

      fn category(&self) -> Category {
          Category::Hardware
      }

      fn title(&self) -> String {
          self.title.clone()
      }

      fn icon_name(&self) -> String {
          self.icon.clone()
      }

      fn x_ayatana_label(&self) -> String {
          self.label.clone()
      }

      fn x_ayatana_label_guide(&self) -> String {
          label_guide(self.reader).to_owned()
      }

      fn menu(&self) -> Vec<MenuItem<Self>> {
          self.rows.iter().map(|text| row(text)).collect()
      }
  }
  ```

- [x] **Step 4: Implement `run.rs`.**
  - Change `use ksni::{Handle, Icon, TrayMethods};` to `use ksni::{Handle, TrayMethods};`.
  - Delete `use statsbar_core::label::Metric;`.
  - Change `use crate::app::{App, Command, READERS, slot};` to
    `use crate::app::{App, BatteryLabel, Command, READERS, slot};`.
  - Replace the `Look` alias, `percent_look` and `rates_look` with:

  ```rust
  /// What an item shows: its themed icon, its label text and its title.
  struct Look {
      icon: String,
      label: String,
      title: String,
  }

  fn percent_look(id: ReaderId, value: u8) -> Look {
      Look {
          icon: tray::metric_icon(id).to_owned(),
          label: tray::percent_label(value),
          title: summary::title(tray::name(id), Some(value)),
      }
  }

  fn battery_look(label: BatteryLabel) -> Look {
      Look {
          icon: tray::battery_icon(label),
          label: tray::percent_label(label.percent),
          title: summary::title(tray::name(ReaderId::Battery), Some(label.percent)),
      }
  }

  fn network_look(download_text: &str, reading: &Reading) -> Look {
      Look {
          icon: tray::metric_icon(ReaderId::Network).to_owned(),
          label: tray::download_label(download_text),
          title: summary::network_title(reading),
      }
  }
  ```

  In `sample`, change:
  - `percent_look(id, Metric::Cpu, value)`, `percent_look(id, Metric::Ram, value)` and
    `percent_look(id, Metric::Fan, value)` to `percent_look(id, value)`.
  - The Battery closure (Task 2's interim) to
    `.map(|reading| app.record_battery(now, &reading.info, reading.adapter_online).map(battery_look))`.
  - The Network closure to:

  ```rust
              .map(|sample| {
                  let download = app.record_network(now, sample.as_ref())?;
                  app.network()
                      .map(|status| network_look(&download, &status.reading))
              }),
  ```

  In `refresh_item`, change the closure body to:

  ```rust
              move |item: &mut MetricItem| {
                  item.rows = rows;
                  if let Some(look) = redraw {
                      item.icon = look.icon;
                      item.label = look.label;
                      item.title = look.title;
                  }
              },
  ```

  Also change its comment's "the icon and title" to "the icon, label and title".

- [x] **Step 5: Run them and expect PASS.**

  Run: `cargo test -p statsbard`
  Expected: all pass, including the five tray tests above and `ayatana_label`.

  Run: `./scripts/check.sh`
  Expected: exit 0. Nothing in `crates/daemon` uses `label::render` or `label::render_rates` any
  more.

  Run: `grep -rn "render\|Metric::" crates/daemon/src crates/daemon/tests`
  Expected: no match.

- [x] **Step 6: Commit checkpoint.**

  Ready to commit: `crates/daemon/src/tray.rs crates/daemon/src/run.rs crates/daemon/tests/tray.rs`
  — `"feat(daemon): show a themed icon with the value as panel text"`

### Task 4: Remove the pixmap label renderer

`RED: n/a — dead code removed. Covered by rate_text_bands_like_the_menu_bar_rate,
./scripts/check.sh, and a build of spikes/tray.` After Task 3 nothing outside `crates/core`
calls the removed items. The deleted tests cover only the deleted functions.

**Test level:** unit (the remaining `rate_text` test), plus builds.

**Files:**
- Modify: `crates/core/src/label.rs`. Keep only `rate_text`, lines 179–216, and its doc.
- Modify: `crates/core/src/lib.rs`, line 3 (the module doc mentions pixmaps)
- Modify: `crates/core/tests/label.rs`. Keep only `rate_text_bands_like_the_menu_bar_rate`,
  lines 241–264.
- Delete: `spikes/tray/src/bin/sharp.rs`
- Modify: `spikes/tray/Cargo.toml` (lines 16–17, the `statsbar-core` dependency and its comment),
  `spikes/tray/Cargo.lock`
- Modify: `docs/spikes/2026-11-21-s3-panel-legibility.md` (the "Larger pixmaps" section)

**Interfaces:** removes `label::{SIZE, Metric, render, render_rates}`. `label::rate_text(f64) -> String`
is unchanged.

- [x] **Step 1: Run the covering tests and expect PASS.**

  Run: `cargo test -p statsbar-core --test label rate_text`
  Expected: `rate_text_bands_like_the_menu_bar_rate ... ok`.

  Run: `grep -rn "label::render\|label::SIZE\|Metric::\|label::Metric" crates`
  Expected: matches only in `crates/core/src/label.rs` and `crates/core/tests/label.rs`.

- [x] **Step 2: Remove.** Replace `crates/core/src/label.rs` with:

  ```rust
  //! The tray's short rate text. The labels themselves are text that the shell draws in its panel
  //! font (spike S3), so nothing here draws pixels.

  /// A rate as the network label shows it: at most three characters of number and a unit letter,
  /// `B`, `K`, `M` or `G`, counted in powers of 1000 as network tools do. One decimal only below
  /// 10 M and 10 G, as in `format::menu_bar_rate`. 999.5 GB/s and above read `999G`. NaN and
  /// anything not above zero read `0B`.
  pub fn rate_text(bytes_per_second: f64) -> String {
      // Promote at 999.5, where whole-number rounding would print "1000".
      const PROMOTE_ABOVE: f64 = 999.5;
      fn banded(value: f64, unit: char) -> String {
          // One decimal only while it fits in three characters ("9.9").
          if value < 9.95 {
              format!("{value:.1}{unit}")
          } else {
              format!("{value:.0}{unit}")
          }
      }
      // A comparison, not `max`: it also turns NaN and -0.0 into 0, and "-0" is not a rate.
      let rate = if bytes_per_second > 0.0 {
          bytes_per_second
      } else {
          0.0
      };
      if rate < PROMOTE_ABOVE {
          return format!("{rate:.0}B");
      }
      let kilobytes = rate / 1000.0;
      if kilobytes < PROMOTE_ABOVE {
          return format!("{kilobytes:.0}K");
      }
      let megabytes = kilobytes / 1000.0;
      if megabytes < PROMOTE_ABOVE {
          return banded(megabytes, 'M');
      }
      let gigabytes = megabytes / 1000.0;
      if gigabytes < PROMOTE_ABOVE {
          return banded(gigabytes, 'G');
      }
      "999G".to_owned()
  }
  ```

  Replace `crates/core/tests/label.rs` with its import changed to
  `use statsbar_core::label::rate_text;` and only the `rate_text_bands_like_the_menu_bar_rate`
  test, unchanged. Delete the `HashSet` import, the `pixel` helper and every other test: each one
  draws with `render` or `render_rates`.

  In `crates/core/src/lib.rs`, change `Everything here turns inputs into values, labels or pixmaps.`
  to `Everything here turns inputs into values and label text.`

  Delete `spikes/tray/src/bin/sharp.rs`. In `spikes/tray/Cargo.toml`, delete the
  `# Spike S3 draws with the real renderer; …` comment and the `statsbar-core = { … }` line.

  In `docs/spikes/2026-11-21-s3-panel-legibility.md`, add at the end of "Larger pixmaps":

  ```markdown
  Plan 03b removed the renderer and this probe. To rerun it, check out a30a747 and build
  `spikes/tray` there.
  ```

  Replace the "**Decision:** pending. …" paragraph with:

  ```markdown
  **Decision (2026-11-21):** Plan 03b publishes the label through a vendored `ksni` 0.3.7 patched
  with `XAyatanaLabel` (`vendor/ksni/PATCHES.md`), and removes the pixmap labels of Plans 02b
  and 03.
  ```

- [x] **Step 3: Run the covering tests and builds and expect PASS.**

  Run: `./scripts/check.sh`
  Expected: exit 0. The purity gate still passes, since `label.rs` only lost code.

  Run: `cargo build --manifest-path spikes/tray/Cargo.toml --bins`
  Expected: success. `Cargo.lock` there drops `statsbar-core`.

  Run: `grep -rn "render\|Metric" crates/core/src crates/core/tests/label.rs`
  Expected: no match.

- [x] **Step 4: Commit checkpoint.**

  Ready to commit: `crates/core/src/label.rs crates/core/src/lib.rs crates/core/tests/label.rs spikes/tray/src/bin/sharp.rs spikes/tray/Cargo.toml spikes/tray/Cargo.lock docs/spikes/2026-11-21-s3-panel-legibility.md`
  — `"refactor(core): remove the pixmap label renderer"`

### Task 5: Verification & Docs

**Test level:** end to end, with the release daemon in the user's GNOME session.

**Files:**
- Create: `docs/spikes/2026-11-21-03b-text-labels-e2e.md`
- Modify: `docs/plans/2026-11-16-rust-statsbar-roadmap.md`:
  - **Tech stack**: `ksni` 0.3.7, vendored with `XAyatanaLabel`.
  - The **Tray label** line in Technical Defaults.
  - The first **Risks** row.
  - A new **Plan 03b** entry after Plan 03.
- Modify: `README.md` (a Plan 03b line after the Plan 03 lines)

- [x] **Step 1: Run the full suite and the gates.**

  Run: `./scripts/check.sh`
  Expected: exit 0.

  Run: `cargo audit`
  Expected: no advisory.

  Run: `cargo build --release -p statsbard`
  Expected: success.

  Run: `cargo tree -p statsbard -i ksni`
  Expected: `ksni v0.3.7 (…/vendor/ksni)`.

- [x] **Step 2: Run the daemon.** Stop any running `statsbard` (`pkill -x statsbard`). Then start
  `./target/release/statsbard` in the background.

- [x] **Step 3: Check the properties over D-Bus.** For each item name from
  `busctl --user list | grep StatusNotifierItem`, run:

  ```bash
  busctl --user get-property <name> /StatusNotifierItem org.kde.StatusNotifierItem Id IconName XAyatanaLabel XAyatanaLabelGuide
  ```

  Expected for every item:
  - CPU and Memory show their icon with `NN%` and guide `100%`.
  - Battery shows `battery-level-N0-symbolic` with `NN%`, where N0 is the charge from
    `cat /sys/class/power_supply/BAT0/capacity` floored to a multiple of 10.
  - Fan shows `weather-windy-symbolic` with `NN%`.
  - Network shows `network-transmit-receive-symbolic` with `↓…` and guide `↓999M`.

- [x] **Step 4: Check the signal and the values.**

  Run: `timeout 10 dbus-monitor --session "type='signal',member='XAyatanaNewLabel'"`
  Expected: a signal whenever a label changes, with label and guide strings. An idle machine
  sends few or none.

  Compare the CPU label with `top`, the Memory label with `free`, and the Network label with
  the item's title. Each must be within one sample.

- [x] **Step 5: Check the panel.** — deviated: with all five items shown GNOME ellipsizes every label; accepted by the user as a gap (checkpoint 2). Take a screenshot with the Desktop Screenshot portal, as spike
  S3 did, and keep the crop outside Git. Ask the user to confirm:
  - each icon with its number is legible at 125 % scale;
  - nothing pushes into the clock;
  - what happens to the neighbouring items when a label's width changes (the guide is ignored).

  The user's verdict is the evidence. A screenshot alone is not.

- [x] **Step 6: Check visibility, threads and memory.**
  - Untick and retick *Show Network* through the hub menu over D-Bus, as in the 02b e2e run.
    Expected: the item disappears and reappears with its icon and label.
  - Run `ps -T -p "$(pgrep -x statsbard)"`. Expected: 4 threads, as the invariant allows.
  - Run `grep Pss /proc/$(pgrep -x statsbard)/smaps_rollup`. Record the value against Plan 03's
    2997 kB.
  - If the laptop is not charging (charge above the 75 % start threshold), report
    `battery-level-N0-charging-symbolic` as seen in tests only.

- [x] **Step 7: Write the docs.**
  - `docs/spikes/2026-11-21-03b-text-labels-e2e.md`: environment (as in S3), the commands above
    and their output, the user's verdict, threads and Pss, and every gap (the charging icon if
    unseen, the ignored guide).
  - Roadmap:
    - **Tech stack**: `ksni` 0.3.7 *(vendored at `vendor/ksni` with `XAyatanaLabel`, see its
      `PATCHES.md`)*.
    - Technical Defaults **Tray label**: replace the Stats Mini pixmap text with "a themed Yaru
      icon with the value as `XAyatanaLabel` text (Plan 03b)".
    - First Risks row: mitigation "Plan 03b shows the value as panel text", and the evidence link.
    - A Plan 03b entry with scope, tests and evidence, in the style of the Plan 03 entry.
    - In "Why this shape", change the row "Tray pixmap redrawn only when its text changes" to
      "Tray label resent only when its text changes".
  - README: after the Plan 03 lines,

  ```markdown
  - [Plan 03b — text labels](docs/plans/2026-11-21-rust-statsbar-03b-text-labels.md): a themed
    icon with the value as panel text, through a vendored `ksni` with `XAyatanaLabel`;
    [end-to-end run](docs/spikes/2026-11-21-03b-text-labels-e2e.md).
  ```

- [x] **Step 8: Final gate.** — review: 2 rounds, Approved; round 1 Minor (Look glue untested) fixed by moving the builders to `tray` with whole-value tests; other Minor items left as suggestions (checkpoint 3)
  - Read the full diff against `main`, untracked files included. It must
    contain nothing unrelated, no debug output, no TODOs and no secrets. `vendor/ksni` differs
    from the registry copy only as `PATCHES.md` lists; check with
    `diff -r ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/ksni-0.3.7 vendor/ksni`.
  - Run `./scripts/check.sh` again after the last edit.
  - Self-review: requirements 1–6, error paths (a failed `update` is still logged by `run::update`),
    no new thread, and naming.
  - Run `code-review-strict` and address or report its findings. If anything changed, repeat
    this step.

- [x] **Step 9: Commit checkpoint.**

  Ready to commit: `docs/spikes/2026-11-21-03b-text-labels-e2e.md docs/plans/2026-11-16-rust-statsbar-roadmap.md README.md docs/plans/2026-11-21-rust-statsbar-03b-text-labels.md`
  — `"docs: record the text label end-to-end run"`

  Report as gaps if they happen:
  - the charging icon was not seen live;
  - the guide had no effect in GNOME;
  - any panel check the user could not make.
