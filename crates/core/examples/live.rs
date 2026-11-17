//! Prints what the core makes of this machine's real /proc files. A manual check, not a test:
//! its output depends on the machine. The example may read files; the crate under test may not.

use statsbar_core::cpu::{CpuLoad, parse_proc_stat};
use statsbar_core::format::fmt_gb;
use statsbar_core::memory::{MemoryBuckets, parse_meminfo};

fn main() -> std::io::Result<()> {
    let first = std::fs::read_to_string("/proc/stat")?;
    std::thread::sleep(std::time::Duration::from_secs(1));
    let second = std::fs::read_to_string("/proc/stat")?;
    match (parse_proc_stat(&first), parse_proc_stat(&second)) {
        (Some(previous), Some(current)) => match CpuLoad::between(&previous, &current) {
            Some(load) => println!(
                "cpu: {:.1}% over {} cores",
                load.usage_percent(),
                current.len()
            ),
            None => println!("cpu: core count changed between samples"),
        },
        _ => println!("cpu: /proc/stat did not parse"),
    }
    match parse_meminfo(&std::fs::read_to_string("/proc/meminfo")?) {
        Some(info) => {
            let buckets = MemoryBuckets::from_meminfo(&info);
            println!(
                "memory: {:.1}% used — app {}, kernel {}, cached {}, free {}, total {}",
                buckets.usage_percent(),
                fmt_gb(buckets.app),
                fmt_gb(buckets.kernel),
                fmt_gb(buckets.cached),
                fmt_gb(buckets.free),
                fmt_gb(buckets.total),
            );
        }
        None => println!("memory: /proc/meminfo did not parse"),
    }
    Ok(())
}
