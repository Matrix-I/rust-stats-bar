# Plan 02b — Daemon skeleton: CPU and Memory in the tray

> **For agentic workers:** REQUIRED SUB-SKILL: development-superpowers:subagent-driven-development or
> executing-plans-strict. Steps use `- [ ]`.

**Goal:** a running `statsbard` that shows live CPU and Memory labels in the GNOME panel, a hub
item with show/hide switches, launch at login and Quit, and reads nothing for a hidden item.

**Architecture:** three crates. `statsbar-core` gains a pure `schedule` module. A new
`statsbar-readers` crate keeps `/proc/stat` and `/proc/meminfo` open and re-reads them with `pread`.
A new `statsbard` crate (lib + bin) holds the config, autostart, menu text, app state, the `ksni`
tray items, the power-profile watch and the event loop. Everything except the loop is in the lib and
tested. The loop runs on the main thread under `async_io::block_on`. It ticks the daemon's single
system-bus `zbus` connection, as spike S2 did with the session bus.

**Tech stack:** Rust edition 2024, workspace floor 1.85, and 1.87 for `statsbard` (zbus 5.19.0
requires it). New dependencies, approved by the user on
2026-11-18: `ksni =0.3.7` (`async-io`), `zbus =5.19.0` (`async-io`), `async-io =2.6.0`,
`futures-lite =2.6.1`, `async-channel =2.5.0`, `serde =1.0.229` (`derive`), `toml =1.2.1`. All but
`toml` are already in the S2 spike's lock file. crates.io was checked on 2026-11-18:
`toml 1.2.1+spec-1.1.0` is the newest stable version.

**Branch:** `feature/daemon-skeleton`, cut from `main`. `main` already has the label renderer. The
roadmap's one-branch-per-plan rule applies. The 02b exception covered only the label renderer.

## Confirmed Requirements

The user accepted every recommendation on 2026-11-18 ("Theo đề xuất"):

1. `config::parse` lives in `crates/daemon` and uses `toml` and `serde`. `crates/core` and its
   purity gate stay unchanged. This deviates from the roadmap, which listed `config::parse` under
   the core additions.
2. Config schema: `[items]` with `cpu` and `memory` booleans, both `true` by default. A missing file
   means defaults. An invalid or unreadable file means defaults plus a message on stderr, and the
   daemon never overwrites it. Unknown keys are ignored, and they are kept when the daemon saves.
3. Launch at login: the setting *is* the presence of `~/.config/autostart/statsbar.desktop`. There
   is no config key for it, and it is off on first run.
4. Hub menu, in order: overview rows (`CPU 24 %`, `Memory 50 % · 8.00 GB / 16.00 GB`), separator,
   the `Show CPU` and `Show Memory` checkmarks, separator, the `Launch at login` checkmark, separator,
   `Quit`. The icon is `utilities-system-monitor`. The hub cannot be hidden.
5. Item menus. CPU shows Usage, User, System, Idle and Cores. Memory shows Used (x of y, %), App,
   Kernel, Cached, Free and Swap, plus Compressed when it is above 0. There is no "Open details…"
   item until Plan 04.
6. Power saver lives in 02b. The daemon watches `ActiveProfile` of
   `org.freedesktop.UPower.PowerProfiles` on the **system** bus. The property stream yields the
   current value first, then each change. If the bus or the service is missing, power saver is
   off and the daemon logs the reason once.
7. The worker thread moves to Plan 05, where its first caller (the public IP lookup) arrives.
8. The thread invariant is reworded now with the Plan 02a Step 3 text (Task 0). That text is the
   list `ps -T` is checked against.
9. When a switch saves the config, comments in the file are lost and its keys are re-sorted.
   Unknown keys and their values are still kept. No `toml_edit` dependency. *(user, 2026-11-18,
   after the plan review raised it)*

## Global Constraints

These come from the roadmap and the user's global rules. The roadmap wins on project matters.

- Test-first: happy path, edge cases, error paths. Untested code is reported incomplete, never done.
- Each test can fail without its change, and asserts observable behaviour. Mock only true
  boundaries. No sleeps, wall clock, ordering or network in tests.
- Never weaken, skip or delete a test to make it pass.
- Commit or push only when asked. Each task therefore ends with `Ready to commit: …`, never
  `git commit`. Use Conventional Commits in the imperative mood, one logical change per commit.
- Code, comments, names, commit messages and docs are in English. Comments explain why.
- Every external call has a timeout. Setup calls get 5 s and per-tick D-Bus or tray calls get 1 s,
  which are the values S2 ran with. Nothing is unbounded: the command queue holds 8 and a file read
  is capped at 256 KiB.
- The host clock is never used in logic or tests. The loop passes monotonic nanoseconds from an
  `Instant` taken at start. `crates/core` never reads a clock.
- **Project invariant (enforced by `scripts/check-core-purity.sh`):** `crates/core` does no I/O,
  reads no clock, spawns nothing, has no `unsafe` and no build script, and depends on nothing but
  `serde`.
