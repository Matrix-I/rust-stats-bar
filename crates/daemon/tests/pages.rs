//! The page snapshots, against a `/proc` and `/sys` written by each test.

use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use statsbar_core::cpu::CpuLoad;
use statsbar_core::ipc::Page;
use statsbar_core::memory::{MemoryBuckets, Pressure};
use statsbar_core::system::Thermal;
use statsbard::pages::{Pages, System};

const SECOND: u64 = 1_000_000_000;

struct Machine {
    root: PathBuf,
}

impl Machine {
    /// Two CPUs, coretemp at 91 °C and eight processes, all readable.
    fn new(name: &str) -> Machine {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("pages-{name}"));
        match fs::remove_dir_all(&root) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => panic!("clearing {}: {error}", root.display()),
        }
        let machine = Machine { root };
        machine.write("sys/devices/system/cpu/online", "0-1\n");
        for cpu in 0..2 {
            let dir = format!("sys/devices/system/cpu/cpu{cpu}");
            machine.write(&format!("{dir}/topology/core_id"), "0\n");
            machine.write(&format!("{dir}/cpufreq/scaling_cur_freq"), "1900000\n");
        }
        let temps = "sys/class/hwmon/hwmon0";
        machine.write(&format!("{temps}/name"), "coretemp\n");
        machine.write(&format!("{temps}/temp1_label"), "Package id 0\n");
        machine.write(&format!("{temps}/temp1_input"), "91000\n");
        machine.write(&format!("{temps}/temp2_label"), "Core 0\n");
        machine.write(&format!("{temps}/temp2_input"), "88000\n");
        machine.write("proc/loadavg", "2.00 1.00 0.50 1/100 9\n");
        machine.write("proc/uptime", "3600.00 7000.00\n");
        machine.write("proc/cpuinfo", "model name\t: i5\n");
        machine.psi("12.00");
        for pid in 11..19 {
            machine.process(pid, 1, 100, pid);
        }
        machine
    }

    fn write(&self, path: &str, contents: &str) {
        let path = self.root.join(path);
        fs::create_dir_all(path.parent().expect("has a parent")).expect("create the directory");
        fs::write(path, contents).expect("write");
    }

    fn psi(&self, some: &str) {
        self.write(
            "proc/pressure/memory",
            &format!("some avg10={some} avg60=0 avg300=0 total=1\nfull avg10=0.00 avg60=0 avg300=0 total=1\n"),
        );
    }

    fn process(&self, pid: u32, start: u64, cpu_ticks: u64, private_pages: u32) {
        self.write(
            &format!("proc/{pid}/stat"),
            &format!(
                "{pid} (p{pid}) S 1 1 1 0 -1 0 0 0 0 0 {cpu_ticks} 0 0 0 20 0 1 0 {start} 0 0\n"
            ),
        );
        let resident = private_pages + 5;
        self.write(
            &format!("proc/{pid}/statm"),
            &format!("900 {resident} 5 0 0 0 0\n"),
        );
    }

    fn pages(&self) -> Pages {
        let own_uid = fs::metadata(&self.root).expect("the root exists").uid();
        let system = System {
            own_uid,
            page_size: 4096,
            ticks_per_second: 100,
        };
        Pages::new(&self.root, system)
    }
}

fn load(cores: usize) -> CpuLoad {
    CpuLoad {
        system_percent: 10.0,
        user_percent: 20.0,
        idle_percent: 70.0,
        per_core_busy: vec![30.0; cores],
    }
}

fn buckets() -> MemoryBuckets {
    MemoryBuckets {
        total: 1000,
        app: 500,
        free: 500,
        ..MemoryBuckets::default()
    }
}

