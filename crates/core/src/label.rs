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
