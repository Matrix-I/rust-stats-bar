//! What the two pages show, worked out from the daemon's events without GTK, so every figure
//! and colour can be tested.

use statsbar_core::format::{fmt_gb, fmt_process_memory, fmt_uptime, rounded_int};
use statsbar_core::ipc::{CpuPage, MemoryPage, ProcessInfo};
use statsbar_core::memory::Pressure;
use statsbar_core::system::Thermal;

/// Shown for a figure the daemon could not read.
pub const MISSING: &str = "—";
/// The temperature ring's full scale, and where every reading is clamped.
const FULL_SCALE_CELSIUS: f64 = 100.0;
/// Temperatures below this stay quiet; above it they take the ring's colours (upstream).
const WARM_FROM_CELSIUS: f64 = 75.0;
const HOT_FROM_CELSIUS: f64 = 90.0;
/// Memory usage colours (upstream).
const BUSY_FROM_PERCENT: f64 = 70.0;
const FULL_FROM_PERCENT: f64 = 85.0;

/// A colour, named by meaning; the window maps it to a libadwaita style class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// Secondary text, an empty track.
    Dim,
    Good,
    Warning,
    Bad,
    /// The CPU's system share.
    System,
    /// The CPU's user share and the cores' bars.
    User,
    /// Memory segments.
    App,
    Kernel,
    Cached,
    Free,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    /// 0–1 of the whole track.
    pub fraction: f64,
    pub tone: Tone,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ring {
    pub segments: Vec<Segment>,
    pub center: String,
    pub center_tone: Option<Tone>,
    pub caption: &'static str,
}

