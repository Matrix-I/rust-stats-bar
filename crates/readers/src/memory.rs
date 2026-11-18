//! `/proc/meminfo` → the fields of the memory bar.

use std::io;
use std::path::Path;

use statsbar_core::memory::{MemInfo, parse_meminfo};

use crate::procfile::{ProcFile, invalid_data};

pub struct MemoryReader {
    meminfo: ProcFile,
}

impl MemoryReader {
    pub fn open(root: &Path) -> io::Result<MemoryReader> {
        Ok(MemoryReader {
            meminfo: ProcFile::open(&root.join("proc/meminfo"))?,
        })
    }

    pub fn read(&mut self) -> io::Result<MemInfo> {
        parse_meminfo(self.meminfo.read()?).ok_or_else(|| {
            invalid_data("proc/meminfo lacks MemTotal or MemAvailable, or has a malformed value")
        })
    }
}
