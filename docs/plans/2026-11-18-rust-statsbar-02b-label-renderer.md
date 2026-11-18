# Plan 02b — CPU and RAM tray label renderer

> **For agentic workers:** REQUIRED SUB-SKILL: executing-plans-strict. Steps use `- [ ]`.

**Goal:** render CPU and RAM as the validated two-line Mini layout, and check representative values
on GNOME. The rest of the daemon still has a pending thread invariant decision.

**Architecture:** add a pure label module in `statsbar-core`. Its fixed-size byte array is passed to
the existing standalone tray harness for the live check. The harness runs outside the workspace.

**Tech stack:** Rust edition 2024 at the workspace's Rust 1.85 API floor, no new dependencies.
For live verification reuse the installed GNOME watcher, Desktop Screenshot portal and the
existing [02b probe runner](2026-11-18-rust-statsbar-02b-font-probe.md).

**Branch:** `feature/tray-spikes`, continuing the 02b probe on the current checkout. Preserve the
already staged 02a changes.

## Confirmed Requirements

- Follow the [validated Mini arrangement](../spikes/2026-11-18-02b-system-font.md). Put a small
  CPU or RAM label above the larger value. Use a fixed 20×20 pixmap and right-aligned lines.
- Render whole percentages from 0 through 100. Exercise the RAM label separately in GNOME.
- Keep the renderer pure and the existing crate interfaces compatible.

## Global Constraints

- Run test-first. Tests assert outputs, with no I/O or clock in the renderer.
- Code, comments, names and docs in English. Reuse the existing GNOME runner with bounded
  session-bus calls, watcher cleanup and an appearance restoration trap.
- Never weaken, skip or delete a test. Run `./scripts/check.sh`, release build and independent
  review after the final edit. Commit and push only when asked.

## Technical Defaults

- The 20×20 image has opaque white ink and transparent background. The same foreground read
  against the observed dark GNOME panel in both appearances.
- Preserve the CPU prototype's pixels. Use equally sized letters for RAM and extend the small
  percentage bitmap font to all ten digits.
- Reject a percentage above 100 at the renderer boundary. The calling daemon will own the
  sensor-to-integer conversion.

## Risks & Rollback

| Risk | Mitigation | Rollback |
|---|---|---|
| Alignment or edge values drift | Compare 100% against the executed CPU frame and check the suffix across all 101 values | Revert the new renderer and tests |
| RAM renders differently after GNOME scales it | Capture CPU and RAM separately in both styles | Revert the new renderer and tests |
| A live probe leaves tray items or Light appearance behind | Existing runner terminates and reaps the process, verifies removal, and restores appearance | Return to the prior style and stop the reported probe PID |

## Task 1 — Pure renderer

**Files:** add `crates/core/tests/label.rs` and `crates/core/src/label.rs`. Export the module in
`crates/core/src/lib.rs`.

**Consumes:** a metric and an integer percentage.
**Produces:** `label::SIZE: usize`, `label::Metric::{Cpu, Ram}` and
`label::render(metric: Metric, percent: u8) -> Option<[u8; 1600]>`.

- [x] Save the tests below as `crates/core/tests/label.rs`. Run
  `cargo test --offline --locked -p statsbar-core --test label`. Expect an unresolved
  `statsbar_core::label` import.
- [x] Add the renderer below as `crates/core/src/label.rs` and add `pub mod label;` to
  `crates/core/src/lib.rs`. Run the same test command and expect all five tests to pass.
- [x] Ready to commit `crates/core` label changes with `feat(core): render compact CPU and RAM labels`.

## Task 2 — Live and final verification

**Files:** this plan for the recorded run, a temporary frame exporter and captures outside Git.
**Consumes:** the release core library and the existing `font_probe --square` runner.
**Produces:** CPU and RAM captures and evidence that the current implementation renders correctly.

- [x] Run `cargo build --workspace --release --locked --offline`. Save the exporter below as
  `export-frames.rs` in a scratch directory. Compile with
  `rustc --edition=2024 -C panic=abort --extern statsbar_core=target/release/libstatsbar_core.rlib SCRATCH/export-frames.rs -o SCRATCH/export-frames`
  from the repository root, substituting the scratch directory path. Run it with that directory as
  the output path. Expect three 1600-byte frames per metric.
- [x] Use the square observation helper from the 02b probe plan with each metric's frames in
  turn. Set `observation_dir` to the directory containing its `capture.py` and `observe.sh`, and
  `scratch` to the export directory. Run from the repo root in the GNOME session:

  ```bash
  for metric in cpu ram; do
      cp "$observation_dir/capture.py" "$scratch/$metric/"
      timeout --kill-after=5s 90s bash "$observation_dir/observe.sh" "$scratch/$metric" \
          "$PWD/spikes/tray/target/release/font_probe" "$PWD/spikes/tray/measure.sh" || exit
  done
  ```

  Capture 7%, 37% and 100% in both styles. Verify cleanup and record the observed label,
  text height and right edge for both metrics.
