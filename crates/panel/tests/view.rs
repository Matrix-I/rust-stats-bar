//! The pages' figures and colours, from daemon events.

use statsbar_core::cpu::CpuLoad;
use statsbar_core::ipc::{CpuPage, CpuProcess, MemoryPage, MemoryProcess, ProcessInfo};
use statsbar_core::memory::{MemoryBuckets, Pressure};
use statsbar_core::system::Thermal;
use statsbar_panel::view::{
    MISSING, QuitTarget, Segment, Tone, cpu_view, memory_view, pressure_tone, temperature_tone,
    thermal_tone, usage_tone,
};

const GIB: u64 = 1 << 30;

fn process(pid: u32, name: &str, can_quit: bool) -> ProcessInfo {
    ProcessInfo {
        pid,
        start_ticks: u64::from(pid) * 10,
        name: name.to_owned(),
        can_quit,
    }
}

fn first_cpu_page() -> CpuPage {
    CpuPage {
        model: None,
        load: None,
        temperature_c: None,
        thermal: None,
        core_temperatures_c: Vec::new(),
        frequency_mhz: None,
        load_per_cpu: None,
        uptime_seconds: None,
        power_saver: false,
        processes: None,
    }
}

fn full_cpu_page() -> CpuPage {
    CpuPage {
        model: Some("Intel(R) Core(TM) i5-8250U CPU @ 1.60GHz".to_owned()),
        load: Some(CpuLoad {
            system_percent: 12.5,
            user_percent: 25.0,
            idle_percent: 62.5,
            per_core_busy: vec![50.0, 12.6],
        }),
        temperature_c: Some(57.0),
        thermal: Some(Thermal::Nominal),
        core_temperatures_c: vec![Some(56.0), Some(80.4)],
        frequency_mhz: Some(2345.6),
        load_per_cpu: Some([0.254, 0.3, 1.0]),
        uptime_seconds: Some(3_700.0),
        power_saver: false,
        processes: Some(vec![
            CpuProcess {
                process: process(42, "firefox", true),
                cpu_percent: 105.04,
            },
            CpuProcess {
                process: process(1, "systemd", false),
                cpu_percent: 0.0,
            },
        ]),
    }
}

#[test]
fn a_full_cpu_page_reads_as_upstream() {
    let view = cpu_view(&full_cpu_page());
    assert_eq!(
        view.subtitle.as_deref(),
        Some("Intel(R) Core(TM) i5-8250U CPU @ 1.60GHz")
    );
    let [usage, temperature] = &view.rings;
    // Half-way values round away from zero.
    assert_eq!(usage.center, "38%");
    assert_eq!(usage.caption, "Usage");
    assert_eq!(
        usage.segments,
        [
            Segment {
                fraction: 0.125,
                tone: Tone::System
            },
            Segment {
                fraction: 0.25,
                tone: Tone::User
            },
        ]
    );
    assert_eq!(temperature.center, "57°");
    assert_eq!(
        temperature.segments,
        [Segment {
            fraction: 0.57,
            tone: Tone::Good
        }]
    );
    let details: Vec<_> = view
        .details
        .iter()
        .map(|row| (row.label, row.value.as_str(), row.dot))
        .collect();
    assert_eq!(
        details,
        [
            ("System", "13%", Some(Tone::System)),
            ("User", "25%", Some(Tone::User)),
            ("Idle", "63%", Some(Tone::Dim)),
            ("Thermal", "Nominal", Some(Tone::Good)),
            ("Load avg / core (1/5/15m)", "0.25 / 0.30 / 1.00", None),
            ("Uptime", "1 hour, 1 minute", None),
        ]
    );
    assert_eq!(view.cores.len(), 2);
    assert_eq!(view.cores[0].busy, 0.5);
    assert_eq!(view.cores[0].busy_text, "50%");
    // Quiet below 75 °C, the ring's colours above it.
    assert_eq!(
        view.cores[0].temperature,
        Some(("56°".to_owned(), Tone::Dim))
    );
    assert_eq!(
        view.cores[1].temperature,
        Some(("80°".to_owned(), Tone::Warning))
    );
    let frequency = view.frequency.unwrap();
    assert_eq!(
        (frequency.label, frequency.value.as_str()),
        ("All cores", "2346 MHz")
    );
    let processes = view.processes.unwrap();
    assert_eq!(processes[0].name, "firefox");
    assert_eq!(processes[0].value, "105.0%");
    assert_eq!(
        processes[0].quit,
        Some(QuitTarget {
            pid: 42,
            start_ticks: 420,
            name: "firefox".to_owned()
        })
    );
    assert_eq!(processes[1].value, "0.0%");
    assert_eq!(processes[1].quit, None);
}

#[test]
fn the_first_cpu_page_shows_dashes_and_reading() {
    let view = cpu_view(&first_cpu_page());
    assert_eq!(view.subtitle, None);
    for ring in &view.rings {
        assert_eq!(ring.center, MISSING);
        assert!(ring.segments.is_empty());
    }
    let values: Vec<_> = view
        .details
        .iter()
        .map(|row| (row.label, row.value.as_str()))
        .collect();
    assert_eq!(
        values,
        [
            ("System", MISSING),
            ("User", MISSING),
            ("Idle", MISSING),
            ("Thermal", MISSING),
            ("Uptime", MISSING),
        ]
    );
    assert_eq!(view.details[3].dot, Some(Tone::Dim));
    assert!(view.cores.is_empty());
    assert_eq!(view.frequency, None);
    assert_eq!(view.processes, None);
}

