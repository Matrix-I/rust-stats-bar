//! The text of the tray menus and titles.

use statsbar_core::battery::{BatteryFigures, Estimate};
use statsbar_core::cpu::CpuLoad;
use statsbar_core::fan::FanSpeed;
use statsbar_core::format::{fmt_bytes, fmt_gb, fmt_minutes, fmt_rate, rounded_int};
use statsbar_core::memory::MemoryBuckets;
use statsbar_core::throughput::Reading;

use crate::config::Items;

/// A whole percent for a label or a row: rounded, then clamped to what the label can draw.
pub fn percent(value: f64) -> u8 {
    let clamped = rounded_int(value).clamp(0, 100);
    // In 0–100 after the clamp, so the conversion cannot fail.
    u8::try_from(clamped).unwrap_or(100)
}

pub fn cpu_rows(load: &CpuLoad) -> Vec<String> {
    vec![
        format!("Usage {} %", percent(load.usage_percent())),
        format!("User {} %", percent(load.user_percent)),
        format!("System {} %", percent(load.system_percent)),
        format!("Idle {} %", percent(load.idle_percent)),
        format!("Cores {}", load.per_core_busy.len()),
    ]
}

pub fn memory_rows(memory: &MemoryBuckets) -> Vec<String> {
    let mut rows = vec![
        format!(
            "Used {} of {} ({} %)",
            fmt_gb(memory.used()),
            fmt_gb(memory.total),
            percent(memory.usage_percent())
        ),
        format!("App {}", fmt_gb(memory.app)),
        format!("Kernel {}", fmt_gb(memory.kernel)),
        format!("Cached {}", fmt_gb(memory.cached)),
        format!("Free {}", fmt_gb(memory.free)),
        format!("Swap {}", fmt_gb(memory.swap_used)),
    ];
    if memory.compressed > 0 {
        rows.push(format!("Compressed {}", fmt_gb(memory.compressed)));
    }
    rows
}

/// A tray item's title: what screen readers and the item's tooltip use.
pub fn title(name: &str, percent: Option<u8>) -> String {
    match percent {
        Some(value) => format!("{name} {value} %"),
        None => name.to_owned(),
    }
}

pub fn battery_rows(battery: &BatteryFigures) -> Vec<String> {
    let mut rows = vec![
        format!("Charge {} %", battery.percent),
        format!("Status {}", battery.status.label()),
    ];
    match battery.estimate {
        Some(Estimate::ToEmpty { minutes }) => {
            rows.push(format!("Time left {}", fmt_minutes(whole_minutes(minutes))));
        }
        Some(Estimate::ToFull { minutes }) => {
            rows.push(format!(
                "Time to full {}",
                fmt_minutes(whole_minutes(minutes))
            ));
        }
        None => {}
    }
    if let Some(watts) = battery.power_watts {
        rows.push(format!("Power {watts:.1} W"));
    }
    if let Some(health) = battery.health_percent {
        rows.push(format!("Health {} %", rounded_int(health)));
    }
    if let (Some(full), Some(design)) = (battery.full_mah, battery.design_mah) {
        rows.push(format!("Capacity {full} / {design} mAh"));
    }
    if let Some(volts) = battery.voltage_volts {
        rows.push(format!("Voltage {volts:.2} V"));
    }
    if let Some(cycles) = battery.cycles {
        rows.push(format!("Cycles {cycles}"));
    }
    match battery.adapter_online {
        Some(true) => rows.push("Adapter connected".to_owned()),
        Some(false) => rows.push("Adapter not connected".to_owned()),
        None => {}
    }
    rows
}

/// Estimates are at most a day (`battery::MAX_ESTIMATE_MINUTES`), so this never saturates.
fn whole_minutes(minutes: u64) -> i64 {
    i64::try_from(minutes).unwrap_or(i64::MAX)
}

pub fn fan_rows(fan: &FanSpeed) -> Vec<String> {
    vec![
        format!("Speed {} RPM", fan.rpm),
        format!("Max {} RPM", fan.max_rpm),
    ]
}

/// The totals count from when the item was shown or the interface changed.
pub fn network_rows(interface: Option<&str>, reading: &Reading) -> Vec<String> {
    vec![
        match interface {
            Some(name) => format!("Interface {name}"),
            None => "No active interface".to_owned(),
        },
        format!("Download {}", fmt_rate(reading.download_rate)),
        format!("Upload {}", fmt_rate(reading.upload_rate)),
        format!("Downloaded {}", fmt_bytes(reading.download_total)),
        format!("Uploaded {}", fmt_bytes(reading.upload_total)),
    ]
}

/// The network item's title and its hub row.
pub fn network_title(reading: &Reading) -> String {
    format!(
        "Network ↓ {} ↑ {}",
        fmt_rate(reading.download_rate),
        fmt_rate(reading.upload_rate)
    )
}

/// What the hub shows for one metric.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Entry<T> {
    /// The item is hidden, so the metric is not read and has no row.
    Hidden,
    /// Shown, but not read yet: the row reads `CPU —`.
    Waiting,
    Value(T),
}

impl<T> Entry<T> {
    pub fn waiting(shown: bool) -> Entry<T> {
        Entry::of(shown, None)
    }

    pub fn of(shown: bool, value: Option<T>) -> Entry<T> {
        match (shown, value) {
            (false, _) => Entry::Hidden,
            (true, None) => Entry::Waiting,
            (true, Some(value)) => Entry::Value(value),
        }
    }
}

/// The hub's overview, one entry per metric in hub order.
#[derive(Debug, Clone, Copy)]
pub struct Overview<'a> {
    pub cpu: Entry<&'a CpuLoad>,
    pub memory: Entry<&'a MemoryBuckets>,
    pub battery: Entry<&'a BatteryFigures>,
    pub fan: Entry<&'a FanSpeed>,
    pub network: Entry<&'a Reading>,
}

impl Overview<'_> {
    /// Every shown metric waiting for its first value: what a new hub starts with.
    pub fn waiting(items: &Items) -> Overview<'static> {
        Overview {
            cpu: Entry::waiting(items.cpu),
            memory: Entry::waiting(items.memory),
            battery: Entry::waiting(items.battery),
            fan: Entry::waiting(items.fan),
            network: Entry::waiting(items.network),
        }
    }
}

/// The hub's rows. A hidden metric is not read, so it has no row.
pub fn hub_rows(overview: &Overview) -> Vec<String> {
    let mut rows = Vec::with_capacity(5);
    push(&mut rows, "CPU", overview.cpu, |load| {
        format!("CPU {} %", percent(load.usage_percent()))
    });
    push(&mut rows, "Memory", overview.memory, |memory| {
        format!(
            "Memory {} % · {} / {}",
            percent(memory.usage_percent()),
            fmt_gb(memory.used()),
            fmt_gb(memory.total)
        )
    });
    push(&mut rows, "Battery", overview.battery, |battery| {
        format!("Battery {} % · {}", battery.percent, battery.status.label())
    });
    push(&mut rows, "Fan", overview.fan, |fan| {
        format!("Fan {} RPM", fan.rpm)
    });
    push(&mut rows, "Network", overview.network, network_title);
    rows
}

fn push<T>(rows: &mut Vec<String>, name: &str, entry: Entry<T>, text: impl FnOnce(T) -> String) {
    match entry {
        Entry::Hidden => {}
        Entry::Waiting => rows.push(format!("{name} —")),
        Entry::Value(value) => rows.push(text(value)),
    }
}
