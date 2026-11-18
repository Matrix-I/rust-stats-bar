//! When each reader is due. One timer serves every reader, and a metric nobody is looking at is
//! never scheduled. Times are monotonic nanoseconds passed in by the caller.

const NANOS_PER_SECOND: u64 = 1_000_000_000;
/// Item visible, its page closed.
pub const VISIBLE_INTERVAL_NANOS: u64 = 2 * NANOS_PER_SECOND;
/// That metric's page is open in the panel.
pub const PAGE_OPEN_INTERVAL_NANOS: u64 = NANOS_PER_SECOND;
/// The `power-saver` profile is active. Upstream's CPU reader lets Low Power Mode win over an
/// open page, and so does this.
pub const POWER_SAVER_INTERVAL_NANOS: u64 = 5 * NANOS_PER_SECOND;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReaderId {
    Cpu,
    Memory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReaderState {
    pub id: ReaderId,
    /// Its tray item is shown.
    pub visible: bool,
    /// Its page is open in the panel.
    pub page_open: bool,
    /// When it was last read, successfully or not. `None` before the first read.
    pub last_read_nanos: Option<u64>,
}

/// How often a reader is read, or `None` when nothing shows its value.
pub fn interval_nanos(state: &ReaderState, power_saver: bool) -> Option<u64> {
    if !state.visible && !state.page_open {
        return None;
    }
    Some(if power_saver {
        POWER_SAVER_INTERVAL_NANOS
    } else if state.page_open {
        PAGE_OPEN_INTERVAL_NANOS
    } else {
        VISIBLE_INTERVAL_NANOS
    })
}

/// When `state` is next due, or `None` when it is not scheduled. Saturating, so a time near the
/// end of `u64` cannot wrap round to "due long ago".
fn due_at(state: &ReaderState, power_saver: bool) -> Option<u64> {
    let interval = interval_nanos(state, power_saver)?;
    Some(
        state
            .last_read_nanos
            .map_or(0, |last| last.saturating_add(interval)),
    )
}

/// The readers due at `now_nanos`, in the order given.
pub fn due(now_nanos: u64, power_saver: bool, readers: &[ReaderState]) -> Vec<ReaderId> {
    readers
        .iter()
        .filter(|state| due_at(state, power_saver).is_some_and(|at| now_nanos >= at))
        .map(|state| state.id)
        .collect()
}

/// When the loop should next wake, or `None` when no reader is scheduled and only a menu click or
/// a profile change can wake it.
pub fn next_wake_nanos(power_saver: bool, readers: &[ReaderState]) -> Option<u64> {
    readers
        .iter()
        .filter_map(|state| due_at(state, power_saver))
        .min()
}