- **Project invariant (as reworded in Task 0):** `statsbard` never links GTK and never starts a
  tokio runtime. Its threads are the main loop (which also ticks the daemon's `zbus` connection),
  the `async-io` reactor, the single `ksni` driver, the worker (from Plan 05), and the `blocking`
  pool threads that `zbus` uses while connecting.
- Verification entry point: `./scripts/check.sh` (fmt, clippy `-D warnings`, tests, purity gate).
- Final gate: read the full diff, run `./scripts/check.sh`, self-review, run `code-review-strict`,
  and repeat if anything changed.

## Technical Defaults

- **Readers keep files open.** The roadmap shape `cpu::read(root) -> io::Result<…>` becomes
  `CpuReader::open(root)` followed by `.read()`. The reason is the roadmap's "files kept open and
  re-read with `pread`" row. A hidden item drops its reader, which closes the file.
- **`schedule::due` takes `power_saver`** as an argument, next to `now_nanos` and the reader
  states. Power saver is daemon-wide state, not per-reader state.
- **Precedence follows upstream:** hidden with the page closed means not polled. Otherwise power
  saver gives 5 s, page open 1 s and visible 2 s. Power saver wins over page open, as in upstream's
  CPU reader. `page_open` stays `false` until Plan 04 adds the panel.
- **Percent:** `format::rounded_int` clamped to 0–100. This is the existing core rounding.
- **Gigabytes:** the existing `format::fmt_gb`, with two decimals.
- **Redraws:** a label is redrawn only when its whole percent changes. Menu rows are sent when they
  change, and `ksni` diffs the layout before it emits `LayoutUpdated`.
- **Before the first value**, a metric item shows the themed icon `utilities-system-monitor`. A CPU
  load needs two samples, so this lasts one interval.
- **Hub rows:** a hidden metric's row is left out, because a hidden metric is not read. A shown
  metric with no value yet reads `CPU —`.
- **Reads of the daemon's own files** (config, autostart entry) are capped at 256 KiB, the same
  cap as `/proc` reads: nothing is read unbounded.
- **Readers drift apart after a toggle:** a shown-again item restarts its own interval, so the
  loop may wake twice per interval until the next restart. That is accepted for two readers;
  Plan 11 measures idle wake-ups.
- **Failed reads** count as reads for the schedule, so a broken file is retried each interval
  instead of in a busy loop. Only the first failure in a row is logged. The reader is dropped and
  reopened on the next attempt.
- **Config writes** go to `<file>.tmp`, then `sync_all`, then a rename over the target. A leftover
  `.tmp` is truncated by the next write, so it needs no cleanup.
- **Config path:** `$XDG_CONFIG_HOME`, else `$HOME/.config`. A relative or empty value is ignored,
  as the XDG Base Directory spec says.
- **Exec quoting** follows the Desktop Entry Specification, "The Exec key": the general string
  escaping is applied before the quoting rule, and `%` is written as `%%`.
- **Tests that need files** write under `env!("CARGO_TARGET_TMPDIR")`, in one directory per test.

## Risks & Rollback

| Risk | Mitigation | Revert |
|---|---|---|
| A crate API differs from what this plan assumes | Each task's first step reads docs.rs for the pinned version. Signatures this plan relies on were read from the registry sources on 2026-11-18: `ksni` `Handle::update`/`shutdown` and the menu item structs; `zbus` `Builder::internal_executor`, `Proxy::new` and `receive_property_changed`; `toml::map::Entry::or_insert` | Fix the code to match the docs, not the docs to the code |
| The daemon writes to the user's `~/.config` | Every write is atomic and additive. Unknown keys survive a save. An invalid file is never overwritten. The end-to-end run uses a scratch `XDG_CONFIG_HOME` | Delete `~/.config/statsbar/config.toml` and `~/.config/autostart/statsbar.desktop` |
| A menu row update each tick costs D-Bus traffic | `ksni` diffs the layout; rows are sent only when their text changes | Plan 11 measures it; fallback is to refresh rows in `menu_about_to_show` |
| The `blocking` pool can start more threads under load | Same as S2: only while `zbus` connects | The `ps -T` check reports what it finds |
| Power profiles on the system bus differs from S2's session bus | Same `internal_executor(false)` path; if it fails, power saver stays off and the daemon still runs | — |
| Config format is persisted | It only gains keys; `[items]` is new. A save drops comments and re-sorts keys (requirement 9) | Delete the file |
| Rewording the thread invariant | Applied with the user's approval, and recorded in the roadmap and in 02a Step 3 | `git revert` of the Task 0 commit |

---

## Task 0 — Branch and thread invariant

**Files:** `docs/plans/2026-11-16-rust-statsbar-roadmap.md`,
`docs/plans/2026-11-17-rust-statsbar-02a-spikes.md`.

- [x] **Step 1: Branch.** Run `git switch main && git pull --ff-only && git switch -c
  feature/daemon-skeleton`. Expect a clean tree, apart from this plan file if it is still
  untracked.
- [x] **Step 2: Reword the invariant.** In the roadmap, replace the whole bullet that starts with
  `**Project invariant:** \`statsbard\` never links GTK`, including its *Pending revision*
  paragraph, with:

```markdown
- **Project invariant:** `statsbard` never links GTK and never starts a tokio runtime. Its threads are
  bounded and named in this list, measured by spike S2 (`docs/spikes/2026-11-17-s2-threads.md`): the
  main event loop, which also ticks the daemon's own `zbus` connection; `async-io`'s reactor thread;
  `ksni`'s single driver thread, shared by every tray item; and one worker that runs blocking calls
  (HTTP, `ideviceinfo`, `adb`) one at a time, each with its own timeout, so a slow call never stalls
  polling or the tray. Threads of the `blocking` pool that `zbus` uses while connecting are allowed;
  S2 found one (`blocking-1`) still present after 30 s, and Plan 11 measures whether it retires.
  *Reworded 2026-11-18 with the user's approval.*
```

- [x] **Step 3: Update the roadmap's other mentions of the invariant.**
  - Risks row "Tray and D-Bus threads count towards the daemon's budget": set its Revert cell to
    `Settled by spike S2: the invariant lists the threads S2 measured.`
  - Plan 02a's Evidence: replace `The daemon thread invariant revision is deferred by the user.`
    with `The thread invariant was reworded from S2's measurements on 2026-11-18.`
  - Plan 02b entry: replace its last sentence (`This check waits on …`) with
    `Written: [the daemon plan](2026-11-18-rust-statsbar-02b-daemon.md). The worker thread moves to Plan 05; config parsing lives in the daemon crate, since \`toml\` cannot enter \`core\`.`
  - Plan 05 Scope: append `the worker thread (moved from 02b), with the public IP lookup as its first caller;`.
- [x] **Step 4: Tick 02a Step 3.** In `2026-11-17-rust-statsbar-02a-spikes.md`, change
  `- [x] **Step 3: Record S2's outcome` to `- [x]` and append under it:
  `Executed 2026-11-18: the user approved the wording; blocking-1 was recorded as present after 30 s.`
  On line 894 of the same file, replace `The user deferred changing the project invariant.` with
  `The user approved the reworded invariant on 2026-11-18.`
- [x] **Step 5: Check.** Run `grep -n 'Pending revision\|deferred by the user\|user deferred' docs/plans/2026-11-16-rust-statsbar-roadmap.md docs/plans/2026-11-17-rust-statsbar-02a-spikes.md`
  and expect no output.

Ready to commit: `docs/plans/2026-11-16-rust-statsbar-roadmap.md docs/plans/2026-11-17-rust-statsbar-02a-spikes.md docs/plans/2026-11-18-rust-statsbar-02b-daemon.md` — "docs: reword the thread invariant from spike S2 and add the daemon plan"

---

## Task 1 — `schedule` in core

**Files:** create `crates/core/tests/schedule.rs` and `crates/core/src/schedule.rs`. Modify
`crates/core/src/lib.rs` to add `pub mod schedule;` after `pub mod memory;`.

**Consumes:** nothing new.
**Produces:** `schedule::ReaderId::{Cpu, Memory}`,
`schedule::ReaderState { id, visible, page_open, last_read_nanos: Option<u64> }`,
`schedule::interval_nanos(&ReaderState, power_saver: bool) -> Option<u64>`,
`schedule::due(now_nanos: u64, power_saver: bool, readers: &[ReaderState]) -> Vec<ReaderId>`, and
`schedule::next_wake_nanos(power_saver: bool, readers: &[ReaderState]) -> Option<u64>`.

- [x] **Step 1: Write the failing test** `crates/core/tests/schedule.rs`:

```rust
use statsbar_core::schedule::{ReaderId, ReaderState, due, interval_nanos, next_wake_nanos};

const SECOND: u64 = 1_000_000_000;

fn reader(id: ReaderId, visible: bool, page_open: bool, last: Option<u64>) -> ReaderState {
    ReaderState {
        id,
        visible,
        page_open,
        last_read_nanos: last,
    }
}

#[test]
fn intervals_follow_the_roadmap_cadence_table() {
    // (visible, page open, power saver, interval)
    let cases = [
        (false, false, false, None),
        (false, false, true, None),
        (true, false, false, Some(2 * SECOND)),
        (true, true, false, Some(SECOND)),
        (false, true, false, Some(SECOND)),
        (true, false, true, Some(5 * SECOND)),
        (true, true, true, Some(5 * SECOND)),
    ];
    for (visible, page_open, power_saver, expected) in cases {
        let state = reader(ReaderId::Cpu, visible, page_open, None);
        assert_eq!(
            interval_nanos(&state, power_saver),
            expected,
            "visible {visible}, page open {page_open}, power saver {power_saver}"
        );
    }
}

#[test]
fn a_reader_never_read_is_due_at_once() {
    let readers = [reader(ReaderId::Cpu, true, false, None)];
    assert_eq!(due(0, false, &readers), vec![ReaderId::Cpu]);
    assert_eq!(next_wake_nanos(false, &readers), Some(0));
}

#[test]
fn a_visible_reader_is_due_exactly_one_interval_after_its_last_read() {
    let readers = [reader(ReaderId::Memory, true, false, Some(10 * SECOND))];
    assert!(due(12 * SECOND - 1, false, &readers).is_empty());
    assert_eq!(due(12 * SECOND, false, &readers), vec![ReaderId::Memory]);
}

#[test]
fn a_hidden_reader_with_its_page_closed_is_never_due() {
    let readers = [reader(ReaderId::Cpu, false, false, None)];
    assert!(due(u64::MAX, false, &readers).is_empty());
    assert_eq!(next_wake_nanos(false, &readers), None);
}

#[test]
fn power_saver_stretches_the_interval_to_five_seconds() {
    let readers = [reader(ReaderId::Cpu, true, false, Some(0))];
    assert!(due(5 * SECOND - 1, true, &readers).is_empty());
    assert_eq!(due(5 * SECOND, true, &readers), vec![ReaderId::Cpu]);
}

#[test]
fn due_readers_keep_their_input_order() {
    let readers = [
        reader(ReaderId::Memory, true, false, None),
        reader(ReaderId::Cpu, true, false, None),
    ];
    assert_eq!(
        due(0, false, &readers),
        vec![ReaderId::Memory, ReaderId::Cpu]
    );
}

#[test]
fn next_wake_is_the_earliest_due_time() {
    let readers = [
        reader(ReaderId::Cpu, true, false, Some(3 * SECOND)),
        reader(ReaderId::Memory, true, true, Some(3 * SECOND)),
    ];
    assert_eq!(next_wake_nanos(false, &readers), Some(4 * SECOND));
}

#[test]
fn a_last_read_near_the_end_of_time_saturates_instead_of_wrapping() {
    let readers = [reader(ReaderId::Cpu, true, false, Some(u64::MAX - 1))];
    assert!(due(u64::MAX - 1, false, &readers).is_empty());
    assert_eq!(next_wake_nanos(false, &readers), Some(u64::MAX));
}
```

- [x] **Step 2: Run it.** `cargo test -p statsbar-core --test schedule` should FAIL to compile with
  `unresolved import statsbar_core::schedule`.
- [x] **Step 3: Implement** `crates/core/src/schedule.rs`:

```rust
//! When each reader is due. One timer serves every reader, and a metric nobody is looking at is
//! never scheduled. Times are monotonic nanoseconds passed in by the caller.

const NANOS_PER_SECOND: u64 = 1_000_000_000;
/// Item visible, its page closed.
pub const VISIBLE_INTERVAL_NANOS: u64 = 2 * NANOS_PER_SECOND;
/// That metric's page is open in the panel.
pub const PAGE_OPEN_INTERVAL_NANOS: u64 = NANOS_PER_SECOND;
/// The `power-saver` profile is active. Upstream's CPU reader lets Low Power Mode win over an
/// open page, and so does this.
pub const POWER_SAVER_INTERVAL_NANOS: u64 = 5 * NANOS_PER_SECOND;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReaderId {
    Cpu,
    Memory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReaderState {
    pub id: ReaderId,
    /// Its tray item is shown.
    pub visible: bool,
    /// Its page is open in the panel.
    pub page_open: bool,
    /// When it was last read, successfully or not. `None` before the first read.
    pub last_read_nanos: Option<u64>,
}

/// How often a reader is read, or `None` when nothing shows its value.
pub fn interval_nanos(state: &ReaderState, power_saver: bool) -> Option<u64> {
    if !state.visible && !state.page_open {
        return None;
    }
    Some(if power_saver {
        POWER_SAVER_INTERVAL_NANOS
    } else if state.page_open {
        PAGE_OPEN_INTERVAL_NANOS
    } else {
        VISIBLE_INTERVAL_NANOS
    })
}

/// When `state` is next due, or `None` when it is not scheduled. Saturating, so a time near the
/// end of `u64` cannot wrap round to "due long ago".
fn due_at(state: &ReaderState, power_saver: bool) -> Option<u64> {
    let interval = interval_nanos(state, power_saver)?;
    Some(state.last_read_nanos.map_or(0, |last| last.saturating_add(interval)))
}

/// The readers due at `now_nanos`, in the order given.
pub fn due(now_nanos: u64, power_saver: bool, readers: &[ReaderState]) -> Vec<ReaderId> {
    readers
        .iter()
        .filter(|state| due_at(state, power_saver).is_some_and(|at| now_nanos >= at))
        .map(|state| state.id)
        .collect()
}

/// When the loop should next wake, or `None` when no reader is scheduled and only a menu click or
/// a profile change can wake it.
pub fn next_wake_nanos(power_saver: bool, readers: &[ReaderState]) -> Option<u64> {
    readers
        .iter()
        .filter_map(|state| due_at(state, power_saver))
        .min()
}
```

- [x] **Step 4: Run** `cargo test -p statsbar-core --test schedule` and expect 8 passed. Then run
  `./scripts/check-core-purity.sh` and expect `core purity: ok`.

- [x] **Format and lint.** Run `cargo fmt --all`, then
  `cargo clippy --workspace --all-targets -- -D warnings`, and expect it clean. CI runs
  `cargo fmt --check`, so an unformatted commit fails there.

Ready to commit: `crates/core/src/schedule.rs crates/core/src/lib.rs crates/core/tests/schedule.rs` — "feat(core): schedule readers by visibility, open page and power saver"

---

## Task 2 — `statsbar-readers`: CPU and memory readers

**Files:** create `crates/readers/Cargo.toml`, `crates/readers/src/lib.rs`,
`crates/readers/src/procfile.rs`, `crates/readers/src/cpu.rs`, `crates/readers/src/memory.rs`,
`crates/readers/tests/cpu.rs`, `crates/readers/tests/memory.rs`, and the fixture files below.
Modify the root `Cargo.toml`, setting `members = ["crates/core", "crates/readers"]`.

**Consumes:** `statsbar_core::cpu::{CoreTicks, parse_proc_stat}` and
`statsbar_core::memory::{MemInfo, parse_meminfo}`.
**Produces:** `CpuReader::open(root: &Path) -> io::Result<CpuReader>`,
`CpuReader::read(&mut self) -> io::Result<Vec<CoreTicks>>`,
`MemoryReader::open(root: &Path) -> io::Result<MemoryReader>`, and
`MemoryReader::read(&mut self) -> io::Result<MemInfo>`. A missing file returns `NotFound`.
Truncated, malformed, non-UTF-8 or oversized content returns `InvalidData`.

- [x] **Step 1: Manifest and fixtures.** `crates/readers/Cargo.toml`:

```toml
[package]
name = "statsbar-readers"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
publish.workspace = true

[dependencies]
statsbar-core = { path = "../core" }

[lints]
workspace = true
```

The fixtures keep the layout of this machine's files. The values are reduced to two cores so the
expected numbers can be checked by hand. Create
`crates/readers/tests/fixtures/normal/proc/stat`:

```
cpu  10195 21 3675 181101 584 0 309 0 0 0
cpu0 5208 12 1873 90311 304 0 211 0 0 0
cpu1 4987 9 1802 90790 280 0 98 0 0 0
intr 4310288 9 0 0 0
ctxt 8123456
btime 1763452800
processes 25411
procs_running 2
procs_blocked 0
softirq 1520034 12 402331 3 120044 0 0 9021 610233 0 378390
```

`crates/readers/tests/fixtures/normal/proc/meminfo`:

```
MemTotal:       16222828 kB
MemFree:         9012344 kB
MemAvailable:   11873320 kB
Buffers:          212044 kB
Cached:          2861532 kB
SwapCached:            0 kB
Active:          4290112 kB
Inactive:        1977580 kB
Shmem:            402840 kB
KReclaimable:     187900 kB
Slab:             286660 kB
SReclaimable:     187900 kB
SUnreclaim:        98760 kB
KernelStack:       14592 kB
PageTables:        41236 kB
SwapTotal:       4194300 kB
SwapFree:        4194300 kB
Zswap:                 0 kB
Zswapped:              0 kB
HugePages_Total:       0
Hugepagesize:       2048 kB
```

`crates/readers/tests/fixtures/truncated/proc/stat` (cut mid-line):

```
cpu  10195 21 3675 181101 584 0 309 0 0 0
cpu0 5208 12
```

`crates/readers/tests/fixtures/truncated/proc/meminfo` (cut after a key):

```
MemTotal:       16222828 kB
MemFree:
```

- [x] **Step 2: Write the failing tests.** `crates/readers/tests/cpu.rs`:

```rust
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use statsbar_core::cpu::CoreTicks;
use statsbar_readers::cpu::CpuReader;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// A fresh root per test under Cargo's scratch space, so parallel tests never share files.
fn scratch(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    match fs::remove_dir_all(&root) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => panic!("clearing {}: {error}", root.display()),
    }
    fs::create_dir_all(root.join("proc")).expect("create scratch proc dir");
    root
}

#[test]
fn reads_per_core_ticks_from_the_fixture_tree() {
    let mut reader = CpuReader::open(&fixture("normal")).expect("fixture exists");
    assert_eq!(
        reader.read().expect("well-formed"),
        vec![
            CoreTicks { user: 5208, nice: 12, system: 2084, idle: 90615 },
            CoreTicks { user: 4987, nice: 9, system: 1900, idle: 91070 },
        ]
    );
}

#[test]
fn a_missing_file_fails_to_open_with_not_found() {
    let error = CpuReader::open(&fixture("no-such-root"))
        .err()
        .expect("there is no proc/stat");
    assert_eq!(error.kind(), ErrorKind::NotFound);
}

#[test]
fn a_truncated_file_is_invalid_data() {
    let mut reader = CpuReader::open(&fixture("truncated")).expect("fixture exists");
    assert_eq!(
        reader.read().expect_err("cut mid-line").kind(),
        ErrorKind::InvalidData
    );
}

#[test]
fn every_read_sees_the_file_as_it_is_now() {
    let root = scratch("cpu-reread");
    let stat = root.join("proc/stat");
    fs::write(&stat, "cpu0 1 2 3 4 0 0 0 0 0 0\ncpu1 5 6 7 8 0 0 0 0 0 0\n").expect("first");
    let mut reader = CpuReader::open(&root).expect("just written");
    assert_eq!(reader.read().expect("first sample").len(), 2);
    // Rewritten in place and shorter: a reader that cached the text or its length would still
    // report two cores.
    fs::write(&stat, "cpu0 10 20 30 40\n").expect("second");
    assert_eq!(
        reader.read().expect("second sample"),
        vec![CoreTicks { user: 10, nice: 20, system: 30, idle: 40 }]
    );
}

#[test]
fn a_file_larger_than_the_cap_is_rejected_instead_of_buffered() {
    let root = scratch("cpu-oversized");
    let line = "cpu0 1 2 3 4\n";
    fs::write(root.join("proc/stat"), line.repeat(300 * 1024 / line.len())).expect("write");
    let mut reader = CpuReader::open(&root).expect("just written");
    let error = reader.read().expect_err("over the cap");
    assert_eq!(error.kind(), ErrorKind::InvalidData);
    // The content parses, so only the size cap can have produced this error.
    assert!(error.to_string().contains("256 KiB"), "{error}");
}
```

`crates/readers/tests/memory.rs`:

```rust
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use statsbar_core::memory::MemInfo;
use statsbar_readers::memory::MemoryReader;

const KIB: u64 = 1024;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn reads_meminfo_from_the_fixture_tree() {
    let mut reader = MemoryReader::open(&fixture("normal")).expect("fixture exists");
    assert_eq!(
        reader.read().expect("well-formed"),
        MemInfo {
            total: 16222828 * KIB,
            free: 9012344 * KIB,
            available: 11873320 * KIB,
            s_unreclaim: 98760 * KIB,
            kernel_stack: 14592 * KIB,
            page_tables: 41236 * KIB,
            swap_total: 4194300 * KIB,
            swap_free: 4194300 * KIB,
            zswap: 0,
        }
    );
}

#[test]
fn a_missing_file_fails_to_open_with_not_found() {
    let error = MemoryReader::open(&fixture("no-such-root"))
        .err()
        .expect("there is no proc/meminfo");
    assert_eq!(error.kind(), ErrorKind::NotFound);
}

#[test]
fn a_truncated_file_is_invalid_data() {
    let mut reader = MemoryReader::open(&fixture("truncated")).expect("fixture exists");
    assert_eq!(
        reader.read().expect_err("cut after a key").kind(),
        ErrorKind::InvalidData
    );
}

#[test]
fn bytes_that_are_not_utf8_are_invalid_data() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("memory-not-utf8");
    fs::create_dir_all(root.join("proc")).expect("create scratch proc dir");
    fs::write(root.join("proc/meminfo"), [0xff, 0xfe, b'\n']).expect("write");
    let mut reader = MemoryReader::open(&root).expect("just written");
    assert_eq!(
        reader.read().expect_err("not UTF-8").kind(),
        ErrorKind::InvalidData
    );
}
```

- [x] **Step 3: Run** `cargo test -p statsbar-readers`. Expect FAIL: the package is not a
  workspace member yet. Add it to `members`, create `crates/readers/src/lib.rs` empty, rerun,
  and expect a compile error on `unresolved import statsbar_readers::cpu`.
- [x] **Step 4: Implement.** `crates/readers/src/lib.rs`:

```rust
//! Readers: files on this machine in, core types out.
//!
//! Each reader opens its file once and re-reads it from offset 0 with `pread`, so a sample costs
//! a read instead of an open, a read and a close. Readers take the filesystem root as an argument
//! (`/` in production), which lets the tests run them against fixture trees.

pub mod cpu;
pub mod memory;
mod procfile;
```

`crates/readers/src/procfile.rs`:

```rust
use std::fs::File;
use std::io;
use std::os::unix::fs::FileExt;
use std::path::Path;

/// procfs reports a size of 0, so the length is only known by reading to the end. The cap stops a
/// corrupt file from growing the buffer without bound. /proc/stat on this 8-thread laptop is
/// about 3 KiB and /proc/meminfo about 1.5 KiB.
const MAX_BYTES: usize = 256 * 1024;
const CHUNK: usize = 4096;

/// A file kept open and re-read whole on every call.
pub(crate) struct ProcFile {
    file: File,
    buffer: Vec<u8>,
}

impl ProcFile {
    pub(crate) fn open(path: &Path) -> io::Result<ProcFile> {
        Ok(ProcFile {
            file: File::open(path)?,
            buffer: Vec::new(),
        })
    }

    /// The file's current content. The buffer is reused, so a steady-state read allocates
    /// nothing.
    pub(crate) fn read(&mut self) -> io::Result<&str> {
        self.buffer.clear();
        let mut offset: u64 = 0;
        loop {
            if self.buffer.len() >= MAX_BYTES {
                return Err(invalid_data("file is at least 256 KiB"));
            }
            let start = self.buffer.len();
            self.buffer.resize(start + CHUNK, 0);
            let result = self.file.read_at(&mut self.buffer[start..], offset);
            let read = match result {
                Ok(read) => read,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                    self.buffer.truncate(start);
                    continue;
                }
                Err(error) => {
                    self.buffer.truncate(start);
                    return Err(error);
                }
            };
            self.buffer.truncate(start + read);
            if read == 0 {
                break;
            }
            offset += read as u64;
        }
        std::str::from_utf8(&self.buffer).map_err(|error| invalid_data(&error.to_string()))
    }
}

pub(crate) fn invalid_data(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_owned())
}
```

`crates/readers/src/cpu.rs`:

```rust
//! `/proc/stat` → per-core ticks.

use std::io;
use std::path::Path;

use statsbar_core::cpu::{CoreTicks, parse_proc_stat};

use crate::procfile::{ProcFile, invalid_data};

pub struct CpuReader {
    stat: ProcFile,
}

impl CpuReader {
    pub fn open(root: &Path) -> io::Result<CpuReader> {
        Ok(CpuReader {
            stat: ProcFile::open(&root.join("proc/stat"))?,
        })
    }

    pub fn read(&mut self) -> io::Result<Vec<CoreTicks>> {
        parse_proc_stat(self.stat.read()?)
            .ok_or_else(|| invalid_data("proc/stat has no well-formed per-CPU line"))
    }
}
```

`crates/readers/src/memory.rs`:

```rust
//! `/proc/meminfo` → the fields of the memory bar.

use std::io;
use std::path::Path;

use statsbar_core::memory::{MemInfo, parse_meminfo};

use crate::procfile::{ProcFile, invalid_data};

pub struct MemoryReader {
    meminfo: ProcFile,
}

impl MemoryReader {
    pub fn open(root: &Path) -> io::Result<MemoryReader> {
        Ok(MemoryReader {
            meminfo: ProcFile::open(&root.join("proc/meminfo"))?,
        })
    }

    pub fn read(&mut self) -> io::Result<MemInfo> {
        parse_meminfo(self.meminfo.read()?).ok_or_else(|| {
            invalid_data("proc/meminfo lacks MemTotal or MemAvailable, or has a malformed value")
        })
    }
}
```

- [x] **Step 5: Run** `cargo test -p statsbar-readers` and expect 9 passed.

- [x] **Format and lint.** Run `cargo fmt --all`, then
  `cargo clippy --workspace --all-targets -- -D warnings`, and expect it clean. CI runs
  `cargo fmt --check`, so an unformatted commit fails there.

Ready to commit: `Cargo.toml Cargo.lock crates/readers` — "feat(readers): read /proc/stat and /proc/meminfo with kept-open files"

---

## Task 3 — `statsbard` crate: paths and config

**Files:** create `crates/daemon/Cargo.toml`, `crates/daemon/src/lib.rs`,
`crates/daemon/src/main.rs`, `crates/daemon/src/paths.rs`, `crates/daemon/src/fsutil.rs`,
`crates/daemon/src/config.rs`, `crates/daemon/tests/paths.rs`, `crates/daemon/tests/config.rs`.
Modify the root `Cargo.toml`, setting
`members = ["crates/core", "crates/readers", "crates/daemon"]`.

**Consumes:** nothing from earlier tasks.
**Produces:**
- `paths::config_home(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf>`,
  `paths::config_file(&Path) -> PathBuf`, `paths::autostart_file(&Path) -> PathBuf`
- `fsutil::write_atomically(&Path, &[u8]) -> io::Result<()>`,
  `fsutil::read_to_string_capped(&Path) -> io::Result<String>`, `fsutil::MAX_READ_BYTES`
- `config::{Config, Items, ConfigError, Loaded}`,
  `config::parse(&str) -> Result<Config, ConfigError>`,
  `config::update_text(&str, &Config) -> Result<String, ConfigError>`,
  `config::load(&Path) -> Loaded`, `config::save(&Path, &Config) -> io::Result<()>`,
  `config::PersistError::{NotWritable, Io}`,
  `config::persist(&Path, &Config, writable: bool) -> Result<(), PersistError>`

- [x] **Step 1: Check the dependencies.**
  - Read docs.rs for `toml 1.2.1`: `from_str`, `Table`, `map::Entry::or_insert`, `to_string`,
    `de::Error` and `ser::Error`. Confirm the default features include `parse`, `display` and
    `serde`.
  - Run `cargo info toml@1.2.1` and check that the licence is MIT OR Apache-2.0 and the crate is
    not yanked.
  - If any of this differs, stop and report it.
- [x] **Step 2: Manifest.** Add `"crates/daemon"` to `members` in the root `Cargo.toml`. Create
  `crates/daemon/Cargo.toml` with every dependency the crate needs.
  Later tasks use the rest, and adding them now keeps the lock file change in one commit:

```toml
[package]
name = "statsbard"
version.workspace = true
edition.workspace = true
# zbus 5.19.0 declares rust-version 1.87, above the workspace floor of 1.85.
rust-version = "1.87"
publish.workspace = true

[dependencies]
statsbar-core = { path = "../core" }
statsbar-readers = { path = "../readers" }
async-channel = "=2.5.0"
async-io = "=2.6.0"
futures-lite = "=2.6.1"
ksni = { version = "=0.3.7", default-features = false, features = ["async-io"] }
serde = { version = "=1.0.229", features = ["derive"] }
toml = "=1.2.1"
zbus = { version = "=5.19.0", default-features = false, features = ["async-io"] }

[lints]
workspace = true
```

`crates/daemon/src/lib.rs` starts as:

```rust
//! `statsbard`: the session daemon. It polls, draws the tray items and keeps the config. Pure
//! decisions live in `statsbar-core`; this crate holds the parts that touch the machine and the
//! bus, split so that everything except the event loop is testable without either.

pub mod config;
pub mod fsutil;
pub mod paths;
```

`crates/daemon/src/main.rs` is a temporary stub until Task 7, so the binary target builds:

```rust
fn main() {}
```

- [x] **Step 3: Write the failing tests.** `crates/daemon/tests/paths.rs`:

```rust
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use statsbard::paths::{autostart_file, config_file, config_home};

fn env(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<OsString> {
    move |key| {
        pairs
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| OsString::from(value))
    }
}

#[test]
fn an_absolute_xdg_config_home_wins() {
    let vars = env(&[("XDG_CONFIG_HOME", "/x/conf"), ("HOME", "/home/u")]);
    assert_eq!(config_home(vars), Some(PathBuf::from("/x/conf")));
}

#[test]
fn a_relative_or_empty_xdg_config_home_falls_back_to_home() {
    let fallback = Some(PathBuf::from("/home/u/.config"));
    let empty = env(&[("XDG_CONFIG_HOME", ""), ("HOME", "/home/u")]);
    assert_eq!(config_home(empty), fallback, "empty");
    let relative = env(&[("XDG_CONFIG_HOME", "rel/conf"), ("HOME", "/home/u")]);
    assert_eq!(config_home(relative), fallback, "relative");
}

#[test]
fn without_an_absolute_home_there_is_no_config_home() {
    assert_eq!(config_home(env(&[])), None);
    assert_eq!(config_home(env(&[("HOME", "home/u")])), None);
}

#[test]
fn files_sit_under_the_config_home() {
    let home = Path::new("/home/u/.config");
    assert_eq!(config_file(home), Path::new("/home/u/.config/statsbar/config.toml"));
    assert_eq!(
        autostart_file(home),
        Path::new("/home/u/.config/autostart/statsbar.desktop")
    );
}
```

`crates/daemon/tests/config.rs`:

```rust
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use statsbard::config::{self, Config, Items};

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    match fs::remove_dir_all(&dir) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => panic!("clearing {}: {error}", dir.display()),
    }
    dir
}

fn shown(cpu: bool, memory: bool) -> Config {
    Config { items: Items { cpu, memory } }
}

#[test]
fn an_empty_file_shows_every_item() {
    assert_eq!(config::parse("").expect("empty is valid"), shown(true, true));
}

#[test]
fn a_missing_key_keeps_its_default() {
    let parsed = config::parse("[items]\ncpu = false\n").expect("valid");
    assert_eq!(parsed, shown(false, true));
}

#[test]
fn unknown_keys_and_tables_are_ignored() {
    let parsed = config::parse("[items]\ngpu = true\n[future]\nx = 1\n").expect("valid");
    assert_eq!(parsed, shown(true, true));
}

#[test]
fn a_wrong_type_or_broken_syntax_is_an_error() {
    assert!(config::parse("[items]\ncpu = \"no\"\n").is_err());
    assert!(config::parse("[items\n").is_err());
}

#[test]
fn saving_keeps_keys_this_version_does_not_know() {
    let text = config::update_text("[future]\nx = 1\n\n[items]\ncpu = true\n", &shown(false, true))
        .expect("valid input");
    let table: toml::Table = text.parse().expect("output is TOML");
    assert_eq!(table["future"]["x"].as_integer(), Some(1));
    assert_eq!(table["items"]["cpu"].as_bool(), Some(false));
    assert_eq!(table["items"]["memory"].as_bool(), Some(true));
}

#[test]
fn saving_over_a_non_table_items_key_is_refused() {
    let error = config::update_text("items = 3\n", &shown(true, true)).expect_err("not a table");
    assert!(error.to_string().contains("items"), "{error}");
}

#[test]
fn a_missing_file_loads_defaults_and_may_be_written() {
    let loaded = config::load(&scratch("config-missing").join("config.toml"));
    assert_eq!(loaded.config, shown(true, true));
    assert!(loaded.writable);
    assert_eq!(loaded.warning, None);
}

#[test]
fn an_invalid_file_loads_defaults_and_is_never_written() {
    let dir = scratch("config-invalid");
    fs::create_dir_all(&dir).expect("create dir");
    let path = dir.join("config.toml");
    fs::write(&path, "[items\n").expect("write");
    let loaded = config::load(&path);
    assert_eq!(loaded.config, shown(true, true));
    assert!(!loaded.writable);
    let warning = loaded.warning.expect("a warning");
    assert!(warning.contains(&path.display().to_string()), "{warning}");
}

#[test]
fn a_file_over_the_size_cap_is_refused_and_left_alone() {
    let dir = scratch("config-oversized");
    fs::create_dir_all(&dir).expect("create dir");
    let path = dir.join("config.toml");
    let text = format!("# {}\n[items]\ncpu = false\n", "x".repeat(300 * 1024));
    fs::write(&path, &text).expect("write");
    let loaded = config::load(&path);
    assert_eq!(loaded.config, shown(true, true), "defaults, not the oversized file");
    assert!(!loaded.writable);
    let warning = loaded.warning.expect("a warning");
    assert!(warning.contains("256 KiB"), "{warning}");
    assert_eq!(fs::read_to_string(&path).expect("still there"), text);
}

#[test]
fn save_creates_the_directory_and_round_trips() {
    let path = scratch("config-save").join("statsbar/config.toml");
    config::save(&path, &shown(true, false)).expect("save");
    let loaded = config::load(&path);
    assert_eq!(loaded.config, shown(true, false));
    assert!(loaded.writable);
    let mut temporary = path.clone().into_os_string();
    temporary.push(".tmp");
    assert!(!Path::new(&temporary).exists(), "the temporary file was renamed away");
}

#[test]
fn save_refuses_to_rewrite_a_file_that_does_not_parse() {
    let dir = scratch("config-save-invalid");
    fs::create_dir_all(&dir).expect("create dir");
    let path = dir.join("config.toml");
    fs::write(&path, "[items\n").expect("write");
    assert!(config::save(&path, &shown(false, false)).is_err());
    assert_eq!(fs::read_to_string(&path).expect("still there"), "[items\n");
}

#[test]
fn persist_leaves_a_file_that_was_not_writable_at_load_untouched() {
    let dir = scratch("config-persist-refused");
    fs::create_dir_all(&dir).expect("create dir");
    let path = dir.join("config.toml");
    // Valid now, but the daemon loaded it while it was invalid: the load-time decision holds.
    fs::write(&path, "[items]\ncpu = true\n").expect("write");
    let result = config::persist(&path, &shown(false, true), false);
    assert!(matches!(result, Err(config::PersistError::NotWritable)), "{result:?}");
    assert_eq!(fs::read_to_string(&path).expect("still there"), "[items]\ncpu = true\n");
}

#[test]
fn persist_saves_when_the_file_was_writable_at_load() {
    let path = scratch("config-persist").join("config.toml");
    config::persist(&path, &shown(false, true), true).expect("saved");
    assert_eq!(config::load(&path).config, shown(false, true));
}
```

The test's `toml::Table` comes from the crate's own `[dependencies]` entry, which integration
tests can use. No dev-dependency is added.

- [x] **Step 4: Run** `cargo test -p statsbard`. Expect FAIL with E0583 (`file not found for
  module paths`, and likewise for `config` and `fsutil`), because `lib.rs` declares modules whose
  files do not exist yet.
- [x] **Step 5: Implement.** `crates/daemon/src/paths.rs`:

```rust
//! Where the daemon keeps its files, after the XDG Base Directory spec.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// `$XDG_CONFIG_HOME`, else `$HOME/.config`. The spec says a relative path is invalid and must be
/// ignored; an empty value is relative too. `var` is the environment, passed in so that tests do
/// not touch the process environment.
pub fn config_home(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let absolute = |key: &str| var(key).map(PathBuf::from).filter(|path| path.is_absolute());
    absolute("XDG_CONFIG_HOME").or_else(|| absolute("HOME").map(|home| home.join(".config")))
}

pub fn config_file(config_home: &Path) -> PathBuf {
    config_home.join("statsbar/config.toml")
}

pub fn autostart_file(config_home: &Path) -> PathBuf {
    config_home.join("autostart/statsbar.desktop")
}
```

`crates/daemon/src/fsutil.rs`:

```rust
//! Writes that never leave a half-written file behind.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// Writes `contents` to `<path>.tmp`, syncs it, and renames it over `path`, so anything reading
/// the file — or the daemon after a crash — sees either the old content or the new. A `.tmp` left
/// by a failed write is truncated by the next attempt, so it needs no cleanup.
pub fn write_atomically(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "path has no parent directory")
    })?;
    fs::create_dir_all(parent)?;
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    let mut file = File::create(&temporary)?;
    file.write_all(contents)?;
    file.sync_all()?;
    fs::rename(&temporary, path)
}

/// Far above any config or entry this daemon writes. A larger file was not written by it, and is
/// refused instead of being read into memory whole.
pub const MAX_READ_BYTES: u64 = 256 * 1024;

/// `fs::read_to_string` with the cap above. A missing file is still `NotFound`.
pub fn read_to_string_capped(path: &Path) -> io::Result<String> {
    let mut text = String::new();
    File::open(path)?
        .take(MAX_READ_BYTES + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > MAX_READ_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "file is larger than 256 KiB",
        ));
    }
    Ok(text)
}
```

`crates/daemon/src/config.rs`:

```rust
//! `~/.config/statsbar/config.toml`. Every key is optional, so a file written by an older version
//! keeps working. When the daemon saves, it rewrites only its own keys and keeps the rest, so a
//! newer version's settings survive a downgrade. A save drops comments and re-sorts keys, which the
//! user accepted (requirement 9) over adding `toml_edit`.

use std::fmt;
use std::io;
use std::path::Path;

use serde::Deserialize;

use crate::fsutil::{read_to_string_capped, write_atomically};

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Config {
    pub items: Items,
}

/// Which tray items are shown. The hub is always shown, so it has no key.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Items {
    pub cpu: bool,
    pub memory: bool,
}

impl Default for Items {
    fn default() -> Items {
        Items {
            cpu: true,
            memory: true,
        }
    }
}

/// Boxed, because the `toml` errors carry the offending text and would make every `Result` here
/// large.
#[derive(Debug)]
pub enum ConfigError {
    Parse(Box<toml::de::Error>),
    Render(Box<toml::ser::Error>),
    NotATable(&'static str),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Parse(error) => write!(f, "{error}"),
            ConfigError::Render(error) => write!(f, "cannot write the config: {error}"),
            ConfigError::NotATable(key) => write!(f, "`{key}` must be a table"),
        }
    }
}

impl std::error::Error for ConfigError {}

pub fn parse(text: &str) -> Result<Config, ConfigError> {
    toml::from_str(text).map_err(|error| ConfigError::Parse(Box::new(error)))
}

/// `existing` with `[items]` set from `config`, and every other key left as it was.
pub fn update_text(existing: &str, config: &Config) -> Result<String, ConfigError> {
    let mut table: toml::Table = existing
        .parse()
        .map_err(|error| ConfigError::Parse(Box::new(error)))?;
    let items = table
        .entry("items")
        .or_insert(toml::Value::Table(toml::Table::new()));
    let toml::Value::Table(items) = items else {
        return Err(ConfigError::NotATable("items"));
    };
    items.insert("cpu".to_owned(), toml::Value::Boolean(config.items.cpu));
    items.insert("memory".to_owned(), toml::Value::Boolean(config.items.memory));
    toml::to_string(&table).map_err(|error| ConfigError::Render(Box::new(error)))
}

/// What the daemon starts with.
#[derive(Debug)]
pub struct Loaded {
    pub config: Config,
    /// False when the file exists but could not be read or parsed: the user's file is then left
    /// alone, and switches last only until the daemon exits.
    pub writable: bool,
    pub warning: Option<String>,
}

pub fn load(path: &Path) -> Loaded {
    let refuse = |why: String| Loaded {
        config: Config::default(),
        writable: false,
        warning: Some(format!(
            "{}: {why}; using defaults and leaving the file unchanged",
            path.display()
        )),
    };
    match read_to_string_capped(path) {
        Ok(text) => match parse(&text) {
            Ok(config) => Loaded {
                config,
                writable: true,
                warning: None,
            },
            Err(error) => refuse(format!("not valid: {error}")),
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => Loaded {
            config: Config::default(),
            writable: true,
            warning: None,
        },
        Err(error) => refuse(format!("cannot be read: {error}")),
    }
}

pub fn save(path: &Path, config: &Config) -> io::Result<()> {
    let existing = match read_to_string_capped(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };
    let text = update_text(&existing, config)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    write_atomically(path, text.as_bytes())
}

#[derive(Debug)]
pub enum PersistError {
    /// The file could not be read or parsed at load time, so the user's file is left alone.
    NotWritable,
    Io(io::Error),
}

impl fmt::Display for PersistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PersistError::NotWritable => {
                write!(f, "left unchanged; this switch lasts until statsbard exits")
            }
            PersistError::Io(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for PersistError {}

/// What a menu switch does to the file: save, unless the file was refused when it was loaded.
pub fn persist(path: &Path, config: &Config, writable: bool) -> Result<(), PersistError> {
    if !writable {
        return Err(PersistError::NotWritable);
    }
    save(path, config).map_err(PersistError::Io)
}
```

- [x] **Step 6: Run** `cargo test -p statsbard` and expect 17 passed. Then run `cargo audit` and
  expect no vulnerabilities. Three "unmaintained" warnings are expected: RUSTSEC-2026-0370, 0371
  and 0372 for `zvariant`, `zbus_names` and `zvariant_derive`, which came in with `zbus` in 02a.
  Report any other finding with its ID. Until Task 7, Cargo prints `unused dependency` warnings
  for `ksni` and `statsbar-readers`. They do not fail clippy, and they are expected.

- [x] **Format and lint.** Run `cargo fmt --all`, then
  `cargo clippy --workspace --all-targets -- -D warnings`, and expect it clean. CI runs
  `cargo fmt --check`, so an unformatted commit fails there.

Ready to commit: `Cargo.toml Cargo.lock crates/daemon` — "feat(daemon): add the statsbard crate with config load and atomic save"

---

## Task 4 — Launch at login

**Files:** create `crates/daemon/src/autostart.rs` and `crates/daemon/tests/autostart.rs`. Add
`pub mod autostart;` to `crates/daemon/src/lib.rs`.

**Consumes:** `fsutil::write_atomically`.
**Produces:** `autostart::desktop_entry(exe: &Path) -> io::Result<String>`,
`autostart::is_enabled(&Path) -> io::Result<bool>`,
`autostart::enable(path: &Path, exe: &Path) -> io::Result<()>`,
`autostart::disable(&Path) -> io::Result<()>`, and
`autostart::toggle(&Path, enabled: bool, exe: impl FnOnce() -> io::Result<PathBuf>) -> io::Result<bool>`.

- [x] **Step 1: Write the failing test** `crates/daemon/tests/autostart.rs`:

```rust
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use statsbard::autostart::{desktop_entry, disable, enable, is_enabled, toggle};

fn exec_line(exe: &str) -> String {
    let entry = desktop_entry(Path::new(exe)).expect("valid path");
    entry
        .lines()
        .find(|line| line.starts_with("Exec="))
        .expect("an Exec line")
        .to_owned()
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    match fs::remove_dir_all(&dir) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => panic!("clearing {}: {error}", dir.display()),
    }
    dir
}

#[test]
fn the_entry_starts_the_daemon_and_stays_out_of_the_app_grid() {
    assert_eq!(
        desktop_entry(Path::new("/home/u/.cargo/bin/statsbard")).expect("valid"),
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=StatsBar\n\
         Comment=System figures in the top bar\n\
         Exec=/home/u/.cargo/bin/statsbard\n\
         NoDisplay=true\n\
         X-GNOME-Autostart-enabled=true\n"
    );
}

#[test]
fn a_path_with_a_space_is_quoted() {
    assert_eq!(exec_line("/home/u/my apps/statsbard"), r#"Exec="/home/u/my apps/statsbard""#);
}

#[test]
fn a_percent_sign_is_doubled_so_it_is_not_a_field_code() {
    assert_eq!(exec_line("/opt/100%/statsbard"), "Exec=/opt/100%%/statsbard");
}

#[test]
fn quote_dollar_and_backslash_are_escaped_twice() {
    // The string-value escape runs first and turns `\\` into `\`; the quoting rule then needs
    // `\"`, `\$` and `\\`.
    assert_eq!(exec_line(r#"/opt/a$b"c/statsbard"#), r#"Exec="/opt/a\\$b\\"c/statsbard""#);
    assert_eq!(exec_line(r"/opt/a\b/statsbard"), r#"Exec="/opt/a\\\\b/statsbard""#);
}

#[test]
fn a_relative_path_or_a_control_character_is_refused() {
    for exe in ["statsbard", "/opt/a\nb/statsbard"] {
        let error = desktop_entry(Path::new(exe)).expect_err("refused");
        assert_eq!(error.kind(), ErrorKind::InvalidInput, "{exe:?}");
    }
}

#[test]
fn enable_then_disable_round_trips_and_disable_twice_is_fine() {
    let path = scratch("autostart").join("autostart/statsbar.desktop");
    assert!(!is_enabled(&path).expect("readable"));
    enable(&path, Path::new("/usr/bin/statsbard")).expect("enable");
    assert!(is_enabled(&path).expect("readable"));
    assert_eq!(
        fs::read_to_string(&path).expect("written"),
        desktop_entry(Path::new("/usr/bin/statsbard")).expect("valid")
    );
    disable(&path).expect("disable");
    assert!(!is_enabled(&path).expect("readable"));
    disable(&path).expect("disabling again is not an error");
}

#[test]
fn toggle_flips_the_entry_and_returns_the_new_state() {
    let path = scratch("autostart-toggle").join("autostart/statsbar.desktop");
    let exe = || Ok(PathBuf::from("/usr/bin/statsbard"));
    assert!(toggle(&path, false, exe).expect("enable"));
    assert!(is_enabled(&path).expect("readable"));
    assert!(!toggle(&path, true, exe).expect("disable"));
    assert!(!is_enabled(&path).expect("readable"));
}

#[test]
fn a_failed_toggle_changes_nothing() {
    let path = scratch("autostart-toggle-fails").join("autostart/statsbar.desktop");
    let unknown = || Err(std::io::Error::other("no current_exe"));
    assert!(toggle(&path, false, unknown).is_err());
    let relative = || Ok(PathBuf::from("statsbard"));
    assert_eq!(
        toggle(&path, false, relative).expect_err("relative").kind(),
        ErrorKind::InvalidInput
    );
    assert!(!is_enabled(&path).expect("readable"), "no entry was written");
}
```

- [x] **Step 2: Run** `cargo test -p statsbard --test autostart` and expect FAIL with
  `unresolved import statsbard::autostart`.
- [x] **Step 3: Implement** `crates/daemon/src/autostart.rs`:

```rust
//! Launch at login: the XDG autostart entry. Its presence *is* the setting. The config file does
//! not repeat it, so the two can never disagree.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::fsutil::write_atomically;

/// Characters that force an Exec argument into double quotes (Desktop Entry Specification, "The
/// Exec key").
const RESERVED: &[char] = &[
    ' ', '\t', '\n', '"', '\'', '\\', '>', '<', '~', '|', '&', ';', '$', '*', '?', '#', '(', ')',
    '`',
];

pub fn desktop_entry(exe: &Path) -> io::Result<String> {
    Ok(format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=StatsBar\n\
         Comment=System figures in the top bar\n\
         Exec={}\n\
         NoDisplay=true\n\
         X-GNOME-Autostart-enabled=true\n",
        exec_value(exe)?
    ))
}

fn exec_value(exe: &Path) -> io::Result<String> {
    let refuse = |why: &str| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("cannot start {} at login: {why}", exe.display()),
        )
    };
    if !exe.is_absolute() {
        return Err(refuse("not an absolute path"));
    }
    let text = exe.to_str().ok_or_else(|| refuse("not valid UTF-8"))?;
    if text.chars().any(char::is_control) {
        return Err(refuse("contains a control character"));
    }
    // Inside quotes, `"`, `` ` ``, `$` and `\` take a backslash. The general string escaping runs
    // before the quoting rule and halves every `\\`, so each of those backslashes is written
    // twice. `%` would start a field code. Every character escaped here is also in RESERVED, so
    // the escapes only ever appear inside quotes.
    let quoted = text.contains(RESERVED);
    let mut value = String::with_capacity(text.len() + 2);
    if quoted {
        value.push('"');
    }
    for c in text.chars() {
        match c {
            '%' => value.push_str("%%"),
            '\\' => value.push_str(r"\\\\"),
            '"' | '`' | '$' => {
                value.push_str(r"\\");
                value.push(c);
            }
            _ => value.push(c),
        }
    }
    if quoted {
        value.push('"');
    }
    Ok(value)
}

