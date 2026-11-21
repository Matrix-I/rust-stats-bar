# Plan 03 — Battery, Fan and Network in the tray

> **For agentic workers:** REQUIRED SUB-SKILL: subagent-driven-strict or executing-plans-strict.
> Tasks are `### Task N: <name>`; steps use `- [ ]`.

**Goal:** three more tray items next to CPU and Memory. Battery shows `BAT` or `CHG` over its
charge. Fan shows `FAN` over the share of its range. Network shows upload over download, as two
short rate lines. Each item has a summary menu and a show/hide switch in the hub, and nothing is
read for a hidden item.

**Architecture:** the same three crates as Plan 02b. `statsbar-core` gains pure `sysfs`,
`battery`, `fan` and `network` modules, three label words and a two-line rate label.
`statsbar-readers` gains `BatteryReader`, `FanReader` and `NetworkReader`. Like the CPU and
memory readers, they keep their files open and re-read them from offset 0. `statsbard` gains three
config keys, the menu text, the app state, and the tray and loop wiring for five metrics. All
reads stay on the main loop: the slowest one, the battery's `uevent`, measured 9 ms here.

**Tech stack:** Rust edition 2024, workspace floor 1.85, `statsbard` 1.87. Development toolchain
on 2026-11-19: `rustc 1.100.0`. No new dependency.

**Branch:** `feature/battery-fan-network`, cut from `main`. This follows the roadmap's
one-branch-per-plan rule.

## Confirmed Requirements

Answered by the user on 2026-11-19:

1. **Three new tray items: Battery, Fan and Network.** Fan gets an item of its own. Upstream has
   no fan item and shows fans inside its Battery panel, so this deviates from upstream.
2. **Battery label:** the word `BAT` over the charge percent, as CPU and RAM do. The word is
   `CHG` only while the kernel status is `Charging`. Every other status shows `BAT`, including
   `Not charging`, which this laptop reports while it holds its 75–80 % charge thresholds.
3. **Fan label:** the word `FAN` over the fan's share of its range, from `fan1_min` to
   `fan1_max`, clamped at 100 %. This laptop was read at 6040 RPM against a `fan1_max` of 5300.
4. **Network label:** two lines in the 20×20 pixmap. Upload is on top behind an up arrow, and
   download is below behind a down arrow, as upstream draws them. Each rate has at most three
   characters of number and one unit letter (`B`, `K`, `M`, `G`).
5. **Cycle count:** the `Cycles` row is left out when the kernel reports 0 or nothing. This
   battery reports 0 at 65 % health.
6. **Item menus,** accepted as proposed:
   - **Battery:** `Charge 76 %`, `Status Not charging`. `Time left 2h 05m` while discharging,
     or `Time to full 0h 40m` while charging. `Power 8.0 W`, `Health 65 %`,
     `Capacity 5803 / 8948 mAh`, `Voltage 8.00 V`, `Cycles 312`, then `Adapter connected` or
     `Adapter not connected`. A row whose value is unknown is left out.
   - **Fan:** `Speed 6040 RPM`, `Max 5300 RPM`.
   - **Network:** `Interface wlo1`, or `No active interface`. Then `Download 1.2 MB/s`,
     `Upload 34.0 KB/s`, and `Downloaded 1.2 GB` and `Uploaded 300.0 MB`, which count from when
     the item was shown or the interface changed.
   - The battery serial number and the USB-C adapter's wattage wait for the Battery page in
     Plan 05.
7. **Hub:** rows and switches follow the order CPU, Memory, Battery, Fan, Network. The new rows
   are `Battery 76 % · Not charging`, `Fan 6040 RPM` and `Network ↓ 1.2 MB/s ↑ 34.0 KB/s`. A shown
   item with no value yet reads `Battery —`, and so on. The new switches are `Show Battery`,
   `Show Fan` and `Show Network`.
8. **Config:** `[items]` gains `battery`, `fan` and `network`, each `true` by default. An existing
   config file without these keys therefore shows the three new items after an upgrade.
9. **Estimates longer than 24 h are dropped,** so the `Time left` or `Time to full` row is left
   out. A tiny current, such as the 1 mA this laptop reads right after unplugging, would
   otherwise promise 184 days.

## Global Constraints

These come from the roadmap and the user's global rules. The roadmap wins on project matters.

- Test-first: happy path, edge cases, error paths. Untested code is reported incomplete, never done.
- Each test can fail without its change, and asserts observable behaviour. Mock only true
  boundaries. No sleeps, wall clock, ordering or network in tests.
- Never weaken, skip or delete a test to make it pass.
- Commit or push only when asked. Each task therefore ends with `Ready to commit: …`, never
  `git commit`. Use Conventional Commits in the imperative mood, one logical change per commit.
- Code, comments, names, commit messages and docs are in English. Comments explain why.
- Every external call has a timeout: setup calls get 5 s, per-tick D-Bus or tray calls get 1 s.
  Nothing is unbounded, and a file read is capped at 256 KiB (`ProcFile`).
- The host clock is never used in logic or tests. The loop passes monotonic nanoseconds.
- **Project invariant (enforced by `scripts/check-core-purity.sh`):** `crates/core` does no I/O,
  reads no clock, spawns nothing, has no `unsafe` and no build script, and depends on nothing but
  `serde`.
