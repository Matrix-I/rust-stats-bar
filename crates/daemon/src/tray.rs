//! The `ksni` tray items: one per metric, plus the hub. `ksni`'s driver thread calls their `Tray`
//! methods and menu callbacks; the main loop changes their fields through `Handle::update`, whose
//! closure runs on the main thread under `ksni`'s lock. Menu clicks become `Command`s for the main
//! loop; nothing here reads or writes files.

use async_channel::Sender;
use ksni::menu::{CheckmarkItem, StandardItem};
use ksni::{Category, Icon, MenuItem};
use statsbar_core::label::{self, Metric};

use crate::app::Command;

/// Shown before a metric's first value, and as the hub's icon.
pub const FALLBACK_ICON: &str = "utilities-system-monitor";

/// The label pixmap for `percent`, or `None` above 100.
pub fn icon(metric: Metric, percent: u8) -> Option<Icon> {
    // SIZE is 20, so the cast cannot truncate.
    let side = label::SIZE as i32;
    label::render(metric, percent).map(|data| Icon {
        width: side,
        height: side,
        data: data.to_vec(),
    })
}

fn row<T>(text: &str) -> MenuItem<T> {
    StandardItem {
        label: text.to_owned(),
        enabled: false,
        ..Default::default()
    }
    .into()
}

pub struct MetricItem {
    pub metric: Metric,
    pub icon: Option<Icon>,
    pub title: String,
    pub rows: Vec<String>,
}

impl MetricItem {
    pub fn new(metric: Metric) -> MetricItem {
        MetricItem {
            metric,
            icon: None,
            title: name(metric).to_owned(),
            rows: Vec::new(),
        }
    }
}

/// The name in titles and the hub's rows.
pub fn name(metric: Metric) -> &'static str {
    match metric {
        Metric::Cpu => "CPU",
        Metric::Ram => "Memory",
    }
}

impl ksni::Tray for MetricItem {
    fn id(&self) -> String {
        match self.metric {
            Metric::Cpu => "statsbar-cpu",
            Metric::Ram => "statsbar-memory",
        }
        .to_owned()
    }

    fn category(&self) -> Category {
        Category::Hardware
    }

    fn title(&self) -> String {
        self.title.clone()
    }

    /// Empty once a pixmap exists, so the panel draws the label and not the themed icon.
    fn icon_name(&self) -> String {
        if self.icon.is_some() {
            String::new()
        } else {
            FALLBACK_ICON.to_owned()
        }
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        self.icon.iter().cloned().collect()
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        self.rows.iter().map(|text| row(text)).collect()
    }
}

pub struct Hub {
    pub rows: Vec<String>,
    pub show_cpu: bool,
    pub show_memory: bool,
    pub launch_at_login: bool,
    commands: Sender<Command>,
}

impl Hub {
    pub fn new(
        commands: Sender<Command>,
        show_cpu: bool,
        show_memory: bool,
        launch_at_login: bool,
    ) -> Hub {
        Hub {
            rows: Vec::new(),
            show_cpu,
            show_memory,
            launch_at_login,
            commands,
        }
    }

    /// Never blocks: this runs on the tray thread, which also answers the panel's D-Bus calls. A
    /// click while the queue is full is dropped and logged; the checkmark then still shows the
    /// state the main loop last set.
    fn send(&self, command: Command) {
        if let Err(error) = self.commands.try_send(command) {
            eprintln!("statsbard: menu click dropped: {error}");
        }
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
        menu.push(switch("Show CPU", self.show_cpu, Command::ToggleCpu));
        menu.push(switch(
            "Show Memory",
            self.show_memory,
            Command::ToggleMemory,
        ));
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