pub fn is_enabled(path: &Path) -> io::Result<bool> {
    path.try_exists()
}

pub fn enable(path: &Path, exe: &Path) -> io::Result<()> {
    write_atomically(path, desktop_entry(exe)?.as_bytes())
}

pub fn disable(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

/// The *Launch at login* switch: flips the entry and returns the new state. On an error nothing
/// was changed, so the caller keeps showing the old state. `exe` is asked for only when enabling.
pub fn toggle(
    path: &Path,
    enabled: bool,
    exe: impl FnOnce() -> io::Result<PathBuf>,
) -> io::Result<bool> {
    if enabled {
        disable(path)?;
        Ok(false)
    } else {
        enable(path, &exe()?)?;
        Ok(true)
    }
}
```

- [x] **Step 4: Run** `cargo test -p statsbard --test autostart` and expect 8 passed. The entry is
  validated with `desktop-file-validate` on the real file in Task 9, Step 2.7.

- [x] **Format and lint.** Run `cargo fmt --all`, then
  `cargo clippy --workspace --all-targets -- -D warnings`, and expect it clean. CI runs
  `cargo fmt --check`, so an unformatted commit fails there.

Ready to commit: `crates/daemon/src/autostart.rs crates/daemon/src/lib.rs crates/daemon/tests/autostart.rs` — "feat(daemon): toggle launch at login through an XDG autostart entry"

---

## Task 5 — Menu text

**Files:** create `crates/daemon/src/summary.rs` and `crates/daemon/tests/summary.rs`. Add
`pub mod summary;` to `lib.rs`.

**Consumes:** `CpuLoad`, `MemoryBuckets`, `format::{rounded_int, fmt_gb}`.
**Produces:** `summary::percent(f64) -> u8`, `summary::cpu_rows(&CpuLoad) -> Vec<String>`,
`summary::memory_rows(&MemoryBuckets) -> Vec<String>`,
`summary::hub_rows(Option<&CpuLoad>, Option<&MemoryBuckets>, show_cpu: bool, show_memory: bool) -> Vec<String>`,
and `summary::title(name: &str, percent: Option<u8>) -> String`.

- [x] **Step 1: Write the failing test** `crates/daemon/tests/summary.rs`:

```rust
use statsbar_core::cpu::CpuLoad;
use statsbar_core::memory::MemoryBuckets;
use statsbard::summary::{cpu_rows, hub_rows, memory_rows, percent, title};

const GIB: u64 = 1024 * 1024 * 1024;

fn load() -> CpuLoad {
    CpuLoad {
        system_percent: 8.4,
        user_percent: 15.2,
        idle_percent: 76.4,
        per_core_busy: vec![20.0, 27.2, 23.6, 23.6],
    }
}

fn buckets(compressed: u64) -> MemoryBuckets {
    MemoryBuckets {
        total: 16 * GIB,
        app: 6 * GIB,
        kernel: 2 * GIB,
        cached: 4 * GIB,
        free: 4 * GIB,
        swap_used: GIB / 2,
        compressed,
    }
}

#[test]
fn percent_rounds_and_clamps() {
    assert_eq!(percent(23.5), 24);
    assert_eq!(percent(23.4), 23);
    assert_eq!(percent(-3.0), 0);
    assert_eq!(percent(100.4), 100);
    assert_eq!(percent(250.0), 100);
    assert_eq!(percent(f64::NAN), 0);
}

#[test]
fn cpu_rows_show_usage_split_and_cores() {
    assert_eq!(
        cpu_rows(&load()),
        ["Usage 24 %", "User 15 %", "System 8 %", "Idle 76 %", "Cores 4"]
    );
}

#[test]
fn memory_rows_show_used_and_each_bucket() {
    assert_eq!(
        memory_rows(&buckets(0)),
        [
            "Used 8.00 GB of 16.00 GB (50 %)",
            "App 6.00 GB",
            "Kernel 2.00 GB",
            "Cached 4.00 GB",
            "Free 4.00 GB",
            "Swap 0.50 GB",
        ]
    );
}

#[test]
fn compressed_appears_only_when_there_is_some() {
    let rows = memory_rows(&buckets(GIB));
    assert_eq!(rows.last().map(String::as_str), Some("Compressed 1.00 GB"));
}

#[test]
fn hub_rows_cover_shown_metrics_only() {
    let (cpu, memory) = (load(), buckets(0));
    assert_eq!(
        hub_rows(Some(&cpu), Some(&memory), true, true),
        ["CPU 24 %", "Memory 50 % · 8.00 GB / 16.00 GB"]
    );
    assert_eq!(hub_rows(Some(&cpu), Some(&memory), false, true).len(), 1);
    assert!(hub_rows(Some(&cpu), Some(&memory), false, false).is_empty());
}

#[test]
fn a_shown_metric_without_a_value_yet_reads_a_dash() {
    assert_eq!(hub_rows(None, None, true, true), ["CPU —", "Memory —"]);
}

#[test]
fn titles_carry_the_value_when_there_is_one() {
    assert_eq!(title("CPU", Some(7)), "CPU 7 %");
    assert_eq!(title("Memory", None), "Memory");
}
```

- [x] **Step 2: Run** `cargo test -p statsbard --test summary` and expect FAIL with
  `unresolved import statsbard::summary`.
- [x] **Step 3: Implement** `crates/daemon/src/summary.rs`:

```rust
//! The text of the tray menus and titles.

use statsbar_core::cpu::CpuLoad;
use statsbar_core::format::{fmt_gb, rounded_int};
use statsbar_core::memory::MemoryBuckets;

/// A whole percent for a label or a row: rounded, then clamped to what the label can draw.
pub fn percent(value: f64) -> u8 {
    let clamped = rounded_int(value).clamp(0, 100);
    // In 0–100 after the clamp, so the conversion cannot fail.
    u8::try_from(clamped).unwrap_or(100)
}

pub fn cpu_rows(load: &CpuLoad) -> Vec<String> {
    vec![
        format!("Usage {} %", percent(load.usage_percent())),
        format!("User {} %", percent(load.user_percent)),
        format!("System {} %", percent(load.system_percent)),
        format!("Idle {} %", percent(load.idle_percent)),
        format!("Cores {}", load.per_core_busy.len()),
    ]
}

pub fn memory_rows(memory: &MemoryBuckets) -> Vec<String> {
    let mut rows = vec![
        format!(
            "Used {} of {} ({} %)",
            fmt_gb(memory.used()),
            fmt_gb(memory.total),
            percent(memory.usage_percent())
        ),
        format!("App {}", fmt_gb(memory.app)),
        format!("Kernel {}", fmt_gb(memory.kernel)),
        format!("Cached {}", fmt_gb(memory.cached)),
        format!("Free {}", fmt_gb(memory.free)),
        format!("Swap {}", fmt_gb(memory.swap_used)),
    ];
    if memory.compressed > 0 {
        rows.push(format!("Compressed {}", fmt_gb(memory.compressed)));
    }
    rows
}

/// The hub's overview. A hidden metric is not read, so it has no row.
pub fn hub_rows(
    cpu: Option<&CpuLoad>,
    memory: Option<&MemoryBuckets>,
    show_cpu: bool,
    show_memory: bool,
) -> Vec<String> {
    let mut rows = Vec::with_capacity(2);
    if show_cpu {
        rows.push(match cpu {
            Some(load) => format!("CPU {} %", percent(load.usage_percent())),
            None => "CPU —".to_owned(),
        });
    }
    if show_memory {
        rows.push(match memory {
            Some(memory) => format!(
                "Memory {} % · {} / {}",
                percent(memory.usage_percent()),
                fmt_gb(memory.used()),
                fmt_gb(memory.total)
            ),
            None => "Memory —".to_owned(),
        });
    }
    rows
}

/// A tray item's title: what screen readers and the item's tooltip use.
pub fn title(name: &str, percent: Option<u8>) -> String {
    match percent {
        Some(value) => format!("{name} {value} %"),
        None => name.to_owned(),
    }
}
```

- [x] **Step 4: Run** `cargo test -p statsbard --test summary` and expect 7 passed.

- [x] **Format and lint.** Run `cargo fmt --all`, then
  `cargo clippy --workspace --all-targets -- -D warnings`, and expect it clean. CI runs
  `cargo fmt --check`, so an unformatted commit fails there.

Ready to commit: `crates/daemon/src/summary.rs crates/daemon/src/lib.rs crates/daemon/tests/summary.rs` — "feat(daemon): format the CPU, memory and hub menu rows"

---

## Task 6 — App state

**Files:** create `crates/daemon/src/app.rs` and `crates/daemon/tests/app.rs`. Add `pub mod app;` to
`lib.rs`.

**Consumes:** `schedule::{ReaderId, ReaderState, due, next_wake_nanos}`, `CpuLoad::between`,
`MemoryBuckets::from_meminfo`, `summary::percent`, and `config::Config`.
**Produces:** `app::Command::{ToggleCpu, ToggleMemory, ToggleLaunchAtLogin, Quit}` and
`app::App` with:
- `new(Config, launch_at_login: bool)`, the pub fields `config`, `launch_at_login` and
  `power_saver`, and `is_visible(ReaderId) -> bool`
- `due(u64) -> Vec<ReaderId>`, `next_wake_nanos() -> Option<u64>` and `toggle(ReaderId) -> bool`
- `record_cpu(u64, Vec<CoreTicks>) -> Option<u8>`, `record_memory(u64, &MemInfo) -> Option<u8>` and
  `record_error(ReaderId, u64) -> bool`
- `cpu() -> Option<&CpuLoad>` and `memory() -> Option<&MemoryBuckets>`

- [ ] **Step 1: Write the failing test** `crates/daemon/tests/app.rs`:

```rust
use statsbar_core::cpu::CoreTicks;
use statsbar_core::memory::MemInfo;
use statsbar_core::schedule::ReaderId::{Cpu, Memory};
use statsbard::app::App;
use statsbard::config::{Config, Items};

const SECOND: u64 = 1_000_000_000;

fn both_shown() -> App {
    App::new(Config::default(), false)
}

fn ticks(user: u64, idle: u64) -> Vec<CoreTicks> {
    vec![CoreTicks { user, nice: 0, system: 0, idle }]
}

fn memory(total: u64, available: u64) -> MemInfo {
    MemInfo { total, available, ..MemInfo::default() }
}

#[test]
fn a_hidden_item_is_never_read() {
    let app = App::new(Config { items: Items { cpu: false, memory: true } }, false);
    assert!(!app.is_visible(Cpu));
    assert_eq!(app.due(0), vec![Memory]);
}

#[test]
fn hiding_an_item_stops_its_reads_and_updates_the_config() {
    let mut app = both_shown();
    assert!(!app.toggle(Cpu));
    assert!(!app.config.items.cpu);
    assert_eq!(app.due(100 * SECOND), vec![Memory]);
}

#[test]
fn showing_an_item_again_reads_it_at_once_from_a_fresh_sample() {
    let mut app = both_shown();
    app.record_cpu(0, ticks(0, 0));
    app.record_cpu(2 * SECOND, ticks(25, 75));
    app.toggle(Cpu);
    assert!(app.toggle(Cpu));
    assert!(app.config.items.cpu);
    assert_eq!(app.cpu(), None);
    assert!(app.due(2 * SECOND).contains(&Cpu));
    assert_eq!(
        app.record_cpu(2 * SECOND, ticks(50, 150)),
        None,
        "the first sample after showing again has nothing to compare with"
    );
}

#[test]
fn the_cpu_label_needs_two_samples_and_changes_only_with_its_value() {
    let mut app = both_shown();
    assert_eq!(app.record_cpu(0, ticks(0, 0)), None);
    assert_eq!(app.record_cpu(2 * SECOND, ticks(25, 75)), Some(25));
    assert_eq!(app.record_cpu(4 * SECOND, ticks(50, 150)), None);
    assert_eq!(app.record_cpu(6 * SECOND, ticks(100, 150)), Some(100));
}

#[test]
fn a_core_count_change_skips_one_label_update() {
    let mut app = both_shown();
    app.record_cpu(0, ticks(0, 0));
    let two = |n| vec![CoreTicks { user: n, nice: 0, system: 0, idle: n }; 2];
    assert_eq!(app.record_cpu(2 * SECOND, two(10)), None);
    assert_eq!(app.record_cpu(4 * SECOND, two(20)), Some(50));
}

#[test]
fn the_memory_label_changes_only_with_its_value() {
    let mut app = both_shown();
    assert_eq!(app.record_memory(0, &memory(1000, 600)), Some(40));
    assert_eq!(app.record_memory(2 * SECOND, &memory(1000, 600)), None);
    assert_eq!(app.record_memory(4 * SECOND, &memory(1000, 500)), Some(50));
    assert_eq!(app.memory().map(|m| m.used()), Some(500));
}

#[test]
fn a_read_counts_as_a_read_for_the_schedule() {
    let mut app = both_shown();
    app.record_memory(10 * SECOND, &memory(1000, 600));
    assert_eq!(app.due(10 * SECOND), vec![Cpu]);
    assert_eq!(app.due(12 * SECOND), vec![Cpu, Memory]);
}

#[test]
fn a_failed_read_waits_one_interval_and_is_reported_once_per_run() {
    let mut app = both_shown();
    assert!(app.record_error(Cpu, 0), "first failure is reported");
    assert!(!app.due(SECOND).contains(&Cpu), "no busy retry");
    assert!(!app.record_error(Cpu, 2 * SECOND), "second in a row is not");
    app.record_cpu(4 * SECOND, ticks(0, 0));
    assert!(app.record_error(Cpu, 6 * SECOND), "a new run of failures is");
}

#[test]
fn power_saver_moves_the_next_wake_to_five_seconds() {
    let mut app = both_shown();
    app.record_cpu(0, ticks(0, 0));
    app.record_memory(0, &memory(1000, 600));
    assert_eq!(app.next_wake_nanos(), Some(2 * SECOND));
    app.power_saver = true;
    assert_eq!(app.next_wake_nanos(), Some(5 * SECOND));
}
```

- [ ] **Step 2: Run** `cargo test -p statsbard --test app` and expect FAIL with
  `unresolved import statsbard::app`.
- [ ] **Step 3: Implement** `crates/daemon/src/app.rs`:

```rust
//! What the daemon knows and decides, with no I/O: which items are shown, when each reader is
//! due, and whether a new sample changes a label. The event loop does the reading and drawing.

use statsbar_core::cpu::{CoreTicks, CpuLoad};
use statsbar_core::memory::{MemInfo, MemoryBuckets};
use statsbar_core::schedule::{self, ReaderId, ReaderState};

use crate::config::Config;
use crate::summary::percent;

/// A click in the hub's menu, sent from the `ksni` thread to the main loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    ToggleCpu,
    ToggleMemory,
    ToggleLaunchAtLogin,
    Quit,
}

