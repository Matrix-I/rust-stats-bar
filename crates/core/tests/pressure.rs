use statsbar_core::memory::{MemoryBuckets, Pressure};

fn psi(some: &str, full: &str) -> String {
    format!(
        "some avg10={some} avg60=0.00 avg300=0.00 total=6639897\n\
         full avg10={full} avg60=0.00 avg300=0.00 total=5310078\n"
    )
}

#[test]
fn pressure_changes_at_ten_percent() {
    use Pressure::{Critical, Normal, Warning};
    for (some, full, expected) in [
        ("0.00", "0.00", Normal),
        ("9.99", "0.00", Normal),
        ("10.00", "0.00", Warning),
        ("10.01", "9.99", Warning),
        ("12.00", "10.00", Critical),
        ("12.00", "10.01", Critical),
    ] {
        assert_eq!(
            Pressure::from_psi(&psi(some, full)),
            Some(expected),
            "{some} {full}"
        );
    }
}

#[test]
fn pressure_that_cannot_be_read_is_none_and_never_normal() {
    for bad in [
        String::new(),
        "some avg10=0.00 avg60=0.00\n".to_owned(),
        "full avg10=0.00 avg60=0.00\n".to_owned(),
        psi("x", "0.00"),
        psi("NaN", "0.00"),
    ] {
        assert_eq!(Pressure::from_psi(&bad), None, "{bad:?}");
    }
}

#[test]
fn pressure_is_named_as_upstream_names_it() {
    use Pressure::{Critical, Normal, Warning};
    assert_eq!(
        [Normal, Warning, Critical].map(Pressure::label),
        ["Normal", "Warning", "Critical"]
    );
}

#[test]
fn the_pressure_ring_fills_to_kernel_plus_zswap() {
    let buckets = MemoryBuckets {
        total: 1000,
        app: 400,
        kernel: 150,
        cached: 300,
        free: 150,
        swap_used: 0,
        compressed: 100,
    };
    assert_eq!(buckets.pressure_fraction(), 0.25);
    let overfull = MemoryBuckets {
        kernel: u64::MAX,
        ..buckets
    };
    assert_eq!(overfull.pressure_fraction(), 1.0);
    assert_eq!(MemoryBuckets::default().pressure_fraction(), 0.0);
}