- [x] Run `./scripts/check.sh`, the workspace release build, `git diff --check`, and review
  tracked and untracked changes with `code-review-strict`. Check the existing staging fingerprint.
  Record verification and open gaps in this plan. Ready to commit the plan when requested.

### Verification record

On 2026-11-18, the focused suite first failed on the missing module, then passed all five tests.
The three exported CPU frames matched the earlier Mini probe byte-for-byte. The release renderer
also produced three 1600-byte RAM frames.

At 125% display scale, GNOME showed 7%, 37%, and 100% from the release renderer for both metrics
in Dark and Light. The label occupied 7 visible pixels vertically and the percentage occupied
11. The percentage's right edge stayed fixed. The CPU Dark 100% capture had GNOME's hover
highlight, so its pixel bounds used a threshold above the highlighted background. The other five
Dark/Light pairs matched pixel-for-pixel in the tray crop.

The CPU and RAM probes used four threads each. Pss at tick 15 was 2004 kB for CPU and 1992 kB
for RAM. These were separate single-item runs with prepared frames. The GNOME watcher removed
both items, and Dark was restored. Captures are outside Git.

`./scripts/check.sh` passed formatting, Clippy, 63 workspace tests and the core purity gate.
The locked offline workspace release build passed on Rust 1.100.0. The Git index fingerprint
matched before and after.

The live run used fixture percentages at one scale. The integration tests covered the complete
0–100% range, and the daemon still needs integration with real readings and a production memory
measurement. The thread invariant revision remains deferred.

### Tests

```rust
use std::collections::HashSet;

use statsbar_core::label::{Metric, SIZE, render};

fn pixel(pixmap: &[u8], x: usize, y: usize) -> &[u8] {
    let start = (y * SIZE + x) * 4;
    &pixmap[start..start + 4]
}

#[test]
fn cpu_at_100_percent_matches_the_gnome_probe() {
    let expected = [
        "....................",
        "....................",
        ".........##.##..#.#.",
        "........#...#.#.#.#.",
        "........#...##..#.#.",
        "........#...#...#.#.",
        ".........##.#....#..",
        "....................",
        "....................",
        "....................",
        "...#...#...#..##..#.",
        "..##..#.#.#.#.##..#.",
        "...#..#.#.#.#....#..",
        "...#..#.#.#.#....#..",
        "...#..#.#.#.#...#...",
        "...#..#.#.#.#..#....",
        "...#..#.#.#.#..#.##.",
        "...#..#.#.#.#.#..##.",
        "..###..#...#..#.....",
        "....................",
    ];
    let pixmap = render(Metric::Cpu, 100).unwrap();
    assert_eq!(SIZE, 20);
    for (y, line) in expected.iter().enumerate() {
        for (x, cell) in line.bytes().enumerate() {
            let channel = if cell == b'#' { 255 } else { 0 };
            assert_eq!(pixel(&pixmap, x, y), [channel; 4], "pixel ({x}, {y})");
        }
    }
}

#[test]
fn ram_uses_its_own_label_and_the_same_percentage() {
    let ram = render(Metric::Ram, 37).unwrap();
    let cpu = render(Metric::Cpu, 37).unwrap();
    let label = [
        "##...#..#.#",
        "#.#.#.#.###",
        "##..###.###",
        "#.#.#.#.#.#",
        "#.#.#.#.#.#",
    ];
    for (row, line) in label.iter().enumerate() {
        for (col, cell) in line.bytes().enumerate() {
            assert_eq!(
                pixel(&ram, 8 + col, 2 + row)[0],
                if cell == b'#' { 255 } else { 0 }
            );
        }
    }
    let value_start = 10 * SIZE * 4;
    assert_ne!(ram[..value_start], cpu[..value_start]);
    assert_eq!(ram[value_start..], cpu[value_start..]);
}

#[test]
fn all_percentages_are_distinct_and_have_a_fixed_suffix() {
    let mut values = HashSet::new();
    let reference = render(Metric::Cpu, 0).unwrap();
    for percent in 0..=100 {
        let pixmap = render(Metric::Cpu, percent).unwrap();
        assert!(values.insert(pixmap), "duplicate pixmap at {percent}%");
        assert_eq!(pixmap[..10 * SIZE * 4], reference[..10 * SIZE * 4]);
        for y in 10..SIZE {
            for x in 14..SIZE {
                assert_eq!(pixel(&pixmap, x, y), pixel(&reference, x, y));
            }
        }
        let first_ink = (0..14)
            .find(|&x| (10..SIZE).any(|y| pixel(&pixmap, x, y)[0] != 0))
            .unwrap();
        let expected_columns = match percent {
            0..=9 => 10..=11,
            10..=99 => 6..=7,
            _ => 2..=3,
        };
        assert!(
            expected_columns.contains(&first_ink),
            "value starts at {first_ink} for {percent}%"
        );
    }
}

#[test]
fn single_digit_values_draw_the_expected_numeral() {
    let expected = [
        [
            ".#.", "#.#", "#.#", "#.#", "#.#", "#.#", "#.#", "#.#", ".#.",
        ],
        [
            ".#.", "##.", ".#.", ".#.", ".#.", ".#.", ".#.", ".#.", "###",
        ],
        [
            "##.", "..#", "..#", "..#", ".#.", "#..", "#..", "#..", "###",
        ],
        [
            "##.", "..#", "..#", "..#", ".##", "..#", "..#", "..#", "##.",
        ],
        [
            "#.#", "#.#", "#.#", "#.#", "###", "..#", "..#", "..#", "..#",
        ],
        [
            "###", "#..", "#..", "#..", "##.", "..#", "..#", "..#", "##.",
        ],
        [
            ".##", "#..", "#..", "#..", "##.", "#.#", "#.#", "#.#", ".#.",
        ],
        [
            "###", "..#", "..#", "..#", ".#.", ".#.", "#..", "#..", "#..",
        ],
        [
            ".#.", "#.#", "#.#", "#.#", ".#.", "#.#", "#.#", "#.#", ".#.",
        ],
        [
            ".#.", "#.#", "#.#", "#.#", ".##", "..#", "..#", "..#", "##.",
        ],
    ];
    for (percent, rows) in (0_u8..=9).zip(expected) {
        let pixmap = render(Metric::Cpu, percent).unwrap();
        for (row, line) in rows.iter().enumerate() {
            for (col, cell) in line.bytes().enumerate() {
                let expected_alpha = if cell == b'#' { 255 } else { 0 };
                assert_eq!(
                    pixel(&pixmap, 10 + col, 10 + row)[0],
                    expected_alpha,
                    "{percent}%"
                );
            }
        }
    }
}

#[test]
fn out_of_range_values_are_rejected() {
    for value in 101..=u8::MAX {
        assert!(render(Metric::Cpu, value).is_none());
        assert!(render(Metric::Ram, value).is_none());
    }
}
```

