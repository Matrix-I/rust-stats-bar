# Plan 04b — Panel Skeleton (CPU and Memory pages) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: subagent-driven-strict or executing-plans-strict. Tasks are `### Task N: <name>`; steps use `- [ ]`.

**Goal:** `statsbar-panel`, a GTK4/libadwaita window with the CPU and Memory pages, fed by the
daemon's socket from [Plan 04a](2026-11-22-rust-statsbar-04a-ipc.md); "Open details…" at the top
of the CPU and Memory tray menus starts it. [Roadmap](2026-11-16-rust-statsbar-roadmap.md) Plan 04b.

**Architecture:**

- New crate `crates/panel` (lib + bin `statsbar-panel`). Pure, tested modules: `link` (socket
  client: handshake, requests, a reader thread feeding the GTK loop through `async-channel`),
  `view` (events → figures, fractions and tones), `quit` (pidfd, start-time check, SIGTERM), `cli`
  (`--page`). GTK only in `widgets` and `window`.
- One `adw::Application` (id `io.github.MatrixI.StatsBar`, `HANDLES_COMMAND_LINE`): a second
  launch hands its `--page` to the running window over D-Bus and exits. One window, a CPU | Memory
  `AdwViewSwitcher`; switching sends `Subscribe`, the footer's Refresh sends `Refresh`, closing the
  window ends the process and the socket.
- Daemon: `Command::OpenPage(Page)` from a new "Open details…" entry; `launch::Launcher` spawns the
  panel beside `statsbard`, reaps it each loop turn, and caps running children at 4. The CPU and
  Memory menus reserve their rows from the start as hidden entries, so `ksni` never changes the
  layout (requirement 17). No thread is added to the daemon; it still links no GTK.

**Tech Stack:** Rust edition 2024; `statsbar-panel` floor 1.92 (`gtk4` 0.11.5 declares it);
`rustc 1.100.0`. Approved (requirement 3): `gtk4` 0.11.5 (MIT, feature `v4_12`), `libadwaita`
0.9.2 (MIT, feature `v1_5`); reused at their locked versions: `serde` 1.0.229, `serde_json`
1.0.154, `rustix` 1.1.6 (`process`), `async-channel` 2.5.0. System: GTK 4.22.4 and libadwaita
1.9.1 on the laptop (installed 2026-11-24); CI installs `libgtk-4-dev libadwaita-1-dev`.

**Branch:** `feature/panel-skeleton`, cut from `origin/main` at 4c96984.

**How the code was checked:** every block below was built and its tests run in a scratch worktree
of 4c96984 on 2026-11-24 (`./scripts/check.sh`: fmt, clippy, 382 tests passed, purity gate passed; `cargo audit` with the GTK tree reports only the 3 known unmaintained `zbus` advisories). The window ran against a real daemon
through GTK's broadway backend: both pages filled, the switcher re-subscribed, a second launch
with `--page memory` reused the window, ✕ opened the confirmation (cancelled), and stopping the
daemon showed the error page with Retry and a disabled Refresh.

**How to read the code blocks:** *Create* gives a whole file. *Apply to* gives a diff against the
tree as the previous task left it; apply it with `git apply` from the repository root.

## Confirmed Requirements

Plan 04a's decisions 2, 3, 5, 8, 11, 13, 15, 16 and 17 (its Confirmed Requirements, user,
2026-11-22) carry into this plan. Asked and answered on 2026-11-24:

1. **One window with a CPU | Memory switcher**, opened on the page whose "Open details…" was
   clicked, so a hidden item's page stays reachable (04a decision 12). *(user)*
2. **Quit is a visible ✕** on each row the daemon marks quittable, then a confirmation dialog, then
   SIGTERM. A failure shows a toast. *(user)*
3. **Daemon lost while open:** an error page with Retry; no automatic reconnect. *(user)*
4. **Memory menu:** the Compressed row keeps a hidden place, shown only while zswap holds pages;
   the menu looks as today. *(user)*

## Decision Log

Not posted: the project has no ticket. PR description lines:

| # | Decision (user, 2026-11-24) | PR description line |
|---|---|---|
| 1 | One window with a switcher, so a hidden item's page is reachable | One panel window with CPU and Memory tabs, opened on the clicked page. |
| 2 | A visible ✕, confirmed, then SIGTERM | Quit a listed process with ✕, after a confirmation. |
| 3 | Error page with Retry, no auto-reconnect | When StatsBar is down the panel says so and offers Retry. |
| 4 | Hidden Compressed place | Menus keep their ids from the start; Compressed appears in place. |

## Scope Map

**Invariant:** the panel shows only what the daemon sends for the page it watches, signals only
the process a row named (pid and start time) after the user confirmed, and a menu click on "Open
details…" reaches the main loop unless the command queue is full, which logs and drops the click as
the hub's menu already does.

| Path | In or out | Test |
|---|---|---|
| Socket path, handshake, version mismatch, refusal, oversized or bad line, EOF | in (Task 1) | `tests/link.rs` |
| Page figures, "—" for missing data, tone thresholds, Compressed row, quittable rows | in (Task 2) | `tests/view.rs` |
| SIGTERM to the listed process; pid reused, process gone, out-of-range pid | in (Task 3) | `tests/quit.rs` |
| EPERM (another user's process) | in (Task 3): the errno mapping only, since a test cannot own another uid's process | `kernel_refusals_map_to_their_reasons` |
| `MemoryBuckets::used()` on buckets the daemon sent | in (Task 2): saturates instead of overflowing (04a follow-up) | `used_saturates_on_buckets_that_do_not_add_up` |
| Process names with Unicode format characters (bidi overrides) in rows and in "Quit {name}?" | out (follow-up: 04a follow-ups, `display_name`; the dialog still targets the pid and start time, so a spoofed name cannot redirect the signal) | — |
| `--page` parsing | in (Task 4) | `tests/cli.rs` |
| Window: switcher subscribe, Refresh, Retry, error page, ✕ dialog, toasts, single instance | in (Task 5) | e2e only (Task 7): no GTK display in `cargo test` |
| "Open details…" on CPU and Memory; none on Battery, Fan, Network | in (Task 6) | `open_details_sends_its_page_and_other_items_have_none` |
| "Open details…" with the command queue full | in (Task 6): logged and dropped, never blocks the tray thread | `a_full_command_queue_drops_open_details_too` |
| Menu layout stable while rows fill in (requirement 17) | in (Task 6) | `cpu_and_memory_menus_open_their_page_above_reserved_rows`, `the_cpu_menu_keeps_its_ids_while_its_rows_fill_in` (D-Bus) |
| Launching, reaping, cap, missing binary, renderer default | in (Task 6) | `tests/launch.rs` |
| Hub menu | none: unchanged | existing `tests/tray.rs` |
| Protocol (`statsbar_core::ipc`) | none: used as 04a left it | — |
| `.desktop` entry, install path | out: Plan 11 | — |
| Battery and Network pages | out: Plan 05 | — |

## Impact Map

| Changed symbol | Caller | Before → after | Intended |
|---|---|---|---|
| `tray::MetricItem::new(reader)` → `new(reader, commands)` | `run::Daemon::show_item`, `tests/tray.rs` | — → passes the command sender | yes (Task 6) |
| `MetricItem::menu` for CPU and Memory | GNOME's AppIndicator | rows only → "Open details…", separator, reserved rows | yes (Task 6): `metric_item_ids_are_stable_and_rows_are_not_clickable` now checks the Fan menu's first row, since requirement 11 puts "Open details…" first on CPU and Memory |
| `app::Command` gains `OpenPage(Page)` | `run::Daemon::handle`, `Hub` | — | yes (Task 6) |
| `Daemon::start` gains `launcher` | `run::run` | — | yes (Task 6) |
| `Hub::send` | hub switches and Quit | same behaviour, via the shared `tray::send` | unchanged: `a_full_command_queue_drops_the_click_instead_of_blocking_the_tray_thread` |
| `run::Daemon` keeps a command sender for new metric items | `Daemon::run`'s `Wake::CommandsClosed` exit | unchanged: the channel still closes once the hub and every item have dropped their senders | yes (Task 6): `Daemon` holds a `WeakSender`. No test: `Daemon` is private and closing needs the session bus to go; checked by reading `show_item`'s `upgrade()` |
| `run::run` calls `std::env::current_exe()` | daemon start | new failure at start if `/proc/self/exe` cannot be read | intended (Task 6): it is always readable for a running Linux process; failing loudly beats a menu entry that never works |
| `MemoryBuckets::used()` saturates | `summary::{cpu_rows, memory_rows}` (daemon), `usage_percent`, `view::memory_view` | same value for every bucket `from_meminfo` builds (its sum never exceeds `total`); no panic on peer data | intended (Task 2): `used_saturates_on_buckets_that_do_not_add_up`; existing `tests/memory.rs` and `tests/summary.rs` pin the old values |

## Existing Data & States

| Case | Behaviour | Test |
|---|---|---|
| Panel already open, "Open details…" again | The running window comes forward on that page | Task 7 e2e (second launch) |
| Daemon not running at launch | Error page with the reason and Retry | Task 1 `no_daemon_reads_as_not_running`; Task 7 e2e |
| Daemon from before 04a (no socket) | Same as not running | as above |
| Panel on its error page, StatsBar started again, "Open details…" clicked | The second launch hands its page to the running window, which connects because it has no link: a user action, so decision 3 (no automatic reconnect) holds | Task 7 Step 6 |
| Four panels already starting | Fifth click logged and dropped | `exited_panels_are_reaped_and_free_their_place` |
| No `statsbar-panel` beside `statsbard` | Click logged, nothing counted | `a_missing_panel_is_an_error_and_takes_no_place` |
| User set `GSK_RENDERER` | Inherited, not replaced | `a_renderer_the_user_chose_is_inherited_not_replaced` |
| Config, autostart file | Unchanged formats | — |

## Global Constraints

Plan 04a's list holds unchanged (test-first, determinism, no commits unless asked, English,
timeouts, no sensitive data in logs, stay in scope, invariants). In addition:

- `statsbard` never links GTK: check `cargo tree -p statsbard -e normal | grep -c gtk` is `0`.
- No process name, pid or other personal data in repository docs or logs: the e2e record names
  processes generically.
- The ✕ path signals real processes: e2e quits only a stand-in process the executor started.
- One test expectation changes: `metric_item_ids_are_stable_and_rows_are_not_clickable` checks
  the Fan menu instead of Memory, because requirement 11 adds "Open details…" above Memory's rows.
- Verification entry point: `./scripts/check.sh`.

## Technical Defaults

| Choice | Reason |
|---|---|
| Blocking socket I/O on one reader thread + `async-channel` into `glib::spawn_future_local`; connect via `gio::spawn_blocking` | The handshake can wait up to its 2 s timeout without freezing the window; the client is testable without GTK |
| pidfd opened before the start-time check | A pid reused after the check cannot receive the signal |
| Widgets built once, updated in place (six fixed process rows) | A once-a-second rebuild would swallow a ✕ press |
| GNOME HIG palette, one table for Cairo and CSS | Same colour for a ring and its legend dot |
| Window 340 × 860 | Fits the CPU page of 8 threads; GTK sizes the window before the first event |
| `gtk4` `v4_12`, `libadwaita` `v1_5` | `WidgetExt::color`, `load_from_string`, `AlertDialog`; Ubuntu 24.04 has 4.14 / 1.5 |
| Panel binary found beside `statsbard` | No PATH lookup; Plan 11 installs both together |
| `GSK_RENDERER` set by the launcher to the measured lighter renderer unless inherited | Roadmap: measure Cairo against GL and keep the lighter (Task 7) |
| Memory subtitle is `MemTotal` ("14.93 GB total") | Linux has no installed-size figure without root |
| Colour thresholds ported from upstream (roadmap decision 6): usage ring green below 70 %, orange from 70 %, red from 85 % (`Sources/View/MemorySection.swift:130`); temperature ring green below 75 °C, orange from 75 °C, red from 90 °C on a 0–100 °C scale (`Sources/View/CPUSection.swift:110,131`); per-core temperatures dim below 75 °C (`CPUSection.swift:253`) | Same look as upstream; `Thermal` (04a requirement 6) is separate and unchanged |

## Risks & Rollback

| Risk | Mitigation | Revert |
|---|---|---|
| ✕ kills the wrong process | Confirmation; pidfd + start time; kernel refuses other users | Revert Task 3/5 |
| GTK code untested by `cargo test` | Pure logic is in tested modules; Task 7 runs the window on the desktop | — |
| CI runner's GTK older than the features | Features capped at GTK 4.12 / libadwaita 1.5 | Pin `runs-on: ubuntu-24.04` |
| Renderer choice | Measured in Task 7; `GSK_RENDERER` overrides it | Change `launch::RENDERER` |
| Persisted data | None added | — |
| A process name with bidi overrides reads differently in the dialog | Out of scope (04a follow-up); the signal still targets pid + start time | Fix `display_name` in a later plan |
| GTK's dependency tree brings advisories | `cargo audit` after Tasks 1 and 5 and in the final gate; at planning time only the 3 known `zbus` unmaintained warnings | Pin or replace the crate the advisory names |

---

### Task 1: Panel crate and its socket client

**Type:** feature · **Test level:** integration (real Unix sockets, a stand-in daemon thread)

**Files:** Create `crates/panel/Cargo.toml`, `crates/panel/src/lib.rs`, `crates/panel/src/link.rs`, `crates/panel/tests/link.rs`. Modify `Cargo.toml` (members), `.github/workflows/ci.yml`, `Cargo.lock`.

**Interfaces:** Produces `link::{socket_path(Option<&OsStr>) -> Result<PathBuf, LinkError>, Link::connect(&Path, Duration) -> Result<Link, LinkError>, Link::send(&self, &Request) -> io::Result<()>, Link.events: Receiver<Incoming>, Incoming::{Event(Box<Event>), Closed(Closed)}, Closed::{Eof, Refused(ErrorReason), BadLine, Io(ErrorKind)} (+ Display), MAX_EVENT_BYTES}`.

- [x] **Step 0: Branch and setup.** `git switch main && git pull --ff-only && git switch -c feature/panel-skeleton`. Confirm `pkg-config --modversion gtk4 libadwaita-1` prints `4.22.4` and `1.9.1`. Copy this plan file into the branch.

- [x] **Step 1: Write the failing test.** (Under `crates/panel/tests/`; it fails before the crate exists.)

*Create* `crates/panel/tests/link.rs`:

```rust
//! The panel's client against a stand-in daemon on a socket in a temporary directory.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use statsbar_core::ipc::{ErrorReason, Event, MemoryPage, Page, Request};
use statsbar_core::memory::MemoryBuckets;
use statsbar_panel::link::{Closed, Incoming, Link, LinkError, MAX_EVENT_BYTES, socket_path};

/// Only fails a hung test; nothing waits for it to pass.
const LIMIT: Duration = Duration::from_secs(10);
const HELLO: &str = "{\"protocol\":1}\n";

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        let path =
            std::env::temp_dir().join(format!("statsbar-link-{}-{name}", std::process::id()));
        // Left over only by a killed earlier run.
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).unwrap();
        Dir(path)
    }

    fn socket(&self) -> PathBuf {
        self.0.join("statsbar.sock")
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        // Best effort: a leftover directory in the temp dir harms nothing.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone, Copy)]
enum Then {
    /// Close the connection after the script.
    Close,
    /// Keep reading until the panel closes, and return what it sent.
    Listen,
}

/// A daemon that writes `script` once the client's first line has come.
fn daemon(dir: &Dir, script: String, then: Then) -> thread::JoinHandle<Vec<String>> {
    let listener = UnixListener::bind(dir.socket()).unwrap();
    thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(LIMIT)).unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut first = String::new();
        reader.read_line(&mut first).unwrap();
        let mut received = vec![first];
        (&stream).write_all(script.as_bytes()).unwrap();
        if let Then::Listen = then {
            received.extend(reader.lines().map(Result::unwrap));
        }
        received
    })
}

fn next(link: &Link) -> Incoming {
    let (sender, receiver) = mpsc::channel();
    let events = link.events.clone();
    thread::spawn(move || {
        // The receiver lives until the test ends.
        let _ = sender.send(events.recv_blocking());
    });
    receiver.recv_timeout(LIMIT).unwrap().unwrap()
}

fn memory_line(app: u64) -> String {
    let event = Event::Memory(MemoryPage {
        buckets: MemoryBuckets {
            total: 100,
            app,
            ..MemoryBuckets::default()
        },
        pressure: None,
        processes: None,
    });
    format!("{}\n", serde_json::to_string(&event).unwrap())
}

#[test]
fn the_socket_lives_in_the_runtime_directory() {
    assert_eq!(
        socket_path(Some("/run/user/1000".as_ref())).unwrap(),
        PathBuf::from("/run/user/1000/statsbar.sock")
    );
    assert!(matches!(socket_path(None), Err(LinkError::NoRuntimeDir)));
    assert!(matches!(
        socket_path(Some("relative".as_ref())),
        Err(LinkError::NoRuntimeDir)
    ));
}

#[test]
fn no_daemon_reads_as_not_running() {
    let dir = Dir::new("absent");
    assert!(matches!(
        Link::connect(&dir.socket(), LIMIT),
        Err(LinkError::NotRunning(_))
    ));
}

#[test]
fn hellos_are_exchanged_and_requests_follow_until_the_link_is_dropped() {
    let dir = Dir::new("hello");
    let daemon = daemon(&dir, HELLO.to_owned(), Then::Listen);
    let link = Link::connect(&dir.socket(), LIMIT).unwrap();
    link.send(&Request::Subscribe { page: Page::Cpu }).unwrap();
    link.send(&Request::Refresh).unwrap();
    // The stand-in returns only at end of file, so this also proves the drop closes the socket.
    drop(link);
    assert_eq!(
        daemon.join().unwrap(),
        [
            HELLO,
            "{\"type\":\"subscribe\",\"page\":\"cpu\"}",
            "{\"type\":\"refresh\"}"
        ]
    );
}

#[test]
fn events_arrive_in_order_and_the_end_is_reported() {
    let dir = Dir::new("events");
    let script = format!("{HELLO}{}{}", memory_line(40), memory_line(41));
    let _daemon = daemon(&dir, script, Then::Close);
    let link = Link::connect(&dir.socket(), LIMIT).unwrap();
    for app in [40, 41] {
        let Incoming::Event(event) = next(&link) else {
            panic!("expected an event");
        };
        let Event::Memory(page) = *event else {
            panic!("expected a memory page");
        };
        assert_eq!(page.buckets.app, app);
    }
    assert_eq!(next(&link), Incoming::Closed(Closed::Eof));
}

#[test]
fn an_error_line_closes_with_its_reason() {
    let dir = Dir::new("busy");
    let script = format!("{HELLO}{{\"type\":\"error\",\"reason\":\"busy\",\"message\":\"x\"}}\n");
    let _daemon = daemon(&dir, script, Then::Close);
    let link = Link::connect(&dir.socket(), LIMIT).unwrap();
    assert_eq!(
        next(&link),
        Incoming::Closed(Closed::Refused(ErrorReason::Busy))
    );
}

#[test]
fn another_protocol_version_is_refused() {
    let dir = Dir::new("version");
    let _daemon = daemon(&dir, "{\"protocol\":2}\n".to_owned(), Then::Close);
    assert!(matches!(
        Link::connect(&dir.socket(), LIMIT),
        Err(LinkError::VersionMismatch { daemon: 2 })
    ));
}

#[test]
fn a_first_line_that_is_not_a_hello_is_a_failed_handshake() {
    let dir = Dir::new("not-hello");
    let _daemon = daemon(&dir, memory_line(1), Then::Close);
    assert!(matches!(
        Link::connect(&dir.socket(), LIMIT),
        Err(LinkError::Handshake(_))
    ));
}

#[test]
fn a_daemon_that_never_says_hello_times_out() {
    let dir = Dir::new("silent");
    let daemon = daemon(&dir, String::new(), Then::Listen);
    assert!(matches!(
        Link::connect(&dir.socket(), Duration::from_millis(50)),
        Err(LinkError::Handshake(_))
    ));
    // The failed connect closed its socket, so the stand-in saw only the hello.
    assert_eq!(daemon.join().unwrap(), [HELLO]);
}

#[test]
fn a_line_over_the_limit_or_not_an_event_ends_the_link() {
    let oversized = "a".repeat(usize::try_from(MAX_EVENT_BYTES).unwrap() + 1);
    for (name, bad) in [
        ("long", oversized),
        ("garbage", "{\"type\":\"nope\"}".to_owned()),
    ] {
        let dir = Dir::new(name);
        let script = format!("{HELLO}{}{bad}\n{}", memory_line(7), memory_line(8));
        let _daemon = daemon(&dir, script, Then::Close);
        let link = Link::connect(&dir.socket(), LIMIT).unwrap();
        assert!(
            matches!(next(&link), Incoming::Event(event) if matches!(*event, Event::Memory(_))),
            "{name}"
        );
        assert_eq!(next(&link), Incoming::Closed(Closed::BadLine), "{name}");
    }
}

#[test]
fn every_end_says_what_happened() {
    let cases = [
        (Closed::Eof, "StatsBar quit or restarted"),
        (
            Closed::Refused(ErrorReason::Busy),
            "StatsBar already serves as many panels as it can",
        ),
        (
            Closed::Refused(ErrorReason::VersionMismatch),
            "StatsBar speaks another protocol version; restart StatsBar",
        ),
        (
            Closed::Refused(ErrorReason::BadRequest),
            "StatsBar closed the connection",
        ),
        (
            Closed::BadLine,
            "StatsBar sent something this panel cannot read",
        ),
        (
            Closed::Io(std::io::ErrorKind::BrokenPipe),
            "the connection to StatsBar failed: broken pipe",
        ),
    ];
    for (closed, text) in cases {
        assert_eq!(closed.to_string(), text);
    }
}
```

- [x] **Step 2: Run it, expect FAIL.** `cargo test -p statsbar-panel --test link` fails: the crate does not exist (`package ID specification statsbar-panel did not match`).

- [x] **Step 3: Implement.**

*Apply to* the workspace and CI:

```diff
diff --git a/.github/workflows/ci.yml b/.github/workflows/ci.yml
index 1eea2cf..047605d 100644
--- a/.github/workflows/ci.yml
+++ b/.github/workflows/ci.yml
@@ -45,5 +45,8 @@ jobs:
       # tests/ayatana_label.rs runs a private session bus; it fails, never skips, without one.
       - run: command -v dbus-daemon || (sudo apt-get update && sudo apt-get install -y --no-install-recommends dbus-daemon)
 
+      # statsbar-panel links GTK 4 and libadwaita; their headers and .pc files come from these.
+      - run: sudo apt-get update && sudo apt-get install -y --no-install-recommends libgtk-4-dev libadwaita-1-dev
+
       # The single verification entry point: fmt, clippy, tests and the core purity gate.
       - run: ./scripts/check.sh
diff --git a/Cargo.toml b/Cargo.toml
index d303390..7511111 100644
--- a/Cargo.toml
+++ b/Cargo.toml
@@ -1,6 +1,6 @@
 [workspace]
 resolver = "3"
-members = ["crates/core", "crates/readers", "crates/daemon"]
+members = ["crates/core", "crates/readers", "crates/daemon", "crates/panel"]
 # The patched ksni (vendor/ksni/PATCHES.md) is a path dependency, not a member, so upstream's
 # code keeps upstream's formatting and is not held to this workspace's lints.
 exclude = ["vendor/ksni"]
```

*Create* `crates/panel/Cargo.toml`:

```toml
[package]
name = "statsbar-panel"
version.workspace = true
edition.workspace = true
# gtk4 0.11.5 declares rust-version 1.92.
rust-version = "1.92"
publish.workspace = true

[dependencies]
statsbar-core = { path = "../core" }
async-channel = "=2.5.0"
serde = "=1.0.229"
serde_json = "=1.0.154"

[lints]
workspace = true
```

*Create* `crates/panel/src/lib.rs`:

```rust
//! `statsbar-panel`: the CPU and Memory pages, in a window the daemon opens on demand.

pub mod link;
```

*Create* `crates/panel/src/link.rs`:

```rust
//! The panel's end of the daemon's socket: the handshake, the requests, and a reader thread
//! that hands each event to the GTK main loop.
//!
//! Blocking I/O on its own thread keeps the main loop free, and a read timeout bounds the
//! handshake. The thread ends when the daemon closes the socket or the [`Link`] is dropped.

use std::fmt;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use async_channel::{Receiver, Sender};
use statsbar_core::ipc::{ErrorReason, Event, Hello, PROTOCOL_VERSION, Request};

const SOCKET: &str = "statsbar.sock";
/// A page with six processes is under 2 KiB; this only bounds a broken peer.
pub const MAX_EVENT_BYTES: u64 = 256 * 1024;
/// Events not yet taken by the main loop. A full queue stops the reader, and the daemon then
/// drops a client that has stopped reading.
const QUEUED_EVENTS: usize = 8;

/// What the reader thread passes to the main loop.
#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    /// Boxed: a page is two hundred bytes, the other variant two.
    Event(Box<Event>),
    /// The last message: the connection is gone.
    Closed(Closed),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Closed {
    /// The daemon closed the socket or quit.
    Eof,
    /// The daemon's last line said why.
    Refused(ErrorReason),
    /// A line that is not an event, or one over [`MAX_EVENT_BYTES`].
    BadLine,
    Io(io::ErrorKind),
}

impl fmt::Display for Closed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Closed::Eof => write!(f, "StatsBar quit or restarted"),
            Closed::Refused(ErrorReason::Busy) => {
                write!(f, "StatsBar already serves as many panels as it can")
            }
            Closed::Refused(ErrorReason::VersionMismatch) => {
                write!(
                    f,
                    "StatsBar speaks another protocol version; restart StatsBar"
                )
            }
            Closed::Refused(ErrorReason::BadRequest | ErrorReason::HandshakeTimeout) => {
                write!(f, "StatsBar closed the connection")
            }
            Closed::BadLine => write!(f, "StatsBar sent something this panel cannot read"),
            Closed::Io(kind) => write!(f, "the connection to StatsBar failed: {kind}"),
        }
    }
}

#[derive(Debug)]
pub enum LinkError {
    NoRuntimeDir,
    /// No daemon listens: not started, or quit.
    NotRunning(io::Error),
    /// The daemon speaks another protocol version.
    VersionMismatch {
        daemon: u32,
    },
    /// No hello in time, or one that is not a hello.
    Handshake(String),
    Io(io::Error),
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinkError::NoRuntimeDir => write!(f, "XDG_RUNTIME_DIR is not an absolute path"),
            LinkError::NotRunning(error) => write!(f, "StatsBar is not running ({error})"),
            LinkError::VersionMismatch { daemon } => write!(
                f,
                "StatsBar speaks protocol {daemon}, this panel {PROTOCOL_VERSION}; restart StatsBar"
            ),
            LinkError::Handshake(why) => write!(f, "StatsBar did not answer: {why}"),
            LinkError::Io(error) => write!(f, "talking to StatsBar failed: {error}"),
        }
    }
}

impl std::error::Error for LinkError {}

/// `$XDG_RUNTIME_DIR/statsbar.sock`, where the daemon binds it. `runtime_dir` is the variable's
/// value, passed in so tests need not change the environment.
pub fn socket_path(runtime_dir: Option<&std::ffi::OsStr>) -> Result<PathBuf, LinkError> {
    let dir = Path::new(runtime_dir.ok_or(LinkError::NoRuntimeDir)?);
    if dir.is_absolute() {
        Ok(dir.join(SOCKET))
    } else {
        Err(LinkError::NoRuntimeDir)
    }
}

pub struct Link {
    stream: UnixStream,
    pub events: Receiver<Incoming>,
}

impl Link {
    /// Connects and exchanges hellos. `timeout` bounds each read and write of the handshake,
    /// and every later request.
    pub fn connect(path: &Path, timeout: Duration) -> Result<Link, LinkError> {
        let stream = UnixStream::connect(path).map_err(LinkError::NotRunning)?;
        stream
            .set_read_timeout(Some(timeout))
            .map_err(LinkError::Io)?;
        stream
            .set_write_timeout(Some(timeout))
            .map_err(LinkError::Io)?;
        let mut reader = BufReader::new(stream.try_clone().map_err(LinkError::Io)?);
        // The daemon writes its hello on accept; ours goes first all the same, so neither waits.
        (&stream)
            .write_all(&line(&Hello::current()).map_err(LinkError::Io)?)
            .map_err(LinkError::Io)?;
        let first = read_line(&mut reader).map_err(|error| match error.kind() {
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => {
                LinkError::Handshake("no hello within the timeout".to_owned())
            }
            _ => LinkError::Io(error),
        })?;
        let Some(first) = first else {
            return Err(LinkError::Handshake("closed before its hello".to_owned()));
        };
        let hello: Hello = serde_json::from_slice(&first)
            .map_err(|_| LinkError::Handshake("its first line is not a hello".to_owned()))?;
        if hello.protocol != PROTOCOL_VERSION {
            return Err(LinkError::VersionMismatch {
                daemon: hello.protocol,
            });
        }
        // Events come one a second; the reader waits for them without a deadline.
        stream.set_read_timeout(None).map_err(LinkError::Io)?;
        let (sender, events) = async_channel::bounded(QUEUED_EVENTS);
        thread::Builder::new()
            .name("statsbar-link".to_owned())
            .spawn(move || pump(reader, &sender))
            .map_err(LinkError::Io)?;
        Ok(Link { stream, events })
    }

    pub fn send(&self, request: &Request) -> io::Result<()> {
        (&self.stream).write_all(&line(request)?)
    }
}

impl Drop for Link {
    /// Wakes the reader thread with an end of file, so it exits instead of waiting.
    fn drop(&mut self) {
        // Fails only when the socket is already closed, which is the goal.
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }
}

fn line<T: serde::Serialize>(value: &T) -> io::Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// One line without its newline; `None` at a clean end of file. A line over
/// [`MAX_EVENT_BYTES`] is an `InvalidData` error.
fn read_line(reader: &mut BufReader<UnixStream>) -> io::Result<Option<Vec<u8>>> {
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(MAX_EVENT_BYTES + 1)
        .read_until(b'\n', &mut bytes)?;
    match bytes.pop() {
        None => Ok(None),
        Some(b'\n') => Ok(Some(bytes)),
        // No newline: the line was cut at the limit, or the peer closed in mid-line.
        Some(_) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unterminated line",
        )),
    }
}

fn pump(mut reader: BufReader<UnixStream>, sender: &Sender<Incoming>) {
    let closed = loop {
        let bytes = match read_line(&mut reader) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => break Closed::Eof,
            Err(error) if error.kind() == io::ErrorKind::InvalidData => break Closed::BadLine,
            Err(error) => break Closed::Io(error.kind()),
        };
        match serde_json::from_slice::<Event>(&bytes) {
            Ok(Event::Error { reason, .. }) => break Closed::Refused(reason),
            Ok(event) => {
                if sender
                    .send_blocking(Incoming::Event(Box::new(event)))
                    .is_err()
                {
                    // The panel has gone; nobody is left to tell.
                    return;
                }
            }
            Err(_) => break Closed::BadLine,
        }
    };
    // The receiver may already be gone, and then nobody needs to know.
    let _ = sender.send_blocking(Incoming::Closed(closed));
}
```

Then `cargo build -p statsbar-panel` to update `Cargo.lock` (offline after the first fetch).

- [x] **Step 4: Run, expect PASS.** `cargo test -p statsbar-panel --test link`, then `cargo clippy -p statsbar-panel --all-targets -- -D warnings`, `cargo fmt --check` and `cargo audit` (new lock entries; expect only the 3 known `zbus` unmaintained warnings).

- [x] **Step 5: Checkpoint.** Ready to commit: `Cargo.toml`, `Cargo.lock`, `.github/workflows/ci.yml`, `crates/panel/{Cargo.toml,src/lib.rs,src/link.rs,tests/link.rs}`, this plan — "feat(panel): add the panel crate and its socket client"

### Task 2: Page figures without GTK

**Type:** feature · **Test level:** unit

**Files:** Create `crates/panel/src/view.rs`, `crates/panel/tests/view.rs`. Modify `crates/panel/src/lib.rs`, `crates/core/src/memory.rs:102-106` (`MemoryBuckets::used`), `crates/core/tests/memory.rs` (end).

**Interfaces:** Consumes `ipc::{CpuPage, MemoryPage, ProcessInfo}`, `format::{fmt_gb, fmt_process_memory, fmt_uptime, rounded_int}`. Changes `statsbar_core::memory::MemoryBuckets::used` (saturating). Produces `view::{cpu_view, memory_view, CpuView, MemoryView, Ring, Segment, Row, Core, ProcessRow, QuitTarget, Tone, MISSING, temperature_tone, thermal_tone, pressure_tone, usage_tone}`.

- [x] **Step 1: Write the failing test.**

*Create* `crates/panel/tests/view.rs`:

