use statsbar_core::memory::{MemInfo, MemoryBuckets, parse_meminfo};

const KIB: u64 = 1024;

const MEMINFO: &str = "\
MemTotal:       16000000 kB
MemFree:         2000000 kB
MemAvailable:    9000000 kB
Buffers:          500000 kB
Cached:          6000000 kB
SwapCached:        12345 kB
SwapTotal:       4000000 kB
SwapFree:        3000000 kB
Zswap:             64000 kB
Shmem:           1000000 kB
Slab:            1000000 kB
SReclaimable:     700000 kB
SUnreclaim:       300000 kB
KernelStack:       20000 kB
PageTables:        80000 kB
HugePages_Total:       0
";

#[test]
fn parses_the_fields_the_buckets_need_in_bytes() {
    let info = parse_meminfo(MEMINFO).expect("MemTotal and MemAvailable are present");
    assert_eq!(
        info,
        MemInfo {
            total: 16_000_000 * KIB,
            free: 2_000_000 * KIB,
            available: 9_000_000 * KIB,
            s_unreclaim: 300_000 * KIB,
            kernel_stack: 20_000 * KIB,
            page_tables: 80_000 * KIB,
            swap_total: 4_000_000 * KIB,
            swap_free: 3_000_000 * KIB,
            zswap: 64_000 * KIB,
        }
    );
}

#[test]
fn a_missing_optional_field_reads_as_zero() {
    let info =
        parse_meminfo("MemTotal: 1000 kB\nMemAvailable: 600 kB\n").expect("required present");
    assert_eq!(info.total, 1000 * KIB);
    assert_eq!(info.free, 0);
    assert_eq!(info.zswap, 0);
}

#[test]
fn a_zero_mem_available_is_a_value_not_a_missing_field() {
    let info = parse_meminfo("MemTotal: 1000 kB\nMemAvailable: 0 kB\n").expect("present");
    assert_eq!(MemoryBuckets::from_meminfo(&info).usage_percent(), 100.0);
}

#[test]
fn rejects_text_without_a_positive_total_or_without_mem_available() {
    assert_eq!(parse_meminfo(""), None);
    assert_eq!(parse_meminfo("MemAvailable: 400 kB\n"), None);
    assert_eq!(parse_meminfo("MemTotal: 0 kB\nMemAvailable: 0 kB\n"), None);
    assert_eq!(parse_meminfo("MemTotal: 1000 kB\nMemFree: 400 kB\n"), None);
}

#[test]
fn rejects_a_field_that_is_not_a_number() {
    assert_eq!(
        parse_meminfo("MemTotal: lots kB\nMemAvailable: 1 kB\n"),
        None
    );
}

#[test]
fn rejects_a_kib_value_whose_byte_count_overflows() {
    let text = "MemTotal: 18446744073709551615 kB\nMemAvailable: 1 kB\n";
    assert_eq!(parse_meminfo(text), None);
}

#[test]
fn used_is_total_minus_available_split_into_app_and_kernel() {
    let buckets = MemoryBuckets::from_meminfo(&parse_meminfo(MEMINFO).expect("valid"));
    assert_eq!(buckets.used(), 7_000_000 * KIB); // MemTotal - MemAvailable, as GNOME shows
    assert_eq!(buckets.kernel, 400_000 * KIB); // SUnreclaim + KernelStack + PageTables
    assert_eq!(buckets.app, 6_600_000 * KIB);
    assert_eq!(buckets.free, 2_000_000 * KIB);
    assert_eq!(buckets.cached, 7_000_000 * KIB); // MemAvailable - MemFree
    assert_eq!(buckets.swap_used, 1_000_000 * KIB);
    assert_eq!(buckets.compressed, 64_000 * KIB);
    assert_eq!(buckets.usage_percent(), 43.75);
    assert_eq!(buckets.fraction(buckets.free), 0.125);
}

fn assert_adds_up(buckets: &MemoryBuckets) {
    assert_eq!(
        buckets.app + buckets.kernel + buckets.cached + buckets.free,
        buckets.total
    );
}

#[test]
fn free_above_available_is_clamped_so_the_bar_adds_up() {
    // MemAvailable holds back the kernel's reserve, so it can be below MemFree.
    let info = MemInfo {
        total: 1000,
        free: 300,
        available: 200,
        ..MemInfo::default()
    };
    let buckets = MemoryBuckets::from_meminfo(&info);
    assert_eq!(buckets.used(), 800);
    assert_eq!(buckets.free, 200);
    assert_eq!(buckets.cached, 0);
    assert_adds_up(&buckets);
}

#[test]
fn kernel_memory_above_used_is_clamped_to_used() {
    let info = MemInfo {
        total: 1000,
        free: 900,
        available: 950,
        s_unreclaim: 200,
        ..MemInfo::default()
    };
    let buckets = MemoryBuckets::from_meminfo(&info);
    assert_eq!(buckets.kernel, 50);
    assert_eq!(buckets.app, 0);
    assert_adds_up(&buckets);
}

#[test]
fn available_above_total_reads_as_nothing_used() {
    let info = MemInfo {
        total: 1000,
        free: 1500,
        available: 1200,
        page_tables: 10,
        ..MemInfo::default()
    };
    let buckets = MemoryBuckets::from_meminfo(&info);
    assert_eq!(buckets.used(), 0);
    assert_eq!(buckets.free, 1000);
    assert_adds_up(&buckets);
}

#[test]
fn kernel_fields_that_would_overflow_saturate() {
    let info = MemInfo {
        total: 1000,
        available: 400,
        s_unreclaim: u64::MAX,
        kernel_stack: u64::MAX,
        ..MemInfo::default()
    };
    let buckets = MemoryBuckets::from_meminfo(&info);
    assert_eq!(buckets.kernel, 600);
    assert_adds_up(&buckets);
}

#[test]
fn swap_free_above_swap_total_reads_as_no_swap_used() {
    let info = MemInfo {
        total: 1000,
        swap_total: 10,
        swap_free: 20,
        ..MemInfo::default()
    };
    assert_eq!(MemoryBuckets::from_meminfo(&info).swap_used, 0);
}

#[test]
fn an_unknown_total_gives_zero_fractions_instead_of_dividing_by_zero() {
    let buckets = MemoryBuckets::default();
    assert_eq!(buckets.fraction(123), 0.0);
    assert_eq!(buckets.usage_percent(), 0.0);
}

#[test]
fn used_saturates_on_buckets_that_do_not_add_up() {
    // Buckets a peer sent, not ones `from_meminfo` built.
    let buckets = MemoryBuckets {
        total: u64::MAX,
        app: u64::MAX,
        kernel: 1,
        ..MemoryBuckets::default()
    };
    assert_eq!(buckets.used(), u64::MAX);
    assert_eq!(buckets.usage_percent(), 100.0);
}