pub struct App {
    pub config: Config,
    pub launch_at_login: bool,
    pub power_saver: bool,
    /// Indexed by `slot`.
    readers: [ReaderState; 2],
    failing: [bool; 2],
    cpu_previous: Option<Vec<CoreTicks>>,
    cpu: Option<CpuLoad>,
    cpu_label: Option<u8>,
    memory: Option<MemoryBuckets>,
    memory_label: Option<u8>,
}

fn slot(id: ReaderId) -> usize {
    match id {
        ReaderId::Cpu => 0,
        ReaderId::Memory => 1,
    }
}

fn fresh(id: ReaderId, visible: bool) -> ReaderState {
    ReaderState {
        id,
        visible,
        page_open: false,
        last_read_nanos: None,
    }
}

/// `Some(value)` when it differs from what the label shows, which is then updated.
fn changed(shown: &mut Option<u8>, value: u8) -> Option<u8> {
    if *shown == Some(value) {
        None
    } else {
        *shown = Some(value);
        Some(value)
    }
}

impl App {
    pub fn new(config: Config, launch_at_login: bool) -> App {
        App {
            readers: [
                fresh(ReaderId::Cpu, config.items.cpu),
                fresh(ReaderId::Memory, config.items.memory),
            ],
            config,
            launch_at_login,
            power_saver: false,
            failing: [false; 2],
            cpu_previous: None,
            cpu: None,
            cpu_label: None,
            memory: None,
            memory_label: None,
        }
    }