```rust
//! The pages' figures and colours, from daemon events.

use statsbar_core::cpu::CpuLoad;
use statsbar_core::ipc::{CpuPage, CpuProcess, MemoryPage, MemoryProcess, ProcessInfo};
use statsbar_core::memory::{MemoryBuckets, Pressure};
use statsbar_core::system::Thermal;
use statsbar_panel::view::{
    MISSING, QuitTarget, Segment, Tone, cpu_view, memory_view, pressure_tone, temperature_tone,
    thermal_tone, usage_tone,
};

const GIB: u64 = 1 << 30;

fn process(pid: u32, name: &str, can_quit: bool) -> ProcessInfo {
    ProcessInfo {
        pid,
        start_ticks: u64::from(pid) * 10,
        name: name.to_owned(),
        can_quit,
    }
}

fn first_cpu_page() -> CpuPage {
    CpuPage {
        model: None,
        load: None,
        temperature_c: None,
        thermal: None,
        core_temperatures_c: Vec::new(),
        frequency_mhz: None,
        load_per_cpu: None,
        uptime_seconds: None,
        power_saver: false,
        processes: None,
    }
}

fn full_cpu_page() -> CpuPage {
    CpuPage {
        model: Some("Intel(R) Core(TM) i5-8250U CPU @ 1.60GHz".to_owned()),
        load: Some(CpuLoad {
            system_percent: 12.5,
            user_percent: 25.0,
            idle_percent: 62.5,
            per_core_busy: vec![50.0, 12.6],
        }),
        temperature_c: Some(57.0),
        thermal: Some(Thermal::Nominal),
        core_temperatures_c: vec![Some(56.0), Some(80.4)],
        frequency_mhz: Some(2345.6),
        load_per_cpu: Some([0.254, 0.3, 1.0]),
        uptime_seconds: Some(3_700.0),
        power_saver: false,
        processes: Some(vec![
            CpuProcess {
                process: process(42, "firefox", true),
                cpu_percent: 105.04,
            },
            CpuProcess {
                process: process(1, "systemd", false),
                cpu_percent: 0.0,
            },
        ]),
    }
}

#[test]
fn a_full_cpu_page_reads_as_upstream() {
    let view = cpu_view(&full_cpu_page());
    assert_eq!(
        view.subtitle.as_deref(),
        Some("Intel(R) Core(TM) i5-8250U CPU @ 1.60GHz")
    );
    let [usage, temperature] = &view.rings;
    // Half-way values round away from zero.
    assert_eq!(usage.center, "38%");
    assert_eq!(usage.caption, "Usage");
    assert_eq!(
        usage.segments,
        [
            Segment {
                fraction: 0.125,
                tone: Tone::System
            },
            Segment {
                fraction: 0.25,
                tone: Tone::User
            },
        ]
    );
    assert_eq!(temperature.center, "57°");
    assert_eq!(
        temperature.segments,
        [Segment {
            fraction: 0.57,
            tone: Tone::Good
        }]
    );
    let details: Vec<_> = view
        .details
        .iter()
        .map(|row| (row.label, row.value.as_str(), row.dot))
        .collect();
    assert_eq!(
        details,
        [
            ("System", "13%", Some(Tone::System)),
            ("User", "25%", Some(Tone::User)),
            ("Idle", "63%", Some(Tone::Dim)),
            ("Thermal", "Nominal", Some(Tone::Good)),
            ("Load avg / core (1/5/15m)", "0.25 / 0.30 / 1.00", None),
            ("Uptime", "1 hour, 1 minute", None),
        ]
    );
    assert_eq!(view.cores.len(), 2);
    assert_eq!(view.cores[0].busy, 0.5);
    assert_eq!(view.cores[0].busy_text, "50%");
    // Quiet below 75 °C, the ring's colours above it.
    assert_eq!(
        view.cores[0].temperature,
        Some(("56°".to_owned(), Tone::Dim))
    );
    assert_eq!(
        view.cores[1].temperature,
        Some(("80°".to_owned(), Tone::Warning))
    );
    let frequency = view.frequency.unwrap();
    assert_eq!(
        (frequency.label, frequency.value.as_str()),
        ("All cores", "2346 MHz")
    );
    let processes = view.processes.unwrap();
    assert_eq!(processes[0].name, "firefox");
    assert_eq!(processes[0].value, "105.0%");
    assert_eq!(
        processes[0].quit,
        Some(QuitTarget {
            pid: 42,
            start_ticks: 420,
            name: "firefox".to_owned()
        })
    );
    assert_eq!(processes[1].value, "0.0%");
    assert_eq!(processes[1].quit, None);
}

#[test]
fn the_first_cpu_page_shows_dashes_and_reading() {
    let view = cpu_view(&first_cpu_page());
    assert_eq!(view.subtitle, None);
    for ring in &view.rings {
        assert_eq!(ring.center, MISSING);
        assert!(ring.segments.is_empty());
    }
    let values: Vec<_> = view
        .details
        .iter()
        .map(|row| (row.label, row.value.as_str()))
        .collect();
    assert_eq!(
        values,
        [
            ("System", MISSING),
            ("User", MISSING),
            ("Idle", MISSING),
            ("Thermal", MISSING),
            ("Uptime", MISSING),
        ]
    );
    assert_eq!(view.details[3].dot, Some(Tone::Dim));
    assert!(view.cores.is_empty());
    assert_eq!(view.frequency, None);
    assert_eq!(view.processes, None);
}

#[test]
fn power_saver_adds_its_row_before_the_load() {
    let page = CpuPage {
        power_saver: true,
        ..full_cpu_page()
    };
    let labels: Vec<_> = cpu_view(&page)
        .details
        .iter()
        .map(|row| row.label)
        .collect();
    assert_eq!(labels[4], "Power saver");
    assert_eq!(cpu_view(&page).details[4].value, "On");
    assert_eq!(labels[5], "Load avg / core (1/5/15m)");
}

#[test]
fn a_core_without_a_sensor_or_past_the_list_has_no_temperature() {
    let page = CpuPage {
        core_temperatures_c: vec![None],
        ..full_cpu_page()
    };
    let view = cpu_view(&page);
    assert_eq!(view.cores[0].temperature, None);
    assert_eq!(view.cores[1].temperature, None);
}

#[test]
fn a_reading_past_full_scale_fills_the_ring_and_no_more() {
    let page = CpuPage {
        temperature_c: Some(105.0),
        ..full_cpu_page()
    };
    let view = cpu_view(&page);
    assert_eq!(view.rings[1].center, "105°");
    assert_eq!(
        view.rings[1].segments,
        [Segment {
            fraction: 1.0,
            tone: Tone::Bad
        }]
    );
}

#[test]
fn temperature_tones_change_at_75_and_90() {
    assert_eq!(temperature_tone(74.9), Tone::Good);
    assert_eq!(temperature_tone(75.0), Tone::Warning);
    assert_eq!(temperature_tone(89.9), Tone::Warning);
    assert_eq!(temperature_tone(90.0), Tone::Bad);
}

#[test]
fn every_level_has_its_tone_and_missing_psi_is_dim() {
    assert_eq!(thermal_tone(Thermal::Nominal), Tone::Good);
    assert_eq!(thermal_tone(Thermal::Fair), Tone::Warning);
    assert_eq!(thermal_tone(Thermal::Serious), Tone::Warning);
    assert_eq!(thermal_tone(Thermal::Critical), Tone::Bad);
    assert_eq!(pressure_tone(None), Tone::Dim);
    assert_eq!(pressure_tone(Some(Pressure::Normal)), Tone::Good);
    assert_eq!(pressure_tone(Some(Pressure::Warning)), Tone::Warning);
    assert_eq!(pressure_tone(Some(Pressure::Critical)), Tone::Bad);
}

#[test]
fn usage_tones_change_at_70_and_85() {
    assert_eq!(usage_tone(69.9), Tone::Good);
    assert_eq!(usage_tone(70.0), Tone::Warning);
    assert_eq!(usage_tone(84.9), Tone::Warning);
    assert_eq!(usage_tone(85.0), Tone::Bad);
}

fn memory_page(compressed: u64, pressure: Option<Pressure>) -> MemoryPage {
    MemoryPage {
        buckets: MemoryBuckets {
            total: 16 * GIB,
            app: 6 * GIB,
            kernel: 2 * GIB,
            cached: 4 * GIB,
            free: 4 * GIB,
            swap_used: GIB / 2,
            compressed,
        },
        pressure,
        processes: Some(vec![MemoryProcess {
            process: process(7, "code", true),
            bytes: 300 * (1 << 20),
        }]),
    }
}

#[test]
fn a_memory_page_reads_as_upstream() {
    let view = memory_view(&memory_page(GIB, Some(Pressure::Normal)));
    assert_eq!(view.subtitle, "16.00 GB total");
    let [pressure, usage] = &view.rings;
    assert_eq!(pressure.center, "Normal");
    assert_eq!(pressure.center_tone, Some(Tone::Good));
    // (kernel + zswap) / total.
    assert_eq!(
        pressure.segments,
        [Segment {
            fraction: 3.0 / 16.0,
            tone: Tone::Good
        }]
    );
    assert_eq!(usage.center, "50%");
    assert_eq!(
        usage.segments,
        [Segment {
            fraction: 0.5,
            tone: Tone::Good
        }]
    );
    assert_eq!(view.used, "8.00 GB");
    let bar: Vec<_> = view
        .bar
        .iter()
        .map(|segment| (segment.tone, segment.fraction))
        .collect();
    assert_eq!(
        bar,
        [
            (Tone::App, 0.375),
            (Tone::Kernel, 0.125),
            (Tone::Cached, 0.25),
            (Tone::Free, 0.25)
        ]
    );
    let details: Vec<_> = view
        .details
        .iter()
        .map(|row| (row.label, row.value.as_str()))
        .collect();
    assert_eq!(
        details,
        [
            ("App", "6.00 GB"),
            ("Kernel", "2.00 GB"),
            ("Cached", "4.00 GB"),
            ("Free", "4.00 GB"),
            ("Compressed", "1.00 GB"),
            ("Swap", "0.50 GB"),
        ]
    );
    let processes = view.processes.unwrap();
    assert_eq!(
        (processes[0].name.as_str(), processes[0].value.as_str()),
        ("code", "300 MB")
    );
    assert_eq!(processes[0].quit.as_ref().map(|target| target.pid), Some(7));
}

#[test]
fn without_zswap_or_psi_the_row_is_left_out_and_pressure_is_a_dash() {
    let view = memory_view(&memory_page(0, None));
    assert!(view.details.iter().all(|row| row.label != "Compressed"));
    assert_eq!(view.rings[0].center, MISSING);
    assert_eq!(view.rings[0].center_tone, Some(Tone::Dim));
    assert_eq!(view.rings[0].segments[0].tone, Tone::Dim);
}

#[test]
fn an_unknown_total_gives_empty_fractions() {
    let page = MemoryPage {
        buckets: MemoryBuckets::default(),
        pressure: None,
        processes: None,
    };
    let view = memory_view(&page);
    assert!(view.bar.iter().all(|segment| segment.fraction == 0.0));
    assert_eq!(view.rings[1].center, "0%");
    assert_eq!(view.processes, None);
}
```

The panel calls `MemoryBuckets::used()` on buckets the daemon sent, so its add must not overflow (04a follow-up). *Apply to* `crates/core/tests/memory.rs`:

```diff
diff --git a/crates/core/tests/memory.rs b/crates/core/tests/memory.rs
index e35a37c..7e79b84 100644
--- a/crates/core/tests/memory.rs
+++ b/crates/core/tests/memory.rs
@@ -175,3 +175,16 @@ fn an_unknown_total_gives_zero_fractions_instead_of_dividing_by_zero() {
     assert_eq!(buckets.fraction(123), 0.0);
     assert_eq!(buckets.usage_percent(), 0.0);
 }
+
+#[test]
+fn used_saturates_on_buckets_that_do_not_add_up() {
+    // Buckets a peer sent, not ones `from_meminfo` built.
+    let buckets = MemoryBuckets {
+        total: u64::MAX,
+        app: u64::MAX,
+        kernel: 1,
+        ..MemoryBuckets::default()
+    };
+    assert_eq!(buckets.used(), u64::MAX);
+    assert_eq!(buckets.usage_percent(), 100.0);
+}
```

- [x] **Step 2: Run it, expect FAIL.** `cargo test -p statsbar-panel --test view` fails: `unresolved import statsbar_panel::view`. `cargo test -p statsbar-core --test memory used_saturates` fails: `attempt to add with overflow`.

- [x] **Step 3: Implement.**

*Replace* `crates/panel/src/lib.rs` with:

```rust
//! `statsbar-panel`: the CPU and Memory pages, in a window the daemon opens on demand.

pub mod link;
pub mod view;
```

*Create* `crates/panel/src/view.rs`:

```rust
//! What the two pages show, worked out from the daemon's events without GTK, so every figure
//! and colour can be tested.

use statsbar_core::format::{fmt_gb, fmt_process_memory, fmt_uptime, rounded_int};
use statsbar_core::ipc::{CpuPage, MemoryPage, ProcessInfo};
use statsbar_core::memory::Pressure;
use statsbar_core::system::Thermal;

/// Shown for a figure the daemon could not read.
pub const MISSING: &str = "—";
/// The temperature ring's full scale, and where every reading is clamped.
const FULL_SCALE_CELSIUS: f64 = 100.0;
/// Temperatures below this stay quiet; above it they take the ring's colours (upstream).
const WARM_FROM_CELSIUS: f64 = 75.0;
const HOT_FROM_CELSIUS: f64 = 90.0;
/// Memory usage colours (upstream).
const BUSY_FROM_PERCENT: f64 = 70.0;
const FULL_FROM_PERCENT: f64 = 85.0;

/// A colour, named by meaning; the window maps it to a libadwaita style class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// Secondary text, an empty track.
    Dim,
    Good,
    Warning,
    Bad,
    /// The CPU's system share.
    System,
    /// The CPU's user share and the cores' bars.
    User,
    /// Memory segments.
    App,
    Kernel,
    Cached,
    Free,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    /// 0–1 of the whole track.
    pub fraction: f64,
    pub tone: Tone,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ring {
    pub segments: Vec<Segment>,
    pub center: String,
    pub center_tone: Option<Tone>,
    pub caption: &'static str,
}

/// A labelled figure. A row with a tone has a colour dot, as in a legend.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub label: &'static str,
    pub value: String,
    pub dot: Option<Tone>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Core {
    pub busy: f64,
    pub busy_text: String,
    pub temperature: Option<(String, Tone)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProcessRow {
    pub name: String,
    pub value: String,
    /// `Some` only when the daemon marked the process as quittable.
    pub quit: Option<QuitTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuitTarget {
    pub pid: u32,
    pub start_ticks: u64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CpuView {
    pub subtitle: Option<String>,
    pub rings: [Ring; 2],
    pub details: Vec<Row>,
    pub cores: Vec<Core>,
    pub frequency: Option<Row>,
    /// `None` until the daemon's first ranking: "Reading…".
    pub processes: Option<Vec<ProcessRow>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryView {
    pub subtitle: String,
    pub rings: [Ring; 2],
    pub used: String,
    pub bar: Vec<Segment>,
    pub details: Vec<Row>,
    pub processes: Option<Vec<ProcessRow>>,
}

fn percent_text(value: f64) -> String {
    format!("{}%", rounded_int(value))
}

fn fraction(percent: f64) -> f64 {
    (percent / 100.0).clamp(0.0, 1.0)
}

/// Green, then orange from 75 °C and red from 90 °C, as upstream's temperature ring.
pub fn temperature_tone(celsius: f64) -> Tone {
    if celsius >= HOT_FROM_CELSIUS {
        Tone::Bad
    } else if celsius >= WARM_FROM_CELSIUS {
        Tone::Warning
    } else {
        Tone::Good
    }
}

pub fn thermal_tone(thermal: Thermal) -> Tone {
    match thermal {
        Thermal::Nominal => Tone::Good,
        Thermal::Fair | Thermal::Serious => Tone::Warning,
        Thermal::Critical => Tone::Bad,
    }
}

pub fn pressure_tone(pressure: Option<Pressure>) -> Tone {
    match pressure {
        None => Tone::Dim,
        Some(Pressure::Normal) => Tone::Good,
        Some(Pressure::Warning) => Tone::Warning,
        Some(Pressure::Critical) => Tone::Bad,
    }
}

pub fn usage_tone(percent: f64) -> Tone {
    if percent >= FULL_FROM_PERCENT {
        Tone::Bad
    } else if percent >= BUSY_FROM_PERCENT {
        Tone::Warning
    } else {
        Tone::Good
    }
}

fn quit_target(process: &ProcessInfo) -> Option<QuitTarget> {
    process.can_quit.then(|| QuitTarget {
        pid: process.pid,
        start_ticks: process.start_ticks,
        name: process.name.clone(),
    })
}

fn row(label: &'static str, value: String) -> Row {
    Row {
        label,
        value,
        dot: None,
    }
}

fn legend(label: &'static str, value: String, tone: Tone) -> Row {
    Row {
        label,
        value,
        dot: Some(tone),
    }
}

pub fn cpu_view(page: &CpuPage) -> CpuView {
    let load = page.load.as_ref();
    let usage = Ring {
        segments: load.map_or_else(Vec::new, |load| {
            vec![
                Segment {
                    fraction: fraction(load.system_percent),
                    tone: Tone::System,
                },
                Segment {
                    fraction: fraction(load.user_percent),
                    tone: Tone::User,
                },
            ]
        }),
        center: load.map_or_else(
            || MISSING.to_owned(),
            |load| percent_text(load.usage_percent()),
        ),
        center_tone: None,
        caption: "Usage",
    };
    let temperature = Ring {
        segments: page.temperature_c.map_or_else(Vec::new, |celsius| {
            vec![Segment {
                fraction: (celsius / FULL_SCALE_CELSIUS).clamp(0.0, 1.0),
                tone: temperature_tone(celsius),
            }]
        }),
        center: page.temperature_c.map_or_else(
            || MISSING.to_owned(),
            |celsius| format!("{}°", rounded_int(celsius)),
        ),
        center_tone: None,
        caption: "Temperature",
    };
    let share = |pick: fn(&statsbar_core::cpu::CpuLoad) -> f64| {
        load.map_or_else(|| MISSING.to_owned(), |load| percent_text(pick(load)))
    };
    let mut details = vec![
        legend("System", share(|load| load.system_percent), Tone::System),
        legend("User", share(|load| load.user_percent), Tone::User),
        legend("Idle", share(|load| load.idle_percent), Tone::Dim),
        legend(
            "Thermal",
            page.thermal
                .map_or_else(|| MISSING.to_owned(), |thermal| thermal.label().to_owned()),
            page.thermal.map_or(Tone::Dim, thermal_tone),
        ),
    ];
    if page.power_saver {
        details.push(row("Power saver", "On".to_owned()));
    }
    if let Some(load) = page.load_per_cpu {
        details.push(row(
            "Load avg / core (1/5/15m)",
            load.map(|value| format!("{value:.2}")).join(" / "),
        ));
    }
    details.push(row(
        "Uptime",
        page.uptime_seconds
            .map_or_else(|| MISSING.to_owned(), fmt_uptime),
    ));
    let cores = load.map_or_else(Vec::new, |load| {
        load.per_core_busy
            .iter()
            .enumerate()
            .map(|(index, &busy)| Core {
                busy: fraction(busy),
                busy_text: percent_text(busy),
                // A list of another length belongs to other CPUs, and the daemon then sends none.
                temperature: page.core_temperatures_c.get(index).copied().flatten().map(
                    |celsius| {
                        let tone = if celsius < WARM_FROM_CELSIUS {
                            Tone::Dim
                        } else {
                            temperature_tone(celsius)
                        };
                        (format!("{}°", rounded_int(celsius)), tone)
                    },
                ),
            })
            .collect()
    });
    CpuView {
        subtitle: page.model.clone(),
        rings: [usage, temperature],
        details,
        cores,
        frequency: page
            .frequency_mhz
            .map(|mhz| row("All cores", format!("{} MHz", rounded_int(mhz)))),
        processes: page.processes.as_ref().map(|processes| {
            processes
                .iter()
                .map(|entry| ProcessRow {
                    name: entry.process.name.clone(),
                    value: format!("{:.1}%", entry.cpu_percent),
                    quit: quit_target(&entry.process),
                })
                .collect()
        }),
    }
}

pub fn memory_view(page: &MemoryPage) -> MemoryView {
    let memory = &page.buckets;
    let usage_percent = memory.usage_percent();
    let pressure_tone = pressure_tone(page.pressure);
    let pressure = Ring {
        segments: vec![Segment {
            fraction: memory.pressure_fraction(),
            tone: pressure_tone,
        }],
        center: page.pressure.map_or_else(
            || MISSING.to_owned(),
            |pressure| pressure.label().to_owned(),
        ),
        center_tone: Some(pressure_tone),
        caption: "Pressure",
    };
    let usage = Ring {
        segments: vec![Segment {
            fraction: memory.fraction(memory.used()),
            tone: usage_tone(usage_percent),
        }],
        center: percent_text(usage_percent),
        center_tone: None,
        caption: "Usage",
    };
    // Compressed is inside App, so it is a figure, not a segment: the bar tiles the total.
    let bar = [
        (memory.app, Tone::App),
        (memory.kernel, Tone::Kernel),
        (memory.cached, Tone::Cached),
        (memory.free, Tone::Free),
    ]
    .into_iter()
    .map(|(bytes, tone)| Segment {
        fraction: memory.fraction(bytes),
        tone,
    })
    .collect();
    let mut details = vec![
        legend("App", fmt_gb(memory.app), Tone::App),
        legend("Kernel", fmt_gb(memory.kernel), Tone::Kernel),
        legend("Cached", fmt_gb(memory.cached), Tone::Cached),
        legend("Free", fmt_gb(memory.free), Tone::Free),
    ];
    if memory.compressed > 0 {
        details.push(row("Compressed", fmt_gb(memory.compressed)));
    }
    details.push(row("Swap", fmt_gb(memory.swap_used)));
    MemoryView {
        subtitle: format!("{} total", fmt_gb(memory.total)),
        rings: [pressure, usage],
        used: fmt_gb(memory.used()),
        bar,
        details,
        processes: page.processes.as_ref().map(|processes| {
            processes
                .iter()
                .map(|entry| ProcessRow {
                    name: entry.process.name.clone(),
                    value: fmt_process_memory(entry.bytes),
                    quit: quit_target(&entry.process),
                })
                .collect()
        }),
    }
}
```

