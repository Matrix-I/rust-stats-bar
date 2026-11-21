//! The pure layer of StatsBar for Linux.
//!
//! Everything here turns inputs into values and label text. Nothing reads a file, a socket
//! or a clock: callers pass in what they read and when they read it. That is what lets the
//! tests pin the arithmetic on any machine, and `scripts/check-core-purity.sh` enforces it.

pub mod alert;
pub mod battery;
pub mod cpu;
pub mod fan;
pub mod format;
pub mod history;
pub mod label;
pub mod memory;
pub mod network;
pub mod schedule;
pub mod sysfs;
pub mod throughput;
