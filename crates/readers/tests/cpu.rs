use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use statsbar_core::cpu::CoreTicks;
use statsbar_readers::cpu::CpuReader;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// A fresh root per test under Cargo's scratch space, so parallel tests never share files.
fn scratch(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    match fs::remove_dir_all(&root) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => panic!("clearing {}: {error}", root.display()),
    }
    fs::create_dir_all(root.join("proc")).expect("create scratch proc dir");
    root
}

#[test]
fn reads_per_core_ticks_from_the_fixture_tree() {
    let mut reader = CpuReader::open(&fixture("normal")).expect("fixture exists");
    assert_eq!(
        reader.read().expect("well-formed"),
        vec![
            CoreTicks {
                user: 5208,
                nice: 12,
                system: 2084,
                idle: 90615
            },
            CoreTicks {
                user: 4987,
                nice: 9,
                system: 1900,
                idle: 91070
            },
        ]
    );
}

#[test]
fn a_missing_file_fails_to_open_with_not_found() {
    let error = CpuReader::open(&fixture("no-such-root"))
        .err()
        .expect("there is no proc/stat");
    assert_eq!(error.kind(), ErrorKind::NotFound);
}

#[test]
fn a_truncated_file_is_invalid_data() {
    let mut reader = CpuReader::open(&fixture("truncated")).expect("fixture exists");
    assert_eq!(
        reader.read().expect_err("cut mid-line").kind(),
        ErrorKind::InvalidData
    );
}

#[test]
fn every_read_sees_the_file_as_it_is_now() {
    let root = scratch("cpu-reread");
    let stat = root.join("proc/stat");
    fs::write(
        &stat,
        "cpu0 1 2 3 4 0 0 0 0 0 0\ncpu1 5 6 7 8 0 0 0 0 0 0\n",
    )
    .expect("first");
    let mut reader = CpuReader::open(&root).expect("just written");
    assert_eq!(reader.read().expect("first sample").len(), 2);
    // Rewritten in place and shorter: a reader that cached the text or its length would still
    // report two cores.
    fs::write(&stat, "cpu0 10 20 30 40\n").expect("second");
    assert_eq!(
        reader.read().expect("second sample"),
        vec![CoreTicks {
            user: 10,
            nice: 20,
            system: 30,
            idle: 40
        }]
    );
}

#[test]
fn a_file_larger_than_the_cap_is_rejected_instead_of_buffered() {
    let root = scratch("cpu-oversized");
    let line = "cpu0 1 2 3 4\n";
    fs::write(root.join("proc/stat"), line.repeat(300 * 1024 / line.len())).expect("write");
    let mut reader = CpuReader::open(&root).expect("just written");
    let error = reader.read().expect_err("over the cap");
    assert_eq!(error.kind(), ErrorKind::InvalidData);
    // The content parses, so only the size cap can have produced this error.
    assert!(error.to_string().contains("256 KiB"), "{error}");
}
