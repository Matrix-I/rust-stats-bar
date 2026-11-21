use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use statsbar_core::battery::{BatteryInfo, ChargeStatus};
use statsbar_readers::battery::{BatteryReader, BatteryReading};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// A fresh root per test with an empty `BAT0` directory, so parallel tests never share files.
fn scratch(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    match fs::remove_dir_all(&root) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => panic!("clearing {}: {error}", root.display()),
    }
    fs::create_dir_all(root.join("sys/class/power_supply/BAT0")).expect("create BAT0");
    root
}

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().expect("has a parent")).expect("create parent");
    fs::write(path, text).expect("write");
}

fn uevent(capacity: u8) -> String {
    format!("POWER_SUPPLY_STATUS=Discharging\nPOWER_SUPPLY_CAPACITY={capacity}\n")
}

const UEVENT: &str = "sys/class/power_supply/BAT0/uevent";
const ONLINE: &str = "sys/class/power_supply/AC/online";

#[test]
fn reads_the_battery_and_the_adapter_from_the_fixture_tree() {
    let mut reader = BatteryReader::open(&fixture("normal")).expect("fixture exists");
    assert_eq!(
        reader.read().expect("well-formed"),
        BatteryReading {
            info: BatteryInfo {
                status: ChargeStatus::NotCharging,
                capacity_percent: 76,
                charge_now_uah: Some(4_418_000),
                charge_full_uah: Some(5_803_000),
                charge_full_design_uah: Some(8_948_000),
                current_now_ua: Some(1000),
                voltage_now_uv: Some(7_997_000),
                cycle_count: Some(0),
            },
            adapter_online: Some(true),
        }
    );
}

#[test]
fn a_second_read_sees_the_files_as_they_are_now() {
    let root = scratch("battery-reread");
    write(&root, UEVENT, &uevent(50));
    write(&root, ONLINE, "0\n");
    let mut reader = BatteryReader::open(&root).expect("just written");
    let first = reader.read().expect("well-formed");
    assert_eq!(
        (first.info.capacity_percent, first.adapter_online),
        (50, Some(false))
    );
    write(&root, UEVENT, &uevent(51));
    write(&root, ONLINE, "1\n");
    let second = reader.read().expect("well-formed");
    assert_eq!(
        (second.info.capacity_percent, second.adapter_online),
        (51, Some(true))
    );
}

#[test]
fn a_machine_without_an_adapter_reads_none_for_it() {
    let root = scratch("battery-no-adapter");
    write(&root, UEVENT, &uevent(50));
    let mut reader = BatteryReader::open(&root).expect("BAT0 is there");
    assert_eq!(reader.read().expect("well-formed").adapter_online, None);
}

#[test]
fn a_missing_battery_fails_to_open_with_not_found() {
    let error = BatteryReader::open(&fixture("no-such-root"))
        .err()
        .expect("there is no BAT0");
    assert_eq!(error.kind(), ErrorKind::NotFound);
}

#[test]
fn a_truncated_uevent_is_invalid_data() {
    let mut reader = BatteryReader::open(&fixture("truncated")).expect("fixture exists");
    assert_eq!(
        reader.read().expect_err("no capacity").kind(),
        ErrorKind::InvalidData
    );
}

#[test]
fn an_adapter_value_other_than_0_or_1_is_invalid_data() {
    let root = scratch("battery-bad-adapter");
    write(&root, UEVENT, &uevent(50));
    write(&root, ONLINE, "2\n");
    let mut reader = BatteryReader::open(&root).expect("just written");
    assert_eq!(
        reader.read().expect_err("not a flag").kind(),
        ErrorKind::InvalidData
    );
}