*Apply to* `crates/core/src/memory.rs`:

```diff
diff --git a/crates/core/src/memory.rs b/crates/core/src/memory.rs
index 70ee279..016b7f9 100644
--- a/crates/core/src/memory.rs
+++ b/crates/core/src/memory.rs
@@ -100,9 +100,9 @@ impl MemoryBuckets {
     }
 
     /// `MemTotal − MemAvailable`: memory that cannot be handed out without evicting or
-    /// swapping something.
+    /// swapping something. Saturates, because the panel gets these buckets from a peer.
     pub fn used(&self) -> u64 {
-        self.app + self.kernel
+        self.app.saturating_add(self.kernel)
     }
 
     pub fn usage_percent(&self) -> f64 {
```

- [x] **Step 4: Run, expect PASS.** `cargo test -p statsbar-panel --test view`, `cargo test -p statsbar-core`, then `cargo clippy -p statsbar-panel --all-targets -- -D warnings` and `cargo fmt --check`.

- [x] **Step 5: Checkpoint.** Ready to commit: `crates/panel/src/{lib.rs,view.rs}`, `crates/panel/tests/view.rs`, `crates/core/{src,tests}/memory.rs` — "feat(panel): work out the CPU and Memory page figures"

### Task 3: Quit a listed process

**Type:** feature · **Test level:** integration (real child processes; only `sleep` children are signalled)

**Files:** Create `crates/panel/src/quit.rs`, `crates/panel/tests/quit.rs`. Modify `crates/panel/Cargo.toml`, `crates/panel/src/lib.rs`, `Cargo.lock`.

**Interfaces:** Consumes `process::parse_stat`, `view::QuitTarget`. Produces `quit::{quit(&Path, &QuitTarget) -> Result<(), QuitError>, QuitError::{Gone, Replaced, NotPermitted, Io}, impl From<Errno> for QuitError}`.

- [x] **Step 1: Write the failing test.**

*Create* `crates/panel/tests/quit.rs`:

```rust
//! Quitting real child processes. Each test spawns its own `sleep` and reaps it.

use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::{Child, Command};

use rustix::io::Errno;
use statsbar_core::process::parse_stat;
use statsbar_panel::quit::{QuitError, quit};
use statsbar_panel::view::QuitTarget;

const PROC: &str = "/proc";
const SIGTERM: i32 = 15;

/// Kills and reaps on drop, so a failed assertion never leaks a process.
struct Sleeper(Child);

impl Sleeper {
    fn spawn() -> Sleeper {
        Sleeper(Command::new("sleep").arg("600").spawn().unwrap())
    }

    fn target(&self) -> QuitTarget {
        let pid = self.0.id();
        let text = std::fs::read_to_string(format!("{PROC}/{pid}/stat")).unwrap();
        QuitTarget {
            pid,
            start_ticks: parse_stat(&text).unwrap().start_ticks,
            name: "sleep".to_owned(),
        }
    }
}

impl Drop for Sleeper {
    fn drop(&mut self) {
        // Both fail only once the process is gone, which is the goal.
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn the_listed_process_gets_sigterm() {
    let mut sleeper = Sleeper::spawn();
    quit(Path::new(PROC), &sleeper.target()).unwrap();
    assert_eq!(sleeper.0.wait().unwrap().signal(), Some(SIGTERM));
}

#[test]
fn a_pid_with_another_start_time_is_left_alone() {
    let mut sleeper = Sleeper::spawn();
    let mut target = sleeper.target();
    target.start_ticks += 1;
    assert!(matches!(
        quit(Path::new(PROC), &target),
        Err(QuitError::Replaced)
    ));
    assert!(
        sleeper.0.try_wait().unwrap().is_none(),
        "the process must still run"
    );
}

#[test]
fn a_reaped_process_is_gone() {
    let mut sleeper = Sleeper::spawn();
    let target = sleeper.target();
    sleeper.0.kill().unwrap();
    sleeper.0.wait().unwrap();
    assert!(matches!(
        quit(Path::new(PROC), &target),
        Err(QuitError::Gone)
    ));
}

#[test]
fn a_pid_outside_the_kernels_range_is_gone() {
    for pid in [0, u32::MAX] {
        let target = QuitTarget {
            pid,
            start_ticks: 0,
            name: "x".to_owned(),
        };
        assert!(
            matches!(quit(Path::new(PROC), &target), Err(QuitError::Gone)),
            "{pid}"
        );
    }
}

#[test]
fn kernel_refusals_map_to_their_reasons() {
    assert!(matches!(QuitError::from(Errno::SRCH), QuitError::Gone));
    assert!(matches!(
        QuitError::from(Errno::PERM),
        QuitError::NotPermitted
    ));
    assert!(matches!(QuitError::from(Errno::INVAL), QuitError::Io(_)));
}
```

- [x] **Step 2: Run it, expect FAIL.** `cargo test -p statsbar-panel --test quit` fails: `unresolved import statsbar_panel::quit`.

- [x] **Step 3: Implement.**

*Replace* `crates/panel/Cargo.toml`'s `[dependencies]` with:

```toml
statsbar-core = { path = "../core" }
async-channel = "=2.5.0"
rustix = { version = "=1.1.6", features = ["process"] }
serde = "=1.0.229"
serde_json = "=1.0.154"
```

*Replace* `crates/panel/src/lib.rs` with:

```rust
//! `statsbar-panel`: the CPU and Memory pages, in a window the daemon opens on demand.

pub mod link;
pub mod quit;
pub mod view;
```

*Create* `crates/panel/src/quit.rs`:

```rust
//! Quitting a process from a top-process list: SIGTERM, and only to the process the row named.
//!
//! The daemon names a process by pid and start time. A pidfd is opened first and the start time
//! checked after, so a pid reused in between is caught, and one reused after the check cannot
//! receive the signal: the pidfd still refers to the process that was checked.

use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use rustix::io::Errno;
use rustix::process::{Pid, PidfdFlags, Signal, pidfd_open, pidfd_send_signal};
use statsbar_core::process::parse_stat;

use crate::view::QuitTarget;

#[derive(Debug)]
pub enum QuitError {
    /// The process has already exited.
    Gone,
    /// The pid now belongs to another process.
    Replaced,
    /// Another user's process, which the kernel refuses to signal.
    NotPermitted,
    Io(io::Error),
}

impl fmt::Display for QuitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QuitError::Gone | QuitError::Replaced => write!(f, "it has already exited"),
            QuitError::NotPermitted => write!(f, "it belongs to another user"),
            QuitError::Io(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for QuitError {}

impl From<Errno> for QuitError {
    fn from(errno: Errno) -> QuitError {
        match errno {
            Errno::SRCH => QuitError::Gone,
            Errno::PERM => QuitError::NotPermitted,
            other => QuitError::Io(other.into()),
        }
    }
}

/// Sends SIGTERM to `target`, after checking under `proc_root` (`/proc`) that its pid still
/// names the process the daemon listed.
pub fn quit(proc_root: &Path, target: &QuitTarget) -> Result<(), QuitError> {
    let raw = i32::try_from(target.pid).map_err(|_| QuitError::Gone)?;
    let pid = Pid::from_raw(raw).ok_or(QuitError::Gone)?;
    let pidfd = pidfd_open(pid, PidfdFlags::empty())?;
    let text = match fs::read_to_string(proc_root.join(target.pid.to_string()).join("stat")) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Err(QuitError::Gone),
        Err(error) => return Err(QuitError::Io(error)),
    };
    let stat = parse_stat(&text).ok_or_else(|| {
        QuitError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            "unreadable stat",
        ))
    })?;
    if stat.start_ticks != target.start_ticks {
        return Err(QuitError::Replaced);
    }
    pidfd_send_signal(&pidfd, Signal::TERM)?;
    Ok(())
}
```

- [x] **Step 4: Run, expect PASS.** `cargo test -p statsbar-panel --test quit`, then `cargo clippy -p statsbar-panel --all-targets -- -D warnings` and `cargo fmt --check`.

- [x] **Step 5: Checkpoint.** Ready to commit: `crates/panel/{Cargo.toml,src/lib.rs,src/quit.rs,tests/quit.rs}`, `Cargo.lock` — "feat(panel): quit a listed process after checking its start time"

### Task 4: The `--page` argument

**Type:** feature · **Test level:** unit

**Files:** Create `crates/panel/src/cli.rs`, `crates/panel/tests/cli.rs`. Modify `crates/panel/src/lib.rs`.

**Interfaces:** Produces `cli::page_from_args(&[OsString]) -> Result<Page, String>`.

- [x] **Step 1: Write the failing test.**

*Create* `crates/panel/tests/cli.rs`:

```rust
use std::ffi::OsString;

use statsbar_core::ipc::Page;
use statsbar_panel::cli::page_from_args;

fn args(list: &[&str]) -> Vec<OsString> {
    list.iter().map(OsString::from).collect()
}

#[test]
fn the_page_comes_from_the_flag_and_defaults_to_cpu() {
    assert_eq!(page_from_args(&args(&[])), Ok(Page::Cpu));
    assert_eq!(page_from_args(&args(&["--page", "cpu"])), Ok(Page::Cpu));
    assert_eq!(
        page_from_args(&args(&["--page", "memory"])),
        Ok(Page::Memory)
    );
}

#[test]
fn anything_else_is_a_usage_error() {
    for bad in [
        &["--page"][..],
        &["--page", "gpu"],
        &["memory"],
        &["--page", "cpu", "x"],
    ] {
        assert!(page_from_args(&args(bad)).is_err(), "{bad:?}");
    }
}
```

- [x] **Step 2: Run it, expect FAIL.** `cargo test -p statsbar-panel --test cli` fails: `unresolved import statsbar_panel::cli`.

- [x] **Step 3: Implement.**

*Replace* `crates/panel/src/lib.rs` with:

```rust
//! `statsbar-panel`: the CPU and Memory pages, in a window the daemon opens on demand.

pub mod cli;
pub mod link;
pub mod quit;
pub mod view;
```

*Create* `crates/panel/src/cli.rs`:

```rust
//! `statsbar-panel [--page cpu|memory]`. The daemon's menus pass the page; without one the
//! window opens on CPU.

use std::ffi::OsString;

use statsbar_core::ipc::Page;

/// The arguments after the program name.
pub fn page_from_args(args: &[OsString]) -> Result<Page, String> {
    match args {
        [] => Ok(Page::Cpu),
        [flag, page] if flag == "--page" => match page.to_str() {
            Some("cpu") => Ok(Page::Cpu),
            Some("memory") => Ok(Page::Memory),
            _ => Err(format!("unknown page {page:?}; expected cpu or memory")),
        },
        _ => Err("usage: statsbar-panel [--page cpu|memory]".to_owned()),
    }
}
```

- [x] **Step 4: Run, expect PASS.** `cargo test -p statsbar-panel --test cli`, then `cargo clippy -p statsbar-panel --all-targets -- -D warnings` and `cargo fmt --check`.

- [x] **Step 5: Checkpoint.** Ready to commit: `crates/panel/src/{lib.rs,cli.rs}`, `crates/panel/tests/cli.rs` — "feat(panel): read the page to open from the command line"

### Task 5: The window

**Type:** feature · **Test level:** e2e only (Task 7). `RED: n/a — GTK needs a display, which `cargo test` and CI do not have; the logic it shows is Tasks 1–4's, and Task 7 exercises every control on the desktop, including Refresh and a failed quit.`

**Files:** Create `crates/panel/src/widgets.rs`, `crates/panel/src/window.rs`, `crates/panel/src/main.rs`. Modify `crates/panel/Cargo.toml`, `crates/panel/src/lib.rs`, `Cargo.lock`.

**Interfaces:** Consumes Tasks 1–4. Produces `window::{run() -> glib::ExitCode, APP_ID}`; `widgets` is private.

- [x] **Step 1: Dependencies.** *Replace* `crates/panel/Cargo.toml` with:

```toml
[package]
name = "statsbar-panel"
version.workspace = true
edition.workspace = true
# gtk4 0.11.5 declares rust-version 1.92.
rust-version = "1.92"
publish.workspace = true

[dependencies]
statsbar-core = { path = "../core" }
async-channel = "=2.5.0"
# libadwaita 1.5 is the newest every target has: Ubuntu 24.04, which CI may run, ships 1.5.
adw = { package = "libadwaita", version = "=0.9.2", features = ["v1_5"] }
# 4.12 for `WidgetExt::color` (4.10), which the drawn rings take their dim tones from, and
# `CssProvider::load_from_string`. Ubuntu 24.04 ships 4.14.
gtk = { package = "gtk4", version = "=0.11.5", features = ["v4_12"] }
rustix = { version = "=1.1.6", features = ["process"] }
serde = "=1.0.229"
serde_json = "=1.0.154"

[lints]
workspace = true
```

