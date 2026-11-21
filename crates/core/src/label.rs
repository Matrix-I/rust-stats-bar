//! The tray's short rate text. The labels themselves are text that the shell draws in its panel
//! font (spike S3), so nothing here draws pixels.

/// A rate as the network label shows it: at most three characters of number and a unit letter,
/// `B`, `K`, `M` or `G`, counted in powers of 1000 as network tools do. One decimal only below
/// 10 M and 10 G, as in `format::menu_bar_rate`. 999.5 GB/s and above read `999G`. NaN and
/// anything not above zero read `0B`.
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
