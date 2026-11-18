use std::fs::File;
use std::io;
use std::os::unix::fs::FileExt;
use std::path::Path;

/// procfs reports a size of 0, so the length is only known by reading to the end. The cap stops a
/// corrupt file from growing the buffer without bound. /proc/stat on this 8-thread laptop is
/// about 3 KiB and /proc/meminfo about 1.5 KiB.
const MAX_BYTES: usize = 256 * 1024;
const CHUNK: usize = 4096;

/// A file kept open and re-read whole on every call.
pub(crate) struct ProcFile {
    file: File,
    buffer: Vec<u8>,
}

impl ProcFile {
    pub(crate) fn open(path: &Path) -> io::Result<ProcFile> {
        Ok(ProcFile {
            file: File::open(path)?,
            buffer: Vec::new(),
        })
    }

    /// The file's current content. The buffer is reused, so a steady-state read allocates
    /// nothing.
    pub(crate) fn read(&mut self) -> io::Result<&str> {
        self.buffer.clear();
        let mut offset: u64 = 0;
        loop {
            if self.buffer.len() >= MAX_BYTES {
                return Err(invalid_data("file is at least 256 KiB"));
            }
            let start = self.buffer.len();
            self.buffer.resize(start + CHUNK, 0);
            let result = self.file.read_at(&mut self.buffer[start..], offset);
            let read = match result {
                Ok(read) => read,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                    self.buffer.truncate(start);
                    continue;
                }
                Err(error) => {
                    self.buffer.truncate(start);
                    return Err(error);
                }
            };
            self.buffer.truncate(start + read);
            if read == 0 {
                break;
            }
            offset += read as u64;
        }
        std::str::from_utf8(&self.buffer).map_err(|error| invalid_data(&error.to_string()))
    }
}

pub(crate) fn invalid_data(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_owned())
}
