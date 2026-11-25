//! The panel's end of the daemon's socket: the handshake, the requests, and a reader thread
//! that hands each event to the GTK main loop.
//!
//! Blocking I/O on its own thread keeps the main loop free, and a read timeout bounds the
//! handshake. The thread ends when the daemon closes the socket or the [`Link`] is dropped.

use std::fmt;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use async_channel::{Receiver, Sender};
use statsbar_core::ipc::{ErrorReason, Event, Hello, PROTOCOL_VERSION, Request};

const SOCKET: &str = "statsbar.sock";
/// A page with six processes is under 2 KiB; this only bounds a broken peer.
pub const MAX_EVENT_BYTES: u64 = 256 * 1024;
/// Events not yet taken by the main loop. A full queue stops the reader, and the daemon then
/// drops a client that has stopped reading.
const QUEUED_EVENTS: usize = 8;

/// What the reader thread passes to the main loop.
#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    /// Boxed: a page is two hundred bytes, the other variant two.
    Event(Box<Event>),
    /// The last message: the connection is gone.
    Closed(Closed),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Closed {
    /// The daemon closed the socket or quit.
    Eof,
    /// The daemon's last line said why.
    Refused(ErrorReason),
    /// A line that is not an event, or one over [`MAX_EVENT_BYTES`].
    BadLine,
    Io(io::ErrorKind),
}

impl fmt::Display for Closed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Closed::Eof => write!(f, "StatsBar quit or restarted"),
            Closed::Refused(ErrorReason::Busy) => {
                write!(f, "StatsBar already serves as many panels as it can")
            }
            Closed::Refused(ErrorReason::VersionMismatch) => {
                write!(
                    f,
                    "StatsBar speaks another protocol version; restart StatsBar"
                )
            }
            Closed::Refused(ErrorReason::BadRequest | ErrorReason::HandshakeTimeout) => {
                write!(f, "StatsBar closed the connection")
            }
            Closed::BadLine => write!(f, "StatsBar sent something this panel cannot read"),
            Closed::Io(kind) => write!(f, "the connection to StatsBar failed: {kind}"),
        }
    }
}

#[derive(Debug)]
pub enum LinkError {
    NoRuntimeDir,
    /// No daemon listens: not started, or quit.
    NotRunning(io::Error),
    /// The daemon speaks another protocol version.
    VersionMismatch {
        daemon: u32,
    },
    /// No hello in time, or one that is not a hello.
    Handshake(String),
    Io(io::Error),
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinkError::NoRuntimeDir => write!(f, "XDG_RUNTIME_DIR is not an absolute path"),
            LinkError::NotRunning(error) => write!(f, "StatsBar is not running ({error})"),
            LinkError::VersionMismatch { daemon } => write!(
                f,
                "StatsBar speaks protocol {daemon}, this panel {PROTOCOL_VERSION}; restart StatsBar"
            ),
            LinkError::Handshake(why) => write!(f, "StatsBar did not answer: {why}"),
            LinkError::Io(error) => write!(f, "talking to StatsBar failed: {error}"),
        }
    }
}

impl std::error::Error for LinkError {}

/// `$XDG_RUNTIME_DIR/statsbar.sock`, where the daemon binds it. `runtime_dir` is the variable's
/// value, passed in so tests need not change the environment.
pub fn socket_path(runtime_dir: Option<&std::ffi::OsStr>) -> Result<PathBuf, LinkError> {
    let dir = Path::new(runtime_dir.ok_or(LinkError::NoRuntimeDir)?);
    if dir.is_absolute() {
        Ok(dir.join(SOCKET))
    } else {
        Err(LinkError::NoRuntimeDir)
    }
}

pub struct Link {
    stream: UnixStream,
    pub events: Receiver<Incoming>,
}

impl Link {
    /// Connects and exchanges hellos. `timeout` bounds each read and write of the handshake,
    /// and every later request.
    pub fn connect(path: &Path, timeout: Duration) -> Result<Link, LinkError> {
        let stream = UnixStream::connect(path).map_err(LinkError::NotRunning)?;
        stream
            .set_read_timeout(Some(timeout))
            .map_err(LinkError::Io)?;
        stream
            .set_write_timeout(Some(timeout))
            .map_err(LinkError::Io)?;
        let mut reader = BufReader::new(stream.try_clone().map_err(LinkError::Io)?);
        // The daemon writes its hello on accept; ours goes first all the same, so neither waits.
        (&stream)
            .write_all(&line(&Hello::current()).map_err(LinkError::Io)?)
            .map_err(LinkError::Io)?;
        let first = read_line(&mut reader).map_err(|error| match error.kind() {
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => {
                LinkError::Handshake("no hello within the timeout".to_owned())
            }
            _ => LinkError::Io(error),
        })?;
        let Some(first) = first else {
            return Err(LinkError::Handshake("closed before its hello".to_owned()));
        };
        let hello: Hello = serde_json::from_slice(&first)
            .map_err(|_| LinkError::Handshake("its first line is not a hello".to_owned()))?;
        if hello.protocol != PROTOCOL_VERSION {
            return Err(LinkError::VersionMismatch {
                daemon: hello.protocol,
            });
        }
        // Events come one a second; the reader waits for them without a deadline.
        stream.set_read_timeout(None).map_err(LinkError::Io)?;
        let (sender, events) = async_channel::bounded(QUEUED_EVENTS);
        thread::Builder::new()
            .name("statsbar-link".to_owned())
            .spawn(move || pump(reader, &sender))
            .map_err(LinkError::Io)?;
        Ok(Link { stream, events })
    }

    pub fn send(&self, request: &Request) -> io::Result<()> {
        (&self.stream).write_all(&line(request)?)
    }
}

impl Drop for Link {
    /// Wakes the reader thread with an end of file, so it exits instead of waiting.
    fn drop(&mut self) {
        // Fails only when the socket is already closed, which is the goal.
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }
}

fn line<T: serde::Serialize>(value: &T) -> io::Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// One line without its newline; `None` at a clean end of file. A line over
/// [`MAX_EVENT_BYTES`] is an `InvalidData` error.
fn read_line(reader: &mut BufReader<UnixStream>) -> io::Result<Option<Vec<u8>>> {
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(MAX_EVENT_BYTES + 1)
        .read_until(b'\n', &mut bytes)?;
    match bytes.pop() {
        None => Ok(None),
        Some(b'\n') => Ok(Some(bytes)),
        // No newline: the line was cut at the limit, or the peer closed in mid-line.
        Some(_) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unterminated line",
        )),
    }
}

fn pump(mut reader: BufReader<UnixStream>, sender: &Sender<Incoming>) {
    let closed = loop {
        let bytes = match read_line(&mut reader) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => break Closed::Eof,
            Err(error) if error.kind() == io::ErrorKind::InvalidData => break Closed::BadLine,
            Err(error) => break Closed::Io(error.kind()),
        };
        match serde_json::from_slice::<Event>(&bytes) {
            Ok(Event::Error { reason, .. }) => break Closed::Refused(reason),
            Ok(event) => {
                if sender
                    .send_blocking(Incoming::Event(Box::new(event)))
                    .is_err()
                {
                    // The panel has gone; nobody is left to tell.
                    return;
                }
            }
            Err(_) => break Closed::BadLine,
        }
    };
    // The receiver may already be gone, and then nobody needs to know.
    let _ = sender.send_blocking(Incoming::Closed(closed));
}
