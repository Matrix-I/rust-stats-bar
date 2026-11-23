use std::ffi::OsString;
use std::path::{Path, PathBuf};

use statsbard::paths::{autostart_file, config_file, config_home, runtime_dir};

fn env(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<OsString> {
    move |key| {
        pairs
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| OsString::from(value))
    }
}

#[test]
fn an_absolute_xdg_config_home_wins() {
    let vars = env(&[("XDG_CONFIG_HOME", "/x/conf"), ("HOME", "/home/u")]);
    assert_eq!(config_home(vars), Some(PathBuf::from("/x/conf")));
}

#[test]
fn a_relative_or_empty_xdg_config_home_falls_back_to_home() {
    let fallback = Some(PathBuf::from("/home/u/.config"));
    let empty = env(&[("XDG_CONFIG_HOME", ""), ("HOME", "/home/u")]);
    assert_eq!(config_home(empty), fallback, "empty");
    let relative = env(&[("XDG_CONFIG_HOME", "rel/conf"), ("HOME", "/home/u")]);
    assert_eq!(config_home(relative), fallback, "relative");
}

#[test]
fn without_an_absolute_home_there_is_no_config_home() {
    assert_eq!(config_home(env(&[])), None);
    assert_eq!(config_home(env(&[("HOME", "home/u")])), None);
}

#[test]
fn files_sit_under_the_config_home() {
    let home = Path::new("/home/u/.config");
    assert_eq!(
        config_file(home),
        Path::new("/home/u/.config/statsbar/config.toml")
    );
    assert_eq!(
        autostart_file(home),
        Path::new("/home/u/.config/autostart/statsbar.desktop")
    );
}

#[test]
fn the_runtime_directory_must_be_an_absolute_path() {
    let set = env(&[("XDG_RUNTIME_DIR", "/run/user/1000")]);
    assert_eq!(runtime_dir(set), Some(PathBuf::from("/run/user/1000")));
    assert_eq!(runtime_dir(env(&[("XDG_RUNTIME_DIR", "run")])), None);
    assert_eq!(runtime_dir(env(&[("XDG_RUNTIME_DIR", "")])), None);
    assert_eq!(runtime_dir(env(&[("HOME", "/home/u")])), None);
}
