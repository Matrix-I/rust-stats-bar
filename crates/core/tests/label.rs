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
fn every_pixel_is_transparent_or_opaque_white() {
    // The other tests compare alpha only for RAM and single digits; this one pins the colour of
    // every frame, so ink of another colour cannot pass unnoticed outside the CPU 100% frame.
    for metric in [Metric::Cpu, Metric::Ram] {
        for percent in 0..=100 {
            let pixmap = render(metric, percent).unwrap();
            for (index, pixel) in pixmap.chunks_exact(4).enumerate() {
                assert!(
                    pixel == [0; 4] || pixel == [255; 4],
                    "{metric:?} {percent}% pixel {index}: {pixel:?}"
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
