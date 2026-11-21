//! The `dell_smm` hwmon device's `fan1_*` attributes → a fan speed.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use statsbar_core::fan::FanSpeed;
use statsbar_core::sysfs::parse_u64;

use crate::procfile::{ProcFile, invalid_data};

const HWMON: &str = "sys/class/hwmon";
/// hwmon numbers change between boots, so the device is found by its `name`.
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
        let device = find_device(&root.join(HWMON))?;
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

fn find_device(hwmon: &Path) -> io::Result<PathBuf> {
    for entry in fs::read_dir(hwmon)? {
        let device = entry?.path();
        let mut name = match ProcFile::open(&device.join("name")) {
            Ok(file) => file,
            // A device without a name cannot be the one looked for.
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        if name.read()?.trim_end() == DRIVER {
            return Ok(device);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("no {DRIVER} device under {}", hwmon.display()),
    ))
}

fn read_rpm(path: &Path) -> io::Result<u64> {
    let mut file = ProcFile::open(path)?;
    parse_u64(file.read()?)
        .ok_or_else(|| invalid_data(&format!("{} is not a number", path.display())))
}
