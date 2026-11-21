use ksni::Tray;
use ksni::menu::MenuItem;
use statsbar_core::schedule::ReaderId;
use statsbar_core::throughput::Reading;
use statsbard::app::BatteryLabel;
use statsbard::app::Command;
use statsbard::app::READERS;
use statsbard::config::Items;
use statsbard::tray::{self, Hub, Look, MetricItem};

fn hub(capacity: usize) -> (Hub, async_channel::Receiver<Command>) {
    let (sender, receiver) = async_channel::bounded(capacity);
    let items = Items {
        cpu: true,
        memory: false,
        battery: true,
        fan: false,
        network: true,
    };
    (Hub::new(sender, items, true), receiver)
}

fn labels<T>(menu: &[MenuItem<T>]) -> Vec<String> {
    menu.iter()
        .map(|item| match item {
            MenuItem::Standard(item) => item.label.clone(),
            MenuItem::Checkmark(item) => {
                format!("[{}] {}", if item.checked { "x" } else { " " }, item.label)
            }
            MenuItem::Separator => "---".to_owned(),
            other => format!("{other:?}"),
        })
        .collect()
}

fn click(hub: &mut Hub, label: &str) {
    let menu = hub.menu();
    let activate = menu
        .iter()
        .find_map(|item| match item {
            MenuItem::Checkmark(item) if item.label == label => Some(&item.activate),
            MenuItem::Standard(item) if item.label == label => Some(&item.activate),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no menu item {label}"));
    activate(hub);
}

#[test]
fn the_hub_menu_lists_rows_switches_and_quit() {
    let (mut hub, _receiver) = hub(8);
    hub.rows = vec!["CPU 24 %".to_owned()];
    assert_eq!(
        labels(&hub.menu()),
        [
            "CPU 24 %",
            "---",
            "[x] Show CPU",
            "[ ] Show Memory",
            "[x] Show Battery",
            "[ ] Show Fan",
            "[x] Show Network",
            "---",
            "[x] Launch at login",
            "---",
            "Quit",
        ]
    );
}

/// The first refresh then only relabels rows. Adding rows would change the layout, which makes
/// ksni renumber every menu id, so a click the panel sends with an old id would be lost.
#[test]
fn a_new_hub_already_has_a_row_for_each_shown_metric() {
    let (hub, _receiver) = hub(8);
    assert_eq!(
        labels(&hub.menu())[..5],
        ["CPU —", "Battery —", "Network —", "---", "[x] Show CPU"]
    );
}

#[test]
fn hub_rows_are_not_clickable() {
    let (mut hub, _receiver) = hub(8);
    hub.rows = vec!["CPU 24 %".to_owned()];
    match &hub.menu()[0] {
        MenuItem::Standard(item) => assert!(!item.enabled),
        other => panic!("expected a row, got {other:?}"),
    }
}

#[test]
fn each_switch_and_quit_send_their_command() {
    let (mut hub, receiver) = hub(8);
    for (label, expected) in [
        ("Show CPU", Command::Toggle(ReaderId::Cpu)),
        ("Show Memory", Command::Toggle(ReaderId::Memory)),
        ("Show Battery", Command::Toggle(ReaderId::Battery)),
        ("Show Fan", Command::Toggle(ReaderId::Fan)),
        ("Show Network", Command::Toggle(ReaderId::Network)),
        ("Launch at login", Command::ToggleLaunchAtLogin),
        ("Quit", Command::Quit),
    ] {
        click(&mut hub, label);
        assert_eq!(receiver.try_recv(), Ok(expected), "{label}");
    }
}

#[test]
fn a_full_command_queue_drops_the_click_instead_of_blocking_the_tray_thread() {
    let (mut hub, receiver) = hub(1);
    click(&mut hub, "Quit");
    click(&mut hub, "Show CPU");
    assert_eq!(receiver.try_recv(), Ok(Command::Quit));
    assert!(receiver.try_recv().is_err());
}

#[test]
fn a_metric_item_shows_its_icon_alone_until_its_first_value() {
    let mut item = MetricItem::new(ReaderId::Cpu);
    assert_eq!(item.icon_name(), "computer-chip-symbolic");
    assert_eq!(item.x_ayatana_label(), "");
    assert_eq!(item.x_ayatana_label_guide(), "100%");
    assert!(item.icon_pixmap().is_empty());

    item.label = tray::percent_label(42);
    assert_eq!(item.x_ayatana_label(), "42%");
    assert!(item.icon_pixmap().is_empty(), "the shell draws the text");
}

#[test]
fn every_metric_has_its_yaru_icon() {
    let icons: Vec<&str> = READERS.map(tray::metric_icon).to_vec();
    assert_eq!(
        icons,
        [
            "computer-chip-symbolic",
            "drive-harddisk-solidstate-symbolic",
            "battery-missing-symbolic",
            "weather-windy-symbolic",
            "network-transmit-receive-symbolic",
        ]
    );
    let icons: Vec<String> = READERS.map(|id| MetricItem::new(id).icon_name()).to_vec();
    assert_eq!(
        icons,
        READERS.map(|id| tray::metric_icon(id).to_owned()).to_vec()
    );
}

#[test]
fn the_battery_icon_floors_to_tens_and_has_the_bolt_only_while_charging() {
    for (percent, charging, icon) in [
        (76, false, "battery-level-70-symbolic"),
        (76, true, "battery-level-70-charging-symbolic"),
        (9, false, "battery-level-0-symbolic"),
        (0, true, "battery-level-0-charging-symbolic"),
        (99, false, "battery-level-90-symbolic"),
        (100, false, "battery-level-100-symbolic"),
        (100, true, "battery-level-100-charging-symbolic"),
        (255, false, "battery-level-100-symbolic"),
    ] {
        assert_eq!(
            tray::battery_icon(BatteryLabel { percent, charging }),
            icon,
            "{percent} {charging}"
        );
    }
}

#[test]
fn labels_are_a_bare_percent_or_the_download_rate() {
    assert_eq!(tray::percent_label(0), "0%");
    assert_eq!(tray::percent_label(37), "37%");
    assert_eq!(tray::percent_label(100), "100%");
    assert_eq!(tray::download_label("1.5M"), "↓1.5M");
    assert_eq!(tray::download_label("0B"), "↓0B");
}

#[test]
fn each_guide_is_its_widest_label() {
    for id in READERS {
        let expected = if id == ReaderId::Network {
            "↓999M"
        } else {
            "100%"
        };
        assert_eq!(tray::label_guide(id), expected, "{id:?}");
        assert_eq!(
            MetricItem::new(id).x_ayatana_label_guide(),
            expected,
            "{id:?}"
        );
    }
}

#[test]
fn metric_item_ids_are_stable_and_rows_are_not_clickable() {
    let ids: Vec<String> = READERS.map(|id| MetricItem::new(id).id()).to_vec();
    assert_eq!(
        ids,
        [
            "statsbar-cpu",
            "statsbar-memory",
            "statsbar-battery",
            "statsbar-fan",
            "statsbar-network",
        ]
    );
    let mut memory = MetricItem::new(ReaderId::Memory);
    memory.rows = vec!["App 6.00 GB".to_owned()];
    match &memory.menu()[0] {
        MenuItem::Standard(item) => {
            assert_eq!(item.label, "App 6.00 GB");
            assert!(!item.enabled);
        }
        other => panic!("expected a row, got {other:?}"),
    }
}

#[test]
fn a_percent_item_shows_its_icon_percent_label_and_titled_name() {
    assert_eq!(
        tray::percent_look(ReaderId::Cpu, 37),
        Look {
            icon: "computer-chip-symbolic".to_owned(),
            label: "37%".to_owned(),
            title: "CPU 37 %".to_owned(),
        }
    );
    assert_eq!(
        tray::percent_look(ReaderId::Fan, 100),
        Look {
            icon: "weather-windy-symbolic".to_owned(),
            label: "100%".to_owned(),
            title: "Fan 100 %".to_owned(),
        }
    );
}

#[test]
fn the_battery_item_shows_a_charging_level_icon_and_the_charge() {
    assert_eq!(
        tray::battery_look(BatteryLabel {
            percent: 76,
            charging: true
        }),
        Look {
            icon: "battery-level-70-charging-symbolic".to_owned(),
            label: "76%".to_owned(),
            title: "Battery 76 %".to_owned(),
        }
    );
}

#[test]
fn the_network_item_labels_the_download_and_titles_both_rates() {
    let reading = Reading {
        download_total: 0,
        upload_total: 0,
        download_rate: 12_000.0,
        upload_rate: 2_000.0,
    };
    assert_eq!(
        tray::network_look("12K", &reading),
        Look {
            icon: "network-transmit-receive-symbolic".to_owned(),
            label: "↓12K".to_owned(),
            title: "Network ↓ 12.0 KB/s ↑ 2.0 KB/s".to_owned(),
        }
    );
}
