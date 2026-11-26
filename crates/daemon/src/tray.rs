//! The `ksni` tray items: one per metric, plus the hub. `ksni`'s driver thread calls their `Tray`
//! methods and menu callbacks; the main loop changes their fields through `Handle::update`, whose
//! closure runs on the main thread under `ksni`'s lock. Menu clicks and left clicks become
//! `Command`s for the main loop; nothing here reads or writes files.
//!
//! A metric item is a themed icon with its value as `XAyatanaLabel` text. The shell draws that
//! text in its panel font, which stays legible at fractional scales where a pixmap blurs
//! (spike S3).

use async_channel::Sender;
use ksni::menu::{CheckmarkItem, StandardItem};
use ksni::{Category, MenuItem};
use statsbar_core::schedule::ReaderId;
use statsbar_core::throughput::Reading;

use crate::app::{BatteryLabel, Command, READERS, page_of};
use crate::config::Items;
use crate::summary::{self, Overview};

/// The hub's icon.
pub const FALLBACK_ICON: &str = "utilities-system-monitor";

/// The icon an item shows next to its label. Only the battery's changes after its first value
/// (`battery_icon`). Every name was found in Yaru on 2026-11-21.
pub fn metric_icon(reader: ReaderId) -> &'static str {
    match reader {
        ReaderId::Cpu => "computer-chip-symbolic",
        ReaderId::Memory => "drive-harddisk-solidstate-symbolic",
        // Until the first sample says how full the battery is.
        ReaderId::Battery => "battery-missing-symbolic",
        // Yaru has no fan icon; moving air is the nearest.
        ReaderId::Fan => "weather-windy-symbolic",
        ReaderId::Network => "network-transmit-receive-symbolic",
    }
}

/// The icon GNOME Shell's own battery indicator would pick: the charge floored to a multiple of
/// 10, with the bolt while charging.
pub fn battery_icon(label: BatteryLabel) -> String {
    let level = label.percent.min(100) / 10 * 10;
    if label.charging {
        format!("battery-level-{level}-charging-symbolic")
    } else {
        format!("battery-level-{level}-symbolic")
    }
}

/// The text next to a percentage item's icon. No space, unlike the titles, to keep the panel
/// narrow.
pub fn percent_label(percent: u8) -> String {
    format!("{percent}%")
}

/// The text next to the network icon: the download rate, banded by `label::rate_text`.
pub fn download_label(download_text: &str) -> String {
    format!("↓{download_text}")
}

/// The widest label each item takes, published as `XAyatanaLabelGuide`. Ubuntu's AppIndicator
/// extension 50.26 does not read it (only listed in its `appIndicator.js`, checked 2026-11-21);
/// it is there for hosts that do.
pub fn label_guide(reader: ReaderId) -> &'static str {
    match reader {
        ReaderId::Network => "↓999M",
        ReaderId::Cpu | ReaderId::Memory | ReaderId::Battery | ReaderId::Fan => "100%",
    }
}

fn row<T>(text: &str) -> MenuItem<T> {
    StandardItem {
        label: text.to_owned(),
        enabled: false,
        ..Default::default()
    }
    .into()
}

/// A place kept for a row that has no value yet. Showing it later changes a property, not the
/// layout, so `ksni` keeps every menu id and a click on "Open details…" is never lost.
fn reserved_row<T>() -> MenuItem<T> {
    StandardItem {
        enabled: false,
        visible: false,
        ..Default::default()
    }
    .into()
}

/// The most rows `summary::cpu_rows` gives.
pub const CPU_MENU_ROWS: usize = 5;
/// The most rows `summary::memory_rows` gives: six, and Compressed while zswap holds pages.
pub const MEMORY_MENU_ROWS: usize = 7;
pub const OPEN_DETAILS: &str = "Open details…";

/// Rows kept from the start in a menu that has "Open details…". Other menus have no clickable
/// item, so their layout may change.
fn reserved_rows(reader: ReaderId) -> usize {
    match reader {
        ReaderId::Cpu => CPU_MENU_ROWS,
        ReaderId::Memory => MEMORY_MENU_ROWS,
        ReaderId::Battery | ReaderId::Fan | ReaderId::Network => 0,
    }
}

pub struct MetricItem {
    pub reader: ReaderId,
    /// A themed icon name.
    pub icon: String,
    /// Empty until the first value, so the panel shows the icon alone.
    pub label: String,
    pub title: String,
    pub rows: Vec<String>,
    commands: Sender<Command>,
}

impl MetricItem {
    pub fn new(reader: ReaderId, commands: Sender<Command>) -> MetricItem {
        MetricItem {
            reader,
            icon: metric_icon(reader).to_owned(),
            label: String::new(),
            title: name(reader).to_owned(),
            rows: Vec::new(),
            commands,
        }
    }
}

/// The name in titles, the hub's rows and its switches.
pub fn name(reader: ReaderId) -> &'static str {
    match reader {
        ReaderId::Cpu => "CPU",
        ReaderId::Memory => "Memory",
        ReaderId::Battery => "Battery",
        ReaderId::Fan => "Fan",
        ReaderId::Network => "Network",
    }
}