    pub fn is_visible(&self, id: ReaderId) -> bool {
        self.readers[slot(id)].visible
    }

    pub fn due(&self, now_nanos: u64) -> Vec<ReaderId> {
        schedule::due(now_nanos, self.power_saver, &self.readers)
    }

    pub fn next_wake_nanos(&self) -> Option<u64> {
        schedule::next_wake_nanos(self.power_saver, &self.readers)
    }

    /// Flips an item's visibility in the config and the schedule, and returns the new value.
    /// Hiding drops the item's samples, so an item shown again starts from a fresh reading and is
    /// due at once, instead of showing a stale value.
    pub fn toggle(&mut self, id: ReaderId) -> bool {
        let visible = !self.is_visible(id);
        match id {
            ReaderId::Cpu => {
                self.config.items.cpu = visible;
                self.cpu_previous = None;
                self.cpu = None;
                self.cpu_label = None;
            }
            ReaderId::Memory => {
                self.config.items.memory = visible;
                self.memory = None;
                self.memory_label = None;
            }
        }
        self.readers[slot(id)] = fresh(id, visible);
        self.failing[slot(id)] = false;
        visible
    }

    /// Takes a `/proc/stat` sample. Returns the new label value when it changed. The first sample,
    /// and one taken after a core went offline, only set the baseline.
    pub fn record_cpu(&mut self, now_nanos: u64, ticks: Vec<CoreTicks>) -> Option<u8> {
        self.mark_read(ReaderId::Cpu, now_nanos);
        let load = self
            .cpu_previous
            .as_deref()
            .and_then(|previous| CpuLoad::between(previous, &ticks));
        self.cpu_previous = Some(ticks);
        let load = load?;
        let value = percent(load.usage_percent());
        self.cpu = Some(load);
        changed(&mut self.cpu_label, value)
    }

