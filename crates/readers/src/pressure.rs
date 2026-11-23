//! `/proc/pressure/memory` → the pressure level.

use std::io;
use std::path::Path;

use statsbar_core::memory::Pressure;

use crate::procfile::Reopening;

pub struct PressureReader {
    psi: Reopening,
}

impl PressureReader {
    pub fn new(root: &Path) -> PressureReader {
        PressureReader {
            psi: Reopening::new(root.join("proc/pressure/memory")),
        }
    }

    pub fn read(&mut self) -> io::Result<Pressure> {
        self.psi.read(Pressure::from_psi)
    }
}
