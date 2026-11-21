use statsbar_core::battery::{BatteryInfo, ChargeStatus};
use statsbar_core::cpu::CoreTicks;
use statsbar_core::fan::FanSpeed;
use statsbar_core::memory::MemInfo;
use statsbar_core::schedule::ReaderId::{self, Battery, Cpu, Fan, Memory, Network};
use statsbar_core::throughput::Sample;
use statsbard::app::{App, BatteryLabel, READERS};
use statsbard::config::{Config, Items};
use statsbard::summary;

const SECOND: u64 = 1_000_000_000;

fn only(shown: &[ReaderId]) -> App {
    let mut items = Items::default();
    for id in READERS {
        items.set_shown(id, shown.contains(&id));
    }
    App::new(Config { items }, false)
}

fn cpu_and_memory_shown() -> App {
    only(&[Cpu, Memory])
}

fn battery(status: ChargeStatus, capacity_percent: u8) -> BatteryInfo {
    BatteryInfo {
        status,
        capacity_percent,
        charge_now_uah: None,
        charge_full_uah: None,
        charge_full_design_uah: None,
        current_now_ua: None,
        voltage_now_uv: None,
        cycle_count: None,
    }
}

fn fan(rpm: u64) -> FanSpeed {
    FanSpeed {
        rpm,
        min_rpm: 0,
        max_rpm: 5300,
    }
}

fn wlo1(rx_bytes: u64, tx_bytes: u64) -> Sample {
    Sample {
        interface: "wlo1".to_owned(),
        rx_bytes,
        tx_bytes,
    }
}

fn download(text: &str) -> Option<String> {
    Some(text.to_owned())
}

fn battery_label(percent: u8, charging: bool) -> Option<BatteryLabel> {
    Some(BatteryLabel { percent, charging })
}

fn ticks(user: u64, idle: u64) -> Vec<CoreTicks> {
    vec![CoreTicks {
        user,
        nice: 0,
        system: 0,
        idle,
    }]
}

fn memory(total: u64, available: u64) -> MemInfo {
    MemInfo {
        total,
        available,
        ..MemInfo::default()
    }
}

#[test]
fn a_hidden_item_is_never_read() {
    let app = App::new(
        Config {
            items: Items {
                cpu: false,
                memory: true,
                battery: false,
                fan: false,
                network: false,
            },
        },
        false,
    );
    assert!(!app.is_visible(Cpu));
    assert_eq!(app.due(0), vec![Memory]);
}

#[test]
fn hiding_an_item_stops_its_reads_and_updates_the_config() {
    let mut app = cpu_and_memory_shown();
    assert!(!app.toggle(Cpu));
    assert!(!app.config.items.cpu);
    assert_eq!(app.due(100 * SECOND), vec![Memory]);
}

#[test]
fn showing_an_item_again_reads_it_at_once_from_a_fresh_sample() {
    let mut app = cpu_and_memory_shown();
    app.record_cpu(0, ticks(0, 0));
    app.record_cpu(2 * SECOND, ticks(25, 75));
    app.toggle(Cpu);
    assert!(app.toggle(Cpu));
    assert!(app.config.items.cpu);
    assert_eq!(app.cpu(), None);
    assert!(app.due(2 * SECOND).contains(&Cpu));
    assert_eq!(
        app.record_cpu(2 * SECOND, ticks(50, 150)),
        None,
        "the first sample after showing again has nothing to compare with"
    );
}

#[test]
fn the_cpu_label_needs_two_samples_and_changes_only_with_its_value() {
    let mut app = cpu_and_memory_shown();
    assert_eq!(app.record_cpu(0, ticks(0, 0)), None);
    assert_eq!(app.record_cpu(2 * SECOND, ticks(25, 75)), Some(25));
    assert_eq!(app.record_cpu(4 * SECOND, ticks(50, 150)), None);
    assert_eq!(app.record_cpu(6 * SECOND, ticks(100, 150)), Some(100));
}

#[test]
fn a_core_count_change_skips_one_label_update() {
    let mut app = cpu_and_memory_shown();
    app.record_cpu(0, ticks(0, 0));
    let two = |n| {
        vec![
            CoreTicks {
                user: n,
                nice: 0,
                system: 0,
                idle: n
            };
            2
        ]
    };
    assert_eq!(app.record_cpu(2 * SECOND, two(10)), None);
    assert_eq!(app.record_cpu(4 * SECOND, two(20)), Some(50));
}