impl ksni::Tray for MetricItem {
    fn id(&self) -> String {
        match self.reader {
            ReaderId::Cpu => "statsbar-cpu",
            ReaderId::Memory => "statsbar-memory",
            ReaderId::Battery => "statsbar-battery",
            ReaderId::Fan => "statsbar-fan",
            ReaderId::Network => "statsbar-network",
        }
        .to_owned()
    }

    fn category(&self) -> Category {
        Category::Hardware
    }

    fn title(&self) -> String {
        self.title.clone()
    }

    fn icon_name(&self) -> String {
        self.icon.clone()
    }

    fn x_ayatana_label(&self) -> String {
        self.label.clone()
    }

    fn x_ayatana_label_guide(&self) -> String {
        label_guide(self.reader).to_owned()
    }

    /// A left click opens the same page as "Open details…"; items without a page do nothing.
    fn activate(&mut self, _x: i32, _y: i32) {
        if let Some(page) = page_of(self.reader) {
            send(&self.commands, Command::OpenPage(page));
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let Some(page) = page_of(self.reader) else {
            return self.rows.iter().map(|text| row(text)).collect();
        };
        let mut menu: Vec<MenuItem<Self>> = vec![
            StandardItem {
                label: OPEN_DETAILS.to_owned(),
                activate: Box::new(move |item: &mut MetricItem| {
                    send(&item.commands, Command::OpenPage(page));
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
        ];
        let places = reserved_rows(self.reader).max(self.rows.len());
        menu.extend((0..places).map(|index| match self.rows.get(index) {
            Some(text) => row(text),
            None => reserved_row(),
        }));
        menu
    }
}

/// Never blocks: this runs on the tray thread, which also answers the panel's D-Bus calls. A
/// click while the queue is full is dropped and logged.
fn send(commands: &Sender<Command>, command: Command) {
    if let Err(error) = commands.try_send(command) {
        eprintln!("statsbard: click dropped: {error}");
    }
}

pub struct Hub {
    pub rows: Vec<String>,
    pub items: Items,
    pub launch_at_login: bool,
    commands: Sender<Command>,
}

impl Hub {
    pub fn new(commands: Sender<Command>, items: Items, launch_at_login: bool) -> Hub {
        Hub {
            // Placeholders now, so the first values only relabel rows. Adding rows later would
            // change the layout, which makes ksni renumber every menu id and lose a click sent
            // with an old one.
            rows: summary::hub_rows(&Overview::waiting(&items)),
            items,
            launch_at_login,
            commands,
        }
    }

    /// A dropped click leaves the checkmark showing the state the main loop last set.
    fn send(&self, command: Command) {
        send(&self.commands, command);
    }
}

fn switch(label: &str, checked: bool, command: Command) -> MenuItem<Hub> {
    CheckmarkItem {
        label: label.to_owned(),
        checked,
        activate: Box::new(move |hub: &mut Hub| hub.send(command)),
        ..Default::default()
    }
    .into()
}

impl ksni::Tray for Hub {
    fn id(&self) -> String {
        "statsbar-hub".to_owned()
    }

    fn category(&self) -> Category {
        Category::Hardware
    }

    fn title(&self) -> String {
        "StatsBar".to_owned()
    }

    fn icon_name(&self) -> String {
        FALLBACK_ICON.to_owned()
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let mut menu: Vec<MenuItem<Self>> = self.rows.iter().map(|text| row(text)).collect();
        if !menu.is_empty() {
            menu.push(MenuItem::Separator);
        }
        for id in READERS {
            menu.push(switch(
                &format!("Show {}", name(id)),
                self.items.shown(id),
                Command::Toggle(id),
            ));
        }
        menu.push(MenuItem::Separator);
        menu.push(switch(
            "Launch at login",
            self.launch_at_login,
            Command::ToggleLaunchAtLogin,
        ));
        menu.push(MenuItem::Separator);
        menu.push(
            StandardItem {
                label: "Quit".to_owned(),
                activate: Box::new(|hub: &mut Hub| hub.send(Command::Quit)),
                ..Default::default()
            }
            .into(),
        );
        menu
    }
}

/// What an item shows: its themed icon, its label text and its title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Look {
    pub icon: String,
    pub label: String,
    pub title: String,
}

pub fn percent_look(id: ReaderId, value: u8) -> Look {
    Look {
        icon: metric_icon(id).to_owned(),
        label: percent_label(value),
        title: summary::title(name(id), Some(value)),
    }
}

pub fn battery_look(label: BatteryLabel) -> Look {
    Look {
        icon: battery_icon(label),
        label: percent_label(label.percent),
        title: summary::title(name(ReaderId::Battery), Some(label.percent)),
    }
}

pub fn network_look(download_text: &str, reading: &Reading) -> Look {
    Look {
        icon: metric_icon(ReaderId::Network).to_owned(),
        label: download_label(download_text),
        title: summary::network_title(reading),
    }
}
