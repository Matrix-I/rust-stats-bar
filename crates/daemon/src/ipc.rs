//! The socket `statsbar-panel` connects to. It runs on the main loop: every read, write and
//! accept is non-blocking, so no client can hold up polling or the tray.
//!
//! The socket lets a peer watch figures, never act on the machine: no request writes or signals.

use std::fmt;
use std::fs::{self, File};
use std::future::{Future, poll_fn};
use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use std::time::Duration;

use async_io::{Async, Timer};
use futures_lite::io::{AsyncRead, AsyncWrite};
use rustix::fs::{FlockOperation, flock};
use statsbar_core::ipc::{ErrorReason, Event, Hello, PROTOCOL_VERSION, Page, Request};

const SOCKET: &str = "statsbar.sock";
const LOCK: &str = "statsbar.lock";
const PRIVATE: u32 = 0o600;
/// The XDG Base Directory spec lets the session clean up runtime files that have neither this
/// bit nor a fresh access time. A removed lock would let a second daemon start.
const STICKY: u32 = 0o1000;
/// Group and other permission bits.
const SHARED: u32 = 0o077;
/// One panel is the norm; the rest is room for `socat` and a panel that is restarting.
pub const MAX_CLIENTS: usize = 4;
/// The longest request is under 50 bytes.
pub const MAX_REQUEST_BYTES: usize = 4096;
/// About half a minute of page events. A client this far behind has stopped reading.
const MAX_UNSENT_BYTES: usize = 64 * 1024;
/// A failing accept, such as out of file descriptors, stays ready and would spin the loop.
const ACCEPT_RETRY: Duration = Duration::from_secs(1);
/// Requests that change nothing, taken from one client in one turn. Past it the client waits
/// for the next turn, so a stream of them cannot hold up the other clients or the main loop.
pub const LINES_PER_TURN: usize = 16;
/// Connections taken in one turn, for the same reason.
pub const ACCEPTS_PER_TURN: usize = 8;

#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Only this uid may connect.
    pub allowed_uid: u32,
    /// For the client's first line, and for flushing an error before closing.
    pub handshake_timeout: Duration,
}

/// What the main loop has to act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Subscribed(Page),
    Refresh(Page),
    /// A watcher left its page. Others may still watch it.
    Unwatched,
}

#[derive(Debug)]
pub enum BindError {
    AlreadyRunning,
    Io(io::Error),
}

impl fmt::Display for BindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BindError::AlreadyRunning => write!(f, "StatsBar is already running"),
            BindError::Io(error) => write!(f, "cannot open the panel's socket: {error}"),
        }
    }
}

impl std::error::Error for BindError {}

