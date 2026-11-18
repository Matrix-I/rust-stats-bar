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
