# rust-statsbar — Plan 02a: Tray Spikes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: development-superpowers:subagent-driven-development or
> executing-plans-strict. Steps use `- [ ]`.

**Goal:** settle two facts every later daemon plan depends on, with measurements from this machine:
**S1** — does a wide "icon + 37%" pixmap stay wide and legible in the GNOME panel, in the light and the
dark style; **S2** — how many threads and how much memory do tray items plus one `zbus` connection
driven from the main thread cost. The results are recorded in `docs/spikes/` and folded into the
roadmap, so Plan 02b (config, scheduler, worker, CPU and Memory tray items, hub) is written against
facts instead of fallbacks.

**Architecture:** one throwaway crate, `spikes/tray`, outside the Cargo workspace (its own `[workspace]`
table), so `./scripts/check.sh` never builds it; CI still starts on a spike-only change because its
path filters match `**/*.rs`, and then checks only the workspace. It has two binaries: `label` (S1) and `threads`
(S2), and a `measure.sh` that prints a process's threads and memory. Nothing in `crates/` changes. See
`docs/plans/2026-11-16-rust-statsbar-roadmap.md` for how the spikes fit the whole.

**Tech Stack:** Rust 1.99.1, edition 2024 · `ksni` 0.3.7 (`async-io` feature, default features off) ·
`zbus` 5.19.0 (`async-io` feature, default features off) · `async-io` 2.6.0 · `futures-lite` 2.6.1.
All four were fetched from crates.io on 2026-11-17 and the code below compiled, passed
`cargo clippy -- -D warnings` and `cargo fmt --check` against them, and `threads 1` ran for 7 s.

**Branch:** `feature/tray-spikes`, from `origin/main`.

## Confirmed Requirements

All answered by the user on 2026-11-17 unless marked otherwise.

1. Plan 02 is split: 02a holds only the two spikes and records their results; 02b is written after
   them, against the results. *(user)*
2. New dependencies approved: `ksni` 0.3.7, `zbus`, and `async-io` / `futures-lite` to drive the
   executor. *(user)* Any other direct dependency needs a new question.
3. `zbus` is pinned at **5.19.0**, not the roadmap's 6.3.0, because `ksni` 0.3.7 depends on `zbus` 5
   and two copies of `zbus` would be linked otherwise. *(user)* Task 3, Step 1 corrects the roadmap.
4. Tray label design under test: a small glyph followed by the percentage, right-aligned in a field
   as wide as `"100%"`, as upstream's `symbolPercentMenuBarImage` does, so the item keeps one width
   from 7 % to 100 %. *(user)*
5. Spike code is kept and committed in `spikes/`, outside the workspace; results go to
   `docs/spikes/`. *(user)*
6. From upstream, for reference by 02b and not built here: the hub item is never hideable, every metric
   item is shown by default, and launch at login is off until switched on. *(upstream
   `ControlCenterView.swift`, `LoginItem.swift`)*

## Global Constraints

From the user's global rules and the roadmap; a project `CLAUDE.md`, once the repo has one, wins.

- **Test-first does not apply to this plan's code, and that is reported, not hidden.** The spike crate
  is throwaway, lives outside the workspace and is never linked into a product; its outcome is a
  person looking at the GNOME panel and a process's thread list, which no automated test can assert.
  The one pure piece, `render`, could be tested, but it is a sketch that Plan 02b replaces with the
  real renderer in `crates/core`, test first, from what S1 settles; `label` prints the size of every
  pixmap it offers, so the results record observed sizes rather than computed ones.
- Never weaken, skip or delete a test to make it pass. `./scripts/check.sh` must stay green on the
  branch, proving the workspace is untouched.
- Commit only when asked: each task ends with `Ready to commit`, and nothing runs `git commit`.
  Conventional Commits, imperative mood, one logical change per commit.
- Code, comments, names and docs in English; comments explain why.
- Every external call has a timeout: every session-bus call in the spike (connecting, registering a
  tray item, creating a proxy) goes through `bounded` with a 5 s limit, and the per-tick `GetId` call
  is raced against a 1 s timer. The implemented binaries also bound the `ksni` update wait because it
  waits on the tray service's D-Bus update.
- New dependency: exact name checked on crates.io, licence and maintenance checked, versions pinned
  with `=`, `Cargo.lock` committed, audit run (Task 1).
- Steps that change what is on the user's screen (tray items appearing, the light/dark style) run
  with the user present; the user switches the style, not the executor.
- Final gate after the last edit: read the full diff; run `./scripts/check.sh`; self-review;
  `code-review-strict`; repeat if anything changed.

## Technical Defaults

- `spikes/tray/Cargo.toml` carries an empty `[workspace]` table — keeps it out of the root workspace
  without editing the root `Cargo.toml`, and Cargo would otherwise refuse to build a package nested
  under a workspace that does not list it.
- `spikes/tray/Cargo.lock` is committed — the spike can be re-run later (after a GNOME or extension
  update) with the same dependency versions.
- The spike's release profile copies the workspace's — S2's memory figures are then the ones the
  daemon's release build would show.