impl From<io::Error> for BindError {
    fn from(error: io::Error) -> BindError {
        BindError::Io(error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Greeting,
    Ready,
    /// Only flushes what is left, then closes.
    Closing,
}

enum Step {
    Changed(Change),
    Gone,
}

struct Client {
    stream: Async<UnixStream>,
    state: State,
    page: Option<Page>,
    /// Subscribed since the page was last sent.
    fresh: bool,
    input: Vec<u8>,
    output: Vec<u8>,
    /// Bounds `Greeting` and `Closing`.
    deadline: Timer,
    timeout: Duration,
}

pub struct Server {
    listener: Async<UnixListener>,
    socket: PathBuf,
    /// Held for the daemon's lifetime; the kernel releases it however the process ends.
    _lock: File,
    options: Options,
    clients: Vec<Client>,
    accept_retry: Option<Timer>,
}

impl Server {
    /// Claims the runtime directory for this daemon, then listens there. The lock is what makes
    /// replacing a leftover socket safe: its owner is known to be gone.
    pub fn bind(runtime_dir: &Path, options: Options) -> Result<Server, BindError> {
        check_private(runtime_dir)?;
        // `truncate(false)`: the file only carries the lock.
        let lock = File::options()
            .write(true)
            .create(true)
            .truncate(false)
            .open(runtime_dir.join(LOCK))?;
        match flock(&lock, FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => {}
            Err(rustix::io::Errno::WOULDBLOCK) => return Err(BindError::AlreadyRunning),
            Err(error) => return Err(BindError::Io(error.into())),
        }
        // Only once the lock is ours, so a running daemon's file is left as it is.
        lock.set_permissions(fs::Permissions::from_mode(PRIVATE | STICKY))?;

        let socket = runtime_dir.join(SOCKET);
        match fs::symlink_metadata(&socket) {
            Ok(found) if found.file_type().is_socket() => fs::remove_file(&socket)?,
            Ok(_) => {
                return Err(BindError::Io(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("{} exists and is not a socket", socket.display()),
                )));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let listener = UnixListener::bind(&socket)?;
        let listener =
            match fs::set_permissions(&socket, fs::Permissions::from_mode(PRIVATE | STICKY))
                .and_then(|()| Async::new(listener))
            {
                Ok(listener) => listener,
                Err(error) => {
                    // No `Server` owns the socket yet, so its `Drop` would not remove it.
                    if let Err(removal) = fs::remove_file(&socket) {
                        eprintln!("statsbard: removing {} failed: {removal}", socket.display());
                    }
                    return Err(error.into());
                }
            };
        Ok(Server {
            listener,
            socket,
            _lock: lock,
            options,
            clients: Vec::new(),
            accept_retry: None,
        })
    }

    pub fn watched(&self, page: Page) -> bool {
        self.clients.iter().any(|client| client.page == Some(page))
    }

    /// Queues `event` for the page's watchers. It is sent while [`Server::next`] is awaited.
    pub fn publish(&mut self, page: Page, event: &Event) {
        self.queue(page, event, false);
    }

    /// Queues `event`, the page as last published, for the watchers that subscribed since then,
    /// so a page opens with figures without a read outside the cadence.
    pub fn welcome(&mut self, page: Page, event: &Event) {
        self.queue(page, event, true);
    }

    fn queue(&mut self, page: Page, event: &Event, only_fresh: bool) {
        let is_for = |client: &Client| client.page == Some(page) && (client.fresh || !only_fresh);
        if !self.clients.iter().any(is_for) {
            return;
        }
        let line = match encode(event) {
            Ok(line) => line,
            Err(error) => {
                eprintln!("statsbard: encoding the {page:?} page failed: {error}");
                return;
            }
        };
        self.clients.retain_mut(|client| {
            if !is_for(client) {
                return true;
            }
            if client.output.len() + line.len() > MAX_UNSENT_BYTES {
                eprintln!("statsbard: dropped a panel connection that stopped reading");
                return false;
            }
            client.output.extend_from_slice(&line);
            client.fresh = false;
            true
        });
    }

    /// Serves until something changes. Dropping the future loses nothing: all state is in
    /// `self`.
    pub async fn next(&mut self) -> Change {
        poll_fn(|cx| self.poll_next(cx)).await
    }

    fn poll_next(&mut self, cx: &mut Context<'_>) -> Poll<Change> {
        // Accepts first, so a client with a stream of requests cannot keep new ones out.
        let mut accepted = 0;
        while accepted < ACCEPTS_PER_TURN && self.poll_accept(cx).is_ready() {
            accepted += 1;
        }
        if accepted == ACCEPTS_PER_TURN {
            // More may be waiting, and the listener's waker was not registered.
            cx.waker().wake_by_ref();
        }
        // New clients are polled here too, which registers their wakers.
        let mut index = 0;
        while index < self.clients.len() {
            match self.clients[index].poll(cx) {
                Poll::Pending => index += 1,
                Poll::Ready(Step::Changed(change)) => {
                    // To the back, so the others are polled first next time.
                    let client = self.clients.remove(index);
                    self.clients.push(client);
                    return Poll::Ready(change);
                }
                Poll::Ready(Step::Gone) => {
                    if self.clients.remove(index).page.is_some() {
                        return Poll::Ready(Change::Unwatched);
                    }
                }
            }
        }
        Poll::Pending
    }

    /// Ready after accepting or refusing one connection.
    fn poll_accept(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        if let Some(retry) = &mut self.accept_retry {
            ready!(Pin::new(retry).poll(cx));
            self.accept_retry = None;
        }
        loop {
            match self.listener.get_ref().accept() {
                Ok((stream, _)) => {
                    self.admit(stream);
                    return Poll::Ready(());
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    match ready!(self.listener.poll_readable(cx)) {
                        Ok(()) => continue,
                        Err(error) => return self.accept_failed(&error),
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return self.accept_failed(&error),
            }
        }
    }

    fn accept_failed(&mut self, error: &io::Error) -> Poll<()> {
        eprintln!("statsbard: accepting a panel connection failed: {error}");
        self.accept_retry = Some(Timer::after(ACCEPT_RETRY));
        // Polls the retry timer on the next turn, which registers its waker.
        Poll::Ready(())
    }

    fn admit(&mut self, stream: UnixStream) {
        match rustix::net::sockopt::socket_peercred(&stream) {
            Ok(peer) if peer.uid.as_raw() == self.options.allowed_uid => {}
            Ok(peer) => {
                eprintln!(
                    "statsbard: refused a connection from uid {}",
                    peer.uid.as_raw()
                );
                return;
            }
            Err(error) => {
                eprintln!("statsbard: refused a connection of unknown uid: {error}");
                return;
            }
        }
        let stream = match Async::new(stream) {
            Ok(stream) => stream,
            Err(error) => {
                eprintln!("statsbard: setting up a panel connection failed: {error}");
                return;
            }
        };
        let hello = match encode(&Hello::current()) {
            Ok(line) => line,
            Err(error) => {
                eprintln!("statsbard: encoding the hello failed: {error}");
                return;
            }
        };
        if self.clients.len() >= MAX_CLIENTS {
            refuse_busy(&stream, hello);
            return;
        }
        self.clients.push(Client {
            stream,
            state: State::Greeting,
            page: None,
            fresh: false,
            input: Vec::new(),
            output: hello,
            deadline: Timer::after(self.options.handshake_timeout),
            timeout: self.options.handshake_timeout,
        });
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        // Before the lock goes, so this can never remove a successor's socket.
        if let Err(error) = fs::remove_file(&self.socket) {
            eprintln!(
                "statsbard: removing {} failed: {error}",
                self.socket.display()
            );
        }
    }
}

impl Client {
    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<Step> {
        let mut handled = 0;
        loop {
            while !self.output.is_empty() {
                match Pin::new(&mut &self.stream).poll_write(cx, &self.output) {
                    Poll::Ready(Ok(0) | Err(_)) => return Poll::Ready(Step::Gone),
                    Poll::Ready(Ok(written)) => {
                        self.output.drain(..written);
                    }
                    Poll::Pending => break,
                }
            }
            if self.state == State::Closing {
                if self.output.is_empty() {
                    return Poll::Ready(Step::Gone);
                }
                ready!(Pin::new(&mut self.deadline).poll(cx));
                return Poll::Ready(Step::Gone);
            }

            let end = self.input.iter().position(|&byte| byte == b'\n');
            // Whether or not its end has arrived, so the limit is the same however reads split.
            if end.unwrap_or(self.input.len()) > MAX_REQUEST_BYTES {
                if let Some(change) = self.fail(ErrorReason::BadRequest, "the line is too long") {
                    return Poll::Ready(Step::Changed(change));
                }
                continue;
            }
            if let Some(end) = end {
                let line: Vec<u8> = self.input.drain(..=end).collect();
                if let Some(change) = self.take_line(&line) {
                    return Poll::Ready(Step::Changed(change));
                }
                handled += 1;
                if handled == LINES_PER_TURN {
                    cx.waker().wake_by_ref();
                    return Poll::Pending;
                }
                continue;
            }
            let mut chunk = [0; 1024];
            match Pin::new(&mut &self.stream).poll_read(cx, &mut chunk) {
                Poll::Ready(Ok(0) | Err(_)) => return Poll::Ready(Step::Gone),
                Poll::Ready(Ok(read)) => self.input.extend_from_slice(&chunk[..read]),
                // Only once nothing is left to read: a hello that came in time is taken even
                // when the loop was held up past the deadline.
                Poll::Pending
                    if self.state == State::Greeting
                        && Pin::new(&mut self.deadline).poll(cx).is_ready() =>
                {
                    // Nothing is watched before the greeting, so this changes no page.
                    self.fail(ErrorReason::HandshakeTimeout, "no protocol line arrived");
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }

    fn take_line(&mut self, line: &[u8]) -> Option<Change> {
        if self.state == State::Greeting {
            match serde_json::from_slice::<Hello>(line) {
                Ok(hello) if hello.protocol == PROTOCOL_VERSION => {
                    self.state = State::Ready;
                    return None;
                }
                Ok(hello) => {
                    let message = format!(
                        "statsbard speaks protocol {PROTOCOL_VERSION}, not {}; restart StatsBar",
                        hello.protocol
                    );
                    return self.fail(ErrorReason::VersionMismatch, &message);
                }
                Err(_) => {
                    return self.fail(
                        ErrorReason::BadRequest,
                        "the first line must be the protocol",
                    );
                }
            }
        }
        match serde_json::from_slice::<Request>(line) {
            Ok(Request::Subscribe { page }) => {
                self.page = Some(page);
                self.fresh = true;
                Some(Change::Subscribed(page))
            }
            Ok(Request::Unsubscribe) => self.page.take().map(|_| Change::Unwatched),
            Ok(Request::Refresh) => self.page.map(Change::Refresh),
            // The line is the peer's, so it is neither echoed nor logged.
            Err(_) => self.fail(ErrorReason::BadRequest, "not a request of this protocol"),
        }
    }

    /// Queues the error and starts closing.
    fn fail(&mut self, reason: ErrorReason, message: &str) -> Option<Change> {
        self.state = State::Closing;
        self.deadline = Timer::after(self.timeout);
        self.input.clear();
        let error = Event::Error {
            reason,
            message: message.to_owned(),
        };
        match encode(&error) {
            Ok(line) => self.output.extend_from_slice(&line),
            Err(error) => eprintln!("statsbard: encoding an error line failed: {error}"),
        }
        self.page.take().map(|_| Change::Unwatched)
    }
}

/// Refuses a runtime directory another user could write to or read: there they could hold the
/// lock or plant a link in the socket's place.
fn check_private(dir: &Path) -> io::Result<()> {
    let found = fs::metadata(dir)?;
    if found.is_dir()
        && found.uid() == rustix::process::geteuid().as_raw()
        && found.mode() & SHARED == 0
    {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        format!("{} is not a directory private to this user", dir.display()),
    ))
}

/// One try, without waiting: two short lines always fit a new socket's buffer, and a connection
/// over the limit gets no state of its own.
fn refuse_busy(stream: &Async<UnixStream>, hello: Vec<u8>) {
    let busy = Event::Error {
        reason: ErrorReason::Busy,
        message: format!("statsbard already has {MAX_CLIENTS} connections"),
    };
    let sent = encode(&busy)
        .map_err(io::Error::from)
        .and_then(|line| io::Write::write_all(&mut stream.get_ref(), &[hello, line].concat()));
    if let Err(error) = sent {
        eprintln!("statsbard: refusing a connection over the limit failed: {error}");
    }
}

fn encode(value: &impl serde::Serialize) -> serde_json::Result<Vec<u8>> {
    let mut line = serde_json::to_vec(value)?;
    line.push(b'\n');
    Ok(line)
}
