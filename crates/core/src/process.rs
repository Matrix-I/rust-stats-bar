//! Processes: `/proc/<pid>` text in, the rows of the two top-process tables out.

use std::collections::HashMap;

const NANOS_PER_SECOND: f64 = 1e9;
/// What the kernel appends to `/proc/<pid>/exe` once the binary has been replaced.
const DELETED: &str = " (deleted)";
/// Quitting one of these would end the session or StatsBar itself.
const PROTECTED: [&str; 5] = [
    "gnome-shell",
    "systemd",
    "(sd-pam)",
    "statsbard",
    "statsbar-panel",
];

/// Every GNOME session process is protected by prefix: the session helpers are
/// named `gnome-session-binary` (older GNOME) or `gnome-session-ctl`,
/// `-service`, `-init-worker` (GNOME 50), and the kernel truncates `comm` to
/// 15 bytes, so only a prefix covers all variants and the truncated names.
const PROTECTED_PREFIX: &str = "gnome-session";

/// The fields of `/proc/<pid>/stat` the tables need. Times are in clock ticks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stat {
    pub comm: String,
    /// utime + stime.
    pub cpu_ticks: u64,
    /// Since boot. With the pid, it names one process for good, even after the pid is reused.
    pub start_ticks: u64,
}

/// Parses `/proc/<pid>/stat`. The name may itself hold spaces and parentheses, so it ends at the
/// last `)`.
pub fn parse_stat(text: &str) -> Option<Stat> {
    let (head, tail) = text.rsplit_once(')')?;
    let (_, comm) = head.split_once('(')?;
    let mut fields = tail.split_ascii_whitespace();
    // The tail starts at field 3 (state); utime is 14, stime 15 and starttime 22.
    let utime: u64 = fields.nth(11)?.parse().ok()?;
    let stime: u64 = fields.next()?.parse().ok()?;
    let start_ticks = fields.nth(6)?.parse().ok()?;
    Some(Stat {
        comm: comm.to_owned(),
        cpu_ticks: utime.saturating_add(stime),
        start_ticks,
    })
}

/// Parses `/proc/<pid>/statm` into resident − shared, in bytes: memory no other process shares,
/// as GNOME System Monitor's "Memory" column shows it.
pub fn parse_statm(text: &str, page_size: u64) -> Option<u64> {
    let mut fields = text.split_ascii_whitespace().skip(1);
    let resident: u64 = fields.next()?.parse().ok()?;
    let shared: u64 = fields.next()?.parse().ok()?;
    resident.saturating_sub(shared).checked_mul(page_size)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuSample {
    pub pid: u32,
    pub start_ticks: u64,
    pub cpu_ticks: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CpuUse {
    pub pid: u32,
    pub start_ticks: u64,
    /// 100 is one core, as `top` counts.
    pub percent: f64,
}

/// The `count` busiest processes between two scans. Only a process present in both counts, so a
/// new one appears one scan later and a reused pid never inherits the old process's time. Ties
/// break on the lower pid, so rows do not swap under the cursor.
pub fn rank_by_cpu(
    previous: &[CpuSample],
    current: &[CpuSample],
    elapsed_nanos: u64,
    ticks_per_second: u64,
    count: usize,
) -> Vec<CpuUse> {
    if elapsed_nanos == 0 || ticks_per_second == 0 {
        return Vec::new();
    }
    let elapsed_ticks = elapsed_nanos as f64 / NANOS_PER_SECOND * ticks_per_second as f64;
    let before: HashMap<(u32, u64), u64> = previous
        .iter()
        .map(|sample| ((sample.pid, sample.start_ticks), sample.cpu_ticks))
        .collect();
    let mut ranked: Vec<CpuUse> = current
        .iter()
        .filter_map(|sample| {
            let earlier = before.get(&(sample.pid, sample.start_ticks))?;
            Some(CpuUse {
                pid: sample.pid,
                start_ticks: sample.start_ticks,
                percent: sample.cpu_ticks.saturating_sub(*earlier) as f64 * 100.0 / elapsed_ticks,
            })
        })
        .collect();
    ranked.sort_unstable_by(|a, b| b.percent.total_cmp(&a.percent).then(a.pid.cmp(&b.pid)));
    ranked.truncate(count);
    ranked
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemorySample {
    pub pid: u32,
    pub bytes: u64,
}

/// The `count` heaviest processes, heaviest first, ties on the lower pid.
pub fn rank_by_memory(samples: &[MemorySample], count: usize) -> Vec<MemorySample> {
    let mut ranked = samples.to_vec();
    ranked.sort_unstable_by(|a, b| b.bytes.cmp(&a.bytes).then(a.pid.cmp(&b.pid)));
    ranked.truncate(count);
    ranked
}

/// The executable's file name, else the kernel's 15-character name. A process chooses both, so
/// control characters are replaced.
pub fn display_name(exe: Option<&str>, comm: &str) -> String {
    let exe = exe.map_or("", |name| name.trim_end_matches(DELETED));
    let name = [exe, comm]
        .into_iter()
        .find(|name| !name.trim().is_empty())
        .unwrap_or("—");
    name.chars()
        .map(|c| {
            if c.is_control() {
                char::REPLACEMENT_CHARACTER
            } else {
                c
            }
        })
        .collect()
}

/// Whether the panel may offer to quit this process: the user's own, and not one that would take
/// the session or StatsBar with it. Either name protects it, because `exe` of the user's
/// `systemd` is not readable.
pub fn can_quit(pid: u32, owner_uid: u32, own_uid: u32, exe: Option<&str>, comm: &str) -> bool {
    let exe = exe.map_or("", |name| name.trim_end_matches(DELETED));
    let protected = |name: &str| PROTECTED.contains(&name) || name.starts_with(PROTECTED_PREFIX);
    pid > 1 && owner_uid == own_uid && !protected(exe) && !protected(comm)
}