- Hand-drawn 5×7 font for `0–9` and `%`, and 7×7 glyphs for CPU and memory, as string tables — no font
  file, so no font licence and no font stack in the process; S1 tests exactly what 02b would ship.
- Pixmap data is ARGB32 in network byte order, `[A, R, G, B]` per pixel — what `ksni::Icon` documents.
- Memory is reported as `Pss` from `/proc/<pid>/smaps_rollup` — the roadmap's budget is stated in Pss.
- Screenshots are not committed — they show the user's desktop. The results doc describes them.

## Risks & Rollback

| Risk | Mitigation | Revert |
|---|---|---|
| The spike's tray items stay in the panel if the process is killed uncleanly | The watcher drops the items when their D-Bus connections close. S1 has a quit menu and the S2 measurement loop terminates the process on exit | `pkill -x label` / `pkill -x threads` |
| The user's appearance setting is left on the light style after S1 | The user switches it themselves and back again in Task 1, Step 7 | Settings → Appearance → Dark |
| `ksni` with `async-io` always starts its own driver thread, so the roadmap's "tray on the main loop" assumption is already known to be false (read in `ksni-0.3.7/src/compat.rs`, `kick_driver`) | S2 measures what is actually there; Task 3, Step 3 rewrites the invariant only after the user approves the new wording | — |
| A dependency has an advisory | Task 1 runs the audit; an advisory is reported before continuing | Remove `spikes/tray` |
| `cargo-audit` is not installed on this machine (checked 2026-11-17) | Task 1 asks before installing it; if declined, the audit is reported as not run | — |

No persisted data, public API, message format or shipped behaviour changes: the plan adds a
standalone crate and documents. Rollback is deleting `spikes/` and `docs/spikes/`.

---

## Task 1: Spike crate and S1 — tray label pixmap

**Files:**
- Create: `spikes/tray/Cargo.toml`, `spikes/tray/Cargo.lock` (generated), `spikes/tray/.gitignore`,
  `spikes/tray/src/lib.rs`, `spikes/tray/src/bin/label.rs`,
  `docs/spikes/2026-11-17-s1-tray-label.md`

**Interfaces:**
- Produces: `statsbar_tray_spike::{Glyph, render}` —
  `pub fn render(glyph: Glyph, text: &str, dot: usize, height: usize, argb: [u8; 4]) -> ksni::Icon`;
  `pub const SETUP_TIMEOUT: Duration` (5 s) and
  `pub async fn bounded<T>(what: &str, timeout: Duration, call: impl Future<Output = T>) -> Result<T, Box<dyn std::error::Error>>`;
  binary `spikes/tray/target/release/label` (five tray items, A–E); the S1 results document.
- Consumes: `ksni::Tray` (`id`, `title`, `tool_tip`, `icon_pixmap`, `menu`), `ksni::TrayMethods::spawn`,
  `ksni::Handle::update`, `async_io::block_on`, `async_io::Timer::interval`.

- [x] **Step 1: Create the branch**

```bash
git fetch origin
git switch -c feature/tray-spikes origin/main
```

This plan file was written on `main` as an untracked file and moves to the branch with the switch.
`Ready to commit: docs/plans/2026-11-17-rust-statsbar-02a-spikes.md — "docs: add plan 02a for the tray
spikes"` — a commit of its own, before anything else.

- [x] **Step 2: Check the dependencies before adding them**

Run each and read the `license` and `repository` lines:

```bash
cargo info ksni@0.3.7
cargo info zbus@5.19.0
cargo info async-io@2.6.0
cargo info futures-lite@2.6.1
```

Expected: `ksni` — Unlicense, `github.com/iovxw/ksni`; `zbus` — MIT, `github.com/z-galaxy/zbus`;
`async-io` and `futures-lite` — `Apache-2.0 OR MIT`, `github.com/smol-rs/…`. A different name, licence
or repository is a stop: report it to the user.

Then check maintenance — the release date of each pinned version and of each crate's newest release
(for `zbus` the newest is 6.x; the 5.x line in use is judged by 5.19.0's own date):

```bash
for spec in ksni/0.3.7 zbus/5.19.0 async-io/2.6.0 futures-lite/2.6.1; do
    crate="${spec%/*}"
    echo "$spec"
    curl -fsS --max-time 10 -A 'crate-check' "https://crates.io/api/v1/crates/$spec" \
        | grep -oE '"created_at":"[^"]*"' | head -1
    curl -fsS --max-time 10 -A 'crate-check' \
        "https://crates.io/api/v1/crates/$crate/versions?per_page=1&sort=date" \
        | grep -oE '"(num|created_at)":"[^"]*"' | head -2
done
```

Expected: each crate's newest release is within the last 12 months (on 2026-11-17: `ksni` 0.3.7 of
2026-10-20; `zbus` 5.19.0 of 2026-08-09, newest 6.3.0 of 2026-11-13). A `curl` error is a failed
check: retry once, then report it. An older release is not a stop, but goes into the S1 results
document under **Dependencies**, and into the final report.

- [x] **Step 3: Confirm the crate does not exist yet**

