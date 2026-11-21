use statsbar_core::battery::{BatteryFigures, ChargeStatus, Estimate};
use statsbar_core::cpu::CpuLoad;
use statsbar_core::fan::FanSpeed;
use statsbar_core::memory::MemoryBuckets;
use statsbar_core::throughput::Reading;
use statsbard::config::Items;
use statsbard::summary::{
    Entry, Overview, battery_rows, cpu_rows, fan_rows, hub_rows, memory_rows, network_rows,
    network_title, percent, title,
};

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

fn battery() -> BatteryFigures {
    BatteryFigures {
        percent: 76,
        status: ChargeStatus::Discharging,
        estimate: Some(Estimate::ToEmpty { minutes: 125 }),
        power_watts: Some(7.96),
        health_percent: Some(64.85),
        full_mah: Some(5803),
        design_mah: Some(8948),
        voltage_volts: Some(7.997),
        cycles: Some(312),
        adapter_online: Some(false),
    }
}

fn fan() -> FanSpeed {
    FanSpeed {
        rpm: 6040,
        min_rpm: 0,
        max_rpm: 5300,
    }
}

fn reading() -> Reading {
    Reading {
        download_total: 1_200_000_000,
        upload_total: 300_000_000,
        download_rate: 1_200_000.0,
        upload_rate: 34_000.0,
    }
}

fn cpu_and_memory<'a>(cpu: Entry<&'a CpuLoad>, memory: Entry<&'a MemoryBuckets>) -> Overview<'a> {
    Overview {
        cpu,
        memory,
        battery: Entry::Hidden,
        fan: Entry::Hidden,
        network: Entry::Hidden,
    }
}

#[test]
fn hub_rows_cover_shown_metrics_only() {
    let (cpu, memory) = (load(), buckets(0));
    assert_eq!(
        hub_rows(&cpu_and_memory(Entry::Value(&cpu), Entry::Value(&memory))),
        ["CPU 24 %", "Memory 50 % · 8.00 GB / 16.00 GB"]
    );
    assert_eq!(
        hub_rows(&cpu_and_memory(Entry::Hidden, Entry::Value(&memory))).len(),
        1
    );
    assert!(hub_rows(&cpu_and_memory(Entry::Hidden, Entry::Hidden)).is_empty());
}

#[test]
fn a_shown_metric_without_a_value_yet_reads_a_dash() {
    assert_eq!(
        hub_rows(&Overview::waiting(&Items::default())),
        ["CPU —", "Memory —", "Battery —", "Fan —", "Network —"]
    );
    let only_fan = Items {
        cpu: false,
        memory: false,
        battery: false,
        fan: true,
        network: false,
    };
    assert_eq!(hub_rows(&Overview::waiting(&only_fan)), ["Fan —"]);
}

#[test]
fn hub_rows_for_battery_fan_and_network_follow_cpu_and_memory() {
    let (battery, fan, reading) = (battery(), fan(), reading());
    let overview = Overview {
        cpu: Entry::Waiting,
        memory: Entry::Hidden,
        battery: Entry::Value(&battery),
        fan: Entry::Value(&fan),
        network: Entry::Value(&reading),
    };
    assert_eq!(
        hub_rows(&overview),
        [
            "CPU —",
            "Battery 76 % · Discharging",
            "Fan 6040 RPM",
            "Network ↓ 1.2 MB/s ↑ 34.0 KB/s",
        ]
    );
}

#[test]
fn entry_of_hides_waits_or_shows() {
    assert_eq!(Entry::of(false, Some(1)), Entry::Hidden);
    assert_eq!(Entry::<u8>::of(true, None), Entry::Waiting);
    assert_eq!(Entry::of(true, Some(1)), Entry::Value(1));
    assert_eq!(Entry::<u8>::waiting(false), Entry::Hidden);
    assert_eq!(Entry::<u8>::waiting(true), Entry::Waiting);
}

#[test]
fn battery_rows_show_every_known_figure_in_order() {
    assert_eq!(
        battery_rows(&battery()),
        [
            "Charge 76 %",
            "Status Discharging",
            "Time left 2h 05m",
            "Power 8.0 W",
            "Health 65 %",
            "Capacity 5803 / 8948 mAh",
            "Voltage 8.00 V",
            "Cycles 312",
            "Adapter not connected",
        ]
    );
}

#[test]
fn charging_shows_time_to_full_and_a_connected_adapter() {
    let charging = BatteryFigures {
        status: ChargeStatus::Charging,
        estimate: Some(Estimate::ToFull { minutes: 40 }),
        adapter_online: Some(true),
        ..battery()
    };
    let rows = battery_rows(&charging);
    assert_eq!(rows[1..3], ["Status Charging", "Time to full 0h 40m"]);
    assert_eq!(rows.last().map(String::as_str), Some("Adapter connected"));
}

#[test]
fn unknown_battery_figures_have_no_row() {
    let bare = BatteryFigures {
        percent: 76,
        status: ChargeStatus::NotCharging,
        estimate: None,
        power_watts: None,
        health_percent: None,
        full_mah: None,
        design_mah: Some(8948),
        voltage_volts: None,
        cycles: None,
        adapter_online: None,
    };
    assert_eq!(battery_rows(&bare), ["Charge 76 %", "Status Not charging"]);
}

#[test]
fn health_above_100_is_shown_as_it_is() {
    let new = BatteryFigures {
        health_percent: Some(103.4),
        ..battery()
    };
    assert!(battery_rows(&new).contains(&"Health 103 %".to_owned()));
}

#[test]
fn fan_rows_show_speed_and_max() {
    assert_eq!(fan_rows(&fan()), ["Speed 6040 RPM", "Max 5300 RPM"]);
}

#[test]
fn network_rows_show_interface_rates_and_session_totals() {
    assert_eq!(
        network_rows(Some("wlo1"), &reading()),
        [
            "Interface wlo1",
            "Download 1.2 MB/s",
            "Upload 34.0 KB/s",
            "Downloaded 1.2 GB",
            "Uploaded 300.0 MB",
        ]
    );
    assert_eq!(
        network_rows(None, &Reading::default())[0],
        "No active interface"
    );
}

#[test]
fn the_network_title_carries_both_rates() {
    assert_eq!(network_title(&reading()), "Network ↓ 1.2 MB/s ↑ 34.0 KB/s");
}

#[test]
fn titles_carry_the_value_when_there_is_one() {
    assert_eq!(title("CPU", Some(7)), "CPU 7 %");
    assert_eq!(title("Memory", None), "Memory");
}
