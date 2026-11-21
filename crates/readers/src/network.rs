//! `/proc/net/route` and `/sys/class/net/<interface>/statistics/{rx,tx}_bytes` → a counter sample.

use std::io;
use std::path::{Path, PathBuf};

use statsbar_core::network::default_route_interface;
use statsbar_core::sysfs::parse_u64;
use statsbar_core::throughput::Sample;

use crate::procfile::{ProcFile, invalid_data};

pub struct NetworkReader {
    root: PathBuf,
    route: ProcFile,
    counters: Option<Counters>,
}

/// The counter files of the interface the default route used last time, kept open while it
/// stays the same.
struct Counters {
    interface: String,
    rx: ProcFile,
    tx: ProcFile,
}

impl Counters {
    /// `interface` has passed `network::is_interface_name`, so it stays under `sys/class/net`.
    fn open(root: &Path, interface: String) -> io::Result<Counters> {
        let statistics = root
            .join("sys/class/net")
            .join(&interface)
            .join("statistics");
        Ok(Counters {
            rx: ProcFile::open(&statistics.join("rx_bytes"))?,
            tx: ProcFile::open(&statistics.join("tx_bytes"))?,
            interface,
        })
    }
}

impl NetworkReader {
    pub fn open(root: &Path) -> io::Result<NetworkReader> {
        Ok(NetworkReader {
            root: root.to_owned(),
            route: ProcFile::open(&root.join("proc/net/route"))?,
            counters: None,
        })
    }

    /// The default route's interface and its counters, or `None` without a default route.
    pub fn read(&mut self) -> io::Result<Option<Sample>> {
        let interface = default_route_interface(self.route.read()?)
            .map_err(|error| invalid_data(&error.to_string()))?;
        let Some(interface) = interface else {
            self.counters = None;
            return Ok(None);
        };
        let counters = match self.counters.take() {
            Some(counters) if counters.interface == interface => counters,
            _ => Counters::open(&self.root, interface)?,
        };
        let counters = self.counters.insert(counters);
        let rx_bytes = read_counter(&mut counters.rx, "rx_bytes")?;
        let tx_bytes = read_counter(&mut counters.tx, "tx_bytes")?;
        Ok(Some(Sample {
            interface: counters.interface.clone(),
            rx_bytes,
            tx_bytes,
        }))
    }
}

fn read_counter(file: &mut ProcFile, name: &str) -> io::Result<u64> {
    parse_u64(file.read()?).ok_or_else(|| invalid_data(&format!("{name} is not a number")))
}