Run: `cargo build --release --manifest-path spikes/tray/Cargo.toml`
Expected: FAIL — ``error: manifest path `spikes/tray/Cargo.toml` does not exist``.

- [x] **Step 4: Write the crate**

`spikes/tray/.gitignore`:

```gitignore
/target
```

`spikes/tray/Cargo.toml`:

```toml
# Throwaway spike for Plan 02a. Its own [workspace] keeps it out of the main workspace, so
# scripts/check.sh and CI never build it.
[workspace]

[package]
name = "statsbar-tray-spike"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
ksni = { version = "=0.3.7", default-features = false, features = ["async-io"] }
zbus = { version = "=5.19.0", default-features = false, features = ["async-io"] }
async-io = "=2.6.0"
futures-lite = "=2.6.1"

[profile.release]
opt-level = "s"
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true
```

`spikes/tray/src/lib.rs`:

```rust
//! Spike S1's label drawing: an icon and a percentage in a hand-made 5×7 bitmap font, drawn
//! into an ARGB32 pixmap. Throwaway: Plan 02b writes the real renderer in `crates/core`, test
//! first, with whatever S1 settles on.

use std::future::Future;
use std::time::Duration;

/// How long a setup call to the session bus (connecting, registering a tray item with the
/// watcher) may take. Generous: it only has to separate "slow" from "hung".
pub const SETUP_TIMEOUT: Duration = Duration::from_secs(5);

/// Runs `call`, giving up after `timeout`, so a hung bus or watcher ends the spike with a
/// message naming the call instead of hanging it silently.
pub async fn bounded<T>(
    what: &str,
    timeout: Duration,
    call: impl Future<Output = T>,
) -> Result<T, Box<dyn std::error::Error>> {
    futures_lite::future::or(async { Ok(call.await) }, async {
        async_io::Timer::after(timeout).await;
        Err(format!("{what} timed out after {timeout:?}").into())
    })
    .await
}

/// A 7×7 glyph drawn before the number, as upstream draws an SF Symbol there.
#[derive(Clone, Copy)]
pub enum Glyph {
    Cpu,
    Memory,
}

const CPU: [&str; 7] = [
    " # # # ", " ##### ", "##   ##", " # # # ", "##   ##", " ##### ", " # # # ",
];
const MEMORY: [&str; 7] = [
    "       ", "#######", "# # # #", "# # # #", "#######", "# # # #", "       ",
];

fn char_rows(c: char) -> Option<[&'static str; 7]> {
    Some(match c {
        '0' => [
            " ### ", "#   #", "#  ##", "# # #", "##  #", "#   #", " ### ",
        ],
        '1' => [
            "  #  ", " ##  ", "  #  ", "  #  ", "  #  ", "  #  ", " ### ",
        ],
        '2' => [
            " ### ", "#   #", "    #", "   # ", "  #  ", " #   ", "#####",
        ],
        '3' => [
            "#####", "   # ", "  #  ", "   # ", "    #", "#   #", " ### ",
        ],
        '4' => [
            "   # ", "  ## ", " # # ", "#  # ", "#####", "   # ", "   # ",
        ],
        '5' => [
            "#####", "#    ", "#### ", "    #", "    #", "#   #", " ### ",
        ],
        '6' => [
            "  ## ", " #   ", "#    ", "#### ", "#   #", "#   #", " ### ",
        ],
        '7' => [
            "#####", "    #", "   # ", "  #  ", " #   ", " #   ", " #   ",
        ],
        '8' => [
            " ### ", "#   #", "#   #", " ### ", "#   #", "#   #", " ### ",
        ],
        '9' => [
            " ### ", "#   #", "#   #", " ####", "    #", "   # ", " ##  ",
        ],
        '%' => [
            "##   ", "##  #", "   # ", "  #  ", " #   ", "#  ##", "   ##",
        ],
        _ => return None,
    })
}

const ROWS: usize = 7;
const CHAR_COLS: usize = 5;
const GLYPH_COLS: usize = 7;
/// Blank columns between the glyph and the number field.
const GLYPH_GAP: usize = 2;
/// The number sits in a field as wide as "100%", right-aligned, as upstream does, so the item
/// keeps one width from 7% to 100%.
const WIDEST: &str = "100%";

fn text_cols(text: &str) -> usize {
    let n = text.chars().count();
    if n == 0 { 0 } else { n * CHAR_COLS + (n - 1) }
}

/// Draws `glyph` and `text` with each font dot `dot`×`dot` pixels, centred vertically in a
/// pixmap `height` pixels tall. `argb` is the colour of a lit dot; unlit pixels are transparent.
pub fn render(glyph: Glyph, text: &str, dot: usize, height: usize, argb: [u8; 4]) -> ksni::Icon {
    let field = text_cols(WIDEST).max(text_cols(text));
    let cols = GLYPH_COLS + GLYPH_GAP + field;
    let width = cols * dot;
    let height = height.max(ROWS * dot);
    let top = (height - ROWS * dot) / 2;
    let mut data = vec![0u8; width * height * 4];
    let mut lit = |col: usize, row: usize| {
        for y in 0..dot {
            for x in 0..dot {
                let offset = ((top + row * dot + y) * width + col * dot + x) * 4;
                data[offset..offset + 4].copy_from_slice(&argb);
            }
        }
    };
    let glyph_rows = match glyph {
        Glyph::Cpu => CPU,
        Glyph::Memory => MEMORY,
    };
    for (row, line) in glyph_rows.iter().enumerate() {
        for (col, cell) in line.chars().enumerate() {
            if cell == '#' {
                lit(col, row);
            }
        }
    }
    let mut left = GLYPH_COLS + GLYPH_GAP + field - text_cols(text);
    for c in text.chars() {
        if let Some(rows) = char_rows(c) {
            for (row, line) in rows.iter().enumerate() {
                for (col, cell) in line.chars().enumerate() {
                    if cell == '#' {
                        lit(left + col, row);
                    }
                }
            }
        }
        left += CHAR_COLS + 1;
    }
    ksni::Icon {
        width: width as i32,
        height: height as i32,
        data,
    }
}
```

