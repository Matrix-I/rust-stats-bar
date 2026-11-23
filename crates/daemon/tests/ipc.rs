//! The socket server, over real Unix sockets.

use std::fs;
use std::future::Future;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use async_io::Async;
use futures_lite::future;
use futures_lite::io::{AsyncReadExt, AsyncWriteExt};
use statsbar_core::ipc::{ErrorReason, Event, Hello, MemoryPage, Page};
use statsbar_core::memory::MemoryBuckets;
use statsbard::ipc::{
    ACCEPTS_PER_TURN, BindError, Change, LINES_PER_TURN, MAX_CLIENTS, MAX_REQUEST_BYTES, Options,
    Server,
};

/// Generous for a local socket; it only has to tell "slow" from "hung".
const LIMIT: Duration = Duration::from_secs(5);
const HELLO: &str = "{\"protocol\":1}\n";
const WATCH_CPU: &str = "{\"type\":\"subscribe\",\"page\":\"cpu\"}\n";
const WATCH_MEMORY: &str = "{\"type\":\"subscribe\",\"page\":\"memory\"}\n";

/// A short private directory: a socket path is limited to 108 bytes, which a path under
/// `CARGO_TARGET_TMPDIR` can exceed.
struct RuntimeDir(PathBuf);

impl RuntimeDir {
    fn new(name: &str) -> RuntimeDir {
        let dir = std::env::temp_dir().join(format!("sbar-{}-{name}", std::process::id()));
        fs::create_dir(&dir).expect("create the runtime directory");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).expect("chmod");
        RuntimeDir(dir)
    }

    fn socket(&self) -> PathBuf {
        self.0.join("statsbar.sock")
    }
}