#[test]
fn power_saver_adds_its_row_before_the_load() {
    let page = CpuPage {
        power_saver: true,
        ..full_cpu_page()
    };
    let labels: Vec<_> = cpu_view(&page)
        .details
        .iter()
        .map(|row| row.label)
        .collect();
    assert_eq!(labels[4], "Power saver");
    assert_eq!(cpu_view(&page).details[4].value, "On");
    assert_eq!(labels[5], "Load avg / core (1/5/15m)");
}

#[test]
fn a_core_without_a_sensor_or_past_the_list_has_no_temperature() {
    let page = CpuPage {
        core_temperatures_c: vec![None],
        ..full_cpu_page()
    };
    let view = cpu_view(&page);
    assert_eq!(view.cores[0].temperature, None);
    assert_eq!(view.cores[1].temperature, None);
}

#[test]
fn a_reading_past_full_scale_fills_the_ring_and_no_more() {
    let page = CpuPage {
        temperature_c: Some(105.0),
        ..full_cpu_page()
    };
    let view = cpu_view(&page);
    assert_eq!(view.rings[1].center, "105°");
    assert_eq!(
        view.rings[1].segments,
        [Segment {
            fraction: 1.0,
            tone: Tone::Bad
        }]
    );
}

#[test]
fn temperature_tones_change_at_75_and_90() {
    assert_eq!(temperature_tone(74.9), Tone::Good);
    assert_eq!(temperature_tone(75.0), Tone::Warning);
    assert_eq!(temperature_tone(89.9), Tone::Warning);
    assert_eq!(temperature_tone(90.0), Tone::Bad);
}

#[test]
fn every_level_has_its_tone_and_missing_psi_is_dim() {
    assert_eq!(thermal_tone(Thermal::Nominal), Tone::Good);
    assert_eq!(thermal_tone(Thermal::Fair), Tone::Warning);
    assert_eq!(thermal_tone(Thermal::Serious), Tone::Warning);
    assert_eq!(thermal_tone(Thermal::Critical), Tone::Bad);
    assert_eq!(pressure_tone(None), Tone::Dim);
    assert_eq!(pressure_tone(Some(Pressure::Normal)), Tone::Good);
    assert_eq!(pressure_tone(Some(Pressure::Warning)), Tone::Warning);
    assert_eq!(pressure_tone(Some(Pressure::Critical)), Tone::Bad);
}

#[test]
fn usage_tones_change_at_70_and_85() {
    assert_eq!(usage_tone(69.9), Tone::Good);
    assert_eq!(usage_tone(70.0), Tone::Warning);
    assert_eq!(usage_tone(84.9), Tone::Warning);
    assert_eq!(usage_tone(85.0), Tone::Bad);
}

fn memory_page(compressed: u64, pressure: Option<Pressure>) -> MemoryPage {
    MemoryPage {
        buckets: MemoryBuckets {
            total: 16 * GIB,
            app: 6 * GIB,
            kernel: 2 * GIB,
            cached: 4 * GIB,
            free: 4 * GIB,
            swap_used: GIB / 2,
            compressed,
        },
        pressure,
        processes: Some(vec![MemoryProcess {
            process: process(7, "code", true),
            bytes: 300 * (1 << 20),
        }]),
    }
}

#[test]
fn a_memory_page_reads_as_upstream() {
    let view = memory_view(&memory_page(GIB, Some(Pressure::Normal)));
    assert_eq!(view.subtitle, "16.00 GB total");
    let [pressure, usage] = &view.rings;
    assert_eq!(pressure.center, "Normal");
    assert_eq!(pressure.center_tone, Some(Tone::Good));
    // (kernel + zswap) / total.
    assert_eq!(
        pressure.segments,
        [Segment {
            fraction: 3.0 / 16.0,
            tone: Tone::Good
        }]
    );
    assert_eq!(usage.center, "50%");
    assert_eq!(
        usage.segments,
        [Segment {
            fraction: 0.5,
            tone: Tone::Good
        }]
    );
    assert_eq!(view.used, "8.00 GB");
    let bar: Vec<_> = view
        .bar
        .iter()
        .map(|segment| (segment.tone, segment.fraction))
        .collect();
    assert_eq!(
        bar,
        [
            (Tone::App, 0.375),
            (Tone::Kernel, 0.125),
            (Tone::Cached, 0.25),
            (Tone::Free, 0.25)
        ]
    );
    let details: Vec<_> = view
        .details
        .iter()
        .map(|row| (row.label, row.value.as_str()))
        .collect();
    assert_eq!(
        details,
        [
            ("App", "6.00 GB"),
            ("Kernel", "2.00 GB"),
            ("Cached", "4.00 GB"),
            ("Free", "4.00 GB"),
            ("Compressed", "1.00 GB"),
            ("Swap", "0.50 GB"),
        ]
    );
    let processes = view.processes.unwrap();
    assert_eq!(
        (processes[0].name.as_str(), processes[0].value.as_str()),
        ("code", "300 MB")
    );
    assert_eq!(processes[0].quit.as_ref().map(|target| target.pid), Some(7));
}

#[test]
fn without_zswap_or_psi_the_row_is_left_out_and_pressure_is_a_dash() {
    let view = memory_view(&memory_page(0, None));
    assert!(view.details.iter().all(|row| row.label != "Compressed"));
    assert_eq!(view.rings[0].center, MISSING);
    assert_eq!(view.rings[0].center_tone, Some(Tone::Dim));
    assert_eq!(view.rings[0].segments[0].tone, Tone::Dim);
}

#[test]
fn an_unknown_total_gives_empty_fractions() {
    let page = MemoryPage {
        buckets: MemoryBuckets::default(),
        pressure: None,
        processes: None,
    };
    let view = memory_view(&page);
    assert!(view.bar.iter().all(|segment| segment.fraction == 0.0));
    assert_eq!(view.rings[1].center, "0%");
    assert_eq!(view.processes, None);
}
