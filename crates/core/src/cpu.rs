//! CPU load: `/proc/stat` text in, percentages out.

/// Cumulative jiffies of one logical CPU, folded into the buckets the UI shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CoreTicks {
    pub user: u64,
    pub nice: u64,
    pub system: u64,
    pub idle: u64,
}

/// Parses the per-CPU lines (`cpu0`, `cpu1`, …) of `/proc/stat`, in file order.
///
/// irq, softirq and steal are folded into `system`, and iowait into `idle`, so the four
/// buckets add up to all the time the kernel accounted for. The aggregate `cpu ` line is
/// skipped because it is the sum of the others.
///
/// Returns `None` when there is no per-CPU line or one of them is malformed. Skipping a bad
/// line instead would shift every later core by one position, and the per-core view would
/// then pair a core with its neighbour's temperature.
pub fn parse_proc_stat(text: &str) -> Option<Vec<CoreTicks>> {
    let mut cores = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("cpu") else {
            continue;
        };
        if !rest.starts_with(|c: char| c.is_ascii_digit()) {
            continue;
        }
        let mut fields = rest.split_ascii_whitespace();
        fields.next()?; // the CPU index glued to the "cpu" prefix
        let mut values = [0u64; 8];
        let mut count = 0;
        for (slot, field) in values.iter_mut().zip(fields) {
            *slot = field.parse().ok()?;
            count += 1;
        }
        // user, nice, system and idle exist on every kernel; the later columns were added
        // over time and are zero when absent.
        if count < 4 {
            return None;
        }
        let [user, nice, system, idle, iowait, irq, softirq, steal] = values;
        // Saturating: the text is untrusted input, and a plain `+` panics on overflow in
        // debug builds.
        cores.push(CoreTicks {
            user,
            nice,
            system: system
                .saturating_add(irq)
                .saturating_add(softirq)
                .saturating_add(steal),
            idle: idle.saturating_add(iowait),
        });
    }
    if cores.is_empty() { None } else { Some(cores) }
}

/// CPU usage over the interval between two `/proc/stat` samples.
#[derive(Debug, Clone, PartialEq)]
pub struct CpuLoad {
    pub system_percent: f64,
    pub user_percent: f64,
    pub idle_percent: f64,
    /// Busy share of each logical CPU, 0–100, in `/proc/stat` order.
    pub per_core_busy: Vec<f64>,
}

impl CpuLoad {
    /// Returns `None` when the two samples do not describe the same set of CPUs (a core was
    /// taken offline between them), because the deltas would then mix different cores.
    pub fn between(previous: &[CoreTicks], current: &[CoreTicks]) -> Option<CpuLoad> {
        if previous.len() != current.len() || current.is_empty() {
            return None;
        }
        let (mut sum_user, mut sum_system, mut sum_idle, mut sum_total) = (0.0, 0.0, 0.0, 0.0);
        let mut per_core_busy = Vec::with_capacity(current.len());
        for (prev, cur) in previous.iter().zip(current) {
            // A counter that went backwards (suspend/resume on some kernels) counts as no
            // progress instead of underflowing into an enormous delta.
            let user = cur.user.saturating_sub(prev.user) as f64;
            let nice = cur.nice.saturating_sub(prev.nice) as f64;
            let system = cur.system.saturating_sub(prev.system) as f64;
            let idle = cur.idle.saturating_sub(prev.idle) as f64;
            let total = user + nice + system + idle;
            sum_user += user + nice;
            sum_system += system;
            sum_idle += idle;
            sum_total += total;
            per_core_busy.push(if total > 0.0 {
                (user + nice + system) / total * 100.0
            } else {
                0.0
            });
        }
        let mut load = CpuLoad {
            system_percent: 0.0,
            user_percent: 0.0,
            idle_percent: 100.0,
            per_core_busy,
        };
        if sum_total > 0.0 {
            load.user_percent = sum_user / sum_total * 100.0;
            load.system_percent = sum_system / sum_total * 100.0;
            load.idle_percent = sum_idle / sum_total * 100.0;
        }
        Some(load)
    }

    pub fn usage_percent(&self) -> f64 {
        (100.0 - self.idle_percent).clamp(0.0, 100.0)
    }
}
