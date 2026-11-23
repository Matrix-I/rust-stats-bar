//! What a page shows beyond its tray item's figures. Nothing here runs unless someone watches
//! that page, and closing it drops what it held.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use statsbar_core::cpu::CpuLoad;
use statsbar_core::ipc::{CpuPage, CpuProcess, MemoryPage, MemoryProcess, Page};
use statsbar_core::memory::MemoryBuckets;
use statsbar_core::process::{self, CpuSample};
use statsbar_core::system::Thermal;
use statsbar_readers::cpu_details::CpuDetailsReader;
use statsbar_readers::pressure::PressureReader;
use statsbar_readers::processes::ProcessReader;

pub use statsbar_readers::cpu_details::Problem;

/// Rows in each table, as upstream.
const TOP_COUNT: usize = 6;
/// CPU time counts in 10 ms ticks, so a shorter interval mostly measures rounding.
const MIN_CPU_SCAN_NANOS: u64 = 500_000_000;
/// As upstream's memory table.
const MEMORY_SCAN_NANOS: u64 = 2_000_000_000;

/// The numbers only the kernel knows.
#[derive(Debug, Clone, Copy)]
pub struct System {
    pub own_uid: u32,
    pub page_size: u64,
    pub ticks_per_second: u64,
}

struct Cpu {
    details: CpuDetailsReader,
    previous: Option<(u64, Vec<CpuSample>)>,
    processes: Option<Vec<CpuProcess>>,
    failing: HashSet<&'static str>,
}

struct Memory {
    pressure: PressureReader,
    scanned_at: Option<u64>,
    processes: Option<Vec<MemoryProcess>>,
    failing: HashSet<&'static str>,
}

pub struct Pages {
    root: PathBuf,
    ticks_per_second: u64,
    reader: ProcessReader,
    cpu: Option<Cpu>,
    memory: Option<Memory>,
}

impl Pages {
    pub fn new(root: &Path, system: System) -> Pages {
        Pages {
            root: root.to_owned(),
            ticks_per_second: system.ticks_per_second,
            reader: ProcessReader::new(root, system.own_uid, system.page_size),
            cpu: None,
            memory: None,
        }
    }

    /// The CPU page, and the problems that are new since the last call.
    pub fn cpu(
        &mut self,
        now_nanos: u64,
        load: Option<&CpuLoad>,
        power_saver: bool,
    ) -> (CpuPage, Vec<Problem>) {
        let cpu = self.cpu.get_or_insert_with(|| Cpu {
            details: CpuDetailsReader::new(&self.root),
            previous: None,
            processes: None,
            failing: HashSet::new(),
        });
        let (details, mut problems) = cpu.details.read();

        let rested = cpu
            .previous
            .as_ref()
            .is_none_or(|(at, _)| now_nanos.saturating_sub(*at) >= MIN_CPU_SCAN_NANOS);
        if rested {
            match self.reader.cpu_samples() {
                Ok(samples) => {
                    if let Some((at, previous)) = &cpu.previous {
                        let ranked = process::rank_by_cpu(
                            previous,
                            &samples,
                            now_nanos.saturating_sub(*at),
                            self.ticks_per_second,
                            TOP_COUNT,
                        );
                        cpu.processes = Some(
                            ranked
                                .iter()
                                .filter_map(|row| {
                                    let process = self.reader.identify(row.pid)?;
                                    // Otherwise the pid changed hands since the scan.
                                    (process.start_ticks == row.start_ticks).then_some(CpuProcess {
                                        process,
                                        cpu_percent: row.percent,
                                    })
                                })
                                .collect(),
                        );
                    }
                    cpu.previous = Some((now_nanos, samples));
                }
                Err(error) => problems.push(("processes", error)),
            }
        }

        let mut core_temperatures_c = details.core_celsius;
        if let Some(load) = load {
            // Different CPUs than the load's, for one sample after a hotplug: pair none of them.
            if load.per_core_busy.len() != core_temperatures_c.len() {
                core_temperatures_c = vec![None; load.per_core_busy.len()];
            }
        }
        let page = CpuPage {
            model: details.model,
            load: load.cloned(),
            temperature_c: details.package_celsius,
            thermal: details.package_celsius.map(Thermal::from_celsius),
            core_temperatures_c,
            frequency_mhz: details.frequency_mhz,
            load_per_cpu: details.load_per_cpu,
            uptime_seconds: details.uptime_seconds,
            power_saver,
            processes: cpu.processes.clone(),
        };
        (page, newly_failing(&mut cpu.failing, problems))
    }

    /// The Memory page, and the problems that are new since the last call.
    pub fn memory(&mut self, now_nanos: u64, buckets: MemoryBuckets) -> (MemoryPage, Vec<Problem>) {
        let memory = self.memory.get_or_insert_with(|| Memory {
            pressure: PressureReader::new(&self.root),
            scanned_at: None,
            processes: None,
            failing: HashSet::new(),
        });
        let mut problems = Vec::new();
        let pressure = memory
            .pressure
            .read()
            .map_err(|error| problems.push(("memory pressure", error)))
            .ok();

        let due = memory
            .scanned_at
            .is_none_or(|at| now_nanos.saturating_sub(at) >= MEMORY_SCAN_NANOS);
        if due {
            match self.reader.memory_samples() {
                Ok(samples) => {
                    memory.scanned_at = Some(now_nanos);
                    memory.processes = Some(
                        process::rank_by_memory(&samples, TOP_COUNT)
                            .iter()
                            .filter_map(|row| {
                                Some(MemoryProcess {
                                    process: self.reader.identify(row.pid)?,
                                    bytes: row.bytes,
                                })
                            })
                            .collect(),
                    );
                }
                Err(error) => problems.push(("processes", error)),
            }
        }

        let page = MemoryPage {
            buckets,
            pressure,
            processes: memory.processes.clone(),
        };
        (page, newly_failing(&mut memory.failing, problems))
    }

    /// The page's Refresh: its next reading scans the processes too.
    pub fn refresh(&mut self, page: Page) {
        // The CPU table already follows every reading.
        if page != Page::Memory {
            return;
        }
        if let Some(memory) = &mut self.memory {
            memory.scanned_at = None;
        }
    }

    /// Nobody watches the page: its files close and its samples go.
    pub fn close(&mut self, page: Page) {
        match page {
            Page::Cpu => self.cpu = None,
            Page::Memory => self.memory = None,
        }
    }
}

/// The problems that were not failing last time, which are the ones worth logging.
fn newly_failing(failing: &mut HashSet<&'static str>, problems: Vec<Problem>) -> Vec<Problem> {
    let now: HashSet<&'static str> = problems.iter().map(|(what, _)| *what).collect();
    let new = problems
        .into_iter()
        .filter(|(what, _)| !failing.contains(what))
        .collect();
    *failing = now;
    new
}
