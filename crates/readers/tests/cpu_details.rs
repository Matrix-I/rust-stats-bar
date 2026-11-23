mod support;

use std::fs;
use std::path::Path;

use statsbar_readers::cpu_details::{CpuDetails, CpuDetailsReader};
use support::Tree;

const TEMPS: &str = "sys/class/hwmon/hwmon8";

/// Two cores with two threads each, numbered as this laptop numbers them.
fn laptop(name: &str) -> Tree {
    let tree = Tree::new(name);
    tree.write("sys/devices/system/cpu/online", "0-3\n");
    for (cpu, core, khz) in [
        (0, 0, 800_000),
        (1, 1, 1_000_000),
        (2, 0, 1_200_000),
        (3, 1, 1_400_000),
    ] {
        let dir = format!("sys/devices/system/cpu/cpu{cpu}");
        tree.write(&format!("{dir}/topology/core_id"), format!("{core}\n"));
        tree.write(
            &format!("{dir}/cpufreq/scaling_cur_freq"),
            format!("{khz}\n"),
        );
    }
    tree.write("sys/class/hwmon/hwmon1/name", "nvme\n");
    tree.write(&format!("{TEMPS}/name"), "coretemp\n");
    for (index, label, milli) in [
        (1, "Package id 0", 84000),
        (2, "Core 0", 82000),
        (3, "Core 1", 81000),
    ] {
        tree.write(&format!("{TEMPS}/temp{index}_label"), format!("{label}\n"));
        tree.write(&format!("{TEMPS}/temp{index}_input"), format!("{milli}\n"));
    }
    tree.write("proc/loadavg", "4.00 2.00 1.00 2/1738 706483\n");
    tree.write("proc/uptime", "125693.31 876543.21\n");
    tree.write("proc/cpuinfo", "processor\t: 0\nmodel name\t: i5-8365U\n");
    tree
}

/// The details, and what could not be read.
fn read(reader: &mut CpuDetailsReader) -> (CpuDetails, Vec<&'static str>) {
    let (details, problems) = reader.read();
    (
        details,
        problems.into_iter().map(|(what, _)| what).collect(),
    )
}

#[test]
fn every_figure_is_read_and_cores_are_paired_through_topology() {
    let tree = laptop("details-all");
    assert_eq!(
        read(&mut CpuDetailsReader::new(&tree.root)),
        (
            CpuDetails {
                model: Some("i5-8365U".to_owned()),
                package_celsius: Some(84.0),
                core_celsius: vec![Some(82.0), Some(81.0), Some(82.0), Some(81.0)],
                frequency_mhz: Some(1100.0),
                load_per_cpu: Some([1.0, 0.5, 0.25]),
                uptime_seconds: Some(125_693.31),
            },
            vec![]
        )
    );
}

#[test]
fn a_second_read_sees_new_values() {
    let tree = laptop("details-reread");
    let mut reader = CpuDetailsReader::new(&tree.root);
    read(&mut reader);
    tree.write(&format!("{TEMPS}/temp1_input"), "91000\n");
    tree.write("proc/loadavg", "8.00 2.00 1.00 2/1738 706483\n");
    let (details, _) = read(&mut reader);
    assert_eq!(details.package_celsius, Some(91.0));
    assert_eq!(details.load_per_cpu, Some([2.0, 0.5, 0.25]));
}

#[test]
fn an_offline_cpu_does_not_shift_the_others_temperatures() {
    let tree = laptop("details-offline");
    let mut reader = CpuDetailsReader::new(&tree.root);
    read(&mut reader);
    tree.write("sys/devices/system/cpu/online", "0,2-3\n");
    let (details, _) = read(&mut reader);
    assert_eq!(details.core_celsius, [Some(82.0), Some(82.0), Some(81.0)]);
    assert_eq!(
        details.load_per_cpu,
        Some([4.0 / 3.0, 2.0 / 3.0, 1.0 / 3.0])
    );
}

#[test]
fn without_coretemp_only_the_temperatures_are_missing() {
    let tree = laptop("details-no-coretemp");
    tree.write(&format!("{TEMPS}/name"), "k10temp\n");
    let (details, problems) = read(&mut CpuDetailsReader::new(&tree.root));
    assert_eq!(details.package_celsius, None);
    assert_eq!(details.core_celsius, [None; 4]);
    assert_eq!(details.frequency_mhz, Some(1100.0));
    assert_eq!(problems, ["temperatures"]);
}

#[test]
fn a_sensor_that_moves_to_another_hwmon_is_found_again() {
    let tree = laptop("details-renumbered");
    let mut reader = CpuDetailsReader::new(&tree.root);
    read(&mut reader);
    std::fs::rename(
        tree.root.join(TEMPS),
        tree.root.join("sys/class/hwmon/hwmon5"),
    )
    .expect("move");
    // The open file still answers, so make it fail as a removed sysfs attribute does.
    tree.write("sys/class/hwmon/hwmon5/temp1_input", "");
    assert_eq!(read(&mut reader).1, ["temperatures"]);
    tree.write("sys/class/hwmon/hwmon5/temp1_input", "70000\n");
    let (details, problems) = read(&mut reader);
    assert_eq!((details.package_celsius, problems), (Some(70.0), vec![]));
}

#[test]
fn without_cpufreq_only_the_frequency_is_missing() {
    let tree = laptop("details-no-cpufreq");
    let gone = "sys/devices/system/cpu/cpu2/cpufreq/scaling_cur_freq";
    std::fs::remove_file(tree.root.join(gone)).expect("remove");
    let (details, problems) = read(&mut CpuDetailsReader::new(&tree.root));
    assert_eq!(
        details.frequency_mhz, None,
        "not the mean of the other three"
    );
    assert_eq!(details.package_celsius, Some(84.0));
    assert_eq!(problems, ["frequency"]);
}

#[test]
fn an_empty_root_gives_no_figures_and_names_each_gap() {
    let tree = Tree::new("details-empty");
    let (details, problems) = read(&mut CpuDetailsReader::new(&tree.root));
    assert_eq!(details, CpuDetails::default());
    assert_eq!(
        problems,
        [
            "online CPUs",
            "model",
            "temperatures",
            "frequency",
            "load average",
            "uptime"
        ]
    );
}

/// This machine, or the CI runner: whatever it lacks may only be the optional sensors.
#[test]
fn the_real_root_gives_load_and_uptime_and_names_each_gap() {
    let (details, problems) = CpuDetailsReader::new(Path::new("/")).read();
    let gaps: Vec<&str> = problems.iter().map(|(what, _)| *what).collect();
    for gap in &gaps {
        assert!(
            ["temperatures", "frequency", "model"].contains(gap),
            "{gap}: {problems:?}"
        );
    }
    assert!(details.uptime_seconds.is_some_and(|seconds| seconds > 0.0));
    assert!(details.load_per_cpu.is_some());
    assert!(!details.core_celsius.is_empty(), "online CPUs are known");
    let coretemp = fs::read_dir("/sys/class/hwmon").is_ok_and(|devices| {
        devices.filter_map(Result::ok).any(|device| {
            fs::read_to_string(device.path().join("name"))
                .is_ok_and(|name| name.trim() == "coretemp")
        })
    });
    if coretemp {
        assert!(
            details
                .package_celsius
                .is_some_and(|celsius| (1.0..=125.0).contains(&celsius)),
            "{details:?}"
        );
    }
    if Path::new("/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq").exists() {
        assert!(
            details.frequency_mhz.is_some_and(|mhz| mhz > 0.0),
            "{details:?}"
        );
    }
}
