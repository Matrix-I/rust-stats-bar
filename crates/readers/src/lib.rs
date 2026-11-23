//! Readers: files on this machine in, core types out.
//!
//! Each reader opens its file once and re-reads it from offset 0 with `pread`, so a sample costs
//! a read instead of an open, a read and a close. Readers take the filesystem root as an argument
//! (`/` in production), which lets the tests run them against fixture trees.
//!
//! `processes` is the exception: pids come and go, so it opens a file per process, and only
//! while a page that lists them is open.

pub mod battery;
pub mod cpu;
pub mod cpu_details;
pub mod fan;
mod hwmon;
pub mod memory;
pub mod network;
pub mod pressure;
pub mod processes;
mod procfile;
