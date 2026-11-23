//! A failing accept backs off instead of spinning. Its own test binary: lowering the process's
//! file limit would break any test running beside it.

use std::fs::{self, File};
use std::future::Future;
use std::io::{ErrorKind, Read};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::pin::pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;

use async_io::Async;
use futures_lite::future;
use futures_lite::io::AsyncReadExt;
use rustix::io::Errno;
use rustix::process::{Resource, Rlimit, getrlimit, setrlimit};
use statsbard::ipc::{Options, Server};

/// Generous for a local socket; it only has to tell "slow" from "hung".
const LIMIT: Duration = Duration::from_secs(5);
const HELLO: &[u8] = b"{\"protocol\":1}\n";
/// Low enough that filling every free descriptor below it is quick.
const FILE_LIMIT: u64 = 256;

/// A short private directory: a socket path is limited to 108 bytes.
struct RuntimeDir(PathBuf);

impl RuntimeDir {
    fn new(name: &str) -> RuntimeDir {
        let dir = std::env::temp_dir().join(format!("sbar-{}-{name}", std::process::id()));
        fs::create_dir(&dir).expect("create the runtime directory");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).expect("chmod");
        RuntimeDir(dir)
    }
}

impl Drop for RuntimeDir {
    fn drop(&mut self) {
        // Only scratch files are left, and a failure here must not hide the test's own.
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Takes every free descriptor, so the next one asked for fails with EMFILE. Dropping it gives
/// them back and restores the file limit, also when the test panics.
struct NoFreeDescriptors {
    original: Rlimit,
    taken: Vec<File>,
}

impl NoFreeDescriptors {
    fn take() -> NoFreeDescriptors {
        let original = getrlimit(Resource::Nofile);
        let lowered = Rlimit {
            current: Some(
                original
                    .current
                    .map_or(FILE_LIMIT, |soft| soft.min(FILE_LIMIT)),
            ),
            maximum: original.maximum,
        };
        setrlimit(Resource::Nofile, lowered).expect("lower the file limit");
        let mut held = NoFreeDescriptors {
            original,
            taken: Vec::new(),
        };
        loop {
            match File::open("/dev/null") {
                Ok(file) => held.taken.push(file),
                Err(error) if Errno::from_io_error(&error) == Some(Errno::MFILE) => return held,
                Err(error) => panic!("taking a descriptor failed: {error}"),
            }
            assert!(
                held.taken.len() as u64 <= FILE_LIMIT,
                "the file limit did not hold"
            );
        }
    }
}

impl Drop for NoFreeDescriptors {
    fn drop(&mut self) {
        self.taken.clear();
        // Not a panic: this also runs while a failed test unwinds.
        if let Err(error) = setrlimit(Resource::Nofile, self.original) {
            eprintln!("restoring the file limit failed: {error}");
        }
    }
}

/// Counts wake-ups, which a spinning loop asks for on every turn.
#[derive(Default)]
struct WakeCount(AtomicUsize);

impl Wake for WakeCount {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// Polls the server once and reports whether it asked to be polled again at once.
fn turn(server: &mut Server) -> (Poll<statsbard::ipc::Change>, usize) {
    let count = Arc::new(WakeCount::default());
    let waker = Waker::from(count.clone());
    let polled = pin!(server.next()).poll(&mut Context::from_waker(&waker));
    (polled, count.0.load(Ordering::SeqCst))
}

fn has_nothing(client: &Async<UnixStream>) -> bool {
    let mut byte = [0];
    let mut stream = client.get_ref();
    matches!(
        stream.read(&mut byte),
        Err(error) if error.kind() == ErrorKind::WouldBlock
    )
}

#[test]
fn a_failing_accept_waits_before_it_is_retried() {
    let dir = RuntimeDir::new("emfile");
    let options = Options {
        allowed_uid: rustix::process::geteuid().as_raw(),
        handshake_timeout: LIMIT,
    };
    let mut server = Server::bind(&dir.0, options).expect("bind");
    let mut client = async_io::block_on(Async::<UnixStream>::connect(dir.0.join("statsbar.sock")))
        .expect("connect");

    let held = NoFreeDescriptors::take();
    let (polled, wakes) = turn(&mut server);
    assert!(polled.is_pending(), "{polled:?}");
    assert_eq!(wakes, 0, "the server asked to be polled again at once");
    drop(held);
    assert!(has_nothing(&client), "accepted without a free descriptor");

    // A descriptor is free now, but the retry is not due yet.
    let (polled, wakes) = turn(&mut server);
    assert!(polled.is_pending(), "{polled:?}");
    assert_eq!(wakes, 0);
    assert!(has_nothing(&client), "retried before the backoff");

    let mut hello = [0; HELLO.len()];
    async_io::block_on(future::or(
        async {
            future::or(client.read_exact(&mut hello), async {
                let change = server.next().await;
                panic!("unexpected {change:?}")
            })
            .await
            .expect("the daemon's hello");
        },
        async {
            async_io::Timer::after(LIMIT).await;
            panic!("the retry timed out after {LIMIT:?}")
        },
    ));
    assert_eq!(hello, HELLO);
}
