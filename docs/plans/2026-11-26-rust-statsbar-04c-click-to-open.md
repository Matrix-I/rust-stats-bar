# Plan 04c — Left Click Opens the Panel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: subagent-driven-strict or executing-plans-strict. Tasks are `### Task N: <name>`; steps use `- [ ]`.

**Goal:** a left click on the CPU or Memory tray icon opens the panel on that page, exactly as
"Open details…" in that icon's menu does. Today `MetricItem` keeps `ksni`'s default
`Tray::activate`, which does nothing, so a left click on GNOME does nothing at all.

**Architecture:** `MetricItem` implements `ksni::Tray::activate`: when `page_of(self.reader)` is
`Some(page)`, it sends `Command::OpenPage(page)` through the existing non-blocking `send`. The
main loop's `Command::OpenPage` branch (`crates/daemon/src/run.rs:531`) and `launch::Launcher`
are reused unchanged. `MENU_ON_ACTIVATE` stays `false` (the default): with it `true`, `ksni`
answers `Activate` with `UnknownMethod` so the shell opens the menu instead, which is the opposite
of the goal. No ADR: no alternative design inside existing boundaries.

**Tech Stack:** Rust edition 2024, `rustc 1.100.0`, vendored `ksni` 0.3.7 (`vendor/ksni`, its
`Tray::activate(&mut self, x: i32, y: i32)` at `vendor/ksni/src/lib.rs:103`), unchanged.

**Branch:** `feature/panel-skeleton` (the branch Plan 04b is on; the user asked for this on it).

## Confirmed Requirements

1. Left click on the CPU or Memory icon opens the panel on that icon's page (user, 2026-11-26:
   "Có, thêm vào", answering "Thêm click trái vào icon để mở panel không?").

## Decision Log

- Left click opens the panel on CPU and Memory → not posted: no ticket; this project tracks
  decisions in its plans. PR line: "A left click on the CPU or Memory icon opens its page."

## Scope Map

Invariant: an activation of the CPU or Memory item reaches the main loop as
`Command::OpenPage(<its page>)`, and nothing else does.

| Path | Decision |
|---|---|
| `MetricItem::activate` for CPU and Memory | in (Task 1) |
| `MetricItem::activate` for Battery, Fan, Network (no page) | in (Task 1): sends nothing, as today |
| A full command queue on activate | in (Task 1): dropped and logged by `send`, like a menu click |
| `Hub` left click | out (follow-up: no request; stays a no-op) |
| `secondary_activate` (middle click) | out (follow-up: no request; stays a no-op) |
| Right click / menu, including "Open details…" | unchanged: menu code untouched; existing tests in `crates/daemon/tests/tray.rs` pin it |
| Left click while a panel is open | unchanged: `Launcher::open` path as for the menu (second launch switches the page, Plan 04b e2e "Second launch") |

## Impact Map

None: no shared symbol's signature or behavior changes. `send`, `Command::OpenPage`, `page_of`
and `Launcher` gain one caller each with the same arguments the menu already passes.

## Existing Data & States

none: no persisted data or format changes; config, socket protocol and menus are untouched.

## Global Constraints

- Project rules: none beyond `README.md`; global rules in `/home/thang-tran/.claude/CLAUDE.md`.
- Verification: `./scripts/check.sh` (fmt, clippy `-D warnings`, `cargo test --workspace`, core
  purity).
- Commits: only when the user asks; tasks end with "Ready to commit".
- `activate` runs on the `ksni` thread: it must not block (use `tray::send`, never `send_blocking`).

## Technical Defaults

- Reuse `tray::send` for the drop-and-log behavior — same thread and queue as the menu click.
- Unit test on the `Tray` method, as the existing `open_details_sends_its_page_*` tests do; the
  D-Bus `Activate` → `Tray::activate` wiring is `ksni`'s and is covered by the end-to-end check.

## Risks & Rollback

- A shell that opens the menu on left click no longer does so for CPU and Memory once `Activate`
  succeeds → right click still opens it; checked on GNOME in Task 2 → revert the one method.
- No persisted data, API or format change; rollback is reverting the commit.

### Task 1: Left click on CPU and Memory sends `OpenPage`

**Type:** feature · **Test level:** unit
**Files:** `crates/daemon/src/tray.rs` (the `impl ksni::Tray for MetricItem` block, about lines
140–192), `crates/daemon/tests/tray.rs` (append after `a_full_command_queue_drops_open_details_too`,
about line 372)
**Interfaces:** Consumes `page_of(ReaderId) -> Option<Page>`, `send(&Sender<Command>, Command)`.
Produces `impl ksni::Tray for MetricItem { fn activate(&mut self, _x: i32, _y: i32) }`.

