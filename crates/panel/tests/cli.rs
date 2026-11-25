use std::ffi::OsString;

use statsbar_core::ipc::Page;
use statsbar_panel::cli::page_from_args;

fn args(list: &[&str]) -> Vec<OsString> {
    list.iter().map(OsString::from).collect()
}

#[test]
fn the_page_comes_from_the_flag_and_defaults_to_cpu() {
    assert_eq!(page_from_args(&args(&[])), Ok(Page::Cpu));
    assert_eq!(page_from_args(&args(&["--page", "cpu"])), Ok(Page::Cpu));
    assert_eq!(
        page_from_args(&args(&["--page", "memory"])),
        Ok(Page::Memory)
    );
}

#[test]
fn anything_else_is_a_usage_error() {
    for bad in [
        &["--page"][..],
        &["--page", "gpu"],
        &["memory"],
        &["--page", "cpu", "x"],
    ] {
        assert!(page_from_args(&args(bad)).is_err(), "{bad:?}");
    }
}
