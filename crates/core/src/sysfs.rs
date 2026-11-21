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
