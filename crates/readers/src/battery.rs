//! `/sys/class/power_supply/BAT0/uevent` and `AC/online` → a battery reading.

use std::io;
use std::path::Path;

use statsbar_core::battery::{BatteryInfo, parse_uevent};
use statsbar_core::sysfs::parse_bool;

use crate::procfile::{ProcFile, invalid_data};

/// This laptop's supplies (roadmap requirement 5). One `uevent` read is one ACPI call, where
/// reading the attributes one by one would be ten.
const BATTERY: &str = "sys/class/power_supply/BAT0/uevent";
const ADAPTER: &str = "sys/class/power_supply/AC/online";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatteryReading {
    pub info: BatteryInfo,
    /// `None` when the machine has no `AC` supply.
    pub adapter_online: Option<bool>,
}

pub struct BatteryReader {
    uevent: ProcFile,
    adapter: Option<ProcFile>,
}

impl BatteryReader {
    /// Opens both files. A missing `AC` is not an error, because the battery's figures stand
    /// alone; any other failure to open it is.
    pub fn open(root: &Path) -> io::Result<BatteryReader> {
        let uevent = ProcFile::open(&root.join(BATTERY))?;
        let adapter = match ProcFile::open(&root.join(ADAPTER)) {
            Ok(file) => Some(file),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        Ok(BatteryReader { uevent, adapter })
    }

    pub fn read(&mut self) -> io::Result<BatteryReading> {
        let info = parse_uevent(self.uevent.read()?).ok_or_else(|| {
            invalid_data(
                "BAT0/uevent lacks a status or a capacity from 0 to 100, or has a malformed number",
            )
        })?;
        let adapter_online = match &mut self.adapter {
            Some(file) => Some(
                parse_bool(file.read()?)
                    .ok_or_else(|| invalid_data("AC/online is neither 0 nor 1"))?,
            ),
            None => None,
        };
        Ok(BatteryReading {
            info,
            adapter_online,
        })
    }
}
