use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use statsbar_core::memory::MemInfo;
use statsbar_readers::memory::MemoryReader;

const KIB: u64 = 1024;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn reads_meminfo_from_the_fixture_tree() {
    let mut reader = MemoryReader::open(&fixture("normal")).expect("fixture exists");
    assert_eq!(
        reader.read().expect("well-formed"),
        MemInfo {
            total: 16222828 * KIB,
            free: 9012344 * KIB,
            available: 11873320 * KIB,
            s_unreclaim: 98760 * KIB,
            kernel_stack: 14592 * KIB,
            page_tables: 41236 * KIB,
            swap_total: 4194300 * KIB,
            swap_free: 4194300 * KIB,
            zswap: 0,
        }
    );
}

#[test]
fn a_missing_file_fails_to_open_with_not_found() {
    let error = MemoryReader::open(&fixture("no-such-root"))
        .err()
        .expect("there is no proc/meminfo");
    assert_eq!(error.kind(), ErrorKind::NotFound);
}

#[test]
fn a_truncated_file_is_invalid_data() {
    let mut reader = MemoryReader::open(&fixture("truncated")).expect("fixture exists");
    assert_eq!(
        reader.read().expect_err("cut after a key").kind(),
        ErrorKind::InvalidData
    );
}

#[test]
fn bytes_that_are_not_utf8_are_invalid_data() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("memory-not-utf8");
    fs::create_dir_all(root.join("proc")).expect("create scratch proc dir");
    fs::write(root.join("proc/meminfo"), [0xff, 0xfe, b'\n']).expect("write");
    let mut reader = MemoryReader::open(&root).expect("just written");
    assert_eq!(
        reader.read().expect_err("not UTF-8").kind(),
        ErrorKind::InvalidData
    );
}
