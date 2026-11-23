//! The `dell_smm` hwmon device's `fan1_*` attributes → a fan speed.

use std::io;
use std::path::Path;

use statsbar_core::fan::FanSpeed;
use statsbar_core::sysfs::parse_u64;

use crate::hwmon::find_device;
use crate::procfile::{ProcFile, invalid_data};

const DRIVER: &str = "dell_smm";

pub struct FanReader {
    input: ProcFile,
    min_rpm: u64,
    max_rpm: u64,
}

impl FanReader {
    /// Finds the device and reads its limits, which the driver sets when it loads. Only
    /// `fan1_input` is kept open, because each read of it is a BIOS (SMM) call: 0.7 ms here.
    pub fn open(root: &Path) -> io::Result<FanReader> {
        let device = find_device(root, DRIVER)?;
        Ok(FanReader {
            min_rpm: read_rpm(&device.join("fan1_min"))?,
            max_rpm: read_rpm(&device.join("fan1_max"))?,
            input: ProcFile::open(&device.join("fan1_input"))?,
        })
    }

    pub fn read(&mut self) -> io::Result<FanSpeed> {
        let rpm = parse_u64(self.input.read()?)
            .ok_or_else(|| invalid_data("fan1_input is not a number"))?;
        Ok(FanSpeed {
            rpm,
            min_rpm: self.min_rpm,
            max_rpm: self.max_rpm,
        })
    }
}

fn read_rpm(path: &Path) -> io::Result<u64> {
    let mut file = ProcFile::open(path)?;
    parse_u64(file.read()?)
        .ok_or_else(|| invalid_data(&format!("{} is not a number", path.display())))
}
