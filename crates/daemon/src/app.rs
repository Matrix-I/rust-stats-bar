//! What the daemon knows and decides, with no I/O: which items are shown, when each reader is
//! due, and whether a new sample changes a label. The event loop does the reading and drawing.

use statsbar_core::battery::{BatteryFigures, BatteryInfo, ChargeStatus};
use statsbar_core::cpu::{CoreTicks, CpuLoad};
use statsbar_core::fan::{self, FanSpeed};
use statsbar_core::label::{self, Metric};
use statsbar_core::memory::{MemInfo, MemoryBuckets};
use statsbar_core::schedule::{self, ReaderId, ReaderState};
use statsbar_core::throughput::{Reading, Sample, ThroughputTracker};

use crate::config::Config;
use crate::summary::{self, Entry, Overview, percent};

/// Every metric, in the order of the hub's rows and switches and of the items' first spawn.
pub const READERS: [ReaderId; 5] = [
    ReaderId::Cpu,
    ReaderId::Memory,
    ReaderId::Battery,
    ReaderId::Fan,
    ReaderId::Network,
];

/// A click in the hub's menu, sent from the `ksni` thread to the main loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Toggle(ReaderId),
    ToggleLaunchAtLogin,
    Quit,
}

/// The network label's texts: upload, then download.
pub type RateLabel = (String, String);

/// The network figures of the last sample.
#[derive(Debug, Clone, PartialEq)]
pub struct NetworkStatus {
    /// `None` when there was no default route.
    pub interface: Option<String>,
    pub reading: Reading,
}

pub struct App {
    pub config: Config,
    pub launch_at_login: bool,
    pub power_saver: bool,
    /// Indexed by `slot`.
    readers: [ReaderState; 5],
    failing: [bool; 5],
    cpu_previous: Option<Vec<CoreTicks>>,
    cpu: Option<CpuLoad>,
    cpu_label: Option<u8>,
    memory: Option<MemoryBuckets>,
    memory_label: Option<u8>,
    battery: Option<BatteryFigures>,
    battery_label: Option<(Metric, u8)>,
    fan: Option<FanSpeed>,
    fan_label: Option<u8>,
    throughput: ThroughputTracker,
    network: Option<NetworkStatus>,
    /// The label's texts and the item's title, as last drawn.
    network_label: Option<(RateLabel, String)>,
}

