//! The pure layer of StatsBar for Linux.
//!
//! Everything here turns text or numbers into numbers or text. Nothing reads a file, a socket
//! or a clock: callers pass in what they read and when they read it. That is what lets the
//! tests pin the arithmetic on any machine, and `scripts/check-core-purity.sh` enforces it.

pub mod alert;
pub mod cpu;
pub mod format;
pub mod history;
pub mod memory;
pub mod throughput;
