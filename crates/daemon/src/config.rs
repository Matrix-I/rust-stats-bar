//! `~/.config/statsbar/config.toml`. Every key is optional, so a file written by an older version
//! keeps working. When the daemon saves, it rewrites only its own keys and keeps the rest, so a
//! newer version's settings survive a downgrade. A save drops comments and re-sorts keys, which the
//! user accepted (requirement 9) over adding `toml_edit`.

use std::fmt;
use std::io;
use std::path::Path;

use serde::Deserialize;
use statsbar_core::schedule::ReaderId;

use crate::fsutil::{read_to_string_capped, write_atomically};

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Config {
    pub items: Items,
}

/// Which tray items are shown. The hub is always shown, so it has no key.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Items {
    pub cpu: bool,
    pub memory: bool,
    pub battery: bool,
    pub fan: bool,
    pub network: bool,
}

impl Default for Items {
    fn default() -> Items {
        Items {
            cpu: true,
            memory: true,
            battery: true,
            fan: true,
            network: true,
        }
    }
}

impl Items {
    pub fn shown(&self, id: ReaderId) -> bool {
        match id {
            ReaderId::Cpu => self.cpu,
            ReaderId::Memory => self.memory,
            ReaderId::Battery => self.battery,
            ReaderId::Fan => self.fan,
            ReaderId::Network => self.network,
        }
    }

    pub fn set_shown(&mut self, id: ReaderId, shown: bool) {
        let flag = match id {
            ReaderId::Cpu => &mut self.cpu,
            ReaderId::Memory => &mut self.memory,
            ReaderId::Battery => &mut self.battery,
            ReaderId::Fan => &mut self.fan,
            ReaderId::Network => &mut self.network,
        };
        *flag = shown;
    }
}

/// Boxed, because the `toml` errors carry the offending text and would make every `Result` here
/// large.
#[derive(Debug)]
pub enum ConfigError {
    Parse(Box<toml::de::Error>),
    Render(Box<toml::ser::Error>),
    NotATable(&'static str),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Parse(error) => write!(f, "{error}"),
            ConfigError::Render(error) => write!(f, "cannot write the config: {error}"),
            ConfigError::NotATable(key) => write!(f, "`{key}` must be a table"),
        }
    }
}

impl std::error::Error for ConfigError {}

pub fn parse(text: &str) -> Result<Config, ConfigError> {
    toml::from_str(text).map_err(|error| ConfigError::Parse(Box::new(error)))
}

/// `existing` with `[items]` set from `config`, and every other key left as it was.
pub fn update_text(existing: &str, config: &Config) -> Result<String, ConfigError> {
    let mut table: toml::Table = existing
        .parse()
        .map_err(|error| ConfigError::Parse(Box::new(error)))?;
    let items = table
        .entry("items")
        .or_insert(toml::Value::Table(toml::Table::new()));
    let toml::Value::Table(items) = items else {
        return Err(ConfigError::NotATable("items"));
    };
    let shown = &config.items;
    for (key, value) in [
        ("cpu", shown.cpu),
        ("memory", shown.memory),
        ("battery", shown.battery),
        ("fan", shown.fan),
        ("network", shown.network),
    ] {
        items.insert(key.to_owned(), toml::Value::Boolean(value));
    }
    toml::to_string(&table).map_err(|error| ConfigError::Render(Box::new(error)))
}

/// What the daemon starts with.
#[derive(Debug)]
pub struct Loaded {
    pub config: Config,
    /// False when the file exists but could not be read or parsed: the user's file is then left
    /// alone, and switches last only until the daemon exits.
    pub writable: bool,
    pub warning: Option<String>,
}

pub fn load(path: &Path) -> Loaded {
    let refuse = |why: String| Loaded {
        config: Config::default(),
        writable: false,
        warning: Some(format!(
            "{}: {why}; using defaults and leaving the file unchanged",
            path.display()
        )),
    };
    match read_to_string_capped(path) {
        Ok(text) => match parse(&text) {
            Ok(config) => Loaded {
                config,
                writable: true,
                warning: None,
            },
            Err(error) => refuse(format!("not valid: {error}")),
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => Loaded {
            config: Config::default(),
            writable: true,
            warning: None,
        },
        Err(error) => refuse(format!("cannot be read: {error}")),
    }
}

pub fn save(path: &Path, config: &Config) -> io::Result<()> {
    let existing = match read_to_string_capped(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };
    let text = update_text(&existing, config)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    write_atomically(path, text.as_bytes())
}

#[derive(Debug)]
pub enum PersistError {
    /// The file could not be read or parsed at load time, so the user's file is left alone.
    NotWritable,
    Io(io::Error),
}

impl fmt::Display for PersistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PersistError::NotWritable => {
                write!(f, "left unchanged; this switch lasts until statsbard exits")
            }
            PersistError::Io(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for PersistError {}

/// What a menu switch does to the file: save, unless the file was refused when it was loaded.
pub fn persist(path: &Path, config: &Config, writable: bool) -> Result<(), PersistError> {
    if !writable {
        return Err(PersistError::NotWritable);
    }
    save(path, config).map_err(PersistError::Io)
}
