//! `/proc/stat` → per-core ticks.

use std::io;
use std::path::Path;

use statsbar_core::cpu::{CoreTicks, parse_proc_stat};

use crate::procfile::{ProcFile, invalid_data};

pub struct CpuReader {
    stat: ProcFile,
}

impl CpuReader {
    pub fn open(root: &Path) -> io::Result<CpuReader> {
        Ok(CpuReader {
            stat: ProcFile::open(&root.join("proc/stat"))?,
        })
    }

    pub fn read(&mut self) -> io::Result<Vec<CoreTicks>> {
        parse_proc_stat(self.stat.read()?)
            .ok_or_else(|| invalid_data("proc/stat has no well-formed per-CPU line"))
    }
}
