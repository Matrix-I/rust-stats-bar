//! Writes that never leave a half-written file behind.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// Writes `contents` to `<path>.tmp`, syncs it, and renames it over `path`, so anything reading
/// the file — or the daemon after a crash — sees either the old content or the new. A `.tmp` left
/// by a failed write is truncated by the next attempt, so it needs no cleanup.
pub fn write_atomically(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "path has no parent directory")
    })?;
    fs::create_dir_all(parent)?;
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    let mut file = File::create(&temporary)?;
    file.write_all(contents)?;
    file.sync_all()?;
    fs::rename(&temporary, path)
}

/// Far above any config or entry this daemon writes. A larger file was not written by it, and is
/// refused instead of being read into memory whole.
pub const MAX_READ_BYTES: u64 = 256 * 1024;

/// `fs::read_to_string` with the cap above. A missing file is still `NotFound`.
pub fn read_to_string_capped(path: &Path) -> io::Result<String> {
    let mut text = String::new();
    File::open(path)?
        .take(MAX_READ_BYTES + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > MAX_READ_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "file is larger than 256 KiB",
        ));
    }
    Ok(text)
}
