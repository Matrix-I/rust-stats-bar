//! Launching a stand-in panel: a shell script that records its arguments and environment.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use statsbar_core::ipc::Page;
use statsbard::launch::{Launcher, MAX_RUNNING, RENDERER};

/// Only fails a hung test.
const LIMIT: Duration = Duration::from_secs(10);

/// Held while a stand-in is written and while one is started. A child forked by another test
/// while a script is open for writing keeps that file open until it execs, and running the
/// script meanwhile fails with ETXTBSY.
static FORKS: Mutex<()> = Mutex::new(());

fn open(launcher: &mut Launcher, page: Page) -> std::io::Result<()> {
    let _forks = FORKS.lock().unwrap();
    launcher.open(page)
}

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        let path =
            std::env::temp_dir().join(format!("statsbard-launch-{}-{name}", std::process::id()));
        // Left over only by a killed earlier run.
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).unwrap();
        Dir(path)
    }

    /// A panel that writes `$@` and `GSK_RENDERER` to `out`, then waits for `release` to exist,
    /// or for the directory to go, so no stand-in outlives its test.
    fn panel(&self) -> PathBuf {
        let _forks = FORKS.lock().unwrap();
        let script = self.0.join("statsbar-panel");
        fs::write(
            &script,
            format!(
                "#!/bin/sh\necho \"$* ${{GSK_RENDERER-unset}}\" >> {out}\nwhile [ -d {dir} ] && [ ! -e {release} ]; do sleep 0.01; done\n",
                out = self.out().display(),
                dir = self.0.display(),
                release = self.0.join("release").display(),
            ),
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        script
    }

    fn out(&self) -> PathBuf {
        self.0.join("out")
    }

    fn release(&self) {
        fs::write(self.0.join("release"), "").unwrap();
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        self.release();
        // Best effort: a leftover directory in the temp dir harms nothing.
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Waits until `out` has `lines` lines.
fn recorded(out: &Path, lines: usize) -> Vec<String> {
    let deadline = Instant::now() + LIMIT;
    loop {
        let text = fs::read_to_string(out).unwrap_or_default();
        let got: Vec<String> = text.lines().map(str::to_owned).collect();
        if got.len() >= lines {
            return got;
        }
        assert!(Instant::now() < deadline, "the panel never ran: {got:?}");
        std::thread::yield_now();
    }
}

fn reaped(launcher: &mut Launcher) {
    let deadline = Instant::now() + LIMIT;
    while launcher.running() > 0 {
        assert!(Instant::now() < deadline, "the children were never reaped");
        launcher.reap();
        std::thread::yield_now();
    }
}

#[test]
fn the_page_is_passed_with_the_default_renderer() {
    let dir = Dir::new("page");
    let mut launcher = Launcher::new(dir.panel(), None);
    open(&mut launcher, Page::Memory).unwrap();
    open(&mut launcher, Page::Cpu).unwrap();
    let mut lines = recorded(&dir.out(), 2);
    lines.sort();
    assert_eq!(
        lines,
        [
            format!("--page cpu {RENDERER}"),
            format!("--page memory {RENDERER}")
        ]
    );
}

#[test]
fn a_renderer_the_user_chose_is_inherited_not_replaced() {
    let dir = Dir::new("inherit");
    let mut launcher = Launcher::new(dir.panel(), Some("ngl".into()));
    open(&mut launcher, Page::Cpu).unwrap();
    assert_eq!(recorded(&dir.out(), 1), ["--page cpu ngl"]);
}

#[test]
fn exited_panels_are_reaped_and_free_their_place() {
    let dir = Dir::new("reap");
    let mut launcher = Launcher::new(dir.panel(), None);
    for _ in 0..MAX_RUNNING {
        open(&mut launcher, Page::Cpu).unwrap();
    }
    assert_eq!(launcher.running(), MAX_RUNNING);
    let refused = open(&mut launcher, Page::Cpu).unwrap_err();
    assert_eq!(refused.to_string(), "4 panels are still running");
    dir.release();
    reaped(&mut launcher);
    open(&mut launcher, Page::Cpu).unwrap();
    assert_eq!(launcher.running(), 1);
    // Before the directory goes, or the last stand-in cannot read its script.
    recorded(&dir.out(), MAX_RUNNING + 1);
}

#[test]
fn a_missing_panel_is_an_error_and_takes_no_place() {
    let dir = Dir::new("missing");
    let mut launcher = Launcher::beside(dir.0.join("statsbard"), None);
    let error = open(&mut launcher, Page::Cpu).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert_eq!(launcher.running(), 0);
}

#[test]
fn the_panel_is_looked_for_beside_the_daemon() {
    let dir = Dir::new("beside");
    dir.panel();
    let mut launcher = Launcher::beside(dir.0.join("statsbard"), None);
    open(&mut launcher, Page::Memory).unwrap();
    assert_eq!(
        recorded(&dir.out(), 1),
        [format!("--page memory {RENDERER}")]
    );
}
