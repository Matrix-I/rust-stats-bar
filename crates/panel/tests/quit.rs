//! Quitting real child processes. Each test spawns its own `sleep` and reaps it.

use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::{Child, Command};

use rustix::io::Errno;
use statsbar_core::process::parse_stat;
use statsbar_panel::quit::{QuitError, quit};
use statsbar_panel::view::QuitTarget;

const PROC: &str = "/proc";
const SIGTERM: i32 = 15;

/// Kills and reaps on drop, so a failed assertion never leaks a process.
struct Sleeper(Child);

impl Sleeper {
    fn spawn() -> Sleeper {
        Sleeper(Command::new("sleep").arg("600").spawn().unwrap())
    }

    fn target(&self) -> QuitTarget {
        let pid = self.0.id();
        let text = std::fs::read_to_string(format!("{PROC}/{pid}/stat")).unwrap();
        QuitTarget {
            pid,
            start_ticks: parse_stat(&text).unwrap().start_ticks,
            name: "sleep".to_owned(),
        }
    }
}

impl Drop for Sleeper {
    fn drop(&mut self) {
        // Both fail only once the process is gone, which is the goal.
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn the_listed_process_gets_sigterm() {
    let mut sleeper = Sleeper::spawn();
    quit(Path::new(PROC), &sleeper.target()).unwrap();
    assert_eq!(sleeper.0.wait().unwrap().signal(), Some(SIGTERM));
}

#[test]
fn a_pid_with_another_start_time_is_left_alone() {
    let mut sleeper = Sleeper::spawn();
    let mut target = sleeper.target();
    target.start_ticks += 1;
    assert!(matches!(
        quit(Path::new(PROC), &target),
        Err(QuitError::Replaced)
    ));
    assert!(
        sleeper.0.try_wait().unwrap().is_none(),
        "the process must still run"
    );
}

#[test]
fn a_reaped_process_is_gone() {
    let mut sleeper = Sleeper::spawn();
    let target = sleeper.target();
    sleeper.0.kill().unwrap();
    sleeper.0.wait().unwrap();
    assert!(matches!(
        quit(Path::new(PROC), &target),
        Err(QuitError::Gone)
    ));
}

#[test]
fn a_pid_outside_the_kernels_range_is_gone() {
    for pid in [0, u32::MAX] {
        let target = QuitTarget {
            pid,
            start_ticks: 0,
            name: "x".to_owned(),
        };
        assert!(
            matches!(quit(Path::new(PROC), &target), Err(QuitError::Gone)),
            "{pid}"
        );
    }
}

#[test]
fn kernel_refusals_map_to_their_reasons() {
    assert!(matches!(QuitError::from(Errno::SRCH), QuitError::Gone));
    assert!(matches!(
        QuitError::from(Errno::PERM),
        QuitError::NotPermitted
    ));
    assert!(matches!(QuitError::from(Errno::INVAL), QuitError::Io(_)));
}
