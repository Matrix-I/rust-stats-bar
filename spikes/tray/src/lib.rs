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