#[test]
fn the_first_cpu_page_has_the_details_and_no_ranking_yet() {
    let machine = Machine::new("cpu-first");
    let (page, problems) = machine.pages().cpu(0, None, true);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(page.model.as_deref(), Some("i5"));
    assert_eq!((page.load, page.processes), (None, None));
    assert_eq!(
        (page.temperature_c, page.thermal),
        (Some(91.0), Some(Thermal::Serious))
    );
    assert_eq!(page.core_temperatures_c, [Some(88.0), Some(88.0)]);
    assert_eq!(page.frequency_mhz, Some(1900.0));
    assert_eq!(page.load_per_cpu, Some([1.0, 0.5, 0.25]));
    assert_eq!(page.uptime_seconds, Some(3600.0));
    assert!(page.power_saver);
}

#[test]
fn the_second_scan_ranks_the_six_busiest() {
    let machine = Machine::new("cpu-rank");
    let mut pages = machine.pages();
    pages.cpu(0, None, false);
    for pid in 11..19 {
        machine.process(pid, 1, 100 + u64::from(pid), pid);
    }
    let (page, _) = pages.cpu(SECOND, Some(&load(2)), false);
    assert_eq!(page.load, Some(load(2)));
    let rows: Vec<_> = page
        .processes
        .expect("ranked")
        .into_iter()
        .map(|row| (row.process.name, row.cpu_percent, row.process.start_ticks))
        .collect();
    let expected: Vec<_> = (13..19)
        .rev()
        .map(|pid| (format!("p{pid}"), f64::from(pid), 1))
        .collect();
    assert_eq!(rows, expected);
}

#[test]
fn a_scan_too_soon_keeps_the_last_ranking_and_baseline() {
    let machine = Machine::new("cpu-soon");
    let mut pages = machine.pages();
    pages.cpu(0, None, false);
    machine.process(18, 1, 150, 18);
    let (soon, _) = pages.cpu(SECOND / 2 - 1, None, false);
    assert_eq!(soon.processes, None, "no figure from a tiny interval");
    let (page, _) = pages.cpu(SECOND, None, false);
    assert_eq!(page.processes.expect("ranked")[0].cpu_percent, 50.0);
}

#[test]
fn temperatures_are_dropped_when_the_load_is_of_other_cpus() {
    let machine = Machine::new("cpu-hotplug");
    let (page, _) = machine.pages().cpu(0, Some(&load(3)), false);
    assert_eq!(page.core_temperatures_c, [None; 3]);
}

#[test]
fn closing_the_cpu_page_forgets_its_baseline() {
    let machine = Machine::new("cpu-close");
    let mut pages = machine.pages();
    pages.cpu(0, None, false);
    pages.cpu(SECOND, None, false);
    pages.close(Page::Cpu);
    let (page, _) = pages.cpu(60 * SECOND, None, false);
    assert_eq!(page.processes, None);
}

#[test]
fn the_memory_page_has_pressure_and_the_six_heaviest_at_once() {
    let machine = Machine::new("memory-first");
    let (page, problems) = machine.pages().memory(0, buckets());
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(
        (page.buckets, page.pressure),
        (buckets(), Some(Pressure::Warning))
    );
    let rows: Vec<_> = page
        .processes
        .expect("ranked")
        .into_iter()
        .map(|row| (row.process.pid, row.bytes))
        .collect();
    let expected: Vec<_> = (13..19)
        .rev()
        .map(|pid| (pid, u64::from(pid) * 4096))
        .collect();
    assert_eq!(rows, expected);
}

#[test]
fn memory_processes_are_rescanned_every_two_seconds_or_on_refresh() {
    let machine = Machine::new("memory-cadence");
    let mut pages = machine.pages();
    let top = |pages: &mut Pages, at: u64| {
        pages.memory(at, buckets()).0.processes.expect("ranked")[0]
            .process
            .pid
    };
    assert_eq!(top(&mut pages, 0), 18);
    machine.process(11, 1, 100, 500);
    assert_eq!(top(&mut pages, 2 * SECOND - 1), 18);
    assert_eq!(top(&mut pages, 2 * SECOND), 11);
    machine.process(12, 1, 100, 900);
    pages.refresh(Page::Memory);
    assert_eq!(top(&mut pages, 2 * SECOND + 1), 12);
}

