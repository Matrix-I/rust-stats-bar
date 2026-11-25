//! `statsbard`: the session daemon. It polls, draws the tray items and keeps the config. Pure
//! decisions live in `statsbar-core`; this crate holds the parts that touch the machine and the
//! bus, split so that everything except the event loop is testable without either.

pub mod app;
pub mod autostart;
pub mod config;
pub mod fsutil;
pub mod ipc;
pub mod launch;
pub mod pages;
pub mod paths;
pub mod power;
pub mod run;
pub mod summary;
pub mod tray;
