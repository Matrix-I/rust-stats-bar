//! `/proc/<pid>` → samples for ranking, and who the ranked processes are.

use std::fs::{self, File};
use std::io::{self, Read};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use statsbar_core::ipc::ProcessInfo;
use statsbar_core::process::{
    CpuSample, MemorySample, can_quit, display_name, parse_stat, parse_statm,
};

use crate::procfile::invalid_data;

/// `stat` and `statm` are one short line each; the kernel's name is at most 64 bytes of it.
const MAX_BYTES: u64 = 4096;

pub struct ProcessReader {
    proc: PathBuf,
    own_uid: u32,
    page_size: u64,
}

impl ProcessReader {
    pub fn new(root: &Path, own_uid: u32, page_size: u64) -> ProcessReader {
        ProcessReader {
            proc: root.join("proc"),
            own_uid,
            page_size,
        }
    }

    pub fn cpu_samples(&self) -> io::Result<Vec<CpuSample>> {
        self.scan("stat", |pid, text| {
            parse_stat(text).map(|stat| CpuSample {
                pid,
                start_ticks: stat.start_ticks,
                cpu_ticks: stat.cpu_ticks,
            })
        })
    }

    pub fn memory_samples(&self) -> io::Result<Vec<MemorySample>> {
        self.scan("statm", |pid, text| {
            parse_statm(text, self.page_size).map(|bytes| MemorySample { pid, bytes })
        })
    }

    /// Who `pid` is now, or `None` once it has exited. Asked only for the ranked few, since it
    /// costs three lookups.
    pub fn identify(&self, pid: u32) -> Option<ProcessInfo> {
        let dir = self.proc.join(pid.to_string());
        let stat = parse_stat(&read_lossy(&dir.join("stat")).ok()?)?;
        // The directory keeps the process's effective uid even where its files belong to root.
        let owner = fs::metadata(&dir).ok()?.uid();
        // Not readable for other users' processes, kernel threads and non-dumpable processes.
        let exe = fs::read_link(dir.join("exe")).ok().and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        });
        Some(ProcessInfo {
            pid,
            start_ticks: stat.start_ticks,
            name: display_name(exe.as_deref(), &stat.comm),
            can_quit: can_quit(pid, owner, self.own_uid, exe.as_deref(), &stat.comm),
        })
    }

    /// A process that exits during the scan, or cannot be read, is left out. Only a scan that
    /// finds no process at all is an error.
    fn scan<T>(&self, file: &str, parse: impl Fn(u32, &str) -> Option<T>) -> io::Result<Vec<T>> {
        let mut samples = Vec::new();
        for entry in fs::read_dir(&self.proc)? {
            let entry = entry?;
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|name| name.parse().ok())
            else {
                continue;
            };
            let Ok(text) = read_lossy(&entry.path().join(file)) else {
                continue;
            };
            samples.extend(parse(pid, &text));
        }
        if samples.is_empty() {
            return Err(invalid_data(&format!("no process has a readable {file}")));
        }
        Ok(samples)
    }
}

/// A process names itself with any bytes, so this cannot insist on UTF-8 as `ProcFile` does.
fn read_lossy(path: &Path) -> io::Result<String> {
    let mut bytes = Vec::new();
    File::open(path)?.take(MAX_BYTES).read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
