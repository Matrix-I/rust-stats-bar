//! Readers: files on this machine in, core types out.
//!
//! Each reader opens its file once and re-reads it from offset 0 with `pread`, so a sample costs
//! a read instead of an open, a read and a close. Readers take the filesystem root as an argument
//! (`/` in production), which lets the tests run them against fixture trees.

pub mod cpu;
pub mod memory;
mod procfile;
