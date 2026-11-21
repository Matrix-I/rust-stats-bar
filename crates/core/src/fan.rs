//! The fan: hwmon RPM readings in, how far along its range it runs out.

/// One reading of a fan and the limits its driver reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FanSpeed {
    pub rpm: u64,
    pub min_rpm: u64,
    pub max_rpm: u64,
}

/// Where `speed.rpm` sits between `min_rpm` and `max_rpm`, from 0 to 1. Clamped, because
/// `dell_smm` reports a `fan1_max` of 5300 while this laptop's fan was read at 6040. `None` when
/// the range is empty or inverted.
pub fn range_fraction(speed: &FanSpeed) -> Option<f64> {
    if speed.max_rpm <= speed.min_rpm {
        return None;
    }
    let span = (speed.max_rpm - speed.min_rpm) as f64;
    let above_min = speed.rpm.saturating_sub(speed.min_rpm) as f64;
    Some((above_min / span).min(1.0))
}