`spikes/tray/src/bin/label.rs`:

```rust
//! Spike S1: does a wide pixmap stay wide and legible in the GNOME panel?
//!
//! Shows five items side by side. Look at each in the panel, with the light and the dark
//! style, at the display's normal scale, and record what you see in docs/spikes/.

use std::time::Duration;

use futures_lite::StreamExt;
use ksni::TrayMethods;
use statsbar_tray_spike::{Glyph, SETUP_TIMEOUT, bounded, render};

const WHITE: [u8; 4] = [255, 255, 255, 255];
const BLACK: [u8; 4] = [255, 0, 0, 0];

struct Variant {
    /// Single letter shown in the item's title and tooltip, so a screenshot can name it.
    name: &'static str,
    glyph: Glyph,
    text: String,
    /// Pixmaps offered to the host, as (dot size, height).
    sizes: Vec<(usize, usize)>,
    argb: [u8; 4],
}

impl ksni::Tray for Variant {
    fn id(&self) -> String {
        format!("statsbar-spike-label-{}", self.name)
    }
    fn title(&self) -> String {
        format!("Variant {}", self.name)
    }
    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: format!("Variant {}", self.name),
            ..Default::default()
        }
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        self.sizes
            .iter()
            .map(|&(dot, height)| render(self.glyph, &self.text, dot, height, self.argb))
            .collect()
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            ksni::menu::StandardItem {
                label: format!("Variant {}: quit spike", self.name),
                activate: Box::new(|_| std::process::exit(0)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let variants = vec![
        // A: the candidate design, 16 px tall, cycling so the redraw and the fixed width show.
        Variant {
            name: "A",
            glyph: Glyph::Cpu,
            text: "7%".into(),
            sizes: vec![(2, 16)],
            argb: WHITE,
        },
        // B: the same drawing at twice the pixels, for a host that wants a larger pixmap.
        Variant {
            name: "B",
            glyph: Glyph::Memory,
            text: "100%".into(),
            sizes: vec![(4, 32)],
            argb: WHITE,
        },
        // C: both sizes offered, letting the host choose.
        Variant {
            name: "C",
            glyph: Glyph::Cpu,
            text: "100%".into(),
            sizes: vec![(2, 16), (4, 32)],
            argb: WHITE,
        },
        // D: dark dots, for a light panel.
        Variant {
            name: "D",
            glyph: Glyph::Memory,
            text: "100%".into(),
            sizes: vec![(2, 16)],
            argb: BLACK,
        },
        // E: one dot per pixel, the smallest the font can be.
        Variant {
            name: "E",
            glyph: Glyph::Cpu,
            text: "100%".into(),
            sizes: vec![(1, 16)],
            argb: WHITE,
        },
    ];
    async_io::block_on(async {
        let mut handles = Vec::new();
        for variant in variants {
            // Printed so the results document records the sizes actually offered, not ones
            // worked out by hand.
            for &(dot, height) in &variant.sizes {
                let icon = render(variant.glyph, &variant.text, dot, height, variant.argb);
                println!(
                    "variant {}: offers {}x{}",
                    variant.name, icon.width, icon.height
                );
            }
            handles.push(bounded("tray item spawn", SETUP_TIMEOUT, variant.spawn()).await??);
        }
        println!(
            "pid {}: five items shown; quit from any item's menu",
            std::process::id()
        );
        let cycle = ["7%", "37%", "100%"];
        let mut ticks = async_io::Timer::interval(Duration::from_secs(2));
        let mut step = 0;
        loop {
            ticks.next().await;
            step = (step + 1) % cycle.len();
            handles[0]
                .update(|variant: &mut Variant| variant.text = cycle[step].to_string())
                .await;
        }
    })
}
```

- [x] **Step 5: Build, lint, and check for a second `zbus`**

```bash
cd spikes/tray
cargo build --release
cargo clippy --release --all-targets -- -D warnings
cargo fmt --check
cargo tree -e normal | grep -oE 'zbus v[0-9.]+' | sort -u
cd ../..
```

Expected: build and clippy end in `Finished`, `cargo fmt --check` prints nothing, and the last command
prints exactly one line: `zbus v5.19.0`. Two lines mean a second `zbus` came in: stop and report.

