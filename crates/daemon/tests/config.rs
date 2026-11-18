use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use statsbard::config::{self, Config, Items};

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    match fs::remove_dir_all(&dir) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => panic!("clearing {}: {error}", dir.display()),
    }
    dir
}

fn shown(cpu: bool, memory: bool) -> Config {
    Config {
        items: Items { cpu, memory },
    }
}

#[test]
fn an_empty_file_shows_every_item() {
    assert_eq!(
        config::parse("").expect("empty is valid"),
        shown(true, true)
    );
}

#[test]
fn a_missing_key_keeps_its_default() {
    let parsed = config::parse("[items]\ncpu = false\n").expect("valid");
    assert_eq!(parsed, shown(false, true));
}

#[test]
fn unknown_keys_and_tables_are_ignored() {
    let parsed = config::parse("[items]\ngpu = true\n[future]\nx = 1\n").expect("valid");
    assert_eq!(parsed, shown(true, true));
}

#[test]
fn a_wrong_type_or_broken_syntax_is_an_error() {
    assert!(config::parse("[items]\ncpu = \"no\"\n").is_err());
    assert!(config::parse("[items\n").is_err());
}

#[test]
fn saving_keeps_keys_this_version_does_not_know() {
    let text = config::update_text(
        "[future]\nx = 1\n\n[items]\ncpu = true\n",
        &shown(false, true),
    )
    .expect("valid input");
    let table: toml::Table = text.parse().expect("output is TOML");
    assert_eq!(table["future"]["x"].as_integer(), Some(1));
    assert_eq!(table["items"]["cpu"].as_bool(), Some(false));
    assert_eq!(table["items"]["memory"].as_bool(), Some(true));
}

#[test]
fn saving_over_a_non_table_items_key_is_refused() {
    let error = config::update_text("items = 3\n", &shown(true, true)).expect_err("not a table");
    assert!(error.to_string().contains("items"), "{error}");
}

#[test]
fn a_missing_file_loads_defaults_and_may_be_written() {
    let loaded = config::load(&scratch("config-missing").join("config.toml"));
    assert_eq!(loaded.config, shown(true, true));
    assert!(loaded.writable);
    assert_eq!(loaded.warning, None);
}

#[test]
fn an_invalid_file_loads_defaults_and_is_never_written() {
    let dir = scratch("config-invalid");
    fs::create_dir_all(&dir).expect("create dir");
    let path = dir.join("config.toml");
    fs::write(&path, "[items\n").expect("write");
    let loaded = config::load(&path);
    assert_eq!(loaded.config, shown(true, true));
    assert!(!loaded.writable);
    let warning = loaded.warning.expect("a warning");
    assert!(warning.contains(&path.display().to_string()), "{warning}");
}

#[test]
fn a_file_over_the_size_cap_is_refused_and_left_alone() {
    let dir = scratch("config-oversized");
    fs::create_dir_all(&dir).expect("create dir");
    let path = dir.join("config.toml");
    let text = format!("# {}\n[items]\ncpu = false\n", "x".repeat(300 * 1024));
    fs::write(&path, &text).expect("write");
    let loaded = config::load(&path);
    assert_eq!(
        loaded.config,
        shown(true, true),
        "defaults, not the oversized file"
    );
    assert!(!loaded.writable);
    let warning = loaded.warning.expect("a warning");
    assert!(warning.contains("256 KiB"), "{warning}");
    assert_eq!(fs::read_to_string(&path).expect("still there"), text);
}

#[test]
fn save_creates_the_directory_and_round_trips() {
    let path = scratch("config-save").join("statsbar/config.toml");
    config::save(&path, &shown(true, false)).expect("save");
    let loaded = config::load(&path);
    assert_eq!(loaded.config, shown(true, false));
    assert!(loaded.writable);
    let mut temporary = path.clone().into_os_string();
    temporary.push(".tmp");
    assert!(
        !Path::new(&temporary).exists(),
        "the temporary file was renamed away"
    );
}

#[test]
fn save_refuses_to_rewrite_a_file_that_does_not_parse() {
    let dir = scratch("config-save-invalid");
    fs::create_dir_all(&dir).expect("create dir");
    let path = dir.join("config.toml");
    fs::write(&path, "[items\n").expect("write");
    assert!(config::save(&path, &shown(false, false)).is_err());
    assert_eq!(fs::read_to_string(&path).expect("still there"), "[items\n");
}

#[test]
fn persist_leaves_a_file_that_was_not_writable_at_load_untouched() {
    let dir = scratch("config-persist-refused");
    fs::create_dir_all(&dir).expect("create dir");
    let path = dir.join("config.toml");
    // Valid now, but the daemon loaded it while it was invalid: the load-time decision holds.
    fs::write(&path, "[items]\ncpu = true\n").expect("write");
    let result = config::persist(&path, &shown(false, true), false);
    assert!(
        matches!(result, Err(config::PersistError::NotWritable)),
        "{result:?}"
    );
    assert_eq!(
        fs::read_to_string(&path).expect("still there"),
        "[items]\ncpu = true\n"
    );
}

#[test]
fn persist_saves_when_the_file_was_writable_at_load() {
    let path = scratch("config-persist").join("config.toml");
    config::persist(&path, &shown(false, true), true).expect("saved");
    assert_eq!(config::load(&path).config, shown(false, true));
}