- [x] Write the failing tests, appended to `crates/daemon/tests/tray.rs`:

```rust
#[test]
fn a_left_click_opens_the_page_of_cpu_and_memory_only() {
    for (reader, page) in [
        (ReaderId::Cpu, statsbar_core::ipc::Page::Cpu),
        (ReaderId::Memory, statsbar_core::ipc::Page::Memory),
    ] {
        let (mut item, receiver) = item(reader);
        item.activate(10, 20);
        assert_eq!(receiver.try_recv(), Ok(Command::OpenPage(page)), "{reader:?}");
        assert!(receiver.try_recv().is_err(), "one command per click");
    }
    for reader in [ReaderId::Battery, ReaderId::Fan, ReaderId::Network] {
        let (mut item, receiver) = item(reader);
        item.activate(10, 20);
        assert!(receiver.try_recv().is_err(), "{reader:?} sends nothing");
    }
}

#[test]
fn a_full_command_queue_drops_a_left_click() {
    let (mut item, receiver) = item(ReaderId::Memory);
    item.activate(0, 0);
    item.activate(0, 0);
    assert_eq!(
        receiver.try_recv(),
        Ok(Command::OpenPage(statsbar_core::ipc::Page::Memory))
    );
    assert!(receiver.try_recv().is_err());
}
```

- [x] Run `cargo test -p statsbard --test tray left_click`; expect FAIL:
  `a_left_click_opens_the_page_of_cpu_and_memory_only` panics at the first `assert_eq!` with
  `left: Err(Empty)` / `right: Ok(OpenPage(Cpu))`, because the default `activate` sends nothing.
  (`a_full_command_queue_drops_a_left_click` fails the same way.)
- [x] Implement in `crates/daemon/src/tray.rs`, inside `impl ksni::Tray for MetricItem`, after
  `x_ayatana_label_guide`:

```rust
    /// A left click opens the same page as "Open details…"; items without a page do nothing.
    fn activate(&mut self, _x: i32, _y: i32) {
        if let Some(page) = page_of(self.reader) {
            send(&self.commands, Command::OpenPage(page));
        }
    }
```

  and update the module doc's third sentence (`crates/daemon/src/tray.rs:3`) to
  "Menu clicks and left clicks become `Command`s for the main loop; nothing here reads or writes
  files."
- [x] Run `cargo test -p statsbard --test tray`; expect PASS, all tests including the two new ones.
- [x] Ready to commit: `crates/daemon/src/tray.rs crates/daemon/tests/tray.rs` —
  "feat(daemon): open the panel on a left click on the CPU and Memory icons" — review Minor fixed in round 1: `send` logs "click dropped" (was "menu click dropped"), user-approved

### Task 2: Verification & Docs

**Type:** docs · **Test level:** e2e
**Files:** `README.md` (lines 6–8), this plan.

- [x] Update `README.md` line 7 to: "CPU and Memory pages (statsbar-panel), opened by a left
  click on their icons or from "Open details…" in their menus; run with". RED: n/a — prose, checked
  by reading the rendered diff.
- [x] Run `./scripts/check.sh`; expect exit 0 (quote the test totals).
- [x] Run `cargo build --workspace --release --locked`; expect success.
- [x] End-to-end on the laptop (GNOME, Wayland): stop the running `statsbard`, start the new
  release build detached (`setsid nohup target/release/statsbard >"$LOG" 2>&1 </dev/null &`, `LOG`
  in the scratchpad). Then, with `D` its pid:
  1. `gdbus call --session -d org.kde.StatusNotifierItem-$D-2 -o /StatusNotifierItem -m org.kde.StatusNotifierItem.Activate 0 0`
     → reply `()`; within 3 s one `statsbar-panel --page cpu` whose parent is `$D`; AT-SPI shows
     the CPU page labels (`Usage`, `Temperature`, `Cores`).
  2. Same call on `-3` (Memory) while the panel is open → the panel count returns to 1 within
     2 s and AT-SPI shows the Memory page.
  3. Same call on the Battery item (`-4`) → `()`, no new `statsbar-panel`.
  4. Right click still opens the menu: `com.canonical.dbusmenu.GetLayout` on `-2` still lists
     "Open details…" first. A physical left and right click on GNOME is the user's to confirm
     (report as not self-verified if they do not).
  Expected evidence: the commands' output and the process listings.
- [x] Note for the PR: CI runs `./scripts/check.sh`, which runs the new unit tests; the
  end-to-end check is local only.
- [x] Ready to commit: `README.md docs/plans/2026-11-26-rust-statsbar-04c-click-to-open.md` —
  "docs: say a left click opens the panel"
