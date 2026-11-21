use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use statsbar_core::throughput::Sample;
use statsbar_readers::network::NetworkReader;

const HEADER: &str =
    "Iface\tDestination\tGateway \tFlags\tRefCnt\tUse\tMetric\tMask\t\tMTU\tWindow\tIRTT\n";

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
    fs::create_dir_all(root.join("proc/net")).expect("create proc/net");
    root
}

fn route_to(root: &Path, interface: Option<&str>) {
    let line = interface.map_or(String::new(), |name| {
        format!("{name}\t00000000\t0164A8C0\t0003\t0\t0\t600\t00000000\t0\t0\t0\n")
    });
    fs::write(root.join("proc/net/route"), format!("{HEADER}{line}")).expect("write route");
}

fn counters(root: &Path, interface: &str, rx: u64, tx: u64) {
    let dir = root.join(format!("sys/class/net/{interface}/statistics"));
    fs::create_dir_all(&dir).expect("create statistics");
    fs::write(dir.join("rx_bytes"), format!("{rx}\n")).expect("write rx");
    fs::write(dir.join("tx_bytes"), format!("{tx}\n")).expect("write tx");
}

fn sample(interface: &str, rx: u64, tx: u64) -> Option<Sample> {
    Some(Sample {
        interface: interface.to_owned(),
        rx_bytes: rx,
        tx_bytes: tx,
    })
}

#[test]
fn reads_the_default_route_interfaces_counters() {
    let mut reader = NetworkReader::open(&fixture("normal")).expect("fixture exists");
    assert_eq!(
        reader.read().expect("well-formed"),
        sample("wlo1", 678_488_787, 51_234_567)
    );
}

#[test]
fn counters_are_reread_on_each_sample() {
    let root = scratch("network-reread");
    route_to(&root, Some("wlo1"));
    counters(&root, "wlo1", 100, 10);
    let mut reader = NetworkReader::open(&root).expect("just written");
    assert_eq!(reader.read().expect("well-formed"), sample("wlo1", 100, 10));
    counters(&root, "wlo1", 200, 20);
    assert_eq!(reader.read().expect("well-formed"), sample("wlo1", 200, 20));
}

#[test]
fn a_route_change_switches_to_the_new_interfaces_counters() {
    let root = scratch("network-switch");
    route_to(&root, Some("wlo1"));
    counters(&root, "wlo1", 100, 10);
    counters(&root, "eno2", 5000, 500);
    let mut reader = NetworkReader::open(&root).expect("just written");
    assert_eq!(reader.read().expect("well-formed"), sample("wlo1", 100, 10));
    route_to(&root, Some("eno2"));
    assert_eq!(
        reader.read().expect("well-formed"),
        sample("eno2", 5000, 500)
    );
}

#[test]
fn no_default_route_reads_none() {
    let root = scratch("network-no-route");
    route_to(&root, None);
    let mut reader = NetworkReader::open(&root).expect("just written");
    assert_eq!(reader.read().expect("well-formed"), None);
}

#[test]
fn a_default_route_to_an_interface_without_counters_is_not_found() {
    let root = scratch("network-ghost");
    route_to(&root, Some("ghost0"));
    let mut reader = NetworkReader::open(&root).expect("just written");
    assert_eq!(
        reader.read().expect_err("no counters").kind(),
        ErrorKind::NotFound
    );
}

#[test]
fn a_malformed_route_table_is_invalid_data() {
    let mut reader = NetworkReader::open(&fixture("truncated")).expect("fixture exists");
    assert_eq!(
        reader.read().expect_err("cut mid-line").kind(),
        ErrorKind::InvalidData
    );
}

#[test]
fn a_malformed_counter_is_invalid_data_not_a_reset() {
    let root = scratch("network-garbled");
    route_to(&root, Some("wlo1"));
    counters(&root, "wlo1", 10, 20);
    fs::write(root.join("sys/class/net/wlo1/statistics/rx_bytes"), "x\n").expect("write rx");
    let mut reader = NetworkReader::open(&root).expect("just written");
    assert_eq!(
        reader.read().expect_err("rx_bytes is not a number").kind(),
        ErrorKind::InvalidData
    );
}

#[test]
fn a_missing_route_table_fails_to_open_with_not_found() {
    let error = NetworkReader::open(&fixture("no-such-root"))
        .err()
        .expect("no proc/net/route");
    assert_eq!(error.kind(), ErrorKind::NotFound);
}
