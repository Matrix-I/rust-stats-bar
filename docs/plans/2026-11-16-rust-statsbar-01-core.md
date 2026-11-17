# rust-statsbar — Plan 01: Core Pure Layer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: development-superpowers:subagent-driven-development or
> executing-plans-strict. Steps use `- [ ]`.

**Goal:** a Cargo workspace whose `statsbar-core` crate holds the arithmetic every later plan builds on
— CPU load, memory buckets, network throughput, usage history, the hot-sensor alert policy and number
formatting — with a verification script and a gate that keeps the crate free of I/O.

**Architecture:** `crates/core` is pure: text and numbers in, numbers and text out. Callers pass in
what they read and when they read it. Each module has an integration-test file in `crates/core/tests/`
that is written, and fails, before the module exists. See
`docs/plans/2026-11-16-rust-statsbar-roadmap.md` for how this crate fits the whole.

**Tech Stack:** Rust 1.99.1, edition 2024, `cargo fmt`, `cargo clippy` (both verified installed). No
third-party crates in this plan.

**Branch:** `feature/core-pure-layer`

## Confirmed Requirements

- Rust; the smallest idle CPU and memory footprint decides design choices. *(user; see roadmap)*
- Behaviour follows upstream StatsBar where Linux has an equivalent. For this plan that means:
  `ThroughputTracker`, `UsageHistory`, `HotDeviceAlertPolicy` and `Formatting` keep upstream's rules
  and thresholds (0.2 s minimum interval; promote units at 999.95 and 999.5; health bands at 80 and 60;
  alert once, re-arm after cooling by a margin).
- Target hardware is this machine only, so CPU load has no efficiency/performance core split. *(user)*
- "Memory used" is `MemTotal − MemAvailable`, the figure GNOME System Monitor and `free` show. The
  bar's App and Kernel segments add up to it. *(user)*
- The upstream repository belongs to the user, so its pure files may be ported. *(user)*
- Project and repository name: `rust-statsbar`. *(user)* Crate and binary names keep the `statsbar`
  stem (`statsbar-core`, `statsbard`, …) as the roadmap proposed; the user named only the project.

## Global Constraints

- Test-first. Each test must be able to fail without its change. No sleeps, wall clock or network.
- Never weaken, skip or delete a test to make it pass.
- Commit only when asked: each task ends with `Ready to commit`, and nothing runs `git commit`.
- Code, comments, names and commit messages in English; comments explain why.
- `crates/core` does no I/O, reads no clock, starts no thread, contains no `unsafe`, has no build
  script, and depends on nothing but (later) `serde` in any dependency table.
  `scripts/check-core-purity.sh` enforces it.
- Verification command for every task and for the final gate: `./scripts/check.sh`.

## Technical Defaults

- Integration tests under `crates/core/tests/`, not inline `mod tests` — the test file can exist and
  fail before its module does.
- `u64` counters with `saturating_sub` and `saturating_add` — Linux counters are 64-bit, a counter
  that steps backwards must read as "no progress", and parsed text must never panic on overflow.