#[test]
fn closing_the_memory_page_forgets_its_ranking() {
    let machine = Machine::new("memory-close");
    let mut pages = machine.pages();
    pages.memory(0, buckets());
    machine.process(11, 1, 100, 500);
    pages.close(Page::Memory);
    let (page, _) = pages.memory(1, buckets());
    assert_eq!(page.processes.expect("ranked")[0].process.pid, 11);
}

#[test]
fn missing_psi_is_none_and_reported_once_until_it_recovers() {
    let machine = Machine::new("memory-psi");
    let psi = machine.root.join("proc/pressure/memory");
    fs::remove_file(&psi).expect("remove");
    let mut pages = machine.pages();
    let named = |(page, problems): (
        statsbar_core::ipc::MemoryPage,
        Vec<statsbard::pages::Problem>,
    )| {
        let names: Vec<_> = problems.into_iter().map(|(what, _)| what).collect();
        (page.pressure, names)
    };
    assert_eq!(
        named(pages.memory(0, buckets())),
        (None, vec!["memory pressure"])
    );
    assert_eq!(named(pages.memory(SECOND, buckets())), (None, vec![]));
    machine.psi("0.00");
    assert_eq!(
        named(pages.memory(2 * SECOND, buckets())),
        (Some(Pressure::Normal), vec![])
    );
    machine.psi("broken");
    assert_eq!(
        named(pages.memory(3 * SECOND, buckets())),
        (None, vec!["memory pressure"])
    );
}

#[test]
fn a_failed_scan_keeps_the_last_ranking_and_reports_once() {
    let machine = Machine::new("memory-scan");
    let mut pages = machine.pages();
    let (first, _) = pages.memory(0, buckets());
    for pid in 11..19 {
        fs::remove_dir_all(machine.root.join(format!("proc/{pid}"))).expect("remove");
    }
    let (page, problems) = pages.memory(2 * SECOND, buckets());
    assert_eq!(page.processes, first.processes);
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].0, "processes");
    assert!(pages.memory(4 * SECOND, buckets()).1.is_empty());
}

#[test]
fn a_failed_cpu_scan_keeps_the_last_ranking_and_reports_once() {
    let machine = Machine::new("cpu-scan");
    let mut pages = machine.pages();
    pages.cpu(0, None, false);
    machine.process(18, 1, 150, 18);
    let (ranked, _) = pages.cpu(SECOND, None, false);
    assert!(ranked.processes.is_some());
    for pid in 11..19 {
        fs::remove_dir_all(machine.root.join(format!("proc/{pid}"))).expect("remove");
    }
    let (page, problems) = pages.cpu(2 * SECOND, None, false);
    assert_eq!(page.processes, ranked.processes);
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].0, "processes");
    assert!(pages.cpu(3 * SECOND, None, false).1.is_empty());
}

#[test]
fn a_pid_that_changed_hands_between_scans_leaves_the_ranking() {
    let machine = Machine::new("cpu-pid-reuse");
    let mut pages = machine.pages();
    pages.cpu(0, None, false);
    // Pid 18 is a new process now: later start, and far more CPU time than the old one had.
    machine.process(18, 2, 5000, 18);
    machine.process(17, 1, 150, 17);
    let (page, _) = pages.cpu(SECOND, None, false);
    let rows: Vec<_> = page
        .processes
        .expect("ranked")
        .into_iter()
        .map(|row| (row.process.pid, row.cpu_percent))
        .collect();
    assert_eq!(rows[0], (17, 50.0));
    assert!(rows.iter().all(|(pid, _)| *pid != 18), "{rows:?}");
}

#[test]
fn a_scan_too_soon_after_a_ranking_keeps_that_ranking() {
    let machine = Machine::new("cpu-soon-after-ranking");
    let mut pages = machine.pages();
    pages.cpu(0, None, false);
    machine.process(18, 1, 150, 18);
    let (ranked, _) = pages.cpu(SECOND, None, false);
    assert!(ranked.processes.is_some());
    machine.process(17, 1, 400, 17);
    let (soon, _) = pages.cpu(SECOND + SECOND / 2 - 1, None, false);
    assert_eq!(soon.processes, ranked.processes);
}