- [x] **Step 2: Implement.** *Replace* `crates/panel/src/lib.rs` with:

```rust
//! `statsbar-panel`: the CPU and Memory pages, in a window the daemon opens on demand.

pub mod cli;
pub mod link;
pub mod quit;
pub mod view;
mod widgets;
pub mod window;
```

*Create* `crates/panel/src/widgets.rs`:

```rust
//! The pages' building blocks: rings, bars, legend rows and the process table. Each is built
//! once and updated in place, so a click is never lost to a rebuild.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Align, Box as GtkBox, DrawingArea, Label, Orientation, cairo, gdk};

use crate::view::{Core, ProcessRow, QuitTarget, Ring, Row, Segment, Tone};

const RING_SIZE: i32 = 86;
const RING_WIDTH: f64 = 8.0;
const BAR_HEIGHT: i32 = 10;
const CORE_BAR_HEIGHT: i32 = 36;
const CORE_COLUMNS: u32 = 4;
/// The daemon sends six; this is the table's fixed size.
pub const PROCESS_ROWS: usize = 6;
const TRACK_ALPHA: f32 = 0.12;
const DIM_ALPHA: f32 = 0.45;
const FREE_ALPHA: f32 = 0.25;

enum Paint {
    /// Red, green, blue and alpha, 0–1.
    Fixed([f32; 4]),
    /// The theme's text colour at this alpha, so it follows dark and light styles.
    Foreground(f32),
}

/// One table for drawn shapes and for text, so a tone looks the same in both.
fn paint(tone: Tone) -> Paint {
    // GNOME HIG palette: green 4, orange 3, red 3, blue 3.
    match tone {
        Tone::Good => Paint::Fixed([0.180, 0.761, 0.494, 1.0]),
        Tone::Warning | Tone::Kernel => Paint::Fixed([1.0, 0.471, 0.0, 1.0]),
        Tone::Bad | Tone::System => Paint::Fixed([0.878, 0.106, 0.141, 1.0]),
        Tone::User => Paint::Fixed([0.208, 0.518, 0.894, 1.0]),
        Tone::Cached => Paint::Fixed([0.208, 0.518, 0.894, 0.6]),
        Tone::App => Paint::Foreground(1.0),
        Tone::Free => Paint::Foreground(FREE_ALPHA),
        Tone::Dim => Paint::Foreground(DIM_ALPHA),
    }
}

fn rgba(tone: Tone, foreground: gdk::RGBA) -> gdk::RGBA {
    match paint(tone) {
        Paint::Fixed([red, green, blue, alpha]) => gdk::RGBA::new(red, green, blue, alpha),
        Paint::Foreground(alpha) => foreground.with_alpha(foreground.alpha() * alpha),
    }
}

fn css_class(tone: Tone) -> &'static str {
    match tone {
        Tone::Dim => "tone-dim",
        Tone::Good => "tone-good",
        Tone::Warning => "tone-warning",
        Tone::Bad => "tone-bad",
        Tone::System => "tone-system",
        Tone::User => "tone-user",
        Tone::App => "tone-app",
        Tone::Kernel => "tone-kernel",
        Tone::Cached => "tone-cached",
        Tone::Free => "tone-free",
    }
}

const TONES: [Tone; 10] = [
    Tone::Dim,
    Tone::Good,
    Tone::Warning,
    Tone::Bad,
    Tone::System,
    Tone::User,
    Tone::App,
    Tone::Kernel,
    Tone::Cached,
    Tone::Free,
];

/// The style sheet for text in a tone, from the same table as the drawn shapes.
pub fn tone_css() -> String {
    TONES
        .iter()
        .map(|&tone| {
            let color = match paint(tone) {
                Paint::Fixed([red, green, blue, alpha]) => format!(
                    "rgba({}, {}, {}, {alpha})",
                    (red * 255.0).round(),
                    (green * 255.0).round(),
                    (blue * 255.0).round()
                ),
                Paint::Foreground(alpha) => format!("alpha(currentColor, {alpha})"),
            };
            format!(".{} {{ color: {color}; }}\n", css_class(tone))
        })
        .collect()
}

fn set_tone(widget: &impl IsA<gtk::Widget>, tone: Option<Tone>) {
    for &other in &TONES {
        widget.remove_css_class(css_class(other));
    }
    if let Some(tone) = tone {
        widget.add_css_class(css_class(tone));
    }
}

/// Segments laid end to end from `start`, each `fraction` of `span` long.
fn stroke_segments(
    context: &cairo::Context,
    foreground: gdk::RGBA,
    segments: &[Segment],
    mut draw: impl FnMut(&cairo::Context, f64, f64),
) {
    let mut start = 0.0;
    for segment in segments {
        let end = (start + segment.fraction).min(1.0);
        if end > start {
            context.set_source_color(&rgba(segment.tone, foreground));
            draw(context, start, end);
        }
        start = end;
    }
}

pub struct RingWidget {
    pub root: GtkBox,
    area: DrawingArea,
    center: Label,
    caption: Label,
    segments: Rc<RefCell<Vec<Segment>>>,
}

impl RingWidget {
    pub fn new() -> RingWidget {
        let segments: Rc<RefCell<Vec<Segment>>> = Rc::default();
        let area = DrawingArea::builder()
            .content_width(RING_SIZE)
            .content_height(RING_SIZE)
            .build();
        let drawn = Rc::clone(&segments);
        area.set_draw_func(move |area, context, width, height| {
            let foreground = area.color();
            let (x, y) = (f64::from(width) / 2.0, f64::from(height) / 2.0);
            let radius = x.min(y) - RING_WIDTH / 2.0;
            context.set_line_width(RING_WIDTH);
            context.set_source_color(&foreground.with_alpha(TRACK_ALPHA));
            context.arc(x, y, radius, 0.0, std::f64::consts::TAU);
            // A failed stroke leaves the ring blank until the next frame; there is nothing to do.
            let _ = context.stroke();
            let top = -std::f64::consts::FRAC_PI_2;
            stroke_segments(context, foreground, &drawn.borrow(), |context, from, to| {
                context.arc(
                    x,
                    y,
                    radius,
                    top + from * std::f64::consts::TAU,
                    top + to * std::f64::consts::TAU,
                );
                let _ = context.stroke();
            });
        });
        let center = Label::new(None);
        center.add_css_class("title-3");
        center.add_css_class("numeric");
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&area));
        overlay.add_overlay(&center);
        let caption = Label::new(None);
        caption.add_css_class("caption-heading");
        let root = GtkBox::new(Orientation::Vertical, 8);
        root.append(&overlay);
        root.append(&caption);
        RingWidget {
            root,
            area,
            center,
            caption,
            segments,
        }
    }

    pub fn show(&self, ring: &Ring) {
        self.segments.replace(ring.segments.clone());
        self.area.queue_draw();
        self.center.set_label(&ring.center);
        set_tone(&self.center, ring.center_tone);
        self.caption.set_label(ring.caption);
    }
}

/// A horizontal bar of segments, as the memory breakdown.
pub struct BarWidget {
    pub area: DrawingArea,
    segments: Rc<RefCell<Vec<Segment>>>,
}

impl BarWidget {
    pub fn new() -> BarWidget {
        let segments: Rc<RefCell<Vec<Segment>>> = Rc::default();
        let area = DrawingArea::builder()
            .content_height(BAR_HEIGHT)
            .hexpand(true)
            .build();
        let drawn = Rc::clone(&segments);
        area.set_draw_func(move |area, context, width, height| {
            let foreground = area.color();
            let (width, height) = (f64::from(width), f64::from(height));
            context.set_source_color(&foreground.with_alpha(TRACK_ALPHA));
            context.rectangle(0.0, 0.0, width, height);
            let _ = context.fill();
            stroke_segments(context, foreground, &drawn.borrow(), |context, from, to| {
                context.rectangle(from * width, 0.0, (to - from) * width, height);
                let _ = context.fill();
            });
        });
        BarWidget { area, segments }
    }

    pub fn show(&self, segments: &[Segment]) {
        self.segments.replace(segments.to_vec());
        self.area.queue_draw();
    }
}

/// A vertical bar filled from the bottom, one per core.
struct CoreWidget {
    root: GtkBox,
    temperature: Label,
    area: DrawingArea,
    busy: Rc<RefCell<f64>>,
    busy_text: Label,
}

impl CoreWidget {
    fn new() -> CoreWidget {
        let busy: Rc<RefCell<f64>> = Rc::default();
        let area = DrawingArea::builder()
            .content_height(CORE_BAR_HEIGHT)
            .hexpand(true)
            .build();
        let drawn = Rc::clone(&busy);
        area.set_draw_func(move |area, context, width, height| {
            let foreground = area.color();
            let (width, height) = (f64::from(width), f64::from(height));
            context.set_source_color(&foreground.with_alpha(TRACK_ALPHA));
            context.rectangle(0.0, 0.0, width, height);
            let _ = context.fill();
            let filled = height * drawn.borrow().clamp(0.0, 1.0);
            context.set_source_color(&rgba(Tone::User, foreground));
            context.rectangle(0.0, height - filled, width, filled);
            let _ = context.fill();
        });
        let temperature = small_label();
        let busy_text = small_label();
        busy_text.add_css_class("dim-label");
        let root = GtkBox::new(Orientation::Vertical, 3);
        root.append(&temperature);
        root.append(&area);
        root.append(&busy_text);
        CoreWidget {
            root,
            temperature,
            area,
            busy,
            busy_text,
        }
    }

    fn show(&self, core: &Core) {
        self.busy.replace(core.busy);
        self.area.queue_draw();
        self.busy_text.set_label(&core.busy_text);
        match &core.temperature {
            Some((text, tone)) => {
                self.temperature.set_label(text);
                set_tone(&self.temperature, Some(*tone));
            }
            None => self.temperature.set_label(""),
        }
    }
}

fn small_label() -> Label {
    let label = Label::new(None);
    label.add_css_class("caption");
    label.add_css_class("numeric");
    label
}

/// One bar per logical CPU, four to a row. Rebuilt only when the number of CPUs changes.
pub struct CoresWidget {
    pub root: gtk::Grid,
    cells: RefCell<Vec<CoreWidget>>,
}

impl CoresWidget {
    pub fn new() -> CoresWidget {
        let root = gtk::Grid::builder()
            .column_spacing(8)
            .row_spacing(8)
            .column_homogeneous(true)
            .build();
        CoresWidget {
            root,
            cells: RefCell::default(),
        }
    }

    pub fn show(&self, cores: &[Core]) {
        let mut cells = self.cells.borrow_mut();
        if cells.len() != cores.len() {
            for cell in cells.drain(..) {
                self.root.remove(&cell.root);
            }
            for index in 0..cores.len() {
                let cell = CoreWidget::new();
                let index = u32::try_from(index).unwrap_or(u32::MAX);
                let (column, row) = (index % CORE_COLUMNS, index / CORE_COLUMNS);
                self.root.attach(
                    &cell.root,
                    i32::try_from(column).unwrap_or(0),
                    i32::try_from(row).unwrap_or(0),
                    1,
                    1,
                );
                cells.push(cell);
            }
        }
        for (cell, core) in cells.iter().zip(cores) {
            cell.show(core);
        }
    }
}

/// A caption above a section, as upstream's "DETAILS".
pub fn section(title: &str) -> Label {
    let label = Label::builder().label(title).xalign(0.0).build();
    label.add_css_class("heading");
    label.add_css_class("dim-label");
    label
}

struct RowWidget {
    root: GtkBox,
    dot: Label,
    label: Label,
    value: Label,
}

/// Legend and info rows. The list grows as needed; unused rows are hidden, not removed.
pub struct RowsWidget {
    pub root: GtkBox,
    rows: RefCell<Vec<RowWidget>>,
}

impl RowsWidget {
    pub fn new() -> RowsWidget {
        RowsWidget {
            root: GtkBox::new(Orientation::Vertical, 6),
            rows: RefCell::default(),
        }
    }

    pub fn show(&self, rows: &[Row]) {
        let mut widgets = self.rows.borrow_mut();
        while widgets.len() < rows.len() {
            let dot = Label::new(Some("●"));
            let label = Label::builder().xalign(0.0).hexpand(true).build();
            let value = Label::builder().xalign(1.0).build();
            value.add_css_class("numeric");
            let root = GtkBox::new(Orientation::Horizontal, 8);
            root.append(&dot);
            root.append(&label);
            root.append(&value);
            self.root.append(&root);
            widgets.push(RowWidget {
                root,
                dot,
                label,
                value,
            });
        }
        for (index, widget) in widgets.iter().enumerate() {
            let Some(row) = rows.get(index) else {
                widget.root.set_visible(false);
                continue;
            };
            widget.root.set_visible(true);
            widget.label.set_label(row.label);
            widget.value.set_label(&row.value);
            // A row without a dot keeps the dot's width, so the labels line up.
            widget
                .dot
                .set_opacity(if row.dot.is_some() { 1.0 } else { 0.0 });
            set_tone(&widget.dot, row.dot);
        }
    }
}

struct ProcessWidget {
    root: GtkBox,
    name: Label,
    value: Label,
    quit: gtk::Button,
    target: Rc<RefCell<Option<QuitTarget>>>,
}

/// The top-process table: six fixed rows, each with a ✕ for a process the user may quit.
pub struct ProcessesWidget {
    pub root: GtkBox,
    reading: Label,
    rows: Vec<ProcessWidget>,
}

impl ProcessesWidget {
    /// `on_quit` gets the target the row showed when its ✕ was clicked.
    pub fn new(value_header: &str, on_quit: Rc<dyn Fn(QuitTarget)>) -> ProcessesWidget {
        let root = GtkBox::new(Orientation::Vertical, 6);
        let reading = Label::builder().label("Reading…").xalign(0.0).build();
        reading.add_css_class("caption");
        reading.add_css_class("dim-label");
        root.append(&reading);
        let header = GtkBox::new(Orientation::Horizontal, 8);
        let process = Label::builder()
            .label("Process")
            .xalign(0.0)
            .hexpand(true)
            .build();
        let value = Label::builder().label(value_header).xalign(1.0).build();
        for label in [&process, &value] {
            label.add_css_class("caption-heading");
            label.add_css_class("dim-label");
            header.append(label);
        }
        // The ✕ column's width, so the header's value lines up with the rows'.
        let spacer = gtk::Image::from_icon_name("window-close-symbolic");
        spacer.set_opacity(0.0);
        header.append(&spacer);
        root.append(&header);
        let rows = (0..PROCESS_ROWS)
            .map(|_| {
                let name = Label::builder()
                    .xalign(0.0)
                    .hexpand(true)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .build();
                let value = Label::builder().xalign(1.0).build();
                value.add_css_class("numeric");
                let quit = gtk::Button::from_icon_name("window-close-symbolic");
                quit.add_css_class("flat");
                quit.add_css_class("circular");
                quit.set_valign(Align::Center);
                let target: Rc<RefCell<Option<QuitTarget>>> = Rc::default();
                let clicked = Rc::clone(&target);
                let on_quit = Rc::clone(&on_quit);
                quit.connect_clicked(move |_| {
                    if let Some(target) = clicked.borrow().clone() {
                        on_quit(target);
                    }
                });
                let row = GtkBox::new(Orientation::Horizontal, 8);
                row.append(&name);
                row.append(&value);
                row.append(&quit);
                root.append(&row);
                ProcessWidget {
                    root: row,
                    name,
                    value,
                    quit,
                    target,
                }
            })
            .collect();
        let widget = ProcessesWidget {
            root,
            reading,
            rows,
        };
        widget.show(None);
        widget
    }

    pub fn show(&self, processes: Option<&[ProcessRow]>) {
        self.reading.set_visible(processes.is_none());
        let processes = processes.unwrap_or_default();
        for (index, widget) in self.rows.iter().enumerate() {
            let Some(process) = processes.get(index) else {
                widget.root.set_visible(false);
                widget.target.replace(None);
                continue;
            };
            widget.root.set_visible(true);
            widget.name.set_label(&process.name);
            widget.value.set_label(&process.value);
            let quittable = process.quit.is_some();
            // Hidden by opacity, not removed, so the values stay in one column.
            widget.quit.set_opacity(if quittable { 1.0 } else { 0.0 });
            widget.quit.set_sensitive(quittable);
            widget.quit.set_can_target(quittable);
            widget.quit.set_tooltip_text(
                quittable
                    .then(|| format!("Quit {}…", process.name))
                    .as_deref(),
            );
            widget.target.replace(process.quit.clone());
        }
    }
}
```

*Create* `crates/panel/src/window.rs`:

