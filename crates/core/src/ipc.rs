//! What `statsbard` and `statsbar-panel` say to each other: one JSON object per line. The first
//! line each way is a [`Hello`]. Then the panel sends [`Request`]s and the daemon [`Event`]s.
//!
//! Receivers ignore fields they do not know, and an absent optional field reads as `None`, so a
//! field can be added within a version. Anything else needs a new [`PROTOCOL_VERSION`].

use serde::{Deserialize, Serialize};

use crate::cpu::CpuLoad;
use crate::memory::{MemoryBuckets, Pressure};
use crate::system::Thermal;

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    pub protocol: u32,
}

impl Hello {
    pub fn current() -> Hello {
        Hello {
            protocol: PROTOCOL_VERSION,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Page {
    Cpu,
    Memory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    /// Replaces the connection's earlier subscription: a connection watches one page.
    Subscribe {
        page: Page,
    },
    Unsubscribe,
    /// Read the subscribed page again without waiting for its interval.
    Refresh,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    Cpu(CpuPage),
    Memory(MemoryPage),
    /// Always the last line before the daemon closes the connection.
    Error {
        reason: ErrorReason,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorReason {
    VersionMismatch,
    BadRequest,
    Busy,
    HandshakeTimeout,
}

/// Names one process. The panel compares `start_ticks` again before it signals `pid`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub start_ticks: u64,
    pub name: String,
    pub can_quit: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CpuProcess {
    pub process: ProcessInfo,
    /// 100 is one core.
    pub cpu_percent: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryProcess {
    pub process: ProcessInfo,
    /// Resident − shared.
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CpuPage {
    pub model: Option<String>,
    /// `None` until the second sample.
    pub load: Option<CpuLoad>,
    /// The package sensor.
    pub temperature_c: Option<f64>,
    pub thermal: Option<Thermal>,
    /// In the order of `load.per_core_busy`. Two logical CPUs of one core share a value.
    pub core_temperatures_c: Vec<Option<f64>>,
    /// The mean over the online CPUs.
    pub frequency_mhz: Option<f64>,
    /// 1, 5 and 15 minute averages, each divided by the number of online CPUs.
    pub load_per_cpu: Option<[f64; 3]>,
    pub uptime_seconds: Option<f64>,
    pub power_saver: bool,
    /// `None` until the first ranking.
    pub processes: Option<Vec<CpuProcess>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryPage {
    pub buckets: MemoryBuckets,
    /// `None` when PSI cannot be read.
    pub pressure: Option<Pressure>,
    /// `None` until the first ranking.
    pub processes: Option<Vec<MemoryProcess>>,
}
