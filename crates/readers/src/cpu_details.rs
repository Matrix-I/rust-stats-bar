//! The CPU page's figures beyond load: model, temperatures, frequency, load average, uptime.
//! Read only while that page is open.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use statsbar_core::sysfs::parse_u64;
use statsbar_core::system::{
    load_per_cpu, parse_cpu_list, parse_cpu_model, parse_loadavg, parse_millidegrees, parse_uptime,
};

use crate::hwmon::find_device;
use crate::procfile::{ProcFile, Reopening, invalid_data};

const DRIVER: &str = "coretemp";
const CPUS: &str = "sys/devices/system/cpu";
const KHZ_PER_MHZ: f64 = 1000.0;

/// Any figure may be missing without taking the others with it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CpuDetails {
    pub model: Option<String>,
    pub package_celsius: Option<f64>,
    /// One per online CPU, in `/proc/stat`'s order.
    pub core_celsius: Vec<Option<f64>>,
    pub frequency_mhz: Option<f64>,
    pub load_per_cpu: Option<[f64; 3]>,
    pub uptime_seconds: Option<f64>,
}

/// A figure that could not be read, and why.
pub type Problem = (&'static str, io::Error);

struct Cpu {
    id: u32,
    core: Option<u64>,
    frequency: Reopening,
}

struct Sensors {
    package: Reopening,
    cores: Vec<(u64, Reopening)>,
}

pub struct CpuDetailsReader {
    root: PathBuf,
    online: Reopening,
    loadavg: Reopening,
    uptime: Reopening,
    /// Fixed until reboot, so read once.
    model: Option<String>,
    cpus: Vec<Cpu>,
    sensors: Option<Sensors>,
}

impl CpuDetailsReader {
    pub fn new(root: &Path) -> CpuDetailsReader {
        CpuDetailsReader {
            root: root.to_owned(),
            online: Reopening::new(root.join(CPUS).join("online")),
            loadavg: Reopening::new(root.join("proc/loadavg")),
            uptime: Reopening::new(root.join("proc/uptime")),
            model: None,
            cpus: Vec::new(),
            sensors: None,
        }
    }

    pub fn read(&mut self) -> (CpuDetails, Vec<Problem>) {
        let mut problems = Vec::new();
        let online = keep(
            "online CPUs",
            self.online.read(parse_cpu_list),
            &mut problems,
        )
        .unwrap_or_default();
        if !self
            .cpus
            .iter()
            .map(|cpu| cpu.id)
            .eq(online.iter().copied())
        {
            self.cpus = online.iter().map(|&id| open_cpu(&self.root, id)).collect();
        }
        if self.model.is_none() {
            self.model = keep("model", read_model(&self.root), &mut problems);
        }

        let mut details = CpuDetails {
            model: self.model.clone(),
            core_celsius: vec![None; self.cpus.len()],
            ..CpuDetails::default()
        };
        if let Some((package, cores)) = keep("temperatures", self.temperatures(), &mut problems) {
            details.package_celsius = Some(package);
            for (slot, cpu) in details.core_celsius.iter_mut().zip(&self.cpus) {
                *slot = cores
                    .iter()
                    .find(|(core, _)| cpu.core == Some(*core))
                    .map(|&(_, celsius)| celsius);
            }
        }
        details.frequency_mhz = keep("frequency", self.frequency(), &mut problems);
        details.load_per_cpu = keep(
            "load average",
            self.loadavg.read(parse_loadavg),
            &mut problems,
        )
        .and_then(|load| load_per_cpu(load, self.cpus.len()));
        details.uptime_seconds = keep("uptime", self.uptime.read(parse_uptime), &mut problems);
        (details, problems)
    }

    /// The package and each core. On a failure the sensors are looked up again next time, since
    /// a driver reload renumbers hwmon.
    fn temperatures(&mut self) -> io::Result<(f64, Vec<(u64, f64)>)> {
        let mut sensors = match self.sensors.take() {
            Some(sensors) => sensors,
            None => find_sensors(&self.root)?,
        };
        let package = sensors.package.read(parse_millidegrees)?;
        let mut cores = Vec::with_capacity(sensors.cores.len());
        for (core, input) in &mut sensors.cores {
            cores.push((*core, input.read(parse_millidegrees)?));
        }
        self.sensors = Some(sensors);
        Ok((package, cores))
    }

    /// The mean of every online CPU; a mean of some of them would be a different figure.
    fn frequency(&mut self) -> io::Result<f64> {
        if self.cpus.is_empty() {
            return Err(invalid_data("no online CPU is known"));
        }
        let mut total_khz = 0.0;
        for cpu in &mut self.cpus {
            total_khz += cpu.frequency.read(parse_u64)? as f64;
        }
        Ok(total_khz / self.cpus.len() as f64 / KHZ_PER_MHZ)
    }
}

fn keep<T>(what: &'static str, result: io::Result<T>, problems: &mut Vec<Problem>) -> Option<T> {
    result.map_err(|error| problems.push((what, error))).ok()
}

fn open_cpu(root: &Path, id: u32) -> Cpu {
    let dir = root.join(CPUS).join(format!("cpu{id}"));
    Cpu {
        id,
        // A CPU without topology only loses its temperature, which the page shows as missing.
        core: Reopening::new(dir.join("topology/core_id"))
            .read(parse_u64)
            .ok(),
        frequency: Reopening::new(dir.join("cpufreq/scaling_cur_freq")),
    }
}

fn read_model(root: &Path) -> io::Result<String> {
    let mut cpuinfo = ProcFile::open(&root.join("proc/cpuinfo"))?;
    parse_cpu_model(cpuinfo.read()?).ok_or_else(|| invalid_data("proc/cpuinfo has no model name"))
}

/// coretemp labels its inputs `Package id 0` and `Core N`, N being the topology's `core_id`.
fn find_sensors(root: &Path) -> io::Result<Sensors> {
    let device = find_device(root, DRIVER)?;
    let mut package = None;
    let mut cores = Vec::new();
    for entry in fs::read_dir(&device)? {
        let path = entry?.path();
        let Some(stem) = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix("_label"))
        else {
            continue;
        };
        let input = Reopening::new(device.join(format!("{stem}_input")));
        let mut label = ProcFile::open(&path)?;
        let label = label.read()?.trim_end();
        if label == "Package id 0" {
            package = Some(input);
        } else if let Some(core) = label.strip_prefix("Core ").and_then(parse_u64) {
            cores.push((core, input));
        }
    }
    let package = package.ok_or_else(|| invalid_data("coretemp has no package sensor"))?;
    Ok(Sensors { package, cores })
}