- [x] **Step 6: Audit the dependency tree**

Run: `cargo audit --file spikes/tray/Cargo.lock`
Expected: no vulnerabilities found. If the command is missing, ask the user whether to run
`cargo install cargo-audit --locked`; if they decline, record "audit not run: cargo-audit not
installed" in the S1 results document and in the final report. An advisory is a stop: report it.

Execution on 2026-11-18: installed `cargo-audit` with approval and scanned the lockfile. The audit
reported three unmaintained dependencies and zero vulnerabilities. The user approved proceeding
with the existing lockfile. See the S1 results document.

- [x] **Step 7: Show the items, with the user**

Tell the user five tray items are about to appear, then start the spike in the background:

```bash
spikes/tray/target/release/label
```

Expected: it prints one `variant <X>: offers <w>x<h>` line per pixmap (A 64x16, B 128x32, C 64x16 and
128x32, D 64x16, E 32x16), then `pid <n>: five items shown; quit from any item's menu`, and five items
appear in the top bar. A `timed out` error means the tray watcher did not answer: stop and report. Ask
the user to:

1. Note each item's look in the current (dark) style: is it wider than tall, as drawn (the pixmap
   sizes printed above, in pixmap pixels; the host may scale them)? Is "100%" readable at normal
   viewing distance? Does item A change between 7 %, 37 % and 100 % every 2 s without changing its
   width? What colour is the top bar's background? (Ubuntu's top bar often stays dark in the light
   style, so variant D's black may be invisible in both; the background colour tells why.)
2. Take a screenshot (Print Screen) for their own reference.
3. Switch Settings → Appearance to the light style, repeat 1–2, then switch back to dark.
4. Read the display scale from Settings → Displays.

Also record the extension version:

```bash
gnome-extensions info ubuntu-appindicators@ubuntu.com | grep -i version
```

Then quit the spike from any item's menu (or `pkill -x label`), and confirm the items are gone.

Execution on 2026-11-18: the user reported that the labels were too small and requested captures.
The executor captured both styles, restored Dark, and verified that the watcher removed all five
items on exit. The executor, not the user, switched the GNOME style, with the user's approval; this
departs from the plan's rule that the user switches it. The desktop images are outside the
repository.

- [x] **Step 8: Record the result**

Write `docs/spikes/2026-11-17-s1-tray-label.md` with what was observed. Every cell gets an observed
value or "not observed"; nothing is inferred.

```markdown
# Spike S1 — tray label pixmap in the GNOME panel

Run on 2026-11-17 with `spikes/tray/target/release/label`.

**Machine:** Ubuntu 26.04, GNOME Shell 50.1, Wayland, `ubuntu-appindicators@ubuntu.com` version
(from Step 7), display scale (from Step 7).

**Question:** does a wide "glyph + percentage" pixmap stay wide and legible in the panel, in both
styles?

| Variant | Offered pixmap(s), as printed | Dark style: shape | Dark style: legible | Light style: shape | Light style: legible |
|---|---|---|---|---|---|
| A — CPU, cycling 7/37/100 %, white | 64×16 (dot 2) | | | | |
| B — memory, 100 %, white | 128×32 (dot 4) | | | | |
| C — CPU, 100 %, white | 64×16 and 128×32 | | | | |
| D — memory, 100 %, black | 64×16 (dot 2) | | | | |
| E — CPU, 100 %, white | 32×16 (dot 1) | | | | |

"Shape" is one of: *kept* (shown wider than tall, at its own aspect ratio), *squashed* (scaled into a
square), *cropped*. "Legible" is the user's yes or no.

Item A's width while cycling: *steady* or *shifting*.

Top bar background: dark style …, light style ….

**Dependencies:** licence, repository and newest release date of each crate (Task 1, Step 2).

**Decision** (one of the rows of the table in Plan 02a, Task 3, Step 2, quoted, with the variant).

**Audit:** (result of Task 1, Step 6).
```

- [x] **Step 9: Ready to commit**

`Ready to commit: spikes/tray/.gitignore spikes/tray/Cargo.toml spikes/tray/Cargo.lock
spikes/tray/src/lib.rs spikes/tray/src/bin/label.rs docs/spikes/2026-11-17-s1-tray-label.md —
"chore(spikes): add tray label spike S1 and record its result"`

---

## Task 2: S2 — threads and memory of tray items plus a main-thread zbus connection

**Files:**
- Create: `spikes/tray/src/bin/threads.rs`, `spikes/tray/measure.sh`,
  `docs/spikes/2026-11-17-s2-threads.md`

**Interfaces:**
- Produces: binary `spikes/tray/target/release/threads <items>` (prints its pid, then one
  `tick <n>: …` line per 2 s); `spikes/tray/measure.sh <pid>` (prints the thread count, each thread's
  id and name, and `Rss` / `Pss`); the S2 results document.
- Consumes: `zbus::connection::Builder::{session, internal_executor, build}`,
  `zbus::Connection::executor`, `zbus::fdo::DBusProxy::{new, get_id}`, `futures_lite::future::or`,
  `statsbar_tray_spike::{SETUP_TIMEOUT, bounded}` (Task 1), plus what Task 1 consumes.

