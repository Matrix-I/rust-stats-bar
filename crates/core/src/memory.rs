//! Memory: `/proc/meminfo` text in, the buckets of the memory bar out.

use serde::{Deserialize, Serialize};

/// The `/proc/meminfo` fields the buckets are built from, in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MemInfo {
    pub total: u64,
    pub free: u64,
    pub available: u64,
    pub s_unreclaim: u64,
    pub kernel_stack: u64,
    pub page_tables: u64,
    pub swap_total: u64,
    pub swap_free: u64,
    pub zswap: u64,
}

const BYTES_PER_KIB: u64 = 1024;

/// Parses `/proc/meminfo`.
///
/// Returns `None` without a positive `MemTotal` (every fraction divides by it) or without
/// `MemAvailable` (the "used" figure is defined by it, and every kernel since 3.14 has it).
/// Any other missing field reads as zero: `Zswap`, for one, exists only when zswap is built in.
pub fn parse_meminfo(text: &str) -> Option<MemInfo> {
    let mut info = MemInfo::default();
    let mut has_available = false;
    for line in text.lines() {
        let Some((key, rest)) = line.split_once(':') else {
            continue;
        };
        let slot = match key {
            "MemTotal" => &mut info.total,
            "MemFree" => &mut info.free,
            "MemAvailable" => {
                has_available = true;
                &mut info.available
            }
            "SUnreclaim" => &mut info.s_unreclaim,
            "KernelStack" => &mut info.kernel_stack,
            "PageTables" => &mut info.page_tables,
            "SwapTotal" => &mut info.swap_total,
            "SwapFree" => &mut info.swap_free,
            "Zswap" => &mut info.zswap,
            _ => continue,
        };
        let kib: u64 = rest.split_ascii_whitespace().next()?.parse().ok()?;
        *slot = kib.checked_mul(BYTES_PER_KIB)?;
    }
    if info.total == 0 || !has_available {
        None
    } else {
        Some(info)
    }
}

/// The segments of the memory bar. `app + kernel + cached + free == total` always holds, so
/// the bar never over- or under-fills, and `app + kernel` is the "used" figure GNOME System
/// Monitor and `free` show: `MemTotal − MemAvailable`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MemoryBuckets {
    pub total: u64,
    /// Used memory that is not kernel memory: processes, tmpfs and shared memory.
    pub app: u64,
    /// Kernel memory that cannot be reclaimed. The Linux counterpart of macOS "Wired".
    pub kernel: u64,
    /// Memory the kernel could hand out by dropping caches. The counterpart of macOS
    /// "Cached Files".
    pub cached: u64,
    pub free: u64,
    pub swap_used: u64,
    /// Bytes held in the zswap pool, already counted inside `app`. Zero without zswap.
    pub compressed: u64,
}

impl MemoryBuckets {
    pub fn from_meminfo(info: &MemInfo) -> MemoryBuckets {
        let total = info.total;
        // The fields are sampled at slightly different instants and MemAvailable is an
        // estimate, so every step is clamped to what is still unassigned. That is what keeps
        // the sum exact. MemFree can even exceed MemAvailable, because the estimate holds back
        // the kernel's reserve.
        let used = total.saturating_sub(info.available);
        let kernel = info
            .s_unreclaim
            .saturating_add(info.kernel_stack)
            .saturating_add(info.page_tables)
            .min(used);
        let free = info.free.min(total - used);
        MemoryBuckets {
            total,
            app: used - kernel,
            kernel,
            cached: total - used - free,
            free,
            swap_used: info.swap_total.saturating_sub(info.swap_free),
            compressed: info.zswap,
        }
    }

    /// `MemTotal − MemAvailable`: memory that cannot be handed out without evicting or
    /// swapping something. Saturates, because the panel gets these buckets from a peer.
    pub fn used(&self) -> u64 {
        self.app.saturating_add(self.kernel)
    }

    pub fn usage_percent(&self) -> f64 {
        self.fraction(self.used()) * 100.0
    }

    /// How far the pressure ring fills: memory that cannot be reclaimed without work (kernel and
    /// zswap pool), 0–1. Its colour and caption come from [`Pressure`].
    pub fn pressure_fraction(&self) -> f64 {
        self.fraction(self.kernel.saturating_add(self.compressed))
    }

    /// `part` as a share of total memory, 0–1. Zero when the total is unknown.
    pub fn fraction(&self, part: u64) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (part as f64 / self.total as f64).clamp(0.0, 1.0)
        }
    }
}

/// The `avg10` percentage at which each line of the PSI file raises the level.
const PRESSURE_THRESHOLD: f64 = 10.0;

/// Memory pressure, from `/proc/pressure/memory`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pressure {
    Normal,
    Warning,
    Critical,
}

impl Pressure {
    /// Critical at `full avg10 >= 10`, else Warning at `some avg10 >= 10`. `None` when either
    /// figure is missing or malformed: unknown pressure must not read as Normal.
    pub fn from_psi(text: &str) -> Option<Pressure> {
        let avg10 = |kind: &str| -> Option<f64> {
            text.lines()
                .find_map(|line| line.strip_prefix(kind)?.strip_prefix(" avg10="))?
                .split_ascii_whitespace()
                .next()?
                .parse()
                .ok()
                .filter(|value: &f64| value.is_finite())
        };
        let (some, full) = (avg10("some")?, avg10("full")?);
        Some(if full >= PRESSURE_THRESHOLD {
            Pressure::Critical
        } else if some >= PRESSURE_THRESHOLD {
            Pressure::Warning
        } else {
            Pressure::Normal
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            Pressure::Normal => "Normal",
            Pressure::Warning => "Warning",
            Pressure::Critical => "Critical",
        }
    }
}
