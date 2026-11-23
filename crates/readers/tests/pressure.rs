mod support;

use std::io::ErrorKind;
use std::path::Path;

use statsbar_core::memory::Pressure;
use statsbar_readers::pressure::PressureReader;
use support::Tree;

const PSI: &str = "proc/pressure/memory";

fn psi(some: &str) -> String {
    format!(
        "some avg10={some} avg60=0.00 avg300=0.00 total=6639897\n\
         full avg10=0.00 avg60=0.00 avg300=0.00 total=5310078\n"
    )
}

#[test]
fn reads_the_level_and_rereads_it() {
    let tree = Tree::new("pressure-reread");
    tree.write(PSI, psi("0.00"));
    let mut reader = PressureReader::new(&tree.root);
    assert_eq!(reader.read().expect("well-formed"), Pressure::Normal);
    tree.write(PSI, psi("11.00"));
    assert_eq!(reader.read().expect("well-formed"), Pressure::Warning);
}

#[test]
fn a_kernel_without_psi_is_not_found_and_is_retried() {
    let tree = Tree::new("pressure-missing");
    let mut reader = PressureReader::new(&tree.root);
    assert_eq!(
        reader.read().expect_err("no file").kind(),
        ErrorKind::NotFound
    );
    tree.write(PSI, psi("0.00"));
    assert_eq!(reader.read().expect("there now"), Pressure::Normal);
}

#[test]
fn a_malformed_file_is_invalid_data() {
    let tree = Tree::new("pressure-malformed");
    tree.write(PSI, "some\n");
    let mut reader = PressureReader::new(&tree.root);
    assert_eq!(
        reader.read().expect_err("malformed").kind(),
        ErrorKind::InvalidData
    );
}

/// This kernel, or the CI runner's: read when it has PSI, not found when it has none.
#[test]
fn the_real_root_is_read_when_the_kernel_has_psi() {
    let result = PressureReader::new(Path::new("/")).read();
    if Path::new("/proc/pressure/memory").exists() {
        result.expect("a kernel with PSI");
    } else {
        assert_eq!(result.expect_err("no PSI").kind(), ErrorKind::NotFound);
    }
}
