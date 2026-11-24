# Plan 04a — Daemon IPC Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: subagent-driven-strict or executing-plans-strict. Tasks are `### Task N: <name>`; steps use `- [ ]`.

**Goal:** `statsbard` serves everything the CPU and Memory pages need over a private Unix socket. It
reads a page's metric once a second while someone watches it, whether the tray item is shown or
hidden. [Roadmap](2026-11-16-rust-statsbar-roadmap.md) Plan 04 is split in two: this plan is the
daemon half, and Plan 04b is the panel.

**Architecture:**

- `statsbar-core` gains the parsers, rankings, levels and wire types, all pure.
- `statsbar-readers` gains the process, CPU-detail and pressure readers.
- `statsbard::app` learns that a page can be open. `statsbard::pages` builds snapshots and keeps
  the rankings' baselines. `statsbard::ipc` is a non-blocking server polled by the main loop.
- `statsbard::run` joins the three. No thread is added.
- The socket is read-only towards the machine: no request writes a file or sends a signal.

**Tech Stack:** Rust edition 2024, workspace floor 1.85, `statsbard` 1.87, `rustc 1.100.0`.
Approved additions: `serde` 1.0.229 in core, `serde_json` 1.0.154 (MIT or Apache-2.0) and `rustix`
1.1.6 (Apache-2.0 with LLVM exception, Apache-2.0 or MIT) in the daemon.

**Branch:** `feature/ipc-server`, cut from `origin/main` at a7a13f0.

**How to read the code blocks:**

- *Create* gives a whole file.
- *Apply to* gives a diff against the tree as the previous task left it. Apply it with `git apply`
  from the repository root. An empty line inside a diff is an empty context line.
- Line ranges under *Files* refer to that same previous tree.

## Confirmed Requirements

The user accepted every recommendation of the Open Questions list on 2026-11-22 ("Theo đề xuất").
The numbers are that list's.

1. **Split:** 04a is the daemon and IPC, then 04b is the panel.
2. **No relm4:** the panel uses `gtk4-rs` directly. This replaces the user's earlier pick. *(04b)*
3. **Dependencies approved:** `gtk4`, `libadwaita`, `serde_json`, `rustix`, and `serde` in core; a
   1.92 floor for the panel; `libgtk-4-dev` and `libadwaita-1-dev` in CI and on the laptop. *(04a
   uses only `serde`, `serde_json` and `rustix`.)*
4. **An open page always polls at 1 s**, under power saver too. The code and the test that pinned
   5 s are fixed.
5. **The CPU page is as complete as upstream's:** temperatures, frequency, load per core, uptime.
   *(data here, UI in 04b)*
6. **Thermal** follows the package temperature: Fair from 75 °C, Serious from 90 °C, Critical from
   100 °C, otherwise Nominal.
7. **A process's memory** is RSS minus shared pages. This replaces the roadmap's RSS.
8. **Quit a process:** confirm first, send SIGTERM, own processes only, never GNOME Shell, systemd
   or StatsBar. *(`can_quit` here; the dialog and the signal in 04b)*
9. **A process's CPU %** is the change between two samples, as in `top`: 100 is one core.
10. **A process's name** is its executable's name, falling back to `comm`. Each list has six rows.
11. **"Open details…"** heads the CPU and Memory menus and reuses the window. *(04b)*
12. **A hidden item's page** can still be viewed, and is polled while it is open.
13. **With the daemon down** the panel shows an error with Retry. *(04b)*
14. **A second daemon** prints an error and exits.
15. **The page footer** has Refresh only. Quit stays in the hub. *(the request here, the button in
    04b)*
16. **The Pressure ring** fills to (kernel + zswap) / total. Missing PSI is shown as "—", never as
    Normal. *(data here, UI in 04b)*
17. **Menu rows are reserved from the start**, so a `ksni` layout change does not lose a click on
    "Open details…". *(04b)*

## Decision Log

Every entry is **not posted: the project has no ticket**. The comment drafts are kept here, and
each last column goes into the PR description of the plan that implements it.

| # | Draft comment (decision and reason; decided by the user, 2026-11-22) | PR description line |
|---|---|---|
| 1 | Split Plan 04 into 04a and 04b, so the socket is reviewed apart from the GTK work. | Plan 04 is split: this is the daemon and IPC half. |
| 2 | Drop relm4: one small window does not need a second runtime layer. | The panel uses gtk4-rs without relm4. |
| 3 | Approve the crates and GTK dev packages that the IPC and panel need. | Adds the approved serde (core), serde_json and rustix. |
| 4 | An open page polls at 1 s under power saver: upstream checks the open panel first, and our comment said the opposite. | Fixes cadence: an open page is read each second under power saver. |
| 5 | Give the CPU page all of upstream's details, so no 04c is needed. | The CPU page carries temperatures, frequency, load and uptime. |
| 6 | Use 75/90/100 °C for Thermal: max and crit are both 100 °C here, so they cannot give four levels. | Thermal levels start at 75, 90 and 100 °C of the package. |
| 7 | Rank memory by RSS minus shared: PSS cost about 218 ms and could read only 179 of 380 processes. | Process memory is RSS minus shared, not RSS. |
| 8 | Guard Quit, because it loses data: confirm, SIGTERM, own processes, protected session processes. | Only the user's unprotected processes are marked quittable. |
| 9 | Show CPU as a delta, as top does, not a lifetime average as ps does. | Process CPU is per sample, with 100 as one core. |
| 10 | Prefer the executable's name, because comm is cut at 15 characters. | Six rows, named by executable with comm as the fallback. |
| 11 | Open the page from the metric's own menu. | CPU and Memory menus open their page. |
| 12 | A hidden item saves panel space and does not withdraw its page. | A hidden item's page is served and polled while watched. |
| 13 | Keep the window and say what is wrong when the daemon is down. | The panel offers Retry when the daemon is down. |
| 14 | Refuse a second daemon instead of replacing the running one. | A second statsbard prints an error and exits. |
| 15 | Keep Quit StatsBar in one place, the hub. | A page can ask for a refresh. |
| 16 | Derive the ring from kernel and zswap, and never invent Normal without PSI. | Missing PSI is sent as null, never as Normal. |
| 17 | Reserve menu rows, because ksni renumbers ids when the layout changes. | Menu ids are stable from the first layout. |

## Scope Map

**Invariant:** a metric is read exactly while its item is visible or its page is watched, at 1 s
while watched (a Refresh may bring one reading forward, never sooner than 500 ms after the last),
and only this user's processes can connect to the socket.

| Path | In or out | Test |
|---|---|---|
| `schedule::interval_nanos` lets power saver beat an open page | in (Task 1) | `intervals_follow_the_roadmap_cadence_table` |
| `schedule::{due, next_wake_nanos}` | in (Tasks 1, 5): both go through `interval_nanos` | `an_open_page_is_read_every_second_under_power_saver` |
| `App::new`: nothing is watched at start | in (Task 5) | `items_without_a_page_follow_their_visibility_alone`, `refresh_without_an_open_page_does_nothing` |
| `App::toggle` resets `page_open`, the samples and the failure flag | in (Tasks 5, 8) | `hiding_an_item_with_its_page_open_keeps_reading_and_keeps_its_figures`, `hiding_an_item_with_its_page_open_keeps_the_page_live` |
| `App::{forget, record_*}`: the label of an item shown while its page is open | in (Task 5) | `showing_an_item_with_its_page_open_draws_its_label_on_the_next_sample` |
| `App::record_error`: retry interval and logging under an open page | in (Task 5) | `a_failure_under_an_open_page_is_retried_each_second_and_logged_afresh_after_closing` |
| `App::{overview, rows}` with a hidden item and an open page | in (Task 5): the hub still shows it as hidden | `hiding_an_item_with_its_page_open_keeps_reading_and_keeps_its_figures` |
| `run::hide_item` and a watched page's reader | in (Task 8) | `hiding_an_item_with_its_page_open_keeps_the_page_live` |
| `run::show_item` | none: it only spawns the item; the first label comes from Task 5's `forget_label` | — |
| `run::sample` as the source of page events | in (Task 8) | `pages_of_hidden_items_are_served_and_switched` |
| `run::read_failed` | none: it publishes nothing, so the page keeps its last values; the retry is `record_error`'s | — |
| `run::refresh_item` with no item | none: it already returns when the item is absent | — |
| Power-profile change while a page is open | in (Tasks 1, 5) | `an_open_page_is_read_every_second_under_power_saver` |
| The last watcher leaves | in (Tasks 7, 8) | Unsubscribe and switch: `subscribing_replaces_the_page_and_unsubscribing_clears_it`; EOF: `a_page_stays_watched_until_its_last_watcher_leaves`; protocol error: `a_bad_request_from_a_watcher_releases_its_page`; overflow: `a_client_that_stops_reading_is_dropped_instead_of_buffered`; in the loop: `a_hidden_items_files_close_with_its_last_watcher` |
| `run::handle_panel` on Subscribe | in (Tasks 7, 8): the newcomer gets the page as last published, and nothing is read for it | `only_a_new_watcher_is_welcomed`, `a_shown_items_page_is_sent_before_its_next_reading`, `pages_of_hidden_items_are_served_and_switched` |
| Subscribe and Unsubscribe, or page switches, over and over on a hidden item | in (Task 5): the time of the last read survives the close, so the page opened again is read when its interval since that read is over, never sooner | `a_hidden_items_page_opened_again_waits_for_its_interval` |
| `run::handle_panel` on Refresh | in (Tasks 5, 6, 8, 9): the page is due at once when it was read 500 ms ago or more, and only then is the Memory ranking scanned again | `refresh_reads_now_once_half_a_second_has_passed`, `refresh_without_an_open_page_does_nothing`, `memory_processes_are_rescanned_every_two_seconds_or_on_refresh`; the wiring in the loop by Task 9's Refresh rows |
| A client's requests or connections hold up the main loop or the other clients | in (Task 7): a budget per turn, and a client that got its turn goes to the back | `a_client_with_a_backlog_does_not_starve_the_others`, `a_client_with_a_stream_of_requests_does_not_starve_the_others`, `accepts_are_bounded_per_turn` |
| Process and detail scans with no watcher | in (Tasks 6, 8): `publish` returns first and `close` forgets | `closing_the_cpu_page_forgets_its_baseline`, `closing_the_memory_page_forgets_its_ranking`, `a_hidden_items_files_close_with_its_last_watcher` |
| Another uid, a first line that is not a hello, an oversized line, too many clients | in (Task 7) | `a_client_of_another_uid_gets_nothing`, `a_first_line_that_is_not_a_hello_is_refused`, `a_line_without_an_end_is_refused_at_the_limit`, `one_client_too_many_is_told_busy_and_a_freed_place_is_reused` |
| A second daemon: started by hand, by autostart, after a crash | in (Tasks 7, 8): refused before any tray item | `a_second_daemon_exits_and_a_killed_one_can_be_replaced` |
| Hub Quit and process death | in (Task 7): the socket goes on Drop, a stale one is replaced | `the_socket_is_private_and_removed_on_drop`, `a_socket_left_by_a_killed_daemon_is_replaced` |
| `autostart::toggle`, `config::persist` | none: no format change, and neither touches readers | — |
| Battery, Fan and Network readers | none: they have no page, so visibility alone decides | `items_without_a_page_follow_their_visibility_alone` |
| Their pages | out: Plan 05 and later, per the roadmap | — |
| Menu entry, GTK window, "—" placeholders, quit dialog and signal | out: Plan 04b, by requirement 1 | — |
| Installing, `.desktop` entries | out: Plan 11, per the roadmap | — |

## Existing Data & States

| Existing case | Intended behavior | Task and test |
|---|---|---|
| A config that hides CPU or Memory | No format change; the page is served and the item stays hidden | 8 `pages_of_hidden_items_are_served_and_switched` |
| A shown item whose page opens | Keeps its baseline. Its first watcher gets the first page at the next due reading, within a second; a later watcher gets the page as last published at once, with no read | 5 `opening_a_shown_items_page_keeps_its_baseline`; 7 `only_a_new_watcher_is_welcomed`; 8 `a_shown_items_page_is_sent_before_its_next_reading` |
| A hidden item's page opened again after it closed | Starts afresh: nothing from the earlier opening is sent. It is read once a second has passed since its last read, at once if that was longer ago | 5 `a_hidden_items_page_opened_again_waits_for_its_interval`; 8 `pages_of_hidden_items_are_served_and_switched` |
| Page opened again | Nothing changes | 5 `opening_an_open_page_again_changes_nothing` |
| Item hidden while its page is open | Reading and figures continue; the files stay open | 5 `hiding_an_item_with_its_page_open_keeps_reading_and_keeps_its_figures`; 8 `hiding_an_item_with_its_page_open_keeps_the_page_live` |
| Item shown while its page is open | The blank item gets its label on the next sample | 5 `showing_an_item_with_its_page_open_draws_its_label_on_the_next_sample` |
| A hidden item's last watcher leaves | Reads stop, samples are forgotten, files close | 5 `closing_a_hidden_items_page_stops_and_forgets`; 8 `a_hidden_items_files_close_with_its_last_watcher` |
| Power saver on while a page is open | 1 s, then 5 s after it closes | 1 the schedule table; 5 `an_open_page_is_read_every_second_under_power_saver` |
| A socket left by a killed daemon | Replaced | 7 `a_socket_left_by_a_killed_daemon_is_replaced`; 8 `a_second_daemon_exits_and_a_killed_one_can_be_replaced` |
| A file that is not a socket at that path | Left alone; start fails | 7 `a_file_that_is_not_a_socket_is_left_alone` |
| A daemon already running | The new one exits; the old one keeps working | 7 `a_second_server_is_refused_and_the_first_keeps_working`; 8 as above |
| A running daemon from before this plan | Holds no lock, so it cannot be detected | 9: stopped before the run; listed under Risks |
| No absolute `XDG_RUNTIME_DIR` | Error and exit | 7 `the_runtime_directory_must_be_an_absolute_path`; 8 `without_a_runtime_directory_it_says_so_and_exits` |
| CPU hotplug | Temperatures keep their CPU; a list of another length is not sent | 4 `an_offline_cpu_does_not_shift_the_others_temperatures`; 6 `temperatures_are_dropped_when_the_load_is_of_other_cpus` |
| Kernel without PSI | `null`, retried, logged once | 4 `a_kernel_without_psi_is_not_found_and_is_retried`; 6 `missing_psi_is_none_and_reported_once_until_it_recovers` |
| No coretemp, no cpufreq | Only those figures are absent | 4 `without_coretemp_only_the_temperatures_are_missing`, `without_cpufreq_only_the_frequency_is_missing` |
| hwmon renumbered | Found again | 4 `a_sensor_that_moves_to_another_hwmon_is_found_again` |
| A vanished, new or reused pid | Left out of that ranking | 2 `a_vanished_a_new_and_a_reused_pid_are_left_out`; 4 `a_vanished_or_unreadable_process_is_skipped` |
| A `comm` that is not UTF-8, or has spaces and parentheses | Still read correctly | 2 `a_name_with_spaces_and_parentheses_does_not_shift_the_fields`; 4 `a_name_that_is_not_utf8_still_gives_a_sample` |
| A replaced executable | Keeps its name | 2 `a_replaced_executable_keeps_its_name` |
| A process whose `exe` is unreadable, such as the user's systemd | Named and protected by `comm` | 4 `identify_falls_back_to_the_kernel_name_and_still_protects_by_it` |
| Another user's process | Listed, never quittable | 4 `another_users_process_cannot_be_quit` |
| A scan that fails | The last ranking stays; logged once | 6 `a_failed_scan_keeps_the_last_ranking_and_reports_once` |
| A client that stops reading | Dropped, never buffered without bound | 7 `a_client_that_stops_reading_is_dropped_instead_of_buffered` |

## Global Constraints

From the user's global rules and the roadmap. The repository has no `CLAUDE.md`.

- Test-first: happy path, edge cases, error paths. Untested code is reported incomplete, never done.
- Each test can fail without its change; assert observable behavior. Mock only true boundaries.
  Deterministic: no sleeps, wall clock, ordering, shared state or network. A bounded deadline that
  only fails a hung test is allowed.
