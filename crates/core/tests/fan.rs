use statsbar_core::fan::{FanSpeed, range_fraction};

fn speed(rpm: u64) -> FanSpeed {
    FanSpeed {
        rpm,
        min_rpm: 0,
        max_rpm: 5300,
    }
}

#[test]
fn half_way_up_the_range_is_one_half() {
    assert_eq!(range_fraction(&speed(2650)), Some(0.5));
}

#[test]
fn above_max_clamps_to_one() {
    // This laptop: 6040 RPM against a fan1_max of 5300.
    assert_eq!(range_fraction(&speed(6040)), Some(1.0));
}

#[test]
fn stopped_or_below_min_is_zero() {
    assert_eq!(range_fraction(&speed(0)), Some(0.0));
    let below = FanSpeed {
        rpm: 500,
        min_rpm: 1000,
        max_rpm: 5000,
    };
    assert_eq!(range_fraction(&below), Some(0.0));
}

#[test]
fn the_range_is_measured_from_min() {
    let speed = FanSpeed {
        rpm: 3000,
        min_rpm: 1000,
        max_rpm: 5000,
    };
    assert_eq!(range_fraction(&speed), Some(0.5));
}

#[test]
fn an_empty_or_inverted_range_has_no_fraction() {
    for (min_rpm, max_rpm) in [(0, 0), (5300, 5300), (5300, 1000)] {
        let speed = FanSpeed {
            rpm: 3000,
            min_rpm,
            max_rpm,
        };
        assert_eq!(range_fraction(&speed), None, "{min_rpm}..{max_rpm}");
    }
}
