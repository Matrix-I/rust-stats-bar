//! What the daemon knows and decides, with no I/O: which items are shown, when each reader is
//! due, and whether a new sample changes a label. The event loop does the reading and drawing.

use statsbar_core::cpu::{CoreTicks, CpuLoad};
use statsbar_core::memory::{MemInfo, MemoryBuckets};
use statsbar_core::schedule::{self, ReaderId, ReaderState};

use crate::config::Config;
use crate::summary::percent;

/// A click in the hub's menu, sent from the `ksni` thread to the main loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    ToggleCpu,
    ToggleMemory,
    ToggleLaunchAtLogin,
    Quit,
}

pub struct App {
    pub config: Config,
    pub launch_at_login: bool,
    pub power_saver: bool,
    /// Indexed by `slot`.
    readers: [ReaderState; 2],
    failing: [bool; 2],
    cpu_previous: Option<Vec<CoreTicks>>,
    cpu: Option<CpuLoad>,
    cpu_label: Option<u8>,
    memory: Option<MemoryBuckets>,
    memory_label: Option<u8>,
}

fn slot(id: ReaderId) -> usize {
    match id {
        ReaderId::Cpu => 0,
        ReaderId::Memory => 1,
    }
}

fn fresh(id: ReaderId, visible: bool) -> ReaderState {
    ReaderState {
        id,
        visible,
        page_open: false,
        last_read_nanos: None,
    }
}

/// `Some(value)` when it differs from what the label shows, which is then updated.
fn changed(shown: &mut Option<u8>, value: u8) -> Option<u8> {
    if *shown == Some(value) {
        None
    } else {
        *shown = Some(value);
        Some(value)
    }
}

impl App {
    pub fn new(config: Config, launch_at_login: bool) -> App {
        App {
            readers: [
                fresh(ReaderId::Cpu, config.items.cpu),
                fresh(ReaderId::Memory, config.items.memory),
            ],
            config,
            launch_at_login,
            power_saver: false,
            failing: [false; 2],
            cpu_previous: None,
            cpu: None,
            cpu_label: None,
            memory: None,
            memory_label: None,
        }
    }

    pub fn is_visible(&self, id: ReaderId) -> bool {
        self.readers[slot(id)].visible
    }

    pub fn due(&self, now_nanos: u64) -> Vec<ReaderId> {
        schedule::due(now_nanos, self.power_saver, &self.readers)
    }

    pub fn next_wake_nanos(&self) -> Option<u64> {
        schedule::next_wake_nanos(self.power_saver, &self.readers)
    }

    /// Flips an item's visibility in the config and the schedule, and returns the new value.
    /// Hiding drops the item's samples, so an item shown again starts from a fresh reading and is
    /// due at once, instead of showing a stale value.
    pub fn toggle(&mut self, id: ReaderId) -> bool {
        let visible = !self.is_visible(id);
        match id {
            ReaderId::Cpu => {
                self.config.items.cpu = visible;
                self.cpu_previous = None;
                self.cpu = None;
                self.cpu_label = None;
            }
            ReaderId::Memory => {
                self.config.items.memory = visible;
                self.memory = None;
                self.memory_label = None;
            }
        }
        self.readers[slot(id)] = fresh(id, visible);
        self.failing[slot(id)] = false;
        visible
    }

    /// Takes a `/proc/stat` sample. Returns the new label value when it changed. The first sample,
    /// and one taken after a core went offline, only set the baseline.
    pub fn record_cpu(&mut self, now_nanos: u64, ticks: Vec<CoreTicks>) -> Option<u8> {
        self.mark_read(ReaderId::Cpu, now_nanos);
        let load = self
            .cpu_previous
            .as_deref()
            .and_then(|previous| CpuLoad::between(previous, &ticks));
        self.cpu_previous = Some(ticks);
        let load = load?;
        let value = percent(load.usage_percent());
        self.cpu = Some(load);
        changed(&mut self.cpu_label, value)
    }

    /// Takes a `/proc/meminfo` sample. Returns the new label value when it changed.
    pub fn record_memory(&mut self, now_nanos: u64, info: &MemInfo) -> Option<u8> {
        self.mark_read(ReaderId::Memory, now_nanos);
        let buckets = MemoryBuckets::from_meminfo(info);
        let value = percent(buckets.usage_percent());
        self.memory = Some(buckets);
        changed(&mut self.memory_label, value)
    }

    /// A failed read still counts as a read, so a broken file is retried each interval, not in a
    /// busy loop. Returns true for the first failure in a row, which is the one worth logging.
    pub fn record_error(&mut self, id: ReaderId, now_nanos: u64) -> bool {
        self.readers[slot(id)].last_read_nanos = Some(now_nanos);
        !std::mem::replace(&mut self.failing[slot(id)], true)
    }

    pub fn cpu(&self) -> Option<&CpuLoad> {
        self.cpu.as_ref()
    }

    pub fn memory(&self) -> Option<&MemoryBuckets> {
        self.memory.as_ref()
    }

    fn mark_read(&mut self, id: ReaderId, now_nanos: u64) {
        self.readers[slot(id)].last_read_nanos = Some(now_nanos);
        self.failing[slot(id)] = false;
    }
}