/// Where `id` sits in `READERS`, and in every per-reader array.
pub fn slot(id: ReaderId) -> usize {
    match id {
        ReaderId::Cpu => 0,
        ReaderId::Memory => 1,
        ReaderId::Battery => 2,
        ReaderId::Fan => 3,
        ReaderId::Network => 4,
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
fn changed<T: Clone + PartialEq>(shown: &mut Option<T>, value: T) -> Option<T> {
    if shown.as_ref() == Some(&value) {
        None
    } else {
        *shown = Some(value.clone());
        Some(value)
    }
}

impl App {
    pub fn new(config: Config, launch_at_login: bool) -> App {
        App {
            readers: READERS.map(|id| fresh(id, config.items.shown(id))),
            config,
            launch_at_login,
            power_saver: false,
            failing: [false; 5],
            cpu_previous: None,
            cpu: None,
            cpu_label: None,
            memory: None,
            memory_label: None,
            battery: None,
            battery_label: None,
            fan: None,
            fan_label: None,
            throughput: ThroughputTracker::default(),
            network: None,
            network_label: None,
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
        self.config.items.set_shown(id, visible);
        self.forget(id);
        self.readers[slot(id)] = fresh(id, visible);
        self.failing[slot(id)] = false;
        visible
    }

    fn forget(&mut self, id: ReaderId) {
        match id {
            ReaderId::Cpu => {
                self.cpu_previous = None;
                self.cpu = None;
                self.cpu_label = None;
            }
            ReaderId::Memory => {
                self.memory = None;
                self.memory_label = None;
            }
            ReaderId::Battery => {
                self.battery = None;
                self.battery_label = None;
            }
            ReaderId::Fan => {
                self.fan = None;
                self.fan_label = None;
            }
            ReaderId::Network => {
                // A new session: the totals count from when the item is shown again.
                self.throughput = ThroughputTracker::default();
                self.network = None;
                self.network_label = None;
            }
        }
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

    /// Takes a battery sample. Returns the new label when its word or its percent changed: `CHG`
    /// while charging and `BAT` otherwise (requirement 2).
    pub fn record_battery(
        &mut self,
        now_nanos: u64,
        info: &BatteryInfo,
        adapter_online: Option<bool>,
    ) -> Option<(Metric, u8)> {
        self.mark_read(ReaderId::Battery, now_nanos);
        let figures = BatteryFigures::new(info, adapter_online);
        let word = if figures.status == ChargeStatus::Charging {
            Metric::Chg
        } else {
            Metric::Bat
        };
        self.battery = Some(figures);
        changed(&mut self.battery_label, (word, figures.percent))
    }

    /// Takes a fan sample. Returns the new label value, its share of the fan's range clamped at
    /// 100 (requirement 3), when it changed. A fan whose range is empty has rows but no label.
    pub fn record_fan(&mut self, now_nanos: u64, speed: FanSpeed) -> Option<u8> {
        self.mark_read(ReaderId::Fan, now_nanos);
        self.fan = Some(speed);
        let value = percent(fan::range_fraction(&speed)? * 100.0);
        changed(&mut self.fan_label, value)
    }

    /// Takes a network sample, `None` when there is no default route. Returns the label's texts
    /// when they or the item's title changed. The title has finer rates than the label, and it
    /// must stay as current as the hub row that repeats it.
    pub fn record_network(&mut self, now_nanos: u64, sample: Option<&Sample>) -> Option<RateLabel> {
        self.mark_read(ReaderId::Network, now_nanos);
        let reading = self.throughput.update(sample, now_nanos);
        self.network = Some(NetworkStatus {
            interface: sample.map(|sample| sample.interface.clone()),
            reading,
        });
        let texts = (
            label::rate_text(reading.upload_rate),
            label::rate_text(reading.download_rate),
        );
        changed(
            &mut self.network_label,
            (texts, summary::network_title(&reading)),
        )
        .map(|(texts, _)| texts)
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

    pub fn battery(&self) -> Option<&BatteryFigures> {
        self.battery.as_ref()
    }

    pub fn fan(&self) -> Option<&FanSpeed> {
        self.fan.as_ref()
    }

    pub fn network(&self) -> Option<&NetworkStatus> {
        self.network.as_ref()
    }

    /// What the hub shows: each metric hidden, waiting for its first value, or with it.
    pub fn overview(&self) -> Overview<'_> {
        let items = &self.config.items;
        Overview {
            cpu: Entry::of(items.cpu, self.cpu()),
            memory: Entry::of(items.memory, self.memory()),
            battery: Entry::of(items.battery, self.battery()),
            fan: Entry::of(items.fan, self.fan()),
            network: Entry::of(items.network, self.network().map(|status| &status.reading)),
        }
    }

    /// A metric item's menu rows, empty before its first value.
    pub fn rows(&self, id: ReaderId) -> Vec<String> {
        match id {
            ReaderId::Cpu => self.cpu().map(summary::cpu_rows),
            ReaderId::Memory => self.memory().map(summary::memory_rows),
            ReaderId::Battery => self.battery().map(summary::battery_rows),
            ReaderId::Fan => self.fan().map(summary::fan_rows),
            ReaderId::Network => self
                .network()
                .map(|status| summary::network_rows(status.interface.as_deref(), &status.reading)),
        }
        .unwrap_or_default()
    }

    fn mark_read(&mut self, id: ReaderId, now_nanos: u64) {
        self.readers[slot(id)].last_read_nanos = Some(now_nanos);
        self.failing[slot(id)] = false;
    }
}
