use std::collections::HashSet;

use statsbar_core::label::{Metric, SIZE, rate_text, render, render_rates};

fn pixel(pixmap: &[u8], x: usize, y: usize) -> &[u8] {
    let start = (y * SIZE + x) * 4;
    &pixmap[start..start + 4]
}

const ALL: [Metric; 5] = [
    Metric::Cpu,
    Metric::Ram,
    Metric::Bat,
    Metric::Chg,
    Metric::Fan,
];

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
    for metric in ALL {
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
        for metric in ALL {
            assert!(render(metric, value).is_none(), "{metric:?} {value}");
        }
    }
}

#[test]
fn battery_charging_and_fan_words_sit_above_the_same_percentage() {
    let cpu = render(Metric::Cpu, 37).unwrap();
    let words = [
        (
            Metric::Bat,
            [
                "##...#..###",
                "#.#.#.#..#.",
                "##..###..#.",
                "#.#.#.#..#.",
                "##..#.#..#.",
            ],
        ),
        (
            Metric::Chg,
            [
                ".##.#.#..##",
                "#...#.#.#..",
                "#...###.#.#",
                "#...#.#.#.#",
                ".##.#.#..##",
            ],
        ),
        (
            Metric::Fan,
            [
                "###..#..##.",
                "#...#.#.#.#",
                "##..###.#.#",
                "#...#.#.#.#",
                "#...#.#.#.#",
            ],
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
            assert_eq!(
                pixel(&swapped, x, y + 10),
                pixel(&original, x, y),
                "({x}, {y})"
            );
            assert_eq!(
                pixel(&swapped, x, y),
                pixel(&original, x, y + 10),
                "({x}, {y})"
            );
        }
    }
}