- [x] **Step 1: Confirm the binary does not exist yet**

Run: `cargo build --release --manifest-path spikes/tray/Cargo.toml --bin threads`
Expected: FAIL — `no bin target named 'threads'`.

- [x] **Step 2: Write the binary and the measuring script**

`spikes/tray/src/bin/threads.rs`:

```rust
//! Spike S2: how many threads, and how much memory, do N tray items plus one zbus connection
//! driven from the main thread cost?
//!
//! Usage: `threads <items>`. Prints its pid; measure it with `measure.sh <pid>`.

use std::time::Duration;

use futures_lite::{StreamExt, future};
use ksni::TrayMethods;
use statsbar_tray_spike::{SETUP_TIMEOUT, bounded};

/// The upper bound on how long a D-Bus call may hold up the loop, per the global constraint
/// that every external call has a timeout.
const CALL_TIMEOUT: Duration = Duration::from_secs(1);
const TICK: Duration = Duration::from_secs(2);

struct Item {
    index: usize,
    value: u32,
}

impl ksni::Tray for Item {
    fn id(&self) -> String {
        format!("statsbar-spike-threads-{}", self.index)
    }
    fn title(&self) -> String {
        format!("Item {}: {}%", self.index, self.value)
    }
    fn icon_name(&self) -> String {
        "utilities-system-monitor".into()
    }
}

type BoxError = Box<dyn std::error::Error>;

fn main() -> Result<(), BoxError> {
    let items: usize = match std::env::args().nth(1) {
        Some(arg) => arg.parse()?,
        None => 3,
    };
    async_io::block_on(async {
        let mut handles = Vec::new();
        for index in 0..items {
            handles.push(
                bounded(
                    "tray item spawn",
                    SETUP_TIMEOUT,
                    Item { index, value: 0 }.spawn(),
                )
                .await??,
            );
        }
        // internal_executor(false): zbus does not start its own thread; the loop below ticks the
        // connection's executor on this thread instead.
        let builder = zbus::connection::Builder::session()?.internal_executor(false);
        let conn = bounded("session bus connect", SETUP_TIMEOUT, builder.build()).await??;
        let executor = conn.executor().clone();
        println!(
            "pid {}: {items} items and one zbus connection",
            std::process::id()
        );
        let work = async {
            let dbus = bounded(
                "DBus proxy",
                SETUP_TIMEOUT,
                zbus::fdo::DBusProxy::new(&conn),
            )
            .await??;
            let mut ticks = async_io::Timer::interval(TICK);
            let mut count: u32 = 0;
            loop {
                ticks.next().await;
                count = count.wrapping_add(1);
                let reply = future::or(async { Some(dbus.get_id().await) }, async {
                    async_io::Timer::after(CALL_TIMEOUT).await;
                    None
                })
                .await;
                match reply {
                    Some(Ok(id)) => println!("tick {count}: bus id {id}"),
                    Some(Err(error)) => println!("tick {count}: GetId failed: {error}"),
                    None => println!("tick {count}: GetId timed out"),
                }
                for handle in &handles {
                    handle
                        .update(|item: &mut Item| item.value = count % 101)
                        .await;
                }
            }
        };
        let drive = async {
            loop {
                executor.tick().await;
            }
        };
        future::or(work, drive).await
    })
}
```

`spikes/tray/measure.sh` (then `chmod +x spikes/tray/measure.sh`):

```bash
#!/usr/bin/env bash
# Prints the threads and the memory of a running spike process, for docs/spikes/.
#
# Pss is the figure the roadmap's budget is stated in: shared pages are split between the
# processes that map them, so libraries the desktop already loaded are not charged in full.
set -euo pipefail

pid="${1:?usage: measure.sh <pid>}"

echo "threads ($(ps -T -o spid= -p "$pid" | wc -l)):"
ps -T -o spid=,comm= -p "$pid"
grep -E '^(Rss|Pss):' "/proc/$pid/smaps_rollup"
```

- [x] **Step 3: Build and lint**

```bash
cd spikes/tray
cargo build --release
cargo clippy --release --all-targets -- -D warnings
cargo fmt --check
bash -n measure.sh
cd ../..
```

Expected: `Finished` twice, no output from the last two commands.

- [x] **Step 4: Measure with 1, 3 and 6 items**

Tell the user that up to six tray items will appear for about half a minute each. Run from `spikes/tray`:

```bash
(
    set -euo pipefail
    pid=
    cleanup() {
        if [[ -n "$pid" ]]; then
            kill "$pid" 2>/dev/null || true
            wait "$pid" 2>/dev/null || true
        fi
    }
    trap cleanup EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM
    for n in 1 3 6; do
        log="target/threads-$n.log"
        ./target/release/threads "$n" > "$log" 2>&1 &
        pid=$!
        if ! timeout 60 bash -c '
            until grep -q "^tick 15:" "$1"; do
                kill -0 "$2" 2>/dev/null || exit 1
                sleep 0.2
            done
        ' bash "$log" "$pid"; then
            cat "$log" >&2
            exit 1
        fi
        ./measure.sh "$pid" | tee "target/measure-$n.txt"
        cleanup
        pid=
        awk '/^tick [0-9]+:/ { ticks++ }
             /^tick [0-9]+: bus id / { answers++ }
             /^tick [0-9]+: GetId / { print }
             END { printf "%d/%d D-Bus calls answered\n", answers, ticks }' "$log"
    done
)
```