- Parsers return `Option`, not `Result` — there is one failure a caller can act on ("this sample is
  unusable, keep the previous one"), and nothing to report beyond that.
- Timestamps enter as `u64` nanoseconds from the caller's monotonic clock — keeps the crate clock-free.
- Default `rustfmt` settings, no `rustfmt.toml` — nothing to argue about.
- `Cargo.lock` is committed — the workspace ships binaries.

## Risks & Rollback

| Risk | Mitigation | Revert |
|---|---|---|
| The split of "used" into App and Kernel, and of the rest into Cached and Free, is this project's mapping onto upstream's bar; only the "used" total is confirmed by the user | Each definition is stated in a doc comment and pinned by a test; Task 7 compares "used" with `free` on the live machine | Change `MemoryBuckets::from_meminfo`; nothing is persisted |
| `MemAvailable` is required, so a kernel older than 3.14 shows no memory figures | Accepted: the target machine runs kernel 7.0 | — |
| The purity gate is a `grep`, so it can be fooled by an alias such as `use std as s;` | Accepted: it guards against accidents, and review covers intent. Task 1 proves it fires on the plain case | — |

No persisted data, public API, message format or user-visible behaviour exists before this plan, so
there is nothing to migrate.

---

## Task 1: Workspace, verification script, purity gate

**Files:**
- Create: `.gitignore`, `Cargo.toml`, `crates/core/Cargo.toml`, `crates/core/src/lib.rs`,
  `scripts/check.sh`, `scripts/check-core-purity.sh`

**Interfaces:**
- Produces: `./scripts/check.sh` (exit 0 when formatting, lints, tests and the purity gate all pass);
  `./scripts/check-core-purity.sh` (exit 1 naming the offending lines).

- [x] **Step 1: Create the branch**

The repository already exists (`~/Desktop/rust-statsbar`, remote `origin` =
`github.com/Matrix-I/rust-stats-bar`) and holds these plans. Branch from the latest `main`:

```bash
git fetch origin
git switch -c feature/core-pure-layer origin/main
```

If the plans' branch `chore/add-plans` is not merged into `main` yet, branch from it instead, so the
plans stay in the tree: `git switch -c feature/core-pure-layer chore/add-plans`.

- [x] **Step 2: Run the check that does not exist yet**

Run: `./scripts/check.sh`
Expected: FAIL — the shell reports that `./scripts/check.sh` does not exist.

- [x] **Step 3: Write the workspace and the scripts**

`.gitignore`:

```gitignore
/target
```

`Cargo.toml`:

```toml
[workspace]
resolver = "3"
members = ["crates/core"]

[workspace.package]
version = "0.1.0"
edition = "2024"
# The oldest toolchain that knows edition 2024. Development uses 1.99; this only states the floor.
rust-version = "1.85"
publish = false

[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
all = { level = "deny", priority = -1 }

# The daemon runs for the whole session, so the release build is tuned for a small resident
# set first and speed second: one codegen unit and fat LTO let dead code be dropped across
# crates, and aborting on panic removes the unwinding tables.
[profile.release]
opt-level = "s"
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true
```

Change `members` only when a later plan adds a crate.

`crates/core/Cargo.toml`:

```toml
[package]
name = "statsbar-core"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
publish.workspace = true

[dependencies]

[lints]
workspace = true
```

`crates/core/src/lib.rs`:

```rust
//! The pure layer of StatsBar for Linux.
//!
//! Everything here turns text or numbers into numbers or text. Nothing reads a file, a socket
//! or a clock: callers pass in what they read and when they read it. That is what lets the
//! tests pin the arithmetic on any machine, and `scripts/check-core-purity.sh` enforces it.
```

`scripts/check-core-purity.sh`:

```bash
#!/usr/bin/env bash
# Fails when crates/core stops being pure.
#
# crates/core is the only crate whose tests can pin arithmetic without a real machine behind
# them, and that only holds while nothing in it reads a file, spawns a process, opens a socket
# or looks at a clock. A reader's output is whatever this laptop's sensors say that second, so
# code that mixes the two cannot be asserted on. This gate makes the rule enforced rather than
# documented.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
src="$root/crates/core/src"

denylist='std::fs|std::net|std::process|std::env|std::thread|std::os::|std::io::|SystemTime|Instant::now|unsafe '

if matches="$(grep -rnE "$denylist" "$src")"; then
    echo "crates/core must stay pure (no I/O, clocks, threads or unsafe). Offending lines:" >&2
    echo "$matches" >&2
    exit 1
fi

# A dependency could do I/O on the crate's behalf, so the manifest is part of the gate.
# serde is the single allowed entry: later plans derive it on the snapshot types. Every
# dependency table counts (dev-, build- and target-specific ones too), because each of them
# can bring code into the build. serde must be declared inline (`serde = { … }`); the table form
# `[dependencies.serde]` would read as a violation here.
manifest="$root/crates/core/Cargo.toml"
deps="$(awk '/^\[(.*\.)?(dev-|build-)?dependencies(\..*)?\]/{flag=1; next} /^\[/{flag=0}
    flag && NF && $0 !~ /^#/' "$manifest" | grep -vE '^serde( |=)' || true)"
if [[ -n "$deps" ]]; then
    echo "crates/core may depend on serde only. Found:" >&2
    echo "$deps" >&2
    exit 1
fi

# A build script runs arbitrary code at compile time.
if [[ -e "$root/crates/core/build.rs" ]] || grep -qE '^\s*build\s*=' "$manifest"; then
    echo "crates/core must not have a build script." >&2
    exit 1
fi

echo "core purity: ok"
```

`scripts/check.sh`:

```bash
#!/usr/bin/env bash
# The single verification entry point: formatting, lints, tests and the purity gate.
# CI and the "before reporting done" gate both run exactly this.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
./scripts/check-core-purity.sh
```

```bash
chmod +x scripts/check.sh scripts/check-core-purity.sh
```

- [x] **Step 4: Run the check**

Run: `./scripts/check.sh`
Expected: PASS — ends with `core purity: ok`.

- [x] **Step 5: Prove both halves of the gate can fail**

```bash
echo 'use std::fs;' >> crates/core/src/lib.rs
./scripts/check-core-purity.sh; echo "exit=$?"
```

Expected: `crates/core must stay pure …`, the planted line, `exit=1`.

```bash
git checkout -- crates/core/src/lib.rs 2>/dev/null || sed -i '$ d' crates/core/src/lib.rs
cp crates/core/Cargo.toml target/core-manifest.toml
printf '\n[dev-dependencies]\nproptest = "1"\n' >> crates/core/Cargo.toml
./scripts/check-core-purity.sh; echo "exit=$?"
cp target/core-manifest.toml crates/core/Cargo.toml
touch crates/core/build.rs
./scripts/check-core-purity.sh; echo "exit=$?"
rm crates/core/build.rs
./scripts/check-core-purity.sh
```

Expected: `crates/core may depend on serde only. Found: proptest = "1"` and `exit=1`; then
`crates/core must not have a build script.` and `exit=1`; then `core purity: ok`. Nothing is
committed yet, so the first restore falls through to `sed`. Confirm no mutant is left behind:
`tail -1 crates/core/src/lib.rs` must print the doc comment's last line, and
`diff crates/core/Cargo.toml target/core-manifest.toml` must print nothing.

- [x] **Step 6: Commit checkpoint**

Ready to commit: `.gitignore Cargo.toml Cargo.lock crates/core/Cargo.toml crates/core/src/lib.rs scripts/check.sh scripts/check-core-purity.sh`
— `chore: scaffold workspace with verification script and core purity gate`

---

## Task 2: CPU load

**Files:**
- Create: `crates/core/tests/cpu.rs`, `crates/core/src/cpu.rs`
- Modify: `crates/core/src/lib.rs` (add one module line)

**Interfaces:**
- Produces:
  - `pub struct CoreTicks { pub user: u64, pub nice: u64, pub system: u64, pub idle: u64 }` — `Debug, Clone, Copy, PartialEq, Eq, Default`
  - `pub fn parse_proc_stat(text: &str) -> Option<Vec<CoreTicks>>`
  - `pub struct CpuLoad { pub system_percent: f64, pub user_percent: f64, pub idle_percent: f64, pub per_core_busy: Vec<f64> }`
  - `impl CpuLoad { pub fn between(previous: &[CoreTicks], current: &[CoreTicks]) -> Option<CpuLoad>; pub fn usage_percent(&self) -> f64 }`

- [x] **Step 1: Write the failing test**

`crates/core/tests/cpu.rs`:

```rust
use statsbar_core::cpu::{CoreTicks, CpuLoad, parse_proc_stat};

const STAT: &str = "\
cpu  300 20 100 1700 40 10 6 4 0 0
cpu0 100 0 40 850 10 5 3 2 0 0
cpu1 200 20 60 850 30 5 3 2 0 0
intr 12345 0 0
ctxt 6789
";

#[test]
fn parses_per_cpu_lines_and_skips_the_aggregate() {
    let cores = parse_proc_stat(STAT).expect("two cpu lines");
    assert_eq!(
        cores,
        vec![
            CoreTicks {
                user: 100,
                nice: 0,
                system: 50,
                idle: 860
            },
            CoreTicks {
                user: 200,
                nice: 20,
                system: 70,
                idle: 880
            },
        ]
    );
}

#[test]
fn accepts_the_four_column_form_of_old_kernels() {
    let cores = parse_proc_stat("cpu0 1 2 3 4\n").expect("four columns are enough");
    assert_eq!(
        cores,
        vec![CoreTicks {
            user: 1,
            nice: 2,
            system: 3,
            idle: 4
        }]
    );
}

#[test]
fn rejects_a_malformed_cpu_line_instead_of_shifting_cores() {
    assert_eq!(parse_proc_stat("cpu0 1 2 3 4\ncpu1 1 x 3 4\n"), None);
    assert_eq!(parse_proc_stat("cpu0 1 2 3\n"), None);
}

#[test]
fn rejects_text_without_per_cpu_lines() {
    assert_eq!(parse_proc_stat(""), None);
    assert_eq!(parse_proc_stat("cpu  1 2 3 4\n"), None);
}

#[test]
fn load_is_the_share_of_each_bucket_in_the_interval() {
    let previous = [CoreTicks {
        user: 100,
        nice: 0,
        system: 50,
        idle: 850,
    }];
    let current = [CoreTicks {
        user: 150,
        nice: 10,
        system: 70,
        idle: 870,
    }];
    let load = CpuLoad::between(&previous, &current).expect("same core count");
    assert_eq!(load.user_percent, 60.0); // nice is folded into user
    assert_eq!(load.system_percent, 20.0);
    assert_eq!(load.idle_percent, 20.0);
    assert_eq!(load.per_core_busy, vec![80.0]);
    assert_eq!(load.usage_percent(), 80.0);
}

#[test]
fn per_core_busy_is_independent_per_core() {
    let previous = [CoreTicks::default(), CoreTicks::default()];
    let current = [
        CoreTicks {
            user: 100,
            nice: 0,
            system: 0,
            idle: 0,
        },
        CoreTicks {
            user: 0,
            nice: 0,
            system: 0,
            idle: 100,
        },
    ];
    let load = CpuLoad::between(&previous, &current).expect("same core count");
    assert_eq!(load.per_core_busy, vec![100.0, 0.0]);
    assert_eq!(load.usage_percent(), 50.0);
}

#[test]
fn an_interval_with_no_ticks_reads_as_idle() {
    let ticks = [CoreTicks {
        user: 5,
        nice: 5,
        system: 5,
        idle: 5,
    }];
    let load = CpuLoad::between(&ticks, &ticks).expect("same core count");
    assert_eq!(load.idle_percent, 100.0);
    assert_eq!(load.usage_percent(), 0.0);
    assert_eq!(load.per_core_busy, vec![0.0]);
}

#[test]
fn a_counter_that_went_backwards_counts_as_no_progress() {
    let previous = [CoreTicks {
        user: 500,
        nice: 0,
        system: 0,
        idle: 100,
    }];
    let current = [CoreTicks {
        user: 10,
        nice: 0,
        system: 0,
        idle: 200,
    }];
    let load = CpuLoad::between(&previous, &current).expect("same core count");
    assert_eq!(load.idle_percent, 100.0);
    assert_eq!(load.per_core_busy, vec![0.0]);
}

#[test]
fn a_changed_core_count_yields_no_load() {
    let one = [CoreTicks::default()];
    let two = [CoreTicks::default(), CoreTicks::default()];
    assert_eq!(CpuLoad::between(&one, &two), None);
    assert_eq!(CpuLoad::between(&[], &[]), None);
}

#[test]
fn columns_that_would_overflow_saturate_instead_of_panicking() {
    let max = u64::MAX;
    let cores =
        parse_proc_stat(&format!("cpu0 1 2 {max} {max} {max} 1 1 1\n")).expect("numbers parse");
    assert_eq!(cores[0].system, u64::MAX);
    assert_eq!(cores[0].idle, u64::MAX);
}
```

- [x] **Step 2: Run it and watch it fail**

Run: `cargo test -p statsbar-core --test cpu`
Expected: FAIL — `error[E0432]: unresolved import `statsbar_core::cpu``.

- [x] **Step 3: Implement**

Append to `crates/core/src/lib.rs`:

```rust

pub mod cpu;
```

`crates/core/src/cpu.rs`:

```rust
//! CPU load: `/proc/stat` text in, percentages out.

/// Cumulative jiffies of one logical CPU, folded into the buckets the UI shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CoreTicks {
    pub user: u64,
    pub nice: u64,
    pub system: u64,
    pub idle: u64,
}

/// Parses the per-CPU lines (`cpu0`, `cpu1`, …) of `/proc/stat`, in file order.
///
/// irq, softirq and steal are folded into `system`, and iowait into `idle`, so the four
/// buckets add up to all the time the kernel accounted for. The aggregate `cpu ` line is
/// skipped because it is the sum of the others.
///
/// Returns `None` when there is no per-CPU line or one of them is malformed. Skipping a bad
/// line instead would shift every later core by one position, and the per-core view would
/// then pair a core with its neighbour's temperature.
pub fn parse_proc_stat(text: &str) -> Option<Vec<CoreTicks>> {
    let mut cores = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("cpu") else {
            continue;
        };
        if !rest.starts_with(|c: char| c.is_ascii_digit()) {
            continue;
        }
        let mut fields = rest.split_ascii_whitespace();
        fields.next()?; // the CPU index glued to the "cpu" prefix
        let mut values = [0u64; 8];
        let mut count = 0;
        for (slot, field) in values.iter_mut().zip(fields) {
            *slot = field.parse().ok()?;
            count += 1;
        }
        // user, nice, system and idle exist on every kernel; the later columns were added
        // over time and are zero when absent.
        if count < 4 {
            return None;
        }
        let [user, nice, system, idle, iowait, irq, softirq, steal] = values;
        // Saturating: the text is untrusted input, and a plain `+` panics on overflow in
        // debug builds.
        cores.push(CoreTicks {
            user,
            nice,
            system: system
                .saturating_add(irq)
                .saturating_add(softirq)
                .saturating_add(steal),
            idle: idle.saturating_add(iowait),
        });
    }
    if cores.is_empty() { None } else { Some(cores) }
}

/// CPU usage over the interval between two `/proc/stat` samples.
#[derive(Debug, Clone, PartialEq)]
pub struct CpuLoad {
    pub system_percent: f64,
    pub user_percent: f64,
    pub idle_percent: f64,
    /// Busy share of each logical CPU, 0–100, in `/proc/stat` order.
    pub per_core_busy: Vec<f64>,
}

impl CpuLoad {
    /// Returns `None` when the two samples do not describe the same set of CPUs (a core was
    /// taken offline between them), because the deltas would then mix different cores.
    pub fn between(previous: &[CoreTicks], current: &[CoreTicks]) -> Option<CpuLoad> {
        if previous.len() != current.len() || current.is_empty() {
            return None;
        }
        let (mut sum_user, mut sum_system, mut sum_idle, mut sum_total) = (0.0, 0.0, 0.0, 0.0);
        let mut per_core_busy = Vec::with_capacity(current.len());
        for (prev, cur) in previous.iter().zip(current) {
            // A counter that went backwards (suspend/resume on some kernels) counts as no
            // progress instead of underflowing into an enormous delta.
            let user = cur.user.saturating_sub(prev.user) as f64;
            let nice = cur.nice.saturating_sub(prev.nice) as f64;
            let system = cur.system.saturating_sub(prev.system) as f64;
            let idle = cur.idle.saturating_sub(prev.idle) as f64;
            let total = user + nice + system + idle;
            sum_user += user + nice;
            sum_system += system;
            sum_idle += idle;
            sum_total += total;
            per_core_busy.push(if total > 0.0 {
                (user + nice + system) / total * 100.0
            } else {
                0.0
            });
        }
        let mut load = CpuLoad {
            system_percent: 0.0,
            user_percent: 0.0,
            idle_percent: 100.0,
            per_core_busy,
        };
        if sum_total > 0.0 {
            load.user_percent = sum_user / sum_total * 100.0;
            load.system_percent = sum_system / sum_total * 100.0;
            load.idle_percent = sum_idle / sum_total * 100.0;
        }
        Some(load)
    }

    pub fn usage_percent(&self) -> f64 {
        (100.0 - self.idle_percent).clamp(0.0, 100.0)
    }
}
```

- [x] **Step 4: Run it and watch it pass**

Run: `cargo test -p statsbar-core --test cpu`
Expected: PASS — `test result: ok. 10 passed`.

Run: `./scripts/check.sh`
Expected: PASS.

- [x] **Step 5: Commit checkpoint**

Ready to commit: `crates/core/src/lib.rs crates/core/src/cpu.rs crates/core/tests/cpu.rs`
— `feat(core): parse /proc/stat and compute CPU load between samples`

---

## Task 3: Memory buckets

**Files:**
- Create: `crates/core/tests/memory.rs`, `crates/core/src/memory.rs`
- Modify: `crates/core/src/lib.rs` (add one module line)

**Interfaces:**
- Produces:
  - `pub struct MemInfo { pub total, free, available, s_unreclaim, kernel_stack, page_tables, swap_total, swap_free, zswap: u64 }` — bytes; `Debug, Clone, Copy, PartialEq, Eq, Default`
  - `pub fn parse_meminfo(text: &str) -> Option<MemInfo>` — `None` without a positive `MemTotal` or without `MemAvailable`
  - `pub struct MemoryBuckets { pub total, app, kernel, cached, free, swap_used, compressed: u64 }` — same derives
  - `impl MemoryBuckets { pub fn from_meminfo(info: &MemInfo) -> MemoryBuckets; pub fn used(&self) -> u64; pub fn usage_percent(&self) -> f64; pub fn fraction(&self, part: u64) -> f64 }`

Bucket definitions, which the tests pin:

| Bucket | Definition |
|---|---|
| `used()` | `MemTotal − MemAvailable` *(user)* |
| `kernel` | `SUnreclaim + KernelStack + PageTables`, at most `used` |
| `app` | `used − kernel` |
| `free` | `MemFree`, at most `total − used` (MemFree can exceed MemAvailable) |
| `cached` | `total − used − free`, which is `MemAvailable − MemFree` |

- [x] **Step 1: Write the failing test**

`crates/core/tests/memory.rs`:

```rust
use statsbar_core::memory::{MemInfo, MemoryBuckets, parse_meminfo};

const KIB: u64 = 1024;

const MEMINFO: &str = "\
MemTotal:       16000000 kB
MemFree:         2000000 kB
MemAvailable:    9000000 kB
Buffers:          500000 kB
Cached:          6000000 kB
SwapCached:        12345 kB
SwapTotal:       4000000 kB
SwapFree:        3000000 kB
Zswap:             64000 kB
Shmem:           1000000 kB
Slab:            1000000 kB
SReclaimable:     700000 kB
SUnreclaim:       300000 kB
KernelStack:       20000 kB
PageTables:        80000 kB
HugePages_Total:       0
";

#[test]
fn parses_the_fields_the_buckets_need_in_bytes() {
    let info = parse_meminfo(MEMINFO).expect("MemTotal and MemAvailable are present");
    assert_eq!(
        info,
        MemInfo {
            total: 16_000_000 * KIB,
            free: 2_000_000 * KIB,
            available: 9_000_000 * KIB,
            s_unreclaim: 300_000 * KIB,
            kernel_stack: 20_000 * KIB,
            page_tables: 80_000 * KIB,
            swap_total: 4_000_000 * KIB,
            swap_free: 3_000_000 * KIB,
            zswap: 64_000 * KIB,
        }
    );
}

#[test]
fn a_missing_optional_field_reads_as_zero() {
    let info =
        parse_meminfo("MemTotal: 1000 kB\nMemAvailable: 600 kB\n").expect("required present");
    assert_eq!(info.total, 1000 * KIB);
    assert_eq!(info.free, 0);
    assert_eq!(info.zswap, 0);
}

#[test]
fn a_zero_mem_available_is_a_value_not_a_missing_field() {
    let info = parse_meminfo("MemTotal: 1000 kB\nMemAvailable: 0 kB\n").expect("present");
    assert_eq!(MemoryBuckets::from_meminfo(&info).usage_percent(), 100.0);
}

#[test]
fn rejects_text_without_a_positive_total_or_without_mem_available() {
    assert_eq!(parse_meminfo(""), None);
    assert_eq!(parse_meminfo("MemAvailable: 400 kB\n"), None);
    assert_eq!(parse_meminfo("MemTotal: 0 kB\nMemAvailable: 0 kB\n"), None);
    assert_eq!(parse_meminfo("MemTotal: 1000 kB\nMemFree: 400 kB\n"), None);
}

#[test]
fn rejects_a_field_that_is_not_a_number() {
    assert_eq!(
        parse_meminfo("MemTotal: lots kB\nMemAvailable: 1 kB\n"),
        None
    );
}

#[test]
fn used_is_total_minus_available_split_into_app_and_kernel() {
    let buckets = MemoryBuckets::from_meminfo(&parse_meminfo(MEMINFO).expect("valid"));
    assert_eq!(buckets.used(), 7_000_000 * KIB); // MemTotal - MemAvailable, as GNOME shows
    assert_eq!(buckets.kernel, 400_000 * KIB); // SUnreclaim + KernelStack + PageTables
    assert_eq!(buckets.app, 6_600_000 * KIB);
    assert_eq!(buckets.free, 2_000_000 * KIB);
    assert_eq!(buckets.cached, 7_000_000 * KIB); // MemAvailable - MemFree
    assert_eq!(buckets.swap_used, 1_000_000 * KIB);
    assert_eq!(buckets.compressed, 64_000 * KIB);
    assert_eq!(buckets.usage_percent(), 43.75);
    assert_eq!(buckets.fraction(buckets.free), 0.125);
}

fn assert_adds_up(buckets: &MemoryBuckets) {
    assert_eq!(
        buckets.app + buckets.kernel + buckets.cached + buckets.free,
        buckets.total
    );
}

#[test]
fn free_above_available_is_clamped_so_the_bar_adds_up() {
    // MemAvailable holds back the kernel's reserve, so it can be below MemFree.
    let info = MemInfo {
        total: 1000,
        free: 300,
        available: 200,
        ..MemInfo::default()
    };
    let buckets = MemoryBuckets::from_meminfo(&info);
    assert_eq!(buckets.used(), 800);
    assert_eq!(buckets.free, 200);
    assert_eq!(buckets.cached, 0);
    assert_adds_up(&buckets);
}

#[test]
fn kernel_memory_above_used_is_clamped_to_used() {
    let info = MemInfo {
        total: 1000,
        free: 900,
        available: 950,
        s_unreclaim: 200,
        ..MemInfo::default()
    };
    let buckets = MemoryBuckets::from_meminfo(&info);
    assert_eq!(buckets.kernel, 50);
    assert_eq!(buckets.app, 0);
    assert_adds_up(&buckets);
}

#[test]
fn available_above_total_reads_as_nothing_used() {
    let info = MemInfo {
        total: 1000,
        free: 1500,
        available: 1200,
        page_tables: 10,
        ..MemInfo::default()
    };
    let buckets = MemoryBuckets::from_meminfo(&info);
    assert_eq!(buckets.used(), 0);
    assert_eq!(buckets.free, 1000);
    assert_adds_up(&buckets);
}

#[test]
fn kernel_fields_that_would_overflow_saturate() {
    let info = MemInfo {
        total: 1000,
        available: 400,
        s_unreclaim: u64::MAX,
        kernel_stack: u64::MAX,
        ..MemInfo::default()
    };
    let buckets = MemoryBuckets::from_meminfo(&info);
    assert_eq!(buckets.kernel, 600);
    assert_adds_up(&buckets);
}

#[test]
fn swap_free_above_swap_total_reads_as_no_swap_used() {
    let info = MemInfo {
        total: 1000,
        swap_total: 10,
        swap_free: 20,
        ..MemInfo::default()
    };
    assert_eq!(MemoryBuckets::from_meminfo(&info).swap_used, 0);
}

#[test]
fn an_unknown_total_gives_zero_fractions_instead_of_dividing_by_zero() {
    let buckets = MemoryBuckets::default();
    assert_eq!(buckets.fraction(123), 0.0);
    assert_eq!(buckets.usage_percent(), 0.0);
}
```

- [x] **Step 2: Run it and watch it fail**

Run: `cargo test -p statsbar-core --test memory`
Expected: FAIL — `error[E0432]: unresolved import `statsbar_core::memory``.

- [x] **Step 3: Implement**

Add to `crates/core/src/lib.rs`, so the module list reads:

```rust
pub mod cpu;
pub mod memory;
```

`crates/core/src/memory.rs`:

```rust
//! Memory: `/proc/meminfo` text in, the buckets of the memory bar out.

/// The `/proc/meminfo` fields the buckets are built from, in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MemInfo {
    pub total: u64,
    pub free: u64,
    pub available: u64,
    pub s_unreclaim: u64,
    pub kernel_stack: u64,
    pub page_tables: u64,
    pub swap_total: u64,
    pub swap_free: u64,
    pub zswap: u64,
}

const BYTES_PER_KIB: u64 = 1024;

/// Parses `/proc/meminfo`.
///
/// Returns `None` without a positive `MemTotal` (every fraction divides by it) or without
/// `MemAvailable` (the "used" figure is defined by it, and every kernel since 3.14 has it).
/// Any other missing field reads as zero: `Zswap`, for one, exists only when zswap is built in.
pub fn parse_meminfo(text: &str) -> Option<MemInfo> {
    let mut info = MemInfo::default();
    let mut has_available = false;
    for line in text.lines() {
        let Some((key, rest)) = line.split_once(':') else {
            continue;
        };
        let slot = match key {
            "MemTotal" => &mut info.total,
            "MemFree" => &mut info.free,
            "MemAvailable" => {
                has_available = true;
                &mut info.available
            }
            "SUnreclaim" => &mut info.s_unreclaim,
            "KernelStack" => &mut info.kernel_stack,
            "PageTables" => &mut info.page_tables,
            "SwapTotal" => &mut info.swap_total,
            "SwapFree" => &mut info.swap_free,
            "Zswap" => &mut info.zswap,
            _ => continue,
        };
        let kib: u64 = rest.split_ascii_whitespace().next()?.parse().ok()?;
        *slot = kib.checked_mul(BYTES_PER_KIB)?;
    }
    if info.total == 0 || !has_available {
        None
    } else {
        Some(info)
    }
}

/// The segments of the memory bar. `app + kernel + cached + free == total` always holds, so
/// the bar never over- or under-fills, and `app + kernel` is the "used" figure GNOME System
/// Monitor and `free` show: `MemTotal − MemAvailable`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MemoryBuckets {
    pub total: u64,
    /// Used memory that is not kernel memory: processes, tmpfs and shared memory.
    pub app: u64,
    /// Kernel memory that cannot be reclaimed. The Linux counterpart of macOS "Wired".
    pub kernel: u64,
    /// Memory the kernel could hand out by dropping caches. The counterpart of macOS
    /// "Cached Files".
    pub cached: u64,
    pub free: u64,
    pub swap_used: u64,
    /// Bytes held in the zswap pool, already counted inside `app`. Zero without zswap.
    pub compressed: u64,
}

impl MemoryBuckets {
    pub fn from_meminfo(info: &MemInfo) -> MemoryBuckets {
        let total = info.total;
        // The fields are sampled at slightly different instants and MemAvailable is an
        // estimate, so every step is clamped to what is still unassigned. That is what keeps
        // the sum exact. MemFree can even exceed MemAvailable, because the estimate holds back
        // the kernel's reserve.
        let used = total.saturating_sub(info.available);
        let kernel = info
            .s_unreclaim
            .saturating_add(info.kernel_stack)
            .saturating_add(info.page_tables)
            .min(used);
        let free = info.free.min(total - used);
        MemoryBuckets {
            total,
            app: used - kernel,
            kernel,
            cached: total - used - free,
            free,
            swap_used: info.swap_total.saturating_sub(info.swap_free),
            compressed: info.zswap,
        }
    }

    /// `MemTotal − MemAvailable`: memory that cannot be handed out without evicting or
    /// swapping something.
    pub fn used(&self) -> u64 {
        self.app + self.kernel
    }

    pub fn usage_percent(&self) -> f64 {
        self.fraction(self.used()) * 100.0
    }

    /// `part` as a share of total memory, 0–1. Zero when the total is unknown.
    pub fn fraction(&self, part: u64) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (part as f64 / self.total as f64).clamp(0.0, 1.0)
        }
    }
}
```

- [x] **Step 4: Run it and watch it pass**

Run: `cargo test -p statsbar-core --test memory`
Expected: PASS — `test result: ok. 12 passed`.

Run: `./scripts/check.sh`
Expected: PASS.

- [x] **Step 5: Commit checkpoint**

Ready to commit: `crates/core/src/lib.rs crates/core/src/memory.rs crates/core/tests/memory.rs`
— `feat(core): parse /proc/meminfo into memory bar buckets`

---

## Task 4: Network throughput and usage history

**Files:**
- Create: `crates/core/tests/throughput.rs`, `crates/core/src/throughput.rs`,
  `crates/core/tests/history.rs`, `crates/core/src/history.rs`
- Modify: `crates/core/src/lib.rs` (add two module lines)

**Interfaces:**
- Produces:
  - `pub struct Sample { pub interface: String, pub rx_bytes: u64, pub tx_bytes: u64 }`
  - `pub struct Reading { pub download_total: u64, pub upload_total: u64, pub download_rate: f64, pub upload_rate: f64 }` — `Copy, Default`
  - `pub struct ThroughputTracker` — `Default`; `pub const MINIMUM_INTERVAL_SECONDS: f64 = 0.2`;
    `pub fn update(&mut self, sample: Option<&Sample>, now_nanos: u64) -> Reading`; `pub fn reading(&self) -> Reading`
  - `pub struct UsageHistory`; `pub fn new(capacity: usize) -> UsageHistory`; `capacity()`, `len()`,
    `is_empty()`, `samples() -> impl Iterator<Item = f64>`, `push(&mut self, value: f64)`,
    `plot_points(&self, width: f64, height: f64, maximum: f64) -> Vec<(f64, f64)>`

- [x] **Step 1: Write the failing tests**

`crates/core/tests/throughput.rs`:

```rust
use statsbar_core::throughput::{Reading, Sample, ThroughputTracker};

const SECOND: u64 = 1_000_000_000;

fn sample(interface: &str, rx_bytes: u64, tx_bytes: u64) -> Sample {
    Sample {
        interface: interface.to_string(),
        rx_bytes,
        tx_bytes,
    }
}

#[test]
fn the_first_sample_sets_the_baseline_and_reports_nothing() {
    let mut tracker = ThroughputTracker::default();
    let reading = tracker.update(Some(&sample("wlo1", 5_000, 1_000)), 10 * SECOND);
    assert_eq!(reading, Reading::default());
}

#[test]
fn rate_is_the_counter_delta_over_the_elapsed_time() {
    let mut tracker = ThroughputTracker::default();
    tracker.update(Some(&sample("wlo1", 5_000, 1_000)), 10 * SECOND);
    let reading = tracker.update(Some(&sample("wlo1", 9_000, 2_000)), 12 * SECOND);
    assert_eq!(reading.download_rate, 2_000.0);
    assert_eq!(reading.upload_rate, 500.0);
    assert_eq!(reading.download_total, 4_000);
    assert_eq!(reading.upload_total, 1_000);
    assert_eq!(tracker.reading(), reading);
}

#[test]
fn a_sample_inside_the_minimum_interval_keeps_the_previous_rate() {
    let mut tracker = ThroughputTracker::default();
    tracker.update(Some(&sample("wlo1", 0, 0)), 0);
    tracker.update(Some(&sample("wlo1", 1_000, 0)), SECOND);
    // 0.1 s later: too short to measure, so the rate stays and the total still advances.
    let reading = tracker.update(Some(&sample("wlo1", 9_000, 0)), SECOND + SECOND / 10);
    assert_eq!(reading.download_rate, 1_000.0);
    assert_eq!(reading.download_total, 9_000);
    // The skipped sample did not move the reference point: the next rate covers 1.0 s → 2.0 s.
    let reading = tracker.update(Some(&sample("wlo1", 11_000, 0)), 2 * SECOND);
    assert_eq!(reading.download_rate, 10_000.0);
}

#[test]
fn a_clock_that_did_not_advance_keeps_the_previous_rate() {
    let mut tracker = ThroughputTracker::default();
    tracker.update(Some(&sample("wlo1", 0, 0)), SECOND);
    tracker.update(Some(&sample("wlo1", 500, 0)), 2 * SECOND);
    let reading = tracker.update(Some(&sample("wlo1", 900, 0)), 2 * SECOND);
    assert_eq!(reading.download_rate, 500.0);
}

#[test]
fn switching_interface_restarts_the_session() {
    let mut tracker = ThroughputTracker::default();
    tracker.update(Some(&sample("wlo1", 1_000, 1_000)), 0);
    tracker.update(Some(&sample("wlo1", 3_000, 2_000)), SECOND);
    let reading = tracker.update(Some(&sample("eno2", 70_000, 80_000)), 2 * SECOND);
    assert_eq!(reading, Reading::default()); // no rate carried over from wlo1
    let reading = tracker.update(Some(&sample("eno2", 71_000, 80_500)), 3 * SECOND);
    assert_eq!(reading.download_rate, 1_000.0);
    assert_eq!(reading.upload_rate, 500.0);
    assert_eq!(reading.download_total, 1_000);
}

#[test]
fn counters_below_the_baseline_restart_the_session() {
    let mut tracker = ThroughputTracker::default();
    tracker.update(Some(&sample("wlo1", 50_000, 50_000)), 0);
    tracker.update(Some(&sample("wlo1", 60_000, 50_000)), SECOND);
    // The interface went down and up: its counters restarted near zero.
    let reading = tracker.update(Some(&sample("wlo1", 100, 40)), 2 * SECOND);
    assert_eq!(reading, Reading::default());
}

#[test]
fn no_interface_zeroes_the_rates_and_keeps_the_totals() {
    let mut tracker = ThroughputTracker::default();
    tracker.update(Some(&sample("wlo1", 0, 0)), 0);
    tracker.update(Some(&sample("wlo1", 4_000, 2_000)), SECOND);
    let reading = tracker.update(None, 2 * SECOND);
    assert_eq!(reading.download_rate, 0.0);
    assert_eq!(reading.upload_rate, 0.0);
    assert_eq!(reading.download_total, 4_000);
    assert_eq!(reading.upload_total, 2_000);
}

#[test]
fn the_first_sample_after_a_gap_does_not_report_a_rate_across_the_gap() {
    let mut tracker = ThroughputTracker::default();
    tracker.update(Some(&sample("wlo1", 0, 0)), 0);
    tracker.update(Some(&sample("wlo1", 1_000, 0)), SECOND);
    tracker.update(None, 2 * SECOND);
    // An hour offline, during which the counter moved: no rate may be derived from it.
    let reading = tracker.update(Some(&sample("wlo1", 900_000, 0)), 3_600 * SECOND);
    assert_eq!(reading.download_rate, 0.0);
    let reading = tracker.update(Some(&sample("wlo1", 903_000, 0)), 3_601 * SECOND);
    assert_eq!(reading.download_rate, 3_000.0);
}
```

`crates/core/tests/history.rs`:

```rust
use statsbar_core::history::UsageHistory;

#[test]
fn keeps_only_the_newest_samples_once_full() {
    let mut history = UsageHistory::new(3);
    for value in [1.0, 2.0, 3.0, 4.0, 5.0] {
        history.push(value);
    }
    assert_eq!(history.samples().collect::<Vec<_>>(), vec![3.0, 4.0, 5.0]);
    assert_eq!(history.len(), 3);
    assert_eq!(history.capacity(), 3);
}

#[test]
fn ignores_values_that_are_not_finite() {
    let mut history = UsageHistory::new(3);
    history.push(f64::NAN);
    history.push(f64::INFINITY);
    history.push(f64::NEG_INFINITY);
    assert!(history.is_empty());
    history.push(7.0);
    assert_eq!(history.samples().collect::<Vec<_>>(), vec![7.0]);
}

#[test]
fn a_zero_capacity_is_raised_to_one() {
    let mut history = UsageHistory::new(0);
    history.push(1.0);
    history.push(2.0);
    assert_eq!(history.capacity(), 1);
    assert_eq!(history.samples().collect::<Vec<_>>(), vec![2.0]);
}

#[test]
fn a_full_window_spans_the_whole_width() {
    let mut history = UsageHistory::new(3);
    for value in [0.0, 50.0, 100.0] {
        history.push(value);
    }
    // y grows downwards: 0 % is the bottom edge, 100 % the top.
    assert_eq!(
        history.plot_points(100.0, 40.0, 100.0),
        vec![(0.0, 40.0), (50.0, 20.0), (100.0, 0.0)]
    );
}

#[test]
fn a_filling_window_grows_in_from_the_right() {
    let mut history = UsageHistory::new(5);
    history.push(100.0);
    history.push(50.0);
    assert_eq!(
        history.plot_points(100.0, 40.0, 100.0),
        vec![(75.0, 0.0), (100.0, 20.0)]
    );
}

#[test]
fn values_outside_the_scale_are_clamped_to_the_chart() {
    let mut history = UsageHistory::new(2);
    history.push(-20.0);
    history.push(250.0);
    assert_eq!(
        history.plot_points(10.0, 40.0, 100.0),
        vec![(0.0, 40.0), (10.0, 0.0)]
    );
}

#[test]
fn nothing_is_plotted_without_samples_or_a_positive_scale() {
    let mut history = UsageHistory::new(3);
    assert!(history.plot_points(100.0, 40.0, 100.0).is_empty());
    history.push(10.0);
    assert!(history.plot_points(100.0, 40.0, 0.0).is_empty());
    assert!(history.plot_points(100.0, 40.0, f64::NAN).is_empty());
}

#[test]
fn a_single_slot_window_plots_on_the_right_edge() {
    let mut history = UsageHistory::new(1);
    history.push(25.0);
    assert_eq!(history.plot_points(80.0, 40.0, 100.0), vec![(80.0, 30.0)]);
}
```

- [x] **Step 2: Run them and watch them fail**

Run: `cargo test -p statsbar-core --test throughput --test history`
Expected: FAIL — `unresolved import `statsbar_core::throughput`` and `` `statsbar_core::history` ``.

- [x] **Step 3: Implement**

Add to `crates/core/src/lib.rs`, so the module list reads as below. `cargo fmt` keeps module
declarations alphabetical, so the order is not a choice:

```rust
pub mod cpu;
pub mod history;
pub mod memory;
pub mod throughput;
```

`crates/core/src/throughput.rs`:

```rust
//! Network throughput: interface byte counters in, rates and session totals out.

/// One reading of an interface's cumulative counters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sample {
    pub interface: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Reading {
    /// Bytes since the session baseline.
    pub download_total: u64,
    pub upload_total: u64,
    /// Bytes per second over the last measured interval.
    pub download_rate: f64,
    pub upload_rate: f64,
}

/// Turns successive counter samples into rates. The caller supplies the timestamp from a
/// monotonic clock, which keeps this type free of I/O and lets tests choose the interval.
#[derive(Debug, Clone, Default)]
pub struct ThroughputTracker {
    baseline: Option<Sample>,
    last_rx: u64,
    last_tx: u64,
    last_sample_nanos: Option<u64>,
    reading: Reading,
}

impl ThroughputTracker {
    /// Two samples closer together than this give a rate dominated by timer jitter, so the
    /// previous rate is kept until enough time has passed.
    pub const MINIMUM_INTERVAL_SECONDS: f64 = 0.2;

    pub fn reading(&self) -> Reading {
        self.reading
    }

    fn prime(&mut self, sample: &Sample, now_nanos: u64) {
        // A new session has no interval yet: a rate carried over would be the previous
        // interface's, shown against the new one.
        self.reading.download_rate = 0.0;
        self.reading.upload_rate = 0.0;
        self.baseline = Some(sample.clone());
        self.last_rx = sample.rx_bytes;
        self.last_tx = sample.tx_bytes;
        self.last_sample_nanos = Some(now_nanos);
    }

    /// Feeds the next sample. `None` means there is no active interface: the rates drop to
    /// zero while the session totals are kept.
    pub fn update(&mut self, sample: Option<&Sample>, now_nanos: u64) -> Reading {
        let Some(sample) = sample else {
            self.reading.download_rate = 0.0;
            self.reading.upload_rate = 0.0;
            self.last_sample_nanos = None;
            return self.reading;
        };
        // A different interface, or counters below the baseline (the interface was reset),
        // starts a new session: totals measured against the old baseline would be garbage.
        let restart = match &self.baseline {
            None => true,
            Some(base) => {
                base.interface != sample.interface
                    || sample.rx_bytes < base.rx_bytes
                    || sample.tx_bytes < base.tx_bytes
            }
        };
        if restart {
            self.prime(sample, now_nanos);
        }
        let (base_rx, base_tx) = self
            .baseline
            .as_ref()
            .map_or((sample.rx_bytes, sample.tx_bytes), |b| {
                (b.rx_bytes, b.tx_bytes)
            });
        self.reading.download_total = sample.rx_bytes.saturating_sub(base_rx);
        self.reading.upload_total = sample.tx_bytes.saturating_sub(base_tx);

        let Some(last) = self.last_sample_nanos else {
            self.last_sample_nanos = Some(now_nanos);
            self.last_rx = sample.rx_bytes;
            self.last_tx = sample.tx_bytes;
            return self.reading;
        };
        if now_nanos <= last {
            return self.reading;
        }
        let seconds = (now_nanos - last) as f64 / 1_000_000_000.0;
        if seconds <= Self::MINIMUM_INTERVAL_SECONDS {
            return self.reading;
        }
        self.reading.download_rate = sample.rx_bytes.saturating_sub(self.last_rx) as f64 / seconds;
        self.reading.upload_rate = sample.tx_bytes.saturating_sub(self.last_tx) as f64 / seconds;
        self.last_rx = sample.rx_bytes;
        self.last_tx = sample.tx_bytes;
        self.last_sample_nanos = Some(now_nanos);
        self.reading
    }
}
```

`crates/core/src/history.rs`:

```rust
//! A fixed-length window of recent samples, and the geometry of the chart drawn from it.

use std::collections::VecDeque;

/// The last `capacity` samples of a metric, oldest first.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageHistory {
    capacity: usize,
    samples: VecDeque<f64>,
}

impl UsageHistory {
    /// A capacity of zero is raised to one, so the window always holds the latest sample.
    pub fn new(capacity: usize) -> UsageHistory {
        let capacity = capacity.max(1);
        UsageHistory {
            capacity,
            samples: VecDeque::with_capacity(capacity),
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn samples(&self) -> impl Iterator<Item = f64> + '_ {
        self.samples.iter().copied()
    }

    /// Appends a sample, dropping the oldest once the window is full. NaN and infinities are
    /// ignored: a single one would otherwise poison the chart's path.
    pub fn push(&mut self, value: f64) {
        if !value.is_finite() {
            return;
        }
        if self.samples.len() == self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(value);
    }

    /// Chart points as `(x, y)` with the origin at the top-left, as Cairo draws.
    ///
    /// The newest sample sits on the right edge and older ones step left by a fixed amount,
    /// so a window that is still filling grows in from the right instead of stretching.
    /// Values are clamped into `0..=maximum`; a NaN or non-positive `maximum` plots nothing.
    pub fn plot_points(&self, width: f64, height: f64, maximum: f64) -> Vec<(f64, f64)> {
        if maximum.is_nan() || maximum <= 0.0 || self.samples.is_empty() {
            return Vec::new();
        }
        let step = if self.capacity > 1 {
            width / (self.capacity - 1) as f64
        } else {
            0.0
        };
        let last = self.samples.len() - 1;
        self.samples
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let fraction = (value / maximum).clamp(0.0, 1.0);
                (
                    width - (last - index) as f64 * step,
                    height - fraction * height,
                )
            })
            .collect()
    }
}
```

- [x] **Step 4: Run them and watch them pass**

Run: `cargo test -p statsbar-core --test throughput --test history`
Expected: PASS — `8 passed` twice.

Run: `./scripts/check.sh`
Expected: PASS.

- [x] **Step 5: Commit checkpoint**

Ready to commit: `crates/core/src/lib.rs crates/core/src/throughput.rs crates/core/src/history.rs crates/core/tests/throughput.rs crates/core/tests/history.rs`
— `feat(core): add throughput tracker and usage history window`

---

## Task 5: Hot-sensor alert policy

**Files:**
- Create: `crates/core/tests/alert.rs`, `crates/core/src/alert.rs`
- Modify: `crates/core/src/lib.rs` (add one module line)

**Interfaces:**
- Produces:
  - `pub struct HotAlertPolicy { pub threshold_c: f64, pub rearm_margin_c: f64, /* private state */ }`
  - `pub fn new(threshold_c: f64, rearm_margin_c: f64) -> HotAlertPolicy`
  - `pub fn sensors_to_alert(&mut self, readings: &[(&str, Option<f64>)], enabled: bool) -> Vec<(String, f64)>`

- [x] **Step 1: Write the failing test**

`crates/core/tests/alert.rs`:

```rust
use statsbar_core::alert::HotAlertPolicy;

fn policy() -> HotAlertPolicy {
    HotAlertPolicy::new(39.0, 2.0)
}

#[test]
fn alerts_once_when_the_threshold_is_reached() {
    let mut policy = policy();
    assert!(
        policy
            .sensors_to_alert(&[("phone", Some(38.9))], true)
            .is_empty()
    );
    assert_eq!(
        policy.sensors_to_alert(&[("phone", Some(39.0))], true),
        vec![("phone".to_string(), 39.0)]
    );
    assert!(
        policy
            .sensors_to_alert(&[("phone", Some(41.0))], true)
            .is_empty()
    );
}

#[test]
fn hovering_just_below_the_threshold_does_not_rearm() {
    let mut policy = policy();
    policy.sensors_to_alert(&[("phone", Some(40.0))], true);
    // 37.1 is below the threshold but inside the 2-degree margin.
    assert!(
        policy
            .sensors_to_alert(&[("phone", Some(37.1))], true)
            .is_empty()
    );
    assert!(
        policy
            .sensors_to_alert(&[("phone", Some(39.5))], true)
            .is_empty()
    );
}

#[test]
fn cooling_by_the_margin_rearms() {
    let mut policy = policy();
    policy.sensors_to_alert(&[("phone", Some(40.0))], true);
    assert!(
        policy
            .sensors_to_alert(&[("phone", Some(37.0))], true)
            .is_empty()
    );
    assert_eq!(
        policy.sensors_to_alert(&[("phone", Some(39.2))], true),
        vec![("phone".to_string(), 39.2)]
    );
}

#[test]
fn a_missing_reading_neither_alerts_nor_rearms() {
    let mut policy = policy();
    policy.sensors_to_alert(&[("phone", Some(40.0))], true);
    assert!(policy.sensors_to_alert(&[("phone", None)], true).is_empty());
    assert!(
        policy
            .sensors_to_alert(&[("phone", Some(40.0))], true)
            .is_empty()
    );
}

#[test]
fn a_sensor_that_disappears_is_forgotten() {
    let mut policy = policy();
    policy.sensors_to_alert(&[("phone", Some(40.0))], true);
    assert!(policy.sensors_to_alert(&[], true).is_empty());
    assert_eq!(
        policy.sensors_to_alert(&[("phone", Some(40.0))], true),
        vec![("phone".to_string(), 40.0)]
    );
}

#[test]
fn sensors_are_tracked_independently_and_reported_in_input_order() {
    let mut policy = policy();
    assert_eq!(
        policy.sensors_to_alert(
            &[
                ("tablet", Some(45.0)),
                ("cpu", Some(20.0)),
                ("phone", Some(39.0))
            ],
            true
        ),
        vec![("tablet".to_string(), 45.0), ("phone".to_string(), 39.0)]
    );
    assert_eq!(
        policy.sensors_to_alert(
            &[
                ("tablet", Some(45.0)),
                ("cpu", Some(50.0)),
                ("phone", Some(39.0))
            ],
            true
        ),
        vec![("cpu".to_string(), 50.0)]
    );
}

#[test]
fn disabling_silences_alerts_and_rearms_everything() {
    let mut policy = policy();
    policy.sensors_to_alert(&[("phone", Some(40.0))], true);
    assert!(
        policy
            .sensors_to_alert(&[("phone", Some(40.0))], false)
            .is_empty()
    );
    assert_eq!(
        policy.sensors_to_alert(&[("phone", Some(40.0))], true),
        vec![("phone".to_string(), 40.0)]
    );
}
```

- [x] **Step 2: Run it and watch it fail**

Run: `cargo test -p statsbar-core --test alert`
Expected: FAIL — `error[E0432]: unresolved import `statsbar_core::alert``.

- [x] **Step 3: Implement**

Add to `crates/core/src/lib.rs`, so the module list reads:

```rust
pub mod alert;
pub mod cpu;
pub mod history;
pub mod memory;
pub mod throughput;
```

`crates/core/src/alert.rs`:

```rust
//! When to raise a "running hot" notification, and when to stay quiet.

use std::collections::BTreeSet;

/// Decides which sensors deserve a notification on this poll.
///
/// A sensor alerts once when it reaches the threshold and not again until it has cooled by
/// the re-arm margin. Without that margin a reading hovering at the threshold would fire on
/// every poll.
#[derive(Debug, Clone, PartialEq)]
pub struct HotAlertPolicy {
    pub threshold_c: f64,
    pub rearm_margin_c: f64,
    alerted: BTreeSet<String>,
}

impl HotAlertPolicy {
    pub fn new(threshold_c: f64, rearm_margin_c: f64) -> HotAlertPolicy {
        HotAlertPolicy {
            threshold_c,
            rearm_margin_c,
            alerted: BTreeSet::new(),
        }
    }

    /// `readings` holds every sensor currently present as `(id, temperature)`; a `None`
    /// temperature means the sensor is there but gave no value this time.
    ///
    /// Returns the sensors to notify about, in input order.
    pub fn sensors_to_alert(
        &mut self,
        readings: &[(&str, Option<f64>)],
        enabled: bool,
    ) -> Vec<(String, f64)> {
        // A sensor that disappeared (a phone was unplugged) is forgotten, so that plugging a
        // hot one back in alerts again.
        self.alerted
            .retain(|id| readings.iter().any(|(present, _)| present == id));
        if !enabled {
            // Switching alerts off and on again re-arms everything.
            self.alerted.clear();
            return Vec::new();
        }
        let mut fire = Vec::new();
        for &(id, temperature) in readings {
            // No value is not evidence of cooling: the armed state is left alone.
            let Some(temperature) = temperature else {
                continue;
            };
            if temperature >= self.threshold_c {
                if self.alerted.insert(id.to_string()) {
                    fire.push((id.to_string(), temperature));
                }
            } else if temperature <= self.threshold_c - self.rearm_margin_c {
                self.alerted.remove(id);
            }
        }
        fire
    }
}
```

- [x] **Step 4: Run it and watch it pass**

Run: `cargo test -p statsbar-core --test alert`
Expected: PASS — `test result: ok. 7 passed`.

Run: `./scripts/check.sh`
Expected: PASS.

- [x] **Step 5: Prove the re-arm margin is tested**

Upstream's rule for trusting a guard: break it and watch the suite go red. In `alert.rs`, change
`self.threshold_c - self.rearm_margin_c` to `self.threshold_c`, then run
`cargo test -p statsbar-core --test alert`.
Expected: FAIL in `hovering_just_below_the_threshold_does_not_rearm`. Restore the line and rerun: PASS.

- [x] **Step 6: Commit checkpoint**

Ready to commit: `crates/core/src/lib.rs crates/core/src/alert.rs crates/core/tests/alert.rs`
— `feat(core): add hot-sensor alert policy with re-arm margin`

---

## Task 6: Formatting

**Files:**
- Create: `crates/core/tests/format.rs`, `crates/core/src/format.rs`
- Modify: `crates/core/src/lib.rs` (add one module line)

**Interfaces:**
- Produces:
  - `pub fn rounded_int(value: f64) -> i64`
  - `pub enum HealthBand { Good, Fair, Poor }`; `pub fn health_band(percent: f64) -> HealthBand`
  - `pub fn fmt_minutes(minutes: i64) -> String`; `pub fn fmt_uptime(seconds: f64) -> String`
  - `pub fn fmt_gb(bytes: u64) -> String`; `pub fn fmt_process_memory(bytes: u64) -> String`
  - `pub fn fmt_bytes(bytes: u64) -> String`; `pub fn fmt_rate(bytes_per_second: f64) -> String`
  - `pub fn menu_bar_rate(bytes_per_second: f64) -> String`; `pub const MENU_BAR_RATE_WIDEST: &str = "999 MB/s"`
  - `pub fn flag_emoji(country_code: &str) -> String`

- [x] **Step 1: Write the failing test**

`crates/core/tests/format.rs`:

```rust
use statsbar_core::format::{
    HealthBand, MENU_BAR_RATE_WIDEST, flag_emoji, fmt_bytes, fmt_gb, fmt_minutes,
    fmt_process_memory, fmt_rate, fmt_uptime, health_band, menu_bar_rate, rounded_int,
};

const MIB: u64 = 1_048_576;

#[test]
fn rounded_int_survives_values_that_are_not_numbers() {
    assert_eq!(rounded_int(2.5), 3);
    assert_eq!(rounded_int(-2.5), -3);
    assert_eq!(rounded_int(f64::NAN), 0);
    assert_eq!(rounded_int(f64::INFINITY), i64::MAX);
    assert_eq!(rounded_int(f64::NEG_INFINITY), i64::MIN);
}

#[test]
fn health_bands_change_at_80_and_60() {
    assert_eq!(health_band(80.0), HealthBand::Good);
    assert_eq!(health_band(79.9), HealthBand::Fair);
    assert_eq!(health_band(60.0), HealthBand::Fair);
    assert_eq!(health_band(59.9), HealthBand::Poor);
    assert_eq!(health_band(f64::NAN), HealthBand::Poor);
}

#[test]
fn minutes_are_shown_as_hours_and_padded_minutes() {
    assert_eq!(fmt_minutes(125), "2h 05m");
    assert_eq!(fmt_minutes(59), "0h 59m");
    assert_eq!(fmt_minutes(0), "0h 00m");
    assert_eq!(fmt_minutes(-7), "0h 00m");
}

#[test]
fn uptime_shows_its_two_largest_units_with_plurals() {
    assert_eq!(fmt_uptime(90_061.0), "1 day, 1 hour");
    assert_eq!(fmt_uptime(2.0 * 86_400.0), "2 days, 0 hours");
    assert_eq!(fmt_uptime(7_260.0), "2 hours, 1 minute");
    assert_eq!(fmt_uptime(59.9), "0 minutes");
    assert_eq!(fmt_uptime(-5.0), "0 minutes");
    assert_eq!(fmt_uptime(f64::NAN), "0 minutes");
}

#[test]
fn installed_memory_uses_binary_gigabytes() {
    assert_eq!(fmt_gb(16 * 1024 * MIB), "16.00 GB");
    assert_eq!(fmt_gb(512 * MIB), "0.50 GB");
}

#[test]
fn process_memory_switches_to_gigabytes_at_a_rounded_1024_mb() {
    assert_eq!(fmt_process_memory(512 * MIB), "512 MB");
    assert_eq!(fmt_process_memory(1023 * MIB + MIB / 4), "1023 MB");
    // 1023.75 MiB rounds to 1024, which must not be printed as "1024 MB".
    assert_eq!(fmt_process_memory(1023 * MIB + 3 * MIB / 4), "1.00 GB");
    assert_eq!(fmt_process_memory(1536 * MIB), "1.50 GB");
}

#[test]
fn bytes_use_decimal_units_and_never_print_four_digits() {
    assert_eq!(fmt_bytes(0), "0 bytes");
    assert_eq!(fmt_bytes(999), "999 bytes");
    assert_eq!(fmt_bytes(1_000), "1.0 KB");
    assert_eq!(fmt_bytes(999_949), "999.9 KB");
    assert_eq!(fmt_bytes(999_950), "1.0 MB");
    assert_eq!(fmt_bytes(2_500_000_000), "2.5 GB");
    assert_eq!(fmt_bytes(u64::MAX), "18446744.1 TB");
}

#[test]
fn rate_rounds_to_whole_bytes_and_clamps_negatives() {
    assert_eq!(fmt_rate(1_536.4), "1.5 KB/s");
    // Rounds rather than truncates: truncation would print "999 bytes/s".
    assert_eq!(fmt_rate(999.6), "1.0 KB/s");
    assert_eq!(fmt_rate(-10.0), "0 bytes/s");
    assert_eq!(fmt_rate(f64::NAN), "0 bytes/s");
}

#[test]
fn menu_bar_rate_never_exceeds_three_digits() {
    assert_eq!(menu_bar_rate(0.0), "0 B/s");
    assert_eq!(menu_bar_rate(999.4), "999 B/s");
    assert_eq!(menu_bar_rate(999.5), "1 KB/s");
    assert_eq!(menu_bar_rate(250_000.0), "250 KB/s");
    assert_eq!(menu_bar_rate(999_499.0), "999 KB/s");
    assert_eq!(menu_bar_rate(999_500.0), "1.0 MB/s");
    assert_eq!(menu_bar_rate(1_500_000.0), "1.5 MB/s");
    assert_eq!(menu_bar_rate(9_940_000.0), "9.9 MB/s");
    assert_eq!(menu_bar_rate(9_950_000.0), "10 MB/s");
    assert_eq!(menu_bar_rate(999_400_000.0), "999 MB/s");
    assert_eq!(menu_bar_rate(999_500_000.0), "1.0 GB/s");
    assert_eq!(menu_bar_rate(-3.0), "0 B/s");
    assert_eq!(menu_bar_rate(f64::NAN), "0 B/s");
}

#[test]
fn the_reserved_width_sample_is_the_widest_label_below_a_gigabyte() {
    let widest = MENU_BAR_RATE_WIDEST.len();
    for rate in [
        0.0,
        999.4,
        999_499.0,
        9_940_000.0,
        12_300_000.0,
        999_400_000.0,
    ] {
        assert!(
            menu_bar_rate(rate).len() <= widest,
            "{}",
            menu_bar_rate(rate)
        );
    }
}

#[test]
fn country_codes_become_flags_and_anything_else_is_empty() {
    assert_eq!(flag_emoji("VN"), "🇻🇳");
    assert_eq!(flag_emoji("vn"), "🇻🇳");
    assert_eq!(flag_emoji("V"), "");
    assert_eq!(flag_emoji("VNM"), "");
    assert_eq!(flag_emoji("V1"), "");
    assert_eq!(flag_emoji("ÉÉ"), "");
    assert_eq!(flag_emoji(""), "");
}
```

- [x] **Step 2: Run it and watch it fail**

Run: `cargo test -p statsbar-core --test format`
Expected: FAIL — `error[E0432]: unresolved import `statsbar_core::format``.

- [x] **Step 3: Implement**

Add to `crates/core/src/lib.rs`, so the module list reads:

```rust
pub mod alert;
pub mod cpu;
pub mod format;
pub mod history;
pub mod memory;
pub mod throughput;
```

`crates/core/src/format.rs`:

```rust
//! Number-to-text formatting shared by the tray labels and the panel.

/// Rounds to the nearest integer. NaN reads as zero and out-of-range values saturate, so a
/// broken sensor reading can never abort a redraw.
pub fn rounded_int(value: f64) -> i64 {
    // `as` on a float is defined to do exactly that: NaN → 0, ±∞ → the nearest bound.
    value.round() as i64
}

/// How a battery health percentage is coloured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthBand {
    Good,
    Fair,
    Poor,
}

pub fn health_band(percent: f64) -> HealthBand {
    if percent >= 80.0 {
        HealthBand::Good
    } else if percent >= 60.0 {
        HealthBand::Fair
    } else {
        HealthBand::Poor
    }
}

/// `125` → `"2h 05m"`. Negative durations read as zero.
pub fn fmt_minutes(minutes: i64) -> String {
    let minutes = minutes.max(0);
    format!("{}h {:02}m", minutes / 60, minutes % 60)
}

/// The two most significant units of an uptime: `"1 day, 1 hour"`, `"2 hours, 1 minute"`.
pub fn fmt_uptime(seconds: f64) -> String {
    fn plural(count: i64, unit: &str) -> String {
        format!("{count} {unit}{}", if count == 1 { "" } else { "s" })
    }
    let total = rounded_int(seconds.max(0.0).floor());
    let (days, hours, minutes) = (total / 86_400, total % 86_400 / 3_600, total % 3_600 / 60);
    if days > 0 {
        format!("{}, {}", plural(days, "day"), plural(hours, "hour"))
    } else if hours > 0 {
        format!("{}, {}", plural(hours, "hour"), plural(minutes, "minute"))
    } else {
        plural(minutes, "minute")
    }
}

const BYTES_PER_MIB: f64 = 1_048_576.0;
const BYTES_PER_GIB: f64 = 1_073_741_824.0;

/// Installed-memory style: binary gigabytes with two decimals.
pub fn fmt_gb(bytes: u64) -> String {
    format!("{:.2} GB", bytes as f64 / BYTES_PER_GIB)
}

/// A process's memory: whole megabytes, switching to gigabytes once the rounded figure
/// reaches 1024 so the list never shows "1024 MB".
pub fn fmt_process_memory(bytes: u64) -> String {
    let mib = bytes as f64 / BYTES_PER_MIB;
    if mib.round() >= 1024.0 {
        format!("{:.2} GB", mib / 1024.0)
    } else {
        format!("{mib:.0} MB")
    }
}

/// Decimal units, as network tools count: `"999 bytes"`, `"1.0 KB"`, `"2.5 GB"`.
pub fn fmt_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["bytes", "KB", "MB", "GB", "TB"];
    // Promote just below 1000 so one-decimal rounding can never print "1000.0 KB".
    const PROMOTE_ABOVE: f64 = 999.95;
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= PROMOTE_ABOVE && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} bytes")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub fn fmt_rate(bytes_per_second: f64) -> String {
    let whole = rounded_int(bytes_per_second).max(0) as u64;
    format!("{}/s", fmt_bytes(whole))
}

/// The tray label's rate: at most three digits, so the widest text is [`MENU_BAR_RATE_WIDEST`]
/// and the item keeps one width at every rate instead of shifting its neighbours.
pub fn menu_bar_rate(bytes_per_second: f64) -> String {
    // Same reasoning as PROMOTE_ABOVE, for whole-number rounding: 999.5 would print "1000".
    const PROMOTE_ABOVE: f64 = 999.5;
    fn banded(value: f64, unit: &str) -> String {
        // One decimal only while it fits in three characters ("9.9"), whole numbers after.
        if value < 9.95 {
            format!("{value:.1} {unit}")
        } else {
            format!("{value:.0} {unit}")
        }
    }
    // `max` discards a NaN operand, so a broken reading prints as zero.
    let rate = bytes_per_second.max(0.0);
    if rate < PROMOTE_ABOVE {
        return format!("{rate:.0} B/s");
    }
    let kilobytes = rate / 1000.0;
    if kilobytes < PROMOTE_ABOVE {
        return format!("{kilobytes:.0} KB/s");
    }
    let megabytes = kilobytes / 1000.0;
    if megabytes < PROMOTE_ABOVE {
        return banded(megabytes, "MB/s");
    }
    banded(megabytes / 1000.0, "GB/s")
}

/// The widest string [`menu_bar_rate`] produces below 1 GB/s; the tray reserves room for it.
pub const MENU_BAR_RATE_WIDEST: &str = "999 MB/s";

/// A two-letter ISO-3166 country code as its flag emoji. Anything else gives an empty
/// string, so a malformed reply from the lookup service shows no flag instead of garbage.
pub fn flag_emoji(country_code: &str) -> String {
    const REGIONAL_INDICATOR_A: u32 = 0x1F1E6;
    if country_code.len() != 2 || !country_code.bytes().all(|b| b.is_ascii_alphabetic()) {
        return String::new();
    }
    country_code
        .bytes()
        .filter_map(|b| {
            char::from_u32(REGIONAL_INDICATOR_A + u32::from(b.to_ascii_uppercase() - b'A'))
        })
        .collect()
}
```

- [x] **Step 4: Run it and watch it pass**

Run: `cargo test -p statsbar-core --test format`
Expected: PASS — `test result: ok. 11 passed`.

Run: `./scripts/check.sh`
Expected: PASS.

- [x] **Step 5: Commit checkpoint**

Ready to commit: `crates/core/src/lib.rs crates/core/src/format.rs crates/core/tests/format.rs`
— `feat(core): add byte, rate, duration and flag formatting`

---

## Task 7: Verification & Docs

**Files:**
- Modify: `README.md` (replace the whole file with the content in Step 2)

- [x] **Step 1: Check the module list**

`crates/core/src/lib.rs` must end with exactly:

```rust
pub mod alert;
pub mod cpu;
pub mod format;
pub mod history;
pub mod memory;
pub mod throughput;
```

- [x] **Step 2: Write the README** — deviated: kept the Plans and License sections from the earlier README (user decision)

`README.md`:

````markdown
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
````

- [x] **Step 3: Run the full gate**

Run: `./scripts/check.sh`
Expected: PASS — the test binaries for alert, cpu, format, history, memory and throughput report `7`,
`10`, `11`, `8`, `12` and `8` passed (56 in all), then `core purity: ok`.

- [x] **Step 4: End-to-end check against this machine**

The crate has no binary, so the end-to-end check feeds it this machine's real files once. Create
`crates/core/examples/live.rs`:

```rust
//! Prints what the core makes of this machine's real /proc files. A manual check, not a test:
//! its output depends on the machine. The example may read files; the crate under test may not.

use statsbar_core::cpu::{CpuLoad, parse_proc_stat};
use statsbar_core::format::fmt_gb;
use statsbar_core::memory::{MemoryBuckets, parse_meminfo};

fn main() -> std::io::Result<()> {
    let first = std::fs::read_to_string("/proc/stat")?;
    std::thread::sleep(std::time::Duration::from_secs(1));
    let second = std::fs::read_to_string("/proc/stat")?;
    match (parse_proc_stat(&first), parse_proc_stat(&second)) {
        (Some(previous), Some(current)) => match CpuLoad::between(&previous, &current) {
            Some(load) => println!(
                "cpu: {:.1}% over {} cores",
                load.usage_percent(),
                current.len()
            ),
            None => println!("cpu: core count changed between samples"),
        },
        _ => println!("cpu: /proc/stat did not parse"),
    }
    match parse_meminfo(&std::fs::read_to_string("/proc/meminfo")?) {
        Some(info) => {
            let buckets = MemoryBuckets::from_meminfo(&info);
            println!(
                "memory: {:.1}% used — app {}, kernel {}, cached {}, free {}, total {}",
                buckets.usage_percent(),
                fmt_gb(buckets.app),
                fmt_gb(buckets.kernel),
                fmt_gb(buckets.cached),
                fmt_gb(buckets.free),
                fmt_gb(buckets.total),
            );
        }
        None => println!("memory: /proc/meminfo did not parse"),
    }
    Ok(())
}
```

Run: `cargo run -p statsbar-core --example live`
Then, straight after, run:

```bash
free -b | awk 'NR==2 { printf "free: %.1f%% used\n", ($2 - $7) / $2 * 100 }'
```

Expected evidence: `cpu:` reports 8 cores. `memory:` shows four buckets that add up to the total, a
total matching `MemTotal` in `/proc/meminfo`, and a used percentage within 0.5 points of the `free:`
line (both are `MemTotal − MemAvailable`; the gap is only the time between the two reads). A
cross-check of the CPU figure against `top` is approximate at best: this code counts iowait as idle,
while `top`'s busy share includes `wa`. Quote the output in the report. `examples/` is outside `crates/core/src`, so the purity gate does not
apply to it; `./scripts/check.sh` still lints it.

- [x] **Step 5: Final gate** — deviated: review added two tests (tx-only restart, meminfo kB overflow); suite is now 58 (memory 13, throughput 9)

1. `git status` and `git diff` over everything, untracked files included: no debug output, no TODO, no
   commented-out code, nothing outside this plan.
2. `./scripts/check.sh` once more after the last edit; quote the result.
3. Self-review with one concrete reason per item: requirements and edge cases; error paths; no
   unbounded growth (`UsageHistory` is capped, `HotAlertPolicy` forgets absent sensors); naming.
4. `code-review-strict`, then address or report its findings. If anything changed, repeat from 1.

- [x] **Step 6: Commit checkpoint** — also ready: `crates/core/tests/memory.rs crates/core/tests/throughput.rs`

Ready to commit: `README.md crates/core/examples/live.rs`
— `docs: add README and a live example for the core crate`

**Verification gaps the executor must report:**
- Nothing here is exercised through a UI, tray or daemon: none exists until Plan 02.
- Only the memory "used" total is compared with another tool; the App/Kernel and Cached/Free splits
  are checked for arithmetic only.
- The purity gate is textual and can be bypassed by an alias; see Risks.
