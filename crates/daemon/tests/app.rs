use statsbar_core::cpu::CoreTicks;
use statsbar_core::memory::MemInfo;
use statsbar_core::schedule::ReaderId::{Cpu, Memory};
use statsbard::app::App;
use statsbard::config::{Config, Items};

const SECOND: u64 = 1_000_000_000;

fn both_shown() -> App {
    App::new(Config::default(), false)
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
            },
        },
        false,
    );
    assert!(!app.is_visible(Cpu));
    assert_eq!(app.due(0), vec![Memory]);
}

#[test]
fn hiding_an_item_stops_its_reads_and_updates_the_config() {
    let mut app = both_shown();
    assert!(!app.toggle(Cpu));
    assert!(!app.config.items.cpu);
    assert_eq!(app.due(100 * SECOND), vec![Memory]);
}

#[test]
fn showing_an_item_again_reads_it_at_once_from_a_fresh_sample() {
    let mut app = both_shown();
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
    let mut app = both_shown();
    assert_eq!(app.record_cpu(0, ticks(0, 0)), None);
    assert_eq!(app.record_cpu(2 * SECOND, ticks(25, 75)), Some(25));
    assert_eq!(app.record_cpu(4 * SECOND, ticks(50, 150)), None);
    assert_eq!(app.record_cpu(6 * SECOND, ticks(100, 150)), Some(100));
}

#[test]
fn a_core_count_change_skips_one_label_update() {
    let mut app = both_shown();
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
    let mut app = both_shown();
    assert_eq!(app.record_memory(0, &memory(1000, 600)), Some(40));
    assert_eq!(app.record_memory(2 * SECOND, &memory(1000, 600)), None);
    assert_eq!(app.record_memory(4 * SECOND, &memory(1000, 500)), Some(50));
    assert_eq!(app.memory().map(|m| m.used()), Some(500));
}

#[test]
fn a_read_counts_as_a_read_for_the_schedule() {
    let mut app = both_shown();
    app.record_memory(10 * SECOND, &memory(1000, 600));
    assert_eq!(app.due(10 * SECOND), vec![Cpu]);
    assert_eq!(app.due(12 * SECOND), vec![Cpu, Memory]);
}

#[test]
fn a_failed_read_waits_one_interval_and_is_reported_once_per_run() {
    let mut app = both_shown();
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
    let mut app = both_shown();
    app.record_cpu(0, ticks(0, 0));
    app.record_memory(0, &memory(1000, 600));
    assert_eq!(app.next_wake_nanos(), Some(2 * SECOND));
    app.power_saver = true;
    assert_eq!(app.next_wake_nanos(), Some(5 * SECOND));
}