/// A labelled figure. A row with a tone has a colour dot, as in a legend.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub label: &'static str,
    pub value: String,
    pub dot: Option<Tone>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Core {
    pub busy: f64,
    pub busy_text: String,
    pub temperature: Option<(String, Tone)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProcessRow {
    pub name: String,
    pub value: String,
    /// `Some` only when the daemon marked the process as quittable.
    pub quit: Option<QuitTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuitTarget {
    pub pid: u32,
    pub start_ticks: u64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CpuView {
    pub subtitle: Option<String>,
    pub rings: [Ring; 2],
    pub details: Vec<Row>,
    pub cores: Vec<Core>,
    pub frequency: Option<Row>,
    /// `None` until the daemon's first ranking: "Reading…".
    pub processes: Option<Vec<ProcessRow>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryView {
    pub subtitle: String,
    pub rings: [Ring; 2],
    pub used: String,
    pub bar: Vec<Segment>,
    pub details: Vec<Row>,
    pub processes: Option<Vec<ProcessRow>>,
}

fn percent_text(value: f64) -> String {
    format!("{}%", rounded_int(value))
}

fn fraction(percent: f64) -> f64 {
    (percent / 100.0).clamp(0.0, 1.0)
}

/// Green, then orange from 75 °C and red from 90 °C, as upstream's temperature ring.
pub fn temperature_tone(celsius: f64) -> Tone {
    if celsius >= HOT_FROM_CELSIUS {
        Tone::Bad
    } else if celsius >= WARM_FROM_CELSIUS {
        Tone::Warning
    } else {
        Tone::Good
    }
}

pub fn thermal_tone(thermal: Thermal) -> Tone {
    match thermal {
        Thermal::Nominal => Tone::Good,
        Thermal::Fair | Thermal::Serious => Tone::Warning,
        Thermal::Critical => Tone::Bad,
    }
}

pub fn pressure_tone(pressure: Option<Pressure>) -> Tone {
    match pressure {
        None => Tone::Dim,
        Some(Pressure::Normal) => Tone::Good,
        Some(Pressure::Warning) => Tone::Warning,
        Some(Pressure::Critical) => Tone::Bad,
    }
}

pub fn usage_tone(percent: f64) -> Tone {
    if percent >= FULL_FROM_PERCENT {
        Tone::Bad
    } else if percent >= BUSY_FROM_PERCENT {
        Tone::Warning
    } else {
        Tone::Good
    }
}

fn quit_target(process: &ProcessInfo) -> Option<QuitTarget> {
    process.can_quit.then(|| QuitTarget {
        pid: process.pid,
        start_ticks: process.start_ticks,
        name: process.name.clone(),
    })
}

fn row(label: &'static str, value: String) -> Row {
    Row {
        label,
        value,
        dot: None,
    }
}

fn legend(label: &'static str, value: String, tone: Tone) -> Row {
    Row {
        label,
        value,
        dot: Some(tone),
    }
}

pub fn cpu_view(page: &CpuPage) -> CpuView {
    let load = page.load.as_ref();
    let usage = Ring {
        segments: load.map_or_else(Vec::new, |load| {
            vec![
                Segment {
                    fraction: fraction(load.system_percent),
                    tone: Tone::System,
                },
                Segment {
                    fraction: fraction(load.user_percent),
                    tone: Tone::User,
                },
            ]
        }),
        center: load.map_or_else(
            || MISSING.to_owned(),
            |load| percent_text(load.usage_percent()),
        ),
        center_tone: None,
        caption: "Usage",
    };
    let temperature = Ring {
        segments: page.temperature_c.map_or_else(Vec::new, |celsius| {
            vec![Segment {
                fraction: (celsius / FULL_SCALE_CELSIUS).clamp(0.0, 1.0),
                tone: temperature_tone(celsius),
            }]
        }),
        center: page.temperature_c.map_or_else(
            || MISSING.to_owned(),
            |celsius| format!("{}°", rounded_int(celsius)),
        ),
        center_tone: None,
        caption: "Temperature",
    };
    let share = |pick: fn(&statsbar_core::cpu::CpuLoad) -> f64| {
        load.map_or_else(|| MISSING.to_owned(), |load| percent_text(pick(load)))
    };
    let mut details = vec![
        legend("System", share(|load| load.system_percent), Tone::System),
        legend("User", share(|load| load.user_percent), Tone::User),
        legend("Idle", share(|load| load.idle_percent), Tone::Dim),
        legend(
            "Thermal",
            page.thermal
                .map_or_else(|| MISSING.to_owned(), |thermal| thermal.label().to_owned()),
            page.thermal.map_or(Tone::Dim, thermal_tone),
        ),
    ];
    if page.power_saver {
        details.push(row("Power saver", "On".to_owned()));
    }
    if let Some(load) = page.load_per_cpu {
        details.push(row(
            "Load avg / core (1/5/15m)",
            load.map(|value| format!("{value:.2}")).join(" / "),
        ));
    }
    details.push(row(
        "Uptime",
        page.uptime_seconds
            .map_or_else(|| MISSING.to_owned(), fmt_uptime),
    ));
    let cores = load.map_or_else(Vec::new, |load| {
        load.per_core_busy
            .iter()
            .enumerate()
            .map(|(index, &busy)| Core {
                busy: fraction(busy),
                busy_text: percent_text(busy),
                // A list of another length belongs to other CPUs, and the daemon then sends none.
                temperature: page.core_temperatures_c.get(index).copied().flatten().map(
                    |celsius| {
                        let tone = if celsius < WARM_FROM_CELSIUS {
                            Tone::Dim
                        } else {
                            temperature_tone(celsius)
                        };
                        (format!("{}°", rounded_int(celsius)), tone)
                    },
                ),
            })
            .collect()
    });
    CpuView {
        subtitle: page.model.clone(),
        rings: [usage, temperature],
        details,
        cores,
        frequency: page
            .frequency_mhz
            .map(|mhz| row("All cores", format!("{} MHz", rounded_int(mhz)))),
        processes: page.processes.as_ref().map(|processes| {
            processes
                .iter()
                .map(|entry| ProcessRow {
                    name: entry.process.name.clone(),
                    value: format!("{:.1}%", entry.cpu_percent),
                    quit: quit_target(&entry.process),
                })
                .collect()
        }),
    }
}

pub fn memory_view(page: &MemoryPage) -> MemoryView {
    let memory = &page.buckets;
    let usage_percent = memory.usage_percent();
    let pressure_tone = pressure_tone(page.pressure);
    let pressure = Ring {
        segments: vec![Segment {
            fraction: memory.pressure_fraction(),
            tone: pressure_tone,
        }],
        center: page.pressure.map_or_else(
            || MISSING.to_owned(),
            |pressure| pressure.label().to_owned(),
        ),
        center_tone: Some(pressure_tone),
        caption: "Pressure",
    };
    let usage = Ring {
        segments: vec![Segment {
            fraction: memory.fraction(memory.used()),
            tone: usage_tone(usage_percent),
        }],
        center: percent_text(usage_percent),
        center_tone: None,
        caption: "Usage",
    };
    // Compressed is inside App, so it is a figure, not a segment: the bar tiles the total.
    let bar = [
        (memory.app, Tone::App),
        (memory.kernel, Tone::Kernel),
        (memory.cached, Tone::Cached),
        (memory.free, Tone::Free),
    ]
    .into_iter()
    .map(|(bytes, tone)| Segment {
        fraction: memory.fraction(bytes),
        tone,
    })
    .collect();
    let mut details = vec![
        legend("App", fmt_gb(memory.app), Tone::App),
        legend("Kernel", fmt_gb(memory.kernel), Tone::Kernel),
        legend("Cached", fmt_gb(memory.cached), Tone::Cached),
        legend("Free", fmt_gb(memory.free), Tone::Free),
    ];
    if memory.compressed > 0 {
        details.push(row("Compressed", fmt_gb(memory.compressed)));
    }
    details.push(row("Swap", fmt_gb(memory.swap_used)));
    MemoryView {
        subtitle: format!("{} total", fmt_gb(memory.total)),
        rings: [pressure, usage],
        used: fmt_gb(memory.used()),
        bar,
        details,
        processes: page.processes.as_ref().map(|processes| {
            processes
                .iter()
                .map(|entry| ProcessRow {
                    name: entry.process.name.clone(),
                    value: fmt_process_memory(entry.bytes),
                    quit: quit_target(&entry.process),
                })
                .collect()
        }),
    }
}
