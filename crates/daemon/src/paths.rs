//! Where the daemon keeps its files, after the XDG Base Directory spec.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// `$XDG_CONFIG_HOME`, else `$HOME/.config`. The spec says a relative path is invalid and must be
/// ignored; an empty value is relative too. `var` is the environment, passed in so that tests do
/// not touch the process environment.
pub fn config_home(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let absolute = |key: &str| {
        var(key)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    };
    absolute("XDG_CONFIG_HOME").or_else(|| absolute("HOME").map(|home| home.join(".config")))
}

pub fn config_file(config_home: &Path) -> PathBuf {
    config_home.join("statsbar/config.toml")
}

pub fn autostart_file(config_home: &Path) -> PathBuf {
    config_home.join("autostart/statsbar.desktop")
}