#[test]
fn the_memory_label_changes_only_with_its_value() {
    let mut app = cpu_and_memory_shown();
    assert_eq!(app.record_memory(0, &memory(1000, 600)), Some(40));
    assert_eq!(app.record_memory(2 * SECOND, &memory(1000, 600)), None);
    assert_eq!(app.record_memory(4 * SECOND, &memory(1000, 500)), Some(50));
    assert_eq!(app.memory().map(|m| m.used()), Some(500));
}

#[test]
fn a_read_counts_as_a_read_for_the_schedule() {
    let mut app = cpu_and_memory_shown();
    app.record_memory(10 * SECOND, &memory(1000, 600));
    assert_eq!(app.due(10 * SECOND), vec![Cpu]);
    assert_eq!(app.due(12 * SECOND), vec![Cpu, Memory]);
}

#[test]
fn a_failed_read_waits_one_interval_and_is_reported_once_per_run() {
    let mut app = cpu_and_memory_shown();
    assert!(app.record_error(Cpu, 0), "first failure is reported");
    assert!(!app.due(SECOND).contains(&Cpu), "no busy retry");
    assert!(!app.record_error(Cpu, 2 * SECOND), "second in a row is not");
    app.record_cpu(4 * SECOND, ticks(0, 0));
    assert!(
        app.record_error(Cpu, 6 * SECOND),
        "a new run of failures is"
    );
}

#[test]
fn power_saver_moves_the_next_wake_to_five_seconds() {
    let mut app = cpu_and_memory_shown();
    app.record_cpu(0, ticks(0, 0));
    app.record_memory(0, &memory(1000, 600));
    assert_eq!(app.next_wake_nanos(), Some(2 * SECOND));
    app.power_saver = true;
    assert_eq!(app.next_wake_nanos(), Some(5 * SECOND));
}

#[test]
fn every_item_is_due_at_start_in_hub_order() {
    let app = App::new(Config::default(), false);
    assert_eq!(app.due(0), READERS.to_vec());
    assert_eq!(READERS, [Cpu, Memory, Battery, Fan, Network]);
}

#[test]
fn the_battery_label_has_the_bolt_only_while_charging() {
    let mut app = only(&[Battery]);
    use ChargeStatus::{Charging, Discharging, NotCharging};
    assert_eq!(
        app.record_battery(0, &battery(Discharging, 76), Some(false)),
        battery_label(76, false)
    );
    assert_eq!(
        app.record_battery(2 * SECOND, &battery(Discharging, 76), Some(false)),
        None
    );
    assert_eq!(
        app.record_battery(4 * SECOND, &battery(Charging, 76), Some(true)),
        battery_label(76, true)
    );
    assert_eq!(
        app.record_battery(6 * SECOND, &battery(NotCharging, 76), Some(true)),
        battery_label(76, false)
    );
    assert_eq!(
        app.record_battery(8 * SECOND, &battery(NotCharging, 76), Some(true)),
        None,
        "Not charging and Discharging look the same, so the status alone redraws nothing"
    );
    assert_eq!(
        app.record_battery(10 * SECOND, &battery(Discharging, 76), Some(false)),
        None
    );
    assert_eq!(
        app.record_battery(12 * SECOND, &battery(NotCharging, 77), Some(true)),
        battery_label(77, false)
    );
    assert_eq!(
        app.battery().map(|figures| figures.adapter_online),
        Some(Some(true))
    );
}

#[test]
fn the_fan_label_is_its_share_of_the_range_clamped_at_100() {
    let mut app = only(&[Fan]);
    assert_eq!(app.record_fan(0, fan(2650)), Some(50));
    assert_eq!(app.record_fan(2 * SECOND, fan(2660)), None);
    assert_eq!(app.record_fan(4 * SECOND, fan(6040)), Some(100));
    assert_eq!(app.fan().map(|speed| speed.rpm), Some(6040));
}

#[test]
fn a_fan_without_a_range_keeps_its_rows_but_draws_no_label() {
    let mut app = only(&[Fan]);
    let no_range = FanSpeed {
        rpm: 3000,
        min_rpm: 0,
        max_rpm: 0,
    };
    assert_eq!(app.record_fan(0, no_range), None);
    assert_eq!(app.rows(Fan), ["Speed 3000 RPM", "Max 0 RPM"]);
    assert!(
        !app.due(SECOND).contains(&Fan),
        "the sample still counts as a read"
    );
}

