use statsbar_core::system::{
    Thermal, load_per_cpu, parse_cpu_list, parse_cpu_model, parse_loadavg, parse_millidegrees,
    parse_uptime,
};

#[test]
fn loadavg_yields_its_three_averages() {
    assert_eq!(
        parse_loadavg("0.96 1.24 1.06 2/1738 706483\n"),
        Some([0.96, 1.24, 1.06])
    );
    assert_eq!(parse_loadavg("0.96 1.24"), None);
    assert_eq!(parse_loadavg("NaN 1 1"), None);
    assert_eq!(parse_loadavg("-1 1 1"), None);
}

#[test]
fn load_is_divided_by_the_logical_cpus() {
    assert_eq!(load_per_cpu([8.0, 4.0, 2.0], 8), Some([1.0, 0.5, 0.25]));
    assert_eq!(load_per_cpu([1.0, 1.0, 1.0], 0), None);
}

#[test]
fn uptime_is_the_first_field() {
    assert_eq!(parse_uptime("125693.31 876543.21\n"), Some(125_693.31));
    for bad in ["", "x 1", "-5 1", "inf 1"] {
        assert_eq!(parse_uptime(bad), None, "{bad:?}");
    }
}

#[test]
fn the_model_is_the_first_non_empty_model_name() {
    let text = "processor\t: 0\nmodel name\t: Intel(R) Core(TM) i5-8365U CPU @ 1.60GHz\n\
                processor\t: 1\nmodel name\t: other\n";
    assert_eq!(
        parse_cpu_model(text).as_deref(),
        Some("Intel(R) Core(TM) i5-8365U CPU @ 1.60GHz")
    );
    assert_eq!(parse_cpu_model("model name\t: \n"), None);
    assert_eq!(parse_cpu_model("processor\t: 0\n"), None);
}

#[test]
fn a_cpu_list_expands_ranges_in_order() {
    assert_eq!(parse_cpu_list("0-3\n"), Some(vec![0, 1, 2, 3]));
    assert_eq!(parse_cpu_list("0-1,3,5-6"), Some(vec![0, 1, 3, 5, 6]));
    for bad in ["", "3-1", "a", "0-", "0-999999999"] {
        assert_eq!(parse_cpu_list(bad), None, "{bad:?}");
    }
}

#[test]
fn temperatures_come_in_millidegrees() {
    assert_eq!(parse_millidegrees("84000\n"), Some(84.0));
    assert_eq!(parse_millidegrees("45500"), Some(45.5));
    for bad in ["", "hot", "0", "-1000", "150000"] {
        assert_eq!(parse_millidegrees(bad), None, "{bad:?}");
    }
}

#[test]
fn thermal_levels_change_at_75_90_and_100_degrees() {
    use Thermal::{Critical, Fair, Nominal, Serious};
    for (celsius, expected) in [
        (20.0, Nominal),
        (74.999, Nominal),
        (75.0, Fair),
        (89.999, Fair),
        (90.0, Serious),
        (99.999, Serious),
        (100.0, Critical),
        (110.0, Critical),
    ] {
        assert_eq!(Thermal::from_celsius(celsius), expected, "{celsius}");
    }
}

#[test]
fn thermal_levels_are_named_as_upstream_names_them() {
    use Thermal::{Critical, Fair, Nominal, Serious};
    assert_eq!(
        [Nominal, Fair, Serious, Critical].map(Thermal::label),
        ["Nominal", "Fair", "Serious", "Critical"]
    );
}
