//! Launch at login: the XDG autostart entry. Its presence *is* the setting. The config file does
//! not repeat it, so the two can never disagree.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::fsutil::write_atomically;

/// Characters that force an Exec argument into double quotes (Desktop Entry Specification, "The
/// Exec key").
const RESERVED: &[char] = &[
    ' ', '\t', '\n', '"', '\'', '\\', '>', '<', '~', '|', '&', ';', '$', '*', '?', '#', '(', ')',
    '`',
];

pub fn desktop_entry(exe: &Path) -> io::Result<String> {
    Ok(format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=StatsBar\n\
         Comment=System figures in the top bar\n\
         Exec={}\n\
         NoDisplay=true\n\
         X-GNOME-Autostart-enabled=true\n",
        exec_value(exe)?
    ))
}

fn exec_value(exe: &Path) -> io::Result<String> {
    let refuse = |why: &str| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("cannot start {} at login: {why}", exe.display()),
        )
    };
    if !exe.is_absolute() {
        return Err(refuse("not an absolute path"));
    }
    let text = exe.to_str().ok_or_else(|| refuse("not valid UTF-8"))?;
    if text.chars().any(char::is_control) {
        return Err(refuse("contains a control character"));
    }
    // What /proc/self/exe reads once the running binary has been replaced, as by an upgrade.
    if text.ends_with(" (deleted)") {
        return Err(refuse(
            "the running binary was replaced; restart StatsBar first",
        ));
    }
    // Inside quotes, `"`, `` ` ``, `$` and `\` take a backslash. The general string escaping runs
    // before the quoting rule and halves every `\\`, so each of those backslashes is written
    // twice. `%` would start a field code. Every character escaped here is also in RESERVED, so
    // the escapes only ever appear inside quotes.
    let quoted = text.contains(RESERVED);
    let mut value = String::with_capacity(text.len() + 2);
    if quoted {
        value.push('"');
    }
    for c in text.chars() {
        match c {
            '%' => value.push_str("%%"),
            '\\' => value.push_str(r"\\\\"),
            '"' | '`' | '$' => {
                value.push_str(r"\\");
                value.push(c);
            }
            _ => value.push(c),
        }
    }
    if quoted {
        value.push('"');
    }
    Ok(value)
}

pub fn is_enabled(path: &Path) -> io::Result<bool> {
    path.try_exists()
}

pub fn enable(path: &Path, exe: &Path) -> io::Result<()> {
    write_atomically(path, desktop_entry(exe)?.as_bytes())
}

pub fn disable(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

/// The *Launch at login* switch: flips the entry and returns the new state. On an error nothing
/// was changed, so the caller keeps showing the old state. `exe` is asked for only when enabling.
pub fn toggle(
    path: &Path,
    enabled: bool,
    exe: impl FnOnce() -> io::Result<PathBuf>,
) -> io::Result<bool> {
    if enabled {
        disable(path)?;
        Ok(false)
    } else {
        enable(path, &exe()?)?;
        Ok(true)
    }
}
