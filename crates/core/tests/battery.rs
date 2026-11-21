use statsbar_core::battery::ChargeStatus::{Charging, Discharging, Full, NotCharging, Unknown};
use statsbar_core::battery::{
    BatteryFigures, BatteryInfo, ChargeStatus, Estimate, MAX_ESTIMATE_MINUTES, parse_uevent,
};

/// `/sys/class/power_supply/BAT0/uevent` on this laptop on 2026-11-19, serial number replaced.
const THIS_LAPTOP: &str = "DEVTYPE=power_supply
POWER_SUPPLY_NAME=BAT0
POWER_SUPPLY_TYPE=Battery
POWER_SUPPLY_STATUS=Not charging
POWER_SUPPLY_PRESENT=1
POWER_SUPPLY_TECHNOLOGY=Li-poly
POWER_SUPPLY_CYCLE_COUNT=0
POWER_SUPPLY_VOLTAGE_MIN_DESIGN=7600000
POWER_SUPPLY_VOLTAGE_NOW=7997000
POWER_SUPPLY_CURRENT_NOW=1000
POWER_SUPPLY_CHARGE_FULL_DESIGN=8948000
POWER_SUPPLY_CHARGE_FULL=5803000
POWER_SUPPLY_CHARGE_NOW=4418000
POWER_SUPPLY_CAPACITY=76
POWER_SUPPLY_CAPACITY_LEVEL=Normal
POWER_SUPPLY_TYPE=Battery
POWER_SUPPLY_MODEL_NAME=DELL C5GV285
POWER_SUPPLY_MANUFACTURER=SMP
POWER_SUPPLY_SERIAL_NUMBER=0000
";

fn info(status: ChargeStatus, charge_now: u64, current: i64) -> BatteryInfo {
    BatteryInfo {
        status,
        capacity_percent: 50,
        charge_now_uah: Some(charge_now),
        charge_full_uah: Some(5_803_000),
        charge_full_design_uah: Some(8_948_000),
        current_now_ua: Some(current),
        voltage_now_uv: Some(8_000_000),
        cycle_count: None,
    }
}

#[test]
fn parses_this_laptops_uevent() {
    assert_eq!(
        parse_uevent(THIS_LAPTOP),
        Some(BatteryInfo {
            status: NotCharging,
            capacity_percent: 76,
            charge_now_uah: Some(4_418_000),
            charge_full_uah: Some(5_803_000),
            charge_full_design_uah: Some(8_948_000),
            current_now_ua: Some(1000),
            voltage_now_uv: Some(7_997_000),
            cycle_count: Some(0),
        })
    );
}

#[test]
fn every_kernel_status_maps_and_anything_else_is_unknown() {
    for (text, status) in [
        ("Charging", Charging),
        ("Discharging", Discharging),
        ("Not charging", NotCharging),
        ("Full", Full),
        ("Unknown", Unknown),
        ("Weird", Unknown),
    ] {
        assert_eq!(ChargeStatus::from_kernel(text), status, "{text}");
    }
}

#[test]
fn status_labels_read_as_the_menu_shows_them() {
    let labels = [Charging, Discharging, NotCharging, Full, Unknown].map(ChargeStatus::label);
    assert_eq!(
        labels,
        ["Charging", "Discharging", "Not charging", "Full", "Unknown"]
    );
}

#[test]
fn status_and_capacity_are_required() {
    assert_eq!(parse_uevent("POWER_SUPPLY_CAPACITY=76\n"), None);
    assert_eq!(parse_uevent("POWER_SUPPLY_STATUS=Full\n"), None);
    assert!(parse_uevent("POWER_SUPPLY_STATUS=Full\nPOWER_SUPPLY_CAPACITY=100\n").is_some());
}

#[test]
fn a_capacity_above_100_or_a_malformed_number_is_refused() {
    let base = "POWER_SUPPLY_STATUS=Full\nPOWER_SUPPLY_CAPACITY=76\n";
    assert_eq!(
        parse_uevent("POWER_SUPPLY_STATUS=Full\nPOWER_SUPPLY_CAPACITY=101\n"),
        None
    );
    assert_eq!(
        parse_uevent(&format!("{base}POWER_SUPPLY_CHARGE_NOW=4x\n")),
        None
    );
    assert_eq!(
        parse_uevent(&format!("{base}POWER_SUPPLY_CHARGE_NOW=\n")),
        None
    );
}

#[test]
fn optional_fields_may_be_missing_and_current_may_be_negative() {
    let parsed = parse_uevent(
        "POWER_SUPPLY_STATUS=Discharging\nPOWER_SUPPLY_CAPACITY=40\n\
         POWER_SUPPLY_CURRENT_NOW=-1500000\n",
    )
    .expect("status and capacity are there");
    assert_eq!(parsed.current_now_ua, Some(-1_500_000));
    assert_eq!(parsed.charge_now_uah, None);
    assert_eq!(parsed.cycle_count, None);
}