### Renderer

```rust
//! Draws CPU and RAM labels into fixed-size, network-byte-order ARGB pixmaps.

/// Width and height of the pixmap in pixels.
pub const SIZE: usize = 20;

const BYTES_PER_PIXEL: usize = 4;
const PIXMAP_BYTES: usize = SIZE * SIZE * BYTES_PER_PIXEL;
const LINE_END: usize = SIZE - 1;
const LABEL_TOP: usize = 2;
const VALUE_TOP: usize = 10;
const DIGIT_WIDTH: usize = 3;
const PERCENT_WIDTH: usize = 5;
const LABEL_WIDTH: usize = 11;
const GAP: usize = 1;

/// The short module label shown above the percentage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    Cpu,
    Ram,
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

const DIGITS: [[u8; 9]; 10] = [
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

/// Draws a right-aligned metric label and whole percentage in white on transparent pixels.
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
    let mut ink = |x, y| {
        let offset = (y * SIZE + x) * BYTES_PER_PIXEL;
        pixmap[offset..offset + BYTES_PER_PIXEL].fill(255);
    };

    let label = match metric {
        Metric::Cpu => CPU,
        Metric::Ram => RAM,
    };
    for (row, line) in label.iter().enumerate() {
        for (col, &cell) in line.iter().enumerate() {
            if cell == b'#' {
                ink(LINE_END - LABEL_WIDTH + col, LABEL_TOP + row);
            }
        }
    }

    // Anchor the percent sign first, then work left so digit count never shifts the suffix.
    let mut left = LINE_END - PERCENT_WIDTH;
    let mut draw = |left, width, rows: &[u8]| {
        for (row, &mask) in rows.iter().enumerate() {
            for col in 0..width {
                if mask & (1 << (width - 1 - col)) != 0 {
                    ink(left + col, VALUE_TOP + row);
                }
            }
        }
    };
    draw(left, PERCENT_WIDTH, &PERCENT);
    for &digit in digits.iter().rev() {
        left -= DIGIT_WIDTH + GAP;
        draw(left, DIGIT_WIDTH, &DIGITS[digit]);
    }

    Some(pixmap)
}
```

### Live frame exporter

```rust
use std::{error::Error, path::PathBuf};

use statsbar_core::label::{Metric, render};

fn main() -> Result<(), Box<dyn Error>> {
    let base = PathBuf::from(std::env::args_os().nth(1).ok_or("expected output directory")?);
    for (name, metric) in [("cpu", Metric::Cpu), ("ram", Metric::Ram)] {
        let folder = base.join(name).join("frames");
        std::fs::create_dir_all(&folder)?;
        for value in [7, 37, 100] {
            let frame = render(metric, value).ok_or("test value must be a percentage")?;
            std::fs::write(folder.join(format!("{value}%.argb")), frame)?;
        }
    }
    Ok(())
}
```
