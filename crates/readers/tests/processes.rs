mod support;

use std::os::unix::fs::{MetadataExt, symlink};
use std::path::Path;

use statsbar_core::process::{CpuSample, MemorySample};
use statsbar_readers::processes::ProcessReader;
use support::Tree;

const PAGE: u64 = 4096;

fn stat(pid: u32, comm: &str, utime: u64, start: u64) -> String {
    format!("{pid} ({comm}) S 1 1 1 0 -1 0 0 0 0 0 {utime} 2 0 0 20 0 1 0 {start} 0 0\n")
}

fn process(tree: &Tree, pid: u32, comm: &str, utime: u64, resident: u64) {
    tree.write(
        &format!("proc/{pid}/stat"),
        stat(pid, comm, utime, u64::from(pid) * 10),
    );
    tree.write(
        &format!("proc/{pid}/statm"),
        format!("9000 {resident} 100 0 0 0 0\n"),
    );
}

/// The uid that owns the fixture trees, which is the one running the tests.
fn uid(tree: &Tree) -> u32 {
    std::fs::metadata(&tree.root)
        .expect("the root exists")
        .uid()
}

fn reader(tree: &Tree) -> ProcessReader {
    ProcessReader::new(&tree.root, uid(tree), PAGE)
}

#[test]
fn cpu_samples_cover_every_numeric_directory() {
    let tree = Tree::new("processes-cpu");
    process(&tree, 10, "ten", 40, 500);
    process(&tree, 20, "twenty", 80, 700);
    tree.write("proc/meminfo", "MemTotal: 1 kB\n");
    tree.write("proc/acpi/wakeup", "x\n");
    let mut samples = reader(&tree).cpu_samples().expect("proc exists");
    samples.sort_by_key(|sample| sample.pid);
    assert_eq!(
        samples,
        [
            CpuSample {
                pid: 10,
                start_ticks: 100,
                cpu_ticks: 42
            },
            CpuSample {
                pid: 20,
                start_ticks: 200,
                cpu_ticks: 82
            },
        ]
    );
}

#[test]
fn memory_samples_are_resident_minus_shared() {
    let tree = Tree::new("processes-memory");
    process(&tree, 10, "ten", 0, 500);
    assert_eq!(
        reader(&tree).memory_samples().expect("proc exists"),
        [MemorySample {
            pid: 10,
            bytes: 400 * PAGE
        }]
    );
}

#[test]
fn a_vanished_or_unreadable_process_is_skipped() {
    let tree = Tree::new("processes-vanished");
    process(&tree, 10, "ten", 40, 500);
    // Exited after the directory was listed: nothing left inside.
    tree.write("proc/11/cmdline", "");
    tree.write("proc/12/stat", "12 (cut) S 1");
    tree.write("proc/12/statm", "9");
    let reader = reader(&tree);
    assert_eq!(reader.cpu_samples().expect("proc exists").len(), 1);
    assert_eq!(reader.memory_samples().expect("proc exists").len(), 1);
}

#[test]
fn a_name_that_is_not_utf8_still_gives_a_sample() {
    let tree = Tree::new("processes-not-utf8");
    let mut line = b"10 (bad".to_vec();
    line.push(0xff);
    line.extend_from_slice(b") S 1 1 1 0 -1 0 0 0 0 0 5 1 0 0 20 0 1 0 77 0 0\n");
    tree.write("proc/10/stat", line);
    let reader = reader(&tree);
    assert_eq!(
        reader.cpu_samples().expect("proc exists"),
        [CpuSample {
            pid: 10,
            start_ticks: 77,
            cpu_ticks: 6
        }]
    );
    assert_eq!(
        reader.identify(10).expect("still there").name,
        "bad\u{fffd}"
    );
}

#[test]
fn without_proc_or_without_one_readable_process_the_scan_fails() {
    let tree = Tree::new("processes-none");
    let reader = reader(&tree);
    assert!(reader.cpu_samples().is_err(), "no proc");
    tree.write("proc/meminfo", "MemTotal: 1 kB\n");
    assert!(reader.cpu_samples().is_err(), "no process");
    assert!(reader.memory_samples().is_err(), "no process");
}

#[test]
fn identify_prefers_the_executable_and_reports_the_start_time() {
    let tree = Tree::new("processes-identify");
    process(&tree, 10, "gnome-keyring-d", 0, 500);
    symlink(
        "/usr/bin/gnome-keyring-daemon",
        tree.root.join("proc/10/exe"),
    )
    .expect("symlink");
    let info = reader(&tree).identify(10).expect("still there");
    assert_eq!(
        (
            info.pid,
            info.start_ticks,
            info.name.as_str(),
            info.can_quit
        ),
        (10, 100, "gnome-keyring-daemon", true)
    );
}

#[test]
fn identify_falls_back_to_the_kernel_name_and_still_protects_by_it() {
    let tree = Tree::new("processes-no-exe");
    process(&tree, 10, "systemd", 0, 500);
    let info = reader(&tree).identify(10).expect("still there");
    assert_eq!((info.name.as_str(), info.can_quit), ("systemd", false));
}

#[test]
fn another_users_process_cannot_be_quit() {
    let tree = Tree::new("processes-other-uid");
    process(&tree, 10, "ten", 0, 500);
    let reader = ProcessReader::new(&tree.root, uid(&tree) + 1, PAGE);
    assert!(!reader.identify(10).expect("still there").can_quit);
}

#[test]
fn identify_of_an_exited_process_is_none() {
    let tree = Tree::new("processes-exited");
    assert_eq!(reader(&tree).identify(10), None);
}

#[test]
fn this_test_process_is_found_in_the_real_proc() {
    let pid = std::process::id();
    let own_uid = std::fs::metadata("/proc/self").expect("procfs").uid();
    let reader = ProcessReader::new(Path::new("/"), own_uid, PAGE);
    let cpu = reader.cpu_samples().expect("procfs");
    let seen = cpu.iter().find(|sample| sample.pid == pid).expect("listed");
    let memory = reader.memory_samples().expect("procfs");
    assert!(
        memory
            .iter()
            .any(|sample| sample.pid == pid && sample.bytes > 0)
    );
    let info = reader.identify(pid).expect("running");
    assert_eq!(info.start_ticks, seen.start_ticks);
    assert!(info.name.starts_with("processes-"), "{}", info.name);
    assert!(info.can_quit);
}
