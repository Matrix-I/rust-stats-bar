//! The real binary: the event loop serving pages over the socket. It runs on a private bus with
//! a stand-in tray watcher, its own config and runtime directory, and reads this machine's
//! `/proc`. The desktop never sees it.

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use statsbar_core::ipc::{CpuPage, Event, MemoryPage};
use zbus::zvariant::{OwnedValue, Value};

/// Generous for a local daemon; it only has to tell "slow" from "hung". Page events come 1 s
/// apart.
const LIMIT: Duration = Duration::from_secs(10);
/// Lines before the awaited one, and events before the awaited state.
const MAX_SKIPPED: usize = 16;
const WATCH_CPU: &str = "{\"type\":\"subscribe\",\"page\":\"cpu\"}\n";
const DBUSMENU: &str = "com.canonical.dbusmenu";
const WATCH_MEMORY: &str = "{\"type\":\"subscribe\",\"page\":\"memory\"}\n";

struct Watcher(mpsc::Sender<String>);

#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    fn register_status_notifier_item(&self, service: &str) {
        // The receiver lives until the test ends.
        let _ = self.0.send(service.to_owned());
    }

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        0
    }
}

/// Kills the process on drop, so a failed assertion never leaks one.
struct Reaped(Child);

impl Drop for Reaped {
    fn drop(&mut self) {
        // Either may fail only because the process has already exited, which is the goal.
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn lines(out: impl Read + Send + 'static) -> mpsc::Receiver<String> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    receiver
}

fn wait_for(lines: &mpsc::Receiver<String>, wanted: &str) -> String {
    for _ in 0..MAX_SKIPPED {
        match lines.recv_timeout(LIMIT) {
            Ok(line) if line.contains(wanted) => return line,
            Ok(_) => {}
            Err(error) => panic!("waiting for {wanted:?}: {error}"),
        }
    }
    panic!("{wanted:?} never came");
}

/// Whether the daemon holds `path` open.
fn holds(daemon: &Reaped, path: &str) -> bool {
    fs::read_dir(format!("/proc/{}/fd", daemon.0.id()))
        .expect("running")
        .filter_map(Result::ok)
        // A descriptor may close between the listing and the read.
        .filter_map(|entry| fs::read_link(entry.path()).ok())
        .any(|target| target == Path::new(path))
}

struct Session {
    _bus: Reaped,
    watcher: zbus::Connection,
    registered: mpsc::Receiver<String>,
    address: String,
    dir: PathBuf,
}

impl Session {
    /// `items` is the `[items]` table the daemon starts with.
    fn new(name: &str, items: &str) -> Session {
        // Short, for the socket path's 108-byte limit.
        let dir = std::env::temp_dir().join(format!("sbard-{}-{name}", std::process::id()));
        fs::create_dir(&dir).expect("create the session directory");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).expect("chmod");
        fs::create_dir_all(dir.join("config/statsbar")).expect("create the config directory");
        fs::write(dir.join("config/statsbar/config.toml"), items).expect("write the config");

        let mut bus = Reaped(
            Command::new("dbus-daemon")
                .args(["--session", "--nofork", "--print-address=1"])
                .stdout(Stdio::piped())
                .spawn()
                .expect("run /usr/bin/dbus-daemon (Ubuntu package dbus-daemon)"),
        );
        let address = wait_for(&lines(bus.0.stdout.take().expect("piped")), "unix:");
        let (sender, registered) = mpsc::channel();
        let watcher = async_io::block_on(async {
            zbus::connection::Builder::address(address.as_str())?
                .method_timeout(LIMIT)
                .name("org.kde.StatusNotifierWatcher")?
                .serve_at("/StatusNotifierWatcher", Watcher(sender))?
                .build()
                .await
        })
        .expect("serve the watcher");
        Session {
            _bus: bus,
            watcher,
            registered,
            address,
            dir,
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_statsbard"));
        command
            .env("DBUS_SESSION_BUS_ADDRESS", &self.address)
            // No system bus, so this machine's power profile cannot change the cadence.
            .env(
                "DBUS_SYSTEM_BUS_ADDRESS",
                format!("unix:path={}/none", self.dir.display()),
            )
            .env("XDG_CONFIG_HOME", self.dir.join("config"))
            .env("XDG_RUNTIME_DIR", &self.dir)
            .stderr(Stdio::piped());
        command
    }

    /// Starts the daemon and waits for its hub, by when the socket is listening. Returns the
    /// hub's bus name last.
    fn start(&self) -> (Reaped, mpsc::Receiver<String>, String) {
        let mut daemon = Reaped(self.command().spawn().expect("start statsbard"));
        let log = lines(daemon.0.stderr.take().expect("piped"));
        let hub = self
            .registered
            .recv_timeout(LIMIT)
            .expect("the hub registers");
        (daemon, log, hub)
    }

    /// Clicks the hub's menu row with this label, as the desktop does.
    fn click(&self, hub: &str, label: &str) {
        async_io::block_on(async {
            let reply = self
                .watcher
                .call_method(
                    Some(hub),
                    "/MenuBar",
                    Some(DBUSMENU),
                    "GetGroupProperties",
                    &(Vec::<i32>::new(), vec!["label"]),
                )
                .await
                .expect("list the menu");
            let rows: Vec<(i32, HashMap<String, OwnedValue>)> =
                reply.body().deserialize().expect("menu rows");
            let id = rows
                .iter()
                .find(|(_, properties)| {
                    properties
                        .get("label")
                        .and_then(|value| <&str>::try_from(value).ok())
                        == Some(label)
                })
                .expect("a row with the label")
                .0;
            self.watcher
                .call_method(
                    Some(hub),
                    "/MenuBar",
                    Some(DBUSMENU),
                    "Event",
                    &(id, "clicked", Value::from(0i32), 0u32),
                )
                .await
                .expect("click the row");
        });
    }

    fn panel(&self) -> Panel {
        let stream = UnixStream::connect(self.dir.join("statsbar.sock")).expect("connect");
        stream
            .set_read_timeout(Some(LIMIT))
            .expect("set the timeout");
        let mut panel = Panel {
            events: BufReader::new(stream.try_clone().expect("clone")),
            stream,
        };
        assert_eq!(panel.line(), "{\"protocol\":1}\n");
        panel.send("{\"protocol\":1}\n");
        panel
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // Only scratch files are left, and a failure here must not hide the test's own.
        let _ = fs::remove_dir_all(&self.dir);
    }
}

struct Panel {
    stream: UnixStream,
    events: BufReader<UnixStream>,
}

impl Panel {
    fn send(&mut self, text: &str) {
        self.stream.write_all(text.as_bytes()).expect("send");
    }

    fn line(&mut self) -> String {
        let mut line = String::new();
        self.events.read_line(&mut line).expect("read a line");
        line
    }

    fn event(&mut self) -> Event {
        serde_json::from_str(&self.line()).expect("an event")
    }

    fn cpu_until(&mut self, wanted: impl Fn(&CpuPage) -> bool) -> CpuPage {
        for _ in 0..MAX_SKIPPED {
            match self.event() {
                Event::Cpu(page) if wanted(&page) => return page,
                Event::Cpu(_) => {}
                other => panic!("not a CPU event: {other:?}"),
            }
        }
        panic!("the CPU page never got there");
    }

    /// Skips Memory events sent before the switch.
    fn cpu(&mut self) -> CpuPage {
        for _ in 0..MAX_SKIPPED {
            match self.event() {
                Event::Cpu(page) => return page,
                Event::Memory(_) => {}
                other => panic!("not a page: {other:?}"),
            }
        }
        panic!("no CPU event");
    }

    /// Skips CPU events sent before the switch.
    fn memory(&mut self) -> MemoryPage {
        for _ in 0..MAX_SKIPPED {
            match self.event() {
                Event::Memory(page) => return page,
                Event::Cpu(_) => {}
                other => panic!("not a page: {other:?}"),
            }
        }
        panic!("no memory event");
    }
}

const ALL_HIDDEN: &str =
    "[items]\ncpu = false\nmemory = false\nbattery = false\nfan = false\nnetwork = false\n";
const CPU_SHOWN: &str =
    "[items]\ncpu = true\nmemory = false\nbattery = false\nfan = false\nnetwork = false\n";

#[test]
fn pages_of_hidden_items_are_served_and_switched() {
    let session = Session::new("hidden", ALL_HIDDEN);
    let (daemon, _log, _hub) = session.start();
    let mut panel = session.panel();

    panel.send(WATCH_CPU);
    let first = panel.cpu_until(|_| true);
    assert_eq!(first.processes, None, "nothing to compare with yet");
    let page = panel.cpu_until(|page| page.load.is_some() && page.processes.is_some());
    let load = page.load.expect("just checked");
    assert!(!load.per_core_busy.is_empty());
    assert_eq!(page.core_temperatures_c.len(), load.per_core_busy.len());
    let processes = page.processes.expect("just checked");
    assert!((1..=6).contains(&processes.len()), "{processes:?}");
    assert!(page.uptime_seconds.is_some_and(|seconds| seconds > 0.0));
    assert!(!page.power_saver);

    panel.send(WATCH_MEMORY);
    let page = panel.memory();
    assert!(page.buckets.total > 0);
    assert!((1..=6).contains(&page.processes.expect("ranked at once").len()));
    // Only the Memory page follows the switch.
    assert!(matches!(panel.event(), Event::Memory(_)));

    let tasks = fs::read_dir(format!("/proc/{}/task", daemon.0.id())).expect("running");
    let mut threads: Vec<String> = tasks
        .map(|task| fs::read_to_string(task.expect("listed").path().join("comm")).expect("comm"))
        .map(|name| name.trim_end().to_owned())
        .filter(|name| !name.starts_with("blocking-"))
        .collect();
    threads.sort();
    assert_eq!(
        threads,
        ["async-io", "statsbard", "statsbard"],
        "the thread invariant"
    );

    // Back to CPU: the hidden item's page starts afresh, not from the page it showed before.
    panel.send(WATCH_CPU);
    let again = panel.cpu();
    assert_eq!((again.load, again.processes), (None, None));
}

#[test]
fn a_shown_items_page_is_sent_before_its_next_reading() {
    let session = Session::new("shown", CPU_SHOWN);
    let (_daemon, _log, _hub) = session.start();
    session
        .registered
        .recv_timeout(LIMIT)
        .expect("the CPU item registers");
    let mut first = session.panel();
    first.send(WATCH_CPU);
    let last = first.cpu_until(|page| page.load.is_some());
    let mut second = session.panel();
    // The next page is about a second away, so a page read before this is the welcome.
    let before = uptime_seconds();
    second.send(WATCH_CPU);
    let welcome = second.cpu_until(|_| true);
    assert!(welcome.load.is_some(), "the last load, at once");
    let welcome_at = welcome.uptime_seconds.expect("uptime");
    assert!(
        welcome_at <= before,
        "read at {welcome_at} s, after the subscribe at {before} s"
    );
    // Nothing was read for the newcomer: the first watcher's next page comes a full interval
    // later.
    let next = first.cpu_until(|_| true);
    let gap = next.uptime_seconds.expect("uptime") - last.uptime_seconds.expect("uptime");
    assert!(gap >= 0.5, "a page {gap} s after the one before");
}

/// The clock the pages' `uptime_seconds` come from.
fn uptime_seconds() -> f64 {
    let uptime = fs::read_to_string("/proc/uptime").expect("uptime");
    let seconds = uptime.split_whitespace().next().expect("a first field");
    seconds.parse().expect("seconds")
}

#[test]
fn hiding_an_item_with_its_page_open_keeps_the_page_live() {
    let session = Session::new("hiding", CPU_SHOWN);
    let (daemon, _log, hub) = session.start();
    session
        .registered
        .recv_timeout(LIMIT)
        .expect("the CPU item registers");
    let mut panel = session.panel();
    panel.send(WATCH_CPU);
    panel.cpu_until(|page| page.load.is_some());

    session.click(&hub, "Show CPU");
    let config = session.dir.join("config/statsbar/config.toml");
    // Saved last, so seeing it means the item has been hidden.
    panel.cpu_until(|page| {
        assert!(page.load.is_some(), "hiding dropped the page's baseline");
        fs::read_to_string(&config).is_ok_and(|saved| saved.contains("cpu = false"))
    });
    for _ in 0..2 {
        let page = panel.cpu_until(|_| true);
        assert!(page.load.is_some(), "hiding dropped the page's baseline");
    }
    assert!(holds(&daemon, "/proc/stat"));
}

#[test]
fn a_hidden_items_files_close_with_its_last_watcher() {
    let session = Session::new("closing", ALL_HIDDEN);
    let (daemon, _log, _hub) = session.start();
    let mut first = session.panel();
    first.send(WATCH_CPU);
    first.cpu_until(|page| page.load.is_some());
    assert!(holds(&daemon, "/proc/stat"), "read while watched");
    assert!(holds(&daemon, "/proc/loadavg"), "read while watched");

    // Leaves by closing the connection. Memory is read only after the loop has caught up with
    // that, so its first event means CPU's files had their chance to close.
    drop(first);
    let mut second = session.panel();
    second.send(WATCH_MEMORY);
    second.memory();
    assert!(!holds(&daemon, "/proc/stat"), "read with nobody watching");
    assert!(
        !holds(&daemon, "/proc/loadavg"),
        "read with nobody watching"
    );
}

#[test]
fn a_second_daemon_exits_and_a_killed_one_can_be_replaced() {
    let session = Session::new("single", ALL_HIDDEN);
    let (first, _log, _hub) = session.start();

    let mut second = Reaped(session.command().spawn().expect("start a second statsbard"));
    let log = lines(second.0.stderr.take().expect("piped"));
    let line = wait_for(&log, "already running");
    assert_eq!(line, "statsbard: StatsBar is already running");
    assert!(!second.0.wait().expect("exits").success());
    assert!(
        session.registered.try_recv().is_err(),
        "it added no tray item"
    );
    drop(session.panel());

    drop(first);
    assert!(
        session.dir.join("statsbar.sock").exists(),
        "a killed daemon leaves its socket"
    );
    let (_third, _log, _hub) = session.start();
    drop(session.panel());
}

#[test]
fn without_a_runtime_directory_it_says_so_and_exits() {
    let session = Session::new("no-runtime", ALL_HIDDEN);
    let mut daemon = Reaped(
        session
            .command()
            .env_remove("XDG_RUNTIME_DIR")
            .spawn()
            .expect("start statsbard"),
    );
    let log = lines(daemon.0.stderr.take().expect("piped"));
    wait_for(&log, "XDG_RUNTIME_DIR");
    assert!(!daemon.0.wait().expect("exits").success());
    assert!(
        session.registered.try_recv().is_err(),
        "it added no tray item"
    );
}
