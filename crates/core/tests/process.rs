use statsbar_core::process::{
    CpuSample, CpuUse, MemorySample, can_quit, display_name, parse_stat, parse_statm, rank_by_cpu,
    rank_by_memory,
};

const SECOND: u64 = 1_000_000_000;
const HZ: u64 = 100;

fn stat_line(comm: &str, utime: u64, stime: u64, start: u64) -> String {
    format!(
        "4242 ({comm}) S 1 4242 4242 0 -1 4194304 521 0 0 0 {utime} {stime} 0 0 20 0 1 0 {start} 18214912 1871"
    )
}

fn sample(pid: u32, start_ticks: u64, cpu_ticks: u64) -> CpuSample {
    CpuSample {
        pid,
        start_ticks,
        cpu_ticks,
    }
}

fn memory(pid: u32, bytes: u64) -> MemorySample {
    MemorySample { pid, bytes }
}

#[test]
fn stat_yields_the_name_the_cpu_time_and_the_start_time() {
    let stat = parse_stat(&stat_line("cat", 30, 12, 9001)).expect("well-formed");
    assert_eq!(stat.comm, "cat");
    assert_eq!(stat.cpu_ticks, 42);
    assert_eq!(stat.start_ticks, 9001);
}

#[test]
fn a_name_with_spaces_and_parentheses_does_not_shift_the_fields() {
    let stat = parse_stat(&stat_line("a) R 1 (b c", 5, 6, 7)).expect("well-formed");
    assert_eq!(stat.comm, "a) R 1 (b c");
    assert_eq!((stat.cpu_ticks, stat.start_ticks), (11, 7));
}

#[test]
fn a_cut_or_non_numeric_stat_is_none() {
    assert_eq!(parse_stat(""), None);
    assert_eq!(parse_stat("4242 (cat) S 1 2 3"), None);
    assert_eq!(parse_stat("4242 cat S 1 2 3"), None);
    assert_eq!(parse_stat(")("), None);
    let broken = stat_line("cat", 30, 12, 9001).replace(" 9001 ", " x ");
    assert_eq!(parse_stat(&broken), None);
}

#[test]
fn statm_memory_is_resident_minus_shared_in_bytes() {
    assert_eq!(
        parse_statm("4447 1882 1537 1232 0 140 0\n", 4096),
        Some(345 * 4096)
    );
    assert_eq!(
        parse_statm("10 2 5 0 0 0 0", 4096),
        Some(0),
        "shared above resident"
    );
    assert_eq!(parse_statm("4447 1882", 4096), None);
    assert_eq!(
        parse_statm("1 18446744073709551615 0", 4096),
        None,
        "overflow"
    );
}

#[test]
fn cpu_is_the_delta_over_the_interval_with_one_core_as_100() {
    let previous = [sample(1, 10, 1000), sample(2, 20, 500)];
    let current = [sample(1, 10, 1050), sample(2, 20, 800)];
    assert_eq!(
        rank_by_cpu(&previous, &current, 2 * SECOND, HZ, 6),
        [
            CpuUse {
                pid: 2,
                start_ticks: 20,
                percent: 150.0
            },
            CpuUse {
                pid: 1,
                start_ticks: 10,
                percent: 25.0
            },
        ]
    );
}

#[test]
fn equal_cpu_ties_break_on_the_lower_pid() {
    let previous = [sample(9, 1, 0), sample(3, 1, 0), sample(5, 1, 0)];
    let current = [sample(9, 1, 10), sample(3, 1, 10), sample(5, 1, 10)];
    let pids: Vec<u32> = rank_by_cpu(&previous, &current, SECOND, HZ, 6)
        .iter()
        .map(|process| process.pid)
        .collect();
    assert_eq!(pids, [3, 5, 9]);
}