```rust
//! The window: a CPU | Memory switcher, the two pages, a Refresh footer, and an error page with
//! Retry while the daemon cannot be reached. One window per session: a second launch only
//! brings it forward on the page it names.

use std::cell::{Cell, RefCell};
use std::ffi::OsString;
use std::path::Path;
use std::rc::{Rc, Weak};
use std::time::Duration;

use adw::prelude::*;
use gtk::{Align, Box as GtkBox, Label, Orientation, gio, glib};
use statsbar_core::ipc::{CpuPage, Event, MemoryPage, Page, Request};

use crate::cli;
use crate::link::{Incoming, Link, LinkError, socket_path};
use crate::quit::quit;
use crate::view::{QuitTarget, cpu_view, memory_view};
use crate::widgets::{
    BarWidget, CoresWidget, ProcessesWidget, RingWidget, RowsWidget, section, tone_css,
};

/// The D-Bus name GTK uses to find a running panel; a second launch hands it its arguments.
pub const APP_ID: &str = "io.github.MatrixI.StatsBar";
/// For the handshake and each request. The daemon answers from its main loop within a tick.
const LINK_TIMEOUT: Duration = Duration::from_secs(2);
const WIDTH: i32 = 340;
/// Tall enough for the CPU page of an eight-thread CPU. The window is sized before the first
/// event fills the page, so it cannot take its height from the content.
const HEIGHT: i32 = 860;
const PROC: &str = "/proc";

fn page_name(page: Page) -> &'static str {
    match page {
        Page::Cpu => "cpu",
        Page::Memory => "memory",
    }
}

fn page_of_name(name: &str) -> Option<Page> {
    match name {
        "cpu" => Some(Page::Cpu),
        "memory" => Some(Page::Memory),
        _ => None,
    }
}

pub fn run() -> glib::ExitCode {
    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    app.connect_startup(|_| {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(&tone_css());
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });
    let panel: Rc<RefCell<Option<Rc<Panel>>>> = Rc::default();
    app.connect_command_line(move |app, line| {
        let args: Vec<OsString> = line.arguments().into_iter().skip(1).collect();
        match cli::page_from_args(&args) {
            Ok(page) => {
                let panel = Rc::clone(panel.borrow_mut().get_or_insert_with(|| Panel::new(app)));
                panel.open(page);
                glib::ExitCode::SUCCESS
            }
            Err(message) => {
                eprintln!("statsbar-panel: {message}");
                glib::ExitCode::FAILURE
            }
        }
    });
    app.run()
}

struct CpuWidgets {
    stack: gtk::Stack,
    subtitle: Label,
    rings: [RingWidget; 2],
    details: RowsWidget,
    cores: CoresWidget,
    frequency_title: Label,
    frequency: RowsWidget,
    processes: ProcessesWidget,
}

struct MemoryWidgets {
    stack: gtk::Stack,
    subtitle: Label,
    rings: [RingWidget; 2],
    used: Label,
    bar: BarWidget,
    details: RowsWidget,
    processes: ProcessesWidget,
}

/// A page waits on "Reading…" until its first event, then shows its figures for good.
fn waiting_stack(content: &impl IsA<gtk::Widget>) -> gtk::Stack {
    let reading = Label::new(Some("Reading…"));
    reading.add_css_class("dim-label");
    reading.set_margin_top(48);
    reading.set_margin_bottom(48);
    let stack = gtk::Stack::new();
    stack.add_named(&reading, Some("waiting"));
    stack.add_named(content, Some("content"));
    stack.set_visible_child_name("waiting");
    stack
}

fn page_box() -> GtkBox {
    let content = GtkBox::new(Orientation::Vertical, 14);
    for set in [GtkBox::set_margin_top, GtkBox::set_margin_bottom] {
        set(&content, 14);
    }
    content.set_margin_start(14);
    content.set_margin_end(14);
    content
}

fn subtitle_label() -> Label {
    let label = Label::builder().halign(Align::Center).wrap(true).build();
    label.add_css_class("dim-label");
    label
}

fn rings_box(rings: &[RingWidget; 2]) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 28);
    row.set_halign(Align::Center);
    for ring in rings {
        row.append(&ring.root);
    }
    row
}

fn scrolled(child: &impl IsA<gtk::Widget>) -> gtk::ScrolledWindow {
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(child)
        .build()
}

impl CpuWidgets {
    fn new(on_quit: Rc<dyn Fn(QuitTarget)>) -> CpuWidgets {
        let content = page_box();
        let subtitle = subtitle_label();
        let rings = [RingWidget::new(), RingWidget::new()];
        let details = RowsWidget::new();
        let cores = CoresWidget::new();
        let frequency_title = section("Frequency");
        let frequency = RowsWidget::new();
        let processes = ProcessesWidget::new("Usage", on_quit);
        content.append(&subtitle);
        content.append(&rings_box(&rings));
        content.append(&section("Details"));
        content.append(&details.root);
        content.append(&section("Cores"));
        content.append(&cores.root);
        content.append(&frequency_title);
        content.append(&frequency.root);
        content.append(&section("Top processes"));
        content.append(&processes.root);
        CpuWidgets {
            stack: waiting_stack(&content),
            subtitle,
            rings,
            details,
            cores,
            frequency_title,
            frequency,
            processes,
        }
    }

    fn show(&self, page: &CpuPage) {
        let view = cpu_view(page);
        self.subtitle.set_visible(view.subtitle.is_some());
        self.subtitle
            .set_label(view.subtitle.as_deref().unwrap_or_default());
        for (widget, ring) in self.rings.iter().zip(&view.rings) {
            widget.show(ring);
        }
        self.details.show(&view.details);
        self.cores.show(&view.cores);
        self.frequency_title.set_visible(view.frequency.is_some());
        self.frequency.show(view.frequency.as_slice());
        self.processes.show(view.processes.as_deref());
        self.stack.set_visible_child_name("content");
    }
}

impl MemoryWidgets {
    fn new(on_quit: Rc<dyn Fn(QuitTarget)>) -> MemoryWidgets {
        let content = page_box();
        let subtitle = subtitle_label();
        let rings = [RingWidget::new(), RingWidget::new()];
        let used = Label::builder().xalign(1.0).hexpand(true).build();
        used.add_css_class("title-4");
        used.add_css_class("numeric");
        let used_row = GtkBox::new(Orientation::Horizontal, 8);
        used_row.append(&Label::new(Some("Used")));
        used_row.append(&used);
        let bar = BarWidget::new();
        let details = RowsWidget::new();
        let processes = ProcessesWidget::new("Memory", on_quit);
        content.append(&subtitle);
        content.append(&rings_box(&rings));
        content.append(&used_row);
        content.append(&bar.area);
        content.append(&section("Details"));
        content.append(&details.root);
        content.append(&section("Top processes"));
        content.append(&processes.root);
        MemoryWidgets {
            stack: waiting_stack(&content),
            subtitle,
            rings,
            used,
            bar,
            details,
            processes,
        }
    }

    fn show(&self, page: &MemoryPage) {
        let view = memory_view(page);
        self.subtitle.set_label(&view.subtitle);
        for (widget, ring) in self.rings.iter().zip(&view.rings) {
            widget.show(ring);
        }
        self.used.set_label(&view.used);
        self.bar.show(&view.bar);
        self.details.show(&view.details);
        self.processes.show(view.processes.as_deref());
        self.stack.set_visible_child_name("content");
    }
}

struct Panel {
    window: adw::ApplicationWindow,
    toasts: adw::ToastOverlay,
    /// "pages" or "error".
    root: gtk::Stack,
    pages: adw::ViewStack,
    status: adw::StatusPage,
    /// Only while connected: there is nobody to ask otherwise.
    refresh: gtk::Button,
    cpu: CpuWidgets,
    memory: MemoryWidgets,
    link: RefCell<Option<Link>>,
    /// Raised by each connection attempt, so the events of an older one are ignored.
    generation: Cell<u64>,
    connecting: Cell<bool>,
}

impl Panel {
    fn new(app: &adw::Application) -> Rc<Panel> {
        Rc::new_cyclic(|weak: &Weak<Panel>| {
            let on_quit: Rc<dyn Fn(QuitTarget)> = {
                let weak = weak.clone();
                Rc::new(move |target| {
                    if let Some(panel) = weak.upgrade() {
                        panel.confirm_quit(target);
                    }
                })
            };
            let cpu = CpuWidgets::new(Rc::clone(&on_quit));
            let memory = MemoryWidgets::new(on_quit);
            let pages = adw::ViewStack::new();
            pages.add_titled_with_icon(
                &scrolled(&cpu.stack),
                Some(page_name(Page::Cpu)),
                "CPU",
                "computer-chip-symbolic",
            );
            pages.add_titled_with_icon(
                &scrolled(&memory.stack),
                Some(page_name(Page::Memory)),
                "Memory",
                "drive-harddisk-solidstate-symbolic",
            );
            let retry = gtk::Button::builder()
                .label("Retry")
                .halign(Align::Center)
                .build();
            retry.add_css_class("pill");
            retry.add_css_class("suggested-action");
            let status = adw::StatusPage::builder()
                .icon_name("dialog-warning-symbolic")
                .title("StatsBar is not available")
                .child(&retry)
                .build();
            let root = gtk::Stack::new();
            root.add_named(&pages, Some("pages"));
            root.add_named(&status, Some("error"));
            let refresh = gtk::Button::with_label("Refresh");
            refresh.set_sensitive(false);
            let footer = gtk::ActionBar::new();
            footer.pack_start(&refresh);
            let header = adw::HeaderBar::builder()
                .title_widget(
                    &adw::ViewSwitcher::builder()
                        .stack(&pages)
                        .policy(adw::ViewSwitcherPolicy::Wide)
                        .build(),
                )
                .build();
            let toolbar = adw::ToolbarView::new();
            toolbar.add_top_bar(&header);
            toolbar.set_content(Some(&root));
            toolbar.add_bottom_bar(&footer);
            let toasts = adw::ToastOverlay::new();
            toasts.set_child(Some(&toolbar));
            let window = adw::ApplicationWindow::builder()
                .application(app)
                .title("StatsBar")
                .default_width(WIDTH)
                .default_height(HEIGHT)
                .content(&toasts)
                .build();
            {
                let weak = weak.clone();
                retry.connect_clicked(move |_| {
                    if let Some(panel) = weak.upgrade() {
                        panel.connect();
                    }
                });
            }
            {
                let weak = weak.clone();
                refresh.connect_clicked(move |_| {
                    if let Some(panel) = weak.upgrade() {
                        panel.send(&Request::Refresh);
                    }
                });
            }
            {
                let weak = weak.clone();
                pages.connect_visible_child_name_notify(move |pages| {
                    let page = pages.visible_child_name().as_deref().and_then(page_of_name);
                    if let (Some(panel), Some(page)) = (weak.upgrade(), page) {
                        panel.send(&Request::Subscribe { page });
                    }
                });
            }
            Panel {
                window,
                toasts,
                root,
                pages,
                status,
                refresh,
                cpu,
                memory,
                link: RefCell::new(None),
                generation: Cell::new(0),
                connecting: Cell::new(false),
            }
        })
    }

    fn current_page(&self) -> Page {
        self.pages
            .visible_child_name()
            .as_deref()
            .and_then(page_of_name)
            .unwrap_or(Page::Cpu)
    }

    fn open(self: &Rc<Panel>, page: Page) {
        // Subscribes through the switcher's notify when the page changes.
        self.pages.set_visible_child_name(page_name(page));
        self.window.present();
        if self.link.borrow().is_none() && !self.connecting.get() {
            self.connect();
        }
    }

    fn connect(self: &Rc<Panel>) {
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        self.connecting.set(true);
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let result = gio::spawn_blocking(|| {
                let path = socket_path(std::env::var_os("XDG_RUNTIME_DIR").as_deref())?;
                Link::connect(&path, LINK_TIMEOUT)
            })
            .await
            .unwrap_or_else(|_| {
                Err(LinkError::Handshake(
                    "the connecting thread panicked".to_owned(),
                ))
            });
            let Some(panel) = weak.upgrade() else { return };
            if panel.generation.get() != generation {
                return;
            }
            panel.connecting.set(false);
            match result {
                Ok(link) => panel.attach(link, generation),
                Err(error) => panel.fail(generation, &error.to_string()),
            }
        });
    }

    fn attach(self: &Rc<Panel>, link: Link, generation: u64) {
        let events = link.events.clone();
        self.link.replace(Some(link));
        self.root.set_visible_child_name("pages");
        self.refresh.set_sensitive(true);
        self.send(&Request::Subscribe {
            page: self.current_page(),
        });
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            while let Ok(incoming) = events.recv().await {
                let Some(panel) = weak.upgrade() else { return };
                if panel.generation.get() != generation {
                    return;
                }
                let event = match incoming {
                    Incoming::Event(event) => event,
                    Incoming::Closed(closed) => {
                        panel.fail(generation, &closed.to_string());
                        return;
                    }
                };
                match *event {
                    Event::Cpu(page) => panel.cpu.show(&page),
                    Event::Memory(page) => panel.memory.show(&page),
                    // The reader turns an error line into `Closed`.
                    Event::Error { .. } => {}
                }
            }
        });
    }

    fn send(&self, request: &Request) {
        let result = match self.link.borrow().as_ref() {
            Some(link) => link.send(request),
            None => return,
        };
        if let Err(error) = result {
            self.fail(
                self.generation.get(),
                &format!("the connection to StatsBar failed: {error}"),
            );
        }
    }

    fn fail(&self, generation: u64, message: &str) {
        if self.generation.get() != generation {
            return;
        }
        // Dropping the link closes the socket and ends its reader thread.
        self.link.replace(None);
        self.refresh.set_sensitive(false);
        self.status.set_description(Some(message));
        self.root.set_visible_child_name("error");
    }

    fn confirm_quit(self: &Rc<Panel>, target: QuitTarget) {
        let dialog = adw::AlertDialog::new(
            Some(&format!("Quit {}?", target.name)),
            Some(
                "It is asked to close, as when you close it yourself. Unsaved work in it may be lost.",
            ),
        );
        dialog.add_responses(&[("cancel", "Cancel"), ("quit", "Quit")]);
        dialog.set_response_appearance("quit", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let Some(window) = weak.upgrade().map(|panel| panel.window.clone()) else {
                return;
            };
            let response = dialog.choose_future(Some(&window)).await;
            let Some(panel) = weak.upgrade() else { return };
            if response != "quit" {
                return;
            }
            let text = match quit(Path::new(PROC), &target) {
                Ok(()) => format!("Asked {} to quit", target.name),
                Err(error) => format!("Could not quit {}: {error}", target.name),
            };
            panel.toasts.add_toast(adw::Toast::new(&text));
        });
    }
}
```

*Create* `crates/panel/src/main.rs`:

```rust
fn main() -> gtk::glib::ExitCode {
    statsbar_panel::window::run()
}
```

- [x] **Step 3: Check.** `cargo build -p statsbar-panel`, `cargo clippy -p statsbar-panel --all-targets -- -D warnings`, `cargo test -p statsbar-panel` (Tasks 1–4 still pass), `cargo audit` (the GTK tree is new; expect only the 3 known `zbus` unmaintained warnings). Then a smoke run with no daemon: `XDG_RUNTIME_DIR=$(mktemp -d) cargo run -p statsbar-panel` shows "StatsBar is not available" with Retry and a disabled Refresh; close it.

- [x] **Step 4: Checkpoint.** Ready to commit: `crates/panel/{Cargo.toml,src/lib.rs,src/widgets.rs,src/window.rs,src/main.rs}`, `Cargo.lock` — "feat(panel): show the CPU and Memory pages in a libadwaita window"

### Task 6: "Open details…" launches the panel

**Type:** feature · **Test level:** unit (tray, launcher with a stand-in script) + integration (real daemon on a private bus, D-Bus menu)

**Files:** Create `crates/daemon/src/launch.rs`, `crates/daemon/tests/launch.rs`. Modify `crates/daemon/src/{lib.rs,app.rs,tray.rs,run.rs}`, `crates/daemon/tests/{tray.rs,daemon_ipc.rs}`.

**Interfaces:** Produces `app::Command::OpenPage(Page)`; `tray::{MetricItem::new(ReaderId, Sender<Command>), CPU_MENU_ROWS = 5, MEMORY_MENU_ROWS = 7, OPEN_DETAILS}`; `launch::{Launcher::new(PathBuf, Option<OsString>), Launcher::beside(PathBuf, Option<OsString>), open(&mut self, Page) -> io::Result<()>, reap(&mut self), running(&self) -> usize, MAX_RUNNING = 4, RENDERER}`.

- [x] **Step 1: Write the failing tests.** *Apply to* the tests:

