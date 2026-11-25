//! `statsbar-panel [--page cpu|memory]`. The daemon's menus pass the page; without one the
//! window opens on CPU.

use std::ffi::OsString;

use statsbar_core::ipc::Page;

/// The arguments after the program name.
pub fn page_from_args(args: &[OsString]) -> Result<Page, String> {
    match args {
        [] => Ok(Page::Cpu),
        [flag, page] if flag == "--page" => match page.to_str() {
            Some("cpu") => Ok(Page::Cpu),
            Some("memory") => Ok(Page::Memory),
            _ => Err(format!("unknown page {page:?}; expected cpu or memory")),
        },
        _ => Err("usage: statsbar-panel [--page cpu|memory]".to_owned()),
    }
}