#[test]
fn a_vanished_a_new_and_a_reused_pid_are_left_out() {
    let previous = [sample(1, 10, 100), sample(2, 20, 100), sample(3, 30, 100)];
    // 1 exited, 2 is another process under the same pid, 4 is new.
    let current = [sample(2, 99, 900), sample(3, 30, 110), sample(4, 40, 500)];
    assert_eq!(
        rank_by_cpu(&previous, &current, SECOND, HZ, 6),
        [CpuUse {
            pid: 3,
            start_ticks: 30,
            percent: 10.0
        }]
    );
}

#[test]
fn cpu_time_that_went_backwards_reads_as_zero() {
    assert_eq!(
        rank_by_cpu(&[sample(1, 1, 50)], &[sample(1, 1, 40)], SECOND, HZ, 6),
        [CpuUse {
            pid: 1,
            start_ticks: 1,
            percent: 0.0
        }]
    );
}

#[test]
fn cpu_ranking_without_an_interval_or_a_tick_rate_is_empty() {
    let both = [sample(1, 1, 1)];
    assert!(rank_by_cpu(&both, &both, 0, HZ, 6).is_empty());
    assert!(rank_by_cpu(&both, &both, SECOND, 0, 6).is_empty());
}

#[test]
fn cpu_ranking_keeps_the_busiest_count() {
    let previous: Vec<_> = (1..=8).map(|pid| sample(pid, 1, 0)).collect();
    let current: Vec<_> = (1..=8).map(|pid| sample(pid, 1, u64::from(pid))).collect();
    let pids: Vec<u32> = rank_by_cpu(&previous, &current, SECOND, HZ, 6)
        .iter()
        .map(|process| process.pid)
        .collect();
    assert_eq!(pids, [8, 7, 6, 5, 4, 3]);
}

#[test]
fn memory_ranks_heaviest_first_with_ties_on_the_lower_pid() {
    let samples = [memory(7, 100), memory(2, 300), memory(4, 100), memory(1, 5)];
    assert_eq!(
        rank_by_memory(&samples, 3),
        [memory(2, 300), memory(4, 100), memory(7, 100)]
    );
    assert!(rank_by_memory(&samples, 0).is_empty());
}

#[test]
fn the_executable_name_beats_the_truncated_kernel_name() {
    assert_eq!(
        display_name(Some("gnome-keyring-daemon"), "gnome-keyring-d"),
        "gnome-keyring-daemon"
    );
    assert_eq!(display_name(Some("node"), "MainThread"), "node");
}

#[test]
fn a_replaced_executable_keeps_its_name() {
    assert_eq!(
        display_name(Some("sample-editor-app (deleted)"), "sample-editor-a"),
        "sample-editor-app"
    );
}

#[test]
fn without_an_executable_the_kernel_name_is_shown() {
    assert_eq!(display_name(None, "kworker/0:1"), "kworker/0:1");
    assert_eq!(display_name(Some("  "), "cat"), "cat");
    assert_eq!(display_name(None, ""), "—");
}

#[test]
fn control_characters_never_reach_a_row() {
    assert_eq!(display_name(None, "two\nlines"), "two\u{fffd}lines");
}

#[test]
fn only_the_users_own_processes_can_be_quit() {
    assert!(can_quit(500, 1000, 1000, Some("firefox"), "firefox"));
    assert!(!can_quit(500, 0, 1000, None, "sshd"));
    assert!(!can_quit(500, 1001, 1000, None, "firefox"));
    assert!(!can_quit(1, 1000, 1000, None, "init"), "pid 1");
}

#[test]
fn the_session_and_statsbar_cannot_be_quit() {
    for name in [
        "gnome-shell",
        "systemd",
        "(sd-pam)",
        "statsbard",
        "statsbar-panel",
    ] {
        assert!(
            !can_quit(500, 1000, 1000, None, name),
            "{name} by kernel name"
        );
        assert!(
            !can_quit(500, 1000, 1000, Some(name), "x"),
            "{name} by executable"
        );
    }
    assert!(!can_quit(500, 1000, 1000, Some("statsbard (deleted)"), "x"));
    assert!(
        can_quit(
            500,
            1000,
            1000,
            Some("gnome-shell-calendar-server"),
            "gnome-shell-cal"
        ),
        "a protected name only matches whole"
    );
}