    /// Takes a `/proc/meminfo` sample. Returns the new label value when it changed.
    pub fn record_memory(&mut self, now_nanos: u64, info: &MemInfo) -> Option<u8> {
        self.mark_read(ReaderId::Memory, now_nanos);
        let buckets = MemoryBuckets::from_meminfo(info);
        let value = percent(buckets.usage_percent());
        self.memory = Some(buckets);
        changed(&mut self.memory_label, value)
    }

    /// A failed read still counts as a read, so a broken file is retried each interval, not in a
    /// busy loop. Returns true for the first failure in a row, which is the one worth logging.
    pub fn record_error(&mut self, id: ReaderId, now_nanos: u64) -> bool {
        self.readers[slot(id)].last_read_nanos = Some(now_nanos);
        !std::mem::replace(&mut self.failing[slot(id)], true)
    }

    pub fn cpu(&self) -> Option<&CpuLoad> {
        self.cpu.as_ref()
    }

    pub fn memory(&self) -> Option<&MemoryBuckets> {
        self.memory.as_ref()
    }

    fn mark_read(&mut self, id: ReaderId, now_nanos: u64) {
        self.readers[slot(id)].last_read_nanos = Some(now_nanos);
        self.failing[slot(id)] = false;
    }
}
```

- [ ] **Step 4: Run** `cargo test -p statsbard --test app` and expect 9 passed.

- [ ] **Format and lint.** Run `cargo fmt --all`, then
  `cargo clippy --workspace --all-targets -- -D warnings`, and expect it clean. CI runs
  `cargo fmt --check`, so an unformatted commit fails there.

Ready to commit: `crates/daemon/src/app.rs crates/daemon/src/lib.rs crates/daemon/tests/app.rs` — "feat(daemon): track item visibility, samples and label changes"

---

## Task 7 — Tray items and power profile

**Files:** create `crates/daemon/src/tray.rs`, `crates/daemon/src/power.rs`,
`crates/daemon/tests/tray.rs`, `crates/daemon/tests/power.rs`. Add `pub mod power;` and
`pub mod tray;` to `lib.rs`.

**Consumes:** `app::Command`, `label::{render, Metric, SIZE}`, `ksni`, `async-channel`, `zbus`.
**Produces:** `tray::MetricItem { metric, icon: Option<ksni::Icon>, title, rows }`,
`MetricItem::new(Metric)`, `tray::icon(Metric, u8) -> Option<ksni::Icon>`, `tray::FALLBACK_ICON`,
`tray::name(Metric) -> &'static str`,
`tray::Hub { rows, show_cpu, show_memory, launch_at_login, .. }`,
`Hub::new(Sender<Command>, bool, bool, bool)`, `power::is_power_saver(&str) -> bool`, and
`power::ProfileWatch::start(&zbus::Connection) -> zbus::Result<ProfileWatch>` with the field
`changes: zbus::proxy::PropertyStream<'static, String>`.

- [ ] **Step 1: Read docs.rs** for `ksni 0.3.7` (`Tray`, `menu::StandardItem`,
  `menu::CheckmarkItem`, `Category`) and `zbus 5.19.0` (`proxy::Builder` with `destination`,
  `path`, `interface`, `cache_properties` and `build`; `CacheProperties::Yes`;
  `Proxy::receive_property_changed`; `proxy::PropertyStream`; `proxy::PropertyChanged::get`;
  `connection::Builder::system`). The plan review on 2026-11-18 confirmed that the default
  `Lazily` caching makes no bus call in `build`. That is why `start` asks for `Yes`.
- [ ] **Step 2: Write the failing tests.** `crates/daemon/tests/tray.rs`:

```rust
use ksni::menu::MenuItem;
use ksni::Tray;
use statsbar_core::label::{self, Metric};
use statsbard::app::Command;
use statsbard::tray::{self, FALLBACK_ICON, Hub, MetricItem};

fn hub(capacity: usize) -> (Hub, async_channel::Receiver<Command>) {
    let (sender, receiver) = async_channel::bounded(capacity);
    (Hub::new(sender, true, false, true), receiver)
}

fn labels<T>(menu: &[MenuItem<T>]) -> Vec<String> {
    menu.iter()
        .map(|item| match item {
            MenuItem::Standard(item) => item.label.clone(),
            MenuItem::Checkmark(item) => {
                format!("[{}] {}", if item.checked { "x" } else { " " }, item.label)
            }
            MenuItem::Separator => "---".to_owned(),
            other => format!("{other:?}"),
        })
        .collect()
}

fn click(hub: &mut Hub, label: &str) {
    let menu = hub.menu();
    let activate = menu
        .iter()
        .find_map(|item| match item {
            MenuItem::Checkmark(item) if item.label == label => Some(&item.activate),
            MenuItem::Standard(item) if item.label == label => Some(&item.activate),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no menu item {label}"));
    activate(hub);
}

#[test]
fn the_hub_menu_lists_rows_switches_and_quit() {
    let (mut hub, _receiver) = hub(8);
    hub.rows = vec!["CPU 24 %".to_owned()];
    assert_eq!(
        labels(&hub.menu()),
        [
            "CPU 24 %",
            "---",
            "[x] Show CPU",
            "[ ] Show Memory",
            "---",
            "[x] Launch at login",
            "---",
            "Quit",
        ]
    );
}

#[test]
fn hub_rows_are_not_clickable() {
    let (mut hub, _receiver) = hub(8);
    hub.rows = vec!["CPU 24 %".to_owned()];
    match &hub.menu()[0] {
        MenuItem::Standard(item) => assert!(!item.enabled),
        other => panic!("expected a row, got {other:?}"),
    }
}

#[test]
fn each_switch_and_quit_send_their_command() {
    let (mut hub, receiver) = hub(8);
    for (label, expected) in [
        ("Show CPU", Command::ToggleCpu),
        ("Show Memory", Command::ToggleMemory),
        ("Launch at login", Command::ToggleLaunchAtLogin),
        ("Quit", Command::Quit),
    ] {
        click(&mut hub, label);
        assert_eq!(receiver.try_recv(), Ok(expected), "{label}");
    }
}

#[test]
fn a_full_command_queue_drops_the_click_instead_of_blocking_the_tray_thread() {
    let (mut hub, receiver) = hub(1);
    click(&mut hub, "Quit");
    click(&mut hub, "Show CPU");
    assert_eq!(receiver.try_recv(), Ok(Command::Quit));
    assert!(receiver.try_recv().is_err());
}

#[test]
fn a_metric_item_shows_a_themed_icon_until_its_first_value() {
    let mut item = MetricItem::new(Metric::Cpu);
    assert_eq!(item.icon_name(), FALLBACK_ICON);
    assert!(item.icon_pixmap().is_empty());

    item.icon = tray::icon(Metric::Cpu, 42);
    assert_eq!(item.icon_name(), "");
    let pixmaps = item.icon_pixmap();
    assert_eq!(pixmaps.len(), 1);
    assert_eq!((pixmaps[0].width, pixmaps[0].height), (20, 20));
    assert_eq!(
        pixmaps[0].data,
        label::render(Metric::Cpu, 42).expect("in range").to_vec()
    );
}

#[test]
fn an_icon_above_100_is_refused() {
    assert!(tray::icon(Metric::Ram, 101).is_none());
}

#[test]
fn metric_item_ids_are_stable_and_rows_are_not_clickable() {
    assert_eq!(MetricItem::new(Metric::Cpu).id(), "statsbar-cpu");
    let mut memory = MetricItem::new(Metric::Ram);
    assert_eq!(memory.id(), "statsbar-memory");
    memory.rows = vec!["App 6.00 GB".to_owned()];
    match &memory.menu()[0] {
        MenuItem::Standard(item) => {
            assert_eq!(item.label, "App 6.00 GB");
            assert!(!item.enabled);
        }
        other => panic!("expected a row, got {other:?}"),
    }
}
```

`crates/daemon/tests/power.rs`:

```rust
use statsbard::power::is_power_saver;

#[test]
fn only_the_power_saver_profile_counts() {
    assert!(is_power_saver("power-saver"));
    for profile in ["balanced", "performance", "", "Power-Saver"] {
        assert!(!is_power_saver(profile), "{profile:?}");
    }
}
```

- [ ] **Step 3: Run** `cargo test -p statsbard --test tray --test power`. Expect FAIL with
  `unresolved import statsbard::tray` and `statsbard::power`.
- [ ] **Step 4: Implement** `crates/daemon/src/tray.rs`:

```rust
//! The `ksni` tray items: one per metric, plus the hub. `ksni`'s driver thread calls their `Tray`
//! methods and menu callbacks; the main loop changes their fields through `Handle::update`, whose
//! closure runs on the main thread under `ksni`'s lock. Menu clicks become `Command`s for the main
//! loop; nothing here reads or writes files.

use async_channel::Sender;
use ksni::menu::{CheckmarkItem, StandardItem};
use ksni::{Category, Icon, MenuItem};
use statsbar_core::label::{self, Metric};

use crate::app::Command;

/// Shown before a metric's first value, and as the hub's icon.
pub const FALLBACK_ICON: &str = "utilities-system-monitor";

/// The label pixmap for `percent`, or `None` above 100.
pub fn icon(metric: Metric, percent: u8) -> Option<Icon> {
    // SIZE is 20, so the cast cannot truncate.
    let side = label::SIZE as i32;
    label::render(metric, percent).map(|data| Icon {
        width: side,
        height: side,
        data: data.to_vec(),
    })
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
    pub metric: Metric,
    pub icon: Option<Icon>,
    pub title: String,
    pub rows: Vec<String>,
}

impl MetricItem {
    pub fn new(metric: Metric) -> MetricItem {
        MetricItem {
            metric,
            icon: None,
            title: name(metric).to_owned(),
            rows: Vec::new(),
        }
    }
}

/// The name in titles and the hub's rows.
pub fn name(metric: Metric) -> &'static str {
    match metric {
        Metric::Cpu => "CPU",
        Metric::Ram => "Memory",
    }
}

impl ksni::Tray for MetricItem {
    fn id(&self) -> String {
        match self.metric {
            Metric::Cpu => "statsbar-cpu",
            Metric::Ram => "statsbar-memory",
        }
        .to_owned()
    }

    fn category(&self) -> Category {
        Category::Hardware
    }

    fn title(&self) -> String {
        self.title.clone()
    }

    /// Empty once a pixmap exists, so the panel draws the label and not the themed icon.
    fn icon_name(&self) -> String {
        if self.icon.is_some() {
            String::new()
        } else {
            FALLBACK_ICON.to_owned()
        }
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        self.icon.iter().cloned().collect()
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        self.rows.iter().map(|text| row(text)).collect()
    }
}

pub struct Hub {
    pub rows: Vec<String>,
    pub show_cpu: bool,
    pub show_memory: bool,
    pub launch_at_login: bool,
    commands: Sender<Command>,
}

impl Hub {
    pub fn new(
        commands: Sender<Command>,
        show_cpu: bool,
        show_memory: bool,
        launch_at_login: bool,
    ) -> Hub {
        Hub {
            rows: Vec::new(),
            show_cpu,
            show_memory,
            launch_at_login,
            commands,
        }
    }

    /// Never blocks: this runs on the tray thread, which also answers the panel's D-Bus calls. A
    /// click while the queue is full is dropped and logged; the checkmark then still shows the
    /// state the main loop last set.
    fn send(&self, command: Command) {
        if let Err(error) = self.commands.try_send(command) {
            eprintln!("statsbard: menu click dropped: {error}");
        }
    }
}

fn switch(label: &str, checked: bool, command: Command) -> MenuItem<Hub> {
    CheckmarkItem {
        label: label.to_owned(),
        checked,
        activate: Box::new(move |hub: &mut Hub| hub.send(command)),
        ..Default::default()
    }
    .into()
}

impl ksni::Tray for Hub {
    fn id(&self) -> String {
        "statsbar-hub".to_owned()
    }

    fn category(&self) -> Category {
        Category::Hardware
    }

    fn title(&self) -> String {
        "StatsBar".to_owned()
    }

    fn icon_name(&self) -> String {
        FALLBACK_ICON.to_owned()
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let mut menu: Vec<MenuItem<Self>> = self.rows.iter().map(|text| row(text)).collect();
        if !menu.is_empty() {
            menu.push(MenuItem::Separator);
        }
        menu.push(switch("Show CPU", self.show_cpu, Command::ToggleCpu));
        menu.push(switch("Show Memory", self.show_memory, Command::ToggleMemory));
        menu.push(MenuItem::Separator);
        menu.push(switch(
            "Launch at login",
            self.launch_at_login,
            Command::ToggleLaunchAtLogin,
        ));
        menu.push(MenuItem::Separator);
        menu.push(
            StandardItem {
                label: "Quit".to_owned(),
                activate: Box::new(|hub: &mut Hub| hub.send(Command::Quit)),
                ..Default::default()
            }
            .into(),
        );
        menu
    }
}
```

`crates/daemon/src/power.rs`:

```rust
//! The active power profile, from power-profiles-daemon on the system bus. Only `power-saver`
//! changes the daemon's behaviour: it stretches every interval to 5 s.

use zbus::proxy::{Builder, CacheProperties, PropertyStream};

const DESTINATION: &str = "org.freedesktop.UPower.PowerProfiles";
const PATH: &str = "/org/freedesktop/UPower/PowerProfiles";
const INTERFACE: &str = "org.freedesktop.UPower.PowerProfiles";
const PROPERTY: &str = "ActiveProfile";

pub fn is_power_saver(profile: &str) -> bool {
    profile == "power-saver"
}

/// A subscription to `ActiveProfile`. The stream yields the current value first, then each
/// change. It holds its own clone of the proxy, which keeps the property cache alive.
pub struct ProfileWatch {
    pub changes: PropertyStream<'static, String>,
}

impl ProfileWatch {
    /// Needs the connection's executor to be ticking, because the proxy subscribes to
    /// `PropertiesChanged` while it is built.
    ///
    /// `CacheProperties::Yes` fetches the properties before returning. With the default
    /// (`Lazily`), a missing power-profiles-daemon fails silently in the background and the stream
    /// simply never yields; fetching up front turns that into an error the caller logs.
    pub async fn start(connection: &zbus::Connection) -> zbus::Result<ProfileWatch> {
        let proxy: zbus::Proxy<'static> = Builder::new(connection)
            .destination(DESTINATION)?
            .path(PATH)?
            .interface(INTERFACE)?
            .cache_properties(CacheProperties::Yes)
            .build()
            .await?;
        let changes = proxy.receive_property_changed::<String>(PROPERTY).await;
        Ok(ProfileWatch { changes })
    }
}
```

- [ ] **Step 5: Run** `cargo test -p statsbard --test tray --test power` and expect 8 passed. Run
  `cargo clippy -p statsbard --all-targets -- -D warnings`. If clippy flags `label::SIZE as i32`
  (`cast_possible_wrap` is pedantic, not in `clippy::all`), replace it with
  `i32::try_from(label::SIZE)` and return `None` on failure. Do not add an `allow`.

- [ ] **Format and lint.** Run `cargo fmt --all`, then
  `cargo clippy --workspace --all-targets -- -D warnings`, and expect it clean. CI runs
  `cargo fmt --check`, so an unformatted commit fails there.

Ready to commit: `crates/daemon/src/tray.rs crates/daemon/src/power.rs crates/daemon/src/lib.rs crates/daemon/tests/tray.rs crates/daemon/tests/power.rs` — "feat(daemon): add the metric and hub tray items and the power profile watch"

---

## Task 8 — Event loop and `main`

**Files:** create `crates/daemon/src/run.rs`. Replace `crates/daemon/src/main.rs`. Add `pub mod run;`
to `lib.rs`.

**Consumes:** every module above.
**Produces:** `run::run() -> Result<(), run::BoxError>` (async) and the `statsbard` binary.

This task is glue: D-Bus, the panel and real files. It has no unit test. Each decision in it was
made, and tested, in Tasks 1–7. The loop is proven by Task 9's end-to-end run. Report this as the
coverage gap for `run.rs`.

- [ ] **Step 1: Implement** `crates/daemon/src/run.rs`:

```rust
//! The daemon's event loop, on the main thread. One timer serves every reader. Menu clicks
//! arrive from the hub, and power-profile changes from the system bus. The single `zbus`
//! connection the daemon owns is ticked here, not on a thread of its own (spike S2).

use std::error::Error;
use std::future::Future;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use async_channel::{Receiver, Sender};
use futures_lite::{StreamExt, future};
use ksni::{Handle, TrayMethods};
use statsbar_core::cpu::CoreTicks;
use statsbar_core::label::Metric;
use statsbar_core::memory::MemInfo;
use statsbar_core::schedule::ReaderId;
use statsbar_readers::cpu::CpuReader;
use statsbar_readers::memory::MemoryReader;

use crate::app::{App, Command};
use crate::power::{ProfileWatch, is_power_saver};
use crate::tray::{self, Hub, MetricItem};
use crate::{autostart, config, paths, summary};

pub type BoxError = Box<dyn Error>;

/// Connecting and registering an item only has to tell "slow" from "hung"; the value S2 ran with.
const SETUP_TIMEOUT: Duration = Duration::from_secs(5);
/// How long one tray update or property read may hold up the loop; the value S2 ran with.
const CALL_TIMEOUT: Duration = Duration::from_secs(1);
/// Clicks come at human speed; a few slots absorb a double click while the loop is reading.
const COMMAND_CAPACITY: usize = 8;

/// Runs `call`, giving up after `timeout`, so a hung bus or watcher is reported by name instead
/// of stalling the loop.
async fn bounded<T>(
    what: &str,
    timeout: Duration,
    call: impl Future<Output = T>,
) -> Result<T, BoxError> {
    future::or(async { Ok(call.await) }, async {
        async_io::Timer::after(timeout).await;
        Err(format!("{what} timed out after {timeout:?}").into())
    })
    .await
}

fn elapsed_nanos(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

pub async fn run() -> Result<(), BoxError> {
    let config_home = paths::config_home(|key| std::env::var_os(key))
        .ok_or("neither XDG_CONFIG_HOME nor HOME is an absolute path")?;
    let config_file = paths::config_file(&config_home);
    let autostart_file = paths::autostart_file(&config_home);

    let loaded = config::load(&config_file);
    if let Some(warning) = &loaded.warning {
        eprintln!("statsbard: {warning}");
    }
    let launch_at_login = autostart::is_enabled(&autostart_file).unwrap_or_else(|error| {
        eprintln!("statsbard: {}: {error}", autostart_file.display());
        false
    });

    let (sender, receiver) = async_channel::bounded(COMMAND_CAPACITY);
    let app = App::new(loaded.config, launch_at_login);
    let mut daemon = Daemon::start(
        app,
        sender,
        Files {
            root: PathBuf::from("/"),
            config: config_file,
            config_writable: loaded.writable,
            autostart: autostart_file,
        },
    )
    .await?;

    let system = connect_system_bus().await;

    let work = daemon.run(receiver, system.as_ref());
    let drive = async {
        match &system {
            Some(connection) => loop {
                connection.executor().tick().await;
            },
            None => future::pending().await,
        }
    };
    future::or(work, drive).await
}

/// The daemon's one `zbus` connection, used for power profiles. Any failure, even a malformed
/// `DBUS_SYSTEM_BUS_ADDRESS`, leaves power saver off (requirement 6) instead of stopping the daemon.
async fn connect_system_bus() -> Option<zbus::Connection> {
    let builder = match zbus::connection::Builder::system() {
        Ok(builder) => builder.internal_executor(false),
        Err(error) => {
            eprintln!("statsbard: no system bus address, power saver stays off: {error}");
            return None;
        }
    };
    match bounded("system bus connect", SETUP_TIMEOUT, builder.build()).await {
        Ok(Ok(connection)) => Some(connection),
        Ok(Err(error)) => {
            eprintln!("statsbard: no system bus, power saver stays off: {error}");
            None
        }
        Err(error) => {
            eprintln!("statsbard: {error}; power saver stays off");
            None
        }
    }
}

struct Files {
    root: PathBuf,
    config: PathBuf,
    config_writable: bool,
    autostart: PathBuf,
}

enum Wake {
    Timer,
    Command(Command),
    CommandsClosed,
    Profile(Result<String, BoxError>),
    ProfileEnded,
}

struct Daemon {
    app: App,
    files: Files,
    hub: Handle<Hub>,
    hub_shown: Option<(Vec<String>, bool, bool, bool)>,
    cpu_item: Option<Handle<MetricItem>>,
    memory_item: Option<Handle<MetricItem>>,
    cpu_reader: Option<CpuReader>,
    memory_reader: Option<MemoryReader>,
}

impl Daemon {
    async fn start(app: App, commands: Sender<Command>, files: Files) -> Result<Daemon, BoxError> {
        let hub = Hub::new(
            commands,
            app.config.items.cpu,
            app.config.items.memory,
            app.launch_at_login,
        );
        let hub = bounded("hub item spawn", SETUP_TIMEOUT, hub.spawn()).await??;
        let mut daemon = Daemon {
            app,
            files,
            hub,
            hub_shown: None,
            cpu_item: None,
            memory_item: None,
            cpu_reader: None,
            memory_reader: None,
        };
        for id in [ReaderId::Cpu, ReaderId::Memory] {
            if daemon.app.is_visible(id) {
                daemon.show_item(id).await;
            }
        }
        Ok(daemon)
    }

    async fn run(
        &mut self,
        commands: Receiver<Command>,
        system: Option<&zbus::Connection>,
    ) -> Result<(), BoxError> {
        let mut profiles = match system {
            Some(connection) => {
                match bounded(
                    "power profile watch",
                    SETUP_TIMEOUT,
                    ProfileWatch::start(connection),
                )
                .await
                {
                    Ok(Ok(watch)) => Some(watch),
                    Ok(Err(error)) => {
                        eprintln!("statsbard: power profiles unavailable: {error}");
                        None
                    }
                    Err(error) => {
                        eprintln!("statsbard: {error}");
                        None
                    }
                }
            }
            None => None,
        };

        let start = Instant::now();
        loop {
            let now = elapsed_nanos(start);
            for id in self.app.due(now) {
                self.sample(id, now).await;
            }
            self.refresh_hub().await;

            let wake_at = self.app.next_wake_nanos();
            let timer = async {
                match wake_at {
                    Some(at) => {
                        let wait = at.saturating_sub(elapsed_nanos(start));
                        async_io::Timer::after(Duration::from_nanos(wait)).await;
                        Wake::Timer
                    }
                    None => future::pending().await,
                }
            };
            let command = async {
                match commands.recv().await {
                    Ok(command) => Wake::Command(command),
                    Err(_) => Wake::CommandsClosed,
                }
            };
            let profile = async {
                let Some(watch) = profiles.as_mut() else {
                    return future::pending().await;
                };
                match watch.changes.next().await {
                    Some(change) => match bounded("ActiveProfile read", CALL_TIMEOUT, change.get()).await {
                        Ok(Ok(value)) => Wake::Profile(Ok(value)),
                        Ok(Err(error)) => Wake::Profile(Err(error.into())),
                        Err(error) => Wake::Profile(Err(error)),
                    },
                    None => Wake::ProfileEnded,
                }
            };

            // Bound first, so the three futures (and their borrows of `profiles`) are gone
            // before the arms below may reset it.
            let wake = future::or(timer, future::or(command, profile)).await;
            match wake {
                Wake::Timer => {}
                Wake::Command(Command::Quit) | Wake::CommandsClosed => break,
                Wake::Command(command) => self.handle(command).await,
                Wake::Profile(Ok(profile)) => self.app.power_saver = is_power_saver(&profile),
                Wake::Profile(Err(error)) => {
                    eprintln!("statsbard: reading the power profile failed: {error}");
                }
                Wake::ProfileEnded => {
                    eprintln!("statsbard: power profile updates ended; keeping the last value");
                    profiles = None;
                }
            }
        }
        self.shutdown().await;
        Ok(())
    }

    async fn sample(&mut self, id: ReaderId, now: u64) {
        match id {
            ReaderId::Cpu => match self.read_cpu() {
                Ok(ticks) => {
                    let label = self.app.record_cpu(now, ticks);
                    self.refresh_item(Metric::Cpu, label).await;
                }
                Err(error) => self.read_failed(id, now, &error),
            },
            ReaderId::Memory => match self.read_memory() {
                Ok(info) => {
                    let label = self.app.record_memory(now, &info);
                    self.refresh_item(Metric::Ram, label).await;
                }
                Err(error) => self.read_failed(id, now, &error),
            },
        }
    }

    fn read_cpu(&mut self) -> io::Result<Vec<CoreTicks>> {
        let reader = match self.cpu_reader.take() {
            Some(reader) => reader,
            None => CpuReader::open(&self.files.root)?,
        };
        self.cpu_reader.insert(reader).read()
    }

    fn read_memory(&mut self) -> io::Result<MemInfo> {
        let reader = match self.memory_reader.take() {
            Some(reader) => reader,
            None => MemoryReader::open(&self.files.root)?,
        };
        self.memory_reader.insert(reader).read()
    }

    /// Drops the reader, so the next attempt reopens the file, and logs the first failure in a row.
    fn read_failed(&mut self, id: ReaderId, now: u64, error: &io::Error) {
        match id {
            ReaderId::Cpu => self.cpu_reader = None,
            ReaderId::Memory => self.memory_reader = None,
        }
        if self.app.record_error(id, now) {
            eprintln!("statsbard: reading {id:?} failed: {error}");
        }
    }

    async fn refresh_item(&self, metric: Metric, label: Option<u8>) {
        let (handle, rows) = match metric {
            Metric::Cpu => (&self.cpu_item, self.app.cpu().map(summary::cpu_rows)),
            Metric::Ram => (&self.memory_item, self.app.memory().map(summary::memory_rows)),
        };
        let Some(handle) = handle else { return };
        let rows = rows.unwrap_or_default();
        let redraw = label.map(|value| {
            (
                tray::icon(metric, value),
                summary::title(tray::name(metric), Some(value)),
            )
        });
        // A missed update is not retried. The rows are resent on the next sample, but the icon and
        // title only when the percent changes again, because `App` has already recorded the new
        // value. A 1 s stall on `ksni`'s lock is itself worth investigating, and the log shows it.
        update(handle, "metric item update", move |item: &mut MetricItem| {
            item.rows = rows;
            if let Some((icon, title)) = redraw {
                item.icon = icon;
                item.title = title;
            }
        })
        .await;
    }

    async fn refresh_hub(&mut self) {
        let config = &self.app.config;
        let shown = (
            summary::hub_rows(
                self.app.cpu(),
                self.app.memory(),
                config.items.cpu,
                config.items.memory,
            ),
            config.items.cpu,
            config.items.memory,
            self.app.launch_at_login,
        );
        if self.hub_shown.as_ref() == Some(&shown) {
            return;
        }
        let (rows, show_cpu, show_memory, launch_at_login) = shown.clone();
        let delivered = update(&self.hub, "hub update", move |hub: &mut Hub| {
            hub.rows = rows;
            hub.show_cpu = show_cpu;
            hub.show_memory = show_memory;
            hub.launch_at_login = launch_at_login;
        })
        .await;
        // Only a delivered update counts as shown, so a timed-out one is retried next pass.
        if delivered {
            self.hub_shown = Some(shown);
        }
    }

    async fn handle(&mut self, command: Command) {
        match command {
            Command::ToggleCpu => self.toggle(ReaderId::Cpu).await,
            Command::ToggleMemory => self.toggle(ReaderId::Memory).await,
            Command::ToggleLaunchAtLogin => self.toggle_launch_at_login(),
            Command::Quit => {}
        }
    }

    async fn toggle(&mut self, id: ReaderId) {
        if self.app.toggle(id) {
            self.show_item(id).await;
        } else {
            self.hide_item(id).await;
        }
        let files = &self.files;
        if let Err(error) = config::persist(&files.config, &self.app.config, files.config_writable)
        {
            eprintln!("statsbard: {}: {error}", files.config.display());
        }
    }

    async fn show_item(&mut self, id: ReaderId) {
        let metric = match id {
            ReaderId::Cpu => Metric::Cpu,
            ReaderId::Memory => Metric::Ram,
        };
        match bounded("metric item spawn", SETUP_TIMEOUT, MetricItem::new(metric).spawn()).await {
            Ok(Ok(handle)) => match id {
                ReaderId::Cpu => self.cpu_item = Some(handle),
                ReaderId::Memory => self.memory_item = Some(handle),
            },
            Ok(Err(error)) => eprintln!("statsbard: showing {id:?} failed: {error}"),
            Err(error) => eprintln!("statsbard: {error}"),
        }
    }

    /// Removes the item and closes its file, so a hidden metric costs nothing.
    async fn hide_item(&mut self, id: ReaderId) {
        let handle = match id {
            ReaderId::Cpu => {
                self.cpu_reader = None;
                self.cpu_item.take()
            }
            ReaderId::Memory => {
                self.memory_reader = None;
                self.memory_item.take()
            }
        };
        if let Some(handle) = handle {
            shut_down(&handle, "metric item shutdown").await;
        }
    }

    fn toggle_launch_at_login(&mut self) {
        let path = &self.files.autostart;
        match autostart::toggle(path, self.app.launch_at_login, std::env::current_exe) {
            Ok(enabled) => self.app.launch_at_login = enabled,
            Err(error) => eprintln!("statsbard: {}: {error}", path.display()),
        }
    }

    async fn shutdown(&mut self) {
        for handle in [self.cpu_item.take(), self.memory_item.take()].into_iter().flatten() {
            shut_down(&handle, "metric item shutdown").await;
        }
        shut_down(&self.hub, "hub shutdown").await;
    }
}

/// Applies `change` to the item and lets `ksni` emit the D-Bus signals. Returns whether the
/// update was delivered.
async fn update<T: ksni::Tray>(
    handle: &Handle<T>,
    what: &str,
    change: impl FnOnce(&mut T),
) -> bool {
    match bounded(what, CALL_TIMEOUT, handle.update(change)).await {
        Ok(Some(())) => true,
        Ok(None) => {
            eprintln!("statsbard: {what}: the tray service has shut down");
            false
        }
        Err(error) => {
            eprintln!("statsbard: {error}");
            false
        }
    }
}

async fn shut_down<T>(handle: &Handle<T>, what: &str) {
    if let Err(error) = bounded(what, CALL_TIMEOUT, handle.shutdown()).await {
        eprintln!("statsbard: {error}");
    }
}
```

`crates/daemon/src/main.rs`:

```rust
use std::process::ExitCode;

fn main() -> ExitCode {
    match async_io::block_on(statsbard::run::run()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("statsbard: {error}");
            ExitCode::FAILURE
        }
    }
}
```

- [ ] **Step 2: Build.** Run `cargo build -p statsbard` and `./scripts/check.sh`, and expect both
  green. Compiler errors from crate API drift are fixed against docs.rs, as the risks table says.
  Record each such fix in this task as a note.
- [ ] **Step 3: Check the dependency invariants.**
  - `cargo tree -p statsbard -e normal | grep -iE 'gtk|tokio'` → expect no output. Exit code 1
    from `grep` is the pass.
  - `cargo tree -p statsbard -d -e normal | grep -E '^zbus '` → expect no output: only one `zbus`.

- [ ] **Format and lint.** Run `cargo fmt --all`, then
  `cargo clippy --workspace --all-targets -- -D warnings`, and expect it clean. CI runs
  `cargo fmt --check`, so an unformatted commit fails there.

Ready to commit: `crates/daemon/src/run.rs crates/daemon/src/main.rs crates/daemon/src/lib.rs` — "feat(daemon): run the tray, scheduler and power profile watch on one loop"

---

## Task 9 — Verification and docs

**Files:** `README.md`, `docs/plans/2026-11-16-rust-statsbar-roadmap.md`, a new
`docs/spikes/2026-11-18-02b-daemon-e2e.md`, and this plan's checkboxes.

- [ ] **Step 1: Full suite.** Run `./scripts/check.sh` and expect it green. Quote the test totals.
  Then run `cargo build --release -p statsbard` and `cargo audit`.
- [ ] **Step 2: End-to-end run, with the user present.** The real panel is the only way to see the
  items. Use a scratch config home, so the user's `~/.config` is untouched. Record each result in
  a new `docs/spikes/2026-11-18-02b-daemon-e2e.md`:
  1. `export XDG_CONFIG_HOME="$(mktemp -d)"`, then
     `strace -f -tt -y -e trace=pread64,openat,close -o "$XDG_CONFIG_HOME/trace.log" target/release/statsbard &`
     and note the PID. Starting the daemon under `strace` avoids needing ptrace rights on a
     running process, since `ptrace_scope` is 1 here.
  2. **Panel:** the user confirms that the CPU and Memory labels, and the hub icon, appear. Take a
     screenshot the same way as the [02b font probe](2026-11-18-rust-statsbar-02b-font-probe.md),
     through the Desktop Screenshot portal, and keep it outside Git.
  3. **Values:** the CPU label is within one sample of `top -bn2 -d2 | grep '^%Cpu' | tail -1`.
     The memory label equals `(total − available) / total` from `free -b`, rounded.
  4. **Hide:** the user unticks *Show CPU*. Expect the CPU item to vanish. In `trace.log`, the
     `pread64` lines on `</proc/stat>` stop after the click: count them over the next 10 s and
     expect 0. A `close` on that fd shows the file was closed. Expect
     `cat $XDG_CONFIG_HOME/statsbar/config.toml` to show `cpu = false`. The roadmap asked for
     `strace -c`, but `-c` counts calls without paths; `-y` shows which file each read hit.
  5. **Persist:** quit from the hub, restart with the same `XDG_CONFIG_HOME`, and expect no CPU
     item. Re-tick *Show CPU* and expect the item back within 2 s.
  6. **Threads:** after 30 s, run `spikes/tray/measure.sh <pid>`. The `strace` parent is not the
     daemon, so use `pgrep -n statsbard`. Expect only the threads the invariant lists: the main
     thread and the `ksni` driver (both named after the process), `async-io`, and at most
     `blocking-N` pool threads. Record Rss and Pss.
  7. **Launch at login:** tick it. Expect
     `desktop-file-validate $XDG_CONFIG_HOME/autostart/statsbar.desktop` to print nothing, and its
     `Exec` to be the release binary path. Untick it and expect the file to be gone. A real
     login is not tested; report that gap.
  8. **Power saver:** ask the user to switch the profile to Power Saver in GNOME's quick
     settings. Changing it is a system setting, which the user does themselves. Expect the
     `pread64` timestamps on `</proc/meminfo>` to go from 2 s to 5 s apart. Ask the user to
     switch back.
  9. **Quit:** from the hub. Expect the items to vanish and the process to exit with code 0.
- [ ] **Step 3: Docs.**
  - README: change `Status:` to
    `Status: CPU and Memory in the tray (statsbard); run with \`cargo run --release -p statsbard\`.`
    Add a Plans bullet for this plan and one for the e2e results document.
  - Roadmap Plan 02b: add `*(written: \`docs/plans/2026-11-18-rust-statsbar-02b-daemon.md\`)*`
    to its heading, and fill its Evidence with the results from Step 2. Replace its Core
    additions and Readers signatures with the forms built here:
    `schedule::due(now_nanos, power_saver, &[ReaderState])`, `CpuReader::open(root)?.read()` and
    `MemoryReader::open(root)?.read()`. Move `config::parse` from the core list to the daemon.
  - Tick this plan's boxes.
- [ ] **Step 4: Final gate** (global rules):
  1. Read `git diff main...HEAD`, with untracked files included. Expect nothing unrelated, no
     debug output, no TODOs, no commented-out code and no secrets.
  2. Rerun `./scripts/check.sh`.
  3. Self-review with one concrete reason per item: requirements 1–9; concurrency (the main
     loop writes `Hub` and `MetricItem` fields only inside `Handle::update`, under `ksni`'s
     lock, and the driver thread reads them under the same lock); error paths; no
     hot-path allocation beyond the menu rows; timeouts; naming.
  4. Run `code-review-strict` on the branch, then fix or report each finding.
  5. If anything changed, repeat from 1.
- [ ] **Step 5: Report the gaps.**
  - `run.rs` is covered only by the end-to-end run.
  - Launch at login was not tested across a real login.
  - The thread count is one 30 s sample, and Plan 11 measures longer.
  - `page_open` is always false until Plan 04.

Ready to commit: `README.md docs/plans/2026-11-16-rust-statsbar-roadmap.md docs/plans/2026-11-18-rust-statsbar-02b-daemon.md docs/spikes/2026-11-18-02b-daemon-e2e.md` — "docs: record the daemon's end-to-end run"