impl Drop for RuntimeDir {
    fn drop(&mut self) {
        // Only scratch files are left, and a failure here must not hide the test's own.
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn options() -> Options {
    Options {
        allowed_uid: rustix::process::geteuid().as_raw(),
        handshake_timeout: LIMIT,
    }
}

fn bind(dir: &RuntimeDir) -> Server {
    Server::bind(&dir.0, options()).expect("bind")
}

async fn within<T>(what: &str, work: impl Future<Output = T>) -> T {
    future::or(work, async {
        async_io::Timer::after(LIMIT).await;
        panic!("{what} timed out after {LIMIT:?}")
    })
    .await
}

/// Runs `work` while the server runs. The server must report no change meanwhile.
async fn serve<T>(server: &mut Server, work: impl Future<Output = T>) -> T {
    within(
        "serving",
        future::or(work, async {
            let change = server.next().await;
            panic!("unexpected {change:?}")
        }),
    )
    .await
}

async fn change(server: &mut Server) -> Change {
    within("a change", server.next()).await
}

struct Client {
    stream: Async<UnixStream>,
}

impl Client {
    async fn connect(dir: &RuntimeDir) -> Client {
        let stream = Async::<UnixStream>::connect(dir.socket())
            .await
            .expect("connect");
        Client { stream }
    }

    async fn greeted(dir: &RuntimeDir, server: &mut Server) -> Client {
        let mut client = Client::connect(dir).await;
        assert_eq!(
            serve(server, client.line()).await.as_deref(),
            Some(HELLO.trim_end())
        );
        client.send(HELLO).await;
        client
    }

    async fn watching(dir: &RuntimeDir, server: &mut Server, line: &str, page: Page) -> Client {
        let mut client = Client::greeted(dir, server).await;
        client.send(line).await;
        assert_eq!(change(server).await, Change::Subscribed(page));
        client
    }

    async fn send(&mut self, text: &str) {
        self.stream.write_all(text.as_bytes()).await.expect("send");
    }

    /// The next line, or `None` once the daemon has closed the connection.
    async fn line(&mut self) -> Option<String> {
        let mut line = Vec::new();
        let mut byte = [0];
        loop {
            match self.stream.read(&mut byte).await {
                Ok(1) if byte[0] == b'\n' => return Some(String::from_utf8(line).expect("UTF-8")),
                Ok(1) => line.push(byte[0]),
                // Closing with a request still unread resets instead of ending; both are closed.
                _ => return None,
            }
        }
    }

    async fn refused(&mut self, server: &mut Server, expected: ErrorReason) {
        let line = serve(server, self.line()).await.expect("an error line");
        match serde_json::from_str(&line).expect("an event") {
            Event::Error { reason, .. } => assert_eq!(reason, expected),
            other => panic!("not an error: {other:?}"),
        }
        assert_eq!(
            serve(server, self.line()).await,
            None,
            "closed after the error"
        );
    }
}

fn memory_event(total: u64) -> Event {
    Event::Memory(MemoryPage {
        buckets: MemoryBuckets {
            total,
            ..MemoryBuckets::default()
        },
        pressure: None,
        processes: None,
    })
}

#[test]
fn the_socket_is_private_and_removed_on_drop() {
    let dir = RuntimeDir::new("private");
    let server = bind(&dir);
    let mode = fs::metadata(dir.socket())
        .expect("the socket exists")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    drop(server);
    assert!(!dir.socket().exists());
}

#[test]
fn a_second_server_is_refused_and_the_first_keeps_working() {
    let dir = RuntimeDir::new("second");
    let mut server = bind(&dir);
    assert!(matches!(
        Server::bind(&dir.0, options()),
        Err(BindError::AlreadyRunning)
    ));
    async_io::block_on(async {
        Client::greeted(&dir, &mut server).await;
    });
}

#[test]
fn a_socket_left_by_a_killed_daemon_is_replaced() {
    let dir = RuntimeDir::new("stale");
    drop(std::os::unix::net::UnixListener::bind(dir.socket()).expect("leave a socket behind"));
    assert!(dir.socket().exists());
    let mut server = bind(&dir);
    async_io::block_on(async {
        Client::greeted(&dir, &mut server).await;
    });
}

#[test]
fn a_file_that_is_not_a_socket_is_left_alone() {
    let dir = RuntimeDir::new("not-socket");
    fs::write(dir.socket(), "keep").expect("write");
    assert!(matches!(
        Server::bind(&dir.0, options()),
        Err(BindError::Io(_))
    ));
    assert_eq!(
        fs::read_to_string(dir.socket()).expect("still there"),
        "keep"
    );
}

#[test]
fn a_missing_runtime_directory_is_an_error() {
    let dir = RuntimeDir::new("missing");
    let result = Server::bind(&dir.0.join("none"), options());
    assert!(matches!(result, Err(BindError::Io(_))));
}

#[test]
fn subscribing_replaces_the_page_and_unsubscribing_clears_it() {
    let dir = RuntimeDir::new("subscribe");
    let mut server = bind(&dir);
    async_io::block_on(async {
        let mut client = Client::watching(&dir, &mut server, WATCH_CPU, Page::Cpu).await;
        assert!(server.watched(Page::Cpu) && !server.watched(Page::Memory));
        client.send(WATCH_MEMORY).await;
        assert_eq!(change(&mut server).await, Change::Subscribed(Page::Memory));
        assert!(!server.watched(Page::Cpu) && server.watched(Page::Memory));
        client.send("{\"type\":\"unsubscribe\"}\n").await;
        assert_eq!(change(&mut server).await, Change::Unwatched);
        assert!(!server.watched(Page::Memory));
    });
}

#[test]
fn requests_that_arrive_together_are_taken_in_order() {
    let dir = RuntimeDir::new("pipelined");
    let mut server = bind(&dir);
    async_io::block_on(async {
        let mut client = Client::connect(&dir).await;
        client
            .send(&format!(
                "{HELLO}{WATCH_CPU}{{\"type\":\"refresh\"}}\n{WATCH_MEMORY}"
            ))
            .await;
        assert_eq!(change(&mut server).await, Change::Subscribed(Page::Cpu));
        assert_eq!(change(&mut server).await, Change::Refresh(Page::Cpu));
        assert_eq!(change(&mut server).await, Change::Subscribed(Page::Memory));
    });
}

#[test]
fn an_event_reaches_only_the_page_it_is_for() {
    let dir = RuntimeDir::new("publish");
    let mut server = bind(&dir);
    async_io::block_on(async {
        let mut cpu = Client::watching(&dir, &mut server, WATCH_CPU, Page::Cpu).await;
        let mut first = Client::watching(&dir, &mut server, WATCH_MEMORY, Page::Memory).await;
        let mut second = Client::watching(&dir, &mut server, WATCH_MEMORY, Page::Memory).await;
        server.publish(Page::Memory, &memory_event(7));
        let expected = serde_json::to_string(&memory_event(7)).expect("serialises");
        assert_eq!(
            serve(&mut server, first.line()).await,
            Some(expected.clone())
        );
        assert_eq!(serve(&mut server, second.line()).await, Some(expected));
        // The first line the CPU watcher gets is the CPU page's, so it never got the other.
        let marker = Event::Error {
            reason: ErrorReason::Busy,
            message: "marker".to_owned(),
        };
        server.publish(Page::Cpu, &marker);
        let expected = serde_json::to_string(&marker).expect("serialises");
        assert_eq!(serve(&mut server, cpu.line()).await, Some(expected));
    });
}

#[test]
fn only_a_new_watcher_is_welcomed() {
    let dir = RuntimeDir::new("welcome");
    let mut server = bind(&dir);
    async_io::block_on(async {
        let mut old = Client::watching(&dir, &mut server, WATCH_MEMORY, Page::Memory).await;
        server.publish(Page::Memory, &memory_event(1));
        let mut new = Client::watching(&dir, &mut server, WATCH_MEMORY, Page::Memory).await;
        server.welcome(Page::Memory, &memory_event(1));
        // Welcomed once: a page published since then is current for everyone.
        server.publish(Page::Memory, &memory_event(2));
        server.welcome(Page::Memory, &memory_event(2));
        server.publish(Page::Memory, &memory_event(3));
        let line = |total| Some(serde_json::to_string(&memory_event(total)).expect("serialises"));
        for total in [1, 2, 3] {
            assert_eq!(serve(&mut server, old.line()).await, line(total));
        }
        for total in [1, 2, 3] {
            assert_eq!(serve(&mut server, new.line()).await, line(total));
        }
    });
}

#[test]
fn a_client_with_a_backlog_does_not_starve_the_others() {
    let dir = RuntimeDir::new("backlog");
    let mut server = bind(&dir);
    async_io::block_on(async {
        // Both connections and every line are queued before the server takes any of them.
        let mut busy = Client::connect(&dir).await;
        let mut other = Client::connect(&dir).await;
        let idle = "{\"type\":\"unsubscribe\"}\n".repeat(LINES_PER_TURN * 10);
        busy.send(&format!("{HELLO}{idle}{WATCH_CPU}")).await;
        other.send(&format!("{HELLO}{WATCH_MEMORY}")).await;
        assert_eq!(change(&mut server).await, Change::Subscribed(Page::Memory));
        assert_eq!(change(&mut server).await, Change::Subscribed(Page::Cpu));
    });
}

#[test]
fn a_client_with_a_stream_of_requests_does_not_starve_the_others() {
    let dir = RuntimeDir::new("stream");
    let mut server = bind(&dir);
    async_io::block_on(async {
        let mut busy = Client::watching(&dir, &mut server, WATCH_CPU, Page::Cpu).await;
        busy.send(&"{\"type\":\"refresh\"}\n".repeat(40)).await;
        assert_eq!(change(&mut server).await, Change::Refresh(Page::Cpu));
        let mut other = Client::connect(&dir).await;
        other.send(&format!("{HELLO}{WATCH_MEMORY}")).await;
        let mut changes = Vec::new();
        for _ in 0..2 {
            changes.push(change(&mut server).await);
        }
        assert!(
            changes.contains(&Change::Subscribed(Page::Memory)),
            "{changes:?}"
        );
    });
}

#[test]
fn accepts_are_bounded_per_turn() {
    let dir = RuntimeDir::new("accepts");
    let mut server = bind(&dir);
    let streams: Vec<UnixStream> = (0..=ACCEPTS_PER_TURN)
        .map(|_| UnixStream::connect(dir.socket()).expect("connect"))
        .collect();
    let last = streams.last().expect("connected");
    last.set_nonblocking(true).expect("non-blocking");
    let mut byte = [0];
    async_io::block_on(async {
        assert_eq!(future::poll_once(server.next()).await, None);
        assert_eq!(
            (&*last).read(&mut byte).expect_err("not taken yet").kind(),
            std::io::ErrorKind::WouldBlock
        );
        assert_eq!(future::poll_once(server.next()).await, None);
    });
    assert_eq!((&*last).read(&mut byte).expect("taken next turn"), 1);
}

#[test]
fn a_page_stays_watched_until_its_last_watcher_leaves() {
    let dir = RuntimeDir::new("two-watchers");
    let mut server = bind(&dir);
    async_io::block_on(async {
        let first = Client::watching(&dir, &mut server, WATCH_CPU, Page::Cpu).await;
        let second = Client::watching(&dir, &mut server, WATCH_CPU, Page::Cpu).await;
        drop(first);
        assert_eq!(change(&mut server).await, Change::Unwatched);
        assert!(server.watched(Page::Cpu));
        drop(second);
        assert_eq!(change(&mut server).await, Change::Unwatched);
        assert!(!server.watched(Page::Cpu));
    });
}

#[test]
fn another_protocol_version_is_refused() {
    let dir = RuntimeDir::new("version");
    let mut server = bind(&dir);
    async_io::block_on(async {
        let mut client = Client::connect(&dir).await;
        let first = serve(&mut server, client.line())
            .await
            .expect("the daemon's hello");
        assert_eq!(
            serde_json::from_str::<Hello>(&first).expect("a hello"),
            Hello::current()
        );
        client.send("{\"protocol\":2}\n").await;
        client
            .refused(&mut server, ErrorReason::VersionMismatch)
            .await;
    });
}

#[test]
fn a_first_line_that_is_not_a_hello_is_refused() {
    let dir = RuntimeDir::new("no-hello");
    let mut server = bind(&dir);
    async_io::block_on(async {
        let mut client = Client::connect(&dir).await;
        serve(&mut server, client.line())
            .await
            .expect("the daemon's hello");
        client.send(WATCH_CPU).await;
        client.refused(&mut server, ErrorReason::BadRequest).await;
    });
}

#[test]
fn malformed_requests_are_refused() {
    for (index, bad) in ["not json\n", "{\"type\":\"kill\",\"pid\":1}\n", "\n"]
        .iter()
        .enumerate()
    {
        let dir = RuntimeDir::new(&format!("bad-{index}"));
        let mut server = bind(&dir);
        async_io::block_on(async {
            let mut client = Client::greeted(&dir, &mut server).await;
            client.send(bad).await;
            client.refused(&mut server, ErrorReason::BadRequest).await;
        });
    }
}

#[test]
fn a_bad_request_from_a_watcher_releases_its_page() {
    let dir = RuntimeDir::new("bad-watcher");
    let mut server = bind(&dir);
    async_io::block_on(async {
        let mut client = Client::watching(&dir, &mut server, WATCH_CPU, Page::Cpu).await;
        client.send("not json\n").await;
        assert_eq!(change(&mut server).await, Change::Unwatched);
        assert!(!server.watched(Page::Cpu));
        client.refused(&mut server, ErrorReason::BadRequest).await;
    });
}

#[test]
fn a_line_without_an_end_is_refused_at_the_limit() {
    let dir = RuntimeDir::new("long-line");
    let mut server = bind(&dir);
    async_io::block_on(async {
        let mut client = Client::greeted(&dir, &mut server).await;
        client.send(&"x".repeat(MAX_REQUEST_BYTES + 1)).await;
        client.refused(&mut server, ErrorReason::BadRequest).await;
    });
}

#[test]
fn a_client_that_never_greets_is_timed_out() {
    let dir = RuntimeDir::new("handshake");
    // Already over when the client connects, so the test waits on no clock.
    let options = Options {
        handshake_timeout: Duration::ZERO,
        ..options()
    };
    let mut server = Server::bind(&dir.0, options).expect("bind");
    async_io::block_on(async {
        let mut client = Client::connect(&dir).await;
        serve(&mut server, client.line())
            .await
            .expect("the daemon's hello");
        client
            .refused(&mut server, ErrorReason::HandshakeTimeout)
            .await;
    });
}

#[test]
fn a_hello_that_came_in_time_is_taken_after_a_stall() {
    let dir = RuntimeDir::new("stall");
    // Over before the server first looks at the client, as after a loop that was held up.
    let options = Options {
        handshake_timeout: Duration::ZERO,
        ..options()
    };
    let mut server = Server::bind(&dir.0, options).expect("bind");
    async_io::block_on(async {
        let mut client = Client::connect(&dir).await;
        client.send(HELLO).await;
        client.send(WATCH_CPU).await;
        assert_eq!(change(&mut server).await, Change::Subscribed(Page::Cpu));
    });
}

#[test]
fn one_client_too_many_is_told_busy_and_a_freed_place_is_reused() {
    let dir = RuntimeDir::new("busy");
    let mut server = bind(&dir);
    async_io::block_on(async {
        let mut clients = Vec::new();
        for _ in 0..MAX_CLIENTS {
            clients.push(Client::watching(&dir, &mut server, WATCH_CPU, Page::Cpu).await);
        }
        let mut extra = Client::connect(&dir).await;
        serve(&mut server, extra.line())
            .await
            .expect("the daemon's hello");
        extra.refused(&mut server, ErrorReason::Busy).await;
        drop(clients.pop());
        assert_eq!(change(&mut server).await, Change::Unwatched);
        Client::watching(&dir, &mut server, WATCH_CPU, Page::Cpu).await;
    });
}

#[test]
fn a_client_of_another_uid_gets_nothing() {
    let dir = RuntimeDir::new("uid");
    let options = Options {
        allowed_uid: options().allowed_uid + 1,
        ..options()
    };
    let mut server = Server::bind(&dir.0, options).expect("bind");
    async_io::block_on(async {
        let mut client = Client::connect(&dir).await;
        assert_eq!(serve(&mut server, client.line()).await, None);
    });
}

#[test]
fn a_client_that_stops_reading_is_dropped_instead_of_buffered() {
    let dir = RuntimeDir::new("stalled");
    let mut server = bind(&dir);
    async_io::block_on(async {
        let _client = Client::watching(&dir, &mut server, WATCH_MEMORY, Page::Memory).await;
        // About 160 bytes each and never flushed, so the 64 KiB bound is passed well before 1000.
        for total in 0..1000 {
            server.publish(Page::Memory, &memory_event(total));
        }
        assert!(!server.watched(Page::Memory));
    });
}