- **Project invariant:** `statsbard` never links GTK and never starts a tokio runtime. Its threads
  are the main loop (which also ticks the daemon's `zbus` connection), the `async-io` reactor, the
  single `ksni` driver shared by every item, the worker (from Plan 05), and the `blocking` pool
  threads that `zbus` uses while connecting. Three more items add no thread.
- Verification entry point: `./scripts/check.sh` (fmt check, clippy `-D warnings`, tests, purity gate).
  Every "Run and expect PASS" step runs `cargo fmt --all` first: the code blocks below are not
  guaranteed to be rustfmt-clean, and the fmt check would otherwise fail first.
- Final gate: read the full diff, run `./scripts/check.sh`, self-review, run `code-review-strict`,
  and repeat if anything changed.
- End-to-end checks drive menu clicks and settings over D-Bus (`busctl`, `powerprofilesctl`), and
  ask the user only for what cannot be observed that way, such as how the panel looks or pulling
  the power cord. Every changed setting is restored, and the report says which clicks were
  simulated.

## Technical Defaults

- **Supplies by name:** `BAT0` and `AC` only, as roadmap requirement 5 targets this laptop. A
  missing `AC` reads as "no adapter row", not as an error.
- **Battery from `uevent`:** one file and one ACPI call per sample instead of ten attribute files.
  The figures use the `CHARGE_*` (µAh) fields that this battery reports. A battery that reports
  only `ENERGY_*` gets no estimate, health or capacity row.
- **Power** is `|current| × voltage`. There is no power row while the current is 0. The current's
  sign is ignored, because some drivers report a negative current while discharging.
- **mAh** are rounded to the nearest whole number. **Health** is shown unclamped, so a new
  battery can read above 100 %.
- **Fan device:** found by its hwmon `name` (`dell_smm`), because hwmon numbers change between
  boots. `fan1_min` and `fan1_max` are read once at open. Only `fan1_input` is read per sample,
  because each read is a BIOS (SMM) call: 0.7 ms here, measured on 2026-11-19.
- **Default route:** the IPv4 table `/proc/net/route`. Of the up routes with destination and mask
  0, the lowest metric wins, and the first line wins on a tie. A machine with only an IPv6
  default route reads as `No active interface`. NetworkManager replaces this in Plan 05.
- **Interface names are validated** before they are joined onto `/sys/class/net/`: 1 to 15 ASCII
  graphic bytes, without `/` or `:`, and not `.` or `..`.
- **Rate text** bands like `format::menu_bar_rate`, with a one-letter unit: `0B`, `999B`, `1K`,
  `999K`, `1.0M`, `9.9M`, `10M`, `999M`, `1.0G` and so on. 999.5 GB/s and above read `999G`.
  NaN, negatives and `-0.0` read `0B`.
- **Rate glyphs** reuse the 3×9 digits. Unit letters are 3×5 and sit on the digit baseline. The
  dot is one pixel. The arrows are 3×9. The upload line is rows 0–8 and the download line rows
  10–18. Text is right-aligned, so its last column is 18, as in the percentage label.
- **Network session totals** restart when the item is shown again, or when the interface changes
  (the existing `ThroughputTracker` rule).
- **`Command::Toggle(ReaderId)`** replaces the per-metric `ToggleCpu` and `ToggleMemory`
  variants, because five near-identical variants add nothing.
- **Items spawn in hub order.** GNOME may still order the items differently, and the end-to-end
  run records what it shows.
- **Tests that need files** write under `env!("CARGO_TARGET_TMPDIR")`, in one directory per test.
  Fixture trees copy this laptop's files with the serial number replaced.

## Risks & Rollback

| Risk | Mitigation | Revert |
|---|---|---|
| Each fan sample is an SMM call that stalls every core for about 0.7 ms | One read per sample (`fan1_input`); limits read once; nothing read while the item is hidden. The end-to-end run times the `pread64` calls with `strace -T` | Untick *Show Fan* |
| The battery's first `uevent` read took 9 ms and `AC/online` takes 1 ms, on the main loop | 2 s cadence (5 s in power saver); the tray answers D-Bus on its own thread; the `strace -T` times are recorded | Untick *Show Battery*; moving reads to the Plan 05 worker is the fallback |
| The two-line rate label or the new words are not legible in the panel | The end-to-end run asks the user to read each label | Fallback: a 3×7 digit font with a gap between the lines, or download only (the user's third option) |
| An IPv6-only network shows `No active interface` | Documented gap; Plan 05 reads NetworkManager | — |
| The config file gains three keys | Additive. Older versions ignore unknown keys and keep them when saving (Plan 02b) | Delete the keys or the file |
| Six tray items cost memory | S2 measured about 68 kB Pss per item; the end-to-end run records Pss | Hide items |
| A crate API differs from what this plan assumes | No new crate; `ksni` and `zbus` usage is unchanged from Plan 02b | — |

---

### Task 1: Core `sysfs` and `battery`

**Files:** create `crates/core/src/sysfs.rs`, `crates/core/src/battery.rs`,
`crates/core/tests/sysfs.rs` and `crates/core/tests/battery.rs`. Modify `crates/core/src/lib.rs`.

**Interfaces — Produces:**
- `sysfs::parse_u64(text: &str) -> Option<u64>` and `sysfs::parse_bool(text: &str) -> Option<bool>`
- `battery::ChargeStatus { Charging, Discharging, NotCharging, Full, Unknown }`, with
  `from_kernel(&str) -> ChargeStatus` and `label(self) -> &'static str`
- `battery::BatteryInfo` (`Copy`) and `battery::parse_uevent(text: &str) -> Option<BatteryInfo>`
- `battery::Estimate { ToEmpty { minutes: u64 }, ToFull { minutes: u64 } }`
- `battery::BatteryFigures` (`Copy`) and
  `BatteryFigures::new(info: &BatteryInfo, adapter_online: Option<bool>) -> BatteryFigures`
- `battery::MAX_ESTIMATE_MINUTES: u64 = 1440`

**Test level:** unit.

- [x] **Step 1: Branch.** Run `git switch main && git pull --ff-only && git switch -c
  feature/battery-fan-network`. Expect a clean tree, apart from this plan file if it is still
  untracked.
- [x] **Step 2: Write the failing tests.** `crates/core/tests/sysfs.rs`:

```rust
use statsbar_core::sysfs::{parse_bool, parse_u64};

#[test]
fn a_number_attribute_parses_without_its_newline() {
    assert_eq!(parse_u64("6040\n"), Some(6040));
    assert_eq!(parse_u64("0"), Some(0));
}

#[test]
fn an_empty_negative_or_non_numeric_attribute_is_refused() {
    for text in ["", "\n", "-1\n", "12 RPM\n", "0x10\n"] {
        assert_eq!(parse_u64(text), None, "{text:?}");
    }
}

#[test]
fn a_flag_attribute_is_zero_or_one_only() {
    assert_eq!(parse_bool("1\n"), Some(true));
    assert_eq!(parse_bool("0\n"), Some(false));
    for text in ["", "2\n", "yes\n"] {
        assert_eq!(parse_bool(text), None, "{text:?}");
    }
}
```

`crates/core/tests/battery.rs`:

```rust
use statsbar_core::battery::ChargeStatus::{Charging, Discharging, Full, NotCharging, Unknown};
use statsbar_core::battery::{
    BatteryFigures, BatteryInfo, ChargeStatus, Estimate, MAX_ESTIMATE_MINUTES, parse_uevent,
};

/// `/sys/class/power_supply/BAT0/uevent` on this laptop on 2026-11-19, serial number replaced.
const THIS_LAPTOP: &str = "DEVTYPE=power_supply
POWER_SUPPLY_NAME=BAT0
POWER_SUPPLY_TYPE=Battery
POWER_SUPPLY_STATUS=Not charging
POWER_SUPPLY_PRESENT=1
POWER_SUPPLY_TECHNOLOGY=Li-poly
POWER_SUPPLY_CYCLE_COUNT=0
POWER_SUPPLY_VOLTAGE_MIN_DESIGN=7600000
POWER_SUPPLY_VOLTAGE_NOW=7997000
POWER_SUPPLY_CURRENT_NOW=1000
POWER_SUPPLY_CHARGE_FULL_DESIGN=8948000
POWER_SUPPLY_CHARGE_FULL=5803000
POWER_SUPPLY_CHARGE_NOW=4418000
POWER_SUPPLY_CAPACITY=76
POWER_SUPPLY_CAPACITY_LEVEL=Normal
POWER_SUPPLY_TYPE=Battery
POWER_SUPPLY_MODEL_NAME=DELL C5GV285
POWER_SUPPLY_MANUFACTURER=SMP
POWER_SUPPLY_SERIAL_NUMBER=0000
";

fn info(status: ChargeStatus, charge_now: u64, current: i64) -> BatteryInfo {
    BatteryInfo {
        status,
        capacity_percent: 50,
        charge_now_uah: Some(charge_now),
        charge_full_uah: Some(5_803_000),
        charge_full_design_uah: Some(8_948_000),
        current_now_ua: Some(current),
        voltage_now_uv: Some(8_000_000),
        cycle_count: None,
    }
}

#[test]
fn parses_this_laptops_uevent() {
    assert_eq!(
        parse_uevent(THIS_LAPTOP),
        Some(BatteryInfo {
            status: NotCharging,
            capacity_percent: 76,
            charge_now_uah: Some(4_418_000),
            charge_full_uah: Some(5_803_000),
            charge_full_design_uah: Some(8_948_000),
            current_now_ua: Some(1000),
            voltage_now_uv: Some(7_997_000),
            cycle_count: Some(0),
        })
    );
}

#[test]
fn every_kernel_status_maps_and_anything_else_is_unknown() {
    for (text, status) in [
        ("Charging", Charging),
        ("Discharging", Discharging),
        ("Not charging", NotCharging),
        ("Full", Full),
        ("Unknown", Unknown),
        ("Weird", Unknown),
    ] {
        assert_eq!(ChargeStatus::from_kernel(text), status, "{text}");
    }
}

#[test]
fn status_labels_read_as_the_menu_shows_them() {
    let labels = [Charging, Discharging, NotCharging, Full, Unknown].map(ChargeStatus::label);
    assert_eq!(
        labels,
        ["Charging", "Discharging", "Not charging", "Full", "Unknown"]
    );
}

#[test]
fn status_and_capacity_are_required() {
    assert_eq!(parse_uevent("POWER_SUPPLY_CAPACITY=76\n"), None);
    assert_eq!(parse_uevent("POWER_SUPPLY_STATUS=Full\n"), None);
    assert!(parse_uevent("POWER_SUPPLY_STATUS=Full\nPOWER_SUPPLY_CAPACITY=100\n").is_some());
}

#[test]
fn a_capacity_above_100_or_a_malformed_number_is_refused() {
    let base = "POWER_SUPPLY_STATUS=Full\nPOWER_SUPPLY_CAPACITY=76\n";
    assert_eq!(
        parse_uevent("POWER_SUPPLY_STATUS=Full\nPOWER_SUPPLY_CAPACITY=101\n"),
        None
    );
    assert_eq!(
        parse_uevent(&format!("{base}POWER_SUPPLY_CHARGE_NOW=4x\n")),
        None
    );
    assert_eq!(
        parse_uevent(&format!("{base}POWER_SUPPLY_CHARGE_NOW=\n")),
        None
    );
}

#[test]
fn optional_fields_may_be_missing_and_current_may_be_negative() {
    let parsed = parse_uevent(
        "POWER_SUPPLY_STATUS=Discharging\nPOWER_SUPPLY_CAPACITY=40\n\
         POWER_SUPPLY_CURRENT_NOW=-1500000\n",
    )
    .expect("status and capacity are there");
    assert_eq!(parsed.current_now_ua, Some(-1_500_000));
    assert_eq!(parsed.charge_now_uah, None);
    assert_eq!(parsed.cycle_count, None);
}

#[test]
fn discharging_estimates_time_to_empty_and_power() {
    let figures = BatteryFigures::new(&info(Discharging, 4_000_000, 2_000_000), Some(false));
    assert_eq!(figures.estimate, Some(Estimate::ToEmpty { minutes: 120 }));
    assert_eq!(figures.power_watts, Some(16.0));
    assert_eq!(figures.adapter_online, Some(false));
}

#[test]
fn a_negative_current_counts_by_its_size() {
    let figures = BatteryFigures::new(&info(Discharging, 4_000_000, -2_000_000), None);
    assert_eq!(figures.estimate, Some(Estimate::ToEmpty { minutes: 120 }));
    assert_eq!(figures.power_watts, Some(16.0));
}

#[test]
fn charging_estimates_time_to_full() {
    // 1.385 Ah missing, at 1.385 A.
    let figures = BatteryFigures::new(&info(Charging, 4_418_000, 1_385_000), Some(true));
    assert_eq!(figures.estimate, Some(Estimate::ToFull { minutes: 60 }));
}

#[test]
fn full_not_charging_and_unknown_have_no_estimate() {
    for status in [Full, NotCharging, Unknown] {
        let figures = BatteryFigures::new(&info(status, 4_000_000, 2_000_000), None);
        assert_eq!(figures.estimate, None, "{status:?}");
    }
}

#[test]
fn zero_current_gives_no_estimate_and_no_power() {
    let figures = BatteryFigures::new(&info(Discharging, 4_000_000, 0), None);
    assert_eq!(figures.estimate, None);
    assert_eq!(figures.power_watts, None);
}

#[test]
fn charging_at_or_above_full_has_no_estimate() {
    for charge_now in [5_803_000, 6_000_000] {
        let figures = BatteryFigures::new(&info(Charging, charge_now, 1_000_000), None);
        assert_eq!(figures.estimate, None, "{charge_now}");
    }
}

#[test]
fn an_estimate_over_a_day_is_dropped() {
    let a_day_of_charge = 1_000_000 * MAX_ESTIMATE_MINUTES / 60;
    let longest = BatteryFigures::new(&info(Discharging, a_day_of_charge, 1_000_000), None);
    assert_eq!(
        longest.estimate,
        Some(Estimate::ToEmpty {
            minutes: MAX_ESTIMATE_MINUTES
        })
    );
    // 4.418 Ah at 1 mA, as this laptop reads right after unplugging: 184 days.
    let too_long = BatteryFigures::new(&info(Discharging, 4_418_000, 1000), None);
    assert_eq!(too_long.estimate, None);
}

#[test]
fn health_capacities_voltage_and_percent_come_from_the_uevent() {
    let info = parse_uevent(THIS_LAPTOP).expect("well-formed");
    let figures = BatteryFigures::new(&info, Some(true));
    assert_eq!(figures.percent, 76);
    assert_eq!(figures.status, NotCharging);
    let health = figures.health_percent.expect("both capacities are known");
    assert!((health - 64.852_48).abs() < 0.001, "{health}");
    assert_eq!(
        (figures.full_mah, figures.design_mah),
        (Some(5803), Some(8948))
    );
    assert_eq!(figures.voltage_volts, Some(7.997));
    assert_eq!(figures.adapter_online, Some(true));
}

#[test]
fn health_needs_both_capacities_above_zero() {
    let mut no_design = info(Full, 5_803_000, 0);
    no_design.charge_full_design_uah = Some(0);
    assert_eq!(BatteryFigures::new(&no_design, None).health_percent, None);
    let mut no_full = info(Full, 5_803_000, 0);
    no_full.charge_full_uah = None;
    assert_eq!(BatteryFigures::new(&no_full, None).health_percent, None);
}

#[test]
fn a_cycle_count_of_zero_reads_as_unknown() {
    let mut zero = info(Full, 5_803_000, 0);
    zero.cycle_count = Some(0);
    assert_eq!(BatteryFigures::new(&zero, None).cycles, None);
    let mut counted = zero;
    counted.cycle_count = Some(312);
    assert_eq!(BatteryFigures::new(&counted, None).cycles, Some(312));
}

#[test]
fn milliamp_hours_round_to_the_nearest() {
    let mut odd = info(Full, 0, 0);
    odd.charge_full_uah = Some(5_803_500);
    odd.charge_full_design_uah = Some(8_948_499);
    let figures = BatteryFigures::new(&odd, None);
    assert_eq!(
        (figures.full_mah, figures.design_mah),
        (Some(5804), Some(8948))
    );
}
```

- [x] **Step 3: Run and expect FAIL.** `cargo test -p statsbar-core --test sysfs --test battery`
  fails to compile with "unresolved import `statsbar_core::sysfs`" and
  "unresolved import `statsbar_core::battery`".
- [x] **Step 4: Implement.** In `crates/core/src/lib.rs`, add `pub mod battery;` before
  `pub mod cpu;`, and `pub mod sysfs;` after `pub mod schedule;`.

`crates/core/src/sysfs.rs`:

```rust
//! Values of one-value sysfs attributes, such as `fan1_input` or `online`.

/// The unsigned integer an attribute holds, without its trailing newline. `None` for anything
/// else, including an empty file.
pub fn parse_u64(text: &str) -> Option<u64> {
    text.trim().parse().ok()
}

/// A `0` or `1` attribute, such as `power_supply/AC/online`.
pub fn parse_bool(text: &str) -> Option<bool> {
    match text.trim() {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}
```

`crates/core/src/battery.rs`:

```rust
//! The laptop battery: `power_supply/BAT0/uevent` text in, the figures of its menu out.

const MICRO: f64 = 1_000_000.0;
const MICRO_PER_MILLI: u64 = 1000;
const MINUTES_PER_HOUR: u128 = 60;

/// Estimates longer than a day are dropped. They come from a current too small to mean anything:
/// right after unplugging, this laptop reads 1 mA, which would promise 184 days.
pub const MAX_ESTIMATE_MINUTES: u64 = 24 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargeStatus {
    Charging,
    Discharging,
    /// Plugged in but held, for example at a charge threshold.
    NotCharging,
    Full,
    Unknown,
}

impl ChargeStatus {
    /// Maps the kernel's `POWER_SUPPLY_STATUS` text. Anything unrecognised reads as `Unknown`.
    pub fn from_kernel(text: &str) -> ChargeStatus {
        match text {
            "Charging" => ChargeStatus::Charging,
            "Discharging" => ChargeStatus::Discharging,
            "Not charging" => ChargeStatus::NotCharging,
            "Full" => ChargeStatus::Full,
            _ => ChargeStatus::Unknown,
        }
    }

    /// The text of the Status row and the hub row.
    pub fn label(self) -> &'static str {
        match self {
            ChargeStatus::Charging => "Charging",
            ChargeStatus::Discharging => "Discharging",
            ChargeStatus::NotCharging => "Not charging",
            ChargeStatus::Full => "Full",
            ChargeStatus::Unknown => "Unknown",
        }
    }
}

/// The `uevent` fields the figures are built from, in the kernel's micro-units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatteryInfo {
    pub status: ChargeStatus,
    pub capacity_percent: u8,
    pub charge_now_uah: Option<u64>,
    pub charge_full_uah: Option<u64>,
    pub charge_full_design_uah: Option<u64>,
    /// Signed, because some drivers report a negative current while discharging. Dell's is
    /// positive both ways.
    pub current_now_ua: Option<i64>,
    pub voltage_now_uv: Option<u64>,
    pub cycle_count: Option<u64>,
}

/// Parses a battery's `uevent`.
///
/// Returns `None` without `POWER_SUPPLY_STATUS`, or without a `POWER_SUPPLY_CAPACITY` from 0 to
/// 100, because the label needs both. It also returns `None` when a number it reads is malformed,
/// as the other `/proc` parsers do. The other fields are `None` when absent: the `CHARGE_*`
/// fields exist only on batteries that count charge rather than energy.
pub fn parse_uevent(text: &str) -> Option<BatteryInfo> {
    let mut status = None;
    let mut capacity = None;
    let mut charge_now = None;
    let mut charge_full = None;
    let mut charge_full_design = None;
    let mut current_now = None;
    let mut voltage_now = None;
    let mut cycle_count = None;
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "POWER_SUPPLY_STATUS" => status = Some(ChargeStatus::from_kernel(value)),
            "POWER_SUPPLY_CAPACITY" => {
                capacity = Some(value.parse::<u8>().ok().filter(|&percent| percent <= 100)?);
            }
            "POWER_SUPPLY_CHARGE_NOW" => charge_now = Some(value.parse().ok()?),
            "POWER_SUPPLY_CHARGE_FULL" => charge_full = Some(value.parse().ok()?),
            "POWER_SUPPLY_CHARGE_FULL_DESIGN" => charge_full_design = Some(value.parse().ok()?),
            "POWER_SUPPLY_CURRENT_NOW" => current_now = Some(value.parse().ok()?),
            "POWER_SUPPLY_VOLTAGE_NOW" => voltage_now = Some(value.parse().ok()?),
            "POWER_SUPPLY_CYCLE_COUNT" => cycle_count = Some(value.parse().ok()?),
            _ => {}
        }
    }
    Some(BatteryInfo {
        status: status?,
        capacity_percent: capacity?,
        charge_now_uah: charge_now,
        charge_full_uah: charge_full,
        charge_full_design_uah: charge_full_design,
        current_now_ua: current_now,
        voltage_now_uv: voltage_now,
        cycle_count,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estimate {
    ToEmpty { minutes: u64 },
    ToFull { minutes: u64 },
}

/// What the battery's menu and label show. A `None` field has no row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BatteryFigures {
    pub percent: u8,
    pub status: ChargeStatus,
    pub estimate: Option<Estimate>,
    pub power_watts: Option<f64>,
    /// Full-charge capacity against design capacity. Not clamped: a new battery can exceed 100.
    pub health_percent: Option<f64>,
    pub full_mah: Option<u64>,
    pub design_mah: Option<u64>,
    pub voltage_volts: Option<f64>,
    /// `None` for 0 as well as for a missing count: a worn battery at zero cycles means the
    /// firmware does not keep the count.
    pub cycles: Option<u64>,
    pub adapter_online: Option<bool>,
}

impl BatteryFigures {
    pub fn new(info: &BatteryInfo, adapter_online: Option<bool>) -> BatteryFigures {
        let current = info
            .current_now_ua
            .map(i64::unsigned_abs)
            .filter(|&current| current > 0);
        let health_percent = match (info.charge_full_uah, info.charge_full_design_uah) {
            (Some(full), Some(design)) if full > 0 && design > 0 => {
                Some(full as f64 / design as f64 * 100.0)
            }
            _ => None,
        };
        BatteryFigures {
            percent: info.capacity_percent,
            status: info.status,
            estimate: current.and_then(|current| estimate(info, current)),
            power_watts: current
                .zip(info.voltage_now_uv)
                .map(|(current, voltage)| current as f64 / MICRO * (voltage as f64 / MICRO)),
            health_percent,
            full_mah: info.charge_full_uah.map(milliamp_hours),
            design_mah: info.charge_full_design_uah.map(milliamp_hours),
            voltage_volts: info.voltage_now_uv.map(|voltage| voltage as f64 / MICRO),
            cycles: info.cycle_count.filter(|&cycles| cycles > 0),
            adapter_online,
        }
    }
}

/// `current_ua` is above zero.
fn estimate(info: &BatteryInfo, current_ua: u64) -> Option<Estimate> {
    let now = info.charge_now_uah?;
    match info.status {
        ChargeStatus::Discharging => Some(Estimate::ToEmpty {
            minutes: minutes_for(now, current_ua)?,
        }),
        ChargeStatus::Charging => {
            let missing = info
                .charge_full_uah?
                .checked_sub(now)
                .filter(|&missing| missing > 0)?;
            Some(Estimate::ToFull {
                minutes: minutes_for(missing, current_ua)?,
            })
        }
        ChargeStatus::NotCharging | ChargeStatus::Full | ChargeStatus::Unknown => None,
    }
}

/// Whole minutes to move `charge_uah` at `current_ua`, or `None` beyond a day. Computed in
/// `u128`, so no product of two `u64` values can overflow.
fn minutes_for(charge_uah: u64, current_ua: u64) -> Option<u64> {
    let minutes = u128::from(charge_uah) * MINUTES_PER_HOUR / u128::from(current_ua);
    u64::try_from(minutes)
        .ok()
        .filter(|&minutes| minutes <= MAX_ESTIMATE_MINUTES)
}

fn milliamp_hours(micro_amp_hours: u64) -> u64 {
    micro_amp_hours.saturating_add(MICRO_PER_MILLI / 2) / MICRO_PER_MILLI
}
```

- [x] **Step 5: Run and expect PASS.** `cargo test -p statsbar-core --test sysfs --test battery`
  passes, and `cargo fmt --all` then `./scripts/check.sh` is green, including `core purity: ok`.

Ready to commit: `crates/core/src/lib.rs crates/core/src/sysfs.rs crates/core/src/battery.rs crates/core/tests/sysfs.rs crates/core/tests/battery.rs docs/plans/2026-11-19-rust-statsbar-03-battery-fan-network.md` — "feat(core): turn a battery uevent into charge, health, power and time figures"

### Task 2: Core `fan` and `network`

**Files:** create `crates/core/src/fan.rs`, `crates/core/src/network.rs`,
`crates/core/tests/fan.rs` and `crates/core/tests/network.rs`. Modify `crates/core/src/lib.rs`.

**Interfaces — Produces:**
- `fan::FanSpeed { rpm: u64, min_rpm: u64, max_rpm: u64 }` (`Copy`)
- `fan::range_fraction(speed: &FanSpeed) -> Option<f64>`
- `network::is_interface_name(name: &str) -> bool`
- `network::RouteError { line: usize }`, which implements `Display` and `Error`
- `network::default_route_interface(route: &str) -> Result<Option<String>, RouteError>`

**Test level:** unit.

- [x] **Step 1: Write the failing tests.** `crates/core/tests/fan.rs`:

```rust
use statsbar_core::fan::{FanSpeed, range_fraction};

fn speed(rpm: u64) -> FanSpeed {
    FanSpeed {
        rpm,
        min_rpm: 0,
        max_rpm: 5300,
    }
}

#[test]
fn half_way_up_the_range_is_one_half() {
    assert_eq!(range_fraction(&speed(2650)), Some(0.5));
}

#[test]
fn above_max_clamps_to_one() {
    // This laptop: 6040 RPM against a fan1_max of 5300.
    assert_eq!(range_fraction(&speed(6040)), Some(1.0));
}

#[test]
fn stopped_or_below_min_is_zero() {
    assert_eq!(range_fraction(&speed(0)), Some(0.0));
    let below = FanSpeed {
        rpm: 500,
        min_rpm: 1000,
        max_rpm: 5000,
    };
    assert_eq!(range_fraction(&below), Some(0.0));
}

#[test]
fn the_range_is_measured_from_min() {
    let speed = FanSpeed {
        rpm: 3000,
        min_rpm: 1000,
        max_rpm: 5000,
    };
    assert_eq!(range_fraction(&speed), Some(0.5));
}

#[test]
fn an_empty_or_inverted_range_has_no_fraction() {
    for (min_rpm, max_rpm) in [(0, 0), (5300, 5300), (5300, 1000)] {
        let speed = FanSpeed {
            rpm: 3000,
            min_rpm,
            max_rpm,
        };
        assert_eq!(range_fraction(&speed), None, "{min_rpm}..{max_rpm}");
    }
}
```

`crates/core/tests/network.rs`:

```rust
use statsbar_core::network::{RouteError, default_route_interface, is_interface_name};

const HEADER: &str =
    "Iface\tDestination\tGateway \tFlags\tRefCnt\tUse\tMetric\tMask\t\tMTU\tWindow\tIRTT\n";

/// `/proc/net/route` on this laptop on 2026-11-19.
fn this_laptop() -> String {
    format!(
        "{HEADER}wlo1\t00000000\t0164A8C0\t0003\t0\t0\t600\t00000000\t0\t0\t0\n\
         wlo1\t0064A8C0\t00000000\t0001\t0\t0\t600\t00FFFFFF\t0\t0\t0\n"
    )
}

fn default_route(interface: &str, flags: &str, metric: u32) -> String {
    format!("{interface}\t00000000\t0164A8C0\t{flags}\t0\t0\t{metric}\t00000000\t0\t0\t0\n")
}

fn interface(text: &str) -> Result<Option<String>, RouteError> {
    default_route_interface(text)
}

#[test]
fn finds_this_laptops_default_route() {
    assert_eq!(interface(&this_laptop()), Ok(Some("wlo1".to_owned())));
}

#[test]
fn the_lowest_metric_wins() {
    let text = format!(
        "{HEADER}{}{}",
        default_route("wlo1", "0003", 600),
        default_route("eno2", "0003", 100)
    );
    assert_eq!(interface(&text), Ok(Some("eno2".to_owned())));
}

#[test]
fn a_tie_goes_to_the_first_line() {
    let text = format!(
        "{HEADER}{}{}",
        default_route("eno2", "0003", 100),
        default_route("wlo1", "0003", 100)
    );
    assert_eq!(interface(&text), Ok(Some("eno2".to_owned())));
}

#[test]
fn a_route_that_is_not_up_is_ignored() {
    let text = format!(
        "{HEADER}{}{}",
        default_route("eno2", "0002", 100),
        default_route("wlo1", "0003", 600)
    );
    assert_eq!(interface(&text), Ok(Some("wlo1".to_owned())));
}

#[test]
fn no_default_route_or_an_empty_table_is_none() {
    let subnet_only = format!("{HEADER}wlo1\t0064A8C0\t00000000\t0001\t0\t0\t600\t00FFFFFF\t0\t0\t0\n");
    for text in ["", HEADER, subnet_only.as_str()] {
        assert_eq!(interface(text), Ok(None), "{text:?}");
    }
}

#[test]
fn a_malformed_line_is_an_error_naming_it() {
    let bad_hex = format!(
        "{HEADER}{}wlo1\tZZ\t0\t0003\t0\t0\t600\t00000000\n",
        default_route("eno2", "0003", 100)
    );
    assert_eq!(interface(&bad_hex), Err(RouteError { line: 3 }));
    let short = format!("{HEADER}wlo1\t00000000\t0\n");
    assert_eq!(interface(&short), Err(RouteError { line: 2 }));
    assert_eq!(
        RouteError { line: 2 }.to_string(),
        "proc/net/route line 2 is malformed"
    );
}

#[test]
fn an_interface_name_that_could_escape_sys_class_net_is_an_error() {
    let text = format!("{HEADER}{}", default_route("..", "0003", 100));
    assert_eq!(interface(&text), Err(RouteError { line: 2 }));
}

#[test]
fn interface_names_follow_the_kernels_rules() {
    for name in ["wlo1", "eno2", "wwp0s20f0u5i12", "a-b_c.d"] {
        assert!(is_interface_name(name), "{name}");
    }
    for name in ["", ".", "..", "a/b", "a:b", "a b", "sixteen-chars-xx", "wlö1"] {
        assert!(!is_interface_name(name), "{name}");
    }
}
```

- [x] **Step 2: Run and expect FAIL.** `cargo test -p statsbar-core --test fan --test network`
  fails to compile with "unresolved import `statsbar_core::fan`" and
  "unresolved import `statsbar_core::network`".
- [x] **Step 3: Implement.** In `crates/core/src/lib.rs`, add `pub mod fan;` after
  `pub mod cpu;`, and `pub mod network;` after `pub mod memory;`.

`crates/core/src/fan.rs`:

```rust
//! The fan: hwmon RPM readings in, how far along its range it runs out.

/// One reading of a fan and the limits its driver reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FanSpeed {
    pub rpm: u64,
    pub min_rpm: u64,
    pub max_rpm: u64,
}

/// Where `speed.rpm` sits between `min_rpm` and `max_rpm`, from 0 to 1. Clamped, because
/// `dell_smm` reports a `fan1_max` of 5300 while this laptop's fan was read at 6040. `None` when
/// the range is empty or inverted.
pub fn range_fraction(speed: &FanSpeed) -> Option<f64> {
    if speed.max_rpm <= speed.min_rpm {
        return None;
    }
    let span = (speed.max_rpm - speed.min_rpm) as f64;
    let above_min = speed.rpm.saturating_sub(speed.min_rpm) as f64;
    Some((above_min / span).min(1.0))
}
```

`crates/core/src/network.rs`:

```rust
//! The interface the default route leaves by: `/proc/net/route` text in, an interface name out.

use std::fmt;

/// IFNAMSIZ is 16, including the terminating NUL.
const MAX_INTERFACE_NAME: usize = 15;
/// `RTF_UP` in `<linux/route.h>`.
const RTF_UP: u32 = 0x1;

/// A line of `/proc/net/route` that does not parse, 1-based, counting the header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteError {
    pub line: usize,
}

impl fmt::Display for RouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "proc/net/route line {} is malformed", self.line)
    }
}

impl std::error::Error for RouteError {}

/// Whether `name` can be an interface name and is safe to join onto `/sys/class/net/`. This is
/// the kernel's `dev_valid_name`, narrowed to ASCII.
pub fn is_interface_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_INTERFACE_NAME
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && byte != b'/' && byte != b':')
}

/// The interface of the up IPv4 default route with the lowest metric, and the first such line
/// on a tie. `Ok(None)` when there is none. The first line is the header and is skipped.
///
/// A line that does not parse, or names an impossible interface, is an error rather than
/// skipped: skipping it could hide the real default route behind a worse one. IPv6 default
/// routes live in `/proc/net/ipv6_route`, so a machine with only those reads as having none.
pub fn default_route_interface(route: &str) -> Result<Option<String>, RouteError> {
    let mut best: Option<(&str, u32)> = None;
    for (index, line) in route.lines().enumerate().skip(1) {
        let error = RouteError { line: index + 1 };
        let fields: Vec<&str> = line.split_ascii_whitespace().collect();
        let [interface, destination, _, flags, _, _, metric, mask, ..] = fields.as_slice() else {
            return Err(error);
        };
        let hex = |field: &str| u32::from_str_radix(field, 16).map_err(|_| error);
        let (destination, flags, mask) = (hex(destination)?, hex(flags)?, hex(mask)?);
        let metric: u32 = metric.parse().map_err(|_| error)?;
        if !is_interface_name(interface) {
            return Err(error);
        }
        let is_default = destination == 0 && mask == 0 && flags & RTF_UP != 0;
        if is_default && best.is_none_or(|(_, lowest)| metric < lowest) {
            best = Some((*interface, metric));
        }
    }
    Ok(best.map(|(interface, _)| interface.to_owned()))
}
```

- [x] **Step 4: Run and expect PASS.** `cargo test -p statsbar-core --test fan --test network`
  passes, and `cargo fmt --all` then `./scripts/check.sh` is green.

Ready to commit: `crates/core/src/lib.rs crates/core/src/fan.rs crates/core/src/network.rs crates/core/tests/fan.rs crates/core/tests/network.rs` — "feat(core): add the fan range fraction and the default route interface"

### Task 3: Key tray items by reader

**Files:** modify `crates/daemon/src/tray.rs`, `crates/daemon/src/run.rs` and
`crates/daemon/tests/tray.rs`.

**Interfaces — Produces:** `MetricItem { pub reader: ReaderId, … }`,
`MetricItem::new(reader: ReaderId)`, `tray::name(reader: ReaderId) -> &'static str`. In
`run.rs`: `type Look = (Option<Icon>, String)`,
`fn percent_look(id: ReaderId, metric: Metric, value: u8) -> Look`, and
`Daemon::refresh_item(&self, id: ReaderId, redraw: Option<Look>)`.

**Why:** Task 4 adds label words, and Task 10 adds a network item that has no `Metric`.
Matching on `label::Metric` in the tray would break at Task 4. Keying the items by `ReaderId`
gives one name and one D-Bus id per item, whatever its label shows.

RED: n/a — behaviour-preserving, covered by `crates/daemon/tests/tray.rs`
(`metric_item_ids_are_stable_and_rows_are_not_clickable`,
`a_metric_item_shows_a_themed_icon_until_its_first_value`). The test changes below only swap
the constructor's argument; the assertions stay the same.

- [x] **Step 1: Run the covering tests and expect PASS.** `cargo test -p statsbard --test tray`.
- [x] **Step 2: Change `tray.rs`.** Add `use statsbar_core::schedule::ReaderId;`. Replace the
  `MetricItem` struct, its `new`, `name`, and the `id` method of `impl ksni::Tray for MetricItem`
  with:

```rust
pub struct MetricItem {
    pub reader: ReaderId,
    pub icon: Option<Icon>,
    pub title: String,
    pub rows: Vec<String>,
}

impl MetricItem {
    pub fn new(reader: ReaderId) -> MetricItem {
        MetricItem {
            reader,
            icon: None,
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
    }
}
```

```rust
    fn id(&self) -> String {
        match self.reader {
            ReaderId::Cpu => "statsbar-cpu",
            ReaderId::Memory => "statsbar-memory",
        }
        .to_owned()
    }
```

  `tray::icon(metric: Metric, percent: u8)` keeps its signature. The `Metric` import stays for it.
- [x] **Step 3: Change `run.rs`.** Add `use ksni::Icon;` (next to the existing `ksni` import).
  Below `elapsed_nanos`, add:

```rust
/// A label to draw: the pixmap and the item's title.
type Look = (Option<Icon>, String);

fn percent_look(id: ReaderId, metric: Metric, value: u8) -> Look {
    (
        tray::icon(metric, value),
        summary::title(tray::name(id), Some(value)),
    )
}
```

  In `Daemon::sample`, replace `self.refresh_item(Metric::Cpu, label).await;` with
  `self.refresh_item(id, label.map(|value| percent_look(id, Metric::Cpu, value))).await;`. Replace
  `self.refresh_item(Metric::Ram, label).await;` with
  `self.refresh_item(id, label.map(|value| percent_look(id, Metric::Ram, value))).await;`.
  Replace `refresh_item` with:

```rust
    async fn refresh_item(&self, id: ReaderId, redraw: Option<Look>) {
        let (handle, rows) = match id {
            ReaderId::Cpu => (&self.cpu_item, self.app.cpu().map(summary::cpu_rows)),
            ReaderId::Memory => (
                &self.memory_item,
                self.app.memory().map(summary::memory_rows),
            ),
        };
        let Some(handle) = handle else { return };
        let rows = rows.unwrap_or_default();
        // A missed update is not retried. The rows are resent on the next sample, but the icon and
        // title only when the label changes again, because `App` has already recorded the new
        // value. A 1 s stall on `ksni`'s lock is itself worth investigating, and the log shows it.
        update(
            handle,
            "metric item update",
            move |item: &mut MetricItem| {
                item.rows = rows;
                if let Some((icon, title)) = redraw {
                    item.icon = icon;
                    item.title = title;
                }
            },
        )
        .await;
    }
```

  In `show_item`, delete the `let metric = match id { … };` block and spawn
  `MetricItem::new(id).spawn()` in place of `MetricItem::new(metric).spawn()`.
- [x] **Step 4: Change the tests' constructor calls.** In `crates/daemon/tests/tray.rs`, add
  `use statsbar_core::schedule::ReaderId;`. Replace `MetricItem::new(Metric::Cpu)` with
  `MetricItem::new(ReaderId::Cpu)` in both places, and `MetricItem::new(Metric::Ram)` with
  `MetricItem::new(ReaderId::Memory)`. `tray::icon(Metric::…, …)` and `label::render` calls stay
  as they are.
- [x] **Step 5: Run and expect PASS.** `cargo test -p statsbard --test tray` passes, and
  `cargo fmt --all` then `./scripts/check.sh` is green.

Ready to commit: `crates/daemon/src/tray.rs crates/daemon/src/run.rs crates/daemon/tests/tray.rs` — "refactor(daemon): key tray items by reader rather than by label"

### Task 4: Label words and the two-line rate label

**Files:** modify `crates/core/src/label.rs` and `crates/core/tests/label.rs`.

**Interfaces — Produces:** `label::Metric { Cpu, Ram, Bat, Chg, Fan }`,
`label::rate_text(bytes_per_second: f64) -> String`, and
`label::render_rates(upload_bytes_per_second: f64, download_bytes_per_second: f64) -> [u8; 1600]`.
`label::render` keeps its signature and its output for `Cpu` and `Ram`.

**Test level:** unit.

- [x] **Step 1: Write the failing tests.** In `crates/core/tests/label.rs`, change the import to
  `use statsbar_core::label::{Metric, SIZE, rate_text, render, render_rates};` and add, below
  `fn pixel`:

```rust
const ALL: [Metric; 5] = [Metric::Cpu, Metric::Ram, Metric::Bat, Metric::Chg, Metric::Fan];
```

  In `every_pixel_is_transparent_or_opaque_white`, replace `for metric in [Metric::Cpu, Metric::Ram]`
  with `for metric in ALL`. In `out_of_range_values_are_rejected`, replace the loop body with
  `for metric in ALL { assert!(render(metric, value).is_none(), "{metric:?} {value}"); }`. Then
  append:

```rust
#[test]
fn battery_charging_and_fan_words_sit_above_the_same_percentage() {
    let cpu = render(Metric::Cpu, 37).unwrap();
    let words = [
        (
            Metric::Bat,
            ["##...#..###", "#.#.#.#..#.", "##..###..#.", "#.#.#.#..#.", "##..#.#..#."],
        ),
        (
            Metric::Chg,
            [".##.#.#..##", "#...#.#.#..", "#...###.#.#", "#...#.#.#.#", ".##.#.#..##"],
        ),
        (
            Metric::Fan,
            ["###..#..##.", "#...#.#.#.#", "##..###.#.#", "#...#.#.#.#", "#...#.#.#.#"],
        ),
    ];
    for (metric, word) in words {
        let pixmap = render(metric, 37).unwrap();
        for y in 0..10 {
            for x in 0..SIZE {
                let inside = (2..7).contains(&y) && (8..19).contains(&x);
                let expected = if inside && word[y - 2].as_bytes()[x - 8] == b'#' {
                    255
                } else {
                    0
                };
                assert_eq!(pixel(&pixmap, x, y)[0], expected, "{metric:?} ({x}, {y})");
            }
        }
        let value_start = 10 * SIZE * 4;
        assert_eq!(pixmap[value_start..], cpu[value_start..], "{metric:?}");
    }
}

#[test]
fn every_word_is_distinct() {
    let tops: HashSet<Vec<u8>> = ALL
        .iter()
        .map(|&metric| render(metric, 0).unwrap()[..10 * SIZE * 4].to_vec())
        .collect();
    assert_eq!(tops.len(), ALL.len());
}

#[test]
fn rate_text_bands_like_the_menu_bar_rate() {
    for (rate, text) in [
        (0.0, "0B"),
        (999.4, "999B"),
        (999.5, "1K"),
        (12_000.0, "12K"),
        (999_499.0, "999K"),
        (999_500.0, "1.0M"),
        (9_949_999.0, "9.9M"),
        (9_950_000.0, "10M"),
        (999_499_999.0, "999M"),
        (999_500_000.0, "1.0G"),
        (12_000_000_000.0, "12G"),
        (999_499_999_999.0, "999G"),
        (1e15, "999G"),
        (f64::INFINITY, "999G"),
        (f64::NAN, "0B"),
        (-5.0, "0B"),
        (-0.0, "0B"),
    ] {
        assert_eq!(rate_text(rate), text, "{rate}");
    }
}

#[test]
fn rates_draw_upload_over_download_behind_their_arrows() {
    let expected = [
        ".#.......#..##......",
        "###.....##....#.....",
        ".#.......#....#.....",
        ".#.......#....#.....",
        ".#.......#...#..#.#.",
        ".#.......#..#...#.#.",
        ".#.......#..#...##..",
        ".#.......#..#...#.#.",
        ".#......###.###.#.#.",
        "....................",
        ".#...........#......",
        ".#..........#.#.....",
        ".#..........#.#.....",
        ".#..........#.#.....",
        ".#..........#.#.##..",
        ".#..........#.#.#.#.",
        ".#..........#.#.##..",
        "###.........#.#.#.#.",
        ".#...........#..##..",
        "....................",
    ];
    let pixmap = render_rates(12_000.0, 0.0);
    for (y, line) in expected.iter().enumerate() {
        for (x, cell) in line.bytes().enumerate() {
            let channel = if cell == b'#' { 255 } else { 0 };
            assert_eq!(pixel(&pixmap, x, y), [channel; 4], "pixel ({x}, {y})");
        }
    }
}

#[test]
fn the_widest_rates_keep_clear_of_their_arrows() {
    let expected = [
        ".#.....#.....#......",
        "###...#.#...#.#.....",
        ".#....#.#...#.#.....",
        ".#....#.#...#.#.....",
        ".#.....##....##.#.#.",
        ".#......#.....#.###.",
        ".#......#.....#.###.",
        ".#......#.....#.#.#.",
        ".#....##..#.##..#.#.",
        "....................",
        ".#...#...#...#......",
        ".#..#.#.#.#.#.#.....",
        ".#..#.#.#.#.#.#.....",
        ".#..#.#.#.#.#.#.....",
        ".#...##..##..##..##.",
        ".#....#...#...#.#...",
        ".#....#...#...#.#.#.",
        "###...#...#...#.#.#.",
        ".#..##..##..##...##.",
        "....................",
    ];
    // "9.9M" over "999G".
    let pixmap = render_rates(9_900_000.0, 1e15);
    for (y, line) in expected.iter().enumerate() {
        for (x, cell) in line.bytes().enumerate() {
            let channel = if cell == b'#' { 255 } else { 0 };
            assert_eq!(pixel(&pixmap, x, y), [channel; 4], "pixel ({x}, {y})");
        }
    }
}

#[test]
fn every_band_fits_in_white_with_clear_gutters() {
    let rates = [
        0.0,
        999.4,
        999.5,
        12_000.0,
        999_499.0,
        999_500.0,
        9_949_999.0,
        9_950_000.0,
        999_499_999.0,
        999_500_000.0,
        1e15,
    ];
    for &upload in &rates {
        for &download in &rates {
            let pixmap = render_rates(upload, download);
            for (index, pixel) in pixmap.chunks_exact(4).enumerate() {
                assert!(
                    pixel == [0; 4] || pixel == [255; 4],
                    "{upload}/{download} pixel {index}: {pixel:?}"
                );
            }
            for y in 0..SIZE {
                for x in [3, 19] {
                    assert_eq!(pixel(&pixmap, x, y)[0], 0, "{upload}/{download} ({x}, {y})");
                }
            }
            for x in 0..SIZE {
                for y in [9, 19] {
                    assert_eq!(pixel(&pixmap, x, y)[0], 0, "{upload}/{download} ({x}, {y})");
                }
            }
        }
    }
}

#[test]
fn swapping_the_rates_swaps_the_lines() {
    let original = render_rates(12_000.0, 0.0);
    let swapped = render_rates(0.0, 12_000.0);
    for y in 0..9 {
        for x in 4..SIZE {
            assert_eq!(pixel(&swapped, x, y + 10), pixel(&original, x, y), "({x}, {y})");
            assert_eq!(pixel(&swapped, x, y), pixel(&original, x, y + 10), "({x}, {y})");
        }
    }
}
```

- [x] **Step 2: Run and expect FAIL.** `cargo test -p statsbar-core --test label` fails to
  compile: "no variant named `Bat`" and "unresolved imports `rate_text`, `render_rates`".
- [x] **Step 3: Implement.** Replace `crates/core/src/label.rs` with the following. The CPU and
  RAM words, digits and percent sign are unchanged. `DIGITS` becomes a `static`, so that a digit
  picked at run time can be borrowed for `'static` in `rate_glyph`.

```rust
//! Draws the tray labels into fixed-size, network-byte-order ARGB pixmaps: a module word over a
//! whole percentage, or an upload line over a download line.

/// Width and height of the pixmap in pixels.
pub const SIZE: usize = 20;

const BYTES_PER_PIXEL: usize = 4;
const PIXMAP_BYTES: usize = SIZE * SIZE * BYTES_PER_PIXEL;
/// One past the last inked column: every right-aligned run ends in column 18.
const LINE_END: usize = SIZE - 1;
const LABEL_TOP: usize = 2;
const VALUE_TOP: usize = 10;
const DIGIT_WIDTH: usize = 3;
const PERCENT_WIDTH: usize = 5;
const LABEL_WIDTH: usize = 11;
const GAP: usize = 1;

/// The short module word shown above the percentage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    Cpu,
    Ram,
    /// The battery, unless it is charging.
    Bat,
    /// The battery while it is charging.
    Chg,
    Fan,
}

const CPU: [&[u8; LABEL_WIDTH]; 5] = [
    b".##.##..#.#",
    b"#...#.#.#.#",
    b"#...##..#.#",
    b"#...#...#.#",
    b".##.#....#.",
];
const RAM: [&[u8; LABEL_WIDTH]; 5] = [
    b"##...#..#.#",
    b"#.#.#.#.###",
    b"##..###.###",
    b"#.#.#.#.#.#",
    b"#.#.#.#.#.#",
];
const BAT: [&[u8; LABEL_WIDTH]; 5] = [
    b"##...#..###",
    b"#.#.#.#..#.",
    b"##..###..#.",
    b"#.#.#.#..#.",
    b"##..#.#..#.",
];
const CHG: [&[u8; LABEL_WIDTH]; 5] = [
    b".##.#.#..##",
    b"#...#.#.#..",
    b"#...###.#.#",
    b"#...#.#.#.#",
    b".##.#.#..##",
];
const FAN: [&[u8; LABEL_WIDTH]; 5] = [
    b"###..#..##.",
    b"#...#.#.#.#",
    b"##..###.#.#",
    b"#...#.#.#.#",
    b"#...#.#.#.#",
];

static DIGITS: [[u8; 9]; 10] = [
    [
        0b010, 0b101, 0b101, 0b101, 0b101, 0b101, 0b101, 0b101, 0b010,
    ],
    [
        0b010, 0b110, 0b010, 0b010, 0b010, 0b010, 0b010, 0b010, 0b111,
    ],
    [
        0b110, 0b001, 0b001, 0b001, 0b010, 0b100, 0b100, 0b100, 0b111,
    ],
    [
        0b110, 0b001, 0b001, 0b001, 0b011, 0b001, 0b001, 0b001, 0b110,
    ],
    [
        0b101, 0b101, 0b101, 0b101, 0b111, 0b001, 0b001, 0b001, 0b001,
    ],
    [
        0b111, 0b100, 0b100, 0b100, 0b110, 0b001, 0b001, 0b001, 0b110,
    ],
    [
        0b011, 0b100, 0b100, 0b100, 0b110, 0b101, 0b101, 0b101, 0b010,
    ],
    [
        0b111, 0b001, 0b001, 0b001, 0b010, 0b010, 0b100, 0b100, 0b100,
    ],
    [
        0b010, 0b101, 0b101, 0b101, 0b010, 0b101, 0b101, 0b101, 0b010,
    ],
    [
        0b010, 0b101, 0b101, 0b101, 0b011, 0b001, 0b001, 0b001, 0b110,
    ],
];
const PERCENT: [u8; 9] = [
    0b11001, 0b11001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01011, 0b10011, 0b10000,
];

/// Top rows of the upload line and the download line.
const RATE_TOPS: [usize; 2] = [0, 10];
const UP: [u8; 9] = [
    0b010, 0b111, 0b010, 0b010, 0b010, 0b010, 0b010, 0b010, 0b010,
];
const DOWN: [u8; 9] = [
    0b010, 0b010, 0b010, 0b010, 0b010, 0b010, 0b010, 0b111, 0b010,
];
/// Unit letters are five rows high and sit on the digits' baseline.
const UNIT_TOP: usize = 4;
const UNIT_B: [u8; 5] = [0b110, 0b101, 0b110, 0b101, 0b110];
const UNIT_K: [u8; 5] = [0b101, 0b101, 0b110, 0b101, 0b101];
const UNIT_M: [u8; 5] = [0b101, 0b111, 0b111, 0b101, 0b101];
const UNIT_G: [u8; 5] = [0b011, 0b100, 0b101, 0b101, 0b011];
const DOT_TOP: usize = 8;
const DOT: [u8; 1] = [0b1];

fn ink(pixmap: &mut [u8; PIXMAP_BYTES], x: usize, y: usize) {
    let offset = (y * SIZE + x) * BYTES_PER_PIXEL;
    pixmap[offset..offset + BYTES_PER_PIXEL].fill(255);
}

/// Draws a glyph whose rows are bit masks, most significant bit leftmost.
fn draw(pixmap: &mut [u8; PIXMAP_BYTES], left: usize, top: usize, width: usize, rows: &[u8]) {
    for (row, &mask) in rows.iter().enumerate() {
        for col in 0..width {
            if mask & (1 << (width - 1 - col)) != 0 {
                ink(pixmap, left + col, top + row);
            }
        }
    }
}

/// Draws a right-aligned metric word and whole percentage in white on transparent pixels.
///
/// Returns one 20×20 pixmap for values from 0 through 100. Values above 100 return
/// None so callers can decide how to represent readings outside the valid range.
pub fn render(metric: Metric, percent: u8) -> Option<[u8; PIXMAP_BYTES]> {
    let digits = [
        usize::from(percent / 100),
        usize::from(percent / 10 % 10),
        usize::from(percent % 10),
    ];
    let digits = match percent {
        0..=9 => &digits[2..],
        10..=99 => &digits[1..],
        100 => &digits[..],
        _ => return None,
    };

    let mut pixmap = [0; PIXMAP_BYTES];
    let word = match metric {
        Metric::Cpu => CPU,
        Metric::Ram => RAM,
        Metric::Bat => BAT,
        Metric::Chg => CHG,
        Metric::Fan => FAN,
    };
    for (row, line) in word.iter().enumerate() {
        for (col, &cell) in line.iter().enumerate() {
            if cell == b'#' {
                ink(&mut pixmap, LINE_END - LABEL_WIDTH + col, LABEL_TOP + row);
            }
        }
    }

    // Anchor the percent sign first, then work left so digit count never shifts the suffix.
    let mut left = LINE_END - PERCENT_WIDTH;
    draw(&mut pixmap, left, VALUE_TOP, PERCENT_WIDTH, &PERCENT);
    for &digit in digits.iter().rev() {
        left -= DIGIT_WIDTH + GAP;
        draw(&mut pixmap, left, VALUE_TOP, DIGIT_WIDTH, &DIGITS[digit]);
    }

    Some(pixmap)
}

/// A rate as the tray draws it: at most three characters of number and a unit letter, `B`, `K`,
/// `M` or `G`, counted in powers of 1000 as network tools do. One decimal only below 10 M and
/// 10 G, as in `format::menu_bar_rate`. 999.5 GB/s and above read `999G`. NaN and anything not
/// above zero read `0B`.
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

struct Glyph {
    width: usize,
    top: usize,
    rows: &'static [u8],
}

/// The glyph of a character `rate_text` produces.
fn rate_glyph(character: u8) -> Option<Glyph> {
    let (width, top, rows): (usize, usize, &'static [u8]) = match character {
        b'0'..=b'9' => (DIGIT_WIDTH, 0, &DIGITS[usize::from(character - b'0')]),
        b'.' => (1, DOT_TOP, &DOT),
        b'B' => (DIGIT_WIDTH, UNIT_TOP, &UNIT_B),
        b'K' => (DIGIT_WIDTH, UNIT_TOP, &UNIT_K),
        b'M' => (DIGIT_WIDTH, UNIT_TOP, &UNIT_M),
        b'G' => (DIGIT_WIDTH, UNIT_TOP, &UNIT_G),
        _ => return None,
    };
    Some(Glyph { width, top, rows })
}

/// Draws the upload rate over the download rate, each behind its arrow and right-aligned, in
/// white on transparent pixels, as upstream's two-line network label does.
pub fn render_rates(
    upload_bytes_per_second: f64,
    download_bytes_per_second: f64,
) -> [u8; PIXMAP_BYTES] {
    let mut pixmap = [0; PIXMAP_BYTES];
    let lines = [
        (&UP, upload_bytes_per_second),
        (&DOWN, download_bytes_per_second),
    ];
    for ((arrow, rate), top) in lines.into_iter().zip(RATE_TOPS) {
        draw(&mut pixmap, 0, top, DIGIT_WIDTH, arrow);
        let mut right = LINE_END;
        for character in rate_text(rate).bytes().rev() {
            let Some(glyph) = rate_glyph(character) else {
                continue;
            };
            // The widest text, "999K", is 15 columns and ends at the arrow's gap, so this never
            // runs out; stopping beats wrapping if a future band were wider.
            let Some(left) = right.checked_sub(glyph.width) else {
                break;
            };
            draw(&mut pixmap, left, top + glyph.top, glyph.width, glyph.rows);
            right = left.saturating_sub(GAP);
        }
    }
    pixmap
}
```

- [x] **Step 4: Run and expect PASS.** `cargo test -p statsbar-core --test label` passes. The
  existing CPU, RAM and digit tests pass unchanged, which shows `render` is unchanged for them.
  `cargo fmt --all` then `./scripts/check.sh` is green: `tray.rs` no longer matches on `Metric` after Task 3.

Ready to commit: `crates/core/src/label.rs crates/core/tests/label.rs` — "feat(core): draw BAT, CHG and FAN words and a two-line rate label"

### Task 5: Battery reader

**Files:** create `crates/readers/src/battery.rs`, `crates/readers/tests/battery.rs`,
`crates/readers/tests/fixtures/normal/sys/class/power_supply/BAT0/uevent`,
`crates/readers/tests/fixtures/normal/sys/class/power_supply/AC/online` and
`crates/readers/tests/fixtures/truncated/sys/class/power_supply/BAT0/uevent`. Modify
`crates/readers/src/lib.rs`.

**Interfaces — Consumes:** `battery::{BatteryInfo, parse_uevent}`, `sysfs::parse_bool`
(Task 1), and `procfile::{ProcFile, invalid_data}`. **Produces:**
`battery::BatteryReading { info: BatteryInfo, adapter_online: Option<bool> }`,
`BatteryReader::open(root: &Path) -> io::Result<BatteryReader>`, and
`BatteryReader::read(&mut self) -> io::Result<BatteryReading>`.

**Test level:** integration against the real file system (fixture trees and scratch roots).

- [x] **Step 1: Fixtures.** Run from the repository root:

```bash
base=crates/readers/tests/fixtures
mkdir -p $base/normal/sys/class/power_supply/BAT0 $base/normal/sys/class/power_supply/AC \
         $base/truncated/sys/class/power_supply/BAT0
cat > $base/normal/sys/class/power_supply/BAT0/uevent <<'EOF'
DEVTYPE=power_supply
POWER_SUPPLY_NAME=BAT0
POWER_SUPPLY_TYPE=Battery
POWER_SUPPLY_STATUS=Not charging
POWER_SUPPLY_PRESENT=1
POWER_SUPPLY_TECHNOLOGY=Li-poly
POWER_SUPPLY_CYCLE_COUNT=0
POWER_SUPPLY_VOLTAGE_MIN_DESIGN=7600000
POWER_SUPPLY_VOLTAGE_NOW=7997000
POWER_SUPPLY_CURRENT_NOW=1000
POWER_SUPPLY_CHARGE_FULL_DESIGN=8948000
POWER_SUPPLY_CHARGE_FULL=5803000
POWER_SUPPLY_CHARGE_NOW=4418000
POWER_SUPPLY_CAPACITY=76
POWER_SUPPLY_CAPACITY_LEVEL=Normal
POWER_SUPPLY_TYPE=Battery
POWER_SUPPLY_MODEL_NAME=DELL C5GV285
POWER_SUPPLY_MANUFACTURER=SMP
POWER_SUPPLY_SERIAL_NUMBER=0000
EOF
printf '1\n' > $base/normal/sys/class/power_supply/AC/online
# Cut mid-line, before POWER_SUPPLY_CAPACITY, as a read racing a driver could leave it.
head -c 300 $base/normal/sys/class/power_supply/BAT0/uevent \
  > $base/truncated/sys/class/power_supply/BAT0/uevent
```

  Check that `grep -c CAPACITY= $base/truncated/sys/class/power_supply/BAT0/uevent` prints `0`.
- [x] **Step 2: Write the failing tests.** `crates/readers/tests/battery.rs`:

```rust
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use statsbar_core::battery::{BatteryInfo, ChargeStatus};
use statsbar_readers::battery::{BatteryReader, BatteryReading};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// A fresh root per test with an empty `BAT0` directory, so parallel tests never share files.
fn scratch(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    match fs::remove_dir_all(&root) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => panic!("clearing {}: {error}", root.display()),
    }
    fs::create_dir_all(root.join("sys/class/power_supply/BAT0")).expect("create BAT0");
    root
}

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().expect("has a parent")).expect("create parent");
    fs::write(path, text).expect("write");
}

fn uevent(capacity: u8) -> String {
    format!("POWER_SUPPLY_STATUS=Discharging\nPOWER_SUPPLY_CAPACITY={capacity}\n")
}

const UEVENT: &str = "sys/class/power_supply/BAT0/uevent";
const ONLINE: &str = "sys/class/power_supply/AC/online";

#[test]
fn reads_the_battery_and_the_adapter_from_the_fixture_tree() {
    let mut reader = BatteryReader::open(&fixture("normal")).expect("fixture exists");
    assert_eq!(
        reader.read().expect("well-formed"),
        BatteryReading {
            info: BatteryInfo {
                status: ChargeStatus::NotCharging,
                capacity_percent: 76,
                charge_now_uah: Some(4_418_000),
                charge_full_uah: Some(5_803_000),
                charge_full_design_uah: Some(8_948_000),
                current_now_ua: Some(1000),
                voltage_now_uv: Some(7_997_000),
                cycle_count: Some(0),
            },
            adapter_online: Some(true),
        }
    );
}

#[test]
fn a_second_read_sees_the_files_as_they_are_now() {
    let root = scratch("battery-reread");
    write(&root, UEVENT, &uevent(50));
    write(&root, ONLINE, "0\n");
    let mut reader = BatteryReader::open(&root).expect("just written");
    let first = reader.read().expect("well-formed");
    assert_eq!(
        (first.info.capacity_percent, first.adapter_online),
        (50, Some(false))
    );
    write(&root, UEVENT, &uevent(51));
    write(&root, ONLINE, "1\n");
    let second = reader.read().expect("well-formed");
    assert_eq!(
        (second.info.capacity_percent, second.adapter_online),
        (51, Some(true))
    );
}

#[test]
fn a_machine_without_an_adapter_reads_none_for_it() {
    let root = scratch("battery-no-adapter");
    write(&root, UEVENT, &uevent(50));
    let mut reader = BatteryReader::open(&root).expect("BAT0 is there");
    assert_eq!(reader.read().expect("well-formed").adapter_online, None);
}

#[test]
fn a_missing_battery_fails_to_open_with_not_found() {
    let error = BatteryReader::open(&fixture("no-such-root"))
        .err()
        .expect("there is no BAT0");
    assert_eq!(error.kind(), ErrorKind::NotFound);
}

#[test]
fn a_truncated_uevent_is_invalid_data() {
    let mut reader = BatteryReader::open(&fixture("truncated")).expect("fixture exists");
    assert_eq!(
        reader.read().expect_err("no capacity").kind(),
        ErrorKind::InvalidData
    );
}

#[test]
fn an_adapter_value_other_than_0_or_1_is_invalid_data() {
    let root = scratch("battery-bad-adapter");
    write(&root, UEVENT, &uevent(50));
    write(&root, ONLINE, "2\n");
    let mut reader = BatteryReader::open(&root).expect("just written");
    assert_eq!(
        reader.read().expect_err("not a flag").kind(),
        ErrorKind::InvalidData
    );
}
```

- [x] **Step 3: Run and expect FAIL.** `cargo test -p statsbar-readers --test battery` fails to
  compile with "unresolved import `statsbar_readers::battery`".
- [x] **Step 4: Implement.** In `crates/readers/src/lib.rs`, add `pub mod battery;` before
  `pub mod cpu;`. `crates/readers/src/battery.rs`:

```rust
//! `/sys/class/power_supply/BAT0/uevent` and `AC/online` → a battery reading.

use std::io;
use std::path::Path;

use statsbar_core::battery::{BatteryInfo, parse_uevent};
use statsbar_core::sysfs::parse_bool;

use crate::procfile::{ProcFile, invalid_data};

/// This laptop's supplies (roadmap requirement 5). One `uevent` read is one ACPI call, where
/// reading the attributes one by one would be ten.
const BATTERY: &str = "sys/class/power_supply/BAT0/uevent";
const ADAPTER: &str = "sys/class/power_supply/AC/online";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatteryReading {
    pub info: BatteryInfo,
    /// `None` when the machine has no `AC` supply.
    pub adapter_online: Option<bool>,
}

pub struct BatteryReader {
    uevent: ProcFile,
    adapter: Option<ProcFile>,
}

impl BatteryReader {
    /// Opens both files. A missing `AC` is not an error, because the battery's figures stand
    /// alone; any other failure to open it is.
    pub fn open(root: &Path) -> io::Result<BatteryReader> {
        let uevent = ProcFile::open(&root.join(BATTERY))?;
        let adapter = match ProcFile::open(&root.join(ADAPTER)) {
            Ok(file) => Some(file),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        Ok(BatteryReader { uevent, adapter })
    }

    pub fn read(&mut self) -> io::Result<BatteryReading> {
        let info = parse_uevent(self.uevent.read()?).ok_or_else(|| {
            invalid_data(
                "BAT0/uevent lacks a status or a capacity from 0 to 100, or has a malformed number",
            )
        })?;
        let adapter_online = match &mut self.adapter {
            Some(file) => Some(
                parse_bool(file.read()?)
                    .ok_or_else(|| invalid_data("AC/online is neither 0 nor 1"))?,
            ),
            None => None,
        };
        Ok(BatteryReading {
            info,
            adapter_online,
        })
    }
}
```

- [x] **Step 5: Run and expect PASS.** `cargo test -p statsbar-readers --test battery` passes,
  and `cargo fmt --all` then `./scripts/check.sh` is green.

Ready to commit: `crates/readers/src/lib.rs crates/readers/src/battery.rs crates/readers/tests/battery.rs crates/readers/tests/fixtures/normal/sys/class/power_supply crates/readers/tests/fixtures/truncated/sys/class/power_supply` — "feat(readers): read the battery uevent and the adapter state"

### Task 6: Fan reader

**Files:** create `crates/readers/src/fan.rs`, `crates/readers/tests/fan.rs`, and fixture files
under `crates/readers/tests/fixtures/{normal,truncated}/sys/class/hwmon/`. Modify
`crates/readers/src/lib.rs`.

**Interfaces — Consumes:** `fan::FanSpeed` (Task 2), `sysfs::parse_u64` (Task 1), and
`ProcFile`. **Produces:** `FanReader::open(root: &Path) -> io::Result<FanReader>` and
`FanReader::read(&mut self) -> io::Result<FanSpeed>`.

**Test level:** integration against the real file system.

- [x] **Step 1: Fixtures.** Run from the repository root:

```bash
base=crates/readers/tests/fixtures
mkdir -p $base/normal/sys/class/hwmon/hwmon0 $base/normal/sys/class/hwmon/hwmon7 \
         $base/truncated/sys/class/hwmon/hwmon7
printf 'AC\n' > $base/normal/sys/class/hwmon/hwmon0/name
printf 'dell_smm\n' > $base/normal/sys/class/hwmon/hwmon7/name
printf '6040\n' > $base/normal/sys/class/hwmon/hwmon7/fan1_input
printf '0\n' > $base/normal/sys/class/hwmon/hwmon7/fan1_min
printf '5300\n' > $base/normal/sys/class/hwmon/hwmon7/fan1_max
printf 'dell_smm\n' > $base/truncated/sys/class/hwmon/hwmon7/name
: > $base/truncated/sys/class/hwmon/hwmon7/fan1_input
printf '0\n' > $base/truncated/sys/class/hwmon/hwmon7/fan1_min
printf '5300\n' > $base/truncated/sys/class/hwmon/hwmon7/fan1_max
```

- [x] **Step 2: Write the failing tests.** `crates/readers/tests/fan.rs`:

```rust
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use statsbar_core::fan::FanSpeed;
use statsbar_readers::fan::FanReader;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn scratch(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    match fs::remove_dir_all(&root) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => panic!("clearing {}: {error}", root.display()),
    }
    root
}

/// Writes an hwmon device with the given files.
fn device(root: &Path, number: u8, files: &[(&str, &str)]) {
    let dir = root.join(format!("sys/class/hwmon/hwmon{number}"));
    fs::create_dir_all(&dir).expect("create device dir");
    for (name, text) in files {
        fs::write(dir.join(name), text).expect("write");
    }
}

#[test]
fn finds_dell_smm_among_the_hwmon_devices_and_reads_its_fan() {
    let mut reader = FanReader::open(&fixture("normal")).expect("fixture exists");
    assert_eq!(
        reader.read().expect("well-formed"),
        FanSpeed {
            rpm: 6040,
            min_rpm: 0,
            max_rpm: 5300
        }
    );
}

#[test]
fn a_second_read_sees_the_new_speed() {
    let root = scratch("fan-reread");
    let limits = [("name", "dell_smm\n"), ("fan1_min", "0\n"), ("fan1_max", "5300\n")];
    device(&root, 3, &[limits[0], limits[1], limits[2], ("fan1_input", "3000\n")]);
    let mut reader = FanReader::open(&root).expect("just written");
    assert_eq!(reader.read().expect("well-formed").rpm, 3000);
    device(&root, 3, &[("fan1_input", "3100\n")]);
    assert_eq!(reader.read().expect("well-formed").rpm, 3100);
}

#[test]
fn no_dell_smm_device_is_not_found() {
    let root = scratch("fan-no-device");
    device(&root, 0, &[("name", "AC\n")]);
    let error = FanReader::open(&root).err().expect("no dell_smm");
    assert_eq!(error.kind(), ErrorKind::NotFound);
}

#[test]
fn a_device_without_a_name_is_skipped() {
    let root = scratch("fan-nameless");
    device(&root, 0, &[]);
    device(
        &root,
        1,
        &[
            ("name", "dell_smm\n"),
            ("fan1_input", "2000\n"),
            ("fan1_min", "0\n"),
            ("fan1_max", "5300\n"),
        ],
    );
    let mut reader = FanReader::open(&root).expect("hwmon1 is dell_smm");
    assert_eq!(reader.read().expect("well-formed").rpm, 2000);
}

#[test]
fn no_hwmon_directory_is_not_found() {
    let error = FanReader::open(&fixture("no-such-root"))
        .err()
        .expect("no sys/class/hwmon");
    assert_eq!(error.kind(), ErrorKind::NotFound);
}

#[test]
fn an_empty_speed_is_invalid_data() {
    let mut reader = FanReader::open(&fixture("truncated")).expect("fixture exists");
    assert_eq!(
        reader.read().expect_err("empty fan1_input").kind(),
        ErrorKind::InvalidData
    );
}

#[test]
fn a_malformed_limit_fails_to_open_with_invalid_data() {
    let root = scratch("fan-bad-limit");
    device(
        &root,
        7,
        &[
            ("name", "dell_smm\n"),
            ("fan1_input", "2000\n"),
            ("fan1_min", "0\n"),
            ("fan1_max", "lots\n"),
        ],
    );
    let error = FanReader::open(&root).err().expect("fan1_max is not a number");
    assert_eq!(error.kind(), ErrorKind::InvalidData);
}
```

- [x] **Step 3: Run and expect FAIL.** `cargo test -p statsbar-readers --test fan` fails to
  compile with "unresolved import `statsbar_readers::fan`".
- [x] **Step 4: Implement.** In `crates/readers/src/lib.rs`, add `pub mod fan;` after
  `pub mod cpu;`. `crates/readers/src/fan.rs`:

```rust
//! The `dell_smm` hwmon device's `fan1_*` attributes → a fan speed.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use statsbar_core::fan::FanSpeed;
use statsbar_core::sysfs::parse_u64;

use crate::procfile::{ProcFile, invalid_data};

const HWMON: &str = "sys/class/hwmon";
/// hwmon numbers change between boots, so the device is found by its `name`.
const DRIVER: &str = "dell_smm";

pub struct FanReader {
    input: ProcFile,
    min_rpm: u64,
    max_rpm: u64,
}

impl FanReader {
    /// Finds the device and reads its limits, which the driver sets when it loads. Only
    /// `fan1_input` is kept open, because each read of it is a BIOS (SMM) call: 0.7 ms here.
    pub fn open(root: &Path) -> io::Result<FanReader> {
        let device = find_device(&root.join(HWMON))?;
        Ok(FanReader {
            min_rpm: read_rpm(&device.join("fan1_min"))?,
            max_rpm: read_rpm(&device.join("fan1_max"))?,
            input: ProcFile::open(&device.join("fan1_input"))?,
        })
    }

    pub fn read(&mut self) -> io::Result<FanSpeed> {
        let rpm = parse_u64(self.input.read()?)
            .ok_or_else(|| invalid_data("fan1_input is not a number"))?;
        Ok(FanSpeed {
            rpm,
            min_rpm: self.min_rpm,
            max_rpm: self.max_rpm,
        })
    }
}

fn find_device(hwmon: &Path) -> io::Result<PathBuf> {
    for entry in fs::read_dir(hwmon)? {
        let device = entry?.path();
        let mut name = match ProcFile::open(&device.join("name")) {
            Ok(file) => file,
            // A device without a name cannot be the one looked for.
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        if name.read()?.trim_end() == DRIVER {
            return Ok(device);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("no {DRIVER} device under {}", hwmon.display()),
    ))
}

fn read_rpm(path: &Path) -> io::Result<u64> {
    let mut file = ProcFile::open(path)?;
    parse_u64(file.read()?)
        .ok_or_else(|| invalid_data(&format!("{} is not a number", path.display())))
}
```

- [x] **Step 5: Run and expect PASS.** `cargo test -p statsbar-readers --test fan` passes, and
  `cargo fmt --all` then `./scripts/check.sh` is green.

Ready to commit: `crates/readers/src/lib.rs crates/readers/src/fan.rs crates/readers/tests/fan.rs crates/readers/tests/fixtures/normal/sys/class/hwmon crates/readers/tests/fixtures/truncated/sys/class/hwmon` — "feat(readers): find the dell_smm fan by name and read its speed"

### Task 7: Network reader

**Files:** create `crates/readers/src/network.rs`, `crates/readers/tests/network.rs`,
`crates/readers/tests/fixtures/normal/proc/net/route`,
`crates/readers/tests/fixtures/normal/sys/class/net/wlo1/statistics/{rx_bytes,tx_bytes}` and
`crates/readers/tests/fixtures/truncated/proc/net/route`. Modify `crates/readers/src/lib.rs`.

**Interfaces — Consumes:** `network::default_route_interface` (Task 2), `sysfs::parse_u64`,
`throughput::Sample` (Plan 01), and `ProcFile`. **Produces:**
`NetworkReader::open(root: &Path) -> io::Result<NetworkReader>` and
`NetworkReader::read(&mut self) -> io::Result<Option<Sample>>`.

**Test level:** integration against the real file system.

- [x] **Step 1: Fixtures.** Run from the repository root:

```bash
base=crates/readers/tests/fixtures
mkdir -p $base/normal/proc/net $base/normal/sys/class/net/wlo1/statistics $base/truncated/proc/net
printf 'Iface\tDestination\tGateway \tFlags\tRefCnt\tUse\tMetric\tMask\t\tMTU\tWindow\tIRTT\nwlo1\t00000000\t0164A8C0\t0003\t0\t0\t600\t00000000\t0\t0\t0\nwlo1\t0064A8C0\t00000000\t0001\t0\t0\t600\t00FFFFFF\t0\t0\t0\n' \
  > $base/normal/proc/net/route
printf '678488787\n' > $base/normal/sys/class/net/wlo1/statistics/rx_bytes
printf '51234567\n' > $base/normal/sys/class/net/wlo1/statistics/tx_bytes
printf 'Iface\tDestination\tGateway \tFlags\nwlo1\t0000' > $base/truncated/proc/net/route
```

- [x] **Step 2: Write the failing tests.** `crates/readers/tests/network.rs`:

```rust
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use statsbar_core::throughput::Sample;
use statsbar_readers::network::NetworkReader;

const HEADER: &str =
    "Iface\tDestination\tGateway \tFlags\tRefCnt\tUse\tMetric\tMask\t\tMTU\tWindow\tIRTT\n";

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn scratch(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    match fs::remove_dir_all(&root) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => panic!("clearing {}: {error}", root.display()),
    }
    fs::create_dir_all(root.join("proc/net")).expect("create proc/net");
    root
}

fn route_to(root: &Path, interface: Option<&str>) {
    let line = interface.map_or(String::new(), |name| {
        format!("{name}\t00000000\t0164A8C0\t0003\t0\t0\t600\t00000000\t0\t0\t0\n")
    });
    fs::write(root.join("proc/net/route"), format!("{HEADER}{line}")).expect("write route");
}

fn counters(root: &Path, interface: &str, rx: u64, tx: u64) {
    let dir = root.join(format!("sys/class/net/{interface}/statistics"));
    fs::create_dir_all(&dir).expect("create statistics");
    fs::write(dir.join("rx_bytes"), format!("{rx}\n")).expect("write rx");
    fs::write(dir.join("tx_bytes"), format!("{tx}\n")).expect("write tx");
}

fn sample(interface: &str, rx: u64, tx: u64) -> Option<Sample> {
    Some(Sample {
        interface: interface.to_owned(),
        rx_bytes: rx,
        tx_bytes: tx,
    })
}

#[test]
fn reads_the_default_route_interfaces_counters() {
    let mut reader = NetworkReader::open(&fixture("normal")).expect("fixture exists");
    assert_eq!(
        reader.read().expect("well-formed"),
        sample("wlo1", 678_488_787, 51_234_567)
    );
}

#[test]
fn counters_are_reread_on_each_sample() {
    let root = scratch("network-reread");
    route_to(&root, Some("wlo1"));
    counters(&root, "wlo1", 100, 10);
    let mut reader = NetworkReader::open(&root).expect("just written");
    assert_eq!(reader.read().expect("well-formed"), sample("wlo1", 100, 10));
    counters(&root, "wlo1", 200, 20);
    assert_eq!(reader.read().expect("well-formed"), sample("wlo1", 200, 20));
}

#[test]
fn a_route_change_switches_to_the_new_interfaces_counters() {
    let root = scratch("network-switch");
    route_to(&root, Some("wlo1"));
    counters(&root, "wlo1", 100, 10);
    counters(&root, "eno2", 5000, 500);
    let mut reader = NetworkReader::open(&root).expect("just written");
    assert_eq!(reader.read().expect("well-formed"), sample("wlo1", 100, 10));
    route_to(&root, Some("eno2"));
    assert_eq!(reader.read().expect("well-formed"), sample("eno2", 5000, 500));
}

#[test]
fn no_default_route_reads_none() {
    let root = scratch("network-no-route");
    route_to(&root, None);
    let mut reader = NetworkReader::open(&root).expect("just written");
    assert_eq!(reader.read().expect("well-formed"), None);
}

#[test]
fn a_default_route_to_an_interface_without_counters_is_not_found() {
    let root = scratch("network-ghost");
    route_to(&root, Some("ghost0"));
    let mut reader = NetworkReader::open(&root).expect("just written");
    assert_eq!(
        reader.read().expect_err("no counters").kind(),
        ErrorKind::NotFound
    );
}

#[test]
fn a_malformed_route_table_is_invalid_data() {
    let mut reader = NetworkReader::open(&fixture("truncated")).expect("fixture exists");
    assert_eq!(
        reader.read().expect_err("cut mid-line").kind(),
        ErrorKind::InvalidData
    );
}

#[test]
fn a_missing_route_table_fails_to_open_with_not_found() {
    let error = NetworkReader::open(&fixture("no-such-root"))
        .err()
        .expect("no proc/net/route");
    assert_eq!(error.kind(), ErrorKind::NotFound);
}
```

- [x] **Step 3: Run and expect FAIL.** `cargo test -p statsbar-readers --test network` fails
  to compile with "unresolved import `statsbar_readers::network`".
- [x] **Step 4: Implement.** In `crates/readers/src/lib.rs`, add `pub mod network;` after
  `pub mod memory;`. `crates/readers/src/network.rs`:

```rust
//! `/proc/net/route` and `/sys/class/net/<interface>/statistics/{rx,tx}_bytes` → a counter sample.

use std::io;
use std::path::{Path, PathBuf};

use statsbar_core::network::default_route_interface;
use statsbar_core::sysfs::parse_u64;
use statsbar_core::throughput::Sample;

use crate::procfile::{ProcFile, invalid_data};

pub struct NetworkReader {
    root: PathBuf,
    route: ProcFile,
    counters: Option<Counters>,
}

/// The counter files of the interface the default route used last time, kept open while it
/// stays the same.
struct Counters {
    interface: String,
    rx: ProcFile,
    tx: ProcFile,
}

impl Counters {
    /// `interface` has passed `network::is_interface_name`, so it stays under `sys/class/net`.
    fn open(root: &Path, interface: String) -> io::Result<Counters> {
        let statistics = root.join("sys/class/net").join(&interface).join("statistics");
        Ok(Counters {
            rx: ProcFile::open(&statistics.join("rx_bytes"))?,
            tx: ProcFile::open(&statistics.join("tx_bytes"))?,
            interface,
        })
    }
}

impl NetworkReader {
    pub fn open(root: &Path) -> io::Result<NetworkReader> {
        Ok(NetworkReader {
            root: root.to_owned(),
            route: ProcFile::open(&root.join("proc/net/route"))?,
            counters: None,
        })
    }

    /// The default route's interface and its counters, or `None` without a default route.
    pub fn read(&mut self) -> io::Result<Option<Sample>> {
        let interface = default_route_interface(self.route.read()?)
            .map_err(|error| invalid_data(&error.to_string()))?;
        let Some(interface) = interface else {
            self.counters = None;
            return Ok(None);
        };
        let counters = match self.counters.take() {
            Some(counters) if counters.interface == interface => counters,
            _ => Counters::open(&self.root, interface)?,
        };
        let counters = self.counters.insert(counters);
        let rx_bytes = read_counter(&mut counters.rx, "rx_bytes")?;
        let tx_bytes = read_counter(&mut counters.tx, "tx_bytes")?;
        Ok(Some(Sample {
            interface: counters.interface.clone(),
            rx_bytes,
            tx_bytes,
        }))
    }
}

fn read_counter(file: &mut ProcFile, name: &str) -> io::Result<u64> {
    parse_u64(file.read()?).ok_or_else(|| invalid_data(&format!("{name} is not a number")))
}
```

- [x] **Step 5: Run and expect PASS.** `cargo test -p statsbar-readers --test network` passes,
  and `cargo fmt --all` then `./scripts/check.sh` is green.

Ready to commit: `crates/readers/src/lib.rs crates/readers/src/network.rs crates/readers/tests/network.rs crates/readers/tests/fixtures/normal/proc/net crates/readers/tests/fixtures/normal/sys/class/net crates/readers/tests/fixtures/truncated/proc/net` — "feat(readers): follow the default route to its interface counters"

### Task 8: Config keys for the new items

**Files:** modify `crates/daemon/src/config.rs`, `crates/daemon/tests/config.rs` and
`crates/daemon/tests/app.rs`.

**Interfaces — Produces:** `config::Items { cpu, memory, battery, fan, network }`, all `bool` and
all `true` by default. `update_text` writes all five keys.

**Test level:** unit (parse, text), plus the existing file-system tests.

- [x] **Step 1: Write the failing tests.** In `crates/daemon/tests/config.rs`, change `shown` to
  keep its meaning ("only these two flags set as given, the rest default"):

```rust
fn shown(cpu: bool, memory: bool) -> Config {
    Config {
        items: Items {
            cpu,
            memory,
            ..Items::default()
        },
    }
}
```

  and append:

```rust
#[test]
fn the_new_items_are_shown_by_default_and_can_be_hidden() {
    let defaults = config::parse("").expect("empty is valid").items;
    assert!(defaults.battery && defaults.fan && defaults.network);
    let hidden = config::parse("[items]\nbattery = false\nfan = false\nnetwork = false\n")
        .expect("valid")
        .items;
    assert!(!hidden.battery && !hidden.fan && !hidden.network);
    assert!(hidden.cpu && hidden.memory);
}

#[test]
fn saving_writes_every_item_key() {
    let config = Config {
        items: Items {
            cpu: true,
            memory: false,
            battery: false,
            fan: true,
            network: false,
        },
    };
    let text = config::update_text("", &config).expect("valid input");
    let table: toml::Table = text.parse().expect("output is TOML");
    let items = &table["items"];
    for (key, expected) in [
        ("cpu", true),
        ("memory", false),
        ("battery", false),
        ("fan", true),
        ("network", false),
    ] {
        assert_eq!(items[key].as_bool(), Some(expected), "{key}");
    }
}
```

  In `crates/daemon/tests/app.rs`, the struct literal in `a_hidden_item_is_never_read` becomes
  `Items { cpu: false, memory: true, ..Items::default() }`. App still has two readers here, so its
  assertions are unchanged.
- [x] **Step 2: Run and expect FAIL.** `cargo test -p statsbard --test config` fails to compile:
  "struct `Items` has no field named `battery`".
- [x] **Step 3: Implement.** In `crates/daemon/src/config.rs`, replace `Items`, its `Default`, and
  the two `items.insert` calls in `update_text`:

```rust
/// Which tray items are shown. The hub is always shown, so it has no key.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Items {
    pub cpu: bool,
    pub memory: bool,
    pub battery: bool,
    pub fan: bool,
    pub network: bool,
}

impl Default for Items {
    fn default() -> Items {
        Items {
            cpu: true,
            memory: true,
            battery: true,
            fan: true,
            network: true,
        }
    }
}
```

```rust
    let shown = &config.items;
    for (key, value) in [
        ("cpu", shown.cpu),
        ("memory", shown.memory),
        ("battery", shown.battery),
        ("fan", shown.fan),
        ("network", shown.network),
    ] {
        items.insert(key.to_owned(), toml::Value::Boolean(value));
    }
```

- [x] **Step 4: Run and expect PASS.** `cargo test -p statsbard` passes, and `cargo fmt --all` then
  `./scripts/check.sh` is green.

Ready to commit: `crates/daemon/src/config.rs crates/daemon/tests/config.rs crates/daemon/tests/app.rs` — "feat(daemon): add battery, fan and network switches to the config"

### Task 9: Menu text for Battery, Fan and Network

**Files:** modify `crates/daemon/src/summary.rs`, `crates/daemon/src/tray.rs`,
`crates/daemon/src/run.rs` and `crates/daemon/tests/summary.rs`.

**Interfaces — Consumes:** `BatteryFigures`, `Estimate` (Task 1), `FanSpeed` (Task 2),
`throughput::Reading`, and `config::Items` (Task 8). **Produces:**
- `summary::battery_rows(battery: &BatteryFigures) -> Vec<String>`
- `summary::fan_rows(fan: &FanSpeed) -> Vec<String>`
- `summary::network_rows(interface: Option<&str>, reading: &Reading) -> Vec<String>`
- `summary::network_title(reading: &Reading) -> String`
- `summary::Entry<T> { Hidden, Waiting, Value(T) }`, with `Entry::waiting(shown: bool)` and
  `Entry::of(shown: bool, value: Option<T>)`
- `summary::Overview<'a> { cpu, memory, battery, fan, network }` and
  `Overview::waiting(items: &Items) -> Overview<'static>`
- `summary::hub_rows(overview: &Overview) -> Vec<String>`, which replaces the four-argument form

**Test level:** unit.

- [x] **Step 1: Write the failing tests.** In `crates/daemon/tests/summary.rs`, replace the imports
  and the two `hub_rows` tests (`hub_rows_cover_shown_metrics_only`,
  `a_shown_metric_without_a_value_yet_reads_a_dash`) with the following. Their assertions on CPU
  and Memory carry over unchanged.

```rust
use statsbar_core::battery::{BatteryFigures, ChargeStatus, Estimate};
use statsbar_core::cpu::CpuLoad;
use statsbar_core::fan::FanSpeed;
use statsbar_core::memory::MemoryBuckets;
use statsbar_core::throughput::Reading;
use statsbard::config::Items;
use statsbard::summary::{
    Entry, Overview, battery_rows, cpu_rows, fan_rows, hub_rows, memory_rows, network_rows,
    network_title, percent, title,
};
```

```rust
fn battery() -> BatteryFigures {
    BatteryFigures {
        percent: 76,
        status: ChargeStatus::Discharging,
        estimate: Some(Estimate::ToEmpty { minutes: 125 }),
        power_watts: Some(7.96),
        health_percent: Some(64.85),
        full_mah: Some(5803),
        design_mah: Some(8948),
        voltage_volts: Some(7.997),
        cycles: Some(312),
        adapter_online: Some(false),
    }
}

fn fan() -> FanSpeed {
    FanSpeed {
        rpm: 6040,
        min_rpm: 0,
        max_rpm: 5300,
    }
}

fn reading() -> Reading {
    Reading {
        download_total: 1_200_000_000,
        upload_total: 300_000_000,
        download_rate: 1_200_000.0,
        upload_rate: 34_000.0,
    }
}

fn cpu_and_memory<'a>(
    cpu: Entry<&'a CpuLoad>,
    memory: Entry<&'a MemoryBuckets>,
) -> Overview<'a> {
    Overview {
        cpu,
        memory,
        battery: Entry::Hidden,
        fan: Entry::Hidden,
        network: Entry::Hidden,
    }
}

#[test]
fn hub_rows_cover_shown_metrics_only() {
    let (cpu, memory) = (load(), buckets(0));
    assert_eq!(
        hub_rows(&cpu_and_memory(Entry::Value(&cpu), Entry::Value(&memory))),
        ["CPU 24 %", "Memory 50 % · 8.00 GB / 16.00 GB"]
    );
    assert_eq!(
        hub_rows(&cpu_and_memory(Entry::Hidden, Entry::Value(&memory))).len(),
        1
    );
    assert!(hub_rows(&cpu_and_memory(Entry::Hidden, Entry::Hidden)).is_empty());
}

#[test]
fn a_shown_metric_without_a_value_yet_reads_a_dash() {
    assert_eq!(
        hub_rows(&Overview::waiting(&Items::default())),
        ["CPU —", "Memory —", "Battery —", "Fan —", "Network —"]
    );
    let only_fan = Items {
        cpu: false,
        memory: false,
        battery: false,
        fan: true,
        network: false,
    };
    assert_eq!(hub_rows(&Overview::waiting(&only_fan)), ["Fan —"]);
}

#[test]
fn hub_rows_for_battery_fan_and_network_follow_cpu_and_memory() {
    let (battery, fan, reading) = (battery(), fan(), reading());
    let overview = Overview {
        cpu: Entry::Waiting,
        memory: Entry::Hidden,
        battery: Entry::Value(&battery),
        fan: Entry::Value(&fan),
        network: Entry::Value(&reading),
    };
    assert_eq!(
        hub_rows(&overview),
        [
            "CPU —",
            "Battery 76 % · Discharging",
            "Fan 6040 RPM",
            "Network ↓ 1.2 MB/s ↑ 34.0 KB/s",
        ]
    );
}

#[test]
fn entry_of_hides_waits_or_shows() {
    assert_eq!(Entry::of(false, Some(1)), Entry::Hidden);
    assert_eq!(Entry::<u8>::of(true, None), Entry::Waiting);
    assert_eq!(Entry::of(true, Some(1)), Entry::Value(1));
    assert_eq!(Entry::<u8>::waiting(false), Entry::Hidden);
    assert_eq!(Entry::<u8>::waiting(true), Entry::Waiting);
}

#[test]
fn battery_rows_show_every_known_figure_in_order() {
    assert_eq!(
        battery_rows(&battery()),
        [
            "Charge 76 %",
            "Status Discharging",
            "Time left 2h 05m",
            "Power 8.0 W",
            "Health 65 %",
            "Capacity 5803 / 8948 mAh",
            "Voltage 8.00 V",
            "Cycles 312",
            "Adapter not connected",
        ]
    );
}

#[test]
fn charging_shows_time_to_full_and_a_connected_adapter() {
    let charging = BatteryFigures {
        status: ChargeStatus::Charging,
        estimate: Some(Estimate::ToFull { minutes: 40 }),
        adapter_online: Some(true),
        ..battery()
    };
    let rows = battery_rows(&charging);
    assert_eq!(rows[1..3], ["Status Charging", "Time to full 0h 40m"]);
    assert_eq!(rows.last().map(String::as_str), Some("Adapter connected"));
}

#[test]
fn unknown_battery_figures_have_no_row() {
    let bare = BatteryFigures {
        percent: 76,
        status: ChargeStatus::NotCharging,
        estimate: None,
        power_watts: None,
        health_percent: None,
        full_mah: None,
        design_mah: Some(8948),
        voltage_volts: None,
        cycles: None,
        adapter_online: None,
    };
    assert_eq!(battery_rows(&bare), ["Charge 76 %", "Status Not charging"]);
}

#[test]
fn health_above_100_is_shown_as_it_is() {
    let new = BatteryFigures {
        health_percent: Some(103.4),
        ..battery()
    };
    assert!(battery_rows(&new).contains(&"Health 103 %".to_owned()));
}

#[test]
fn fan_rows_show_speed_and_max() {
    assert_eq!(fan_rows(&fan()), ["Speed 6040 RPM", "Max 5300 RPM"]);
}

#[test]
fn network_rows_show_interface_rates_and_session_totals() {
    assert_eq!(
        network_rows(Some("wlo1"), &reading()),
        [
            "Interface wlo1",
            "Download 1.2 MB/s",
            "Upload 34.0 KB/s",
            "Downloaded 1.2 GB",
            "Uploaded 300.0 MB",
        ]
    );
    assert_eq!(
        network_rows(None, &Reading::default())[0],
        "No active interface"
    );
}

#[test]
fn the_network_title_carries_both_rates() {
    assert_eq!(network_title(&reading()), "Network ↓ 1.2 MB/s ↑ 34.0 KB/s");
}
```

- [x] **Step 2: Run and expect FAIL.** `cargo test -p statsbard --test summary` fails to
  compile: "unresolved imports `Entry`, `Overview`, `battery_rows`, …".
- [x] **Step 3: Implement `summary.rs`.** Replace the imports and `hub_rows`, keep `percent`,
  `cpu_rows`, `memory_rows` and `title`, and add the rest:

```rust
use statsbar_core::battery::{BatteryFigures, Estimate};
use statsbar_core::cpu::CpuLoad;
use statsbar_core::fan::FanSpeed;
use statsbar_core::format::{fmt_bytes, fmt_gb, fmt_minutes, fmt_rate, rounded_int};
use statsbar_core::memory::MemoryBuckets;
use statsbar_core::throughput::Reading;

use crate::config::Items;
```

```rust
pub fn battery_rows(battery: &BatteryFigures) -> Vec<String> {
    let mut rows = vec![
        format!("Charge {} %", battery.percent),
        format!("Status {}", battery.status.label()),
    ];
    match battery.estimate {
        Some(Estimate::ToEmpty { minutes }) => {
            rows.push(format!("Time left {}", fmt_minutes(whole_minutes(minutes))));
        }
        Some(Estimate::ToFull { minutes }) => {
            rows.push(format!("Time to full {}", fmt_minutes(whole_minutes(minutes))));
        }
        None => {}
    }
    if let Some(watts) = battery.power_watts {
        rows.push(format!("Power {watts:.1} W"));
    }
    if let Some(health) = battery.health_percent {
        rows.push(format!("Health {} %", rounded_int(health)));
    }
    if let (Some(full), Some(design)) = (battery.full_mah, battery.design_mah) {
        rows.push(format!("Capacity {full} / {design} mAh"));
    }
    if let Some(volts) = battery.voltage_volts {
        rows.push(format!("Voltage {volts:.2} V"));
    }
    if let Some(cycles) = battery.cycles {
        rows.push(format!("Cycles {cycles}"));
    }
    match battery.adapter_online {
        Some(true) => rows.push("Adapter connected".to_owned()),
        Some(false) => rows.push("Adapter not connected".to_owned()),
        None => {}
    }
    rows
}

/// Estimates are at most a day (`battery::MAX_ESTIMATE_MINUTES`), so this never saturates.
fn whole_minutes(minutes: u64) -> i64 {
    i64::try_from(minutes).unwrap_or(i64::MAX)
}

pub fn fan_rows(fan: &FanSpeed) -> Vec<String> {
    vec![
        format!("Speed {} RPM", fan.rpm),
        format!("Max {} RPM", fan.max_rpm),
    ]
}

/// The totals count from when the item was shown or the interface changed.
pub fn network_rows(interface: Option<&str>, reading: &Reading) -> Vec<String> {
    vec![
        match interface {
            Some(name) => format!("Interface {name}"),
            None => "No active interface".to_owned(),
        },
        format!("Download {}", fmt_rate(reading.download_rate)),
        format!("Upload {}", fmt_rate(reading.upload_rate)),
        format!("Downloaded {}", fmt_bytes(reading.download_total)),
        format!("Uploaded {}", fmt_bytes(reading.upload_total)),
    ]
}

/// The network item's title and its hub row.
pub fn network_title(reading: &Reading) -> String {
    format!(
        "Network ↓ {} ↑ {}",
        fmt_rate(reading.download_rate),
        fmt_rate(reading.upload_rate)
    )
}

/// What the hub shows for one metric.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Entry<T> {
    /// The item is hidden, so the metric is not read and has no row.
    Hidden,
    /// Shown, but not read yet: the row reads `CPU —`.
    Waiting,
    Value(T),
}

impl<T> Entry<T> {
    pub fn waiting(shown: bool) -> Entry<T> {
        Entry::of(shown, None)
    }

    pub fn of(shown: bool, value: Option<T>) -> Entry<T> {
        match (shown, value) {
            (false, _) => Entry::Hidden,
            (true, None) => Entry::Waiting,
            (true, Some(value)) => Entry::Value(value),
        }
    }
}

/// The hub's overview, one entry per metric in hub order.
#[derive(Debug, Clone, Copy)]
pub struct Overview<'a> {
    pub cpu: Entry<&'a CpuLoad>,
    pub memory: Entry<&'a MemoryBuckets>,
    pub battery: Entry<&'a BatteryFigures>,
    pub fan: Entry<&'a FanSpeed>,
    pub network: Entry<&'a Reading>,
}

impl Overview<'_> {
    /// Every shown metric waiting for its first value: what a new hub starts with.
    pub fn waiting(items: &Items) -> Overview<'static> {
        Overview {
            cpu: Entry::waiting(items.cpu),
            memory: Entry::waiting(items.memory),
            battery: Entry::waiting(items.battery),
            fan: Entry::waiting(items.fan),
            network: Entry::waiting(items.network),
        }
    }
}

/// The hub's rows. A hidden metric is not read, so it has no row.
pub fn hub_rows(overview: &Overview) -> Vec<String> {
    let mut rows = Vec::with_capacity(5);
    push(&mut rows, "CPU", overview.cpu, |load| {
        format!("CPU {} %", percent(load.usage_percent()))
    });
    push(&mut rows, "Memory", overview.memory, |memory| {
        format!(
            "Memory {} % · {} / {}",
            percent(memory.usage_percent()),
            fmt_gb(memory.used()),
            fmt_gb(memory.total)
        )
    });
    push(&mut rows, "Battery", overview.battery, |battery| {
        format!("Battery {} % · {}", battery.percent, battery.status.label())
    });
    push(&mut rows, "Fan", overview.fan, |fan| {
        format!("Fan {} RPM", fan.rpm)
    });
    push(&mut rows, "Network", overview.network, network_title);
    rows
}

fn push<T>(rows: &mut Vec<String>, name: &str, entry: Entry<T>, text: impl FnOnce(T) -> String) {
    match entry {
        Entry::Hidden => {}
        Entry::Waiting => rows.push(format!("{name} —")),
        Entry::Value(value) => rows.push(text(value)),
    }
}
```

- [x] **Step 4: Update the two callers.** Until Task 10 wires them in, Battery, Fan and Network
  are not shown, so they are `Hidden` here. In `tray.rs` `Hub::new`, replace the `rows:`
  initialiser (keep its comment) with:

```rust
            rows: summary::hub_rows(&Overview {
                cpu: Entry::waiting(show_cpu),
                memory: Entry::waiting(show_memory),
                battery: Entry::Hidden,
                fan: Entry::Hidden,
                network: Entry::Hidden,
            }),
```

  and add `use crate::summary::{Entry, Overview};`. In `run.rs` `refresh_hub`, replace the
  `summary::hub_rows(…)` call with:

```rust
            summary::hub_rows(&Overview {
                cpu: Entry::of(config.items.cpu, self.app.cpu()),
                memory: Entry::of(config.items.memory, self.app.memory()),
                battery: Entry::Hidden,
                fan: Entry::Hidden,
                network: Entry::Hidden,
            }),
```

  and add `use crate::summary::{Entry, Overview};`.
- [x] **Step 5: Run and expect PASS.** `cargo test -p statsbard` passes, including the
  unchanged `tests/tray.rs`, and `cargo fmt --all` then `./scripts/check.sh` is green.

Ready to commit: `crates/daemon/src/summary.rs crates/daemon/src/tray.rs crates/daemon/src/run.rs crates/daemon/tests/summary.rs` — "feat(daemon): write the battery, fan and network menu and hub rows"

### Task 10: Wire Battery, Fan and Network into the daemon

**Files:** modify `crates/core/src/schedule.rs`, `crates/daemon/src/config.rs`,
`crates/daemon/src/app.rs`, `crates/daemon/src/tray.rs`, `crates/daemon/src/run.rs`,
`crates/daemon/tests/config.rs`, `crates/daemon/tests/app.rs` and
`crates/daemon/tests/tray.rs`.

**Interfaces — Consumes:** everything from Tasks 1–9. **Produces:**
- `schedule::ReaderId { Cpu, Memory, Battery, Fan, Network }`
- `Items::shown(&self, id: ReaderId) -> bool` and `Items::set_shown(&mut self, id: ReaderId, shown: bool)`
- `app::READERS: [ReaderId; 5]`, `app::slot(id: ReaderId) -> usize`, and
  `app::Command { Toggle(ReaderId), ToggleLaunchAtLogin, Quit }`
- `app::NetworkStatus { interface: Option<String>, reading: Reading }` and
  `app::RateLabel = (String, String)`
- `App::record_battery(&mut self, now_nanos: u64, info: &BatteryInfo, adapter_online: Option<bool>) -> Option<(Metric, u8)>`
- `App::record_fan(&mut self, now_nanos: u64, speed: FanSpeed) -> Option<u8>`
- `App::record_network(&mut self, now_nanos: u64, sample: Option<&Sample>) -> Option<RateLabel>`
- `App::battery()`, `App::fan()`, `App::network()`, `App::overview() -> Overview<'_>`, and
  `App::rows(&self, id: ReaderId) -> Vec<String>`
- `tray::rates_icon(upload: f64, download: f64) -> Icon`, and
  `Hub::new(commands: Sender<Command>, items: Items, launch_at_login: bool)` with
  `pub items: Items` in place of `show_cpu` and `show_memory`

**Why one task:** adding the `ReaderId` variants breaks every exhaustive match on it, in
`app.rs`, `tray.rs` and `run.rs`. Those three files only compile together.

**Test level:** unit (`App`, `Hub`, `MetricItem`). `run.rs` is covered by the end-to-end run in
Task 11, as in Plan 02b.

- [x] **Step 1: Write the failing tests.**
  - `crates/daemon/tests/config.rs`: append:

```rust
#[test]
fn each_reader_has_its_own_switch() {
    use statsbard::app::READERS;
    for id in READERS {
        let mut items = Items::default();
        items.set_shown(id, false);
        for other in READERS {
            assert_eq!(items.shown(other), other != id, "{id:?} then {other:?}");
        }
    }
}
```

  - `crates/daemon/tests/app.rs`: replace the imports and `both_shown`, and rename its callers to
    `cpu_and_memory_shown()`. The existing tests keep their assertions: they are about CPU and
    Memory, and the three new items are hidden so that they do not appear in `due`.

```rust
use statsbar_core::battery::{BatteryInfo, ChargeStatus};
use statsbar_core::cpu::CoreTicks;
use statsbar_core::fan::FanSpeed;
use statsbar_core::label::Metric;
use statsbar_core::memory::MemInfo;
use statsbar_core::schedule::ReaderId::{self, Battery, Cpu, Fan, Memory, Network};
use statsbar_core::throughput::Sample;
use statsbard::app::{App, READERS};
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

fn cpu_and_memory_shown() -> App {
    only(&[Cpu, Memory])
}

fn battery(status: ChargeStatus, capacity_percent: u8) -> BatteryInfo {
    BatteryInfo {
        status,
        capacity_percent,
        charge_now_uah: None,
        charge_full_uah: None,
        charge_full_design_uah: None,
        current_now_ua: None,
        voltage_now_uv: None,
        cycle_count: None,
    }
}

fn fan(rpm: u64) -> FanSpeed {
    FanSpeed {
        rpm,
        min_rpm: 0,
        max_rpm: 5300,
    }
}

fn wlo1(rx_bytes: u64, tx_bytes: u64) -> Sample {
    Sample {
        interface: "wlo1".to_owned(),
        rx_bytes,
        tx_bytes,
    }
}

fn texts(upload: &str, download: &str) -> Option<(String, String)> {
    Some((upload.to_owned(), download.to_owned()))
}
```

  In `a_hidden_item_is_never_read`, the config literal becomes
  `Items { cpu: false, memory: true, battery: false, fan: false, network: false }`. Then append:

```rust
#[test]
fn every_item_is_due_at_start_in_hub_order() {
    let app = App::new(Config::default(), false);
    assert_eq!(app.due(0), READERS.to_vec());
    assert_eq!(READERS, [Cpu, Memory, Battery, Fan, Network]);
}

#[test]
fn the_battery_label_says_chg_only_while_charging() {
    let mut app = only(&[Battery]);
    use ChargeStatus::{Charging, Discharging, NotCharging};
    assert_eq!(
        app.record_battery(0, &battery(Discharging, 76), Some(false)),
        Some((Metric::Bat, 76))
    );
    assert_eq!(
        app.record_battery(2 * SECOND, &battery(Discharging, 76), Some(false)),
        None
    );
    assert_eq!(
        app.record_battery(4 * SECOND, &battery(Charging, 76), Some(true)),
        Some((Metric::Chg, 76))
    );
    assert_eq!(
        app.record_battery(6 * SECOND, &battery(NotCharging, 76), Some(true)),
        Some((Metric::Bat, 76))
    );
    assert_eq!(
        app.record_battery(8 * SECOND, &battery(NotCharging, 77), Some(true)),
        Some((Metric::Bat, 77))
    );
    assert_eq!(
        app.battery().map(|figures| figures.adapter_online),
        Some(Some(true))
    );
}

#[test]
fn the_fan_label_is_its_share_of_the_range_clamped_at_100() {
    let mut app = only(&[Fan]);
    assert_eq!(app.record_fan(0, fan(2650)), Some(50));
    assert_eq!(app.record_fan(2 * SECOND, fan(2660)), None);
    assert_eq!(app.record_fan(4 * SECOND, fan(6040)), Some(100));
    assert_eq!(app.fan().map(|speed| speed.rpm), Some(6040));
}

#[test]
fn a_fan_without_a_range_keeps_its_rows_but_draws_no_label() {
    let mut app = only(&[Fan]);
    let no_range = FanSpeed {
        rpm: 3000,
        min_rpm: 0,
        max_rpm: 0,
    };
    assert_eq!(app.record_fan(0, no_range), None);
    assert_eq!(app.rows(Fan), ["Speed 3000 RPM", "Max 0 RPM"]);
    assert!(!app.due(SECOND).contains(&Fan), "the sample still counts as a read");
}

#[test]
fn the_network_label_follows_its_rate_texts() {
    let mut app = only(&[Network]);
    assert_eq!(app.record_network(0, Some(&wlo1(0, 0))), texts("0B", "0B"));
    assert_eq!(
        app.record_network(2 * SECOND, Some(&wlo1(24_000, 2_000))),
        texts("1K", "12K")
    );
    assert_eq!(
        app.record_network(4 * SECOND, Some(&wlo1(48_000, 4_000))),
        None
    );
    assert_eq!(app.rows(Network)[0], "Interface wlo1");
}

#[test]
fn a_rate_change_too_small_for_the_label_still_redraws_the_title() {
    let mut app = only(&[Network]);
    app.record_network(0, Some(&wlo1(0, 0)));
    app.record_network(2 * SECOND, Some(&wlo1(24_000, 2_000)));
    // 12.1 KB/s still draws "12K", but the title reads "↓ 12.1 KB/s" instead of "↓ 12.0 KB/s".
    assert_eq!(
        app.record_network(4 * SECOND, Some(&wlo1(48_200, 4_000))),
        texts("1K", "12K")
    );
    assert_eq!(
        summary::network_title(&app.network().expect("sampled").reading),
        "Network ↓ 12.1 KB/s ↑ 1.0 KB/s"
    );
}

#[test]
fn no_default_route_reads_as_no_interface_and_zero_rates() {
    let mut app = only(&[Network]);
    app.record_network(0, Some(&wlo1(0, 0)));
    app.record_network(2 * SECOND, Some(&wlo1(24_000, 0)));
    assert_eq!(app.record_network(4 * SECOND, None), texts("0B", "0B"));
    assert_eq!(
        app.rows(Network),
        [
            "No active interface",
            "Download 0 bytes/s",
            "Upload 0 bytes/s",
            "Downloaded 24.0 KB",
            "Uploaded 0 bytes",
        ]
    );
}

#[test]
fn showing_the_network_again_starts_a_new_session() {
    let mut app = only(&[Network]);
    app.record_network(0, Some(&wlo1(0, 0)));
    app.record_network(2 * SECOND, Some(&wlo1(24_000, 0)));
    app.toggle(Network);
    assert!(app.toggle(Network));
    assert_eq!(app.network(), None);
    app.record_network(4 * SECOND, Some(&wlo1(30_000, 0)));
    assert_eq!(app.rows(Network)[3], "Downloaded 0 bytes");
}

#[test]
fn hiding_battery_fan_or_network_stops_its_reads_and_updates_the_config() {
    for id in [Battery, Fan, Network] {
        let mut app = App::new(Config::default(), false);
        assert!(!app.toggle(id), "{id:?}");
        assert!(!app.config.items.shown(id), "{id:?}");
        assert!(!app.due(100 * SECOND).contains(&id), "{id:?}");
    }
}

#[test]
fn hiding_the_battery_drops_its_figures_and_label() {
    let mut app = only(&[Battery]);
    app.record_battery(0, &battery(ChargeStatus::Discharging, 76), None);
    app.toggle(Battery);
    assert!(app.toggle(Battery));
    assert_eq!(app.battery(), None);
    assert_eq!(
        app.record_battery(2 * SECOND, &battery(ChargeStatus::Discharging, 76), None),
        Some((Metric::Bat, 76)),
        "shown again, the same value is drawn again"
    );
}

#[test]
fn the_overview_has_a_dash_for_shown_items_without_a_value() {
    let mut app = App::new(Config::default(), false);
    app.toggle(Fan);
    app.record_battery(0, &battery(ChargeStatus::Discharging, 76), None);
    assert_eq!(
        summary::hub_rows(&app.overview()),
        [
            "CPU —",
            "Memory —",
            "Battery 76 % · Discharging",
            "Network —"
        ]
    );
}

#[test]
fn rows_are_empty_before_the_first_value() {
    let app = App::new(Config::default(), false);
    for id in READERS {
        assert!(app.rows(id).is_empty(), "{id:?}");
    }
}
```

  - `crates/daemon/tests/tray.rs`: replace `hub`, the expected list in
    `the_hub_menu_lists_rows_switches_and_quit`, the `[..3]` assertion in
    `a_new_hub_already_has_a_row_for_each_shown_metric`, the table in
    `each_switch_and_quit_send_their_command`, and `metric_item_ids_are_stable_and_rows_are_not_clickable`.
    Add the rates icon test. `ReaderId` is already imported since Task 3; add:

```rust
use statsbard::app::READERS;
use statsbard::config::Items;

fn hub(capacity: usize) -> (Hub, async_channel::Receiver<Command>) {
    let (sender, receiver) = async_channel::bounded(capacity);
    let items = Items {
        cpu: true,
        memory: false,
        battery: true,
        fan: false,
        network: true,
    };
    (Hub::new(sender, items, true), receiver)
}
```

```rust
    // the_hub_menu_lists_rows_switches_and_quit
    assert_eq!(
        labels(&hub.menu()),
        [
            "CPU 24 %",
            "---",
            "[x] Show CPU",
            "[ ] Show Memory",
            "[x] Show Battery",
            "[ ] Show Fan",
            "[x] Show Network",
            "---",
            "[x] Launch at login",
            "---",
            "Quit",
        ]
    );
```

```rust
    // a_new_hub_already_has_a_row_for_each_shown_metric
    assert_eq!(
        labels(&hub.menu())[..5],
        ["CPU —", "Battery —", "Network —", "---", "[x] Show CPU"]
    );
```

```rust
    // each_switch_and_quit_send_their_command
    for (label, expected) in [
        ("Show CPU", Command::Toggle(ReaderId::Cpu)),
        ("Show Memory", Command::Toggle(ReaderId::Memory)),
        ("Show Battery", Command::Toggle(ReaderId::Battery)),
        ("Show Fan", Command::Toggle(ReaderId::Fan)),
        ("Show Network", Command::Toggle(ReaderId::Network)),
        ("Launch at login", Command::ToggleLaunchAtLogin),
        ("Quit", Command::Quit),
    ] {
```

```rust
#[test]
fn metric_item_ids_are_stable_and_rows_are_not_clickable() {
    let ids: Vec<String> = READERS.map(|id| MetricItem::new(id).id()).to_vec();
    assert_eq!(
        ids,
        [
            "statsbar-cpu",
            "statsbar-memory",
            "statsbar-battery",
            "statsbar-fan",
            "statsbar-network",
        ]
    );
    let mut memory = MetricItem::new(ReaderId::Memory);
    memory.rows = vec!["App 6.00 GB".to_owned()];
    match &memory.menu()[0] {
        MenuItem::Standard(item) => {
            assert_eq!(item.label, "App 6.00 GB");
            assert!(!item.enabled);
        }
        other => panic!("expected a row, got {other:?}"),
    }
}

#[test]
fn the_rates_icon_is_the_two_line_label() {
    let icon = tray::rates_icon(12_000.0, 0.0);
    assert_eq!((icon.width, icon.height), (20, 20));
    assert_eq!(icon.data, label::render_rates(12_000.0, 0.0).to_vec());
}
```

  In `a_full_command_queue_drops_the_click_instead_of_blocking_the_tray_thread`, the second click
  `"Show CPU"` stays, and the assertions are unchanged.
- [x] **Step 2: Run and expect FAIL.** `cargo test -p statsbard` fails to compile:
  "no variant named `Battery` in `ReaderId`", "cannot find `READERS` in `app`", and
  "no method named `set_shown`".
- [x] **Step 3: Implement core and config.** In `crates/core/src/schedule.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReaderId {
    Cpu,
    Memory,
    Battery,
    Fan,
    Network,
}
```

  In `crates/daemon/src/config.rs`, add `use statsbar_core::schedule::ReaderId;` and, below
  `impl Default for Items`:

```rust
impl Items {
    pub fn shown(&self, id: ReaderId) -> bool {
        match id {
            ReaderId::Cpu => self.cpu,
            ReaderId::Memory => self.memory,
            ReaderId::Battery => self.battery,
            ReaderId::Fan => self.fan,
            ReaderId::Network => self.network,
        }
    }

    pub fn set_shown(&mut self, id: ReaderId, shown: bool) {
        let flag = match id {
            ReaderId::Cpu => &mut self.cpu,
            ReaderId::Memory => &mut self.memory,
            ReaderId::Battery => &mut self.battery,
            ReaderId::Fan => &mut self.fan,
            ReaderId::Network => &mut self.network,
        };
        *flag = shown;
    }
}
```

- [x] **Step 4: Implement `app.rs`.** Replace the file with:

```rust
//! What the daemon knows and decides, with no I/O: which items are shown, when each reader is
//! due, and whether a new sample changes a label. The event loop does the reading and drawing.

use statsbar_core::battery::{BatteryFigures, BatteryInfo, ChargeStatus};
use statsbar_core::cpu::{CoreTicks, CpuLoad};
use statsbar_core::fan::{self, FanSpeed};
use statsbar_core::label::{self, Metric};
use statsbar_core::memory::{MemInfo, MemoryBuckets};
use statsbar_core::schedule::{self, ReaderId, ReaderState};
use statsbar_core::throughput::{Reading, Sample, ThroughputTracker};

use crate::config::Config;
use crate::summary::{self, Entry, Overview, percent};

/// Every metric, in the order of the hub's rows and switches and of the items' first spawn.
pub const READERS: [ReaderId; 5] = [
    ReaderId::Cpu,
    ReaderId::Memory,
    ReaderId::Battery,
    ReaderId::Fan,
    ReaderId::Network,
];

/// A click in the hub's menu, sent from the `ksni` thread to the main loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Toggle(ReaderId),
    ToggleLaunchAtLogin,
    Quit,
}

/// The network label's texts: upload, then download.
pub type RateLabel = (String, String);

/// The network figures of the last sample.
#[derive(Debug, Clone, PartialEq)]
pub struct NetworkStatus {
    /// `None` when there was no default route.
    pub interface: Option<String>,
    pub reading: Reading,
}

pub struct App {
    pub config: Config,
    pub launch_at_login: bool,
    pub power_saver: bool,
    /// Indexed by `slot`.
    readers: [ReaderState; 5],
    failing: [bool; 5],
    cpu_previous: Option<Vec<CoreTicks>>,
    cpu: Option<CpuLoad>,
    cpu_label: Option<u8>,
    memory: Option<MemoryBuckets>,
    memory_label: Option<u8>,
    battery: Option<BatteryFigures>,
    battery_label: Option<(Metric, u8)>,
    fan: Option<FanSpeed>,
    fan_label: Option<u8>,
    throughput: ThroughputTracker,
    network: Option<NetworkStatus>,
    /// The label's texts and the item's title, as last drawn.
    network_label: Option<(RateLabel, String)>,
}

/// Where `id` sits in `READERS`, and in every per-reader array.
pub fn slot(id: ReaderId) -> usize {
    match id {
        ReaderId::Cpu => 0,
        ReaderId::Memory => 1,
        ReaderId::Battery => 2,
        ReaderId::Fan => 3,
        ReaderId::Network => 4,
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
fn changed<T: Clone + PartialEq>(shown: &mut Option<T>, value: T) -> Option<T> {
    if shown.as_ref() == Some(&value) {
        None
    } else {
        *shown = Some(value.clone());
        Some(value)
    }
}

impl App {
    pub fn new(config: Config, launch_at_login: bool) -> App {
        App {
            readers: READERS.map(|id| fresh(id, config.items.shown(id))),
            config,
            launch_at_login,
            power_saver: false,
            failing: [false; 5],
            cpu_previous: None,
            cpu: None,
            cpu_label: None,
            memory: None,
            memory_label: None,
            battery: None,
            battery_label: None,
            fan: None,
            fan_label: None,
            throughput: ThroughputTracker::default(),
            network: None,
            network_label: None,
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
        self.config.items.set_shown(id, visible);
        self.forget(id);
        self.readers[slot(id)] = fresh(id, visible);
        self.failing[slot(id)] = false;
        visible
    }

    fn forget(&mut self, id: ReaderId) {
        match id {
            ReaderId::Cpu => {
                self.cpu_previous = None;
                self.cpu = None;
                self.cpu_label = None;
            }
            ReaderId::Memory => {
                self.memory = None;
                self.memory_label = None;
            }
            ReaderId::Battery => {
                self.battery = None;
                self.battery_label = None;
            }
            ReaderId::Fan => {
                self.fan = None;
                self.fan_label = None;
            }
            ReaderId::Network => {
                // A new session: the totals count from when the item is shown again.
                self.throughput = ThroughputTracker::default();
                self.network = None;
                self.network_label = None;
            }
        }
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

    /// Takes a battery sample. Returns the new label when its word or its percent changed: `CHG`
    /// while charging and `BAT` otherwise (requirement 2).
    pub fn record_battery(
        &mut self,
        now_nanos: u64,
        info: &BatteryInfo,
        adapter_online: Option<bool>,
    ) -> Option<(Metric, u8)> {
        self.mark_read(ReaderId::Battery, now_nanos);
        let figures = BatteryFigures::new(info, adapter_online);
        let word = if figures.status == ChargeStatus::Charging {
            Metric::Chg
        } else {
            Metric::Bat
        };
        self.battery = Some(figures);
        changed(&mut self.battery_label, (word, figures.percent))
    }

    /// Takes a fan sample. Returns the new label value, its share of the fan's range clamped at
    /// 100 (requirement 3), when it changed. A fan whose range is empty has rows but no label.
    pub fn record_fan(&mut self, now_nanos: u64, speed: FanSpeed) -> Option<u8> {
        self.mark_read(ReaderId::Fan, now_nanos);
        self.fan = Some(speed);
        let value = percent(fan::range_fraction(&speed)? * 100.0);
        changed(&mut self.fan_label, value)
    }

    /// Takes a network sample, `None` when there is no default route. Returns the label's texts
    /// when they or the item's title changed. The title has finer rates than the label, and it
    /// must stay as current as the hub row that repeats it.
    pub fn record_network(&mut self, now_nanos: u64, sample: Option<&Sample>) -> Option<RateLabel> {
        self.mark_read(ReaderId::Network, now_nanos);
        let reading = self.throughput.update(sample, now_nanos);
        self.network = Some(NetworkStatus {
            interface: sample.map(|sample| sample.interface.clone()),
            reading,
        });
        let texts = (
            label::rate_text(reading.upload_rate),
            label::rate_text(reading.download_rate),
        );
        changed(
            &mut self.network_label,
            (texts, summary::network_title(&reading)),
        )
        .map(|(texts, _)| texts)
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

    pub fn battery(&self) -> Option<&BatteryFigures> {
        self.battery.as_ref()
    }

    pub fn fan(&self) -> Option<&FanSpeed> {
        self.fan.as_ref()
    }

    pub fn network(&self) -> Option<&NetworkStatus> {
        self.network.as_ref()
    }

    /// What the hub shows: each metric hidden, waiting for its first value, or with it.
    pub fn overview(&self) -> Overview<'_> {
        let items = &self.config.items;
        Overview {
            cpu: Entry::of(items.cpu, self.cpu()),
            memory: Entry::of(items.memory, self.memory()),
            battery: Entry::of(items.battery, self.battery()),
            fan: Entry::of(items.fan, self.fan()),
            network: Entry::of(items.network, self.network().map(|status| &status.reading)),
        }
    }

    /// A metric item's menu rows, empty before its first value.
    pub fn rows(&self, id: ReaderId) -> Vec<String> {
        match id {
            ReaderId::Cpu => self.cpu().map(summary::cpu_rows),
            ReaderId::Memory => self.memory().map(summary::memory_rows),
            ReaderId::Battery => self.battery().map(summary::battery_rows),
            ReaderId::Fan => self.fan().map(summary::fan_rows),
            ReaderId::Network => self
                .network()
                .map(|status| summary::network_rows(status.interface.as_deref(), &status.reading)),
        }
        .unwrap_or_default()
    }

    fn mark_read(&mut self, id: ReaderId, now_nanos: u64) {
        self.readers[slot(id)].last_read_nanos = Some(now_nanos);
        self.failing[slot(id)] = false;
    }
}
```

- [x] **Step 5: Implement `tray.rs`.** Change the imports to:

```rust
use async_channel::Sender;
use ksni::menu::{CheckmarkItem, StandardItem};
use ksni::{Category, Icon, MenuItem};
use statsbar_core::label::{self, Metric};
use statsbar_core::schedule::ReaderId;

use crate::app::{Command, READERS};
use crate::config::Items;
use crate::summary::{self, Overview};
```

  Below `icon`, add:

```rust
/// The network label: upload over download.
pub fn rates_icon(upload: f64, download: f64) -> Icon {
    // SIZE is 20, so the cast cannot truncate.
    let side = label::SIZE as i32;
    Icon {
        width: side,
        height: side,
        data: label::render_rates(upload, download).to_vec(),
    }
}
```

  Extend `name` and `MetricItem::id` with the three new arms: `"Battery"`, `"Fan"`, `"Network"`,
  and `"statsbar-battery"`, `"statsbar-fan"`, `"statsbar-network"`. Replace `Hub`, `Hub::new` and
  the switch part of `Hub::menu`:

```rust
pub struct Hub {
    pub rows: Vec<String>,
    pub items: Items,
    pub launch_at_login: bool,
    commands: Sender<Command>,
}

impl Hub {
    pub fn new(commands: Sender<Command>, items: Items, launch_at_login: bool) -> Hub {
        Hub {
            // Placeholders now, so the first values only relabel rows. Adding rows later would
            // change the layout, which makes ksni renumber every menu id and lose a click sent
            // with an old one.
            rows: summary::hub_rows(&Overview::waiting(&items)),
            items,
            launch_at_login,
            commands,
        }
    }
```

  (the `send` method is unchanged), and in `menu`:

```rust
        for id in READERS {
            menu.push(switch(
                &format!("Show {}", name(id)),
                self.items.shown(id),
                Command::Toggle(id),
            ));
        }
```

  in place of the two `Show CPU` and `Show Memory` pushes. Remove the `Entry` import that Task 9
  added; `Overview::waiting` replaces it.
- [x] **Step 6: Implement `run.rs`.** Replace the file with:

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
use ksni::{Handle, Icon, TrayMethods};
use statsbar_core::label::Metric;
use statsbar_core::schedule::ReaderId;
use statsbar_core::throughput::Reading;
use statsbar_readers::battery::BatteryReader;
use statsbar_readers::cpu::CpuReader;
use statsbar_readers::fan::FanReader;
use statsbar_readers::memory::MemoryReader;
use statsbar_readers::network::NetworkReader;

use crate::app::{App, Command, READERS, slot};
use crate::config::Items;
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

/// A label to draw: the pixmap and the item's title.
type Look = (Option<Icon>, String);

fn percent_look(id: ReaderId, metric: Metric, value: u8) -> Look {
    (
        tray::icon(metric, value),
        summary::title(tray::name(id), Some(value)),
    )
}

fn rates_look(reading: &Reading) -> Look {
    (
        Some(tray::rates_icon(reading.upload_rate, reading.download_rate)),
        summary::network_title(reading),
    )
}

/// Reuses the open reader, or opens one, then reads it. On an error the reader is gone, so the
/// next attempt reopens its files.
fn read_with<R, T>(
    slot: &mut Option<R>,
    open: impl FnOnce() -> io::Result<R>,
    read: impl FnOnce(&mut R) -> io::Result<T>,
) -> io::Result<T> {
    let reader = match slot.take() {
        Some(reader) => reader,
        None => open()?,
    };
    read(slot.insert(reader))
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

/// The open readers. A hidden metric's reader is dropped, which closes its files.
#[derive(Default)]
struct Readers {
    cpu: Option<CpuReader>,
    memory: Option<MemoryReader>,
    battery: Option<BatteryReader>,
    fan: Option<FanReader>,
    network: Option<NetworkReader>,
}

impl Readers {
    fn close(&mut self, id: ReaderId) {
        match id {
            ReaderId::Cpu => self.cpu = None,
            ReaderId::Memory => self.memory = None,
            ReaderId::Battery => self.battery = None,
            ReaderId::Fan => self.fan = None,
            ReaderId::Network => self.network = None,
        }
    }
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
    hub_shown: Option<(Vec<String>, Items, bool)>,
    /// Indexed by `app::slot`.
    items: [Option<Handle<MetricItem>>; 5],
    readers: Readers,
}

impl Daemon {
    async fn start(app: App, commands: Sender<Command>, files: Files) -> Result<Daemon, BoxError> {
        let hub = Hub::new(commands, app.config.items.clone(), app.launch_at_login);
        let hub = bounded("hub item spawn", SETUP_TIMEOUT, hub.spawn()).await??;
        let mut daemon = Daemon {
            app,
            files,
            hub,
            hub_shown: None,
            items: Default::default(),
            readers: Readers::default(),
        };
        for id in READERS {
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
                    Some(change) => {
                        match bounded("ActiveProfile read", CALL_TIMEOUT, change.get()).await {
                            Ok(Ok(value)) => Wake::Profile(Ok(value)),
                            Ok(Err(error)) => Wake::Profile(Err(error.into())),
                            Err(error) => Wake::Profile(Err(error)),
                        }
                    }
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

    /// Reads one metric, records it, and redraws its item.
    async fn sample(&mut self, id: ReaderId, now: u64) {
        let root = &self.files.root;
        let readers = &mut self.readers;
        let app = &mut self.app;
        let result = match id {
            ReaderId::Cpu => {
                read_with(&mut readers.cpu, || CpuReader::open(root), CpuReader::read).map(
                    |ticks| {
                        app.record_cpu(now, ticks)
                            .map(|value| percent_look(id, Metric::Cpu, value))
                    },
                )
            }
            ReaderId::Memory => read_with(
                &mut readers.memory,
                || MemoryReader::open(root),
                MemoryReader::read,
            )
            .map(|info| {
                app.record_memory(now, &info)
                    .map(|value| percent_look(id, Metric::Ram, value))
            }),
            ReaderId::Battery => read_with(
                &mut readers.battery,
                || BatteryReader::open(root),
                BatteryReader::read,
            )
            .map(|reading| {
                app.record_battery(now, &reading.info, reading.adapter_online)
                    .map(|(metric, value)| percent_look(id, metric, value))
            }),
            ReaderId::Fan => {
                read_with(&mut readers.fan, || FanReader::open(root), FanReader::read).map(
                    |speed| {
                        app.record_fan(now, speed)
                            .map(|value| percent_look(id, Metric::Fan, value))
                    },
                )
            }
            ReaderId::Network => read_with(
                &mut readers.network,
                || NetworkReader::open(root),
                NetworkReader::read,
            )
            .map(|sample| {
                app.record_network(now, sample.as_ref())?;
                app.network().map(|status| rates_look(&status.reading))
            }),
        };
        match result {
            Ok(redraw) => self.refresh_item(id, redraw).await,
            Err(error) => self.read_failed(id, now, &error),
        }
    }

    /// Closes the reader, so the next attempt reopens its files, and logs the first failure in a
    /// row.
    fn read_failed(&mut self, id: ReaderId, now: u64, error: &io::Error) {
        self.readers.close(id);
        if self.app.record_error(id, now) {
            eprintln!("statsbard: reading {id:?} failed: {error}");
        }
    }

    async fn refresh_item(&self, id: ReaderId, redraw: Option<Look>) {
        let Some(handle) = &self.items[slot(id)] else {
            return;
        };
        let rows = self.app.rows(id);
        // A missed update is not retried. The rows are resent on the next sample, but the icon and
        // title only when the label changes again, because `App` has already recorded the new
        // value. A 1 s stall on `ksni`'s lock is itself worth investigating, and the log shows it.
        update(
            handle,
            "metric item update",
            move |item: &mut MetricItem| {
                item.rows = rows;
                if let Some((icon, title)) = redraw {
                    item.icon = icon;
                    item.title = title;
                }
            },
        )
        .await;
    }

    async fn refresh_hub(&mut self) {
        let shown = (
            summary::hub_rows(&self.app.overview()),
            self.app.config.items.clone(),
            self.app.launch_at_login,
        );
        if self.hub_shown.as_ref() == Some(&shown) {
            return;
        }
        let (rows, items, launch_at_login) = shown.clone();
        let delivered = update(&self.hub, "hub update", move |hub: &mut Hub| {
            hub.rows = rows;
            hub.items = items;
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
            Command::Toggle(id) => self.toggle(id).await,
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
        match bounded(
            "metric item spawn",
            SETUP_TIMEOUT,
            MetricItem::new(id).spawn(),
        )
        .await
        {
            Ok(Ok(handle)) => self.items[slot(id)] = Some(handle),
            Ok(Err(error)) => eprintln!("statsbard: showing {id:?} failed: {error}"),
            Err(error) => eprintln!("statsbard: {error}"),
        }
    }

    /// Removes the item and closes its files, so a hidden metric costs nothing.
    async fn hide_item(&mut self, id: ReaderId) {
        self.readers.close(id);
        if let Some(handle) = self.items[slot(id)].take() {
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
        for item in &mut self.items {
            if let Some(handle) = item.take() {
                shut_down(&handle, "metric item shutdown").await;
            }
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

  Compared with the file left by Task 9, the only changes are: five readers and items held in
  `Readers` and `items`; `sample` dispatching all five through `read_with`; `refresh_item` taking
  rows from `App::rows`; the hub tracking `Items`; and `Command::Toggle`. `bounded`,
  `connect_system_bus`, the `run` loop, `update` and `shut_down` are unchanged. Diff against the
  Task 9 version to confirm that.
- [x] **Step 7: Run and expect PASS.** `cargo test -p statsbard` passes, and `cargo fmt --all` then
  `./scripts/check.sh` is green.

Ready to commit: `crates/core/src/schedule.rs crates/daemon/src/config.rs crates/daemon/src/app.rs crates/daemon/src/tray.rs crates/daemon/src/run.rs crates/daemon/tests/config.rs crates/daemon/tests/app.rs crates/daemon/tests/tray.rs` — "feat(daemon): show Battery, Fan and Network items with hub switches"

### Task 11: Verification and docs

**Files:** `README.md`, `docs/plans/2026-11-16-rust-statsbar-roadmap.md`, a new
`docs/spikes/2026-11-19-03-battery-fan-network-e2e.md`, and this plan's checkboxes.

RED: n/a — no executable behaviour, checked by the end-to-end run and a stated diff review.

- [x] **Step 1: Full suite.** Run `./scripts/check.sh` and expect it green. Quote the test totals.
  Then run `cargo build --release -p statsbard` and `cargo audit`. Quote both results. If
  `cargo audit` is not installed, do not install it without asking: report it as not run.
- [x] **Step 2: End-to-end run.** *(2.2 not met: the user found every label hard to read at 125 % scale; recorded under Risks in the roadmap, with [spike S3](../spikes/2026-11-21-s3-panel-legibility.md). 2.6 not met as planned (6.2 %); met with tick-aligned sampling (+2.0 %), see the e2e doc)* Drive clicks and settings over D-Bus, as in
  [the 02b run](../spikes/2026-11-18-02b-daemon-e2e.md). Ask the user only to look at the panel
  and to pull or replace the power cord. Use a scratch config home. Record every result, with
  commands and raw numbers, in `docs/spikes/2026-11-19-03-battery-fan-network-e2e.md`.
  1. `export XDG_CONFIG_HOME="$(mktemp -d)"`, then
     `strace -f -tt -T -y -s 64 -e trace=pread64,openat,close -o "$XDG_CONFIG_HOME/trace.log" target/release/statsbard &`.
     Note the daemon PID with `pgrep -n statsbard`. `-T` times each call, and `-s 64` shows
     what each read returned.
  2. **Panel:** the user confirms six icons: CPU, RAM, `BAT nn%`, `FAN nn%`, the two-line
     network label, and the hub. The user also says whether each label is legible, and in which
     order GNOME shows the items. If a label is not legible, stop and report it under Risks
     (fallback: a 3×7 digit font, or download only).
  3. **Battery:** read the battery item's menu with `busctl --user call <item bus name>
     /MenuBar com.canonical.dbusmenu GetLayout iias 0 -1 0`. Find the bus name as in the 02b
     run. Compare the menu against `cat /sys/class/power_supply/BAT0/uevent` and
     `upower -i /org/freedesktop/UPower/devices/battery_BAT0`: Charge equals `CAPACITY`, Status
     matches, and Health is `CHARGE_FULL / CHARGE_FULL_DESIGN`. Expect no `Cycles` row while
     `CYCLE_COUNT` is 0.
  4. **Unplug, user present:** the user pulls the power cord. Within 2 s, expect
     `Status Discharging`, `Adapter not connected`, and a `Time left` row once the current
     settles. The user plugs it back in. Expect the label to show `CHG` if the charge is below
     the start threshold (75 %, from `charge_control_start_threshold`), and `BAT` otherwise.
     Record which. If `CHG` could not be observed, report that it is covered only by
     `the_battery_label_says_chg_only_while_charging`.
  5. **Fan:** the Speed row is within one sample of
     `cat /sys/class/hwmon/hwmon*/fan1_input` for the `dell_smm` device. Expect the label at
     `min(100, round(rpm / fan1_max × 100))`. In `trace.log`, expect `pread64` on `fan1_input`
     every 2 s, and none on `fan1_min` or `fan1_max` after open. Quote the `-T` time of five of
     those reads.
  6. **Network rate (the roadmap's evidence):** with the user's consent, run a steady download
     over `wlo1`, for example a large file from the distribution mirror the user names. While it
     runs, read `rx_bytes` with `ip -s -j link show wlo1` at t = 0, read the network item's
     `Title` property every 2 s for 10 s, and read `rx_bytes` again at t = 10 s. Each title must
     match the item's `Download` menu row read with `GetLayout` at the same moment. Expect the mean
     of the five download rates in the titles to be within 5 % of `(rx₁₀ − rx₀) / 10`. Without
     consent, use ambient traffic over 60 s, and report that the 5 % check ran on low traffic.
  7. **Hide each new item:** send a dbusmenu `Event` for *Show Battery*, *Show Fan* and
     *Show Network*. For each, expect the item to vanish and a `close` of its files in
     `trace.log`, with 0 `pread64` on them over the next 10 s. Expect
     `cat $XDG_CONFIG_HOME/statsbar/config.toml` to show `battery = false`, `fan = false` and
     `network = false`. Show them again and expect each item back within 2 s, with the network
     totals restarted.
  8. **Power saver:** `powerprofilesctl set power-saver`. Expect `pread64` on the battery
     `uevent` 5 s apart. Restore the profile that `powerprofilesctl get` showed before, and say
     so in the report.
  9. **Threads and memory:** after 30 s, run `spikes/tray/measure.sh $(pgrep -n statsbard)`.
     Expect the same four threads as in Plan 02b (main and `ksni` named after the process,
     `async-io`, at most `blocking-N`). Record Rss and Pss against the 2657 kB Pss of the 02b
     run.
  10. **Quit:** a dbusmenu `Event` on *Quit*. Expect every item to vanish and exit code 0.
- [x] **Step 3: Docs.**
  - README: change `Status:` to
    `Status: CPU, Memory, Battery, Fan and Network in the tray (statsbard); run with \`cargo run --release -p statsbard\`.`
    Add Plans bullets for this plan and for its end-to-end results.
  - Roadmap, Plan 03: add `*(written: \`docs/plans/2026-11-19-rust-statsbar-03-battery-fan-network.md\`)*`
    to the heading. Rewrite Scope to name the three items and their labels (requirements 1–4).
    Replace the core additions with the forms built here: `battery::BatteryFigures::new`,
    `fan::range_fraction(&FanSpeed)`, `network::default_route_interface`, `label::rate_text` and
    `label::render_rates`. Fill Evidence from Step 2. Note under Plan 05 that it replaces
    `/proc/net/route` with NetworkManager, which closes the IPv6-only gap.
  - Tick this plan's boxes.
- [x] **Step 4: Final gate** (global rules):
  1. Read `git diff main...HEAD`, with untracked files included. Expect nothing unrelated, no
     debug output, no TODOs, no commented-out code and no secrets. Check that no fixture holds a
     real serial number or MAC address.
  2. Rerun `./scripts/check.sh`.
  3. Self-review with one concrete reason per item: requirements 1–9; error paths (every reader
     error reaches `read_failed`, and none is dropped); concurrency (the main loop changes `Hub`
     and `MetricItem` fields only inside `Handle::update`); no hot-path allocation beyond the
     menu rows, the route line split and the rate texts; timeouts unchanged; naming.
  4. Run `code-review-strict` on the branch, then fix or report each finding.
  5. If anything changed, repeat from 1.
- [x] **Step 5: Report the gaps.**
  - `run.rs` is covered only by the end-to-end run.
  - An IPv6-only default route reads as `No active interface` until Plan 05.
  - Battery reads (measured in the end-to-end run at a 13–15 ms mean and a 119 ms peak) and fan
    reads (about 1 ms) run on the main loop.
  - Report whether `CHG` was seen in the panel or only in tests.
  - `page_open` is still always false until Plan 04.

Ready to commit: `docs/spikes/2026-11-21-s3-panel-legibility.md spikes/tray/src/bin/sharp.rs spikes/tray/text_label_probe.py spikes/tray/Cargo.toml spikes/tray/Cargo.lock` — "docs: record spike S3 on tray label legibility", then `README.md docs/plans/2026-11-16-rust-statsbar-roadmap.md docs/plans/2026-11-19-rust-statsbar-03-battery-fan-network.md docs/spikes/2026-11-19-03-battery-fan-network-e2e.md` — "docs: record the battery, fan and network end-to-end run"