```diff
diff --git a/crates/daemon/tests/daemon_ipc.rs b/crates/daemon/tests/daemon_ipc.rs
index e2b75c0..ed024bb 100644
--- a/crates/daemon/tests/daemon_ipc.rs
+++ b/crates/daemon/tests/daemon_ipc.rs
@@ -259,6 +259,56 @@ impl Session {
         });
     }
 
+    /// The menu's layout revision and its entries' ids, labels and visibility, in id order.
+    fn menu(&self, item: &str) -> (u32, Vec<(i32, String, bool)>) {
+        async_io::block_on(async {
+            let layout = self
+                .watcher
+                .call_method(
+                    Some(item),
+                    "/MenuBar",
+                    Some(DBUSMENU),
+                    "GetLayout",
+                    &(0i32, -1i32, Vec::<&str>::new()),
+                )
+                .await
+                .expect("read the layout");
+            type Layout = (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>);
+            let (revision, _): (u32, Layout) = layout.body().deserialize().expect("a layout");
+            let reply = self
+                .watcher
+                .call_method(
+                    Some(item),
+                    "/MenuBar",
+                    Some(DBUSMENU),
+                    "GetGroupProperties",
+                    &(Vec::<i32>::new(), vec!["label", "visible"]),
+                )
+                .await
+                .expect("list the menu");
+            let rows: Vec<(i32, HashMap<String, OwnedValue>)> =
+                reply.body().deserialize().expect("menu rows");
+            let mut entries: Vec<(i32, String, bool)> = rows
+                .into_iter()
+                .map(|(id, properties)| {
+                    let label = properties
+                        .get("label")
+                        .and_then(|value| <&str>::try_from(value).ok())
+                        .unwrap_or_default()
+                        .to_owned();
+                    // dbusmenu leaves out a property at its default, and `visible` defaults to true.
+                    let visible = properties
+                        .get("visible")
+                        .and_then(|value| bool::try_from(value).ok())
+                        .unwrap_or(true);
+                    (id, label, visible)
+                })
+                .collect();
+            entries.sort();
+            (revision, entries)
+        })
+    }
+
     /// Follows who owns `name`, from now on.
     fn owner_changes(&self, name: &str) -> NameOwnerChangedStream {
         async_io::block_on(async {
@@ -593,3 +643,34 @@ fn a_runtime_directory_others_can_read_is_refused() {
     );
     assert!(!session.dir.join("statsbar.sock").exists(), "nor a socket");
 }
+
+#[test]
+fn the_cpu_menu_keeps_its_ids_while_its_rows_fill_in() {
+    let session = Session::new("menu", CPU_SHOWN);
+    let (_daemon, _log, _hub) = session.start();
+    let cpu = session
+        .registered
+        .recv_timeout(LIMIT)
+        .expect("the CPU item registers");
+    let (revision, before) = session.menu(&cpu);
+    // The first reading only has a baseline; rows come with the second.
+    let mut panel = session.panel();
+    panel.send(WATCH_CPU);
+    panel.cpu_until(|page| page.load.is_some());
+    panel.cpu_until(|_| true);
+    let (revision_after, after) = session.menu(&cpu);
+    assert_eq!(revision_after, revision, "the layout never changed");
+    let ids =
+        |entries: &[(i32, String, bool)]| entries.iter().map(|entry| entry.0).collect::<Vec<_>>();
+    assert_eq!(ids(&after), ids(&before));
+    let open = after
+        .iter()
+        .find(|(_, label, _)| label == "Open details…")
+        .expect("Open details…");
+    assert!(open.2, "Open details… is shown");
+    let usage = after
+        .iter()
+        .find(|(_, label, _)| label.starts_with("Usage "))
+        .expect("the usage row");
+    assert!(usage.2, "the filled rows are shown");
+}
diff --git a/crates/daemon/tests/tray.rs b/crates/daemon/tests/tray.rs
index 4c4361b..5cf1b11 100644
--- a/crates/daemon/tests/tray.rs
+++ b/crates/daemon/tests/tray.rs
@@ -6,7 +6,8 @@ use statsbard::app::BatteryLabel;
 use statsbard::app::Command;
 use statsbard::app::READERS;
 use statsbard::config::Items;
-use statsbard::tray::{self, Hub, Look, MetricItem};
+use statsbard::summary;
+use statsbard::tray::{self, CPU_MENU_ROWS, Hub, Look, MEMORY_MENU_ROWS, MetricItem, OPEN_DETAILS};
 
 fn hub(capacity: usize) -> (Hub, async_channel::Receiver<Command>) {
     let (sender, receiver) = async_channel::bounded(capacity);
@@ -20,6 +21,11 @@ fn hub(capacity: usize) -> (Hub, async_channel::Receiver<Command>) {
     (Hub::new(sender, items, true), receiver)
 }
 
+fn item(reader: ReaderId) -> (MetricItem, async_channel::Receiver<Command>) {
+    let (sender, receiver) = async_channel::bounded(1);
+    (MetricItem::new(reader, sender), receiver)
+}
+
 fn labels<T>(menu: &[MenuItem<T>]) -> Vec<String> {
     menu.iter()
         .map(|item| match item {
@@ -117,7 +123,7 @@ fn a_full_command_queue_drops_the_click_instead_of_blocking_the_tray_thread() {
 
 #[test]
 fn a_metric_item_shows_its_icon_alone_until_its_first_value() {
-    let mut item = MetricItem::new(ReaderId::Cpu);
+    let (mut item, _receiver) = item(ReaderId::Cpu);
     assert_eq!(item.icon_name(), "computer-chip-symbolic");
     assert_eq!(item.x_ayatana_label(), "");
     assert_eq!(item.x_ayatana_label_guide(), "100%");
@@ -141,7 +147,7 @@ fn every_metric_has_its_yaru_icon() {
             "network-transmit-receive-symbolic",
         ]
     );
-    let icons: Vec<String> = READERS.map(|id| MetricItem::new(id).icon_name()).to_vec();
+    let icons: Vec<String> = READERS.map(|id| item(id).0.icon_name()).to_vec();
     assert_eq!(
         icons,
         READERS.map(|id| tray::metric_icon(id).to_owned()).to_vec()
@@ -186,17 +192,13 @@ fn each_guide_is_its_widest_label() {
             "100%"
         };
         assert_eq!(tray::label_guide(id), expected, "{id:?}");
-        assert_eq!(
-            MetricItem::new(id).x_ayatana_label_guide(),
-            expected,
-            "{id:?}"
-        );
+        assert_eq!(item(id).0.x_ayatana_label_guide(), expected, "{id:?}");
     }
 }
 
 #[test]
 fn metric_item_ids_are_stable_and_rows_are_not_clickable() {
-    let ids: Vec<String> = READERS.map(|id| MetricItem::new(id).id()).to_vec();
+    let ids: Vec<String> = READERS.map(|id| item(id).0.id()).to_vec();
     assert_eq!(
         ids,
         [
@@ -207,11 +209,11 @@ fn metric_item_ids_are_stable_and_rows_are_not_clickable() {
             "statsbar-network",
         ]
     );
-    let mut memory = MetricItem::new(ReaderId::Memory);
-    memory.rows = vec!["App 6.00 GB".to_owned()];
-    match &memory.menu()[0] {
+    let (mut fan, _receiver) = item(ReaderId::Fan);
+    fan.rows = vec!["Speed 3000 RPM".to_owned()];
+    match &fan.menu()[0] {
         MenuItem::Standard(item) => {
-            assert_eq!(item.label, "App 6.00 GB");
+            assert_eq!(item.label, "Speed 3000 RPM");
             assert!(!item.enabled);
         }
         other => panic!("expected a row, got {other:?}"),
@@ -270,3 +272,102 @@ fn the_network_item_labels_the_download_and_titles_both_rates() {
         }
     );
 }
+
+/// (label, enabled, visible) of each menu entry; a separator reads as `---`.
+fn entries(menu: &[MenuItem<MetricItem>]) -> Vec<(String, bool, bool)> {
+    menu.iter()
+        .map(|entry| match entry {
+            MenuItem::Standard(item) => (item.label.clone(), item.enabled, item.visible),
+            MenuItem::Separator => ("---".to_owned(), false, true),
+            other => panic!("unexpected {other:?}"),
+        })
+        .collect()
+}
+
+#[test]
+fn cpu_and_memory_menus_open_their_page_above_reserved_rows() {
+    for (reader, places) in [
+        (ReaderId::Cpu, CPU_MENU_ROWS),
+        (ReaderId::Memory, MEMORY_MENU_ROWS),
+    ] {
+        let (item, _receiver) = item(reader);
+        let menu = entries(&item.menu());
+        assert_eq!(menu.len(), 2 + places, "{reader:?}");
+        assert_eq!(menu[0], (OPEN_DETAILS.to_owned(), true, true));
+        assert_eq!(menu[1].0, "---");
+        // Kept but hidden until the first value, so showing them changes no menu id.
+        assert!(
+            menu[2..]
+                .iter()
+                .all(|(_, enabled, visible)| !enabled && !visible),
+            "{reader:?}"
+        );
+    }
+}
+
+#[test]
+fn rows_fill_their_places_and_the_rest_stay_hidden() {
+    let (mut memory, _receiver) = item(ReaderId::Memory);
+    memory.rows = summary::memory_rows(&statsbar_core::memory::MemoryBuckets::default());
+    let menu = entries(&memory.menu());
+    assert_eq!(menu.len(), 2 + MEMORY_MENU_ROWS);
+    assert_eq!(
+        menu[2],
+        ("Used 0.00 GB of 0.00 GB (0 %)".to_owned(), false, true)
+    );
+    assert_eq!(menu[7], ("Swap 0.00 GB".to_owned(), false, true));
+    // Compressed's place, while zswap is empty.
+    assert_eq!(menu[8], (String::new(), false, false));
+}
+
+#[test]
+fn every_row_the_summaries_give_has_a_place() {
+    let busy = statsbar_core::cpu::CpuLoad {
+        system_percent: 1.0,
+        user_percent: 1.0,
+        idle_percent: 98.0,
+        per_core_busy: vec![2.0],
+    };
+    assert_eq!(summary::cpu_rows(&busy).len(), CPU_MENU_ROWS);
+    let zswap = statsbar_core::memory::MemoryBuckets {
+        compressed: 1,
+        ..Default::default()
+    };
+    assert_eq!(summary::memory_rows(&zswap).len(), MEMORY_MENU_ROWS);
+}
+
+#[test]
+fn open_details_sends_its_page_and_other_items_have_none() {
+    for (reader, page) in [
+        (ReaderId::Cpu, statsbar_core::ipc::Page::Cpu),
+        (ReaderId::Memory, statsbar_core::ipc::Page::Memory),
+    ] {
+        let (mut item, receiver) = item(reader);
+        let menu = item.menu();
+        let MenuItem::Standard(open) = &menu[0] else {
+            panic!("expected Open details");
+        };
+        (open.activate)(&mut item);
+        assert_eq!(receiver.try_recv(), Ok(Command::OpenPage(page)));
+    }
+    for reader in [ReaderId::Battery, ReaderId::Fan, ReaderId::Network] {
+        let (item, _receiver) = item(reader);
+        assert!(item.menu().is_empty(), "{reader:?}");
+    }
+}
+
+#[test]
+fn a_full_command_queue_drops_open_details_too() {
+    let (mut item, receiver) = item(ReaderId::Cpu);
+    let menu = item.menu();
+    let MenuItem::Standard(open) = &menu[0] else {
+        panic!("expected Open details");
+    };
+    (open.activate)(&mut item);
+    (open.activate)(&mut item);
+    assert_eq!(
+        receiver.try_recv(),
+        Ok(Command::OpenPage(statsbar_core::ipc::Page::Cpu))
+    );
+    assert!(receiver.try_recv().is_err());
+}
```

*Create* `crates/daemon/tests/launch.rs`:

```rust
//! Launching a stand-in panel: a shell script that records its arguments and environment.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use statsbar_core::ipc::Page;
use statsbard::launch::{Launcher, MAX_RUNNING, RENDERER};

/// Only fails a hung test.
const LIMIT: Duration = Duration::from_secs(10);

/// Held while a stand-in is written and while one is started. A child forked by another test
/// while a script is open for writing keeps that file open until it execs, and running the
/// script meanwhile fails with ETXTBSY.
static FORKS: Mutex<()> = Mutex::new(());

fn open(launcher: &mut Launcher, page: Page) -> std::io::Result<()> {
    let _forks = FORKS.lock().unwrap();
    launcher.open(page)
}

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        let path =
            std::env::temp_dir().join(format!("statsbard-launch-{}-{name}", std::process::id()));
        // Left over only by a killed earlier run.
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).unwrap();
        Dir(path)
    }

    /// A panel that writes `$@` and `GSK_RENDERER` to `out`, then waits for `release` to exist,
    /// or for the directory to go, so no stand-in outlives its test.
    fn panel(&self) -> PathBuf {
        let _forks = FORKS.lock().unwrap();
        let script = self.0.join("statsbar-panel");
        fs::write(
            &script,
            format!(
                "#!/bin/sh\necho \"$* ${{GSK_RENDERER-unset}}\" >> {out}\nwhile [ -d {dir} ] && [ ! -e {release} ]; do sleep 0.01; done\n",
                out = self.out().display(),
                dir = self.0.display(),
                release = self.0.join("release").display(),
            ),
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        script
    }

    fn out(&self) -> PathBuf {
        self.0.join("out")
    }

    fn release(&self) {
        fs::write(self.0.join("release"), "").unwrap();
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        self.release();
        // Best effort: a leftover directory in the temp dir harms nothing.
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Waits until `out` has `lines` lines.
fn recorded(out: &Path, lines: usize) -> Vec<String> {
    let deadline = Instant::now() + LIMIT;
    loop {
        let text = fs::read_to_string(out).unwrap_or_default();
        let got: Vec<String> = text.lines().map(str::to_owned).collect();
        if got.len() >= lines {
            return got;
        }
        assert!(Instant::now() < deadline, "the panel never ran: {got:?}");
        std::thread::yield_now();
    }
}

fn reaped(launcher: &mut Launcher) {
    let deadline = Instant::now() + LIMIT;
    while launcher.running() > 0 {
        assert!(Instant::now() < deadline, "the children were never reaped");
        launcher.reap();
        std::thread::yield_now();
    }
}

#[test]
fn the_page_is_passed_with_the_default_renderer() {
    let dir = Dir::new("page");
    let mut launcher = Launcher::new(dir.panel(), None);
    open(&mut launcher, Page::Memory).unwrap();
    open(&mut launcher, Page::Cpu).unwrap();
    let mut lines = recorded(&dir.out(), 2);
    lines.sort();
    assert_eq!(
        lines,
        [
            format!("--page cpu {RENDERER}"),
            format!("--page memory {RENDERER}")
        ]
    );
}

#[test]
fn a_renderer_the_user_chose_is_inherited_not_replaced() {
    let dir = Dir::new("inherit");
    let mut launcher = Launcher::new(dir.panel(), Some("ngl".into()));
    open(&mut launcher, Page::Cpu).unwrap();
    assert_eq!(recorded(&dir.out(), 1), ["--page cpu ngl"]);
}

#[test]
fn exited_panels_are_reaped_and_free_their_place() {
    let dir = Dir::new("reap");
    let mut launcher = Launcher::new(dir.panel(), None);
    for _ in 0..MAX_RUNNING {
        open(&mut launcher, Page::Cpu).unwrap();
    }
    assert_eq!(launcher.running(), MAX_RUNNING);
    let refused = open(&mut launcher, Page::Cpu).unwrap_err();
    assert_eq!(refused.to_string(), "4 panels are still running");
    dir.release();
    reaped(&mut launcher);
    open(&mut launcher, Page::Cpu).unwrap();
    assert_eq!(launcher.running(), 1);
    // Before the directory goes, or the last stand-in cannot read its script.
    recorded(&dir.out(), MAX_RUNNING + 1);
}

#[test]
fn a_missing_panel_is_an_error_and_takes_no_place() {
    let dir = Dir::new("missing");
    let mut launcher = Launcher::beside(dir.0.join("statsbard"), None);
    let error = open(&mut launcher, Page::Cpu).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert_eq!(launcher.running(), 0);
}

#[test]
fn the_panel_is_looked_for_beside_the_daemon() {
    let dir = Dir::new("beside");
    dir.panel();
    let mut launcher = Launcher::beside(dir.0.join("statsbard"), None);
    open(&mut launcher, Page::Memory).unwrap();
    assert_eq!(
        recorded(&dir.out(), 1),
        [format!("--page memory {RENDERER}")]
    );
}
```

- [x] **Step 2: Run, expect FAIL.** `cargo test -p statsbard --test tray --test launch --test daemon_ipc` fails to compile: no `statsbard::launch`, no `CPU_MENU_ROWS`, `MetricItem::new` takes one argument. (Checked in the scratch run: with `reserved_rows` returning 0 for CPU, `the_cpu_menu_keeps_its_ids_while_its_rows_fill_in` fails with `the layout never changed  left: 1 right: 0`.)

- [x] **Step 3: Implement.** *Apply to* the daemon:

