//! What an open page changes: what is read, how often, and what hiding an item then means.

use statsbar_core::cpu::CoreTicks;
use statsbar_core::ipc::Page;
use statsbar_core::memory::MemInfo;
use statsbar_core::schedule::ReaderId::{self, Battery, Cpu, Fan, Memory, Network};
use statsbard::app::{App, READERS, page_of, reader_of};
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

fn ticks(user: u64, idle: u64) -> Vec<CoreTicks> {
    vec![CoreTicks {
        user,
        nice: 0,
        system: 0,
        idle,
    }]
}

fn half_used() -> MemInfo {
    MemInfo {
        total: 1000,
        available: 500,
        ..MemInfo::default()
    }
}

#[test]
fn each_page_has_its_reader() {
    assert_eq!(reader_of(Page::Cpu), Cpu);
    assert_eq!(reader_of(Page::Memory), Memory);
}

#[test]
fn a_hidden_items_page_is_read_at_once_then_every_second() {
    let mut app = only(&[]);
    assert_eq!(
        app.next_wake_nanos(),
        None,
        "nothing is read before it opens"
    );
    app.set_page_open(Cpu, true);
    assert_eq!(app.due(5 * SECOND), [Cpu]);
    app.record_cpu(5 * SECOND, ticks(0, 0));
    assert_eq!(app.next_wake_nanos(), Some(6 * SECOND));
    assert!(app.is_read(Cpu) && !app.is_visible(Cpu));
    assert!(!app.config.items.cpu, "the config is untouched");
}

#[test]
fn an_open_page_is_read_every_second_under_power_saver() {
    let mut app = only(&[Cpu, Memory]);
    app.power_saver = true;
    app.record_cpu(0, ticks(0, 0));
    app.record_memory(0, &half_used());
    app.set_page_open(Cpu, true);
    assert_eq!(app.due(SECOND), [Cpu]);
    assert_eq!(
        app.due(5 * SECOND),
        [Cpu, Memory],
        "the other item stays at 5 s"
    );
}

#[test]
fn opening_a_shown_items_page_keeps_its_baseline() {
    let mut app = only(&[Cpu]);
    app.record_cpu(0, ticks(0, 0));
    app.set_page_open(Cpu, true);
    assert_eq!(app.record_cpu(SECOND, ticks(25, 75)), Some(25));
}

#[test]
fn opening_an_open_page_again_changes_nothing() {
    let mut app = only(&[]);
    app.set_page_open(Cpu, true);
    app.record_cpu(0, ticks(0, 0));
    app.set_page_open(Cpu, true);
    assert!(app.due(SECOND - 1).is_empty());
    assert_eq!(
        app.record_cpu(SECOND, ticks(25, 75)),
        Some(25),
        "the baseline is kept"
    );
}

#[test]
fn hiding_an_item_with_its_page_open_keeps_reading_and_keeps_its_figures() {
    let mut app = only(&[Cpu]);
    app.record_cpu(0, ticks(0, 0));
    app.record_cpu(2 * SECOND, ticks(25, 75));
    app.set_page_open(Cpu, true);
    assert!(!app.toggle(Cpu));
    assert!(!app.config.items.cpu);
    assert!(app.is_read(Cpu));
    assert!(app.due(3 * SECOND - 1).is_empty(), "not reread at once");
    assert_eq!(app.due(3 * SECOND), [Cpu]);
    assert_eq!(app.cpu().map(|load| load.usage_percent()), Some(25.0));
    assert!(
        summary::hub_rows(&app.overview()).is_empty(),
        "no hub row while hidden"
    );
}

#[test]
fn showing_an_item_with_its_page_open_draws_its_label_on_the_next_sample() {
    let mut app = only(&[]);
    app.set_page_open(Cpu, true);
    app.record_cpu(0, ticks(0, 0));
    assert_eq!(app.record_cpu(SECOND, ticks(25, 75)), Some(25));
    assert!(app.toggle(Cpu));
    assert_eq!(
        app.record_cpu(2 * SECOND, ticks(50, 150)),
        Some(25),
        "the same value, on an item that is new and blank"
    );
}

