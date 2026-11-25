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

use futures_lite::{StreamExt, future};
use statsbar_core::ipc::{CpuPage, Event, MemoryPage};
use zbus::fdo::{DBusProxy, NameOwnerChangedStream};
use zbus::zvariant::{OwnedValue, Value};

/// Generous for a local daemon; it only has to tell "slow" from "hung". Page events come 1 s
/// apart.
const LIMIT: Duration = Duration::from_secs(10);
/// Lines before the awaited one, and events before the awaited state.
const MAX_SKIPPED: usize = 16;
const WATCH_CPU: &str = "{\"type\":\"subscribe\",\"page\":\"cpu\"}\n";
const DBUSMENU: &str = "com.canonical.dbusmenu";
const WATCH_MEMORY: &str = "{\"type\":\"subscribe\",\"page\":\"memory\"}\n";
const REFRESH: &str = "{\"type\":\"refresh\"}\n";
const UNWATCH: &str = "{\"type\":\"unsubscribe\"}\n";
/// The daemon ignores a refresh sooner than this after the page's last reading.
const MIN_REFRESH: Duration = Duration::from_millis(500);

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

/// The daemon's descriptors open on `path`, by number.
fn descriptors(daemon: &Reaped, path: &str) -> Vec<String> {
    let mut found: Vec<String> = fs::read_dir(format!("/proc/{}/fd", daemon.0.id()))
        .expect("running")
        .filter_map(Result::ok)
        // A descriptor may close between the listing and the read.
        .filter(|entry| fs::read_link(entry.path()).is_ok_and(|target| target == Path::new(path)))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    found.sort();
    found
}

/// The read calls the daemon has made so far, over all its threads.
fn reads(daemon: &Reaped) -> u64 {
    let io = fs::read_to_string(format!("/proc/{}/io", daemon.0.id())).expect("running");
    io.lines()
        .find_map(|line| line.strip_prefix("syscr: "))
        .expect("a syscr line")
        .parse()
        .expect("a count")
}

/// Whether the daemon scanned the processes since it had made `since` reads. A scan reads every
/// process's `statm`, so it makes at least one read per process, where a reading without one
/// makes under ten.
fn scanned(daemon: &Reaped, since: u64) -> bool {
    let processes = fs::read_dir("/proc")
        .expect("/proc")
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().parse::<u32>().is_ok())
        .count();
    reads(daemon) - since >= u64::try_from(processes).expect("fits")
}

