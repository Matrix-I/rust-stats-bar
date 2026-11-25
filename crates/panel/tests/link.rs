//! The panel's client against a stand-in daemon on a socket in a temporary directory.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use statsbar_core::ipc::{ErrorReason, Event, MemoryPage, Page, Request};
use statsbar_core::memory::MemoryBuckets;
use statsbar_panel::link::{Closed, Incoming, Link, LinkError, MAX_EVENT_BYTES, socket_path};

/// Only fails a hung test; nothing waits for it to pass.
const LIMIT: Duration = Duration::from_secs(10);
const HELLO: &str = "{\"protocol\":1}\n";

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        let path =
            std::env::temp_dir().join(format!("statsbar-link-{}-{name}", std::process::id()));
        // Left over only by a killed earlier run.
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).unwrap();
        Dir(path)
    }

    fn socket(&self) -> PathBuf {
        self.0.join("statsbar.sock")
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        // Best effort: a leftover directory in the temp dir harms nothing.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone, Copy)]
enum Then {
    /// Close the connection after the script.
    Close,
    /// Keep reading until the panel closes, and return what it sent.
    Listen,
}

/// A daemon that writes `script` once the client's first line has come.
fn daemon(dir: &Dir, script: String, then: Then) -> thread::JoinHandle<Vec<String>> {
    let listener = UnixListener::bind(dir.socket()).unwrap();
    thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(LIMIT)).unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut first = String::new();
        reader.read_line(&mut first).unwrap();
        let mut received = vec![first];
        (&stream).write_all(script.as_bytes()).unwrap();
        if let Then::Listen = then {
            received.extend(reader.lines().map(Result::unwrap));
        }
        received
    })
}

fn next(link: &Link) -> Incoming {
    let (sender, receiver) = mpsc::channel();
    let events = link.events.clone();
    thread::spawn(move || {
        // The receiver lives until the test ends.
        let _ = sender.send(events.recv_blocking());
    });
    receiver.recv_timeout(LIMIT).unwrap().unwrap()
}

fn memory_line(app: u64) -> String {
    let event = Event::Memory(MemoryPage {
        buckets: MemoryBuckets {
            total: 100,
            app,
            ..MemoryBuckets::default()
        },
        pressure: None,
        processes: None,
    });
    format!("{}\n", serde_json::to_string(&event).unwrap())
}

#[test]
fn the_socket_lives_in_the_runtime_directory() {
    assert_eq!(
        socket_path(Some("/run/user/1000".as_ref())).unwrap(),
        PathBuf::from("/run/user/1000/statsbar.sock")
    );
    assert!(matches!(socket_path(None), Err(LinkError::NoRuntimeDir)));
    assert!(matches!(
        socket_path(Some("relative".as_ref())),
        Err(LinkError::NoRuntimeDir)
    ));
}

#[test]
fn no_daemon_reads_as_not_running() {
    let dir = Dir::new("absent");
    assert!(matches!(
        Link::connect(&dir.socket(), LIMIT),
        Err(LinkError::NotRunning(_))
    ));
}

#[test]
fn hellos_are_exchanged_and_requests_follow_until_the_link_is_dropped() {
    let dir = Dir::new("hello");
    let daemon = daemon(&dir, HELLO.to_owned(), Then::Listen);
    let link = Link::connect(&dir.socket(), LIMIT).unwrap();
    link.send(&Request::Subscribe { page: Page::Cpu }).unwrap();
    link.send(&Request::Refresh).unwrap();
    // The stand-in returns only at end of file, so this also proves the drop closes the socket.
    drop(link);
    assert_eq!(
        daemon.join().unwrap(),
        [
            HELLO,
            "{\"type\":\"subscribe\",\"page\":\"cpu\"}",
            "{\"type\":\"refresh\"}"
        ]
    );
}

