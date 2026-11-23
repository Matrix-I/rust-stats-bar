//! Finding an hwmon device. Their numbers change between boots, so it goes by `name`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::procfile::ProcFile;

pub(crate) fn find_device(root: &Path, driver: &str) -> io::Result<PathBuf> {
    let hwmon = root.join("sys/class/hwmon");
    for entry in fs::read_dir(&hwmon)? {
        let device = entry?.path();
        let mut name = match ProcFile::open(&device.join("name")) {
            Ok(file) => file,
            // A device without a name cannot be the one looked for.
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        if name.read()?.trim_end() == driver {
            return Ok(device);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("no {driver} device under {}", hwmon.display()),
    ))
}