#[test]
fn discharging_estimates_time_to_empty_and_power() {
    let figures = BatteryFigures::new(&info(Discharging, 4_000_000, 2_000_000), Some(false));
    assert_eq!(figures.estimate, Some(Estimate::ToEmpty { minutes: 120 }));
    assert_eq!(figures.power_watts, Some(16.0));
    assert_eq!(figures.adapter_online, Some(false));
}

#[test]
fn a_negative_current_counts_by_its_size() {
    let figures = BatteryFigures::new(&info(Discharging, 4_000_000, -2_000_000), None);
    assert_eq!(figures.estimate, Some(Estimate::ToEmpty { minutes: 120 }));
    assert_eq!(figures.power_watts, Some(16.0));
}

#[test]
fn charging_estimates_time_to_full() {
    // 1.385 Ah missing, at 1.385 A.
    let figures = BatteryFigures::new(&info(Charging, 4_418_000, 1_385_000), Some(true));
    assert_eq!(figures.estimate, Some(Estimate::ToFull { minutes: 60 }));
}

#[test]
fn full_not_charging_and_unknown_have_no_estimate() {
    for status in [Full, NotCharging, Unknown] {
        let figures = BatteryFigures::new(&info(status, 4_000_000, 2_000_000), None);
        assert_eq!(figures.estimate, None, "{status:?}");
    }
}

#[test]
fn zero_current_gives_no_estimate_and_no_power() {
    let figures = BatteryFigures::new(&info(Discharging, 4_000_000, 0), None);
    assert_eq!(figures.estimate, None);
    assert_eq!(figures.power_watts, None);
}

#[test]
fn charging_at_or_above_full_has_no_estimate() {
    for charge_now in [5_803_000, 6_000_000] {
        let figures = BatteryFigures::new(&info(Charging, charge_now, 1_000_000), None);
        assert_eq!(figures.estimate, None, "{charge_now}");
    }
}

#[test]
fn an_estimate_over_a_day_is_dropped() {
    let a_day_of_charge = 1_000_000 * MAX_ESTIMATE_MINUTES / 60;
    let longest = BatteryFigures::new(&info(Discharging, a_day_of_charge, 1_000_000), None);
    assert_eq!(
        longest.estimate,
        Some(Estimate::ToEmpty {
            minutes: MAX_ESTIMATE_MINUTES
        })
    );
    // 4.418 Ah at 1 mA, as this laptop reads right after unplugging: 184 days.
    let too_long = BatteryFigures::new(&info(Discharging, 4_418_000, 1000), None);
    assert_eq!(too_long.estimate, None);
}

#[test]
fn health_capacities_voltage_and_percent_come_from_the_uevent() {
    let info = parse_uevent(THIS_LAPTOP).expect("well-formed");
    let figures = BatteryFigures::new(&info, Some(true));
    assert_eq!(figures.percent, 76);
    assert_eq!(figures.status, NotCharging);
    let health = figures.health_percent.expect("both capacities are known");
    assert!((health - 64.852_48).abs() < 0.001, "{health}");
    assert_eq!(
        (figures.full_mah, figures.design_mah),
        (Some(5803), Some(8948))
    );
    assert_eq!(figures.voltage_volts, Some(7.997));
    assert_eq!(figures.adapter_online, Some(true));
}

#[test]
fn health_needs_both_capacities_above_zero() {
    let mut no_design = info(Full, 5_803_000, 0);
    no_design.charge_full_design_uah = Some(0);
    assert_eq!(BatteryFigures::new(&no_design, None).health_percent, None);
    let mut no_full = info(Full, 5_803_000, 0);
    no_full.charge_full_uah = None;
    assert_eq!(BatteryFigures::new(&no_full, None).health_percent, None);
}

#[test]
fn a_cycle_count_of_zero_reads_as_unknown() {
    let mut zero = info(Full, 5_803_000, 0);
    zero.cycle_count = Some(0);
    assert_eq!(BatteryFigures::new(&zero, None).cycles, None);
    let mut counted = zero;
    counted.cycle_count = Some(312);
    assert_eq!(BatteryFigures::new(&counted, None).cycles, Some(312));
}

#[test]
fn milliamp_hours_round_to_the_nearest() {
    let mut odd = info(Full, 0, 0);
    odd.charge_full_uah = Some(5_803_500);
    odd.charge_full_design_uah = Some(8_948_499);
    let figures = BatteryFigures::new(&odd, None);
    assert_eq!(
        (figures.full_mah, figures.design_mah),
        (Some(5804), Some(8948))
    );
}
