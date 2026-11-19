use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use statsbard::autostart::{desktop_entry, disable, enable, is_enabled, toggle};

fn exec_line(exe: &str) -> String {
    let entry = desktop_entry(Path::new(exe)).expect("valid path");
    entry
        .lines()
        .find(|line| line.starts_with("Exec="))
        .expect("an Exec line")
        .to_owned()
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    match fs::remove_dir_all(&dir) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => panic!("clearing {}: {error}", dir.display()),
    }
    dir
}

#[test]
fn the_entry_starts_the_daemon_and_stays_out_of_the_app_grid() {
    assert_eq!(
        desktop_entry(Path::new("/home/u/.cargo/bin/statsbard")).expect("valid"),
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=StatsBar\n\
         Comment=System figures in the top bar\n\
         Exec=/home/u/.cargo/bin/statsbard\n\
         NoDisplay=true\n\
         X-GNOME-Autostart-enabled=true\n"
    );
}

#[test]
fn a_path_with_a_space_is_quoted() {
    assert_eq!(
        exec_line("/home/u/my apps/statsbard"),
        r#"Exec="/home/u/my apps/statsbard""#
    );
}

#[test]
fn a_percent_sign_is_doubled_so_it_is_not_a_field_code() {
    assert_eq!(
        exec_line("/opt/100%/statsbard"),
        "Exec=/opt/100%%/statsbard"
    );
}

#[test]
fn quote_dollar_and_backslash_are_escaped_twice() {
    // The string-value escape runs first and turns `\\` into `\`; the quoting rule then needs
    // `\"`, `\$` and `\\`.
    assert_eq!(
        exec_line(r#"/opt/a$b"c/statsbard"#),
        r#"Exec="/opt/a\\$b\\"c/statsbard""#
    );
    assert_eq!(
        exec_line(r"/opt/a\b/statsbard"),
        r#"Exec="/opt/a\\\\b/statsbard""#
    );
}

#[test]
fn a_relative_path_or_a_control_character_is_refused() {
    for exe in ["statsbard", "/opt/a\nb/statsbard"] {
        let error = desktop_entry(Path::new(exe)).expect_err("refused");
        assert_eq!(error.kind(), ErrorKind::InvalidInput, "{exe:?}");
    }
}

/// Linux appends " (deleted)" to /proc/self/exe once the running binary is replaced, for example
/// by an upgrade. That path starts nothing at the next login.
#[test]
fn a_replaced_binary_is_refused() {
    let error =
        desktop_entry(Path::new("/home/u/.cargo/bin/statsbard (deleted)")).expect_err("refused");
    assert_eq!(error.kind(), ErrorKind::InvalidInput);
}

#[test]
fn enable_then_disable_round_trips_and_disable_twice_is_fine() {
    let path = scratch("autostart").join("autostart/statsbar.desktop");
    assert!(!is_enabled(&path).expect("readable"));
    enable(&path, Path::new("/usr/bin/statsbard")).expect("enable");
    assert!(is_enabled(&path).expect("readable"));
    assert_eq!(
        fs::read_to_string(&path).expect("written"),
        desktop_entry(Path::new("/usr/bin/statsbard")).expect("valid")
    );
    disable(&path).expect("disable");
    assert!(!is_enabled(&path).expect("readable"));
    disable(&path).expect("disabling again is not an error");
}

#[test]
fn toggle_flips_the_entry_and_returns_the_new_state() {
    let path = scratch("autostart-toggle").join("autostart/statsbar.desktop");
    let exe = || Ok(PathBuf::from("/usr/bin/statsbard"));
    assert!(toggle(&path, false, exe).expect("enable"));
    assert!(is_enabled(&path).expect("readable"));
    assert!(!toggle(&path, true, exe).expect("disable"));
    assert!(!is_enabled(&path).expect("readable"));
}

#[test]
fn a_failed_toggle_changes_nothing() {
    let path = scratch("autostart-toggle-fails").join("autostart/statsbar.desktop");
    let unknown = || Err(std::io::Error::other("no current_exe"));
    assert!(toggle(&path, false, unknown).is_err());
    let relative = || Ok(PathBuf::from("statsbard"));
    assert_eq!(
        toggle(&path, false, relative).expect_err("relative").kind(),
        ErrorKind::InvalidInput
    );
    assert!(
        !is_enabled(&path).expect("readable"),
        "no entry was written"
    );
}
