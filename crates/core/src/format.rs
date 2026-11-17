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
