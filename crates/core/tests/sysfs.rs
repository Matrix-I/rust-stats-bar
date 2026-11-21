use statsbar_core::sysfs::{parse_bool, parse_u64};

#[test]
fn a_number_attribute_parses_without_its_newline() {
    assert_eq!(parse_u64("6040\n"), Some(6040));
    assert_eq!(parse_u64("0"), Some(0));
}

#[test]
fn an_empty_negative_or_non_numeric_attribute_is_refused() {
    for text in ["", "\n", "-1\n", "12 RPM\n", "0x10\n"] {
        assert_eq!(parse_u64(text), None, "{text:?}");
    }
}

#[test]
fn a_flag_attribute_is_zero_or_one_only() {
    assert_eq!(parse_bool("1\n"), Some(true));
    assert_eq!(parse_bool("0\n"), Some(false));
    for text in ["", "2\n", "yes\n"] {
        assert_eq!(parse_bool(text), None, "{text:?}");
    }
}
