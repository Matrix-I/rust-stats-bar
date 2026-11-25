//! Starting `statsbar-panel` from a menu click. The panel is a GTK application with one window
//! per session: a second launch hands its page to the running one over D-Bus and exits.
//!
//! The daemon keeps each child until it has exited and been reaped, so none is left a zombie,
//! and refuses a click while too many are still running.

use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use statsbar_core::ipc::Page;

const PANEL: &str = "statsbar-panel";
/// One window, plus launches still handing over a page. More means something is stuck.
pub const MAX_RUNNING: usize = 4;
/// GTK's renderer, when the user has not chosen one: the lighter of the two measured in
/// Plan 04b's run (see its spike record).
pub const RENDERER: &str = "cairo";

pub struct Launcher {
    program: PathBuf,
    /// `GSK_RENDERER` for the child.
    renderer: OsString,
    running: Vec<Child>,
}

impl Launcher {
    /// `inherited_renderer` is the daemon's own `GSK_RENDERER`: a choice the user made, which
    /// the child keeps. Without one the child gets [`RENDERER`].
    pub fn new(program: PathBuf, inherited_renderer: Option<OsString>) -> Launcher {
        Launcher {
            program,
            renderer: inherited_renderer.unwrap_or_else(|| RENDERER.into()),
            running: Vec::new(),
        }
    }

    /// The panel installed beside this daemon's executable.
    pub fn beside(daemon: PathBuf, inherited_renderer: Option<OsString>) -> Launcher {
        Launcher::new(daemon.with_file_name(PANEL), inherited_renderer)
    }

    pub fn open(&mut self, page: Page) -> io::Result<()> {
        self.reap();
        if self.running.len() >= MAX_RUNNING {
            return Err(io::Error::other(format!(
                "{MAX_RUNNING} panels are still running"
            )));
        }
        let mut command = Command::new(&self.program);
        command
            .args(["--page", page_arg(page)])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .env("GSK_RENDERER", &self.renderer);
        self.running.push(command.spawn()?);
        Ok(())
    }

    /// Collects the children that have exited. Cheap enough for every turn of the main loop.
    pub fn reap(&mut self) {
        self.running.retain_mut(|child| match child.try_wait() {
            Ok(None) => true,
            Ok(Some(_)) => false,
            Err(error) => {
                // Only an already reaped child fails here; keeping it would count it forever.
                eprintln!("statsbard: waiting for the panel failed: {error}");
                false
            }
        });
    }

    pub fn running(&self) -> usize {
        self.running.len()
    }
}

fn page_arg(page: Page) -> &'static str {
    match page {
        Page::Cpu => "cpu",
        Page::Memory => "memory",
    }
}
