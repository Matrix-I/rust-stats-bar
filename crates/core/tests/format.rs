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
