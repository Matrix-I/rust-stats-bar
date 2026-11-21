//! The laptop battery: `power_supply/BAT0/uevent` text in, the figures of its menu out.

const MICRO: f64 = 1_000_000.0;
const MICRO_PER_MILLI: u64 = 1000;
const MINUTES_PER_HOUR: u128 = 60;

/// Estimates longer than a day are dropped. They come from a current too small to mean anything:
/// right after unplugging, this laptop reads 1 mA, which would promise 184 days.
pub const MAX_ESTIMATE_MINUTES: u64 = 24 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargeStatus {
    Charging,
    Discharging,
    /// Plugged in but held, for example at a charge threshold.
    NotCharging,
    Full,
    Unknown,
}

impl ChargeStatus {
    /// Maps the kernel's `POWER_SUPPLY_STATUS` text. Anything unrecognised reads as `Unknown`.
    pub fn from_kernel(text: &str) -> ChargeStatus {
        match text {
            "Charging" => ChargeStatus::Charging,
            "Discharging" => ChargeStatus::Discharging,
            "Not charging" => ChargeStatus::NotCharging,
            "Full" => ChargeStatus::Full,
            _ => ChargeStatus::Unknown,
        }
    }

    /// The text of the Status row and the hub row.
    pub fn label(self) -> &'static str {
        match self {
            ChargeStatus::Charging => "Charging",
            ChargeStatus::Discharging => "Discharging",
            ChargeStatus::NotCharging => "Not charging",
            ChargeStatus::Full => "Full",
            ChargeStatus::Unknown => "Unknown",
        }
    }
}

/// The `uevent` fields the figures are built from, in the kernel's micro-units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatteryInfo {
    pub status: ChargeStatus,
    pub capacity_percent: u8,
    pub charge_now_uah: Option<u64>,
    pub charge_full_uah: Option<u64>,
    pub charge_full_design_uah: Option<u64>,
    /// Signed, because some drivers report a negative current while discharging. Dell's is
    /// positive both ways.
    pub current_now_ua: Option<i64>,
    pub voltage_now_uv: Option<u64>,
    pub cycle_count: Option<u64>,
}

/// Parses a battery's `uevent`.
///
/// Returns `None` without `POWER_SUPPLY_STATUS`, or without a `POWER_SUPPLY_CAPACITY` from 0 to
/// 100, because the label needs both. It also returns `None` when a number it reads is malformed,
/// as the other `/proc` parsers do. The other fields are `None` when absent: the `CHARGE_*`
/// fields exist only on batteries that count charge rather than energy.
pub fn parse_uevent(text: &str) -> Option<BatteryInfo> {
    let mut status = None;
    let mut capacity = None;
    let mut charge_now = None;
    let mut charge_full = None;
    let mut charge_full_design = None;
    let mut current_now = None;
    let mut voltage_now = None;
    let mut cycle_count = None;
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "POWER_SUPPLY_STATUS" => status = Some(ChargeStatus::from_kernel(value)),
            "POWER_SUPPLY_CAPACITY" => {
                capacity = Some(value.parse::<u8>().ok().filter(|&percent| percent <= 100)?);
            }
            "POWER_SUPPLY_CHARGE_NOW" => charge_now = Some(value.parse().ok()?),
            "POWER_SUPPLY_CHARGE_FULL" => charge_full = Some(value.parse().ok()?),
            "POWER_SUPPLY_CHARGE_FULL_DESIGN" => charge_full_design = Some(value.parse().ok()?),
            "POWER_SUPPLY_CURRENT_NOW" => current_now = Some(value.parse().ok()?),
            "POWER_SUPPLY_VOLTAGE_NOW" => voltage_now = Some(value.parse().ok()?),
            "POWER_SUPPLY_CYCLE_COUNT" => cycle_count = Some(value.parse().ok()?),
            _ => {}
        }
    }
    Some(BatteryInfo {
        status: status?,
        capacity_percent: capacity?,
        charge_now_uah: charge_now,
        charge_full_uah: charge_full,
        charge_full_design_uah: charge_full_design,
        current_now_ua: current_now,
        voltage_now_uv: voltage_now,
        cycle_count,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estimate {
    ToEmpty { minutes: u64 },
    ToFull { minutes: u64 },
}

/// What the battery's menu and label show. A `None` field has no row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BatteryFigures {
    pub percent: u8,
    pub status: ChargeStatus,
    pub estimate: Option<Estimate>,
    pub power_watts: Option<f64>,
    /// Full-charge capacity against design capacity. Not clamped: a new battery can exceed 100.
    pub health_percent: Option<f64>,
    pub full_mah: Option<u64>,
    pub design_mah: Option<u64>,
    pub voltage_volts: Option<f64>,
    /// `None` for 0 as well as for a missing count: a worn battery at zero cycles means the
    /// firmware does not keep the count.
    pub cycles: Option<u64>,
    pub adapter_online: Option<bool>,
}

impl BatteryFigures {
    pub fn new(info: &BatteryInfo, adapter_online: Option<bool>) -> BatteryFigures {
        let current = info
            .current_now_ua
            .map(i64::unsigned_abs)
            .filter(|&current| current > 0);
        let health_percent = match (info.charge_full_uah, info.charge_full_design_uah) {
            (Some(full), Some(design)) if full > 0 && design > 0 => {
                Some(full as f64 / design as f64 * 100.0)
            }
            _ => None,
        };
        BatteryFigures {
            percent: info.capacity_percent,
            status: info.status,
            estimate: current.and_then(|current| estimate(info, current)),
            power_watts: current
                .zip(info.voltage_now_uv)
                .map(|(current, voltage)| current as f64 / MICRO * (voltage as f64 / MICRO)),
            health_percent,
            full_mah: info.charge_full_uah.map(milliamp_hours),
            design_mah: info.charge_full_design_uah.map(milliamp_hours),
            voltage_volts: info.voltage_now_uv.map(|voltage| voltage as f64 / MICRO),
            cycles: info.cycle_count.filter(|&cycles| cycles > 0),
            adapter_online,
        }
    }
}

/// `current_ua` is above zero.
fn estimate(info: &BatteryInfo, current_ua: u64) -> Option<Estimate> {
    let now = info.charge_now_uah?;
    match info.status {
        ChargeStatus::Discharging => Some(Estimate::ToEmpty {
            minutes: minutes_for(now, current_ua)?,
        }),
        ChargeStatus::Charging => {
            let missing = info
                .charge_full_uah?
                .checked_sub(now)
                .filter(|&missing| missing > 0)?;
            Some(Estimate::ToFull {
                minutes: minutes_for(missing, current_ua)?,
            })
        }
        ChargeStatus::NotCharging | ChargeStatus::Full | ChargeStatus::Unknown => None,
    }
}

/// Whole minutes to move `charge_uah` at `current_ua`, or `None` beyond a day. Computed in
/// `u128`, so no product of two `u64` values can overflow.
fn minutes_for(charge_uah: u64, current_ua: u64) -> Option<u64> {
    let minutes = u128::from(charge_uah) * MINUTES_PER_HOUR / u128::from(current_ua);
    u64::try_from(minutes)
        .ok()
        .filter(|&minutes| minutes <= MAX_ESTIMATE_MINUTES)
}

fn milliamp_hours(micro_amp_hours: u64) -> u64 {
    micro_amp_hours.saturating_add(MICRO_PER_MILLI / 2) / MICRO_PER_MILLI
}