```diff
diff --git a/crates/daemon/src/app.rs b/crates/daemon/src/app.rs
index 3195c8d..6acf1a4 100644
--- a/crates/daemon/src/app.rs
+++ b/crates/daemon/src/app.rs
@@ -42,11 +42,13 @@ pub fn page_of(id: ReaderId) -> Option<Page> {
     PAGES.into_iter().find(|&page| reader_of(page) == id)
 }
 
-/// A click in the hub's menu, sent from the `ksni` thread to the main loop.
+/// A menu click, sent from the `ksni` thread to the main loop.
 #[derive(Debug, Clone, Copy, PartialEq, Eq)]
 pub enum Command {
     Toggle(ReaderId),
     ToggleLaunchAtLogin,
+    /// "Open details…" in a metric's menu.
+    OpenPage(Page),
     Quit,
 }
 
diff --git a/crates/daemon/src/lib.rs b/crates/daemon/src/lib.rs
index 5713732..08b39f8 100644
--- a/crates/daemon/src/lib.rs
+++ b/crates/daemon/src/lib.rs
@@ -7,6 +7,7 @@ pub mod autostart;
 pub mod config;
 pub mod fsutil;
 pub mod ipc;
+pub mod launch;
 pub mod pages;
 pub mod paths;
 pub mod power;
diff --git a/crates/daemon/src/run.rs b/crates/daemon/src/run.rs
index 95d2765..d08ba88 100644
--- a/crates/daemon/src/run.rs
+++ b/crates/daemon/src/run.rs
@@ -9,7 +9,7 @@ use std::io;
 use std::path::PathBuf;
 use std::time::{Duration, Instant};
 
-use async_channel::{Receiver, Sender};
+use async_channel::{Receiver, Sender, WeakSender};
 use futures_lite::{StreamExt, future};
 use ksni::{Handle, TrayMethods};
 use statsbar_core::ipc::{Event, Page};
@@ -23,6 +23,7 @@ use statsbar_readers::network::NetworkReader;
 use crate::app::{App, Command, PAGES, READERS, page_of, reader_of, slot};
 use crate::config::Items;
 use crate::ipc::{Change, Options, Server};
+use crate::launch::Launcher;
 use crate::pages::{Pages, System};
 use crate::power::{ProfileWatch, is_power_saver};
 use crate::tray::{Hub, Look, MetricItem, battery_look, network_look, percent_look};
@@ -103,9 +104,11 @@ pub async fn run() -> Result<(), BoxError> {
 
     let (sender, receiver) = async_channel::bounded(COMMAND_CAPACITY);
     let app = App::new(loaded.config, launch_at_login);
+    let launcher = Launcher::beside(std::env::current_exe()?, std::env::var_os("GSK_RENDERER"));
     let mut daemon = Daemon::start(
         app,
         sender,
+        launcher,
         server,
         pages,
         Files {
@@ -196,6 +199,10 @@ struct Daemon {
     app: App,
     files: Files,
     hub: Handle<Hub>,
+    /// For the metric items' "Open details…". Weak, so the command channel still closes, and the
+    /// loop still ends, once the hub and every item have dropped their senders.
+    commands: WeakSender<Command>,
+    launcher: Launcher,
     hub_shown: Option<(Vec<String>, Items, bool)>,
     /// Indexed by `app::slot`.
     items: [Option<Handle<MetricItem>>; 5],
@@ -217,16 +224,20 @@ impl Daemon {
     async fn start(
         app: App,
         commands: Sender<Command>,
+        launcher: Launcher,
         server: Server,
         pages: Pages,
         files: Files,
     ) -> Result<Daemon, BoxError> {
+        let weak_commands = commands.downgrade();
         let hub = Hub::new(commands, app.config.items.clone(), app.launch_at_login);
         let hub = bounded("hub item spawn", SETUP_TIMEOUT, hub.spawn()).await??;
         let mut daemon = Daemon {
             app,
             files,
             hub,
+            commands: weak_commands,
+            launcher,
             hub_shown: None,
             items: Default::default(),
             readers: Readers::default(),
@@ -274,6 +285,7 @@ impl Daemon {
         loop {
             let now = elapsed_nanos(start);
             self.sync_pages();
+            self.launcher.reap();
             for id in self.app.due(now) {
                 self.sample(id, now).await;
             }
@@ -413,6 +425,7 @@ impl Daemon {
         match change {
             Change::Subscribed(page) => {
                 self.sync_pages();
+                self.launcher.reap();
                 // What is known already, so a shown item's page does not open empty. Nothing is
                 // read for it: a read here would be outside the cadence.
                 if let Some(event) = &self.published[page_slot(page)] {
@@ -515,6 +528,11 @@ impl Daemon {
         match command {
             Command::Toggle(id) => self.toggle(id).await,
             Command::ToggleLaunchAtLogin => self.toggle_launch_at_login(),
+            Command::OpenPage(page) => {
+                if let Err(error) = self.launcher.open(page) {
+                    eprintln!("statsbard: opening the panel failed: {error}");
+                }
+            }
             Command::Quit => {}
         }
     }
@@ -533,10 +551,14 @@ impl Daemon {
     }
 
     async fn show_item(&mut self, id: ReaderId) {
+        // `None` only once every sender is gone, when the loop is about to end anyway.
+        let Some(commands) = self.commands.upgrade() else {
+            return;
+        };
         match bounded(
             "metric item spawn",
             SETUP_TIMEOUT,
-            MetricItem::new(id).spawn(),
+            MetricItem::new(id, commands).spawn(),
         )
         .await
         {
diff --git a/crates/daemon/src/tray.rs b/crates/daemon/src/tray.rs
index 50f1681..6ca7395 100644
--- a/crates/daemon/src/tray.rs
+++ b/crates/daemon/src/tray.rs
@@ -13,7 +13,7 @@ use ksni::{Category, MenuItem};
 use statsbar_core::schedule::ReaderId;
 use statsbar_core::throughput::Reading;
 
-use crate::app::{BatteryLabel, Command, READERS};
+use crate::app::{BatteryLabel, Command, READERS, page_of};
 use crate::config::Items;
 use crate::summary::{self, Overview};
 
@@ -75,6 +75,33 @@ fn row<T>(text: &str) -> MenuItem<T> {
     .into()
 }
 
+/// A place kept for a row that has no value yet. Showing it later changes a property, not the
+/// layout, so `ksni` keeps every menu id and a click on "Open details…" is never lost.
+fn reserved_row<T>() -> MenuItem<T> {
+    StandardItem {
+        enabled: false,
+        visible: false,
+        ..Default::default()
+    }
+    .into()
+}
+
+/// The most rows `summary::cpu_rows` gives.
+pub const CPU_MENU_ROWS: usize = 5;
+/// The most rows `summary::memory_rows` gives: six, and Compressed while zswap holds pages.
+pub const MEMORY_MENU_ROWS: usize = 7;
+pub const OPEN_DETAILS: &str = "Open details…";
+
+/// Rows kept from the start in a menu that has "Open details…". Other menus have no clickable
+/// item, so their layout may change.
+fn reserved_rows(reader: ReaderId) -> usize {
+    match reader {
+        ReaderId::Cpu => CPU_MENU_ROWS,
+        ReaderId::Memory => MEMORY_MENU_ROWS,
+        ReaderId::Battery | ReaderId::Fan | ReaderId::Network => 0,
+    }
+}
+
 pub struct MetricItem {
     pub reader: ReaderId,
     /// A themed icon name.
@@ -83,16 +110,18 @@ pub struct MetricItem {
     pub label: String,
     pub title: String,
     pub rows: Vec<String>,
+    commands: Sender<Command>,
 }
 
 impl MetricItem {
-    pub fn new(reader: ReaderId) -> MetricItem {
+    pub fn new(reader: ReaderId, commands: Sender<Command>) -> MetricItem {
         MetricItem {
             reader,
             icon: metric_icon(reader).to_owned(),
             label: String::new(),
             title: name(reader).to_owned(),
             rows: Vec::new(),
+            commands,
         }
     }
 }
@@ -141,7 +170,34 @@ impl ksni::Tray for MetricItem {
     }
 
     fn menu(&self) -> Vec<MenuItem<Self>> {
-        self.rows.iter().map(|text| row(text)).collect()
+        let Some(page) = page_of(self.reader) else {
+            return self.rows.iter().map(|text| row(text)).collect();
+        };
+        let mut menu: Vec<MenuItem<Self>> = vec![
+            StandardItem {
+                label: OPEN_DETAILS.to_owned(),
+                activate: Box::new(move |item: &mut MetricItem| {
+                    send(&item.commands, Command::OpenPage(page));
+                }),
+                ..Default::default()
+            }
+            .into(),
+            MenuItem::Separator,
+        ];
+        let places = reserved_rows(self.reader).max(self.rows.len());
+        menu.extend((0..places).map(|index| match self.rows.get(index) {
+            Some(text) => row(text),
+            None => reserved_row(),
+        }));
+        menu
+    }
+}
+
+/// Never blocks: this runs on the tray thread, which also answers the panel's D-Bus calls. A
+/// click while the queue is full is dropped and logged.
+fn send(commands: &Sender<Command>, command: Command) {
+    if let Err(error) = commands.try_send(command) {
+        eprintln!("statsbard: menu click dropped: {error}");
     }
 }
 
@@ -165,13 +221,9 @@ impl Hub {
         }
     }
 
-    /// Never blocks: this runs on the tray thread, which also answers the panel's D-Bus calls. A
-    /// click while the queue is full is dropped and logged; the checkmark then still shows the
-    /// state the main loop last set.
+    /// A dropped click leaves the checkmark showing the state the main loop last set.
     fn send(&self, command: Command) {
-        if let Err(error) = self.commands.try_send(command) {
-            eprintln!("statsbard: menu click dropped: {error}");
-        }
+        send(&self.commands, command);
     }
 }
 
```

*Create* `crates/daemon/src/launch.rs`:

```rust
//! Starting `statsbar-panel` from a menu click. The panel is a GTK application with one window
//! per session: a second launch hands its page to the running one over D-Bus and exits.
//!
//! The daemon keeps each child until it has exited and been reaped, so none is left a zombie,
//! and refuses a click while too many are still running.

use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use statsbar_core::ipc::Page;

const PANEL: &str = "statsbar-panel";
/// One window, plus launches still handing over a page. More means something is stuck.
pub const MAX_RUNNING: usize = 4;
/// GTK's renderer, when the user has not chosen one: the lighter of the two measured in
/// Plan 04b's run (see its spike record).
pub const RENDERER: &str = "cairo";

pub struct Launcher {
    program: PathBuf,
    /// `GSK_RENDERER` for the child.
    renderer: OsString,
    running: Vec<Child>,
}

impl Launcher {
    /// `inherited_renderer` is the daemon's own `GSK_RENDERER`: a choice the user made, which
    /// the child keeps. Without one the child gets [`RENDERER`].
    pub fn new(program: PathBuf, inherited_renderer: Option<OsString>) -> Launcher {
        Launcher {
            program,
            renderer: inherited_renderer.unwrap_or_else(|| RENDERER.into()),
            running: Vec::new(),
        }
    }

    /// The panel installed beside this daemon's executable.
    pub fn beside(daemon: PathBuf, inherited_renderer: Option<OsString>) -> Launcher {
        Launcher::new(daemon.with_file_name(PANEL), inherited_renderer)
    }

    pub fn open(&mut self, page: Page) -> io::Result<()> {
        self.reap();
        if self.running.len() >= MAX_RUNNING {
            return Err(io::Error::other(format!(
                "{MAX_RUNNING} panels are still running"
            )));
        }
        let mut command = Command::new(&self.program);
        command
            .args(["--page", page_arg(page)])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .env("GSK_RENDERER", &self.renderer);
        self.running.push(command.spawn()?);
        Ok(())
    }

    /// Collects the children that have exited. Cheap enough for every turn of the main loop.
    pub fn reap(&mut self) {
        self.running.retain_mut(|child| match child.try_wait() {
            Ok(None) => true,
            Ok(Some(_)) => false,
            Err(error) => {
                // Only an already reaped child fails here; keeping it would count it forever.
                eprintln!("statsbard: waiting for the panel failed: {error}");
                false
            }
        });
    }

    pub fn running(&self) -> usize {
        self.running.len()
    }
}

fn page_arg(page: Page) -> &'static str {
    match page {
        Page::Cpu => "cpu",
        Page::Memory => "memory",
    }
}
```

`RENDERER` starts as `"cairo"`; Task 7 measures and may change it.

- [x] **Step 4: Run, expect PASS.** `cargo test -p statsbard`, `cargo clippy -p statsbard --all-targets -- -D warnings`; `cargo tree -p statsbard -e normal | grep -c gtk` prints `0`.

- [x] **Step 5: Checkpoint.** Ready to commit: the files above — "feat(daemon): open the panel from the CPU and Memory menus"

### Task 7: Verification & Docs (renderer, end-to-end run)

**Type:** docs + verification · `RED: n/a — measurement and docs, checked by the evidence below`

**Files:** Create `docs/spikes/2026-11-24-04b-panel-e2e.md`. Modify `crates/daemon/src/launch.rs` (only if the measurement picks GL), `docs/plans/2026-11-16-rust-statsbar-roadmap.md` (Plan 04b entry: *written*, evidence link; "Needs first" done), `README.md` (Status line: the panel exists; add Plan 04b to the plan list), `docs/follow-ups/2026-11-24-04a-follow-ups.md` (remove the items this plan closes: `MemoryBuckets::used()`, "recheck uid and `start_ticks` at kill time"; note the Pss result). Create `docs/follow-ups/2026-11-25-04b-follow-ups.md` (the final review's findings not fixed in 04b).

- [x] **Step 1: Release build.** `cargo build --workspace --release --locked`. Stop any running `statsbard` first (`pgrep -x statsbard`), then start `target/release/statsbard` on the desktop session.
- [x] **Step 2: Renderer.** With the CPU page open and idle, measure the panel for 60 s under `GSK_RENDERER=cairo` and `GSK_RENDERER=ngl` (start it by hand with each): Pss from `/proc/<pid>/smaps_rollup`, CPU from `/proc/<pid>/stat` utime+stime delta. Keep the lower Pss (CPU as tie-break) in `launch::RENDERER`; if that is `ngl`, edit the constant and its comment and rerun `cargo test -p statsbard --test launch`. Measure the daemon's idle Pss too (04a follow-up: 3285 kB against 2789 kB expected) and record it.
- [x] **Step 3: Menus.** CPU and Memory menus show "Open details…" first, a separator, then their rows; Battery, Fan, Network unchanged. Click CPU's: the window opens on CPU. Click Memory's while it is open: the same window switches to Memory (one `statsbar-panel` window; `pgrep -c statsbar-panel` drops back to 1 once the second launch exits).
- [x] **Step 4: Figures and Refresh.** Panel figures match `top` (usage, per-process %) and `free` (used = total − available) within one sample; temperature matches `sensors` package. Click Refresh: a new event arrives at once, seen as the Uptime row changing before the next 1 s tick (watch it across several clicks) and the daemon's `syscr` stepping up per click. Hide the CPU item in the hub; its page still updates in the panel (04a decision 12).
- [x] **Step 5: Quit.** Start stand-ins that rank: `python3 -c 'import time; b=bytearray(800<<20); time.sleep(900)' &` (Memory) and `yes >/dev/null &` (CPU). On the Memory page click the stand-in's ✕, Cancel (it survives), then ✕ and Quit: it exits and the toast reads "Asked … to quit". Failure: start a second stand-in, click its ✕, start it with `setsid` so it is reaped at once, kill it from a terminal (`kill <pid>`) while the dialog is open, check `/proc/<pid>` is gone, then Quit: the toast reads "Could not quit …" with the reason. Any row without ✕ (a process the daemon marks not quittable) offers no quit. Kill the `yes` stand-in afterwards.
- [x] **Step 6: Daemon down.** Quit StatsBar from the hub: the panel shows the error page, Refresh is disabled. Start the daemon, click Retry: pages return. Quit StatsBar again, start it, and this time click "Open details…" instead of Retry: the same window connects and shows the page (Existing Data row: a user action, not an automatic reconnect).
- [x] **Step 7: Close.** Close the window: no `statsbar-panel` process left (`pgrep -x statsbar-panel` empty) and, per the daemon's `syscr` or log, the page's 1 s cadence stops (04a e2e method).
- [x] **Step 8: Record.** Write the spike record (numbers, no process names or pids). Update the roadmap, `README.md` and the 04a follow-ups file as listed under *Files*.
- [x] **Step 9: Final gate.** `./scripts/check.sh`, `cargo audit`, full diff read (untracked files included), then `code-review-strict` (the ✕ path signals processes: security review of `quit.rs` and `window.rs::confirm_quit`, including how a process name is shown in the dialog). Report that GTK code has no automated test, that CI runs only Tasks 1–4 and 6's tests, and that the `CommandsClosed` path (Impact Map) is checked by reading only.
- [x] **Step 10: Checkpoint.** Ready to commit: `docs/spikes/2026-11-24-04b-panel-e2e.md`, `docs/follow-ups/2026-11-25-04b-follow-ups.md`, the roadmap, `README.md`, `docs/follow-ups/2026-11-24-04a-follow-ups.md`, `crates/daemon/src/launch.rs` if changed — "docs: record the panel end-to-end run"
