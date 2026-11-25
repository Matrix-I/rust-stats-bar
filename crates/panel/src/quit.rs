//! Quitting a process from a top-process list: SIGTERM, and only to the process the row named.
//!
//! The daemon names a process by pid and start time. A pidfd is opened first and the start time
//! checked after, so a pid reused in between is caught, and one reused after the check cannot
//! receive the signal: the pidfd still refers to the process that was checked.

use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use rustix::io::Errno;
use rustix::process::{Pid, PidfdFlags, Signal, pidfd_open, pidfd_send_signal};
use statsbar_core::process::parse_stat;

use crate::view::QuitTarget;

#[derive(Debug)]
pub enum QuitError {
    /// The process has already exited.
    Gone,
    /// The pid now belongs to another process.
    Replaced,
    /// Another user's process, which the kernel refuses to signal.
    NotPermitted,
    Io(io::Error),
}

impl fmt::Display for QuitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QuitError::Gone | QuitError::Replaced => write!(f, "it has already exited"),
            QuitError::NotPermitted => write!(f, "it belongs to another user"),
            QuitError::Io(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for QuitError {}

impl From<Errno> for QuitError {
    fn from(errno: Errno) -> QuitError {
        match errno {
            Errno::SRCH => QuitError::Gone,
            Errno::PERM => QuitError::NotPermitted,
            other => QuitError::Io(other.into()),
        }
    }
}

/// Sends SIGTERM to `target`, after checking under `proc_root` (`/proc`) that its pid still
/// names the process the daemon listed.
pub fn quit(proc_root: &Path, target: &QuitTarget) -> Result<(), QuitError> {
    let raw = i32::try_from(target.pid).map_err(|_| QuitError::Gone)?;
    let pid = Pid::from_raw(raw).ok_or(QuitError::Gone)?;
    let pidfd = pidfd_open(pid, PidfdFlags::empty())?;
    let text = match fs::read_to_string(proc_root.join(target.pid.to_string()).join("stat")) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Err(QuitError::Gone),
        Err(error) => return Err(QuitError::Io(error)),
    };
    let stat = parse_stat(&text).ok_or_else(|| {
        QuitError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            "unreadable stat",
        ))
    })?;
    if stat.start_ticks != target.start_ticks {
        return Err(QuitError::Replaced);
    }
    pidfd_send_signal(&pidfd, Signal::TERM)?;
    Ok(())
}
