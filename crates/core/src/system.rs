//! System-wide figures of the CPU page: kernel text in, values out.

use serde::{Deserialize, Serialize};

/// Above any CPU id Linux supports (`NR_CPUS` tops out at 8192); bounds a malformed range.
const MAX_CPU_ID: u32 = 8191;
/// A sensor outside this range is broken. Upstream drops the same readings.
const PLAUSIBLE_CELSIUS: std::ops::Range<f64> = 0.0..130.0;
const FAIR_FROM_CELSIUS: f64 = 75.0;
const SERIOUS_FROM_CELSIUS: f64 = 90.0;
const CRITICAL_FROM_CELSIUS: f64 = 100.0;

fn non_negative(text: &str) -> Option<f64> {
    text.parse()
        .ok()
        .filter(|value: &f64| value.is_finite() && *value >= 0.0)
}

/// The 1, 5 and 15 minute averages of `/proc/loadavg`.
pub fn parse_loadavg(text: &str) -> Option<[f64; 3]> {
    let mut fields = text.split_ascii_whitespace();
    let mut averages = [0.0; 3];
    for slot in &mut averages {
        *slot = non_negative(fields.next()?)?;
    }
    Some(averages)
}

/// Load per logical CPU: 1.00 is exactly as much runnable work as there are CPUs.
pub fn load_per_cpu(load: [f64; 3], cpus: usize) -> Option<[f64; 3]> {
    (cpus > 0).then(|| load.map(|value| value / cpus as f64))
}

/// Seconds since boot, the first field of `/proc/uptime`.
pub fn parse_uptime(text: &str) -> Option<f64> {
    non_negative(text.split_ascii_whitespace().next()?)
}

/// The first `model name` of `/proc/cpuinfo`.
pub fn parse_cpu_model(text: &str) -> Option<String> {
    text.lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(key, value)| key.trim_end() == "model name" && !value.trim().is_empty())
        .map(|(_, value)| value.trim().to_owned())
}

/// A kernel CPU list such as `0-3,6`, as `/sys/devices/system/cpu/online` prints it. The order
/// is that of `/proc/stat`'s `cpuN` lines, which is how a load is paired with a CPU id.
pub fn parse_cpu_list(text: &str) -> Option<Vec<u32>> {
    let mut cpus = Vec::new();
    for part in text.trim().split(',') {
        let (first, last) = part.split_once('-').unwrap_or((part, part));
        let (first, last): (u32, u32) = (first.parse().ok()?, last.parse().ok()?);
        // Also refuses a range that repeats or goes back, which bounds the list's length.
        if first > last || last > MAX_CPU_ID || cpus.last().is_some_and(|&cpu| cpu >= first) {
            return None;
        }
        cpus.extend(first..=last);
    }
    Some(cpus)
}

/// An hwmon `temp*_input` in °C.
pub fn parse_millidegrees(text: &str) -> Option<f64> {
    let celsius = text.trim().parse::<i32>().ok().map(f64::from)? / 1000.0;
    (celsius > 0.0 && PLAUSIBLE_CELSIUS.contains(&celsius)).then_some(celsius)
}

/// How hot the CPU package is. Derived: this machine's `temp1_max` equals its `temp1_crit`
/// (100 °C), so the levels are fixed temperatures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Thermal {
    Nominal,
    Fair,
    Serious,
    Critical,
}

impl Thermal {
    pub fn from_celsius(celsius: f64) -> Thermal {
        if celsius >= CRITICAL_FROM_CELSIUS {
            Thermal::Critical
        } else if celsius >= SERIOUS_FROM_CELSIUS {
            Thermal::Serious
        } else if celsius >= FAIR_FROM_CELSIUS {
            Thermal::Fair
        } else {
            Thermal::Nominal
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Thermal::Nominal => "Nominal",
            Thermal::Fair => "Fair",
            Thermal::Serious => "Serious",
            Thermal::Critical => "Critical",
        }
    }
}
