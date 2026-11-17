use statsbar_core::cpu::{CoreTicks, CpuLoad, parse_proc_stat};

const STAT: &str = "\
cpu  300 20 100 1700 40 10 6 4 0 0
cpu0 100 0 40 850 10 5 3 2 0 0
cpu1 200 20 60 850 30 5 3 2 0 0
intr 12345 0 0
ctxt 6789
";

#[test]
fn parses_per_cpu_lines_and_skips_the_aggregate() {
    let cores = parse_proc_stat(STAT).expect("two cpu lines");
    assert_eq!(
        cores,
        vec![
            CoreTicks {
                user: 100,
                nice: 0,
                system: 50,
                idle: 860
            },
            CoreTicks {
                user: 200,
                nice: 20,
                system: 70,
                idle: 880
            },
        ]
    );
}

#[test]
fn accepts_the_four_column_form_of_old_kernels() {
    let cores = parse_proc_stat("cpu0 1 2 3 4\n").expect("four columns are enough");
    assert_eq!(
        cores,
        vec![CoreTicks {
            user: 1,
            nice: 2,
            system: 3,
            idle: 4
        }]
    );
}

#[test]
fn rejects_a_malformed_cpu_line_instead_of_shifting_cores() {
    assert_eq!(parse_proc_stat("cpu0 1 2 3 4\ncpu1 1 x 3 4\n"), None);
    assert_eq!(parse_proc_stat("cpu0 1 2 3\n"), None);
}

#[test]
fn rejects_text_without_per_cpu_lines() {
    assert_eq!(parse_proc_stat(""), None);
    assert_eq!(parse_proc_stat("cpu  1 2 3 4\n"), None);
}

#[test]
fn load_is_the_share_of_each_bucket_in_the_interval() {
    let previous = [CoreTicks {
        user: 100,
        nice: 0,
        system: 50,
        idle: 850,
    }];
    let current = [CoreTicks {
        user: 150,
        nice: 10,
        system: 70,
        idle: 870,
    }];
    let load = CpuLoad::between(&previous, &current).expect("same core count");
    assert_eq!(load.user_percent, 60.0); // nice is folded into user
    assert_eq!(load.system_percent, 20.0);
    assert_eq!(load.idle_percent, 20.0);
    assert_eq!(load.per_core_busy, vec![80.0]);
    assert_eq!(load.usage_percent(), 80.0);
}

#[test]
fn per_core_busy_is_independent_per_core() {
    let previous = [CoreTicks::default(), CoreTicks::default()];
    let current = [
        CoreTicks {
            user: 100,
            nice: 0,
            system: 0,
            idle: 0,
        },
        CoreTicks {
            user: 0,
            nice: 0,
            system: 0,
            idle: 100,
        },
    ];
    let load = CpuLoad::between(&previous, &current).expect("same core count");
    assert_eq!(load.per_core_busy, vec![100.0, 0.0]);
    assert_eq!(load.usage_percent(), 50.0);
}

#[test]
fn an_interval_with_no_ticks_reads_as_idle() {
    let ticks = [CoreTicks {
        user: 5,
        nice: 5,
        system: 5,
        idle: 5,
    }];
    let load = CpuLoad::between(&ticks, &ticks).expect("same core count");
    assert_eq!(load.idle_percent, 100.0);
    assert_eq!(load.usage_percent(), 0.0);
    assert_eq!(load.per_core_busy, vec![0.0]);
}

#[test]
fn a_counter_that_went_backwards_counts_as_no_progress() {
    let previous = [CoreTicks {
        user: 500,
        nice: 0,
        system: 0,
        idle: 100,
    }];
    let current = [CoreTicks {
        user: 10,
        nice: 0,
        system: 0,
        idle: 200,
    }];
    let load = CpuLoad::between(&previous, &current).expect("same core count");
    assert_eq!(load.idle_percent, 100.0);
    assert_eq!(load.per_core_busy, vec![0.0]);
}

#[test]
fn a_changed_core_count_yields_no_load() {
    let one = [CoreTicks::default()];
    let two = [CoreTicks::default(), CoreTicks::default()];
    assert_eq!(CpuLoad::between(&one, &two), None);
    assert_eq!(CpuLoad::between(&[], &[]), None);
}

#[test]
fn columns_that_would_overflow_saturate_instead_of_panicking() {
    let max = u64::MAX;
    let cores =
        parse_proc_stat(&format!("cpu0 1 2 {max} {max} {max} 1 1 1\n")).expect("numbers parse");
    assert_eq!(cores[0].system, u64::MAX);
    assert_eq!(cores[0].idle, u64::MAX);
}
