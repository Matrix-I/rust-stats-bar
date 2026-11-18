use statsbar_core::cpu::CpuLoad;
use statsbar_core::memory::MemoryBuckets;
use statsbard::summary::{cpu_rows, hub_rows, memory_rows, percent, title};

const GIB: u64 = 1024 * 1024 * 1024;

fn load() -> CpuLoad {
    CpuLoad {
        system_percent: 8.4,
        user_percent: 15.2,
        idle_percent: 76.4,
        per_core_busy: vec![20.0, 27.2, 23.6, 23.6],
    }
}

fn buckets(compressed: u64) -> MemoryBuckets {
    MemoryBuckets {
        total: 16 * GIB,
        app: 6 * GIB,
        kernel: 2 * GIB,
        cached: 4 * GIB,
        free: 4 * GIB,
        swap_used: GIB / 2,
        compressed,
    }
}

#[test]
fn percent_rounds_and_clamps() {
    assert_eq!(percent(23.5), 24);
    assert_eq!(percent(23.4), 23);
    assert_eq!(percent(-3.0), 0);
    assert_eq!(percent(100.4), 100);
    assert_eq!(percent(250.0), 100);
    assert_eq!(percent(f64::NAN), 0);
}

#[test]
fn cpu_rows_show_usage_split_and_cores() {
    assert_eq!(
        cpu_rows(&load()),
        [
            "Usage 24 %",
            "User 15 %",
            "System 8 %",
            "Idle 76 %",
            "Cores 4"
        ]
    );
}

#[test]
fn memory_rows_show_used_and_each_bucket() {
    assert_eq!(
        memory_rows(&buckets(0)),
        [
            "Used 8.00 GB of 16.00 GB (50 %)",
            "App 6.00 GB",
            "Kernel 2.00 GB",
            "Cached 4.00 GB",
            "Free 4.00 GB",
            "Swap 0.50 GB",
        ]
    );
}

#[test]
fn compressed_appears_only_when_there_is_some() {
    let rows = memory_rows(&buckets(GIB));
    assert_eq!(rows.last().map(String::as_str), Some("Compressed 1.00 GB"));
}

#[test]
fn hub_rows_cover_shown_metrics_only() {
    let (cpu, memory) = (load(), buckets(0));
    assert_eq!(
        hub_rows(Some(&cpu), Some(&memory), true, true),
        ["CPU 24 %", "Memory 50 % · 8.00 GB / 16.00 GB"]
    );
    assert_eq!(hub_rows(Some(&cpu), Some(&memory), false, true).len(), 1);
    assert!(hub_rows(Some(&cpu), Some(&memory), false, false).is_empty());
}

#[test]
fn a_shown_metric_without_a_value_yet_reads_a_dash() {
    assert_eq!(hub_rows(None, None, true, true), ["CPU —", "Memory —"]);
}

#[test]
fn titles_carry_the_value_when_there_is_one() {
    assert_eq!(title("CPU", Some(7)), "CPU 7 %");
    assert_eq!(title("Memory", None), "Memory");
}