#[test]
fn events_arrive_in_order_and_the_end_is_reported() {
    let dir = Dir::new("events");
    let script = format!("{HELLO}{}{}", memory_line(40), memory_line(41));
    let _daemon = daemon(&dir, script, Then::Close);
    let link = Link::connect(&dir.socket(), LIMIT).unwrap();
    for app in [40, 41] {
        let Incoming::Event(event) = next(&link) else {
            panic!("expected an event");
        };
        let Event::Memory(page) = *event else {
            panic!("expected a memory page");
        };
        assert_eq!(page.buckets.app, app);
    }
    assert_eq!(next(&link), Incoming::Closed(Closed::Eof));
}

#[test]
fn an_error_line_closes_with_its_reason() {
    let dir = Dir::new("busy");
    let script = format!("{HELLO}{{\"type\":\"error\",\"reason\":\"busy\",\"message\":\"x\"}}\n");
    let _daemon = daemon(&dir, script, Then::Close);
    let link = Link::connect(&dir.socket(), LIMIT).unwrap();
    assert_eq!(
        next(&link),
        Incoming::Closed(Closed::Refused(ErrorReason::Busy))
    );
}

#[test]
fn another_protocol_version_is_refused() {
    let dir = Dir::new("version");
    let _daemon = daemon(&dir, "{\"protocol\":2}\n".to_owned(), Then::Close);
    assert!(matches!(
        Link::connect(&dir.socket(), LIMIT),
        Err(LinkError::VersionMismatch { daemon: 2 })
    ));
}

#[test]
fn a_first_line_that_is_not_a_hello_is_a_failed_handshake() {
    let dir = Dir::new("not-hello");
    let _daemon = daemon(&dir, memory_line(1), Then::Close);
    assert!(matches!(
        Link::connect(&dir.socket(), LIMIT),
        Err(LinkError::Handshake(_))
    ));
}

#[test]
fn a_daemon_that_never_says_hello_times_out() {
    let dir = Dir::new("silent");
    let daemon = daemon(&dir, String::new(), Then::Listen);
    assert!(matches!(
        Link::connect(&dir.socket(), Duration::from_millis(50)),
        Err(LinkError::Handshake(_))
    ));
    // The failed connect closed its socket, so the stand-in saw only the hello.
    assert_eq!(daemon.join().unwrap(), [HELLO]);
}

#[test]
fn a_line_over_the_limit_or_not_an_event_ends_the_link() {
    let oversized = "a".repeat(usize::try_from(MAX_EVENT_BYTES).unwrap() + 1);
    for (name, bad) in [
        ("long", oversized),
        ("garbage", "{\"type\":\"nope\"}".to_owned()),
    ] {
        let dir = Dir::new(name);
        let script = format!("{HELLO}{}{bad}\n{}", memory_line(7), memory_line(8));
        let _daemon = daemon(&dir, script, Then::Close);
        let link = Link::connect(&dir.socket(), LIMIT).unwrap();
        assert!(
            matches!(next(&link), Incoming::Event(event) if matches!(*event, Event::Memory(_))),
            "{name}"
        );
        assert_eq!(next(&link), Incoming::Closed(Closed::BadLine), "{name}");
    }
}

#[test]
fn every_end_says_what_happened() {
    let cases = [
        (Closed::Eof, "StatsBar quit or restarted"),
        (
            Closed::Refused(ErrorReason::Busy),
            "StatsBar already serves as many panels as it can",
        ),
        (
            Closed::Refused(ErrorReason::VersionMismatch),
            "StatsBar speaks another protocol version; restart StatsBar",
        ),
        (
            Closed::Refused(ErrorReason::BadRequest),
            "StatsBar closed the connection",
        ),
        (
            Closed::BadLine,
            "StatsBar sent something this panel cannot read",
        ),
        (
            Closed::Io(std::io::ErrorKind::BrokenPipe),
            "the connection to StatsBar failed: broken pipe",
        ),
    ];
    for (closed, text) in cases {
        assert_eq!(closed.to_string(), text);
    }
}