- Never weaken, skip or delete a test to make it pass. Task 1 adds one row to the cadence table
  (a hidden item's open page under power saver) and changes one expectation, a shown item's open
  page under power saver, from 5 s to 1 s: requirement 4 states that change. No other expectation changes.
- Commit or push only when asked. Tasks end with `Ready to commit: …`, never `git commit`.
  Conventional Commits, imperative mood, one logical change per commit.
- Code, comments, names, commit messages, docs and log messages in English. Comments explain why.
- Every external call has a timeout; nothing is unbounded. No host clock in logic: time is passed
  in from a monotonic clock.
- No sensitive data in logs: a peer's line is neither echoed nor logged, and no process name is
  logged. No real process name, pid or other personal data goes into the repository's docs.
- Stay in scope: no drive-by refactors, renames or reformatting. Suggest them instead.
- Ask before adding or upgrading a dependency beyond requirement 3.
- Flag changes to authentication or authorization for review. The socket's uid check is one, so
  Task 9 reviews it for security.
- **Invariant:** `crates/core` does no I/O, reads no clock, spawns nothing, has no `unsafe` and no
  build script, and depends on nothing but `serde`. The gate also counts dev-dependencies, which is
  why the wire-format tests live in `crates/daemon/tests/protocol.rs`.
- **Invariant:** `statsbard` never links GTK and never starts a tokio runtime. Its threads are the
  main loop, `async-io`'s reactor, `ksni`'s one driver and the allowed `blocking` pool threads.
- Verification entry point: `./scripts/check.sh`.

## Technical Defaults

| Choice | Reason |
|---|---|
| `XDG_RUNTIME_DIR` is now required, with no `/tmp` fallback | The roadmap puts the socket there, and requirement 14 needs one private place for the lock. A graphical session always sets it. **This changes startup:** without it the daemon exits. |
| `flock` on `statsbar.lock`, released by the kernel | Makes replacing a stale socket safe after any kind of death. |
| Socket mode 0600 and an `SO_PEERCRED` uid check | Least privilege, as the roadmap already requires of the fan helper. |
| The runtime directory must be the user's own and closed to group and others; the socket and lock carry the sticky bit; a request line over 4096 bytes is refused even when its newline arrives in the same read | Added in Task 7's review at the user's choice: a shared `XDG_RUNTIME_DIR` would let another user hold the lock, and the XDG spec lets unsticky files there be cleaned up. |
| Newline-delimited JSON with a protocol line first each way | Already the roadmap's default. |
| Fields may be added within protocol 1; a removal, rename or change of meaning raises the version | Readers ignore unknown fields, and an absent optional field reads as `None`. |
| 4 clients, 4 KiB per line, 64 KiB unsent, 5 s handshake (checked only when nothing is left to read, so a hello that came in time counts after a stalled loop), 1 s accept backoff; per turn at most 16 requests that change nothing from one client and 8 accepts, and a client that got a change through goes to the back | Bounds. The 5 s is the daemon's existing `SETUP_TIMEOUT`. The per-turn budget keeps one client from holding up the main loop or the others. |
| Refresh under 500 ms after the last read is ignored, and an ignored one does not bring the Memory scan forward either | A load over a few ticks is noise, and it bounds what a client can cost. |
| CPU scans at least 500 ms apart; memory scans every 2 s | The same noise bound, and memory ranks move slowly. |
| A process appears in the CPU list one scan after it starts | Follows from requirement 9: a delta needs two samples. |
| A failed read or scan keeps the last values | Plan 03's rule for a failed read. |
| The temperature ring gets the package sensor; each core's bar gets its core's sensor | The package is what requirement 6 judges. |
| Frequency is the mean of the logical CPUs | Upstream's row is "All cores". |
| systemd's `(sd-pam)` helper and pid ≤ 1 count as protected | Both are systemd, under requirement 8. |
| `serde_json` is a dev-dependency in Task 3 and a normal one from Task 7 | Cargo 1.100 warns about a normal dependency that the crate does not use yet. |
| Wire-format tests are in the daemon | Core may not have dev-dependencies. |

## Risks & Rollback

| Risk | Mitigation | Revert |
|---|---|---|
| A page costs CPU while it is open | Measured on the laptop in a release build, 2026-11-22, 20 readings: CPU page 5.96 ms mean and 7.50 ms at most, Memory 3.32 ms and 5.47 ms, at most once a second on the main loop. Nothing runs without a watcher. Plan 11 measures the budget. | Revert Task 8: nothing calls the pages |
| A same-user client floods requests | It can read nothing it could not read in `/proc` and cannot act on the machine. No request reads outside the cadence beyond Refresh: Subscribe sends the page as last published, a hidden item's page opened again waits for its interval, and Refresh is ignored within 500 ms of the last read. At worst a flood of Refresh reads a page and scans its processes twice a second instead of once. The per-turn budget keeps the main loop and the other clients going. The flood costs that user's own CPU and ends with the connection. | Revert Task 8 |
| Process names and pids leave the daemon | Only over a 0600 socket to the same uid, and never to the log | Revert Task 8 |
| Autostart fails without `XDG_RUNTIME_DIR` | The error names the variable | Revert Task 8 |
| A daemon from before this plan runs beside the new one | Stop it once after upgrading. From this version on the lock refuses a second one. | Quit either from its hub |
| The protocol must change later | The version line and the evolution rule; the panel ships from the same repository | Raise the version |
| Cadence change costs power | Only while a page is open, as upstream | Revert Task 1 |
| Persisted data | None is added or changed. The lock and the socket live in the runtime directory, which logout clears. | Nothing to migrate |
| Declared floors 1.85 and 1.87 are not compiled | Only the stable toolchain is installed here and in CI. Both new crates declare lower floors (1.71, 1.65). Reported as unverified. | — |
| `zbus_names`, `zvariant` and `zvariant_derive` are flagged unmaintained | Already flagged on a7a13f0; this plan adds no advisory (`cargo audit`, 2026-11-22) | — |
| Socket and process tests could be flaky | They wait on events, with deadlines only against hangs. They passed 25 repeats, and 5 rounds with all 8 CPUs busy, while this plan was written; Task 9 repeats them. `a_shown_items_page_is_sent_before_its_next_reading` compares the pages' uptimes, so it can fail without a bug only when the test or the daemon stalls for about a second between the first watcher's page and the newcomer's subscribe. | — |

---

### Task 1: Read an open page every second under power saver

**Test level:** unit. This is a bug fix: **load `bugfix-strict` before changing code.** The root
cause is a misread of upstream, whose `CPUReader.applyCadence` checks `panelOpen` before Low Power
Mode. The wrong comment and the branch order both come from it. All callers go through
`schedule::{due, next_wake_nanos}`.

**Files:**

- Modify: `crates/core/src/schedule.rs` (lines 9–10, 38–40, after 41)
- Modify: `crates/core/tests/schedule.rs` (lines 24)

**Interfaces:**

- Consumes: `schedule::ReaderState { id, visible, page_open, last_read_nanos }`.
- Produces: `pub fn interval_nanos(state: &ReaderState, power_saver: bool) -> Option<u64>`, with
  the same signature: 1 s for an open page, then 5 s under power saver, then 2 s.

- [x] **Step 1: Write the failing test**

Apply to `crates/core/tests/schedule.rs`:

````diff
diff --git a/crates/core/tests/schedule.rs b/crates/core/tests/schedule.rs
--- a/crates/core/tests/schedule.rs
+++ b/crates/core/tests/schedule.rs
@@ -21,7 +21,8 @@ fn intervals_follow_the_roadmap_cadence_table() {
         (true, true, false, Some(SECOND)),
         (false, true, false, Some(SECOND)),
         (true, false, true, Some(5 * SECOND)),
-        (true, true, true, Some(5 * SECOND)),
+        (false, true, true, Some(SECOND)),
+        (true, true, true, Some(SECOND)),
     ];
     for (visible, page_open, power_saver, expected) in cases {
         let state = reader(ReaderId::Cpu, visible, page_open, None);
````

- [x] **Step 2: Run it, expect FAIL**

```bash
cargo test -p statsbar-core --test schedule
```

Expected: one failure with `visible false, page open true, power saver true`, left
`Some(5000000000)`, right `Some(1000000000)`, because power saver is checked first.

- [x] **Step 3: Implement**

Apply to `crates/core/src/schedule.rs`:

````diff
diff --git a/crates/core/src/schedule.rs b/crates/core/src/schedule.rs
--- a/crates/core/src/schedule.rs
+++ b/crates/core/src/schedule.rs
@@ -6,8 +6,8 @@ const NANOS_PER_SECOND: u64 = 1_000_000_000;
 pub const VISIBLE_INTERVAL_NANOS: u64 = 2 * NANOS_PER_SECOND;
 /// That metric's page is open in the panel.
 pub const PAGE_OPEN_INTERVAL_NANOS: u64 = NANOS_PER_SECOND;
-/// The `power-saver` profile is active. Upstream's CPU reader lets Low Power Mode win over an
-/// open page, and so does this.
+/// The `power-saver` profile is active and the page is closed. An open page keeps its 1 s, as in
+/// upstream's `CPUReader.applyCadence`: someone is looking at it.
 pub const POWER_SAVER_INTERVAL_NANOS: u64 = 5 * NANOS_PER_SECOND;

 #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
@@ -35,10 +35,10 @@ pub fn interval_nanos(state: &ReaderState, power_saver: bool) -> Option<u64> {
     if !state.visible && !state.page_open {
         return None;
     }
-    Some(if power_saver {
-        POWER_SAVER_INTERVAL_NANOS
-    } else if state.page_open {
+    Some(if state.page_open {
         PAGE_OPEN_INTERVAL_NANOS
+    } else if power_saver {
+        POWER_SAVER_INTERVAL_NANOS
     } else {
         VISIBLE_INTERVAL_NANOS
     })
````

- [x] **Step 4: Run, expect PASS**

```bash
cargo test -p statsbar-core --test schedule && cargo test -p statsbard --test app
```

Expected: every test passes; `schedule` runs 8.

- [x] **Step 5: Checkpoint**

Ready to commit: `crates/core/src/schedule.rs`, `crates/core/tests/schedule.rs` — "fix(core): read an open page every second under power saver"

### Task 2: Parse and rank processes, system figures and pressure

**Test level:** unit. All of it is pure text and arithmetic.

**Files:**

- Modify: `crates/core/src/lib.rs` (lines after 15, after 17)
- Modify: `crates/core/src/memory.rs` (lines after 109, after 118)
- Create: `crates/core/src/process.rs`
- Create: `crates/core/src/system.rs`
- Create: `crates/core/tests/pressure.rs`
- Create: `crates/core/tests/process.rs`
- Create: `crates/core/tests/system.rs`

**Interfaces:**

- Consumes: `memory::MemoryBuckets`.
- Produces:

```rust
// crates/core/src/lib.rs
pub mod process;
pub mod system;

// crates/core/src/memory.rs
impl MemoryBuckets {
    pub fn pressure_fraction(&self) -> f64;
}
pub enum Pressure {
    Normal,
    Warning,
    Critical,
}
impl Pressure {
    pub fn from_psi(text: &str) -> Option<Pressure>;
    pub fn label(self) -> &'static str;
}

// crates/core/src/process.rs
pub struct Stat {
    pub comm: String,
    pub cpu_ticks: u64,
    pub start_ticks: u64,
}
pub fn parse_stat(text: &str) -> Option<Stat>;
pub fn parse_statm(text: &str, page_size: u64) -> Option<u64>;
pub struct CpuSample {
    pub pid: u32,
    pub start_ticks: u64,
    pub cpu_ticks: u64,
}
pub struct CpuUse {
    pub pid: u32,
    pub start_ticks: u64,
    pub percent: f64,
}
pub fn rank_by_cpu(previous: &[CpuSample], current: &[CpuSample], elapsed_nanos: u64, ticks_per_second: u64, count: usize) -> Vec<CpuUse>;
pub struct MemorySample {
    pub pid: u32,
    pub bytes: u64,
}
pub fn rank_by_memory(samples: &[MemorySample], count: usize) -> Vec<MemorySample>;
pub fn display_name(exe: Option<&str>, comm: &str) -> String;
pub fn can_quit(pid: u32, owner_uid: u32, own_uid: u32, exe: Option<&str>, comm: &str) -> bool;

// crates/core/src/system.rs
pub fn parse_loadavg(text: &str) -> Option<[f64; 3]>;
pub fn load_per_cpu(load: [f64; 3], cpus: usize) -> Option<[f64; 3]>;
pub fn parse_uptime(text: &str) -> Option<f64>;
pub fn parse_cpu_model(text: &str) -> Option<String>;
pub fn parse_cpu_list(text: &str) -> Option<Vec<u32>>;
pub fn parse_millidegrees(text: &str) -> Option<f64>;
pub enum Thermal {
    Nominal,
    Fair,
    Serious,
    Critical,
}
impl Thermal {
    pub fn from_celsius(celsius: f64) -> Thermal;
    pub fn label(self) -> &'static str;
}
```

- [x] **Step 1: Write the failing tests**

Create `crates/core/tests/pressure.rs`:

````rust
use statsbar_core::memory::{MemoryBuckets, Pressure};

fn psi(some: &str, full: &str) -> String {
    format!(
        "some avg10={some} avg60=0.00 avg300=0.00 total=6639897\n\
         full avg10={full} avg60=0.00 avg300=0.00 total=5310078\n"
    )
}

#[test]
fn pressure_changes_at_ten_percent() {
    use Pressure::{Critical, Normal, Warning};
    for (some, full, expected) in [
        ("0.00", "0.00", Normal),
        ("9.99", "0.00", Normal),
        ("10.00", "0.00", Warning),
        ("10.01", "9.99", Warning),
        ("12.00", "10.00", Critical),
        ("12.00", "10.01", Critical),
    ] {
        assert_eq!(
            Pressure::from_psi(&psi(some, full)),
            Some(expected),
            "{some} {full}"
        );
    }
}

#[test]
fn pressure_that_cannot_be_read_is_none_and_never_normal() {
    for bad in [
        String::new(),
        "some avg10=0.00 avg60=0.00\n".to_owned(),
        "full avg10=0.00 avg60=0.00\n".to_owned(),
        psi("x", "0.00"),
        psi("NaN", "0.00"),
    ] {
        assert_eq!(Pressure::from_psi(&bad), None, "{bad:?}");
    }
}

#[test]
fn pressure_is_named_as_upstream_names_it() {
    use Pressure::{Critical, Normal, Warning};
    assert_eq!(
        [Normal, Warning, Critical].map(Pressure::label),
        ["Normal", "Warning", "Critical"]
    );
}

#[test]
fn the_pressure_ring_fills_to_kernel_plus_zswap() {
    let buckets = MemoryBuckets {
        total: 1000,
        app: 400,
        kernel: 150,
        cached: 300,
        free: 150,
        swap_used: 0,
        compressed: 100,
    };
    assert_eq!(buckets.pressure_fraction(), 0.25);
    let overfull = MemoryBuckets {
        kernel: u64::MAX,
        ..buckets
    };
    assert_eq!(overfull.pressure_fraction(), 1.0);
    assert_eq!(MemoryBuckets::default().pressure_fraction(), 0.0);
}
````

Create `crates/core/tests/process.rs`:

````rust
use statsbar_core::process::{
    CpuSample, CpuUse, MemorySample, can_quit, display_name, parse_stat, parse_statm, rank_by_cpu,
    rank_by_memory,
};

const SECOND: u64 = 1_000_000_000;
const HZ: u64 = 100;

fn stat_line(comm: &str, utime: u64, stime: u64, start: u64) -> String {
    format!(
        "4242 ({comm}) S 1 4242 4242 0 -1 4194304 521 0 0 0 {utime} {stime} 0 0 20 0 1 0 {start} 18214912 1871"
    )
}

fn sample(pid: u32, start_ticks: u64, cpu_ticks: u64) -> CpuSample {
    CpuSample {
        pid,
        start_ticks,
        cpu_ticks,
    }
}

fn memory(pid: u32, bytes: u64) -> MemorySample {
    MemorySample { pid, bytes }
}

#[test]
fn stat_yields_the_name_the_cpu_time_and_the_start_time() {
    let stat = parse_stat(&stat_line("cat", 30, 12, 9001)).expect("well-formed");
    assert_eq!(stat.comm, "cat");
    assert_eq!(stat.cpu_ticks, 42);
    assert_eq!(stat.start_ticks, 9001);
}

#[test]
fn a_name_with_spaces_and_parentheses_does_not_shift_the_fields() {
    let stat = parse_stat(&stat_line("a) R 1 (b c", 5, 6, 7)).expect("well-formed");
    assert_eq!(stat.comm, "a) R 1 (b c");
    assert_eq!((stat.cpu_ticks, stat.start_ticks), (11, 7));
}

#[test]
fn a_cut_or_non_numeric_stat_is_none() {
    assert_eq!(parse_stat(""), None);
    assert_eq!(parse_stat("4242 (cat) S 1 2 3"), None);
    assert_eq!(parse_stat("4242 cat S 1 2 3"), None);
    assert_eq!(parse_stat(")("), None);
    let broken = stat_line("cat", 30, 12, 9001).replace(" 9001 ", " x ");
    assert_eq!(parse_stat(&broken), None);
}

#[test]
fn statm_memory_is_resident_minus_shared_in_bytes() {
    assert_eq!(
        parse_statm("4447 1882 1537 1232 0 140 0\n", 4096),
        Some(345 * 4096)
    );
    assert_eq!(
        parse_statm("10 2 5 0 0 0 0", 4096),
        Some(0),
        "shared above resident"
    );
    assert_eq!(parse_statm("4447 1882", 4096), None);
    assert_eq!(
        parse_statm("1 18446744073709551615 0", 4096),
        None,
        "overflow"
    );
}

#[test]
fn cpu_is_the_delta_over_the_interval_with_one_core_as_100() {
    let previous = [sample(1, 10, 1000), sample(2, 20, 500)];
    let current = [sample(1, 10, 1050), sample(2, 20, 800)];
    assert_eq!(
        rank_by_cpu(&previous, &current, 2 * SECOND, HZ, 6),
        [
            CpuUse {
                pid: 2,
                start_ticks: 20,
                percent: 150.0
            },
            CpuUse {
                pid: 1,
                start_ticks: 10,
                percent: 25.0
            },
        ]
    );
}

#[test]
fn equal_cpu_ties_break_on_the_lower_pid() {
    let previous = [sample(9, 1, 0), sample(3, 1, 0), sample(5, 1, 0)];
    let current = [sample(9, 1, 10), sample(3, 1, 10), sample(5, 1, 10)];
    let pids: Vec<u32> = rank_by_cpu(&previous, &current, SECOND, HZ, 6)
        .iter()
        .map(|process| process.pid)
        .collect();
    assert_eq!(pids, [3, 5, 9]);
}

#[test]
fn a_vanished_a_new_and_a_reused_pid_are_left_out() {
    let previous = [sample(1, 10, 100), sample(2, 20, 100), sample(3, 30, 100)];
    // 1 exited, 2 is another process under the same pid, 4 is new.
    let current = [sample(2, 99, 900), sample(3, 30, 110), sample(4, 40, 500)];
    assert_eq!(
        rank_by_cpu(&previous, &current, SECOND, HZ, 6),
        [CpuUse {
            pid: 3,
            start_ticks: 30,
            percent: 10.0
        }]
    );
}

#[test]
fn cpu_time_that_went_backwards_reads_as_zero() {
    assert_eq!(
        rank_by_cpu(&[sample(1, 1, 50)], &[sample(1, 1, 40)], SECOND, HZ, 6),
        [CpuUse {
            pid: 1,
            start_ticks: 1,
            percent: 0.0
        }]
    );
}

#[test]
fn cpu_ranking_without_an_interval_or_a_tick_rate_is_empty() {
    let both = [sample(1, 1, 1)];
    assert!(rank_by_cpu(&both, &both, 0, HZ, 6).is_empty());
    assert!(rank_by_cpu(&both, &both, SECOND, 0, 6).is_empty());
}

#[test]
fn cpu_ranking_keeps_the_busiest_count() {
    let previous: Vec<_> = (1..=8).map(|pid| sample(pid, 1, 0)).collect();
    let current: Vec<_> = (1..=8).map(|pid| sample(pid, 1, u64::from(pid))).collect();
    let pids: Vec<u32> = rank_by_cpu(&previous, &current, SECOND, HZ, 6)
        .iter()
        .map(|process| process.pid)
        .collect();
    assert_eq!(pids, [8, 7, 6, 5, 4, 3]);
}

#[test]
fn memory_ranks_heaviest_first_with_ties_on_the_lower_pid() {
    let samples = [memory(7, 100), memory(2, 300), memory(4, 100), memory(1, 5)];
    assert_eq!(
        rank_by_memory(&samples, 3),
        [memory(2, 300), memory(4, 100), memory(7, 100)]
    );
    assert!(rank_by_memory(&samples, 0).is_empty());
}

#[test]
fn the_executable_name_beats_the_truncated_kernel_name() {
    assert_eq!(
        display_name(Some("gnome-keyring-daemon"), "gnome-keyring-d"),
        "gnome-keyring-daemon"
    );
    assert_eq!(display_name(Some("node"), "MainThread"), "node");
}

#[test]
fn a_replaced_executable_keeps_its_name() {
    assert_eq!(
        display_name(Some("sample-editor-app (deleted)"), "sample-editor-a"),
        "sample-editor-app"
    );
}

#[test]
fn without_an_executable_the_kernel_name_is_shown() {
    assert_eq!(display_name(None, "kworker/0:1"), "kworker/0:1");
    assert_eq!(display_name(Some("  "), "cat"), "cat");
    assert_eq!(display_name(None, ""), "—");
}

#[test]
fn control_characters_never_reach_a_row() {
    assert_eq!(display_name(None, "two\nlines"), "two\u{fffd}lines");
}

#[test]
fn only_the_users_own_processes_can_be_quit() {
    assert!(can_quit(500, 1000, 1000, Some("firefox"), "firefox"));
    assert!(!can_quit(500, 0, 1000, None, "sshd"));
    assert!(!can_quit(500, 1001, 1000, None, "firefox"));
    assert!(!can_quit(1, 1000, 1000, None, "init"), "pid 1");
}

#[test]
fn the_session_and_statsbar_cannot_be_quit() {
    for name in [
        "gnome-shell",
        "systemd",
        "(sd-pam)",
        "statsbard",
        "statsbar-panel",
    ] {
        assert!(
            !can_quit(500, 1000, 1000, None, name),
            "{name} by kernel name"
        );
        assert!(
            !can_quit(500, 1000, 1000, Some(name), "x"),
            "{name} by executable"
        );
    }
    assert!(!can_quit(500, 1000, 1000, Some("statsbard (deleted)"), "x"));
    assert!(
        can_quit(
            500,
            1000,
            1000,
            Some("gnome-shell-calendar-server"),
            "gnome-shell-cal"
        ),
        "a protected name only matches whole"
    );
}
````

Create `crates/core/tests/system.rs`:

````rust
use statsbar_core::system::{
    Thermal, load_per_cpu, parse_cpu_list, parse_cpu_model, parse_loadavg, parse_millidegrees,
    parse_uptime,
};

#[test]
fn loadavg_yields_its_three_averages() {
    assert_eq!(
        parse_loadavg("0.96 1.24 1.06 2/1738 706483\n"),
        Some([0.96, 1.24, 1.06])
    );
    assert_eq!(parse_loadavg("0.96 1.24"), None);
    assert_eq!(parse_loadavg("NaN 1 1"), None);
    assert_eq!(parse_loadavg("-1 1 1"), None);
}

#[test]
fn load_is_divided_by_the_logical_cpus() {
    assert_eq!(load_per_cpu([8.0, 4.0, 2.0], 8), Some([1.0, 0.5, 0.25]));
    assert_eq!(load_per_cpu([1.0, 1.0, 1.0], 0), None);
}

#[test]
fn uptime_is_the_first_field() {
    assert_eq!(parse_uptime("125693.31 876543.21\n"), Some(125_693.31));
    for bad in ["", "x 1", "-5 1", "inf 1"] {
        assert_eq!(parse_uptime(bad), None, "{bad:?}");
    }
}

#[test]
fn the_model_is_the_first_non_empty_model_name() {
    let text = "processor\t: 0\nmodel name\t: Intel(R) Core(TM) i5-8365U CPU @ 1.60GHz\n\
                processor\t: 1\nmodel name\t: other\n";
    assert_eq!(
        parse_cpu_model(text).as_deref(),
        Some("Intel(R) Core(TM) i5-8365U CPU @ 1.60GHz")
    );
    assert_eq!(parse_cpu_model("model name\t: \n"), None);
    assert_eq!(parse_cpu_model("processor\t: 0\n"), None);
}

#[test]
fn a_cpu_list_expands_ranges_in_order() {
    assert_eq!(parse_cpu_list("0-3\n"), Some(vec![0, 1, 2, 3]));
    assert_eq!(parse_cpu_list("0-1,3,5-6"), Some(vec![0, 1, 3, 5, 6]));
    for bad in ["", "3-1", "a", "0-", "0-999999999"] {
        assert_eq!(parse_cpu_list(bad), None, "{bad:?}");
    }
}

#[test]
fn temperatures_come_in_millidegrees() {
    assert_eq!(parse_millidegrees("84000\n"), Some(84.0));
    assert_eq!(parse_millidegrees("45500"), Some(45.5));
    for bad in ["", "hot", "0", "-1000", "150000"] {
        assert_eq!(parse_millidegrees(bad), None, "{bad:?}");
    }
}

#[test]
fn thermal_levels_change_at_75_90_and_100_degrees() {
    use Thermal::{Critical, Fair, Nominal, Serious};
    for (celsius, expected) in [
        (20.0, Nominal),
        (74.999, Nominal),
        (75.0, Fair),
        (89.999, Fair),
        (90.0, Serious),
        (99.999, Serious),
        (100.0, Critical),
        (110.0, Critical),
    ] {
        assert_eq!(Thermal::from_celsius(celsius), expected, "{celsius}");
    }
}

#[test]
fn thermal_levels_are_named_as_upstream_names_them() {
    use Thermal::{Critical, Fair, Nominal, Serious};
    assert_eq!(
        [Nominal, Fair, Serious, Critical].map(Thermal::label),
        ["Nominal", "Fair", "Serious", "Critical"]
    );
}
````

- [x] **Step 2: Run them, expect FAIL**

```bash
cargo test -p statsbar-core --test process --test system --test pressure
```

Expected: the tests do not compile (E0432 for `process`, `system` and `Pressure`; E0599 for
`pressure_fraction`).

- [x] **Step 3: Implement**

Apply to `crates/core/src/lib.rs`:

````diff
diff --git a/crates/core/src/lib.rs b/crates/core/src/lib.rs
--- a/crates/core/src/lib.rs
+++ b/crates/core/src/lib.rs
@@ -13,6 +13,8 @@ pub mod history;
 pub mod label;
 pub mod memory;
 pub mod network;
+pub mod process;
 pub mod schedule;
 pub mod sysfs;
+pub mod system;
 pub mod throughput;
````

Apply to `crates/core/src/memory.rs`:

````diff
diff --git a/crates/core/src/memory.rs b/crates/core/src/memory.rs
--- a/crates/core/src/memory.rs
+++ b/crates/core/src/memory.rs
@@ -107,6 +107,12 @@ impl MemoryBuckets {
         self.fraction(self.used()) * 100.0
     }

+    /// How far the pressure ring fills: memory that cannot be reclaimed without work (kernel and
+    /// zswap pool), 0–1. Its colour and caption come from [`Pressure`].
+    pub fn pressure_fraction(&self) -> f64 {
+        self.fraction(self.kernel.saturating_add(self.compressed))
+    }
+
     /// `part` as a share of total memory, 0–1. Zero when the total is unknown.
     pub fn fraction(&self, part: u64) -> f64 {
         if self.total == 0 {
@@ -116,3 +122,46 @@ impl MemoryBuckets {
         }
     }
 }
+
+/// The `avg10` percentage at which each line of the PSI file raises the level.
+const PRESSURE_THRESHOLD: f64 = 10.0;
+
+/// Memory pressure, from `/proc/pressure/memory`.
+#[derive(Debug, Clone, Copy, PartialEq, Eq)]
+pub enum Pressure {
+    Normal,
+    Warning,
+    Critical,
+}
+
+impl Pressure {
+    /// Critical at `full avg10 >= 10`, else Warning at `some avg10 >= 10`. `None` when either
+    /// figure is missing or malformed: unknown pressure must not read as Normal.
+    pub fn from_psi(text: &str) -> Option<Pressure> {
+        let avg10 = |kind: &str| -> Option<f64> {
+            text.lines()
+                .find_map(|line| line.strip_prefix(kind)?.strip_prefix(" avg10="))?
+                .split_ascii_whitespace()
+                .next()?
+                .parse()
+                .ok()
+                .filter(|value: &f64| value.is_finite())
+        };
+        let (some, full) = (avg10("some")?, avg10("full")?);
+        Some(if full >= PRESSURE_THRESHOLD {
+            Pressure::Critical
+        } else if some >= PRESSURE_THRESHOLD {
+            Pressure::Warning
+        } else {
+            Pressure::Normal
+        })
+    }
+
+    pub fn label(self) -> &'static str {
+        match self {
+            Pressure::Normal => "Normal",
+            Pressure::Warning => "Warning",
+            Pressure::Critical => "Critical",
+        }
+    }
+}
````

Create `crates/core/src/process.rs`:

````rust
//! Processes: `/proc/<pid>` text in, the rows of the two top-process tables out.

use std::collections::HashMap;

const NANOS_PER_SECOND: f64 = 1e9;
/// What the kernel appends to `/proc/<pid>/exe` once the binary has been replaced.
const DELETED: &str = " (deleted)";
/// Quitting one of these would end the session or StatsBar itself.
const PROTECTED: [&str; 5] = [
    "gnome-shell",
    "systemd",
    "(sd-pam)",
    "statsbard",
    "statsbar-panel",
];

/// The fields of `/proc/<pid>/stat` the tables need. Times are in clock ticks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stat {
    pub comm: String,
    /// utime + stime.
    pub cpu_ticks: u64,
    /// Since boot. With the pid, it names one process for good, even after the pid is reused.
    pub start_ticks: u64,
}

/// Parses `/proc/<pid>/stat`. The name may itself hold spaces and parentheses, so it ends at the
/// last `)`.
pub fn parse_stat(text: &str) -> Option<Stat> {
    let (head, tail) = text.rsplit_once(')')?;
    let (_, comm) = head.split_once('(')?;
    let mut fields = tail.split_ascii_whitespace();
    // The tail starts at field 3 (state); utime is 14, stime 15 and starttime 22.
    let utime: u64 = fields.nth(11)?.parse().ok()?;
    let stime: u64 = fields.next()?.parse().ok()?;
    let start_ticks = fields.nth(6)?.parse().ok()?;
    Some(Stat {
        comm: comm.to_owned(),
        cpu_ticks: utime.saturating_add(stime),
        start_ticks,
    })
}

/// Parses `/proc/<pid>/statm` into resident − shared, in bytes: memory no other process shares,
/// as GNOME System Monitor's "Memory" column shows it.
pub fn parse_statm(text: &str, page_size: u64) -> Option<u64> {
    let mut fields = text.split_ascii_whitespace().skip(1);
    let resident: u64 = fields.next()?.parse().ok()?;
    let shared: u64 = fields.next()?.parse().ok()?;
    resident.saturating_sub(shared).checked_mul(page_size)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuSample {
    pub pid: u32,
    pub start_ticks: u64,
    pub cpu_ticks: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CpuUse {
    pub pid: u32,
    pub start_ticks: u64,
    /// 100 is one core, as `top` counts.
    pub percent: f64,
}

/// The `count` busiest processes between two scans. Only a process present in both counts, so a
/// new one appears one scan later and a reused pid never inherits the old process's time. Ties
/// break on the lower pid, so rows do not swap under the cursor.
pub fn rank_by_cpu(
    previous: &[CpuSample],
    current: &[CpuSample],
    elapsed_nanos: u64,
    ticks_per_second: u64,
    count: usize,
) -> Vec<CpuUse> {
    if elapsed_nanos == 0 || ticks_per_second == 0 {
        return Vec::new();
    }
    let elapsed_ticks = elapsed_nanos as f64 / NANOS_PER_SECOND * ticks_per_second as f64;
    let before: HashMap<(u32, u64), u64> = previous
        .iter()
        .map(|sample| ((sample.pid, sample.start_ticks), sample.cpu_ticks))
        .collect();
    let mut ranked: Vec<CpuUse> = current
        .iter()
        .filter_map(|sample| {
            let earlier = before.get(&(sample.pid, sample.start_ticks))?;
            Some(CpuUse {
                pid: sample.pid,
                start_ticks: sample.start_ticks,
                percent: sample.cpu_ticks.saturating_sub(*earlier) as f64 * 100.0 / elapsed_ticks,
            })
        })
        .collect();
    ranked.sort_unstable_by(|a, b| b.percent.total_cmp(&a.percent).then(a.pid.cmp(&b.pid)));
    ranked.truncate(count);
    ranked
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemorySample {
    pub pid: u32,
    pub bytes: u64,
}

/// The `count` heaviest processes, heaviest first, ties on the lower pid.
pub fn rank_by_memory(samples: &[MemorySample], count: usize) -> Vec<MemorySample> {
    let mut ranked = samples.to_vec();
    ranked.sort_unstable_by(|a, b| b.bytes.cmp(&a.bytes).then(a.pid.cmp(&b.pid)));
    ranked.truncate(count);
    ranked
}

/// The executable's file name, else the kernel's 15-character name. A process chooses both, so
/// control characters are replaced.
pub fn display_name(exe: Option<&str>, comm: &str) -> String {
    let exe = exe.map_or("", |name| name.trim_end_matches(DELETED));
    let name = [exe, comm]
        .into_iter()
        .find(|name| !name.trim().is_empty())
        .unwrap_or("—");
    name.chars()
        .map(|c| {
            if c.is_control() {
                char::REPLACEMENT_CHARACTER
            } else {
                c
            }
        })
        .collect()
}

/// Whether the panel may offer to quit this process: the user's own, and not one that would take
/// the session or StatsBar with it. Either name protects it, because `exe` of the user's
/// `systemd` is not readable.
pub fn can_quit(pid: u32, owner_uid: u32, own_uid: u32, exe: Option<&str>, comm: &str) -> bool {
    let exe = exe.map_or("", |name| name.trim_end_matches(DELETED));
    pid > 1 && owner_uid == own_uid && !PROTECTED.contains(&exe) && !PROTECTED.contains(&comm)
}
````

Create `crates/core/src/system.rs`:

````rust
//! System-wide figures of the CPU page: kernel text in, values out.

/// Above any CPU id Linux supports (`NR_CPUS` tops out at 8192); bounds a malformed range.
const MAX_CPU_ID: u32 = 8191;
/// A sensor outside this range is broken. Upstream drops the same readings.
const PLAUSIBLE_CELSIUS: std::ops::Range<f64> = 0.0..130.0;
const FAIR_FROM_CELSIUS: f64 = 75.0;
const SERIOUS_FROM_CELSIUS: f64 = 90.0;
const CRITICAL_FROM_CELSIUS: f64 = 100.0;

fn non_negative(text: &str) -> Option<f64> {
    text.parse()
        .ok()
        .filter(|value: &f64| value.is_finite() && *value >= 0.0)
}

/// The 1, 5 and 15 minute averages of `/proc/loadavg`.
pub fn parse_loadavg(text: &str) -> Option<[f64; 3]> {
    let mut fields = text.split_ascii_whitespace();
    let mut averages = [0.0; 3];
    for slot in &mut averages {
        *slot = non_negative(fields.next()?)?;
    }
    Some(averages)
}

/// Load per logical CPU: 1.00 is exactly as much runnable work as there are CPUs.
pub fn load_per_cpu(load: [f64; 3], cpus: usize) -> Option<[f64; 3]> {
    (cpus > 0).then(|| load.map(|value| value / cpus as f64))
}

/// Seconds since boot, the first field of `/proc/uptime`.
pub fn parse_uptime(text: &str) -> Option<f64> {
    non_negative(text.split_ascii_whitespace().next()?)
}

/// The first `model name` of `/proc/cpuinfo`.
pub fn parse_cpu_model(text: &str) -> Option<String> {
    text.lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(key, value)| key.trim_end() == "model name" && !value.trim().is_empty())
        .map(|(_, value)| value.trim().to_owned())
}

/// A kernel CPU list such as `0-3,6`, as `/sys/devices/system/cpu/online` prints it. The order
/// is that of `/proc/stat`'s `cpuN` lines, which is how a load is paired with a CPU id.
pub fn parse_cpu_list(text: &str) -> Option<Vec<u32>> {
    let mut cpus = Vec::new();
    for part in text.trim().split(',') {
        let (first, last) = part.split_once('-').unwrap_or((part, part));
        let (first, last): (u32, u32) = (first.parse().ok()?, last.parse().ok()?);
        // Also refuses a range that repeats or goes back, which bounds the list's length.
        if first > last || last > MAX_CPU_ID || cpus.last().is_some_and(|&cpu| cpu >= first) {
            return None;
        }
        cpus.extend(first..=last);
    }
    Some(cpus)
}

/// An hwmon `temp*_input` in °C.
pub fn parse_millidegrees(text: &str) -> Option<f64> {
    let celsius = text.trim().parse::<i32>().ok().map(f64::from)? / 1000.0;
    (celsius > 0.0 && PLAUSIBLE_CELSIUS.contains(&celsius)).then_some(celsius)
}

/// How hot the CPU package is. Derived: this machine's `temp1_max` equals its `temp1_crit`
/// (100 °C), so the levels are fixed temperatures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Thermal {
    Nominal,
    Fair,
    Serious,
    Critical,
}

impl Thermal {
    pub fn from_celsius(celsius: f64) -> Thermal {
        if celsius >= CRITICAL_FROM_CELSIUS {
            Thermal::Critical
        } else if celsius >= SERIOUS_FROM_CELSIUS {
            Thermal::Serious
        } else if celsius >= FAIR_FROM_CELSIUS {
            Thermal::Fair
        } else {
            Thermal::Nominal
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Thermal::Nominal => "Nominal",
            Thermal::Fair => "Fair",
            Thermal::Serious => "Serious",
            Thermal::Critical => "Critical",
        }
    }
}
````

- [x] **Step 4: Run, expect PASS**

```bash
cargo test -p statsbar-core && ./scripts/check-core-purity.sh
```

Expected: `process` 17, `system` 8 and `pressure` 4 pass, with every older test, and
`core purity: ok`.

- [x] **Step 5: Checkpoint**

Ready to commit: the files listed above — "feat(core): rank processes and derive thermal and pressure levels"

### Task 3: Define the wire format

**Test level:** unit. The tests pin exact JSON lines, which are the contract with 04b. They sit in
the daemon because core may not have dev-dependencies.

**Files:**

- Modify: `Cargo.lock` (lines after 383, after 580, after 619, after 636, after 934)
- Modify: `crates/core/Cargo.toml` (lines after 8)
- Modify: `crates/core/src/cpu.rs` (lines after 2, 60)
- Create: `crates/core/src/ipc.rs`
- Modify: `crates/core/src/lib.rs` (lines after 12)
- Modify: `crates/core/src/memory.rs` (lines after 2, 59, 130)
- Modify: `crates/core/src/system.rs` (lines after 2, 69)
- Modify: `crates/daemon/Cargo.toml` (lines after 19)
- Create: `crates/daemon/tests/protocol.rs`

**Interfaces:**

- Consumes: `cpu::CpuLoad`, `memory::{MemoryBuckets, Pressure}`, `system::Thermal`.
- Produces:

```rust
// crates/core/src/ipc.rs
pub const PROTOCOL_VERSION: u32 = 1;
pub struct Hello {
    pub protocol: u32,
}
impl Hello {
    pub fn current() -> Hello;
}
pub enum Page {
    Cpu,
    Memory,
}
pub enum Request {
    Subscribe {
        page: Page,
    },
    Unsubscribe,
    Refresh,
}
pub enum Event {
    Cpu(CpuPage),
    Memory(MemoryPage),
    Error {
        reason: ErrorReason,
        message: String,
    },
}
pub enum ErrorReason {
    VersionMismatch,
    BadRequest,
    Busy,
    HandshakeTimeout,
}
pub struct ProcessInfo {
    pub pid: u32,
    pub start_ticks: u64,
    pub name: String,
    pub can_quit: bool,
}
pub struct CpuProcess {
    pub process: ProcessInfo,
    pub cpu_percent: f64,
}
pub struct MemoryProcess {
    pub process: ProcessInfo,
    pub bytes: u64,
}
pub struct CpuPage {
    pub model: Option<String>,
    pub load: Option<CpuLoad>,
    pub temperature_c: Option<f64>,
    pub thermal: Option<Thermal>,
    pub core_temperatures_c: Vec<Option<f64>>,
    pub frequency_mhz: Option<f64>,
    pub load_per_cpu: Option<[f64; 3]>,
    pub uptime_seconds: Option<f64>,
    pub power_saver: bool,
    pub processes: Option<Vec<CpuProcess>>,
}
pub struct MemoryPage {
    pub buckets: MemoryBuckets,
    pub pressure: Option<Pressure>,
    pub processes: Option<Vec<MemoryProcess>>,
}

// crates/core/src/lib.rs
pub mod ipc;
```

- [x] **Step 1: Add the approved dependencies and write the failing tests**

The manifests and the lock file go first, so that the only failure is the missing protocol. The
lock diff pins what was audited; do not run `cargo update`.

Apply to `Cargo.lock`:

````diff
diff --git a/Cargo.lock b/Cargo.lock
--- a/Cargo.lock
+++ b/Cargo.lock
@@ -381,6 +381,12 @@ dependencies = [
  "hashbrown",
 ]

+[[package]]
+name = "itoa"
+version = "1.0.18"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "8f42a60cbdf9a97f5d2305f08a87dc4e09308d1276d28c869c684d7777685682"
+
 [[package]]
 name = "js-sys"
 version = "0.3.107"
@@ -578,6 +584,19 @@ dependencies = [
  "syn 3.0.10",
 ]

+[[package]]
+name = "serde_json"
+version = "1.0.154"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "e7e9cc8b1b85264074fbcc02a88680c4096b1e47df8f739dceb03bf482f04bd6"
+dependencies = [
+ "itoa",
+ "memchr",
+ "serde",
+ "serde_core",
+ "zmij",
+]
+
 [[package]]
 name = "serde_repr"
 version = "0.1.21"
@@ -617,6 +636,9 @@ checksum = "0c790de23124f9ab44544d7ac05d60440adc586479ce501c1d6d7da3cd8c9cf5"
 [[package]]
 name = "statsbar-core"
 version = "0.1.0"
+dependencies = [
+ "serde",
+]

 [[package]]
 name = "statsbar-readers"
@@ -634,6 +656,7 @@ dependencies = [
  "futures-lite",
  "ksni",
  "serde",
+ "serde_json",
  "statsbar-core",
  "statsbar-readers",
  "toml",
@@ -932,6 +955,12 @@ dependencies = [
  "serde",
 ]

+[[package]]
+name = "zmij"
+version = "1.0.23"
+source = "registry+https://github.com/rust-lang/crates.io-index"
+checksum = "29666d0abbfad1e3dc4dcf6144730dd3a3ab225bbbdac83319345b1b44ccfc1b"
+
 [[package]]
 name = "zvariant"
 version = "5.15.1"
````

Apply to `crates/core/Cargo.toml`:

````diff
diff --git a/crates/core/Cargo.toml b/crates/core/Cargo.toml
--- a/crates/core/Cargo.toml
+++ b/crates/core/Cargo.toml
@@ -6,6 +6,7 @@ rust-version.workspace = true
 publish.workspace = true

 [dependencies]
+serde = { version = "=1.0.229", features = ["derive"] }

 [lints]
 workspace = true
````

Apply to `crates/daemon/Cargo.toml`:

````diff
diff --git a/crates/daemon/Cargo.toml b/crates/daemon/Cargo.toml
--- a/crates/daemon/Cargo.toml
+++ b/crates/daemon/Cargo.toml
@@ -17,5 +17,8 @@ serde = { version = "=1.0.229", features = ["derive"] }
 toml = "=1.2.1"
 zbus = { version = "=5.19.0", default-features = false, features = ["async-io"] }

+[dev-dependencies]
+serde_json = "=1.0.154"
+
 [lints]
 workspace = true
````

Create `crates/daemon/tests/protocol.rs`:

````rust
//! The wire format shared with `statsbar-panel`. It lives here because `statsbar-core` may not
//! take `serde_json`, not even for its tests.

use statsbar_core::cpu::CpuLoad;
use statsbar_core::ipc::{
    CpuPage, CpuProcess, ErrorReason, Event, Hello, MemoryPage, MemoryProcess, PROTOCOL_VERSION,
    Page, ProcessInfo, Request,
};
use statsbar_core::memory::{MemoryBuckets, Pressure};
use statsbar_core::system::Thermal;

fn firefox() -> ProcessInfo {
    ProcessInfo {
        pid: 4242,
        start_ticks: 9001,
        name: "firefox".to_owned(),
        can_quit: true,
    }
}

fn json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("serialises")
}

#[test]
fn the_first_line_names_protocol_1() {
    assert_eq!(PROTOCOL_VERSION, 1);
    assert_eq!(json(&Hello::current()), r#"{"protocol":1}"#);
}

#[test]
fn requests_have_these_exact_lines() {
    for (request, line) in [
        (
            Request::Subscribe { page: Page::Cpu },
            r#"{"type":"subscribe","page":"cpu"}"#,
        ),
        (
            Request::Subscribe { page: Page::Memory },
            r#"{"type":"subscribe","page":"memory"}"#,
        ),
        (Request::Unsubscribe, r#"{"type":"unsubscribe"}"#),
        (Request::Refresh, r#"{"type":"refresh"}"#),
    ] {
        assert_eq!(json(&request), line);
        assert_eq!(
            serde_json::from_str::<Request>(line).expect("parses"),
            request
        );
    }
}

#[test]
fn a_cpu_event_has_this_exact_line() {
    let event = Event::Cpu(CpuPage {
        model: Some("i5".to_owned()),
        load: Some(CpuLoad {
            system_percent: 10.0,
            user_percent: 20.5,
            idle_percent: 69.5,
            per_core_busy: vec![30.5],
        }),
        temperature_c: Some(84.0),
        thermal: Some(Thermal::Fair),
        core_temperatures_c: vec![None],
        frequency_mhz: Some(1900.0),
        load_per_cpu: Some([1.0, 0.5, 0.25]),
        uptime_seconds: Some(3600.0),
        power_saver: false,
        processes: Some(vec![CpuProcess {
            process: firefox(),
            cpu_percent: 12.5,
        }]),
    });
    let line = concat!(
        r#"{"type":"cpu","model":"i5","load":{"system_percent":10.0,"user_percent":20.5,"#,
        r#""idle_percent":69.5,"per_core_busy":[30.5]},"temperature_c":84.0,"thermal":"fair","#,
        r#""core_temperatures_c":[null],"frequency_mhz":1900.0,"load_per_cpu":[1.0,0.5,0.25],"#,
        r#""uptime_seconds":3600.0,"power_saver":false,"processes":[{"process":{"pid":4242,"#,
        r#""start_ticks":9001,"name":"firefox","can_quit":true},"cpu_percent":12.5}]}"#,
    );
    assert_eq!(json(&event), line);
    assert_eq!(serde_json::from_str::<Event>(line).expect("parses"), event);
}

#[test]
fn a_memory_event_has_this_exact_line() {
    let event = Event::Memory(MemoryPage {
        buckets: MemoryBuckets {
            total: 1000,
            app: 400,
            kernel: 150,
            cached: 300,
            free: 150,
            swap_used: 7,
            compressed: 100,
        },
        pressure: Some(Pressure::Warning),
        processes: Some(vec![MemoryProcess {
            process: firefox(),
            bytes: 4096,
        }]),
    });
    let line = concat!(
        r#"{"type":"memory","buckets":{"total":1000,"app":400,"kernel":150,"cached":300,"#,
        r#""free":150,"swap_used":7,"compressed":100},"pressure":"warning","processes":["#,
        r#"{"process":{"pid":4242,"start_ticks":9001,"name":"firefox","can_quit":true},"#,
        r#""bytes":4096}]}"#,
    );
    assert_eq!(json(&event), line);
    assert_eq!(serde_json::from_str::<Event>(line).expect("parses"), event);
}

#[test]
fn errors_have_these_exact_reasons() {
    for (reason, name) in [
        (ErrorReason::VersionMismatch, "version_mismatch"),
        (ErrorReason::BadRequest, "bad_request"),
        (ErrorReason::Busy, "busy"),
        (ErrorReason::HandshakeTimeout, "handshake_timeout"),
    ] {
        let event = Event::Error {
            reason,
            message: "why".to_owned(),
        };
        let line = format!(r#"{{"type":"error","reason":"{name}","message":"why"}}"#);
        assert_eq!(json(&event), line);
        assert_eq!(serde_json::from_str::<Event>(&line).expect("parses"), event);
    }
}

#[test]
fn unknown_fields_are_ignored() {
    let hello: Hello = serde_json::from_str(r#"{"protocol":1,"client":"x"}"#).expect("parses");
    assert_eq!(hello, Hello::current());
    let request: Request =
        serde_json::from_str(r#"{"type":"refresh","later":true}"#).expect("parses");
    assert_eq!(request, Request::Refresh);
    let request: Request =
        serde_json::from_str(r#"{"type":"subscribe","page":"cpu","later":1}"#).expect("parses");
    assert_eq!(request, Request::Subscribe { page: Page::Cpu });
}

#[test]
fn an_absent_optional_field_reads_as_none() {
    let line = r#"{"type":"cpu","core_temperatures_c":[],"power_saver":true}"#;
    let Event::Cpu(page) = serde_json::from_str(line).expect("parses") else {
        panic!("not a cpu event");
    };
    assert_eq!(
        (page.load, page.processes, page.thermal),
        (None, None, None)
    );
    assert!(page.power_saver);
}

#[test]
fn an_unknown_request_or_page_does_not_parse() {
    for line in [
        r#"{"type":"kill","pid":1}"#,
        r#"{"type":"subscribe","page":"gpu"}"#,
    ] {
        assert!(serde_json::from_str::<Request>(line).is_err(), "{line}");
    }
}
````

- [x] **Step 2: Run them, expect FAIL**

```bash
cargo test -p statsbard --test protocol
```

Expected: the test does not compile (E0432, `statsbar_core::ipc`).

- [x] **Step 3: Implement**

Apply to `crates/core/src/cpu.rs`:

````diff
diff --git a/crates/core/src/cpu.rs b/crates/core/src/cpu.rs
--- a/crates/core/src/cpu.rs
+++ b/crates/core/src/cpu.rs
@@ -1,5 +1,7 @@
 //! CPU load: `/proc/stat` text in, percentages out.

+use serde::{Deserialize, Serialize};
+
 /// Cumulative jiffies of one logical CPU, folded into the buckets the UI shows.
 #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
 pub struct CoreTicks {
@@ -57,7 +59,7 @@ pub fn parse_proc_stat(text: &str) -> Option<Vec<CoreTicks>> {
 }

 /// CPU usage over the interval between two `/proc/stat` samples.
-#[derive(Debug, Clone, PartialEq)]
+#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
 pub struct CpuLoad {
     pub system_percent: f64,
     pub user_percent: f64,
````

Create `crates/core/src/ipc.rs`:

````rust
//! What `statsbard` and `statsbar-panel` say to each other: one JSON object per line. The first
//! line each way is a [`Hello`]. Then the panel sends [`Request`]s and the daemon [`Event`]s.
//!
//! Receivers ignore fields they do not know, and an absent optional field reads as `None`, so a
//! field can be added within a version. Anything else needs a new [`PROTOCOL_VERSION`].

use serde::{Deserialize, Serialize};

use crate::cpu::CpuLoad;
use crate::memory::{MemoryBuckets, Pressure};
use crate::system::Thermal;

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    pub protocol: u32,
}

impl Hello {
    pub fn current() -> Hello {
        Hello {
            protocol: PROTOCOL_VERSION,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Page {
    Cpu,
    Memory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    /// Replaces the connection's earlier subscription: a connection watches one page.
    Subscribe {
        page: Page,
    },
    Unsubscribe,
    /// Read the subscribed page again without waiting for its interval.
    Refresh,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    Cpu(CpuPage),
    Memory(MemoryPage),
    /// Always the last line before the daemon closes the connection.
    Error {
        reason: ErrorReason,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorReason {
    VersionMismatch,
    BadRequest,
    Busy,
    HandshakeTimeout,
}

/// Names one process. The panel compares `start_ticks` again before it signals `pid`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub start_ticks: u64,
    pub name: String,
    pub can_quit: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CpuProcess {
    pub process: ProcessInfo,
    /// 100 is one core.
    pub cpu_percent: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryProcess {
    pub process: ProcessInfo,
    /// Resident − shared.
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CpuPage {
    pub model: Option<String>,
    /// `None` until the second sample.
    pub load: Option<CpuLoad>,
    /// The package sensor.
    pub temperature_c: Option<f64>,
    pub thermal: Option<Thermal>,
    /// In the order of `load.per_core_busy`. Two logical CPUs of one core share a value.
    pub core_temperatures_c: Vec<Option<f64>>,
    /// The mean over the online CPUs.
    pub frequency_mhz: Option<f64>,
    /// 1, 5 and 15 minute averages, each divided by the number of online CPUs.
    pub load_per_cpu: Option<[f64; 3]>,
    pub uptime_seconds: Option<f64>,
    pub power_saver: bool,
    /// `None` until the first ranking.
    pub processes: Option<Vec<CpuProcess>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryPage {
    pub buckets: MemoryBuckets,
    /// `None` when PSI cannot be read.
    pub pressure: Option<Pressure>,
    /// `None` until the first ranking.
    pub processes: Option<Vec<MemoryProcess>>,
}
````

Apply to `crates/core/src/lib.rs`:

````diff
diff --git a/crates/core/src/lib.rs b/crates/core/src/lib.rs
--- a/crates/core/src/lib.rs
+++ b/crates/core/src/lib.rs
@@ -10,6 +10,7 @@ pub mod cpu;
 pub mod fan;
 pub mod format;
 pub mod history;
+pub mod ipc;
 pub mod label;
 pub mod memory;
 pub mod network;
````

Apply to `crates/core/src/memory.rs`:

````diff
diff --git a/crates/core/src/memory.rs b/crates/core/src/memory.rs
--- a/crates/core/src/memory.rs
+++ b/crates/core/src/memory.rs
@@ -1,5 +1,7 @@
 //! Memory: `/proc/meminfo` text in, the buckets of the memory bar out.

+use serde::{Deserialize, Serialize};
+
 /// The `/proc/meminfo` fields the buckets are built from, in bytes.
 #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
 pub struct MemInfo {
@@ -56,7 +58,7 @@ pub fn parse_meminfo(text: &str) -> Option<MemInfo> {
 /// The segments of the memory bar. `app + kernel + cached + free == total` always holds, so
 /// the bar never over- or under-fills, and `app + kernel` is the "used" figure GNOME System
 /// Monitor and `free` show: `MemTotal − MemAvailable`.
-#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
+#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
 pub struct MemoryBuckets {
     pub total: u64,
     /// Used memory that is not kernel memory: processes, tmpfs and shared memory.
@@ -127,7 +129,8 @@ impl MemoryBuckets {
 const PRESSURE_THRESHOLD: f64 = 10.0;

 /// Memory pressure, from `/proc/pressure/memory`.
-#[derive(Debug, Clone, Copy, PartialEq, Eq)]
+#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
+#[serde(rename_all = "snake_case")]
 pub enum Pressure {
     Normal,
     Warning,
````

Apply to `crates/core/src/system.rs`:

````diff
diff --git a/crates/core/src/system.rs b/crates/core/src/system.rs
--- a/crates/core/src/system.rs
+++ b/crates/core/src/system.rs
@@ -1,5 +1,7 @@
 //! System-wide figures of the CPU page: kernel text in, values out.

+use serde::{Deserialize, Serialize};
+
 /// Above any CPU id Linux supports (`NR_CPUS` tops out at 8192); bounds a malformed range.
 const MAX_CPU_ID: u32 = 8191;
 /// A sensor outside this range is broken. Upstream drops the same readings.
@@ -66,7 +68,8 @@ pub fn parse_millidegrees(text: &str) -> Option<f64> {

 /// How hot the CPU package is. Derived: this machine's `temp1_max` equals its `temp1_crit`
 /// (100 °C), so the levels are fixed temperatures.
-#[derive(Debug, Clone, Copy, PartialEq, Eq)]
+#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
+#[serde(rename_all = "snake_case")]
 pub enum Thermal {
     Nominal,
     Fair,
````

- [x] **Step 4: Run, expect PASS**

```bash
cargo test -p statsbard --test protocol && cargo test -p statsbar-core && ./scripts/check-core-purity.sh
```

Expected: `protocol` 8 pass, with every core test, and `core purity: ok`.

- [x] **Step 5: Checkpoint**

Ready to commit: the files listed above — "feat(core): define the panel protocol"

### Task 4: Read processes, CPU details and pressure

**Test level:** integration against the file system: fixture trees, and the real `/proc` for one
test. Moving `fan.rs`'s hwmon lookup into `hwmon.rs` is `RED: n/a — behavior-preserving, covered by
crates/readers/tests/fan.rs`, run before and after. The readers themselves are test-first.

**Files:**

- Create: `crates/readers/src/cpu_details.rs`
- Modify: `crates/readers/src/fan.rs` (lines 3, 5, after 9, 12–13, 26, 45–63)
- Create: `crates/readers/src/hwmon.rs`
- Modify: `crates/readers/src/lib.rs` (lines after 5, after 8, after 9, after 11)
- Create: `crates/readers/src/pressure.rs`
- Create: `crates/readers/src/processes.rs`
- Modify: `crates/readers/src/procfile.rs` (lines 4, after 61)
- Create: `crates/readers/tests/cpu_details.rs`
- Create: `crates/readers/tests/pressure.rs`
- Create: `crates/readers/tests/processes.rs`
- Create: `crates/readers/tests/support/mod.rs`

**Interfaces:**

- Consumes: `process::{parse_stat, parse_statm, display_name, can_quit, CpuSample, MemorySample}`,
  the `system` parsers, `memory::Pressure`, `ipc::ProcessInfo`.
- Produces:

```rust
// crates/readers/src/cpu_details.rs
pub struct CpuDetails {
    pub model: Option<String>,
    pub package_celsius: Option<f64>,
    pub core_celsius: Vec<Option<f64>>,
    pub frequency_mhz: Option<f64>,
    pub load_per_cpu: Option<[f64; 3]>,
    pub uptime_seconds: Option<f64>,
}
pub type Problem = (&'static str, io::Error);
pub struct CpuDetailsReader { /* private */ }
impl CpuDetailsReader {
    pub fn new(root: &Path) -> CpuDetailsReader;
    pub fn read(&mut self) -> (CpuDetails, Vec<Problem>);
}

// crates/readers/src/lib.rs
pub mod cpu_details;
pub mod pressure;
pub mod processes;

// crates/readers/src/pressure.rs
pub struct PressureReader { /* private */ }
impl PressureReader {
    pub fn new(root: &Path) -> PressureReader;
    pub fn read(&mut self) -> io::Result<Pressure>;
}

// crates/readers/src/processes.rs
pub struct ProcessReader { /* private */ }
impl ProcessReader {
    pub fn new(root: &Path, own_uid: u32, page_size: u64) -> ProcessReader;
    pub fn cpu_samples(&self) -> io::Result<Vec<CpuSample>>;
    pub fn memory_samples(&self) -> io::Result<Vec<MemorySample>>;
    pub fn identify(&self, pid: u32) -> Option<ProcessInfo>;
}
```

- [x] **Step 1: Run the covering test of the move, expect PASS**

```bash
cargo test -p statsbar-readers --test fan
```

- [x] **Step 2: Write the failing tests**

Create `crates/readers/tests/cpu_details.rs`:

````rust
mod support;

use std::fs;
use std::path::Path;

use statsbar_readers::cpu_details::{CpuDetails, CpuDetailsReader};
use support::Tree;

const TEMPS: &str = "sys/class/hwmon/hwmon8";

/// Two cores with two threads each, numbered as this laptop numbers them.
fn laptop(name: &str) -> Tree {
    let tree = Tree::new(name);
    tree.write("sys/devices/system/cpu/online", "0-3\n");
    for (cpu, core, khz) in [
        (0, 0, 800_000),
        (1, 1, 1_000_000),
        (2, 0, 1_200_000),
        (3, 1, 1_400_000),
    ] {
        let dir = format!("sys/devices/system/cpu/cpu{cpu}");
        tree.write(&format!("{dir}/topology/core_id"), format!("{core}\n"));
        tree.write(
            &format!("{dir}/cpufreq/scaling_cur_freq"),
            format!("{khz}\n"),
        );
    }
    tree.write("sys/class/hwmon/hwmon1/name", "nvme\n");
    tree.write(&format!("{TEMPS}/name"), "coretemp\n");
    for (index, label, milli) in [
        (1, "Package id 0", 84000),
        (2, "Core 0", 82000),
        (3, "Core 1", 81000),
    ] {
        tree.write(&format!("{TEMPS}/temp{index}_label"), format!("{label}\n"));
        tree.write(&format!("{TEMPS}/temp{index}_input"), format!("{milli}\n"));
    }
    tree.write("proc/loadavg", "4.00 2.00 1.00 2/1738 706483\n");
    tree.write("proc/uptime", "125693.31 876543.21\n");
    tree.write("proc/cpuinfo", "processor\t: 0\nmodel name\t: i5-8365U\n");
    tree
}

/// The details, and what could not be read.
fn read(reader: &mut CpuDetailsReader) -> (CpuDetails, Vec<&'static str>) {
    let (details, problems) = reader.read();
    (
        details,
        problems.into_iter().map(|(what, _)| what).collect(),
    )
}

#[test]
fn every_figure_is_read_and_cores_are_paired_through_topology() {
    let tree = laptop("details-all");
    assert_eq!(
        read(&mut CpuDetailsReader::new(&tree.root)),
        (
            CpuDetails {
                model: Some("i5-8365U".to_owned()),
                package_celsius: Some(84.0),
                core_celsius: vec![Some(82.0), Some(81.0), Some(82.0), Some(81.0)],
                frequency_mhz: Some(1100.0),
                load_per_cpu: Some([1.0, 0.5, 0.25]),
                uptime_seconds: Some(125_693.31),
            },
            vec![]
        )
    );
}

#[test]
fn a_second_read_sees_new_values() {
    let tree = laptop("details-reread");
    let mut reader = CpuDetailsReader::new(&tree.root);
    read(&mut reader);
    tree.write(&format!("{TEMPS}/temp1_input"), "91000\n");
    tree.write("proc/loadavg", "8.00 2.00 1.00 2/1738 706483\n");
    let (details, _) = read(&mut reader);
    assert_eq!(details.package_celsius, Some(91.0));
    assert_eq!(details.load_per_cpu, Some([2.0, 0.5, 0.25]));
}

#[test]
fn an_offline_cpu_does_not_shift_the_others_temperatures() {
    let tree = laptop("details-offline");
    let mut reader = CpuDetailsReader::new(&tree.root);
    read(&mut reader);
    tree.write("sys/devices/system/cpu/online", "0,2-3\n");
    let (details, _) = read(&mut reader);
    assert_eq!(details.core_celsius, [Some(82.0), Some(82.0), Some(81.0)]);
    assert_eq!(
        details.load_per_cpu,
        Some([4.0 / 3.0, 2.0 / 3.0, 1.0 / 3.0])
    );
}

#[test]
fn without_coretemp_only_the_temperatures_are_missing() {
    let tree = laptop("details-no-coretemp");
    tree.write(&format!("{TEMPS}/name"), "k10temp\n");
    let (details, problems) = read(&mut CpuDetailsReader::new(&tree.root));
    assert_eq!(details.package_celsius, None);
    assert_eq!(details.core_celsius, [None; 4]);
    assert_eq!(details.frequency_mhz, Some(1100.0));
    assert_eq!(problems, ["temperatures"]);
}

#[test]
fn a_sensor_that_moves_to_another_hwmon_is_found_again() {
    let tree = laptop("details-renumbered");
    let mut reader = CpuDetailsReader::new(&tree.root);
    read(&mut reader);
    std::fs::rename(
        tree.root.join(TEMPS),
        tree.root.join("sys/class/hwmon/hwmon5"),
    )
    .expect("move");
    // The open file still answers, so make it fail as a removed sysfs attribute does.
    tree.write("sys/class/hwmon/hwmon5/temp1_input", "");
    assert_eq!(read(&mut reader).1, ["temperatures"]);
    tree.write("sys/class/hwmon/hwmon5/temp1_input", "70000\n");
    let (details, problems) = read(&mut reader);
    assert_eq!((details.package_celsius, problems), (Some(70.0), vec![]));
}

#[test]
fn without_cpufreq_only_the_frequency_is_missing() {
    let tree = laptop("details-no-cpufreq");
    let gone = "sys/devices/system/cpu/cpu2/cpufreq/scaling_cur_freq";
    std::fs::remove_file(tree.root.join(gone)).expect("remove");
    let (details, problems) = read(&mut CpuDetailsReader::new(&tree.root));
    assert_eq!(
        details.frequency_mhz, None,
        "not the mean of the other three"
    );
    assert_eq!(details.package_celsius, Some(84.0));
    assert_eq!(problems, ["frequency"]);
}

#[test]
fn an_empty_root_gives_no_figures_and_names_each_gap() {
    let tree = Tree::new("details-empty");
    let (details, problems) = read(&mut CpuDetailsReader::new(&tree.root));
    assert_eq!(details, CpuDetails::default());
    assert_eq!(
        problems,
        [
            "online CPUs",
            "model",
            "temperatures",
            "frequency",
            "load average",
            "uptime"
        ]
    );
}

/// This machine, or the CI runner: whatever it lacks may only be the optional sensors.
#[test]
fn the_real_root_gives_load_and_uptime_and_names_each_gap() {
    let (details, problems) = CpuDetailsReader::new(Path::new("/")).read();
    let gaps: Vec<&str> = problems.iter().map(|(what, _)| *what).collect();
    for gap in &gaps {
        assert!(
            ["temperatures", "frequency", "model"].contains(gap),
            "{gap}: {problems:?}"
        );
    }
    assert!(details.uptime_seconds.is_some_and(|seconds| seconds > 0.0));
    assert!(details.load_per_cpu.is_some());
    assert!(!details.core_celsius.is_empty(), "online CPUs are known");
    let coretemp = fs::read_dir("/sys/class/hwmon").is_ok_and(|devices| {
        devices.filter_map(Result::ok).any(|device| {
            fs::read_to_string(device.path().join("name"))
                .is_ok_and(|name| name.trim() == "coretemp")
        })
    });
    if coretemp {
        assert!(
            details
                .package_celsius
                .is_some_and(|celsius| (1.0..=125.0).contains(&celsius)),
            "{details:?}"
        );
    }
    if Path::new("/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq").exists() {
        assert!(
            details.frequency_mhz.is_some_and(|mhz| mhz > 0.0),
            "{details:?}"
        );
    }
}
````

Create `crates/readers/tests/pressure.rs`:

````rust
mod support;

use std::io::ErrorKind;
use std::path::Path;

use statsbar_core::memory::Pressure;
use statsbar_readers::pressure::PressureReader;
use support::Tree;

const PSI: &str = "proc/pressure/memory";

fn psi(some: &str) -> String {
    format!(
        "some avg10={some} avg60=0.00 avg300=0.00 total=6639897\n\
         full avg10=0.00 avg60=0.00 avg300=0.00 total=5310078\n"
    )
}

#[test]
fn reads_the_level_and_rereads_it() {
    let tree = Tree::new("pressure-reread");
    tree.write(PSI, psi("0.00"));
    let mut reader = PressureReader::new(&tree.root);
    assert_eq!(reader.read().expect("well-formed"), Pressure::Normal);
    tree.write(PSI, psi("11.00"));
    assert_eq!(reader.read().expect("well-formed"), Pressure::Warning);
}

#[test]
fn a_kernel_without_psi_is_not_found_and_is_retried() {
    let tree = Tree::new("pressure-missing");
    let mut reader = PressureReader::new(&tree.root);
    assert_eq!(
        reader.read().expect_err("no file").kind(),
        ErrorKind::NotFound
    );
    tree.write(PSI, psi("0.00"));
    assert_eq!(reader.read().expect("there now"), Pressure::Normal);
}

#[test]
fn a_malformed_file_is_invalid_data() {
    let tree = Tree::new("pressure-malformed");
    tree.write(PSI, "some\n");
    let mut reader = PressureReader::new(&tree.root);
    assert_eq!(
        reader.read().expect_err("malformed").kind(),
        ErrorKind::InvalidData
    );
}

/// This kernel, or the CI runner's: read when it has PSI, not found when it has none.
#[test]
fn the_real_root_is_read_when_the_kernel_has_psi() {
    let result = PressureReader::new(Path::new("/")).read();
    if Path::new("/proc/pressure/memory").exists() {
        result.expect("a kernel with PSI");
    } else {
        assert_eq!(result.expect_err("no PSI").kind(), ErrorKind::NotFound);
    }
}
````

Create `crates/readers/tests/processes.rs`:

````rust
mod support;

use std::os::unix::fs::{MetadataExt, symlink};
use std::path::Path;

use statsbar_core::process::{CpuSample, MemorySample};
use statsbar_readers::processes::ProcessReader;
use support::Tree;

const PAGE: u64 = 4096;

fn stat(pid: u32, comm: &str, utime: u64, start: u64) -> String {
    format!("{pid} ({comm}) S 1 1 1 0 -1 0 0 0 0 0 {utime} 2 0 0 20 0 1 0 {start} 0 0\n")
}

fn process(tree: &Tree, pid: u32, comm: &str, utime: u64, resident: u64) {
    tree.write(
        &format!("proc/{pid}/stat"),
        stat(pid, comm, utime, u64::from(pid) * 10),
    );
    tree.write(
        &format!("proc/{pid}/statm"),
        format!("9000 {resident} 100 0 0 0 0\n"),
    );
}

/// The uid that owns the fixture trees, which is the one running the tests.
fn uid(tree: &Tree) -> u32 {
    std::fs::metadata(&tree.root)
        .expect("the root exists")
        .uid()
}

fn reader(tree: &Tree) -> ProcessReader {
    ProcessReader::new(&tree.root, uid(tree), PAGE)
}

#[test]
fn cpu_samples_cover_every_numeric_directory() {
    let tree = Tree::new("processes-cpu");
    process(&tree, 10, "ten", 40, 500);
    process(&tree, 20, "twenty", 80, 700);
    tree.write("proc/meminfo", "MemTotal: 1 kB\n");
    tree.write("proc/acpi/wakeup", "x\n");
    let mut samples = reader(&tree).cpu_samples().expect("proc exists");
    samples.sort_by_key(|sample| sample.pid);
    assert_eq!(
        samples,
        [
            CpuSample {
                pid: 10,
                start_ticks: 100,
                cpu_ticks: 42
            },
            CpuSample {
                pid: 20,
                start_ticks: 200,
                cpu_ticks: 82
            },
        ]
    );
}

#[test]
fn memory_samples_are_resident_minus_shared() {
    let tree = Tree::new("processes-memory");
    process(&tree, 10, "ten", 0, 500);
    assert_eq!(
        reader(&tree).memory_samples().expect("proc exists"),
        [MemorySample {
            pid: 10,
            bytes: 400 * PAGE
        }]
    );
}

#[test]
fn a_vanished_or_unreadable_process_is_skipped() {
    let tree = Tree::new("processes-vanished");
    process(&tree, 10, "ten", 40, 500);
    // Exited after the directory was listed: nothing left inside.
    tree.write("proc/11/cmdline", "");
    tree.write("proc/12/stat", "12 (cut) S 1");
    tree.write("proc/12/statm", "9");
    let reader = reader(&tree);
    assert_eq!(reader.cpu_samples().expect("proc exists").len(), 1);
    assert_eq!(reader.memory_samples().expect("proc exists").len(), 1);
}

#[test]
fn a_name_that_is_not_utf8_still_gives_a_sample() {
    let tree = Tree::new("processes-not-utf8");
    let mut line = b"10 (bad".to_vec();
    line.push(0xff);
    line.extend_from_slice(b") S 1 1 1 0 -1 0 0 0 0 0 5 1 0 0 20 0 1 0 77 0 0\n");
    tree.write("proc/10/stat", line);
    let reader = reader(&tree);
    assert_eq!(
        reader.cpu_samples().expect("proc exists"),
        [CpuSample {
            pid: 10,
            start_ticks: 77,
            cpu_ticks: 6
        }]
    );
    assert_eq!(
        reader.identify(10).expect("still there").name,
        "bad\u{fffd}"
    );
}

#[test]
fn without_proc_or_without_one_readable_process_the_scan_fails() {
    let tree = Tree::new("processes-none");
    let reader = reader(&tree);
    assert!(reader.cpu_samples().is_err(), "no proc");
    tree.write("proc/meminfo", "MemTotal: 1 kB\n");
    assert!(reader.cpu_samples().is_err(), "no process");
    assert!(reader.memory_samples().is_err(), "no process");
}

#[test]
fn identify_prefers_the_executable_and_reports_the_start_time() {
    let tree = Tree::new("processes-identify");
    process(&tree, 10, "gnome-keyring-d", 0, 500);
    symlink(
        "/usr/bin/gnome-keyring-daemon",
        tree.root.join("proc/10/exe"),
    )
    .expect("symlink");
    let info = reader(&tree).identify(10).expect("still there");
    assert_eq!(
        (
            info.pid,
            info.start_ticks,
            info.name.as_str(),
            info.can_quit
        ),
        (10, 100, "gnome-keyring-daemon", true)
    );
}

#[test]
fn identify_falls_back_to_the_kernel_name_and_still_protects_by_it() {
    let tree = Tree::new("processes-no-exe");
    process(&tree, 10, "systemd", 0, 500);
    let info = reader(&tree).identify(10).expect("still there");
    assert_eq!((info.name.as_str(), info.can_quit), ("systemd", false));
}

#[test]
fn another_users_process_cannot_be_quit() {
    let tree = Tree::new("processes-other-uid");
    process(&tree, 10, "ten", 0, 500);
    let reader = ProcessReader::new(&tree.root, uid(&tree) + 1, PAGE);
    assert!(!reader.identify(10).expect("still there").can_quit);
}

#[test]
fn identify_of_an_exited_process_is_none() {
    let tree = Tree::new("processes-exited");
    assert_eq!(reader(&tree).identify(10), None);
}

#[test]
fn this_test_process_is_found_in_the_real_proc() {
    let pid = std::process::id();
    let own_uid = std::fs::metadata("/proc/self").expect("procfs").uid();
    let reader = ProcessReader::new(Path::new("/"), own_uid, PAGE);
    let cpu = reader.cpu_samples().expect("procfs");
    let seen = cpu.iter().find(|sample| sample.pid == pid).expect("listed");
    let memory = reader.memory_samples().expect("procfs");
    assert!(
        memory
            .iter()
            .any(|sample| sample.pid == pid && sample.bytes > 0)
    );
    let info = reader.identify(pid).expect("running");
    assert_eq!(info.start_ticks, seen.start_ticks);
    assert!(info.name.starts_with("processes-"), "{}", info.name);
    assert!(info.can_quit);
}
````

Create `crates/readers/tests/support/mod.rs`:

````rust
//! Fixture trees written by the test that reads them.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub struct Tree {
    pub root: PathBuf,
}

impl Tree {
    /// An empty tree. `name` must be unique across the test binaries that share the directory.
    pub fn new(name: &str) -> Tree {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
        match fs::remove_dir_all(&root) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => panic!("clearing {}: {error}", root.display()),
        }
        fs::create_dir_all(&root).expect("create the root");
        Tree { root }
    }

    pub fn write(&self, path: &str, contents: impl AsRef<[u8]>) {
        let path = self.root.join(path);
        fs::create_dir_all(path.parent().expect("has a parent")).expect("create the directory");
        fs::write(path, contents).expect("write");
    }
}
````

- [x] **Step 3: Run them, expect FAIL**

```bash
cargo test -p statsbar-readers --test processes --test cpu_details --test pressure
```

Expected: the tests do not compile (E0432 for the three reader modules).

- [x] **Step 4: Implement**

Create `crates/readers/src/cpu_details.rs`:

````rust
//! The CPU page's figures beyond load: model, temperatures, frequency, load average, uptime.
//! Read only while that page is open.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use statsbar_core::sysfs::parse_u64;
use statsbar_core::system::{
    load_per_cpu, parse_cpu_list, parse_cpu_model, parse_loadavg, parse_millidegrees, parse_uptime,
};

use crate::hwmon::find_device;
use crate::procfile::{ProcFile, Reopening, invalid_data};

const DRIVER: &str = "coretemp";
const CPUS: &str = "sys/devices/system/cpu";
const KHZ_PER_MHZ: f64 = 1000.0;

/// Any figure may be missing without taking the others with it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CpuDetails {
    pub model: Option<String>,
    pub package_celsius: Option<f64>,
    /// One per online CPU, in `/proc/stat`'s order.
    pub core_celsius: Vec<Option<f64>>,
    pub frequency_mhz: Option<f64>,
    pub load_per_cpu: Option<[f64; 3]>,
    pub uptime_seconds: Option<f64>,
}

/// A figure that could not be read, and why.
pub type Problem = (&'static str, io::Error);

struct Cpu {
    id: u32,
    core: Option<u64>,
    frequency: Reopening,
}

struct Sensors {
    package: Reopening,
    cores: Vec<(u64, Reopening)>,
}

pub struct CpuDetailsReader {
    root: PathBuf,
    online: Reopening,
    loadavg: Reopening,
    uptime: Reopening,
    /// Fixed until reboot, so read once.
    model: Option<String>,
    cpus: Vec<Cpu>,
    sensors: Option<Sensors>,
}

impl CpuDetailsReader {
    pub fn new(root: &Path) -> CpuDetailsReader {
        CpuDetailsReader {
            root: root.to_owned(),
            online: Reopening::new(root.join(CPUS).join("online")),
            loadavg: Reopening::new(root.join("proc/loadavg")),
            uptime: Reopening::new(root.join("proc/uptime")),
            model: None,
            cpus: Vec::new(),
            sensors: None,
        }
    }

    pub fn read(&mut self) -> (CpuDetails, Vec<Problem>) {
        let mut problems = Vec::new();
        let online = keep(
            "online CPUs",
            self.online.read(parse_cpu_list),
            &mut problems,
        )
        .unwrap_or_default();
        if !self
            .cpus
            .iter()
            .map(|cpu| cpu.id)
            .eq(online.iter().copied())
        {
            self.cpus = online.iter().map(|&id| open_cpu(&self.root, id)).collect();
        }
        if self.model.is_none() {
            self.model = keep("model", read_model(&self.root), &mut problems);
        }

        let mut details = CpuDetails {
            model: self.model.clone(),
            core_celsius: vec![None; self.cpus.len()],
            ..CpuDetails::default()
        };
        if let Some((package, cores)) = keep("temperatures", self.temperatures(), &mut problems) {
            details.package_celsius = Some(package);
            for (slot, cpu) in details.core_celsius.iter_mut().zip(&self.cpus) {
                *slot = cores
                    .iter()
                    .find(|(core, _)| cpu.core == Some(*core))
                    .map(|&(_, celsius)| celsius);
            }
        }
        details.frequency_mhz = keep("frequency", self.frequency(), &mut problems);
        details.load_per_cpu = keep(
            "load average",
            self.loadavg.read(parse_loadavg),
            &mut problems,
        )
        .and_then(|load| load_per_cpu(load, self.cpus.len()));
        details.uptime_seconds = keep("uptime", self.uptime.read(parse_uptime), &mut problems);
        (details, problems)
    }

    /// The package and each core. On a failure the sensors are looked up again next time, since
    /// a driver reload renumbers hwmon.
    fn temperatures(&mut self) -> io::Result<(f64, Vec<(u64, f64)>)> {
        let mut sensors = match self.sensors.take() {
            Some(sensors) => sensors,
            None => find_sensors(&self.root)?,
        };
        let package = sensors.package.read(parse_millidegrees)?;
        let mut cores = Vec::with_capacity(sensors.cores.len());
        for (core, input) in &mut sensors.cores {
            cores.push((*core, input.read(parse_millidegrees)?));
        }
        self.sensors = Some(sensors);
        Ok((package, cores))
    }

    /// The mean of every online CPU; a mean of some of them would be a different figure.
    fn frequency(&mut self) -> io::Result<f64> {
        if self.cpus.is_empty() {
            return Err(invalid_data("no online CPU is known"));
        }
        let mut total_khz = 0.0;
        for cpu in &mut self.cpus {
            total_khz += cpu.frequency.read(parse_u64)? as f64;
        }
        Ok(total_khz / self.cpus.len() as f64 / KHZ_PER_MHZ)
    }
}

fn keep<T>(what: &'static str, result: io::Result<T>, problems: &mut Vec<Problem>) -> Option<T> {
    result.map_err(|error| problems.push((what, error))).ok()
}

fn open_cpu(root: &Path, id: u32) -> Cpu {
    let dir = root.join(CPUS).join(format!("cpu{id}"));
    Cpu {
        id,
        // A CPU without topology only loses its temperature, which the page shows as missing.
        core: Reopening::new(dir.join("topology/core_id"))
            .read(parse_u64)
            .ok(),
        frequency: Reopening::new(dir.join("cpufreq/scaling_cur_freq")),
    }
}

fn read_model(root: &Path) -> io::Result<String> {
    let mut cpuinfo = ProcFile::open(&root.join("proc/cpuinfo"))?;
    parse_cpu_model(cpuinfo.read()?).ok_or_else(|| invalid_data("proc/cpuinfo has no model name"))
}

/// coretemp labels its inputs `Package id 0` and `Core N`, N being the topology's `core_id`.
fn find_sensors(root: &Path) -> io::Result<Sensors> {
    let device = find_device(root, DRIVER)?;
    let mut package = None;
    let mut cores = Vec::new();
    for entry in fs::read_dir(&device)? {
        let path = entry?.path();
        let Some(stem) = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix("_label"))
        else {
            continue;
        };
        let input = Reopening::new(device.join(format!("{stem}_input")));
        let mut label = ProcFile::open(&path)?;
        let label = label.read()?.trim_end();
        if label == "Package id 0" {
            package = Some(input);
        } else if let Some(core) = label.strip_prefix("Core ").and_then(parse_u64) {
            cores.push((core, input));
        }
    }
    let package = package.ok_or_else(|| invalid_data("coretemp has no package sensor"))?;
    Ok(Sensors { package, cores })
}
````

Apply to `crates/readers/src/fan.rs`:

````diff
diff --git a/crates/readers/src/fan.rs b/crates/readers/src/fan.rs
--- a/crates/readers/src/fan.rs
+++ b/crates/readers/src/fan.rs
@@ -1,16 +1,14 @@
 //! The `dell_smm` hwmon device's `fan1_*` attributes → a fan speed.

-use std::fs;
 use std::io;
-use std::path::{Path, PathBuf};
+use std::path::Path;

 use statsbar_core::fan::FanSpeed;
 use statsbar_core::sysfs::parse_u64;

+use crate::hwmon::find_device;
 use crate::procfile::{ProcFile, invalid_data};

-const HWMON: &str = "sys/class/hwmon";
-/// hwmon numbers change between boots, so the device is found by its `name`.
 const DRIVER: &str = "dell_smm";

 pub struct FanReader {
@@ -23,7 +21,7 @@ impl FanReader {
     /// Finds the device and reads its limits, which the driver sets when it loads. Only
     /// `fan1_input` is kept open, because each read of it is a BIOS (SMM) call: 0.7 ms here.
     pub fn open(root: &Path) -> io::Result<FanReader> {
-        let device = find_device(&root.join(HWMON))?;
+        let device = find_device(root, DRIVER)?;
         Ok(FanReader {
             min_rpm: read_rpm(&device.join("fan1_min"))?,
             max_rpm: read_rpm(&device.join("fan1_max"))?,
@@ -42,25 +40,6 @@ impl FanReader {
     }
 }

-fn find_device(hwmon: &Path) -> io::Result<PathBuf> {
-    for entry in fs::read_dir(hwmon)? {
-        let device = entry?.path();
-        let mut name = match ProcFile::open(&device.join("name")) {
-            Ok(file) => file,
-            // A device without a name cannot be the one looked for.
-            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
-            Err(error) => return Err(error),
-        };
-        if name.read()?.trim_end() == DRIVER {
-            return Ok(device);
-        }
-    }
-    Err(io::Error::new(
-        io::ErrorKind::NotFound,
-        format!("no {DRIVER} device under {}", hwmon.display()),
-    ))
-}
-
 fn read_rpm(path: &Path) -> io::Result<u64> {
     let mut file = ProcFile::open(path)?;
     parse_u64(file.read()?)
````

Create `crates/readers/src/hwmon.rs`:

````rust
//! Finding an hwmon device. Their numbers change between boots, so it goes by `name`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::procfile::ProcFile;

pub(crate) fn find_device(root: &Path, driver: &str) -> io::Result<PathBuf> {
    let hwmon = root.join("sys/class/hwmon");
    for entry in fs::read_dir(&hwmon)? {
        let device = entry?.path();
        let mut name = match ProcFile::open(&device.join("name")) {
            Ok(file) => file,
            // A device without a name cannot be the one looked for.
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        if name.read()?.trim_end() == driver {
            return Ok(device);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("no {driver} device under {}", hwmon.display()),
    ))
}
````

Apply to `crates/readers/src/lib.rs`:

````diff
diff --git a/crates/readers/src/lib.rs b/crates/readers/src/lib.rs
--- a/crates/readers/src/lib.rs
+++ b/crates/readers/src/lib.rs
@@ -3,10 +3,17 @@
 //! Each reader opens its file once and re-reads it from offset 0 with `pread`, so a sample costs
 //! a read instead of an open, a read and a close. Readers take the filesystem root as an argument
 //! (`/` in production), which lets the tests run them against fixture trees.
+//!
+//! `processes` is the exception: pids come and go, so it opens a file per process, and only
+//! while a page that lists them is open.

 pub mod battery;
 pub mod cpu;
+pub mod cpu_details;
 pub mod fan;
+mod hwmon;
 pub mod memory;
 pub mod network;
+pub mod pressure;
+pub mod processes;
 mod procfile;
````

Create `crates/readers/src/pressure.rs`:

````rust
//! `/proc/pressure/memory` → the pressure level.

use std::io;
use std::path::Path;

use statsbar_core::memory::Pressure;

use crate::procfile::Reopening;

pub struct PressureReader {
    psi: Reopening,
}

impl PressureReader {
    pub fn new(root: &Path) -> PressureReader {
        PressureReader {
            psi: Reopening::new(root.join("proc/pressure/memory")),
        }
    }

    pub fn read(&mut self) -> io::Result<Pressure> {
        self.psi.read(Pressure::from_psi)
    }
}
````

Create `crates/readers/src/processes.rs`:

````rust
//! `/proc/<pid>` → samples for ranking, and who the ranked processes are.

use std::fs::{self, File};
use std::io::{self, Read};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use statsbar_core::ipc::ProcessInfo;
use statsbar_core::process::{
    CpuSample, MemorySample, can_quit, display_name, parse_stat, parse_statm,
};

use crate::procfile::invalid_data;

/// `stat` and `statm` are one short line each; the kernel's name is at most 64 bytes of it.
const MAX_BYTES: u64 = 4096;

pub struct ProcessReader {
    proc: PathBuf,
    own_uid: u32,
    page_size: u64,
}

impl ProcessReader {
    pub fn new(root: &Path, own_uid: u32, page_size: u64) -> ProcessReader {
        ProcessReader {
            proc: root.join("proc"),
            own_uid,
            page_size,
        }
    }

    pub fn cpu_samples(&self) -> io::Result<Vec<CpuSample>> {
        self.scan("stat", |pid, text| {
            parse_stat(text).map(|stat| CpuSample {
                pid,
                start_ticks: stat.start_ticks,
                cpu_ticks: stat.cpu_ticks,
            })
        })
    }

    pub fn memory_samples(&self) -> io::Result<Vec<MemorySample>> {
        self.scan("statm", |pid, text| {
            parse_statm(text, self.page_size).map(|bytes| MemorySample { pid, bytes })
        })
    }

    /// Who `pid` is now, or `None` once it has exited. Asked only for the ranked few, since it
    /// costs three lookups.
    pub fn identify(&self, pid: u32) -> Option<ProcessInfo> {
        let dir = self.proc.join(pid.to_string());
        let stat = parse_stat(&read_lossy(&dir.join("stat")).ok()?)?;
        // The directory keeps the process's effective uid even where its files belong to root.
        let owner = fs::metadata(&dir).ok()?.uid();
        // Not readable for other users' processes, kernel threads and non-dumpable processes.
        let exe = fs::read_link(dir.join("exe")).ok().and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        });
        Some(ProcessInfo {
            pid,
            start_ticks: stat.start_ticks,
            name: display_name(exe.as_deref(), &stat.comm),
            can_quit: can_quit(pid, owner, self.own_uid, exe.as_deref(), &stat.comm),
        })
    }

    /// A process that exits during the scan, or cannot be read, is left out. Only a scan that
    /// finds no process at all is an error.
    fn scan<T>(&self, file: &str, parse: impl Fn(u32, &str) -> Option<T>) -> io::Result<Vec<T>> {
        let mut samples = Vec::new();
        for entry in fs::read_dir(&self.proc)? {
            let entry = entry?;
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|name| name.parse().ok())
            else {
                continue;
            };
            let Ok(text) = read_lossy(&entry.path().join(file)) else {
                continue;
            };
            samples.extend(parse(pid, &text));
        }
        if samples.is_empty() {
            return Err(invalid_data(&format!("no process has a readable {file}")));
        }
        Ok(samples)
    }
}

/// A process names itself with any bytes, so this cannot insist on UTF-8 as `ProcFile` does.
fn read_lossy(path: &Path) -> io::Result<String> {
    let mut bytes = Vec::new();
    File::open(path)?.take(MAX_BYTES).read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
````

Apply to `crates/readers/src/procfile.rs`:

````diff
diff --git a/crates/readers/src/procfile.rs b/crates/readers/src/procfile.rs
--- a/crates/readers/src/procfile.rs
+++ b/crates/readers/src/procfile.rs
@@ -1,7 +1,7 @@
 use std::fs::File;
 use std::io;
 use std::os::unix::fs::FileExt;
-use std::path::Path;
+use std::path::{Path, PathBuf};

 /// procfs reports a size of 0, so the length is only known by reading to the end. The cap stops a
 /// corrupt file from growing the buffer without bound. /proc/stat on this 8-thread laptop is
@@ -59,3 +59,26 @@ impl ProcFile {
 pub(crate) fn invalid_data(message: &str) -> io::Error {
     io::Error::new(io::ErrorKind::InvalidData, message.to_owned())
 }
+
+/// A [`ProcFile`] for a figure that may be absent: opened on first use, and again after a failure.
+pub(crate) struct Reopening {
+    path: PathBuf,
+    file: Option<ProcFile>,
+}
+
+impl Reopening {
+    pub(crate) fn new(path: PathBuf) -> Reopening {
+        Reopening { path, file: None }
+    }
+
+    pub(crate) fn read<T>(&mut self, parse: impl FnOnce(&str) -> Option<T>) -> io::Result<T> {
+        let mut file = match self.file.take() {
+            Some(file) => file,
+            None => ProcFile::open(&self.path)?,
+        };
+        let value = parse(file.read()?)
+            .ok_or_else(|| invalid_data(&format!("{} is malformed", self.path.display())))?;
+        self.file = Some(file);
+        Ok(value)
+    }
+}
````

- [x] **Step 5: Run, expect PASS**

```bash
cargo test -p statsbar-readers
```

Expected: `processes` 10, `cpu_details` 8 and `pressure` 4 pass, and so does `fan`, unchanged. The
real-root tests accept a machine without coretemp, cpufreq or PSI, as a CI runner may be.

- [x] **Step 6: Checkpoint**

Ready to commit: the files listed above — "feat(readers): read processes, CPU details and memory pressure"

### Task 5: Track open pages in the app state

**Test level:** unit. `App` does no I/O and takes the time as an argument.

**Files:**

- Modify: `crates/daemon/src/app.rs` (lines after 6, after 23, after 126, 137, after 140, after 147, 152–164, after 165, 170, after 174)
- Create: `crates/daemon/tests/app_pages.rs`

**Interfaces:**

- Consumes: `ipc::Page`, `schedule::{ReaderId, ReaderState}`.
- Produces:

```rust
// crates/daemon/src/app.rs
pub fn reader_of(page: Page) -> ReaderId;
impl App {
    pub fn is_read(&self, id: ReaderId) -> bool;
    pub fn set_page_open(&mut self, id: ReaderId, open: bool);
    pub fn refresh(&mut self, id: ReaderId, now_nanos: u64) -> bool;
}
```

- [x] **Step 1: Write the failing tests**

Create `crates/daemon/tests/app_pages.rs`:

````rust
//! What an open page changes: what is read, how often, and what hiding an item then means.

use statsbar_core::cpu::CoreTicks;
use statsbar_core::ipc::Page;
use statsbar_core::memory::MemInfo;
use statsbar_core::schedule::ReaderId::{self, Battery, Cpu, Fan, Memory, Network};
use statsbard::app::{App, READERS, reader_of};
use statsbard::config::{Config, Items};
use statsbard::summary;

const SECOND: u64 = 1_000_000_000;

fn only(shown: &[ReaderId]) -> App {
    let mut items = Items::default();
    for id in READERS {
        items.set_shown(id, shown.contains(&id));
    }
    App::new(Config { items }, false)
}

fn ticks(user: u64, idle: u64) -> Vec<CoreTicks> {
    vec![CoreTicks {
        user,
        nice: 0,
        system: 0,
        idle,
    }]
}

fn half_used() -> MemInfo {
    MemInfo {
        total: 1000,
        available: 500,
        ..MemInfo::default()
    }
}

#[test]
fn each_page_has_its_reader() {
    assert_eq!(reader_of(Page::Cpu), Cpu);
    assert_eq!(reader_of(Page::Memory), Memory);
}

#[test]
fn a_hidden_items_page_is_read_at_once_then_every_second() {
    let mut app = only(&[]);
    assert_eq!(
        app.next_wake_nanos(),
        None,
        "nothing is read before it opens"
    );
    app.set_page_open(Cpu, true);
    assert_eq!(app.due(5 * SECOND), [Cpu]);
    app.record_cpu(5 * SECOND, ticks(0, 0));
    assert_eq!(app.next_wake_nanos(), Some(6 * SECOND));
    assert!(app.is_read(Cpu) && !app.is_visible(Cpu));
    assert!(!app.config.items.cpu, "the config is untouched");
}

#[test]
fn an_open_page_is_read_every_second_under_power_saver() {
    let mut app = only(&[Cpu, Memory]);
    app.power_saver = true;
    app.record_cpu(0, ticks(0, 0));
    app.record_memory(0, &half_used());
    app.set_page_open(Cpu, true);
    assert_eq!(app.due(SECOND), [Cpu]);
    assert_eq!(
        app.due(5 * SECOND),
        [Cpu, Memory],
        "the other item stays at 5 s"
    );
}

#[test]
fn opening_a_shown_items_page_keeps_its_baseline() {
    let mut app = only(&[Cpu]);
    app.record_cpu(0, ticks(0, 0));
    app.set_page_open(Cpu, true);
    assert_eq!(app.record_cpu(SECOND, ticks(25, 75)), Some(25));
}

#[test]
fn opening_an_open_page_again_changes_nothing() {
    let mut app = only(&[]);
    app.set_page_open(Cpu, true);
    app.record_cpu(0, ticks(0, 0));
    app.set_page_open(Cpu, true);
    assert!(app.due(SECOND - 1).is_empty());
    assert_eq!(
        app.record_cpu(SECOND, ticks(25, 75)),
        Some(25),
        "the baseline is kept"
    );
}

#[test]
fn hiding_an_item_with_its_page_open_keeps_reading_and_keeps_its_figures() {
    let mut app = only(&[Cpu]);
    app.record_cpu(0, ticks(0, 0));
    app.record_cpu(2 * SECOND, ticks(25, 75));
    app.set_page_open(Cpu, true);
    assert!(!app.toggle(Cpu));
    assert!(!app.config.items.cpu);
    assert!(app.is_read(Cpu));
    assert!(app.due(3 * SECOND - 1).is_empty(), "not reread at once");
    assert_eq!(app.due(3 * SECOND), [Cpu]);
    assert_eq!(app.cpu().map(|load| load.usage_percent()), Some(25.0));
    assert!(
        summary::hub_rows(&app.overview()).is_empty(),
        "no hub row while hidden"
    );
}

#[test]
fn showing_an_item_with_its_page_open_draws_its_label_on_the_next_sample() {
    let mut app = only(&[]);
    app.set_page_open(Cpu, true);
    app.record_cpu(0, ticks(0, 0));
    assert_eq!(app.record_cpu(SECOND, ticks(25, 75)), Some(25));
    assert!(app.toggle(Cpu));
    assert_eq!(
        app.record_cpu(2 * SECOND, ticks(50, 150)),
        Some(25),
        "the same value, on an item that is new and blank"
    );
}

#[test]
fn closing_a_hidden_items_page_stops_and_forgets() {
    let mut app = only(&[]);
    app.set_page_open(Cpu, true);
    app.record_cpu(0, ticks(0, 0));
    app.record_cpu(SECOND, ticks(25, 75));
    app.set_page_open(Cpu, false);
    assert!(!app.is_read(Cpu));
    assert_eq!(app.next_wake_nanos(), None);
    assert_eq!(app.cpu(), None);
    app.set_page_open(Cpu, true);
    assert_eq!(app.due(9 * SECOND), [Cpu]);
    assert_eq!(
        app.record_cpu(9 * SECOND, ticks(90, 910)),
        None,
        "a fresh baseline"
    );
}

#[test]
fn a_hidden_items_page_opened_again_waits_for_its_interval() {
    let mut app = only(&[]);
    app.set_page_open(Cpu, true);
    app.record_cpu(0, ticks(0, 0));
    app.set_page_open(Cpu, false);
    app.set_page_open(Cpu, true);
    assert!(
        app.due(SECOND - 1).is_empty(),
        "opening the page again is no reason to read"
    );
    assert_eq!(app.next_wake_nanos(), Some(SECOND));
    assert_eq!(app.due(SECOND), [Cpu]);
}

#[test]
fn closing_a_shown_items_page_returns_to_two_seconds() {
    let mut app = only(&[Memory]);
    app.set_page_open(Memory, true);
    app.record_memory(0, &half_used());
    app.set_page_open(Memory, false);
    assert!(app.is_read(Memory));
    assert!(app.memory().is_some());
    assert_eq!(app.next_wake_nanos(), Some(2 * SECOND));
}

#[test]
fn hiding_then_closing_stops_the_reads() {
    let mut app = only(&[Memory]);
    app.set_page_open(Memory, true);
    app.toggle(Memory);
    app.set_page_open(Memory, false);
    assert!(!app.is_read(Memory));
    assert_eq!(app.next_wake_nanos(), None);
}

#[test]
fn a_failure_under_an_open_page_is_retried_each_second_and_logged_afresh_after_closing() {
    let mut app = only(&[]);
    app.set_page_open(Memory, true);
    assert!(app.record_error(Memory, 0));
    assert!(!app.record_error(Memory, SECOND));
    assert_eq!(app.next_wake_nanos(), Some(2 * SECOND));
    app.set_page_open(Memory, false);
    app.set_page_open(Memory, true);
    assert!(app.record_error(Memory, 3 * SECOND));
}

#[test]
fn refresh_reads_now_once_half_a_second_has_passed() {
    let mut app = only(&[]);
    app.set_page_open(Cpu, true);
    app.record_cpu(0, ticks(0, 0));
    assert!(!app.refresh(Cpu, SECOND / 2 - 1));
    assert!(
        app.due(SECOND / 2 - 1).is_empty(),
        "too short an interval for a load"
    );
    assert!(app.refresh(Cpu, SECOND / 2));
    assert_eq!(app.due(SECOND / 2), [Cpu]);
}

#[test]
fn refresh_without_an_open_page_does_nothing() {
    let mut app = only(&[Cpu]);
    app.record_cpu(0, ticks(0, 0));
    assert!(!app.refresh(Cpu, SECOND));
    assert!(app.due(SECOND).is_empty());
}

#[test]
fn items_without_a_page_follow_their_visibility_alone() {
    let app = only(&[Battery]);
    assert!(app.is_read(Battery));
    assert!(!app.is_read(Fan) && !app.is_read(Network));
}
````

- [x] **Step 2: Run them, expect FAIL**

```bash
cargo test -p statsbard --test app_pages
```

Expected: the test does not compile (E0432 for `reader_of`; E0599 for `set_page_open`, `is_read`
and `refresh`).

- [x] **Step 3: Implement**

Apply to `crates/daemon/src/app.rs`:

````diff
diff --git a/crates/daemon/src/app.rs b/crates/daemon/src/app.rs
--- a/crates/daemon/src/app.rs
+++ b/crates/daemon/src/app.rs
@@ -4,6 +4,7 @@
 use statsbar_core::battery::{BatteryFigures, BatteryInfo, ChargeStatus};
 use statsbar_core::cpu::{CoreTicks, CpuLoad};
 use statsbar_core::fan::{self, FanSpeed};
+use statsbar_core::ipc::Page;
 use statsbar_core::label;
 use statsbar_core::memory::{MemInfo, MemoryBuckets};
 use statsbar_core::schedule::{self, ReaderId, ReaderState};
@@ -21,6 +22,18 @@ pub const READERS: [ReaderId; 5] = [
     ReaderId::Network,
 ];

+/// A refresh sooner than this after the last read is ignored: the next read is under a second
+/// away anyway, and a load over a few milliseconds of ticks is noise.
+const MIN_REFRESH_NANOS: u64 = 500_000_000;
+
+/// The reader behind a page.
+pub fn reader_of(page: Page) -> ReaderId {
+    match page {
+        Page::Cpu => ReaderId::Cpu,
+        Page::Memory => ReaderId::Memory,
+    }
+}
+
 /// A click in the hub's menu, sent from the `ksni` thread to the main loop.
 #[derive(Debug, Clone, Copy, PartialEq, Eq)]
 pub enum Command {
@@ -124,6 +137,45 @@ impl App {
         self.readers[slot(id)].visible
     }

+    /// Whether anything shows the metric: its item or its page.
+    pub fn is_read(&self, id: ReaderId) -> bool {
+        let state = &self.readers[slot(id)];
+        state.visible || state.page_open
+    }
+
+    /// Records whether someone watches the page. A hidden item's page starts from a fresh sample
+    /// when it opens and drops it when it closes; a shown item's only changes the interval.
+    /// The time of the last read stays, so opening and closing a page over and over reads no
+    /// more often than the page's interval.
+    pub fn set_page_open(&mut self, id: ReaderId, open: bool) {
+        let index = slot(id);
+        if self.readers[index].page_open == open {
+            return;
+        }
+        if !self.readers[index].visible {
+            let last_read_nanos = self.readers[index].last_read_nanos;
+            self.forget(id);
+            self.readers[index] = fresh(id, false);
+            self.readers[index].last_read_nanos = last_read_nanos;
+            self.failing[index] = false;
+        }
+        self.readers[index].page_open = open;
+    }
+
+    /// The page's Refresh: due now, unless it was read a moment ago. Returns whether it was
+    /// taken.
+    pub fn refresh(&mut self, id: ReaderId, now_nanos: u64) -> bool {
+        let state = &mut self.readers[slot(id)];
+        let rested = state
+            .last_read_nanos
+            .is_some_and(|last| now_nanos.saturating_sub(last) >= MIN_REFRESH_NANOS);
+        let taken = state.page_open && rested;
+        if taken {
+            state.last_read_nanos = None;
+        }
+        taken
+    }
+
     pub fn due(&self, now_nanos: u64) -> Vec<ReaderId> {
         schedule::due(now_nanos, self.power_saver, &self.readers)
     }
@@ -134,10 +186,16 @@ impl App {

     /// Flips an item's visibility in the config and the schedule, and returns the new value.
     /// Hiding drops the item's samples, so an item shown again starts from a fresh reading and is
-    /// due at once, instead of showing a stale value.
+    /// due at once, instead of showing a stale value. While its page is open the samples stay
+    /// current and are kept; only the drawn label is forgotten, because a shown item starts blank.
     pub fn toggle(&mut self, id: ReaderId) -> bool {
         let visible = !self.is_visible(id);
         self.config.items.set_shown(id, visible);
+        if self.readers[slot(id)].page_open {
+            self.forget_label(id);
+            self.readers[slot(id)].visible = visible;
+            return visible;
+        }
         self.forget(id);
         self.readers[slot(id)] = fresh(id, visible);
         self.failing[slot(id)] = false;
@@ -145,33 +203,33 @@ impl App {
     }

     fn forget(&mut self, id: ReaderId) {
+        self.forget_label(id);
         match id {
             ReaderId::Cpu => {
                 self.cpu_previous = None;
                 self.cpu = None;
-                self.cpu_label = None;
-            }
-            ReaderId::Memory => {
-                self.memory = None;
-                self.memory_label = None;
-            }
-            ReaderId::Battery => {
-                self.battery = None;
-                self.battery_label = None;
-            }
-            ReaderId::Fan => {
-                self.fan = None;
-                self.fan_label = None;
             }
+            ReaderId::Memory => self.memory = None,
+            ReaderId::Battery => self.battery = None,
+            ReaderId::Fan => self.fan = None,
             ReaderId::Network => {
                 // A new session: the totals count from when the item is shown again.
                 self.throughput = ThroughputTracker::default();
                 self.network = None;
-                self.network_label = None;
             }
         }
     }

+    fn forget_label(&mut self, id: ReaderId) {
+        match id {
+            ReaderId::Cpu => self.cpu_label = None,
+            ReaderId::Memory => self.memory_label = None,
+            ReaderId::Battery => self.battery_label = None,
+            ReaderId::Fan => self.fan_label = None,
+            ReaderId::Network => self.network_label = None,
+        }
+    }
+
     /// Takes a `/proc/stat` sample. Returns the new label value when it changed. The first sample,
     /// and one taken after a core went offline, only set the baseline.
     pub fn record_cpu(&mut self, now_nanos: u64, ticks: Vec<CoreTicks>) -> Option<u8> {
````

- [x] **Step 4: Run, expect PASS**

```bash
cargo test -p statsbard --test app_pages --test app
```

Expected: `app_pages` 15 and `app` 22 pass.

- [x] **Step 5: Checkpoint**

Ready to commit: `crates/daemon/src/app.rs`, `crates/daemon/tests/app_pages.rs` — "feat(daemon): track which pages are open"

### Task 6: Build the page snapshots

**Test level:** integration against the file system, through fixture trees and a passed-in clock.

**Files:**

- Modify: `crates/daemon/src/lib.rs` (lines after 8)
- Create: `crates/daemon/src/pages.rs`
- Create: `crates/daemon/tests/pages.rs`

**Interfaces:**

- Consumes: Task 2's rankings, Task 3's snapshot types, Task 4's readers.
- Produces:

```rust
// crates/daemon/src/lib.rs
pub mod pages;

// crates/daemon/src/pages.rs
pub use statsbar_readers::cpu_details::Problem;
pub struct System {
    pub own_uid: u32,
    pub page_size: u64,
    pub ticks_per_second: u64,
}
pub struct Pages { /* private */ }
impl Pages {
    pub fn new(root: &Path, system: System) -> Pages;
    pub fn cpu(&mut self, now_nanos: u64, load: Option<&CpuLoad>, power_saver: bool) -> (CpuPage, Vec<Problem>);
    pub fn memory(&mut self, now_nanos: u64, buckets: MemoryBuckets) -> (MemoryPage, Vec<Problem>);
    pub fn refresh(&mut self, page: Page);
    pub fn close(&mut self, page: Page);
}
```

- [x] **Step 1: Write the failing tests**

Create `crates/daemon/tests/pages.rs`:

````rust
//! The page snapshots, against a `/proc` and `/sys` written by each test.

use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use statsbar_core::cpu::CpuLoad;
use statsbar_core::ipc::Page;
use statsbar_core::memory::{MemoryBuckets, Pressure};
use statsbar_core::system::Thermal;
use statsbard::pages::{Pages, System};

const SECOND: u64 = 1_000_000_000;

struct Machine {
    root: PathBuf,
}

impl Machine {
    /// Two CPUs, coretemp at 91 °C and eight processes, all readable.
    fn new(name: &str) -> Machine {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("pages-{name}"));
        match fs::remove_dir_all(&root) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => panic!("clearing {}: {error}", root.display()),
        }
        let machine = Machine { root };
        machine.write("sys/devices/system/cpu/online", "0-1\n");
        for cpu in 0..2 {
            let dir = format!("sys/devices/system/cpu/cpu{cpu}");
            machine.write(&format!("{dir}/topology/core_id"), "0\n");
            machine.write(&format!("{dir}/cpufreq/scaling_cur_freq"), "1900000\n");
        }
        let temps = "sys/class/hwmon/hwmon0";
        machine.write(&format!("{temps}/name"), "coretemp\n");
        machine.write(&format!("{temps}/temp1_label"), "Package id 0\n");
        machine.write(&format!("{temps}/temp1_input"), "91000\n");
        machine.write(&format!("{temps}/temp2_label"), "Core 0\n");
        machine.write(&format!("{temps}/temp2_input"), "88000\n");
        machine.write("proc/loadavg", "2.00 1.00 0.50 1/100 9\n");
        machine.write("proc/uptime", "3600.00 7000.00\n");
        machine.write("proc/cpuinfo", "model name\t: i5\n");
        machine.psi("12.00");
        for pid in 11..19 {
            machine.process(pid, 1, 100, pid);
        }
        machine
    }

    fn write(&self, path: &str, contents: &str) {
        let path = self.root.join(path);
        fs::create_dir_all(path.parent().expect("has a parent")).expect("create the directory");
        fs::write(path, contents).expect("write");
    }

    fn psi(&self, some: &str) {
        self.write(
            "proc/pressure/memory",
            &format!("some avg10={some} avg60=0 avg300=0 total=1\nfull avg10=0.00 avg60=0 avg300=0 total=1\n"),
        );
    }

    fn process(&self, pid: u32, start: u64, cpu_ticks: u64, private_pages: u32) {
        self.write(
            &format!("proc/{pid}/stat"),
            &format!(
                "{pid} (p{pid}) S 1 1 1 0 -1 0 0 0 0 0 {cpu_ticks} 0 0 0 20 0 1 0 {start} 0 0\n"
            ),
        );
        let resident = private_pages + 5;
        self.write(
            &format!("proc/{pid}/statm"),
            &format!("900 {resident} 5 0 0 0 0\n"),
        );
    }

    fn pages(&self) -> Pages {
        let own_uid = fs::metadata(&self.root).expect("the root exists").uid();
        let system = System {
            own_uid,
            page_size: 4096,
            ticks_per_second: 100,
        };
        Pages::new(&self.root, system)
    }
}

fn load(cores: usize) -> CpuLoad {
    CpuLoad {
        system_percent: 10.0,
        user_percent: 20.0,
        idle_percent: 70.0,
        per_core_busy: vec![30.0; cores],
    }
}

fn buckets() -> MemoryBuckets {
    MemoryBuckets {
        total: 1000,
        app: 500,
        free: 500,
        ..MemoryBuckets::default()
    }
}

#[test]
fn the_first_cpu_page_has_the_details_and_no_ranking_yet() {
    let machine = Machine::new("cpu-first");
    let (page, problems) = machine.pages().cpu(0, None, true);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(page.model.as_deref(), Some("i5"));
    assert_eq!((page.load, page.processes), (None, None));
    assert_eq!(
        (page.temperature_c, page.thermal),
        (Some(91.0), Some(Thermal::Serious))
    );
    assert_eq!(page.core_temperatures_c, [Some(88.0), Some(88.0)]);
    assert_eq!(page.frequency_mhz, Some(1900.0));
    assert_eq!(page.load_per_cpu, Some([1.0, 0.5, 0.25]));
    assert_eq!(page.uptime_seconds, Some(3600.0));
    assert!(page.power_saver);
}

#[test]
fn the_second_scan_ranks_the_six_busiest() {
    let machine = Machine::new("cpu-rank");
    let mut pages = machine.pages();
    pages.cpu(0, None, false);
    for pid in 11..19 {
        machine.process(pid, 1, 100 + u64::from(pid), pid);
    }
    let (page, _) = pages.cpu(SECOND, Some(&load(2)), false);
    assert_eq!(page.load, Some(load(2)));
    let rows: Vec<_> = page
        .processes
        .expect("ranked")
        .into_iter()
        .map(|row| (row.process.name, row.cpu_percent, row.process.start_ticks))
        .collect();
    let expected: Vec<_> = (13..19)
        .rev()
        .map(|pid| (format!("p{pid}"), f64::from(pid), 1))
        .collect();
    assert_eq!(rows, expected);
}

#[test]
fn a_scan_too_soon_keeps_the_last_ranking_and_baseline() {
    let machine = Machine::new("cpu-soon");
    let mut pages = machine.pages();
    pages.cpu(0, None, false);
    machine.process(18, 1, 150, 18);
    let (soon, _) = pages.cpu(SECOND / 2 - 1, None, false);
    assert_eq!(soon.processes, None, "no figure from a tiny interval");
    let (page, _) = pages.cpu(SECOND, None, false);
    assert_eq!(page.processes.expect("ranked")[0].cpu_percent, 50.0);
}

#[test]
fn temperatures_are_dropped_when_the_load_is_of_other_cpus() {
    let machine = Machine::new("cpu-hotplug");
    let (page, _) = machine.pages().cpu(0, Some(&load(3)), false);
    assert_eq!(page.core_temperatures_c, [None; 3]);
}

#[test]
fn closing_the_cpu_page_forgets_its_baseline() {
    let machine = Machine::new("cpu-close");
    let mut pages = machine.pages();
    pages.cpu(0, None, false);
    pages.cpu(SECOND, None, false);
    pages.close(Page::Cpu);
    let (page, _) = pages.cpu(60 * SECOND, None, false);
    assert_eq!(page.processes, None);
}

#[test]
fn the_memory_page_has_pressure_and_the_six_heaviest_at_once() {
    let machine = Machine::new("memory-first");
    let (page, problems) = machine.pages().memory(0, buckets());
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(
        (page.buckets, page.pressure),
        (buckets(), Some(Pressure::Warning))
    );
    let rows: Vec<_> = page
        .processes
        .expect("ranked")
        .into_iter()
        .map(|row| (row.process.pid, row.bytes))
        .collect();
    let expected: Vec<_> = (13..19)
        .rev()
        .map(|pid| (pid, u64::from(pid) * 4096))
        .collect();
    assert_eq!(rows, expected);
}

#[test]
fn memory_processes_are_rescanned_every_two_seconds_or_on_refresh() {
    let machine = Machine::new("memory-cadence");
    let mut pages = machine.pages();
    let top = |pages: &mut Pages, at: u64| {
        pages.memory(at, buckets()).0.processes.expect("ranked")[0]
            .process
            .pid
    };
    assert_eq!(top(&mut pages, 0), 18);
    machine.process(11, 1, 100, 500);
    assert_eq!(top(&mut pages, 2 * SECOND - 1), 18);
    assert_eq!(top(&mut pages, 2 * SECOND), 11);
    machine.process(12, 1, 100, 900);
    pages.refresh(Page::Memory);
    assert_eq!(top(&mut pages, 2 * SECOND + 1), 12);
}

#[test]
fn closing_the_memory_page_forgets_its_ranking() {
    let machine = Machine::new("memory-close");
    let mut pages = machine.pages();
    pages.memory(0, buckets());
    machine.process(11, 1, 100, 500);
    pages.close(Page::Memory);
    let (page, _) = pages.memory(1, buckets());
    assert_eq!(page.processes.expect("ranked")[0].process.pid, 11);
}

#[test]
fn missing_psi_is_none_and_reported_once_until_it_recovers() {
    let machine = Machine::new("memory-psi");
    let psi = machine.root.join("proc/pressure/memory");
    fs::remove_file(&psi).expect("remove");
    let mut pages = machine.pages();
    let named = |(page, problems): (
        statsbar_core::ipc::MemoryPage,
        Vec<statsbard::pages::Problem>,
    )| {
        let names: Vec<_> = problems.into_iter().map(|(what, _)| what).collect();
        (page.pressure, names)
    };
    assert_eq!(
        named(pages.memory(0, buckets())),
        (None, vec!["memory pressure"])
    );
    assert_eq!(named(pages.memory(SECOND, buckets())), (None, vec![]));
    machine.psi("0.00");
    assert_eq!(
        named(pages.memory(2 * SECOND, buckets())),
        (Some(Pressure::Normal), vec![])
    );
    machine.psi("broken");
    assert_eq!(
        named(pages.memory(3 * SECOND, buckets())),
        (None, vec!["memory pressure"])
    );
}

#[test]
fn a_failed_scan_keeps_the_last_ranking_and_reports_once() {
    let machine = Machine::new("memory-scan");
    let mut pages = machine.pages();
    let (first, _) = pages.memory(0, buckets());
    for pid in 11..19 {
        fs::remove_dir_all(machine.root.join(format!("proc/{pid}"))).expect("remove");
    }
    let (page, problems) = pages.memory(2 * SECOND, buckets());
    assert_eq!(page.processes, first.processes);
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].0, "processes");
    assert!(pages.memory(4 * SECOND, buckets()).1.is_empty());
}

#[test]
fn a_failed_cpu_scan_keeps_the_last_ranking_and_reports_once() {
    let machine = Machine::new("cpu-scan");
    let mut pages = machine.pages();
    pages.cpu(0, None, false);
    machine.process(18, 1, 150, 18);
    let (ranked, _) = pages.cpu(SECOND, None, false);
    assert!(ranked.processes.is_some());
    for pid in 11..19 {
        fs::remove_dir_all(machine.root.join(format!("proc/{pid}"))).expect("remove");
    }
    let (page, problems) = pages.cpu(2 * SECOND, None, false);
    assert_eq!(page.processes, ranked.processes);
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].0, "processes");
    assert!(pages.cpu(3 * SECOND, None, false).1.is_empty());
}

#[test]
fn a_pid_that_changed_hands_between_scans_starts_afresh_in_the_ranking() {
    let machine = Machine::new("cpu-pid-reuse");
    let mut pages = machine.pages();
    pages.cpu(0, None, false);
    // Pid 18 is a new process now: later start, and far more CPU time than the old one had.
    machine.process(18, 2, 5000, 18);
    machine.process(17, 1, 150, 17);
    let (page, _) = pages.cpu(SECOND, None, false);
    let rows: Vec<_> = page
        .processes
        .expect("ranked")
        .into_iter()
        .map(|row| (row.process.pid, row.cpu_percent))
        .collect();
    assert_eq!(rows[0], (17, 50.0));
    assert!(rows.iter().all(|(pid, _)| *pid != 18), "{rows:?}");
}

#[test]
fn a_scan_too_soon_after_a_ranking_keeps_that_ranking() {
    let machine = Machine::new("cpu-soon-after-ranking");
    let mut pages = machine.pages();
    pages.cpu(0, None, false);
    machine.process(18, 1, 150, 18);
    let (ranked, _) = pages.cpu(SECOND, None, false);
    assert!(ranked.processes.is_some());
    machine.process(17, 1, 400, 17);
    let (soon, _) = pages.cpu(SECOND + SECOND / 2 - 1, None, false);
    assert_eq!(soon.processes, ranked.processes);
}
````

- [x] **Step 2: Run them, expect FAIL**

```bash
cargo test -p statsbard --test pages
```

Expected: the test does not compile (E0432, `statsbard::pages`).

- [x] **Step 3: Implement**

Apply to `crates/daemon/src/lib.rs`:

````diff
diff --git a/crates/daemon/src/lib.rs b/crates/daemon/src/lib.rs
--- a/crates/daemon/src/lib.rs
+++ b/crates/daemon/src/lib.rs
@@ -6,6 +6,7 @@ pub mod app;
 pub mod autostart;
 pub mod config;
 pub mod fsutil;
+pub mod pages;
 pub mod paths;
 pub mod power;
 pub mod run;
````

Create `crates/daemon/src/pages.rs`:

````rust
//! What a page shows beyond its tray item's figures. Nothing here runs unless someone watches
//! that page, and closing it drops what it held.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use statsbar_core::cpu::CpuLoad;
use statsbar_core::ipc::{CpuPage, CpuProcess, MemoryPage, MemoryProcess, Page, ProcessInfo};
use statsbar_core::memory::MemoryBuckets;
use statsbar_core::process::{self, CpuSample, CpuUse};
use statsbar_core::system::Thermal;
use statsbar_readers::cpu_details::CpuDetailsReader;
use statsbar_readers::pressure::PressureReader;
use statsbar_readers::processes::ProcessReader;

pub use statsbar_readers::cpu_details::Problem;

/// Rows in each table, as upstream.
const TOP_COUNT: usize = 6;
/// CPU time counts in 10 ms ticks, so a shorter interval mostly measures rounding.
const MIN_CPU_SCAN_NANOS: u64 = 500_000_000;
/// As upstream's memory table.
const MEMORY_SCAN_NANOS: u64 = 2_000_000_000;

/// The numbers only the kernel knows.
#[derive(Debug, Clone, Copy)]
pub struct System {
    pub own_uid: u32,
    pub page_size: u64,
    pub ticks_per_second: u64,
}

struct Cpu {
    details: CpuDetailsReader,
    previous: Option<(u64, Vec<CpuSample>)>,
    processes: Option<Vec<CpuProcess>>,
    failing: HashSet<&'static str>,
}

struct Memory {
    pressure: PressureReader,
    scanned_at: Option<u64>,
    processes: Option<Vec<MemoryProcess>>,
    failing: HashSet<&'static str>,
}

pub struct Pages {
    root: PathBuf,
    ticks_per_second: u64,
    reader: ProcessReader,
    cpu: Option<Cpu>,
    memory: Option<Memory>,
}

impl Pages {
    pub fn new(root: &Path, system: System) -> Pages {
        Pages {
            root: root.to_owned(),
            ticks_per_second: system.ticks_per_second,
            reader: ProcessReader::new(root, system.own_uid, system.page_size),
            cpu: None,
            memory: None,
        }
    }

    /// The CPU page, and the problems that are new since the last call.
    pub fn cpu(
        &mut self,
        now_nanos: u64,
        load: Option<&CpuLoad>,
        power_saver: bool,
    ) -> (CpuPage, Vec<Problem>) {
        let cpu = self.cpu.get_or_insert_with(|| Cpu {
            details: CpuDetailsReader::new(&self.root),
            previous: None,
            processes: None,
            failing: HashSet::new(),
        });
        let (details, mut problems) = cpu.details.read();

        let rested = cpu
            .previous
            .as_ref()
            .is_none_or(|(at, _)| now_nanos.saturating_sub(*at) >= MIN_CPU_SCAN_NANOS);
        if rested {
            match self.reader.cpu_samples() {
                Ok(samples) => {
                    if let Some((at, previous)) = &cpu.previous {
                        let ranked = process::rank_by_cpu(
                            previous,
                            &samples,
                            now_nanos.saturating_sub(*at),
                            self.ticks_per_second,
                            TOP_COUNT,
                        );
                        cpu.processes = Some(label(&ranked, |pid| self.reader.identify(pid)));
                    }
                    cpu.previous = Some((now_nanos, samples));
                }
                Err(error) => problems.push(("processes", error)),
            }
        }

        let mut core_temperatures_c = details.core_celsius;
        if let Some(load) = load {
            // Different CPUs than the load's, for one sample after a hotplug: pair none of them.
            if load.per_core_busy.len() != core_temperatures_c.len() {
                core_temperatures_c = vec![None; load.per_core_busy.len()];
            }
        }
        let page = CpuPage {
            model: details.model,
            load: load.cloned(),
            temperature_c: details.package_celsius,
            thermal: details.package_celsius.map(Thermal::from_celsius),
            core_temperatures_c,
            frequency_mhz: details.frequency_mhz,
            load_per_cpu: details.load_per_cpu,
            uptime_seconds: details.uptime_seconds,
            power_saver,
            processes: cpu.processes.clone(),
        };
        (page, newly_failing(&mut cpu.failing, problems))
    }

    /// The Memory page, and the problems that are new since the last call.
    pub fn memory(&mut self, now_nanos: u64, buckets: MemoryBuckets) -> (MemoryPage, Vec<Problem>) {
        let memory = self.memory.get_or_insert_with(|| Memory {
            pressure: PressureReader::new(&self.root),
            scanned_at: None,
            processes: None,
            failing: HashSet::new(),
        });
        let mut problems = Vec::new();
        let pressure = memory
            .pressure
            .read()
            .map_err(|error| problems.push(("memory pressure", error)))
            .ok();

        let due = memory
            .scanned_at
            .is_none_or(|at| now_nanos.saturating_sub(at) >= MEMORY_SCAN_NANOS);
        if due {
            match self.reader.memory_samples() {
                Ok(samples) => {
                    memory.scanned_at = Some(now_nanos);
                    memory.processes = Some(
                        process::rank_by_memory(&samples, TOP_COUNT)
                            .iter()
                            .filter_map(|row| {
                                Some(MemoryProcess {
                                    process: self.reader.identify(row.pid)?,
                                    bytes: row.bytes,
                                })
                            })
                            .collect(),
                    );
                }
                Err(error) => problems.push(("processes", error)),
            }
        }

        let page = MemoryPage {
            buckets,
            pressure,
            processes: memory.processes.clone(),
        };
        (page, newly_failing(&mut memory.failing, problems))
    }

    /// The page's Refresh: its next reading scans the processes too.
    pub fn refresh(&mut self, page: Page) {
        // The CPU table already follows every reading.
        if page != Page::Memory {
            return;
        }
        if let Some(memory) = &mut self.memory {
            memory.scanned_at = None;
        }
    }

    /// Nobody watches the page: its files close and its samples go.
    pub fn close(&mut self, page: Page) {
        match page {
            Page::Cpu => self.cpu = None,
            Page::Memory => self.memory = None,
        }
    }
}

/// Names the ranked processes, dropping those that exited or whose pid changed hands since the
/// scan.
fn label(
    ranked: &[CpuUse],
    mut identify: impl FnMut(u32) -> Option<ProcessInfo>,
) -> Vec<CpuProcess> {
    ranked
        .iter()
        .filter_map(|row| {
            let process = identify(row.pid)?;
            (process.start_ticks == row.start_ticks).then_some(CpuProcess {
                process,
                cpu_percent: row.percent,
            })
        })
        .collect()
}

/// The problems that were not failing last time, which are the ones worth logging.
fn newly_failing(failing: &mut HashSet<&'static str>, problems: Vec<Problem>) -> Vec<Problem> {
    let now: HashSet<&'static str> = problems.iter().map(|(what, _)| *what).collect();
    let new = problems
        .into_iter()
        .filter(|(what, _)| !failing.contains(what))
        .collect();
    *failing = now;
    new
}

#[cfg(test)]
mod tests {
    use super::*;

    fn use_of(pid: u32, start_ticks: u64, percent: f64) -> CpuUse {
        CpuUse {
            pid,
            start_ticks,
            percent,
        }
    }

    fn info(pid: u32, start_ticks: u64) -> ProcessInfo {
        ProcessInfo {
            pid,
            start_ticks,
            name: format!("p{pid}"),
            can_quit: true,
        }
    }

    #[test]
    fn a_pid_that_changed_hands_since_the_scan_is_dropped() {
        let ranked = [
            use_of(1, 10, 90.0),
            use_of(2, 20, 80.0),
            use_of(3, 30, 70.0),
        ];
        // Pid 2 now belongs to a process that started later.
        let rows = label(&ranked, |pid| {
            Some(info(pid, if pid == 2 { 21 } else { pid as u64 * 10 }))
        });
        assert_eq!(
            rows,
            vec![
                CpuProcess {
                    process: info(1, 10),
                    cpu_percent: 90.0
                },
                CpuProcess {
                    process: info(3, 30),
                    cpu_percent: 70.0
                },
            ]
        );
    }

    #[test]
    fn a_process_that_exited_since_the_scan_is_dropped() {
        let ranked = [
            use_of(1, 10, 90.0),
            use_of(2, 20, 80.0),
            use_of(3, 30, 70.0),
        ];
        let rows = label(&ranked, |pid| {
            (pid != 2).then(|| info(pid, pid as u64 * 10))
        });
        assert_eq!(
            rows,
            vec![
                CpuProcess {
                    process: info(1, 10),
                    cpu_percent: 90.0
                },
                CpuProcess {
                    process: info(3, 30),
                    cpu_percent: 70.0
                },
            ]
        );
    }
}
````

- [x] **Step 4: Run, expect PASS**

```bash
cargo test -p statsbard --test pages
```

Expected: 13 pass.

- [x] **Step 5: Checkpoint**

Ready to commit: the files listed above — "feat(daemon): build CPU and memory page snapshots"

### Task 7: Serve the private socket

**Test level:** integration against real Unix sockets, `flock` and peer credentials. The peer-uid
check is an authorization check: flag it in the PR and review it in Task 9.

**Files:**

- Modify: `Cargo.lock` (lines after 657)
- Modify: `crates/core/src/ipc.rs` (lines 52)
- Modify: `crates/daemon/Cargo.toml` (lines after 15, after 16, 20–22)
- Create: `crates/daemon/src/ipc.rs`
- Modify: `crates/daemon/src/lib.rs` (lines after 8)
- Modify: `crates/daemon/src/paths.rs` (lines after 24)
- Create: `crates/daemon/tests/ipc.rs`
- Create: `crates/daemon/tests/ipc_accept_failure.rs`
- Modify: `crates/daemon/tests/paths.rs` (lines 4, after 47)

**Interfaces:**

- Consumes: `ipc::{Hello, Request, Event, ErrorReason, Page, PROTOCOL_VERSION}`.
- Produces:

```rust
// crates/daemon/src/ipc.rs
pub const MAX_CLIENTS: usize = 4;
pub const MAX_REQUEST_BYTES: usize = 4096;
pub const LINES_PER_TURN: usize = 16;
pub const ACCEPTS_PER_TURN: usize = 8;
pub struct Options {
    pub allowed_uid: u32,
    pub handshake_timeout: Duration,
}
pub enum Change {
    Subscribed(Page),
    Refresh(Page),
    Unwatched,
}
pub enum BindError {
    AlreadyRunning,
    Io(io::Error),
}
pub struct Server { /* private */ }
impl Server {
    pub fn bind(runtime_dir: &Path, options: Options) -> Result<Server, BindError>;
    pub fn watched(&self, page: Page) -> bool;
    pub fn publish(&mut self, page: Page, event: &Event);
    pub fn welcome(&mut self, page: Page, event: &Event);
    pub async fn next(&mut self) -> Change;
}

// crates/daemon/src/lib.rs
pub mod ipc;

// crates/daemon/src/paths.rs
pub fn runtime_dir(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf>;
```

- [x] **Step 1: Promote the dependencies and write the failing tests**

Apply to `Cargo.lock`:

````diff
diff --git a/Cargo.lock b/Cargo.lock
--- a/Cargo.lock
+++ b/Cargo.lock
@@ -655,6 +655,7 @@ dependencies = [
  "async-io",
  "futures-lite",
  "ksni",
+ "rustix",
  "serde",
  "serde_json",
  "statsbar-core",
````

Apply to `crates/daemon/Cargo.toml`:

````diff
diff --git a/crates/daemon/Cargo.toml b/crates/daemon/Cargo.toml
--- a/crates/daemon/Cargo.toml
+++ b/crates/daemon/Cargo.toml
@@ -13,12 +13,11 @@ async-channel = "=2.5.0"
 async-io = "=2.6.0"
 futures-lite = "=2.6.1"
 ksni = { version = "=0.3.7", default-features = false, features = ["async-io"] }
+rustix = { version = "=1.1.6", features = ["fs", "net", "param", "process"] }
 serde = { version = "=1.0.229", features = ["derive"] }
+serde_json = "=1.0.154"
 toml = "=1.2.1"
 zbus = { version = "=5.19.0", default-features = false, features = ["async-io"] }

-[dev-dependencies]
-serde_json = "=1.0.154"
-
 [lints]
 workspace = true
````

Create `crates/daemon/tests/ipc.rs`:

````rust
//! The socket server, over real Unix sockets.

use std::fs;
use std::future::Future;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use async_io::Async;
use futures_lite::future;
use futures_lite::io::{AsyncReadExt, AsyncWriteExt};
use statsbar_core::ipc::{ErrorReason, Event, Hello, MemoryPage, Page};
use statsbar_core::memory::MemoryBuckets;
use statsbard::ipc::{
    ACCEPTS_PER_TURN, BindError, Change, LINES_PER_TURN, MAX_CLIENTS, MAX_REQUEST_BYTES, Options,
    Server,
};

/// Generous for a local socket; it only has to tell "slow" from "hung".
const LIMIT: Duration = Duration::from_secs(5);
const HELLO: &str = "{\"protocol\":1}\n";
const WATCH_CPU: &str = "{\"type\":\"subscribe\",\"page\":\"cpu\"}\n";
const WATCH_MEMORY: &str = "{\"type\":\"subscribe\",\"page\":\"memory\"}\n";

/// A short private directory: a socket path is limited to 108 bytes, which a path under
/// `CARGO_TARGET_TMPDIR` can exceed.
struct RuntimeDir(PathBuf);

impl RuntimeDir {
    fn new(name: &str) -> RuntimeDir {
        let dir = std::env::temp_dir().join(format!("sbar-{}-{name}", std::process::id()));
        fs::create_dir(&dir).expect("create the runtime directory");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).expect("chmod");
        RuntimeDir(dir)
    }

    fn socket(&self) -> PathBuf {
        self.0.join("statsbar.sock")
    }
}

impl Drop for RuntimeDir {
    fn drop(&mut self) {
        // Only scratch files are left, and a failure here must not hide the test's own.
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn options() -> Options {
    Options {
        allowed_uid: rustix::process::geteuid().as_raw(),
        handshake_timeout: LIMIT,
    }
}

fn bind(dir: &RuntimeDir) -> Server {
    Server::bind(&dir.0, options()).expect("bind")
}

async fn within<T>(what: &str, work: impl Future<Output = T>) -> T {
    future::or(work, async {
        async_io::Timer::after(LIMIT).await;
        panic!("{what} timed out after {LIMIT:?}")
    })
    .await
}

/// Runs `work` while the server runs. The server must report no change meanwhile.
async fn serve<T>(server: &mut Server, work: impl Future<Output = T>) -> T {
    within(
        "serving",
        future::or(work, async {
            let change = server.next().await;
            panic!("unexpected {change:?}")
        }),
    )
    .await
}

async fn change(server: &mut Server) -> Change {
    within("a change", server.next()).await
}

struct Client {
    stream: Async<UnixStream>,
}

impl Client {
    async fn connect(dir: &RuntimeDir) -> Client {
        let stream = Async::<UnixStream>::connect(dir.socket())
            .await
            .expect("connect");
        Client { stream }
    }

    async fn greeted(dir: &RuntimeDir, server: &mut Server) -> Client {
        let mut client = Client::connect(dir).await;
        assert_eq!(
            serve(server, client.line()).await.as_deref(),
            Some(HELLO.trim_end())
        );
        client.send(HELLO).await;
        client
    }

    async fn watching(dir: &RuntimeDir, server: &mut Server, line: &str, page: Page) -> Client {
        let mut client = Client::greeted(dir, server).await;
        client.send(line).await;
        assert_eq!(change(server).await, Change::Subscribed(page));
        client
    }

    async fn send(&mut self, text: &str) {
        self.stream.write_all(text.as_bytes()).await.expect("send");
    }

    /// The next line, or `None` once the daemon has closed the connection.
    async fn line(&mut self) -> Option<String> {
        let mut line = Vec::new();
        let mut byte = [0];
        loop {
            match self.stream.read(&mut byte).await {
                Ok(1) if byte[0] == b'\n' => return Some(String::from_utf8(line).expect("UTF-8")),
                Ok(1) => line.push(byte[0]),
                // Closing with a request still unread resets instead of ending; both are closed.
                _ => return None,
            }
        }
    }

    async fn refused(&mut self, server: &mut Server, expected: ErrorReason) {
        let line = serve(server, self.line()).await.expect("an error line");
        match serde_json::from_str(&line).expect("an event") {
            Event::Error { reason, .. } => assert_eq!(reason, expected),
            other => panic!("not an error: {other:?}"),
        }
        assert_eq!(
            serve(server, self.line()).await,
            None,
            "closed after the error"
        );
    }
}

fn memory_event(total: u64) -> Event {
    Event::Memory(MemoryPage {
        buckets: MemoryBuckets {
            total,
            ..MemoryBuckets::default()
        },
        pressure: None,
        processes: None,
    })
}

#[test]
fn the_socket_is_private_and_removed_on_drop() {
    let dir = RuntimeDir::new("private");
    let server = bind(&dir);
    let mode = fs::metadata(dir.socket())
        .expect("the socket exists")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    drop(server);
    assert!(!dir.socket().exists());
}

#[test]
fn the_lock_and_the_socket_are_kept_by_the_runtime_directory_cleanup() {
    let dir = RuntimeDir::new("sticky");
    let _server = bind(&dir);
    let mode = |path: PathBuf| fs::metadata(path).expect("exists").permissions().mode();
    let lock = mode(dir.0.join("statsbar.lock"));
    let socket = mode(dir.socket());
    // The XDG spec lets the session clean up runtime files that have neither the sticky bit
    // nor a fresh access time.
    assert_ne!(lock & 0o1000, 0, "lock mode {lock:o}");
    assert_ne!(socket & 0o1000, 0, "socket mode {socket:o}");
    assert_eq!(lock & 0o777, 0o600, "lock mode {lock:o}");
}

#[test]
fn a_runtime_directory_others_can_read_is_refused() {
    let dir = RuntimeDir::new("shared");
    fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o755)).expect("chmod");
    match Server::bind(&dir.0, options()) {
        Err(BindError::Io(error)) => {
            assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
        }
        Err(other) => panic!("refused for another reason: {other:?}"),
        Ok(_) => panic!("bound in a directory others can read"),
    }
    assert!(!dir.0.join("statsbar.lock").exists());
    assert!(!dir.socket().exists());
}

#[test]
fn a_second_server_is_refused_and_the_first_keeps_working() {
    let dir = RuntimeDir::new("second");
    let mut server = bind(&dir);
    assert!(matches!(
        Server::bind(&dir.0, options()),
        Err(BindError::AlreadyRunning)
    ));
    async_io::block_on(async {
        Client::greeted(&dir, &mut server).await;
    });
}

#[test]
fn a_socket_left_by_a_killed_daemon_is_replaced() {
    let dir = RuntimeDir::new("stale");
    drop(std::os::unix::net::UnixListener::bind(dir.socket()).expect("leave a socket behind"));
    assert!(dir.socket().exists());
    let mut server = bind(&dir);
    async_io::block_on(async {
        Client::greeted(&dir, &mut server).await;
    });
}

#[test]
fn a_file_that_is_not_a_socket_is_left_alone() {
    let dir = RuntimeDir::new("not-socket");
    fs::write(dir.socket(), "keep").expect("write");
    assert!(matches!(
        Server::bind(&dir.0, options()),
        Err(BindError::Io(_))
    ));
    assert_eq!(
        fs::read_to_string(dir.socket()).expect("still there"),
        "keep"
    );
}

#[test]
fn a_missing_runtime_directory_is_an_error() {
    let dir = RuntimeDir::new("missing");
    let result = Server::bind(&dir.0.join("none"), options());
    assert!(matches!(result, Err(BindError::Io(_))));
}

#[test]
fn subscribing_replaces_the_page_and_unsubscribing_clears_it() {
    let dir = RuntimeDir::new("subscribe");
    let mut server = bind(&dir);
    async_io::block_on(async {
        let mut client = Client::watching(&dir, &mut server, WATCH_CPU, Page::Cpu).await;
        assert!(server.watched(Page::Cpu) && !server.watched(Page::Memory));
        client.send(WATCH_MEMORY).await;
        assert_eq!(change(&mut server).await, Change::Subscribed(Page::Memory));
        assert!(!server.watched(Page::Cpu) && server.watched(Page::Memory));
        client.send("{\"type\":\"unsubscribe\"}\n").await;
        assert_eq!(change(&mut server).await, Change::Unwatched);
        assert!(!server.watched(Page::Memory));
    });
}

#[test]
fn requests_that_arrive_together_are_taken_in_order() {
    let dir = RuntimeDir::new("pipelined");
    let mut server = bind(&dir);
    async_io::block_on(async {
        let mut client = Client::connect(&dir).await;
        client
            .send(&format!(
                "{HELLO}{WATCH_CPU}{{\"type\":\"refresh\"}}\n{WATCH_MEMORY}"
            ))
            .await;
        assert_eq!(change(&mut server).await, Change::Subscribed(Page::Cpu));
        assert_eq!(change(&mut server).await, Change::Refresh(Page::Cpu));
        assert_eq!(change(&mut server).await, Change::Subscribed(Page::Memory));
    });
}

#[test]
fn an_event_reaches_only_the_page_it_is_for() {
    let dir = RuntimeDir::new("publish");
    let mut server = bind(&dir);
    async_io::block_on(async {
        let mut cpu = Client::watching(&dir, &mut server, WATCH_CPU, Page::Cpu).await;
        let mut first = Client::watching(&dir, &mut server, WATCH_MEMORY, Page::Memory).await;
        let mut second = Client::watching(&dir, &mut server, WATCH_MEMORY, Page::Memory).await;
        server.publish(Page::Memory, &memory_event(7));
        let expected = serde_json::to_string(&memory_event(7)).expect("serialises");
        assert_eq!(
            serve(&mut server, first.line()).await,
            Some(expected.clone())
        );
        assert_eq!(serve(&mut server, second.line()).await, Some(expected));
        // The first line the CPU watcher gets is the CPU page's, so it never got the other.
        let marker = Event::Error {
            reason: ErrorReason::Busy,
            message: "marker".to_owned(),
        };
        server.publish(Page::Cpu, &marker);
        let expected = serde_json::to_string(&marker).expect("serialises");
        assert_eq!(serve(&mut server, cpu.line()).await, Some(expected));
    });
}

#[test]
fn only_a_new_watcher_is_welcomed() {
    let dir = RuntimeDir::new("welcome");
    let mut server = bind(&dir);
    async_io::block_on(async {
        let mut old = Client::watching(&dir, &mut server, WATCH_MEMORY, Page::Memory).await;
        server.publish(Page::Memory, &memory_event(1));
        let mut new = Client::watching(&dir, &mut server, WATCH_MEMORY, Page::Memory).await;
        server.welcome(Page::Memory, &memory_event(1));
        // Welcomed once: a page published since then is current for everyone.
        server.publish(Page::Memory, &memory_event(2));
        server.welcome(Page::Memory, &memory_event(2));
        server.publish(Page::Memory, &memory_event(3));
        let line = |total| Some(serde_json::to_string(&memory_event(total)).expect("serialises"));
        for total in [1, 2, 3] {
            assert_eq!(serve(&mut server, old.line()).await, line(total));
        }
        for total in [1, 2, 3] {
            assert_eq!(serve(&mut server, new.line()).await, line(total));
        }
    });
}

#[test]
fn a_client_with_a_backlog_does_not_starve_the_others() {
    let dir = RuntimeDir::new("backlog");
    let mut server = bind(&dir);
    async_io::block_on(async {
        // Both connections and every line are queued before the server takes any of them.
        let mut busy = Client::connect(&dir).await;
        let mut other = Client::connect(&dir).await;
        let idle = "{\"type\":\"unsubscribe\"}\n".repeat(LINES_PER_TURN * 10);
        busy.send(&format!("{HELLO}{idle}{WATCH_CPU}")).await;
        other.send(&format!("{HELLO}{WATCH_MEMORY}")).await;
        assert_eq!(change(&mut server).await, Change::Subscribed(Page::Memory));
        assert_eq!(change(&mut server).await, Change::Subscribed(Page::Cpu));
    });
}

#[test]
fn a_client_with_a_stream_of_requests_does_not_starve_the_others() {
    let dir = RuntimeDir::new("stream");
    let mut server = bind(&dir);
    async_io::block_on(async {
        let mut busy = Client::watching(&dir, &mut server, WATCH_CPU, Page::Cpu).await;
        busy.send(&"{\"type\":\"refresh\"}\n".repeat(40)).await;
        assert_eq!(change(&mut server).await, Change::Refresh(Page::Cpu));
        let mut other = Client::connect(&dir).await;
        other.send(&format!("{HELLO}{WATCH_MEMORY}")).await;
        let mut changes = Vec::new();
        for _ in 0..2 {
            changes.push(change(&mut server).await);
        }
        assert!(
            changes.contains(&Change::Subscribed(Page::Memory)),
            "{changes:?}"
        );
    });
}

#[test]
fn accepts_are_bounded_per_turn() {
    let dir = RuntimeDir::new("accepts");
    let mut server = bind(&dir);
    let streams: Vec<UnixStream> = (0..=ACCEPTS_PER_TURN)
        .map(|_| UnixStream::connect(dir.socket()).expect("connect"))
        .collect();
    let last = streams.last().expect("connected");
    last.set_nonblocking(true).expect("non-blocking");
    let mut byte = [0];
    async_io::block_on(async {
        assert_eq!(future::poll_once(server.next()).await, None);
        assert_eq!(
            (&*last).read(&mut byte).expect_err("not taken yet").kind(),
            std::io::ErrorKind::WouldBlock
        );
        assert_eq!(future::poll_once(server.next()).await, None);
    });
    assert_eq!((&*last).read(&mut byte).expect("taken next turn"), 1);
}

#[test]
fn a_page_stays_watched_until_its_last_watcher_leaves() {
    let dir = RuntimeDir::new("two-watchers");
    let mut server = bind(&dir);
    async_io::block_on(async {
        let first = Client::watching(&dir, &mut server, WATCH_CPU, Page::Cpu).await;
        let second = Client::watching(&dir, &mut server, WATCH_CPU, Page::Cpu).await;
        drop(first);
        assert_eq!(change(&mut server).await, Change::Unwatched);
        assert!(server.watched(Page::Cpu));
        drop(second);
        assert_eq!(change(&mut server).await, Change::Unwatched);
        assert!(!server.watched(Page::Cpu));
    });
}

#[test]
fn another_protocol_version_is_refused() {
    let dir = RuntimeDir::new("version");
    let mut server = bind(&dir);
    async_io::block_on(async {
        let mut client = Client::connect(&dir).await;
        let first = serve(&mut server, client.line())
            .await
            .expect("the daemon's hello");
        assert_eq!(
            serde_json::from_str::<Hello>(&first).expect("a hello"),
            Hello::current()
        );
        client.send("{\"protocol\":2}\n").await;
        client
            .refused(&mut server, ErrorReason::VersionMismatch)
            .await;
    });
}

#[test]
fn a_first_line_that_is_not_a_hello_is_refused() {
    let dir = RuntimeDir::new("no-hello");
    let mut server = bind(&dir);
    async_io::block_on(async {
        let mut client = Client::connect(&dir).await;
        serve(&mut server, client.line())
            .await
            .expect("the daemon's hello");
        client.send(WATCH_CPU).await;
        client.refused(&mut server, ErrorReason::BadRequest).await;
    });
}

#[test]
fn malformed_requests_are_refused() {
    for (index, bad) in ["not json\n", "{\"type\":\"kill\",\"pid\":1}\n", "\n"]
        .iter()
        .enumerate()
    {
        let dir = RuntimeDir::new(&format!("bad-{index}"));
        let mut server = bind(&dir);
        async_io::block_on(async {
            let mut client = Client::greeted(&dir, &mut server).await;
            client.send(bad).await;
            client.refused(&mut server, ErrorReason::BadRequest).await;
        });
    }
}

#[test]
fn a_bad_request_from_a_watcher_releases_its_page() {
    let dir = RuntimeDir::new("bad-watcher");
    let mut server = bind(&dir);
    async_io::block_on(async {
        let mut client = Client::watching(&dir, &mut server, WATCH_CPU, Page::Cpu).await;
        client.send("not json\n").await;
        assert_eq!(change(&mut server).await, Change::Unwatched);
        assert!(!server.watched(Page::Cpu));
        client.refused(&mut server, ErrorReason::BadRequest).await;
    });
}

#[test]
fn a_line_without_an_end_is_refused_at_the_limit() {
    let dir = RuntimeDir::new("long-line");
    let mut server = bind(&dir);
    async_io::block_on(async {
        let mut client = Client::greeted(&dir, &mut server).await;
        client.send(&"x".repeat(MAX_REQUEST_BYTES + 1)).await;
        client.refused(&mut server, ErrorReason::BadRequest).await;
    });
}

/// `request` padded with spaces, which JSON allows, to `length` bytes before its newline.
fn padded(request: &str, length: usize) -> String {
    format!("{:<length$}\n", request.trim_end())
}

#[test]
fn a_line_over_the_limit_is_refused_even_with_its_end() {
    let dir = RuntimeDir::new("long-ended");
    let mut server = bind(&dir);
    async_io::block_on(async {
        let mut client = Client::greeted(&dir, &mut server).await;
        // A valid request, so only the limit can refuse it.
        client.send(&padded(WATCH_CPU, MAX_REQUEST_BYTES + 1)).await;
        client.refused(&mut server, ErrorReason::BadRequest).await;
    });
}

#[test]
fn a_line_at_the_limit_is_taken() {
    let dir = RuntimeDir::new("at-limit");
    let mut server = bind(&dir);
    async_io::block_on(async {
        let mut client = Client::greeted(&dir, &mut server).await;
        client.send(&padded(WATCH_CPU, MAX_REQUEST_BYTES)).await;
        assert_eq!(change(&mut server).await, Change::Subscribed(Page::Cpu));
    });
}

#[test]
fn a_client_that_never_greets_is_timed_out() {
    let dir = RuntimeDir::new("handshake");
    // Already over when the client connects, so the test waits on no clock.
    let options = Options {
        handshake_timeout: Duration::ZERO,
        ..options()
    };
    let mut server = Server::bind(&dir.0, options).expect("bind");
    async_io::block_on(async {
        let mut client = Client::connect(&dir).await;
        serve(&mut server, client.line())
            .await
            .expect("the daemon's hello");
        client
            .refused(&mut server, ErrorReason::HandshakeTimeout)
            .await;
    });
}

#[test]
fn a_hello_that_came_in_time_is_taken_after_a_stall() {
    let dir = RuntimeDir::new("stall");
    // Over before the server first looks at the client, as after a loop that was held up.
    let options = Options {
        handshake_timeout: Duration::ZERO,
        ..options()
    };
    let mut server = Server::bind(&dir.0, options).expect("bind");
    async_io::block_on(async {
        let mut client = Client::connect(&dir).await;
        client.send(HELLO).await;
        client.send(WATCH_CPU).await;
        assert_eq!(change(&mut server).await, Change::Subscribed(Page::Cpu));
    });
}

#[test]
fn one_client_too_many_is_told_busy_and_a_freed_place_is_reused() {
    let dir = RuntimeDir::new("busy");
    let mut server = bind(&dir);
    async_io::block_on(async {
        let mut clients = Vec::new();
        for _ in 0..MAX_CLIENTS {
            clients.push(Client::watching(&dir, &mut server, WATCH_CPU, Page::Cpu).await);
        }
        let mut extra = Client::connect(&dir).await;
        serve(&mut server, extra.line())
            .await
            .expect("the daemon's hello");
        extra.refused(&mut server, ErrorReason::Busy).await;
        drop(clients.pop());
        assert_eq!(change(&mut server).await, Change::Unwatched);
        Client::watching(&dir, &mut server, WATCH_CPU, Page::Cpu).await;
    });
}

#[test]
fn a_client_of_another_uid_gets_nothing() {
    let dir = RuntimeDir::new("uid");
    let options = Options {
        allowed_uid: options().allowed_uid + 1,
        ..options()
    };
    let mut server = Server::bind(&dir.0, options).expect("bind");
    async_io::block_on(async {
        let mut client = Client::connect(&dir).await;
        assert_eq!(serve(&mut server, client.line()).await, None);
    });
}

#[test]
fn a_client_that_stops_reading_is_dropped_instead_of_buffered() {
    let dir = RuntimeDir::new("stalled");
    let mut server = bind(&dir);
    async_io::block_on(async {
        let _client = Client::watching(&dir, &mut server, WATCH_MEMORY, Page::Memory).await;
        // About 160 bytes each and never flushed, so the 64 KiB bound is passed well before 1000.
        for total in 0..1000 {
            server.publish(Page::Memory, &memory_event(total));
        }
        assert!(!server.watched(Page::Memory));
    });
}
````

Create `crates/daemon/tests/ipc_accept_failure.rs` (its own binary, because it lowers the open-file limit of the whole process):

````rust
//! A failing accept backs off instead of spinning. Its own test binary: lowering the process's
//! file limit would break any test running beside it.

use std::fs::{self, File};
use std::future::Future;
use std::io::{ErrorKind, Read};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::pin::pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;

use async_io::Async;
use futures_lite::future;
use futures_lite::io::AsyncReadExt;
use rustix::io::Errno;
use rustix::process::{Resource, Rlimit, getrlimit, setrlimit};
use statsbard::ipc::{Options, Server};

/// Generous for a local socket; it only has to tell "slow" from "hung".
const LIMIT: Duration = Duration::from_secs(5);
const HELLO: &[u8] = b"{\"protocol\":1}\n";
/// Low enough that filling every free descriptor below it is quick.
const FILE_LIMIT: u64 = 256;

/// A short private directory: a socket path is limited to 108 bytes.
struct RuntimeDir(PathBuf);

impl RuntimeDir {
    fn new(name: &str) -> RuntimeDir {
        let dir = std::env::temp_dir().join(format!("sbar-{}-{name}", std::process::id()));
        fs::create_dir(&dir).expect("create the runtime directory");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).expect("chmod");
        RuntimeDir(dir)
    }
}

impl Drop for RuntimeDir {
    fn drop(&mut self) {
        // Only scratch files are left, and a failure here must not hide the test's own.
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Takes every free descriptor, so the next one asked for fails with EMFILE. Dropping it gives
/// them back and restores the file limit, also when the test panics.
struct NoFreeDescriptors {
    original: Rlimit,
    taken: Vec<File>,
}

impl NoFreeDescriptors {
    fn take() -> NoFreeDescriptors {
        let original = getrlimit(Resource::Nofile);
        let lowered = Rlimit {
            current: Some(
                original
                    .current
                    .map_or(FILE_LIMIT, |soft| soft.min(FILE_LIMIT)),
            ),
            maximum: original.maximum,
        };
        setrlimit(Resource::Nofile, lowered).expect("lower the file limit");
        let mut held = NoFreeDescriptors {
            original,
            taken: Vec::new(),
        };
        loop {
            match File::open("/dev/null") {
                Ok(file) => held.taken.push(file),
                Err(error) if Errno::from_io_error(&error) == Some(Errno::MFILE) => return held,
                Err(error) => panic!("taking a descriptor failed: {error}"),
            }
            assert!(
                held.taken.len() as u64 <= FILE_LIMIT,
                "the file limit did not hold"
            );
        }
    }
}

impl Drop for NoFreeDescriptors {
    fn drop(&mut self) {
        self.taken.clear();
        // Not a panic: this also runs while a failed test unwinds.
        if let Err(error) = setrlimit(Resource::Nofile, self.original) {
            eprintln!("restoring the file limit failed: {error}");
        }
    }
}

/// Counts wake-ups, which a spinning loop asks for on every turn.
#[derive(Default)]
struct WakeCount(AtomicUsize);

impl Wake for WakeCount {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// Polls the server once and reports whether it asked to be polled again at once.
fn turn(server: &mut Server) -> (Poll<statsbard::ipc::Change>, usize) {
    let count = Arc::new(WakeCount::default());
    let waker = Waker::from(count.clone());
    let polled = pin!(server.next()).poll(&mut Context::from_waker(&waker));
    (polled, count.0.load(Ordering::SeqCst))
}

fn has_nothing(client: &Async<UnixStream>) -> bool {
    let mut byte = [0];
    let mut stream = client.get_ref();
    matches!(
        stream.read(&mut byte),
        Err(error) if error.kind() == ErrorKind::WouldBlock
    )
}

#[test]
fn a_failing_accept_waits_before_it_is_retried() {
    let dir = RuntimeDir::new("emfile");
    let options = Options {
        allowed_uid: rustix::process::geteuid().as_raw(),
        handshake_timeout: LIMIT,
    };
    let mut server = Server::bind(&dir.0, options).expect("bind");
    let mut client = async_io::block_on(Async::<UnixStream>::connect(dir.0.join("statsbar.sock")))
        .expect("connect");

    let held = NoFreeDescriptors::take();
    let (polled, wakes) = turn(&mut server);
    assert!(polled.is_pending(), "{polled:?}");
    assert_eq!(wakes, 0, "the server asked to be polled again at once");
    drop(held);
    assert!(has_nothing(&client), "accepted without a free descriptor");

    // A descriptor is free now, but the retry is not due yet.
    let (polled, wakes) = turn(&mut server);
    assert!(polled.is_pending(), "{polled:?}");
    assert_eq!(wakes, 0);
    assert!(has_nothing(&client), "retried before the backoff");

    let mut hello = [0; HELLO.len()];
    async_io::block_on(future::or(
        async {
            future::or(client.read_exact(&mut hello), async {
                let change = server.next().await;
                panic!("unexpected {change:?}")
            })
            .await
            .expect("the daemon's hello");
        },
        async {
            async_io::Timer::after(LIMIT).await;
            panic!("the retry timed out after {LIMIT:?}")
        },
    ));
    assert_eq!(hello, HELLO);
}
````

Apply to `crates/daemon/tests/paths.rs`:

````diff
diff --git a/crates/daemon/tests/paths.rs b/crates/daemon/tests/paths.rs
--- a/crates/daemon/tests/paths.rs
+++ b/crates/daemon/tests/paths.rs
@@ -1,7 +1,7 @@
 use std::ffi::OsString;
 use std::path::{Path, PathBuf};

-use statsbard::paths::{autostart_file, config_file, config_home};
+use statsbard::paths::{autostart_file, config_file, config_home, runtime_dir};

 fn env(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<OsString> {
     move |key| {
@@ -45,3 +45,12 @@ fn files_sit_under_the_config_home() {
         Path::new("/home/u/.config/autostart/statsbar.desktop")
     );
 }
+
+#[test]
+fn the_runtime_directory_must_be_an_absolute_path() {
+    let set = env(&[("XDG_RUNTIME_DIR", "/run/user/1000")]);
+    assert_eq!(runtime_dir(set), Some(PathBuf::from("/run/user/1000")));
+    assert_eq!(runtime_dir(env(&[("XDG_RUNTIME_DIR", "run")])), None);
+    assert_eq!(runtime_dir(env(&[("XDG_RUNTIME_DIR", "")])), None);
+    assert_eq!(runtime_dir(env(&[("HOME", "/home/u")])), None);
+}
````

- [x] **Step 2: Run them, expect FAIL**

```bash
cargo test -p statsbard --test ipc --test ipc_accept_failure --test paths
```

Expected: the tests do not compile (E0432 for `statsbard::ipc` and `runtime_dir`).

- [x] **Step 3: Implement**

Apply to `crates/core/src/ipc.rs`:

````diff
diff --git a/crates/core/src/ipc.rs b/crates/core/src/ipc.rs
--- a/crates/core/src/ipc.rs
+++ b/crates/core/src/ipc.rs
@@ -49,7 +49,8 @@ pub enum Request {
 pub enum Event {
     Cpu(CpuPage),
     Memory(MemoryPage),
-    /// Always the last line before the daemon closes the connection.
+    /// The last line before the daemon closes a connection for a protocol reason. A connection
+    /// it drops for its uid, a stalled reader or a failed write gets none.
     Error {
         reason: ErrorReason,
         message: String,
````

Create `crates/daemon/src/ipc.rs`:

````rust
//! The socket `statsbar-panel` connects to. It runs on the main loop: every read, write and
//! accept is non-blocking, so no client can hold up polling or the tray.
//!
//! The socket lets a peer watch figures, never act on the machine: no request writes or signals.

use std::fmt;
use std::fs::{self, File};
use std::future::{Future, poll_fn};
use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use std::time::Duration;

use async_io::{Async, Timer};
use futures_lite::io::{AsyncRead, AsyncWrite};
use rustix::fs::{FlockOperation, flock};
use statsbar_core::ipc::{ErrorReason, Event, Hello, PROTOCOL_VERSION, Page, Request};

const SOCKET: &str = "statsbar.sock";
const LOCK: &str = "statsbar.lock";
const PRIVATE: u32 = 0o600;
/// The XDG Base Directory spec lets the session clean up runtime files that have neither this
/// bit nor a fresh access time. A removed lock would let a second daemon start.
const STICKY: u32 = 0o1000;
/// Group and other permission bits.
const SHARED: u32 = 0o077;
/// One panel is the norm; the rest is room for `socat` and a panel that is restarting.
pub const MAX_CLIENTS: usize = 4;
/// The longest request is under 50 bytes.
pub const MAX_REQUEST_BYTES: usize = 4096;
/// About half a minute of page events. A client this far behind has stopped reading.
const MAX_UNSENT_BYTES: usize = 64 * 1024;
/// A failing accept, such as out of file descriptors, stays ready and would spin the loop.
const ACCEPT_RETRY: Duration = Duration::from_secs(1);
/// Requests that change nothing, taken from one client in one turn. Past it the client waits
/// for the next turn, so a stream of them cannot hold up the other clients or the main loop.
pub const LINES_PER_TURN: usize = 16;
/// Connections taken in one turn, for the same reason.
pub const ACCEPTS_PER_TURN: usize = 8;

#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Only this uid may connect.
    pub allowed_uid: u32,
    /// For the client's first line, and for flushing an error before closing.
    pub handshake_timeout: Duration,
}

/// What the main loop has to act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Subscribed(Page),
    Refresh(Page),
    /// A watcher left its page. Others may still watch it.
    Unwatched,
}

#[derive(Debug)]
pub enum BindError {
    AlreadyRunning,
    Io(io::Error),
}

impl fmt::Display for BindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BindError::AlreadyRunning => write!(f, "StatsBar is already running"),
            BindError::Io(error) => write!(f, "cannot open the panel's socket: {error}"),
        }
    }
}

impl std::error::Error for BindError {}

impl From<io::Error> for BindError {
    fn from(error: io::Error) -> BindError {
        BindError::Io(error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Greeting,
    Ready,
    /// Only flushes what is left, then closes.
    Closing,
}

enum Step {
    Changed(Change),
    Gone,
}

struct Client {
    stream: Async<UnixStream>,
    state: State,
    page: Option<Page>,
    /// Subscribed since the page was last sent.
    fresh: bool,
    input: Vec<u8>,
    output: Vec<u8>,
    /// Bounds `Greeting` and `Closing`.
    deadline: Timer,
    timeout: Duration,
}

pub struct Server {
    listener: Async<UnixListener>,
    socket: PathBuf,
    /// Held for the daemon's lifetime; the kernel releases it however the process ends.
    _lock: File,
    options: Options,
    clients: Vec<Client>,
    accept_retry: Option<Timer>,
}

impl Server {
    /// Claims the runtime directory for this daemon, then listens there. The lock is what makes
    /// replacing a leftover socket safe: its owner is known to be gone.
    pub fn bind(runtime_dir: &Path, options: Options) -> Result<Server, BindError> {
        check_private(runtime_dir)?;
        // `truncate(false)`: the file only carries the lock.
        let lock = File::options()
            .write(true)
            .create(true)
            .truncate(false)
            .open(runtime_dir.join(LOCK))?;
        match flock(&lock, FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => {}
            Err(rustix::io::Errno::WOULDBLOCK) => return Err(BindError::AlreadyRunning),
            Err(error) => return Err(BindError::Io(error.into())),
        }
        // Only once the lock is ours, so a running daemon's file is left as it is.
        lock.set_permissions(fs::Permissions::from_mode(PRIVATE | STICKY))?;

        let socket = runtime_dir.join(SOCKET);
        match fs::symlink_metadata(&socket) {
            Ok(found) if found.file_type().is_socket() => fs::remove_file(&socket)?,
            Ok(_) => {
                return Err(BindError::Io(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("{} exists and is not a socket", socket.display()),
                )));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let listener = UnixListener::bind(&socket)?;
        let listener =
            match fs::set_permissions(&socket, fs::Permissions::from_mode(PRIVATE | STICKY))
                .and_then(|()| Async::new(listener))
            {
                Ok(listener) => listener,
                Err(error) => {
                    // No `Server` owns the socket yet, so its `Drop` would not remove it.
                    if let Err(removal) = fs::remove_file(&socket) {
                        eprintln!("statsbard: removing {} failed: {removal}", socket.display());
                    }
                    return Err(error.into());
                }
            };
        Ok(Server {
            listener,
            socket,
            _lock: lock,
            options,
            clients: Vec::new(),
            accept_retry: None,
        })
    }

    pub fn watched(&self, page: Page) -> bool {
        self.clients.iter().any(|client| client.page == Some(page))
    }

    /// Queues `event` for the page's watchers. It is sent while [`Server::next`] is awaited.
    pub fn publish(&mut self, page: Page, event: &Event) {
        self.queue(page, event, false);
    }

    /// Queues `event`, the page as last published, for the watchers that subscribed since then,
    /// so a page opens with figures without a read outside the cadence.
    pub fn welcome(&mut self, page: Page, event: &Event) {
        self.queue(page, event, true);
    }

    fn queue(&mut self, page: Page, event: &Event, only_fresh: bool) {
        let is_for = |client: &Client| client.page == Some(page) && (client.fresh || !only_fresh);
        if !self.clients.iter().any(is_for) {
            return;
        }
        let line = match encode(event) {
            Ok(line) => line,
            Err(error) => {
                eprintln!("statsbard: encoding the {page:?} page failed: {error}");
                return;
            }
        };
        self.clients.retain_mut(|client| {
            if !is_for(client) {
                return true;
            }
            if client.output.len() + line.len() > MAX_UNSENT_BYTES {
                eprintln!("statsbard: dropped a panel connection that stopped reading");
                return false;
            }
            client.output.extend_from_slice(&line);
            client.fresh = false;
            true
        });
    }

    /// Serves until something changes. Dropping the future loses nothing: all state is in
    /// `self`.
    pub async fn next(&mut self) -> Change {
        poll_fn(|cx| self.poll_next(cx)).await
    }

    fn poll_next(&mut self, cx: &mut Context<'_>) -> Poll<Change> {
        // Accepts first, so a client with a stream of requests cannot keep new ones out.
        let mut accepted = 0;
        while accepted < ACCEPTS_PER_TURN && self.poll_accept(cx).is_ready() {
            accepted += 1;
        }
        if accepted == ACCEPTS_PER_TURN {
            // More may be waiting, and the listener's waker was not registered.
            cx.waker().wake_by_ref();
        }
        // New clients are polled here too, which registers their wakers.
        let mut index = 0;
        while index < self.clients.len() {
            match self.clients[index].poll(cx) {
                Poll::Pending => index += 1,
                Poll::Ready(Step::Changed(change)) => {
                    // To the back, so the others are polled first next time.
                    let client = self.clients.remove(index);
                    self.clients.push(client);
                    return Poll::Ready(change);
                }
                Poll::Ready(Step::Gone) => {
                    if self.clients.remove(index).page.is_some() {
                        return Poll::Ready(Change::Unwatched);
                    }
                }
            }
        }
        Poll::Pending
    }

    /// Ready after accepting or refusing one connection.
    fn poll_accept(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        if let Some(retry) = &mut self.accept_retry {
            ready!(Pin::new(retry).poll(cx));
            self.accept_retry = None;
        }
        loop {
            match self.listener.get_ref().accept() {
                Ok((stream, _)) => {
                    self.admit(stream);
                    return Poll::Ready(());
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    match ready!(self.listener.poll_readable(cx)) {
                        Ok(()) => continue,
                        Err(error) => return self.accept_failed(&error),
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return self.accept_failed(&error),
            }
        }
    }

    fn accept_failed(&mut self, error: &io::Error) -> Poll<()> {
        eprintln!("statsbard: accepting a panel connection failed: {error}");
        self.accept_retry = Some(Timer::after(ACCEPT_RETRY));
        // Polls the retry timer on the next turn, which registers its waker.
        Poll::Ready(())
    }

    fn admit(&mut self, stream: UnixStream) {
        match rustix::net::sockopt::socket_peercred(&stream) {
            Ok(peer) if peer.uid.as_raw() == self.options.allowed_uid => {}
            Ok(peer) => {
                eprintln!(
                    "statsbard: refused a connection from uid {}",
                    peer.uid.as_raw()
                );
                return;
            }
            Err(error) => {
                eprintln!("statsbard: refused a connection of unknown uid: {error}");
                return;
            }
        }
        let stream = match Async::new(stream) {
            Ok(stream) => stream,
            Err(error) => {
                eprintln!("statsbard: setting up a panel connection failed: {error}");
                return;
            }
        };
        let hello = match encode(&Hello::current()) {
            Ok(line) => line,
            Err(error) => {
                eprintln!("statsbard: encoding the hello failed: {error}");
                return;
            }
        };
        if self.clients.len() >= MAX_CLIENTS {
            refuse_busy(&stream, hello);
            return;
        }
        self.clients.push(Client {
            stream,
            state: State::Greeting,
            page: None,
            fresh: false,
            input: Vec::new(),
            output: hello,
            deadline: Timer::after(self.options.handshake_timeout),
            timeout: self.options.handshake_timeout,
        });
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        // Before the lock goes, so this can never remove a successor's socket.
        if let Err(error) = fs::remove_file(&self.socket) {
            eprintln!(
                "statsbard: removing {} failed: {error}",
                self.socket.display()
            );
        }
    }
}

impl Client {
    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<Step> {
        let mut handled = 0;
        loop {
            while !self.output.is_empty() {
                match Pin::new(&mut &self.stream).poll_write(cx, &self.output) {
                    Poll::Ready(Ok(0) | Err(_)) => return Poll::Ready(Step::Gone),
                    Poll::Ready(Ok(written)) => {
                        self.output.drain(..written);
                    }
                    Poll::Pending => break,
                }
            }
            if self.state == State::Closing {
                if self.output.is_empty() {
                    return Poll::Ready(Step::Gone);
                }
                ready!(Pin::new(&mut self.deadline).poll(cx));
                return Poll::Ready(Step::Gone);
            }

            let end = self.input.iter().position(|&byte| byte == b'\n');
            // Whether or not its end has arrived, so the limit is the same however reads split.
            if end.unwrap_or(self.input.len()) > MAX_REQUEST_BYTES {
                if let Some(change) = self.fail(ErrorReason::BadRequest, "the line is too long") {
                    return Poll::Ready(Step::Changed(change));
                }
                continue;
            }
            if let Some(end) = end {
                let line: Vec<u8> = self.input.drain(..=end).collect();
                if let Some(change) = self.take_line(&line) {
                    return Poll::Ready(Step::Changed(change));
                }
                handled += 1;
                if handled == LINES_PER_TURN {
                    cx.waker().wake_by_ref();
                    return Poll::Pending;
                }
                continue;
            }
            let mut chunk = [0; 1024];
            match Pin::new(&mut &self.stream).poll_read(cx, &mut chunk) {
                Poll::Ready(Ok(0) | Err(_)) => return Poll::Ready(Step::Gone),
                Poll::Ready(Ok(read)) => self.input.extend_from_slice(&chunk[..read]),
                // Only once nothing is left to read: a hello that came in time is taken even
                // when the loop was held up past the deadline.
                Poll::Pending
                    if self.state == State::Greeting
                        && Pin::new(&mut self.deadline).poll(cx).is_ready() =>
                {
                    // Nothing is watched before the greeting, so this changes no page.
                    self.fail(ErrorReason::HandshakeTimeout, "no protocol line arrived");
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }

    fn take_line(&mut self, line: &[u8]) -> Option<Change> {
        if self.state == State::Greeting {
            match serde_json::from_slice::<Hello>(line) {
                Ok(hello) if hello.protocol == PROTOCOL_VERSION => {
                    self.state = State::Ready;
                    return None;
                }
                Ok(hello) => {
                    let message = format!(
                        "statsbard speaks protocol {PROTOCOL_VERSION}, not {}; restart StatsBar",
                        hello.protocol
                    );
                    return self.fail(ErrorReason::VersionMismatch, &message);
                }
                Err(_) => {
                    return self.fail(
                        ErrorReason::BadRequest,
                        "the first line must be the protocol",
                    );
                }
            }
        }
        match serde_json::from_slice::<Request>(line) {
            Ok(Request::Subscribe { page }) => {
                self.page = Some(page);
                self.fresh = true;
                Some(Change::Subscribed(page))
            }
            Ok(Request::Unsubscribe) => self.page.take().map(|_| Change::Unwatched),
            Ok(Request::Refresh) => self.page.map(Change::Refresh),
            // The line is the peer's, so it is neither echoed nor logged.
            Err(_) => self.fail(ErrorReason::BadRequest, "not a request of this protocol"),
        }
    }

    /// Queues the error and starts closing.
    fn fail(&mut self, reason: ErrorReason, message: &str) -> Option<Change> {
        self.state = State::Closing;
        self.deadline = Timer::after(self.timeout);
        self.input.clear();
        let error = Event::Error {
            reason,
            message: message.to_owned(),
        };
        match encode(&error) {
            Ok(line) => self.output.extend_from_slice(&line),
            Err(error) => eprintln!("statsbard: encoding an error line failed: {error}"),
        }
        self.page.take().map(|_| Change::Unwatched)
    }
}

/// Refuses a runtime directory another user could write to or read: there they could hold the
/// lock or plant a link in the socket's place.
fn check_private(dir: &Path) -> io::Result<()> {
    let found = fs::metadata(dir)?;
    if found.is_dir()
        && found.uid() == rustix::process::geteuid().as_raw()
        && found.mode() & SHARED == 0
    {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        format!("{} is not a directory private to this user", dir.display()),
    ))
}

/// One try, without waiting: two short lines always fit a new socket's buffer, and a connection
/// over the limit gets no state of its own.
fn refuse_busy(stream: &Async<UnixStream>, hello: Vec<u8>) {
    let busy = Event::Error {
        reason: ErrorReason::Busy,
        message: format!("statsbard already has {MAX_CLIENTS} connections"),
    };
    let sent = encode(&busy)
        .map_err(io::Error::from)
        .and_then(|line| io::Write::write_all(&mut stream.get_ref(), &[hello, line].concat()));
    if let Err(error) = sent {
        eprintln!("statsbard: refusing a connection over the limit failed: {error}");
    }
}

fn encode(value: &impl serde::Serialize) -> serde_json::Result<Vec<u8>> {
    let mut line = serde_json::to_vec(value)?;
    line.push(b'\n');
    Ok(line)
}
````

Apply to `crates/daemon/src/lib.rs`:

````diff
diff --git a/crates/daemon/src/lib.rs b/crates/daemon/src/lib.rs
--- a/crates/daemon/src/lib.rs
+++ b/crates/daemon/src/lib.rs
@@ -6,6 +6,7 @@ pub mod app;
 pub mod autostart;
 pub mod config;
 pub mod fsutil;
+pub mod ipc;
 pub mod pages;
 pub mod paths;
 pub mod power;
````

Apply to `crates/daemon/src/paths.rs`:

````diff
diff --git a/crates/daemon/src/paths.rs b/crates/daemon/src/paths.rs
--- a/crates/daemon/src/paths.rs
+++ b/crates/daemon/src/paths.rs
@@ -22,3 +22,11 @@ pub fn config_file(config_home: &Path) -> PathBuf {
 pub fn autostart_file(config_home: &Path) -> PathBuf {
     config_home.join("autostart/statsbar.desktop")
 }
+
+/// `$XDG_RUNTIME_DIR`, when it is absolute. It has no fallback: a shared directory such as `/tmp`
+/// would let another user take the socket's place.
+pub fn runtime_dir(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
+    var("XDG_RUNTIME_DIR")
+        .map(PathBuf::from)
+        .filter(|path| path.is_absolute())
+}
````

- [x] **Step 4: Run, expect PASS**

```bash
cargo test -p statsbard --test ipc --test ipc_accept_failure --test paths
```

Expected: `ipc` 27 pass, `ipc_accept_failure` 1 passes, and so does every `paths` test.

- [x] **Step 5: Checkpoint**

Ready to commit: the files listed above — "feat(daemon): serve a private panel socket"

### Task 8: Wire the pages into the main loop

**Test level:** end-to-end. The tests start the real binary on a private `dbus-daemon` with a
stand-in tray watcher and talk to its real socket.

**Files:**

- Modify: `crates/daemon/src/app.rs` (lines after 24, after 36)
- Modify: `crates/daemon/src/run.rs` (lines 2–3, after 13, 21, after 22, after 70, after 87, 89, after 168, after 178, 182, after 191, after 231, 270–272, after 280, 337, after 341, 432, 434)
- Modify: `crates/daemon/tests/app_pages.rs` (lines 7, after 218)
- Create: `crates/daemon/tests/daemon_ipc.rs`

**Interfaces:**

- Consumes: Task 5's `App` methods, Task 6's `Pages`, Task 7's `Server`, `Options`, `Change` and
  `paths::runtime_dir`.
- Produces:

```rust
// crates/daemon/src/app.rs
pub const PAGES: [Page; 2] = [Page::Cpu, Page::Memory];
pub fn page_of(id: ReaderId) -> Option<Page>;
```

- [x] **Step 1: Write the failing tests**

Apply to `crates/daemon/tests/app_pages.rs`:

````diff
diff --git a/crates/daemon/tests/app_pages.rs b/crates/daemon/tests/app_pages.rs
--- a/crates/daemon/tests/app_pages.rs
+++ b/crates/daemon/tests/app_pages.rs
@@ -4,7 +4,7 @@ use statsbar_core::cpu::CoreTicks;
 use statsbar_core::ipc::Page;
 use statsbar_core::memory::MemInfo;
 use statsbar_core::schedule::ReaderId::{self, Battery, Cpu, Fan, Memory, Network};
-use statsbard::app::{App, READERS, reader_of};
+use statsbard::app::{App, READERS, page_of, reader_of};
 use statsbard::config::{Config, Items};
 use statsbard::summary;

@@ -221,3 +221,12 @@ fn items_without_a_page_follow_their_visibility_alone() {
     assert!(app.is_read(Battery));
     assert!(!app.is_read(Fan) && !app.is_read(Network));
 }
+
+#[test]
+fn only_cpu_and_memory_have_a_page() {
+    assert_eq!(page_of(Cpu), Some(Page::Cpu));
+    assert_eq!(page_of(Memory), Some(Page::Memory));
+    for id in [Battery, Fan, Network] {
+        assert_eq!(page_of(id), None, "{id:?}");
+    }
+}
````

Create `crates/daemon/tests/daemon_ipc.rs`:

````rust
//! The real binary: the event loop serving pages over the socket. It runs on a private bus with
//! a stand-in tray watcher, its own config and runtime directory, and reads this machine's
//! `/proc`. The desktop never sees it.

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use futures_lite::{StreamExt, future};
use statsbar_core::ipc::{CpuPage, Event, MemoryPage};
use zbus::fdo::{DBusProxy, NameOwnerChangedStream};
use zbus::zvariant::{OwnedValue, Value};

/// Generous for a local daemon; it only has to tell "slow" from "hung". Page events come 1 s
/// apart.
const LIMIT: Duration = Duration::from_secs(10);
/// Lines before the awaited one, and events before the awaited state.
const MAX_SKIPPED: usize = 16;
const WATCH_CPU: &str = "{\"type\":\"subscribe\",\"page\":\"cpu\"}\n";
const DBUSMENU: &str = "com.canonical.dbusmenu";
const WATCH_MEMORY: &str = "{\"type\":\"subscribe\",\"page\":\"memory\"}\n";
const REFRESH: &str = "{\"type\":\"refresh\"}\n";
const UNWATCH: &str = "{\"type\":\"unsubscribe\"}\n";
/// The daemon ignores a refresh sooner than this after the page's last reading.
const MIN_REFRESH: Duration = Duration::from_millis(500);

struct Watcher(mpsc::Sender<String>);

#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    fn register_status_notifier_item(&self, service: &str) {
        // The receiver lives until the test ends.
        let _ = self.0.send(service.to_owned());
    }

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        0
    }
}

/// Kills the process on drop, so a failed assertion never leaks one.
struct Reaped(Child);

impl Drop for Reaped {
    fn drop(&mut self) {
        // Either may fail only because the process has already exited, which is the goal.
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

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

fn wait_for(lines: &mpsc::Receiver<String>, wanted: &str) -> String {
    for _ in 0..MAX_SKIPPED {
        match lines.recv_timeout(LIMIT) {
            Ok(line) if line.contains(wanted) => return line,
            Ok(_) => {}
            Err(error) => panic!("waiting for {wanted:?}: {error}"),
        }
    }
    panic!("{wanted:?} never came");
}

/// Whether the daemon holds `path` open.
fn holds(daemon: &Reaped, path: &str) -> bool {
    fs::read_dir(format!("/proc/{}/fd", daemon.0.id()))
        .expect("running")
        .filter_map(Result::ok)
        // A descriptor may close between the listing and the read.
        .filter_map(|entry| fs::read_link(entry.path()).ok())
        .any(|target| target == Path::new(path))
}

/// The daemon's descriptors open on `path`, by number.
fn descriptors(daemon: &Reaped, path: &str) -> Vec<String> {
    let mut found: Vec<String> = fs::read_dir(format!("/proc/{}/fd", daemon.0.id()))
        .expect("running")
        .filter_map(Result::ok)
        // A descriptor may close between the listing and the read.
        .filter(|entry| fs::read_link(entry.path()).is_ok_and(|target| target == Path::new(path)))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    found.sort();
    found
}

/// The read calls the daemon has made so far, over all its threads.
fn reads(daemon: &Reaped) -> u64 {
    let io = fs::read_to_string(format!("/proc/{}/io", daemon.0.id())).expect("running");
    io.lines()
        .find_map(|line| line.strip_prefix("syscr: "))
        .expect("a syscr line")
        .parse()
        .expect("a count")
}

/// Whether the daemon scanned the processes since it had made `since` reads. A scan reads every
/// process's `statm`, so it makes at least one read per process, where a reading without one
/// makes under ten.
fn scanned(daemon: &Reaped, since: u64) -> bool {
    let processes = fs::read_dir("/proc")
        .expect("/proc")
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().parse::<u32>().is_ok())
        .count();
    reads(daemon) - since >= u64::try_from(processes).expect("fits")
}

/// Waits until the name followed by `changes` has no owner.
fn released(changes: &mut NameOwnerChangedStream) {
    async_io::block_on(future::or(
        async {
            for _ in 0..MAX_SKIPPED {
                let change = changes.next().await.expect("the bus stays up");
                if change.args().expect("arguments").new_owner().is_none() {
                    return;
                }
            }
            panic!("the name was never released");
        },
        async {
            async_io::Timer::after(LIMIT).await;
            panic!("waiting for the name's release: timed out");
        },
    ));
}

struct Session {
    _bus: Reaped,
    watcher: zbus::Connection,
    registered: mpsc::Receiver<String>,
    address: String,
    dir: PathBuf,
}

impl Session {
    /// `items` is the `[items]` table the daemon starts with.
    fn new(name: &str, items: &str) -> Session {
        // Short, for the socket path's 108-byte limit.
        let dir = std::env::temp_dir().join(format!("sbard-{}-{name}", std::process::id()));
        fs::create_dir(&dir).expect("create the session directory");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).expect("chmod");
        fs::create_dir_all(dir.join("config/statsbar")).expect("create the config directory");
        fs::write(dir.join("config/statsbar/config.toml"), items).expect("write the config");

        let mut bus = Reaped(
            Command::new("dbus-daemon")
                .args(["--session", "--nofork", "--print-address=1"])
                .stdout(Stdio::piped())
                .spawn()
                .expect("run /usr/bin/dbus-daemon (Ubuntu package dbus-daemon)"),
        );
        let address = wait_for(&lines(bus.0.stdout.take().expect("piped")), "unix:");
        let (sender, registered) = mpsc::channel();
        let watcher = async_io::block_on(async {
            zbus::connection::Builder::address(address.as_str())?
                .method_timeout(LIMIT)
                .name("org.kde.StatusNotifierWatcher")?
                .serve_at("/StatusNotifierWatcher", Watcher(sender))?
                .build()
                .await
        })
        .expect("serve the watcher");
        Session {
            _bus: bus,
            watcher,
            registered,
            address,
            dir,
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_statsbard"));
        command
            .env("DBUS_SESSION_BUS_ADDRESS", &self.address)
            // No system bus, so this machine's power profile cannot change the cadence.
            .env(
                "DBUS_SYSTEM_BUS_ADDRESS",
                format!("unix:path={}/none", self.dir.display()),
            )
            .env("XDG_CONFIG_HOME", self.dir.join("config"))
            .env("XDG_RUNTIME_DIR", &self.dir)
            .stderr(Stdio::piped());
        command
    }

    /// Starts the daemon and waits for its hub, by when the socket is listening. Returns the
    /// hub's bus name last.
    fn start(&self) -> (Reaped, mpsc::Receiver<String>, String) {
        let mut daemon = Reaped(self.command().spawn().expect("start statsbard"));
        let log = lines(daemon.0.stderr.take().expect("piped"));
        let hub = self
            .registered
            .recv_timeout(LIMIT)
            .expect("the hub registers");
        (daemon, log, hub)
    }

    /// Clicks the hub's menu row with this label, as the desktop does.
    fn click(&self, hub: &str, label: &str) {
        async_io::block_on(async {
            let reply = self
                .watcher
                .call_method(
                    Some(hub),
                    "/MenuBar",
                    Some(DBUSMENU),
                    "GetGroupProperties",
                    &(Vec::<i32>::new(), vec!["label"]),
                )
                .await
                .expect("list the menu");
            let rows: Vec<(i32, HashMap<String, OwnedValue>)> =
                reply.body().deserialize().expect("menu rows");
            let id = rows
                .iter()
                .find(|(_, properties)| {
                    properties
                        .get("label")
                        .and_then(|value| <&str>::try_from(value).ok())
                        == Some(label)
                })
                .expect("a row with the label")
                .0;
            self.watcher
                .call_method(
                    Some(hub),
                    "/MenuBar",
                    Some(DBUSMENU),
                    "Event",
                    &(id, "clicked", Value::from(0i32), 0u32),
                )
                .await
                .expect("click the row");
        });
    }

    /// Follows who owns `name`, from now on.
    fn owner_changes(&self, name: &str) -> NameOwnerChangedStream {
        async_io::block_on(async {
            DBusProxy::new(&self.watcher)
                .await?
                .receive_name_owner_changed_with_args(&[(0, name)])
                .await
        })
        .expect("follow the name")
    }

    fn panel(&self) -> Panel {
        let stream = UnixStream::connect(self.dir.join("statsbar.sock")).expect("connect");
        stream
            .set_read_timeout(Some(LIMIT))
            .expect("set the timeout");
        let mut panel = Panel {
            events: BufReader::new(stream.try_clone().expect("clone")),
            stream,
        };
        assert_eq!(panel.line(), "{\"protocol\":1}\n");
        panel.send("{\"protocol\":1}\n");
        panel
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // Only scratch files are left, and a failure here must not hide the test's own.
        let _ = fs::remove_dir_all(&self.dir);
    }
}

struct Panel {
    stream: UnixStream,
    events: BufReader<UnixStream>,
}

impl Panel {
    fn send(&mut self, text: &str) {
        self.stream.write_all(text.as_bytes()).expect("send");
    }

    fn line(&mut self) -> String {
        let mut line = String::new();
        self.events.read_line(&mut line).expect("read a line");
        line
    }

    fn event(&mut self) -> Event {
        serde_json::from_str(&self.line()).expect("an event")
    }

    fn cpu_until(&mut self, wanted: impl Fn(&CpuPage) -> bool) -> CpuPage {
        for _ in 0..MAX_SKIPPED {
            match self.event() {
                Event::Cpu(page) if wanted(&page) => return page,
                Event::Cpu(_) => {}
                other => panic!("not a CPU event: {other:?}"),
            }
        }
        panic!("the CPU page never got there");
    }

    /// Skips Memory events sent before the switch.
    fn cpu(&mut self) -> CpuPage {
        for _ in 0..MAX_SKIPPED {
            match self.event() {
                Event::Cpu(page) => return page,
                Event::Memory(_) => {}
                other => panic!("not a page: {other:?}"),
            }
        }
        panic!("no CPU event");
    }

    /// Skips CPU events sent before the switch.
    fn memory(&mut self) -> MemoryPage {
        for _ in 0..MAX_SKIPPED {
            match self.event() {
                Event::Memory(page) => return page,
                Event::Cpu(_) => {}
                other => panic!("not a page: {other:?}"),
            }
        }
        panic!("no memory event");
    }
}

const ALL_HIDDEN: &str =
    "[items]\ncpu = false\nmemory = false\nbattery = false\nfan = false\nnetwork = false\n";
const CPU_SHOWN: &str =
    "[items]\ncpu = true\nmemory = false\nbattery = false\nfan = false\nnetwork = false\n";

#[test]
fn pages_of_hidden_items_are_served_and_switched() {
    let session = Session::new("hidden", ALL_HIDDEN);
    let (daemon, _log, _hub) = session.start();
    let mut panel = session.panel();

    panel.send(WATCH_CPU);
    let first = panel.cpu_until(|_| true);
    assert_eq!(first.processes, None, "nothing to compare with yet");
    let page = panel.cpu_until(|page| page.load.is_some() && page.processes.is_some());
    let load = page.load.expect("just checked");
    assert!(!load.per_core_busy.is_empty());
    assert_eq!(page.core_temperatures_c.len(), load.per_core_busy.len());
    let processes = page.processes.expect("just checked");
    assert!((1..=6).contains(&processes.len()), "{processes:?}");
    assert!(page.uptime_seconds.is_some_and(|seconds| seconds > 0.0));
    assert!(!page.power_saver);

    panel.send(WATCH_MEMORY);
    let page = panel.memory();
    assert!(page.buckets.total > 0);
    assert!((1..=6).contains(&page.processes.expect("ranked at once").len()));
    // Only the Memory page follows the switch.
    assert!(matches!(panel.event(), Event::Memory(_)));

    let tasks = fs::read_dir(format!("/proc/{}/task", daemon.0.id())).expect("running");
    let mut threads: Vec<String> = tasks
        .map(|task| fs::read_to_string(task.expect("listed").path().join("comm")).expect("comm"))
        .map(|name| name.trim_end().to_owned())
        .filter(|name| !name.starts_with("blocking-"))
        .collect();
    threads.sort();
    assert_eq!(
        threads,
        ["async-io", "statsbard", "statsbard"],
        "the thread invariant"
    );

    // Back to CPU: the hidden item's page starts afresh, not from the page it showed before.
    panel.send(WATCH_CPU);
    let again = panel.cpu();
    assert_eq!((again.load, again.processes), (None, None));
}

#[test]
fn a_shown_items_page_is_sent_before_its_next_reading() {
    let session = Session::new("shown", CPU_SHOWN);
    let (_daemon, _log, _hub) = session.start();
    session
        .registered
        .recv_timeout(LIMIT)
        .expect("the CPU item registers");
    let mut first = session.panel();
    first.send(WATCH_CPU);
    let mut seen = vec![first.cpu_until(|page| page.load.is_some())];
    let mut second = session.panel();
    // Leaving at once, so only the welcome can reach the newcomer: the daemon answers each
    // request before it reads the next.
    second.send(&format!("{WATCH_CPU}{UNWATCH}"));
    let welcome = second.cpu_until(|_| true);
    assert!(welcome.load.is_some(), "the last load, at once");

    // The welcome is the page last published: the first watcher has it already, or gets it next
    // when a reading landed before the subscribe.
    let welcome_at = welcome.uptime_seconds.expect("uptime");
    while seen.last().and_then(|page| page.uptime_seconds) < Some(welcome_at) {
        assert!(seen.len() < MAX_SKIPPED, "the welcome was never published");
        seen.push(first.cpu_until(|_| true));
    }
    assert_eq!(
        seen.last(),
        Some(&welcome),
        "the welcome is a published page"
    );
    // Nothing was read for the newcomer: such a reading would reach the first watcher too, sooner
    // than a full interval after the page before it.
    seen.push(first.cpu_until(|_| true));
    for pair in seen.windows(2) {
        let gap = pair[1].uptime_seconds.expect("uptime") - pair[0].uptime_seconds.expect("uptime");
        assert!(gap >= 0.5, "a page {gap} s after the one before");
    }
}

#[test]
fn hiding_an_item_with_its_page_open_keeps_the_page_live() {
    let session = Session::new("hiding", CPU_SHOWN);
    let (daemon, _log, hub) = session.start();
    let item = session
        .registered
        .recv_timeout(LIMIT)
        .expect("the CPU item registers");
    let mut item_owner = session.owner_changes(&item);
    let mut panel = session.panel();
    panel.send(WATCH_CPU);
    panel.cpu_until(|page| page.load.is_some());
    let open = descriptors(&daemon, "/proc/stat");
    assert_eq!(open.len(), 1, "one reader: {open:?}");

    // Just after a reading, so the next one, which would reopen a closed file, is about a second
    // away. The item's name goes after the hide has dealt with the reader.
    session.click(&hub, "Show CPU");
    released(&mut item_owner);
    assert_eq!(
        descriptors(&daemon, "/proc/stat"),
        open,
        "hiding closed the page's reader"
    );
    for _ in 0..2 {
        let page = panel.cpu_until(|_| true);
        assert!(page.load.is_some(), "hiding dropped the page's baseline");
    }
    assert_eq!(
        descriptors(&daemon, "/proc/stat"),
        open,
        "the page's reader was reopened"
    );
}

#[test]
fn a_refresh_rescans_the_memory_page_once_half_a_second_has_passed() {
    let session = Session::new("refresh", ALL_HIDDEN);
    let (daemon, _log, _hub) = session.start();
    let mut panel = session.panel();
    panel.send(WATCH_MEMORY);
    // The first reading scans the processes. The next is due 1 s later and scans only 2 s later.
    panel.memory();
    let since = reads(&daemon);
    // Only time lets a refresh count, and nothing marks the moment. That reading came before
    // this point, so it is at least 500 ms old after the wait, with the next one ~500 ms away.
    std::thread::sleep(MIN_REFRESH);
    panel.send(REFRESH);
    let early = panel.memory();
    assert!(
        scanned(&daemon, since),
        "no early reading with a scan: the regular one 1 s on does not scan"
    );

    // Read a moment ago: the refresh is ignored, and the next reading, a regular one, does not
    // scan either.
    let since = reads(&daemon);
    panel.send(REFRESH);
    let next = panel.memory();
    assert!(
        !scanned(&daemon, since),
        "an ignored refresh brought the scan forward"
    );
    assert_eq!(
        next.processes, early.processes,
        "the refresh's ranking, kept"
    );
}

#[test]
fn a_hidden_items_files_close_with_its_last_watcher() {
    let session = Session::new("closing", ALL_HIDDEN);
    let (daemon, _log, _hub) = session.start();
    let mut first = session.panel();
    first.send(WATCH_CPU);
    first.cpu_until(|page| page.load.is_some());
    assert!(holds(&daemon, "/proc/stat"), "read while watched");
    assert!(holds(&daemon, "/proc/loadavg"), "read while watched");

    // Leaves by closing the connection. Memory is read only after the loop has caught up with
    // that, so its first event means CPU's files had their chance to close.
    drop(first);
    let mut second = session.panel();
    second.send(WATCH_MEMORY);
    second.memory();
    assert!(!holds(&daemon, "/proc/stat"), "read with nobody watching");
    assert!(
        !holds(&daemon, "/proc/loadavg"),
        "read with nobody watching"
    );
}

#[test]
fn a_second_daemon_exits_and_a_killed_one_can_be_replaced() {
    let session = Session::new("single", ALL_HIDDEN);
    let (first, _log, _hub) = session.start();

    let mut second = Reaped(session.command().spawn().expect("start a second statsbard"));
    let log = lines(second.0.stderr.take().expect("piped"));
    let line = wait_for(&log, "already running");
    assert_eq!(line, "statsbard: StatsBar is already running");
    assert!(!second.0.wait().expect("exits").success());
    assert!(
        session.registered.try_recv().is_err(),
        "it added no tray item"
    );
    drop(session.panel());

    drop(first);
    assert!(
        session.dir.join("statsbar.sock").exists(),
        "a killed daemon leaves its socket"
    );
    let (_third, _log, _hub) = session.start();
    drop(session.panel());
}

#[test]
fn without_a_runtime_directory_it_says_so_and_exits() {
    let session = Session::new("no-runtime", ALL_HIDDEN);
    let mut daemon = Reaped(
        session
            .command()
            .env_remove("XDG_RUNTIME_DIR")
            .spawn()
            .expect("start statsbard"),
    );
    let log = lines(daemon.0.stderr.take().expect("piped"));
    wait_for(&log, "XDG_RUNTIME_DIR");
    assert!(!daemon.0.wait().expect("exits").success());
    assert!(
        session.registered.try_recv().is_err(),
        "it added no tray item"
    );
}

#[test]
fn a_runtime_directory_others_can_read_is_refused() {
    let session = Session::new("shared", ALL_HIDDEN);
    fs::set_permissions(&session.dir, fs::Permissions::from_mode(0o750)).expect("chmod");
    let mut daemon = Reaped(session.command().spawn().expect("start statsbard"));
    let log = lines(daemon.0.stderr.take().expect("piped"));
    let line = wait_for(&log, "private");
    assert_eq!(
        line,
        format!(
            "statsbard: cannot open the panel's socket: {} is not a directory private to this user",
            session.dir.display()
        )
    );
    assert!(!daemon.0.wait().expect("exits").success());
    assert!(
        session.registered.try_recv().is_err(),
        "it added no tray item"
    );
    assert!(!session.dir.join("statsbar.sock").exists(), "nor a socket");
}
````

- [x] **Step 2: Run them, expect FAIL**

```bash
cargo test -p statsbard --test app_pages
cargo test -p statsbard --test daemon_ipc
```

Expected:

- `app_pages` does not compile (E0432 for `page_of`).
- `daemon_ipc` fails all 8: the current daemon opens no socket, refuses no second copy and
  ignores `XDG_RUNTIME_DIR`.

- [x] **Step 3: Implement**

Apply to `crates/daemon/src/app.rs`:

````diff
diff --git a/crates/daemon/src/app.rs b/crates/daemon/src/app.rs
--- a/crates/daemon/src/app.rs
+++ b/crates/daemon/src/app.rs
@@ -22,6 +22,9 @@ pub const READERS: [ReaderId; 5] = [
     ReaderId::Network,
 ];

+/// Every page the panel can open.
+pub const PAGES: [Page; 2] = [Page::Cpu, Page::Memory];
+
 /// A refresh sooner than this after the last read is ignored: the next read is under a second
 /// away anyway, and a load over a few milliseconds of ticks is noise.
 const MIN_REFRESH_NANOS: u64 = 500_000_000;
@@ -34,6 +37,11 @@ pub fn reader_of(page: Page) -> ReaderId {
     }
 }

+/// The page that shows a reader, if one does.
+pub fn page_of(id: ReaderId) -> Option<Page> {
+    PAGES.into_iter().find(|&page| reader_of(page) == id)
+}
+
 /// A click in the hub's menu, sent from the `ksni` thread to the main loop.
 #[derive(Debug, Clone, Copy, PartialEq, Eq)]
 pub enum Command {
````

Apply to `crates/daemon/src/run.rs`:

````diff
diff --git a/crates/daemon/src/run.rs b/crates/daemon/src/run.rs
--- a/crates/daemon/src/run.rs
+++ b/crates/daemon/src/run.rs
@@ -1,6 +1,7 @@
 //! The daemon's event loop, on the main thread. One timer serves every reader. Menu clicks
-//! arrive from the hub, and power-profile changes from the system bus. The single `zbus`
-//! connection the daemon owns is ticked here, not on a thread of its own (spike S2).
+//! arrive from the hub, power-profile changes from the system bus, and the panel's requests from
+//! the socket. The single `zbus` connection the daemon owns is ticked here, not on a thread of its
+//! own (spike S2).

 use std::error::Error;
 use std::future::Future;
@@ -11,6 +12,7 @@ use std::time::{Duration, Instant};
 use async_channel::{Receiver, Sender};
 use futures_lite::{StreamExt, future};
 use ksni::{Handle, TrayMethods};
+use statsbar_core::ipc::{Event, Page};
 use statsbar_core::schedule::ReaderId;
 use statsbar_readers::battery::BatteryReader;
 use statsbar_readers::cpu::CpuReader;
@@ -18,8 +20,10 @@ use statsbar_readers::fan::FanReader;
 use statsbar_readers::memory::MemoryReader;
 use statsbar_readers::network::NetworkReader;

-use crate::app::{App, Command, READERS, slot};
+use crate::app::{App, Command, PAGES, READERS, page_of, reader_of, slot};
 use crate::config::Items;
+use crate::ipc::{Change, Options, Server};
+use crate::pages::{Pages, System};
 use crate::power::{ProfileWatch, is_power_saver};
 use crate::tray::{Hub, Look, MetricItem, battery_look, network_look, percent_look};
 use crate::{autostart, config, paths, summary};
@@ -68,6 +72,23 @@ fn read_with<R, T>(
 pub async fn run() -> Result<(), BoxError> {
     let config_home = paths::config_home(|key| std::env::var_os(key))
         .ok_or("neither XDG_CONFIG_HOME nor HOME is an absolute path")?;
+    let runtime_dir = paths::runtime_dir(|key| std::env::var_os(key))
+        .ok_or("XDG_RUNTIME_DIR is not an absolute path")?;
+    let system_facts = System {
+        own_uid: rustix::process::geteuid().as_raw(),
+        page_size: u64::try_from(rustix::param::page_size())?,
+        ticks_per_second: rustix::param::clock_ticks_per_second(),
+    };
+    // Before any tray item: a second daemon must leave no trace.
+    let server = Server::bind(
+        &runtime_dir,
+        Options {
+            allowed_uid: system_facts.own_uid,
+            handshake_timeout: SETUP_TIMEOUT,
+        },
+    )?;
+    let root = PathBuf::from("/");
+    let pages = Pages::new(&root, system_facts);
     let config_file = paths::config_file(&config_home);
     let autostart_file = paths::autostart_file(&config_home);

@@ -85,8 +106,10 @@ pub async fn run() -> Result<(), BoxError> {
     let mut daemon = Daemon::start(
         app,
         sender,
+        server,
+        pages,
         Files {
-            root: PathBuf::from("/"),
+            root,
             config: config_file,
             config_writable: loaded.writable,
             autostart: autostart_file,
@@ -166,6 +189,7 @@ enum Wake {
     CommandsClosed,
     Profile(Result<String, BoxError>),
     ProfileEnded,
+    Panel(Change),
 }

 struct Daemon {
@@ -176,10 +200,27 @@ struct Daemon {
     /// Indexed by `app::slot`.
     items: [Option<Handle<MetricItem>>; 5],
     readers: Readers,
+    server: Server,
+    pages: Pages,
+    /// Each page as last published, indexed by `page_slot`, for a watcher that joins later.
+    published: [Option<Event>; 2],
+}
+
+fn page_slot(page: Page) -> usize {
+    match page {
+        Page::Cpu => 0,
+        Page::Memory => 1,
+    }
 }

 impl Daemon {
-    async fn start(app: App, commands: Sender<Command>, files: Files) -> Result<Daemon, BoxError> {
+    async fn start(
+        app: App,
+        commands: Sender<Command>,
+        server: Server,
+        pages: Pages,
+        files: Files,
+    ) -> Result<Daemon, BoxError> {
         let hub = Hub::new(commands, app.config.items.clone(), app.launch_at_login);
         let hub = bounded("hub item spawn", SETUP_TIMEOUT, hub.spawn()).await??;
         let mut daemon = Daemon {
@@ -189,6 +230,9 @@ impl Daemon {
             hub_shown: None,
             items: Default::default(),
             readers: Readers::default(),
+            server,
+            pages,
+            published: Default::default(),
         };
         for id in READERS {
             if daemon.app.is_visible(id) {
@@ -229,6 +273,7 @@ impl Daemon {
         let start = Instant::now();
         loop {
             let now = elapsed_nanos(start);
+            self.sync_pages();
             for id in self.app.due(now) {
                 self.sample(id, now).await;
             }
@@ -267,9 +312,11 @@ impl Daemon {
                 }
             };

-            // Bound first, so the three futures (and their borrows of `profiles`) are gone
-            // before the arms below may reset it.
-            let wake = future::or(timer, future::or(command, profile)).await;
+            let panel = async { Wake::Panel(self.server.next().await) };
+
+            // Bound first, so the four futures (and their borrows of `profiles` and the server)
+            // are gone before the arms below change either.
+            let wake = future::or(timer, future::or(command, future::or(profile, panel))).await;
             match wake {
                 Wake::Timer => {}
                 Wake::Command(Command::Quit) | Wake::CommandsClosed => break,
@@ -278,6 +325,7 @@ impl Daemon {
                 Wake::Profile(Err(error)) => {
                     eprintln!("statsbard: reading the power profile failed: {error}");
                 }
+                Wake::Panel(change) => self.handle_panel(change, elapsed_nanos(start)),
                 Wake::ProfileEnded => {
                     eprintln!("statsbard: power profile updates ended; keeping the last value");
                     profiles = None;
@@ -334,11 +382,80 @@ impl Daemon {
             }),
         };
         match result {
-            Ok(redraw) => self.refresh_item(id, redraw).await,
+            Ok(redraw) => {
+                self.refresh_item(id, redraw).await;
+                if let Some(page) = page_of(id) {
+                    self.publish(page, now);
+                }
+            }
             Err(error) => self.read_failed(id, now, &error),
         }
     }

+    /// Follows who watches each page. Run every pass, since a connection also ends when a write
+    /// to it fails.
+    fn sync_pages(&mut self) {
+        for page in PAGES {
+            let id = reader_of(page);
+            let open = self.server.watched(page);
+            self.app.set_page_open(id, open);
+            if !open {
+                self.pages.close(page);
+                self.published[page_slot(page)] = None;
+            }
+            if !self.app.is_read(id) {
+                self.readers.close(id);
+            }
+        }
+    }
+
+    fn handle_panel(&mut self, change: Change, now: u64) {
+        match change {
+            Change::Subscribed(page) => {
+                self.sync_pages();
+                // What is known already, so a shown item's page does not open empty. Nothing is
+                // read for it: a read here would be outside the cadence.
+                if let Some(event) = &self.published[page_slot(page)] {
+                    self.server.welcome(page, event);
+                }
+            }
+            Change::Refresh(page) => {
+                // An ignored refresh must not make the next reading a full scan either.
+                if self.app.refresh(reader_of(page), now) {
+                    self.pages.refresh(page);
+                }
+            }
+            Change::Unwatched => {}
+        }
+    }
+
+    /// Sends a page to its watchers. A page nobody watches costs nothing here.
+    fn publish(&mut self, page: Page, now: u64) {
+        if !self.server.watched(page) {
+            return;
+        }
+        let (event, problems) = match page {
+            Page::Cpu => {
+                let (snapshot, problems) =
+                    self.pages.cpu(now, self.app.cpu(), self.app.power_saver);
+                (Event::Cpu(snapshot), problems)
+            }
+            Page::Memory => {
+                // Nothing to show until the first reading, which is due at once.
+                let Some(&buckets) = self.app.memory() else {
+                    return;
+                };
+                let (snapshot, problems) = self.pages.memory(now, buckets);
+                (Event::Memory(snapshot), problems)
+            }
+        };
+        for (what, error) in problems {
+            eprintln!("statsbard: reading {what} for the {page:?} page failed: {error}");
+        }
+        self.server.publish(page, &event);
+        self.published[page_slot(page)] = Some(event);
+    }
+
     /// Closes the reader, so the next attempt reopens its files, and logs the first failure in a
     /// row.
     fn read_failed(&mut self, id: ReaderId, now: u64, error: &io::Error) {
@@ -429,9 +546,12 @@ impl Daemon {
         }
     }

-    /// Removes the item and closes its files, so a hidden metric costs nothing.
+    /// Removes the item and, unless its page is open, closes its files: a hidden metric nobody
+    /// watches costs nothing.
     async fn hide_item(&mut self, id: ReaderId) {
-        self.readers.close(id);
+        if !self.app.is_read(id) {
+            self.readers.close(id);
+        }
         if let Some(handle) = self.items[slot(id)].take() {
             shut_down(&handle, "metric item shutdown").await;
         }
````

- [x] **Step 4: Run, expect PASS**

```bash
cargo test -p statsbard
```

Expected: `app_pages` 16 and `daemon_ipc` 8 pass, with every other daemon test.

- [x] **Step 5: Checkpoint**

Ready to commit: the files listed above — "feat(daemon): publish the CPU and memory pages"

### Task 9: Verification & Docs

**Test level:** end-to-end on the laptop, plus the full gate. The doc edits are
`RED: n/a — no executable behavior, checked by a diff review`.

**CI:** `.github/workflows/ci.yml` runs `./scripts/check.sh`, so it runs every new test. It already
installs `dbus-daemon`. It does not run this task's end-to-end check, which the PR must say.

**Files:**

- Create: `docs/spikes/2026-11-22-04a-ipc-e2e.md`
- Modify: `README.md` (the status line and the plan list)
- Modify: `docs/plans/2026-11-16-rust-statsbar-roadmap.md` (the Plan 04a evidence line)

**Interfaces:** consumes the finished `statsbard`; produces no code.

- [x] **Step 1: Run the gate**

```bash
./scripts/check.sh
cargo audit
cargo build --release -p statsbard
cargo tree -p statsbard -e normal
```

Expected: exit 0 with 341 passed, 0 failed, 1 ignored and `core purity: ok`; no vulnerability and
no advisory beyond the three known "unmaintained" warnings; no `gtk`, `gdk` or `tokio` crate in
the tree.

- [x] **Step 2: Repeat the boundary tests**

```bash
(for i in $(seq 1 25); do cargo test -q -p statsbard --test ipc --test daemon_ipc || exit 1; done)
```

Expected: 25 rounds with no failure. A failure is a bug (`bugfix-strict`), not a retry.

- [x] **Step 3: Start the real daemon**

Note `powerprofilesctl get` so that Step 6 can restore it. If a `statsbard` is running, quit it
from its hub over D-Bus and start it again in Step 6. Use a scratch config, so the user's is never
written:

```bash
CONFIG="$(mktemp -d)"
XDG_CONFIG_HOME="$CONFIG" ./target/release/statsbard &
DAEMON=$!
```

Save this client outside the repository as `page-client.py`. It prints arrival times, so the
cadence can be read off it. Each extra `N:S` argument sends a Refresh S seconds after page N.

```python
import json
import os
import socket
import sys
import time

page, seconds = sys.argv[1], float(sys.argv[2])
refreshes = {int(n): float(s) for n, s in (item.split(":") for item in sys.argv[3:])}
client = socket.socket(socket.AF_UNIX)
client.settimeout(10)
client.connect(os.path.join(os.environ["XDG_RUNTIME_DIR"], "statsbar.sock"))
for message in ({"protocol": 1}, {"type": "subscribe", "page": page}):
    client.sendall(json.dumps(message).encode() + b"\n")
start = time.monotonic()
for count, line in enumerate(client.makefile("rb"), start=1):
    elapsed = time.monotonic() - start
    print(f"{elapsed:6.2f} {line.decode().rstrip()}", flush=True)
    if elapsed >= seconds:
        break
    if count in refreshes:
        time.sleep(refreshes[count])
        print(f"{time.monotonic() - start:6.2f} refresh", flush=True)
        client.sendall(b'{"type":"refresh"}\n')
```

- [x] **Step 4: Check each row and keep the observed values**

Click over dbusmenu with `busctl --user` (`GetLayout` for the id, then `Event`) and switch profiles
with `powerprofilesctl set`. Do not ask the user to click.

| Check | How | Expected |
|---|---|---|
| CPU page | `python3 -I page-client.py cpu 15` beside `top -b -n 3 -d 1` | One event a second; load and the busiest processes within one sample of `top` |
| Memory page | `python3 -I page-client.py memory 15` beside `free -b` | `MemTotal − MemAvailable` within one sample of `free`; six processes, heaviest first |
| CPU details | The CPU client beside `sensors` (or the coretemp `temp*_input` files), the mean of `scaling_cur_freq`, `/proc/loadavg` and `/proc/uptime` | Package and core temperatures within 2 °C, frequency within one sample's swing, load ÷ online CPUs, uptime within a second; Thermal matches requirement 6 |
| Pressure | The Memory client beside `cat /proc/pressure/memory` | The level Task 2 gives for the file's `avg10` values: Critical at `full` ≥ 10, else Warning at `some` ≥ 10 |
| Welcome | With CPU shown, start a second `page-client.py cpu 5` while one runs | The second client's first line comes at about 0.00 s, with a load |
| Refresh, taken | `python3 -I page-client.py cpu 6 3:0.6` | A page about 0.6 s after page 3 instead of 1 s, then one a second again |
| Refresh, ignored | `python3 -I page-client.py cpu 6 3:0` | No page sooner than 1 s after page 3 |
| Hide while watched | Untick *Show CPU* during a CPU client | The item goes, events continue each second, `ls -l /proc/$DAEMON/fd` still lists `/proc/stat` |
| Show while watched | Tick it again | The item is back with a label within a second |
| Last watcher leaves | Untick *Show CPU* again during a CPU client, then end the client | `/proc/stat`, the coretemp and the cpufreq files are gone from the fd list |
| Power saver, open | Set `power-saver` during a client | Still one event a second, with `"power_saver":true` |
| Power saver, closed | `strace -f -tt -e trace=pread64,read -p $DAEMON` with no client | Reads 5 s apart; if ptrace is refused, sample `/proc/$DAEMON/io` `syscr` each second instead |
| Socket | `stat -c '%a %U' "$XDG_RUNTIME_DIR/statsbar.sock"` | `1600` (600 plus the sticky bit) and the user |
| Second daemon | Start the release binary again | Non-zero exit, `StatsBar is already running`, the first still serves |
| Threads | `ps -T -p $DAEMON` | Only the invariant's threads |
| Memory | `grep Pss /proc/$DAEMON/smaps_rollup`, idle and watched | Recorded, against Plan 03b's 2789 kB idle |
| Quit | The hub's Quit over dbusmenu | Exit 0 and no socket left |

- [x] **Step 5: Write the docs**

Create `docs/spikes/2026-11-22-04a-ipc-e2e.md` in the form of
`docs/spikes/2026-11-21-03b-text-labels-e2e.md`: the date, system and tree, one Results row per row
above with the observed value, the gate's results, the commands, and what was not seen. Record
counts and figures only: no process name and no pid of another program.

In `README.md`, replace the Status line with:

```text
Status: CPU, Memory, Battery, Fan and Network in the tray, and the CPU and Memory pages' data on a
private socket for the coming panel (statsbard); run with `cargo run --release -p statsbard`.
```

and add after the Plan 03b entry:

```text
- [Plan 04a — daemon IPC](docs/plans/2026-11-22-rust-statsbar-04a-ipc.md): the panel's protocol
  and socket, with process lists and CPU details;
  [end-to-end run](docs/spikes/2026-11-22-04a-ipc-e2e.md).
```

In the roadmap, replace Plan 04a's `- **Evidence:**` bullet with a link to that run and one
sentence of its observed results, as the earlier plans do.

- [x] **Step 6: Restore the machine**

Set the saved power profile, confirm it with `powerprofilesctl get`, and start the user's earlier
daemon again if there was one. The scratch config directory may stay in the temporary directory.

- [ ] **Step 7: Review and final gate**

1. Read the full diff, untracked files included.
2. Run `code-review-strict` on the branch, and ask it for a security look at `ipc.rs` and
   `paths::runtime_dir`: peer uid, socket mode, the lock, symlinks, bounds, logging.
3. Address or report every finding. If anything changed, repeat from Step 1.
4. Report what stayed unverified: the 1.85 and 1.87 floors, autostart through a real login, and
   any row of Step 4 that was not seen.

- [x] **Step 8: Checkpoint**

Ready to commit: `docs/spikes/2026-11-22-04a-ipc-e2e.md`, `README.md`, the roadmap, this plan — "docs: record the IPC end-to-end run"