Expected: after 30 s of ticks, `measure.sh` prints the thread list and memory, then the loop prints
the number of calls answered and the number of ticks. A setup failure stops the loop. Record any
`GetId timed out` or `GetId failed` lines.

What reading the sources predicts, to compare against (a smoke run of `threads 1` for 7 s on
2026-11-17 showed 4 threads, `Pss` 1900 kB):

| Thread name | Started by | Expected to stay |
|---|---|---|
| `threads` (first) | the process: main loop, `zbus` executor ticked here | yes |
| `async-io` | `async-io`'s reactor fallback driver | yes |
| `threads` (second) | `ksni`'s `compat::kick_driver`, running every tray item's executor | yes, while any item lives |
| `blocking-1` | the `blocking` thread pool (used by `zbus` while connecting) | unknown: the pool may retire idle threads |

- [x] **Step 5: Record the result**

Write `docs/spikes/2026-11-17-s2-threads.md` from `target/measure-*.txt` and the logs:

```markdown
# Spike S2 — threads and memory of the tray

Run on 2026-11-17 with `spikes/tray` (`threads <N>`, release profile as in the workspace), measured
after 15 ticks (30 s) with `spikes/tray/measure.sh`.

**Question:** with `ksni` 0.3.7 (`async-io`) and one `zbus` 5.19.0 connection whose executor is
ticked on the main thread (`internal_executor(false)`), which threads exist, and what does each tray
item cost?

| Items | Threads (count: names) | Rss | Pss | D-Bus calls answered / ticks |
|---|---|---|---|---|
| 1 | | | | |
| 3 | | | | |
| 6 | | | | |

**Per item:** (Pss at 6 − Pss at 1) ÷ 5 = … kB. Each `ksni` item opens its own session-bus connection.

**Threads:** which of the predicted threads exist, which are extra, which retired.

**Finding for the invariant:** the wording proposed in Plan 02a, Task 3, Step 3, or the reason it does
not fit.
```

- [x] **Step 6: Ready to commit**

`Ready to commit: spikes/tray/src/bin/threads.rs spikes/tray/measure.sh
docs/spikes/2026-11-17-s2-threads.md — "chore(spikes): add thread and memory spike S2 and record its
result"`

---

## Task 3: Verification & Docs

**Files:**
- Modify: `docs/plans/2026-11-16-rust-statsbar-roadmap.md` — the Tech stack paragraph (lines 17–21),
  the second project-invariant bullet (lines 80–84), the Technical Defaults "Tray label" bullet
  (lines 102–104), the first two rows of Risks & Rollback (lines 110–111), and the "Plan 02" section
  (lines 217–230). Line numbers are as of `origin/main` on 2026-11-17; match on the quoted text.
- Modify: `README.md` — the "Plans" list.

**Interfaces:**
- Consumes: the two results documents from Tasks 1 and 2.
- Produces: a roadmap that Plan 02b can be written against.

- [x] **Step 1: Correct the `zbus` version in the roadmap**

In the Tech stack paragraph, replace `` `zbus` 6.3.0 (NetworkManager, BlueZ, UPower, notifications) ``
with `` `zbus` 5.19.0 (NetworkManager, BlueZ, UPower, notifications; the major version `ksni` 0.3.7
uses, so only one copy is linked) ``, and append to the paragraph: ``Checked again on 2026-11-17 by
Plan 02a, which built and ran `ksni` and `zbus` together.``

- [x] **Step 2: Record S1's outcome**

Pick the row that matches the S1 results document and make its edit to the "Tray label" bullet:

| S1 observed | Decision | New "Tray label" bullet |
|---|---|---|
| Exactly one variant kept its shape and was legible in both styles | Ship that variant's pixmap size(s) and colour | `Tray label: glyph and percentage drawn into the item's pixmap with an embedded 5×7 bitmap font, <sizes and colour of the variant>. Confirmed legible in the GNOME panel, light and dark, by spike S1 (docs/spikes/2026-11-17-s1-tray-label.md).` |
| Shape kept, but no variant legible | Roadmap fallback 1: rasterise a system font | Stop and ask the user before editing: this adds a font stack to the daemon, against the memory budget |
| Every variant squashed or cropped | Roadmap fallback 2: a level icon, with the figure in the menu | Stop and ask the user before editing: this changes what the user sees in the panel |
| More than one variant kept its shape and was legible in both styles | The user picks which ships | Stop and ask the user, showing the passing variants; then use the first row's wording for their pick |
| Any other outcome (legible in one style only, a colour that works in one style only, some variants squashed and others kept but none passing both styles) | — | Stop: report the S1 table to the user and ask; edit nothing |

Replace the Risks row that starts with `GNOME's AppIndicator extension may scale` with the same
outcome: `Settled by spike S1: <one sentence from the results document>`.

