use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use statsbar_core::fan::FanSpeed;
use statsbar_readers::fan::FanReader;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn scratch(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    match fs::remove_dir_all(&root) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => panic!("clearing {}: {error}", root.display()),
    }
    root
}

/// Writes an hwmon device with the given files.
fn device(root: &Path, number: u8, files: &[(&str, &str)]) {
    let dir = root.join(format!("sys/class/hwmon/hwmon{number}"));
    fs::create_dir_all(&dir).expect("create device dir");
    for (name, text) in files {
        fs::write(dir.join(name), text).expect("write");
    }
}

#[test]
fn finds_dell_smm_among_the_hwmon_devices_and_reads_its_fan() {
    let mut reader = FanReader::open(&fixture("normal")).expect("fixture exists");
    assert_eq!(
        reader.read().expect("well-formed"),
        FanSpeed {
            rpm: 6040,
            min_rpm: 0,
            max_rpm: 5300
        }
    );
}

#[test]
fn a_second_read_sees_the_new_speed() {
    let root = scratch("fan-reread");
    let limits = [
        ("name", "dell_smm\n"),
        ("fan1_min", "0\n"),
        ("fan1_max", "5300\n"),
    ];
    device(
        &root,
        3,
        &[limits[0], limits[1], limits[2], ("fan1_input", "3000\n")],
    );
    let mut reader = FanReader::open(&root).expect("just written");
    assert_eq!(reader.read().expect("well-formed").rpm, 3000);
    device(&root, 3, &[("fan1_input", "3100\n")]);
    assert_eq!(reader.read().expect("well-formed").rpm, 3100);
}

#[test]
fn no_dell_smm_device_is_not_found() {
    let root = scratch("fan-no-device");
    device(&root, 0, &[("name", "AC\n")]);
    let error = FanReader::open(&root).err().expect("no dell_smm");
    assert_eq!(error.kind(), ErrorKind::NotFound);
}

#[test]
fn a_device_without_a_name_is_skipped() {
    let root = scratch("fan-nameless");
    device(&root, 0, &[]);
    device(
        &root,
        1,
        &[
            ("name", "dell_smm\n"),
            ("fan1_input", "2000\n"),
            ("fan1_min", "0\n"),
            ("fan1_max", "5300\n"),
        ],
    );
    let mut reader = FanReader::open(&root).expect("hwmon1 is dell_smm");
    assert_eq!(reader.read().expect("well-formed").rpm, 2000);
}

#[test]
fn no_hwmon_directory_is_not_found() {
    let error = FanReader::open(&fixture("no-such-root"))
        .err()
        .expect("no sys/class/hwmon");
    assert_eq!(error.kind(), ErrorKind::NotFound);
}

#[test]
fn an_empty_speed_is_invalid_data() {
    let mut reader = FanReader::open(&fixture("truncated")).expect("fixture exists");
    assert_eq!(
        reader.read().expect_err("empty fan1_input").kind(),
        ErrorKind::InvalidData
    );
}

#[test]
fn a_malformed_limit_fails_to_open_with_invalid_data() {
    let root = scratch("fan-bad-limit");
    device(
        &root,
        7,
        &[
            ("name", "dell_smm\n"),
            ("fan1_input", "2000\n"),
            ("fan1_min", "0\n"),
            ("fan1_max", "lots\n"),
        ],
    );
    let error = FanReader::open(&root)
        .err()
        .expect("fan1_max is not a number");
    assert_eq!(error.kind(), ErrorKind::InvalidData);
}

#[test]
fn a_missing_limit_fails_to_open_with_not_found() {
    let root = scratch("fan-no-limit");
    device(
        &root,
        7,
        &[
            ("name", "dell_smm\n"),
            ("fan1_input", "2000\n"),
            ("fan1_min", "0\n"),
        ],
    );
    let error = FanReader::open(&root).err().expect("no fan1_max");
    assert_eq!(error.kind(), ErrorKind::NotFound);
}