#[test]
fn the_network_label_follows_its_download_text() {
    let mut app = only(&[Network]);
    assert_eq!(app.record_network(0, Some(&wlo1(0, 0))), download("0B"));
    assert_eq!(
        app.record_network(2 * SECOND, Some(&wlo1(24_000, 2_000))),
        download("12K")
    );
    assert_eq!(
        app.record_network(4 * SECOND, Some(&wlo1(48_000, 4_000))),
        None
    );
    assert_eq!(app.rows(Network)[0], "Interface wlo1");
}

#[test]
fn an_upload_change_redraws_the_title_but_keeps_the_download_label() {
    let mut app = only(&[Network]);
    app.record_network(0, Some(&wlo1(0, 0)));
    app.record_network(2 * SECOND, Some(&wlo1(24_000, 2_000)));
    // Upload goes from 1.0 to 2.0 KB/s; download stays at 12 KB/s.
    assert_eq!(
        app.record_network(4 * SECOND, Some(&wlo1(48_000, 6_000))),
        download("12K")
    );
    assert_eq!(
        summary::network_title(&app.network().expect("sampled").reading),
        "Network ↓ 12.0 KB/s ↑ 2.0 KB/s"
    );
}

#[test]
fn a_rate_change_too_small_for_the_label_still_redraws_the_title() {
    let mut app = only(&[Network]);
    app.record_network(0, Some(&wlo1(0, 0)));
    app.record_network(2 * SECOND, Some(&wlo1(24_000, 2_000)));
    // 12.1 KB/s still draws "12K", but the title reads "↓ 12.1 KB/s" instead of "↓ 12.0 KB/s".
    assert_eq!(
        app.record_network(4 * SECOND, Some(&wlo1(48_200, 4_000))),
        download("12K")
    );
    assert_eq!(
        summary::network_title(&app.network().expect("sampled").reading),
        "Network ↓ 12.1 KB/s ↑ 1.0 KB/s"
    );
}

#[test]
fn no_default_route_reads_as_no_interface_and_zero_rates() {
    let mut app = only(&[Network]);
    app.record_network(0, Some(&wlo1(0, 0)));
    app.record_network(2 * SECOND, Some(&wlo1(24_000, 0)));
    assert_eq!(app.record_network(4 * SECOND, None), download("0B"));
    assert_eq!(
        app.rows(Network),
        [
            "No active interface",
            "Download 0 bytes/s",
            "Upload 0 bytes/s",
            "Downloaded 24.0 KB",
            "Uploaded 0 bytes",
        ]
    );
}

#[test]
fn showing_the_network_again_starts_a_new_session() {
    let mut app = only(&[Network]);
    app.record_network(0, Some(&wlo1(0, 0)));
    app.record_network(2 * SECOND, Some(&wlo1(24_000, 0)));
    app.toggle(Network);
    assert!(app.toggle(Network));
    assert_eq!(app.network(), None);
    app.record_network(4 * SECOND, Some(&wlo1(30_000, 0)));
    assert_eq!(app.rows(Network)[3], "Downloaded 0 bytes");
}

#[test]
fn hiding_battery_fan_or_network_stops_its_reads_and_updates_the_config() {
    for id in [Battery, Fan, Network] {
        let mut app = App::new(Config::default(), false);
        assert!(!app.toggle(id), "{id:?}");
        assert!(!app.config.items.shown(id), "{id:?}");
        assert!(!app.due(100 * SECOND).contains(&id), "{id:?}");
    }
}

#[test]
fn hiding_the_battery_drops_its_figures_and_label() {
    let mut app = only(&[Battery]);
    app.record_battery(0, &battery(ChargeStatus::Discharging, 76), None);
    app.toggle(Battery);
    assert!(app.toggle(Battery));
    assert_eq!(app.battery(), None);
    assert_eq!(
        app.record_battery(2 * SECOND, &battery(ChargeStatus::Discharging, 76), None),
        battery_label(76, false),
        "shown again, the same value is drawn again"
    );
}

#[test]
fn the_overview_has_a_dash_for_shown_items_without_a_value() {
    let mut app = App::new(Config::default(), false);
    app.toggle(Fan);
    app.record_battery(0, &battery(ChargeStatus::Discharging, 76), None);
    assert_eq!(
        summary::hub_rows(&app.overview()),
        [
            "CPU —",
            "Memory —",
            "Battery 76 % · Discharging",
            "Network —"
        ]
    );
}

#[test]
fn rows_are_empty_before_the_first_value() {
    let app = App::new(Config::default(), false);
    for id in READERS {
        assert!(app.rows(id).is_empty(), "{id:?}");
    }
}