Execution on 2026-11-18: all five variants kept their proportions and were too small to read. The
user approved a system-font trial in Plan 02b, with another Pss measurement.

- [ ] **Step 3: Record S2's outcome — with the user's approval**

The project invariant is a rule the user accepted, so its new wording is shown to the user and applied
only on their yes. If S2 found the predicted threads, propose this paragraph to replace the one that
starts `**Project invariant:** \`statsbard\` never links GTK`:

```markdown
- **Project invariant:** `statsbard` never links GTK and never starts a tokio runtime. Its threads are
  bounded and named in this list, measured by spike S2 (`docs/spikes/2026-11-17-s2-threads.md`): the
  main event loop, which also ticks the daemon's own `zbus` connection; `async-io`'s reactor thread;
  `ksni`'s single driver thread, shared by every tray item; and one worker that runs blocking calls
  (HTTP, `ideviceinfo`, `adb`) one at a time, each with its own timeout, so a slow call never stalls
  polling or the tray. Threads of the `blocking` pool that `zbus` uses while connecting are allowed
  only while they retire on their own, as S2 recorded.
```

Adjust the last sentence to what S2 recorded about `blocking-1` (it retired, or it stayed). If S2
found anything else — more threads, a call that timed out, a per-item cost that makes six items alone
exceed half the 15 MB budget — do not propose wording: report the numbers and ask.

Replace the Risks row that starts with `` `zbus` starts its own executor thread `` with: `Settled by
spike S2: <thread list and per-item Pss from the results document>`.

Execution on 2026-11-18: S2 measured four threads and about 68 kB Pss per additional item.
The risks table records that evidence. The user deferred changing the project invariant.

- [x] **Step 4: Split Plan 02 in the roadmap**

Replace the heading `### Plan 02 — Spikes, daemon skeleton, first vertical slice` and its first
bullet (`- **Scope:** spike S1 … summary menus.`) with:

```markdown
### Plan 02a — Tray spikes *(written: `docs/plans/2026-11-17-rust-statsbar-02a-spikes.md`)*
- **Scope:** spike S1 (does a wide label pixmap render legibly in the GNOME panel, light and dark?)
  and spike S2 (threads and memory of `ksni` items plus a main-thread `zbus` connection). Results in
  `docs/spikes/`.
- **Evidence:** both results documents, and the roadmap updated from them.

### Plan 02b — Daemon skeleton, first vertical slice
- **Scope:** the config file, the scheduler, the worker thread, the label renderer, the hub item with
  show/hide switches, launch at login, and CPU and Memory as tray items with summary menus. Written
  after Plan 02a, against its results.
```

Keep the remaining Plan 02 bullets (Core additions, Readers, Tests, Evidence) under Plan 02b.

- [x] **Step 5: List the plan in the README**

In `README.md`, under "## Plans", after the Plan 01 bullet, add:

```markdown
- [Plan 02a — tray spikes](docs/plans/2026-11-17-rust-statsbar-02a-spikes.md): what the GNOME panel
  does with a label pixmap, and what the tray costs in threads and memory. Results in
  [docs/spikes/](docs/spikes/).
```

- [x] **Step 6: Final gate**

*Execution note (2026-11-18):* items 1–3 were run before the commits; `code-review-strict` (item 4)
first ran after them. Round 1: code approved, docs had three Important findings, fixed in
`docs: align roadmap with the label renderer and the S2 thread finding`. Round 2 approved the docs.
Minor findings were handed to the user as follow-ups.

1. Read the full diff, untracked files included: `git status --short` and `git diff origin/main`.
   Only the files named in Tasks 1–3 may appear; no debug output, TODOs or commented-out code; no
   screenshots; nothing under `spikes/tray/target/`.
2. Run `./scripts/check.sh`. Expected: exits 0 and ends with `core purity: ok` — the workspace is
   untouched. Also confirm the spike is outside it:
   `cargo metadata --no-deps --format-version 1 | grep -o '"name":"statsbar-tray-spike"'` prints
   nothing.
3. Self-review, one concrete reason per item: every confirmed requirement maps to a step; every
   bus call is bounded (`SETUP_TIMEOUT` through `bounded`, `CALL_TIMEOUT` for `GetId`); no result in `docs/spikes/` states more than was observed.
4. Run `code-review-strict` over the branch; fix or report its findings.
5. If 1–4 changed anything, repeat from 1.

Report, as verification gaps that stay open: test-first was not applied to the spike crate (reason in
Global Constraints); S1 rests on the user's eyes on one display at one scale; S2 measured one 30 s
window per item count, not an idle-hour average (Plan 11 measures the daemon properly); and the audit
result from Task 1, Step 6, and any dependency whose newest release is older than 12 months.

- [x] **Step 7: Ready to commit**

*Execution note (2026-11-18):* committed with this message once the user asked for commits.

`Ready to commit: docs/plans/2026-11-16-rust-statsbar-roadmap.md README.md — "docs: record tray spike
results and split plan 02"`
