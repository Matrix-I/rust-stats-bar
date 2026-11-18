//! The text of the tray menus and titles.

use statsbar_core::cpu::CpuLoad;
use statsbar_core::format::{fmt_gb, rounded_int};
use statsbar_core::memory::MemoryBuckets;

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

/// The hub's overview. A hidden metric is not read, so it has no row.
pub fn hub_rows(
    cpu: Option<&CpuLoad>,
    memory: Option<&MemoryBuckets>,
    show_cpu: bool,
    show_memory: bool,
) -> Vec<String> {
    let mut rows = Vec::with_capacity(2);
    if show_cpu {
        rows.push(match cpu {
            Some(load) => format!("CPU {} %", percent(load.usage_percent())),
            None => "CPU —".to_owned(),
        });
    }
    if show_memory {
        rows.push(match memory {
            Some(memory) => format!(
                "Memory {} % · {} / {}",
                percent(memory.usage_percent()),
                fmt_gb(memory.used()),
                fmt_gb(memory.total)
            ),
            None => "Memory —".to_owned(),
        });
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
