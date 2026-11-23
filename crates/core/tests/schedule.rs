use statsbar_core::schedule::{ReaderId, ReaderState, due, interval_nanos, next_wake_nanos};

const SECOND: u64 = 1_000_000_000;

fn reader(id: ReaderId, visible: bool, page_open: bool, last: Option<u64>) -> ReaderState {
    ReaderState {
        id,
        visible,
        page_open,
        last_read_nanos: last,
    }
}

#[test]
fn intervals_follow_the_roadmap_cadence_table() {
    // (visible, page open, power saver, interval)
    let cases = [
        (false, false, false, None),
        (false, false, true, None),
        (true, false, false, Some(2 * SECOND)),
        (true, true, false, Some(SECOND)),
        (false, true, false, Some(SECOND)),
        (true, false, true, Some(5 * SECOND)),
        (false, true, true, Some(SECOND)),
        (true, true, true, Some(SECOND)),
    ];
    for (visible, page_open, power_saver, expected) in cases {
        let state = reader(ReaderId::Cpu, visible, page_open, None);
        assert_eq!(
            interval_nanos(&state, power_saver),
            expected,
            "visible {visible}, page open {page_open}, power saver {power_saver}"
        );
    }
}

#[test]
fn a_reader_never_read_is_due_at_once() {
    let readers = [reader(ReaderId::Cpu, true, false, None)];
    assert_eq!(due(0, false, &readers), vec![ReaderId::Cpu]);
    assert_eq!(next_wake_nanos(false, &readers), Some(0));
}

#[test]
fn a_visible_reader_is_due_exactly_one_interval_after_its_last_read() {
    let readers = [reader(ReaderId::Memory, true, false, Some(10 * SECOND))];
    assert!(due(12 * SECOND - 1, false, &readers).is_empty());
    assert_eq!(due(12 * SECOND, false, &readers), vec![ReaderId::Memory]);
}

#[test]
fn a_hidden_reader_with_its_page_closed_is_never_due() {
    let readers = [reader(ReaderId::Cpu, false, false, None)];
    assert!(due(u64::MAX, false, &readers).is_empty());
    assert_eq!(next_wake_nanos(false, &readers), None);
}

#[test]
fn power_saver_stretches_the_interval_to_five_seconds() {
    let readers = [reader(ReaderId::Cpu, true, false, Some(0))];
    assert!(due(5 * SECOND - 1, true, &readers).is_empty());
    assert_eq!(due(5 * SECOND, true, &readers), vec![ReaderId::Cpu]);
}

#[test]
fn due_readers_keep_their_input_order() {
    let readers = [
        reader(ReaderId::Memory, true, false, None),
        reader(ReaderId::Cpu, true, false, None),
    ];
    assert_eq!(
        due(0, false, &readers),
        vec![ReaderId::Memory, ReaderId::Cpu]
    );
}

#[test]
fn next_wake_is_the_earliest_due_time() {
    let readers = [
        reader(ReaderId::Cpu, true, false, Some(3 * SECOND)),
        reader(ReaderId::Memory, true, true, Some(3 * SECOND)),
    ];
    assert_eq!(next_wake_nanos(false, &readers), Some(4 * SECOND));
}

#[test]
fn a_last_read_near_the_end_of_time_saturates_instead_of_wrapping() {
    let readers = [reader(ReaderId::Cpu, true, false, Some(u64::MAX - 1))];
    assert!(due(u64::MAX - 1, false, &readers).is_empty());
    assert_eq!(next_wake_nanos(false, &readers), Some(u64::MAX));
}