#[test]
fn closing_a_hidden_items_page_stops_and_forgets() {
    let mut app = only(&[]);
    app.set_page_open(Cpu, true);
    app.record_cpu(0, ticks(0, 0));
    app.record_cpu(SECOND, ticks(25, 75));
    app.set_page_open(Cpu, false);
    assert!(!app.is_read(Cpu));
    assert_eq!(app.next_wake_nanos(), None);
    assert_eq!(app.cpu(), None);
    app.set_page_open(Cpu, true);
    assert_eq!(app.due(9 * SECOND), [Cpu]);
    assert_eq!(
        app.record_cpu(9 * SECOND, ticks(90, 910)),
        None,
        "a fresh baseline"
    );
}

#[test]
fn a_hidden_items_page_opened_again_waits_for_its_interval() {
    let mut app = only(&[]);
    app.set_page_open(Cpu, true);
    app.record_cpu(0, ticks(0, 0));
    app.set_page_open(Cpu, false);
    app.set_page_open(Cpu, true);
    assert!(
        app.due(SECOND - 1).is_empty(),
        "opening the page again is no reason to read"
    );
    assert_eq!(app.next_wake_nanos(), Some(SECOND));
    assert_eq!(app.due(SECOND), [Cpu]);
}

#[test]
fn closing_a_shown_items_page_returns_to_two_seconds() {
    let mut app = only(&[Memory]);
    app.set_page_open(Memory, true);
    app.record_memory(0, &half_used());
    app.set_page_open(Memory, false);
    assert!(app.is_read(Memory));
    assert!(app.memory().is_some());
    assert_eq!(app.next_wake_nanos(), Some(2 * SECOND));
}

#[test]
fn hiding_then_closing_stops_the_reads() {
    let mut app = only(&[Memory]);
    app.set_page_open(Memory, true);
    app.toggle(Memory);
    app.set_page_open(Memory, false);
    assert!(!app.is_read(Memory));
    assert_eq!(app.next_wake_nanos(), None);
}

#[test]
fn a_failure_under_an_open_page_is_retried_each_second_and_logged_afresh_after_closing() {
    let mut app = only(&[]);
    app.set_page_open(Memory, true);
    assert!(app.record_error(Memory, 0));
    assert!(!app.record_error(Memory, SECOND));
    assert_eq!(app.next_wake_nanos(), Some(2 * SECOND));
    app.set_page_open(Memory, false);
    app.set_page_open(Memory, true);
    assert!(app.record_error(Memory, 3 * SECOND));
}

#[test]
fn refresh_reads_now_once_half_a_second_has_passed() {
    let mut app = only(&[]);
    app.set_page_open(Cpu, true);
    app.record_cpu(0, ticks(0, 0));
    assert!(!app.refresh(Cpu, SECOND / 2 - 1));
    assert!(
        app.due(SECOND / 2 - 1).is_empty(),
        "too short an interval for a load"
    );
    assert!(app.refresh(Cpu, SECOND / 2));
    assert_eq!(app.due(SECOND / 2), [Cpu]);
}

#[test]
fn refresh_without_an_open_page_does_nothing() {
    let mut app = only(&[Cpu]);
    app.record_cpu(0, ticks(0, 0));
    assert!(!app.refresh(Cpu, SECOND));
    assert!(app.due(SECOND).is_empty());
}

#[test]
fn items_without_a_page_follow_their_visibility_alone() {
    let app = only(&[Battery]);
    assert!(app.is_read(Battery));
    assert!(!app.is_read(Fan) && !app.is_read(Network));
}

#[test]
fn only_cpu_and_memory_have_a_page() {
    assert_eq!(page_of(Cpu), Some(Page::Cpu));
    assert_eq!(page_of(Memory), Some(Page::Memory));
    for id in [Battery, Fan, Network] {
        assert_eq!(page_of(id), None, "{id:?}");
    }
}