/// Waits until the name followed by `changes` has no owner.
fn released(changes: &mut NameOwnerChangedStream) {
    async_io::block_on(future::or(
        async {
            for _ in 0..MAX_SKIPPED {
                let change = changes.next().await.expect("the bus stays up");
                if change.args().expect("arguments").new_owner().is_none() {
                    return;
                }
            }
            panic!("the name was never released");
        },
        async {
            async_io::Timer::after(LIMIT).await;
            panic!("waiting for the name's release: timed out");
        },
    ));
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

    /// The menu's layout revision and its entries' ids, labels and visibility, in id order.
    fn menu(&self, item: &str) -> (u32, Vec<(i32, String, bool)>) {
        async_io::block_on(async {
            let layout = self
                .watcher
                .call_method(
                    Some(item),
                    "/MenuBar",
                    Some(DBUSMENU),
                    "GetLayout",
                    &(0i32, -1i32, Vec::<&str>::new()),
                )
                .await
                .expect("read the layout");
            type Layout = (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>);
            let (revision, _): (u32, Layout) = layout.body().deserialize().expect("a layout");
            let reply = self
                .watcher
                .call_method(
                    Some(item),
                    "/MenuBar",
                    Some(DBUSMENU),
                    "GetGroupProperties",
                    &(Vec::<i32>::new(), vec!["label", "visible"]),
                )
                .await
                .expect("list the menu");
            let rows: Vec<(i32, HashMap<String, OwnedValue>)> =
                reply.body().deserialize().expect("menu rows");
            let mut entries: Vec<(i32, String, bool)> = rows
                .into_iter()
                .map(|(id, properties)| {
                    let label = properties
                        .get("label")
                        .and_then(|value| <&str>::try_from(value).ok())
                        .unwrap_or_default()
                        .to_owned();
                    // dbusmenu leaves out a property at its default, and `visible` defaults to true.
                    let visible = properties
                        .get("visible")
                        .and_then(|value| bool::try_from(value).ok())
                        .unwrap_or(true);
                    (id, label, visible)
                })
                .collect();
            entries.sort();
            (revision, entries)
        })
    }

    /// Follows who owns `name`, from now on.
    fn owner_changes(&self, name: &str) -> NameOwnerChangedStream {
        async_io::block_on(async {
            DBusProxy::new(&self.watcher)
                .await?
                .receive_name_owner_changed_with_args(&[(0, name)])
                .await
        })
        .expect("follow the name")
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
    let mut seen = vec![first.cpu_until(|page| page.load.is_some())];
    let mut second = session.panel();
    // Leaving at once, so only the welcome can reach the newcomer: the daemon answers each
    // request before it reads the next.
    second.send(&format!("{WATCH_CPU}{UNWATCH}"));
    let welcome = second.cpu_until(|_| true);
    assert!(welcome.load.is_some(), "the last load, at once");

    // The welcome is the page last published: the first watcher has it already, or gets it next
    // when a reading landed before the subscribe.
    let welcome_at = welcome.uptime_seconds.expect("uptime");
    while seen.last().and_then(|page| page.uptime_seconds) < Some(welcome_at) {
        assert!(seen.len() < MAX_SKIPPED, "the welcome was never published");
        seen.push(first.cpu_until(|_| true));
    }
    assert_eq!(
        seen.last(),
        Some(&welcome),
        "the welcome is a published page"
    );
    // Nothing was read for the newcomer: such a reading would reach the first watcher too, sooner
    // than a full interval after the page before it.
    seen.push(first.cpu_until(|_| true));
    for pair in seen.windows(2) {
        let gap = pair[1].uptime_seconds.expect("uptime") - pair[0].uptime_seconds.expect("uptime");
        assert!(gap >= 0.5, "a page {gap} s after the one before");
    }
}

#[test]
fn hiding_an_item_with_its_page_open_keeps_the_page_live() {
    let session = Session::new("hiding", CPU_SHOWN);
    let (daemon, _log, hub) = session.start();
    let item = session
        .registered
        .recv_timeout(LIMIT)
        .expect("the CPU item registers");
    let mut item_owner = session.owner_changes(&item);
    let mut panel = session.panel();
    panel.send(WATCH_CPU);
    panel.cpu_until(|page| page.load.is_some());
    let open = descriptors(&daemon, "/proc/stat");
    assert_eq!(open.len(), 1, "one reader: {open:?}");

    // Just after a reading, so the next one, which would reopen a closed file, is about a second
    // away. The item's name goes after the hide has dealt with the reader.
    session.click(&hub, "Show CPU");
    released(&mut item_owner);
    assert_eq!(
        descriptors(&daemon, "/proc/stat"),
        open,
        "hiding closed the page's reader"
    );
    for _ in 0..2 {
        let page = panel.cpu_until(|_| true);
        assert!(page.load.is_some(), "hiding dropped the page's baseline");
    }
    assert_eq!(
        descriptors(&daemon, "/proc/stat"),
        open,
        "the page's reader was reopened"
    );
}

#[test]
fn a_refresh_rescans_the_memory_page_once_half_a_second_has_passed() {
    let session = Session::new("refresh", ALL_HIDDEN);
    let (daemon, _log, _hub) = session.start();
    let mut panel = session.panel();
    panel.send(WATCH_MEMORY);
    // The first reading scans the processes. The next is due 1 s later and scans only 2 s later.
    panel.memory();
    let since = reads(&daemon);
    // Only time lets a refresh count, and nothing marks the moment. That reading came before
    // this point, so it is at least 500 ms old after the wait, with the next one ~500 ms away.
    std::thread::sleep(MIN_REFRESH);
    panel.send(REFRESH);
    let early = panel.memory();
    assert!(
        scanned(&daemon, since),
        "no early reading with a scan: the regular one 1 s on does not scan"
    );

    // Read a moment ago: the refresh is ignored, and the next reading, a regular one, does not
    // scan either.
    let since = reads(&daemon);
    panel.send(REFRESH);
    let next = panel.memory();
    assert!(
        !scanned(&daemon, since),
        "an ignored refresh brought the scan forward"
    );
    assert_eq!(
        next.processes, early.processes,
        "the refresh's ranking, kept"
    );
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

#[test]
fn a_runtime_directory_others_can_read_is_refused() {
    let session = Session::new("shared", ALL_HIDDEN);
    fs::set_permissions(&session.dir, fs::Permissions::from_mode(0o750)).expect("chmod");
    let mut daemon = Reaped(session.command().spawn().expect("start statsbard"));
    let log = lines(daemon.0.stderr.take().expect("piped"));
    let line = wait_for(&log, "private");
    assert_eq!(
        line,
        format!(
            "statsbard: cannot open the panel's socket: {} is not a directory private to this user",
            session.dir.display()
        )
    );
    assert!(!daemon.0.wait().expect("exits").success());
    assert!(
        session.registered.try_recv().is_err(),
        "it added no tray item"
    );
    assert!(!session.dir.join("statsbar.sock").exists(), "nor a socket");
}

#[test]
fn the_cpu_menu_keeps_its_ids_while_its_rows_fill_in() {
    let session = Session::new("menu", CPU_SHOWN);
    let (_daemon, _log, _hub) = session.start();
    let cpu = session
        .registered
        .recv_timeout(LIMIT)
        .expect("the CPU item registers");
    let (revision, before) = session.menu(&cpu);
    // The first reading only has a baseline; rows come with the second.
    let mut panel = session.panel();
    panel.send(WATCH_CPU);
    panel.cpu_until(|page| page.load.is_some());
    panel.cpu_until(|_| true);
    let (revision_after, after) = session.menu(&cpu);
    assert_eq!(revision_after, revision, "the layout never changed");
    let ids =
        |entries: &[(i32, String, bool)]| entries.iter().map(|entry| entry.0).collect::<Vec<_>>();
    assert_eq!(ids(&after), ids(&before));
    let open = after
        .iter()
        .find(|(_, label, _)| label == "Open details…")
        .expect("Open details…");
    assert!(open.2, "Open details… is shown");
    let usage = after
        .iter()
        .find(|(_, label, _)| label.starts_with("Usage "))
        .expect("the usage row");
    assert!(usage.2, "the filled rows are shown");
}
