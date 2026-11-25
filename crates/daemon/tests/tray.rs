use ksni::Tray;
use ksni::menu::MenuItem;
use statsbar_core::schedule::ReaderId;
use statsbar_core::throughput::Reading;
use statsbard::app::BatteryLabel;
use statsbard::app::Command;
use statsbard::app::READERS;
use statsbard::config::Items;
use statsbard::summary;
use statsbard::tray::{self, CPU_MENU_ROWS, Hub, Look, MEMORY_MENU_ROWS, MetricItem, OPEN_DETAILS};

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

fn item(reader: ReaderId) -> (MetricItem, async_channel::Receiver<Command>) {
    let (sender, receiver) = async_channel::bounded(1);
    (MetricItem::new(reader, sender), receiver)
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
    let (mut item, _receiver) = item(ReaderId::Cpu);
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
    let icons: Vec<String> = READERS.map(|id| item(id).0.icon_name()).to_vec();
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
        assert_eq!(item(id).0.x_ayatana_label_guide(), expected, "{id:?}");
    }
}

#[test]
fn metric_item_ids_are_stable_and_rows_are_not_clickable() {
    let ids: Vec<String> = READERS.map(|id| item(id).0.id()).to_vec();
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
    let (mut fan, _receiver) = item(ReaderId::Fan);
    fan.rows = vec!["Speed 3000 RPM".to_owned()];
    match &fan.menu()[0] {
        MenuItem::Standard(item) => {
            assert_eq!(item.label, "Speed 3000 RPM");
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

/// (label, enabled, visible) of each menu entry; a separator reads as `---`.
fn entries(menu: &[MenuItem<MetricItem>]) -> Vec<(String, bool, bool)> {
    menu.iter()
        .map(|entry| match entry {
            MenuItem::Standard(item) => (item.label.clone(), item.enabled, item.visible),
            MenuItem::Separator => ("---".to_owned(), false, true),
            other => panic!("unexpected {other:?}"),
        })
        .collect()
}

#[test]
fn cpu_and_memory_menus_open_their_page_above_reserved_rows() {
    for (reader, places) in [
        (ReaderId::Cpu, CPU_MENU_ROWS),
        (ReaderId::Memory, MEMORY_MENU_ROWS),
    ] {
        let (item, _receiver) = item(reader);
        let menu = entries(&item.menu());
        assert_eq!(menu.len(), 2 + places, "{reader:?}");
        assert_eq!(menu[0], (OPEN_DETAILS.to_owned(), true, true));
        assert_eq!(menu[1].0, "---");
        // Kept but hidden until the first value, so showing them changes no menu id.
        assert!(
            menu[2..]
                .iter()
                .all(|(_, enabled, visible)| !enabled && !visible),
            "{reader:?}"
        );
    }
}

#[test]
fn rows_fill_their_places_and_the_rest_stay_hidden() {
    let (mut memory, _receiver) = item(ReaderId::Memory);
    memory.rows = summary::memory_rows(&statsbar_core::memory::MemoryBuckets::default());
    let menu = entries(&memory.menu());
    assert_eq!(menu.len(), 2 + MEMORY_MENU_ROWS);
    assert_eq!(
        menu[2],
        ("Used 0.00 GB of 0.00 GB (0 %)".to_owned(), false, true)
    );
    assert_eq!(menu[7], ("Swap 0.00 GB".to_owned(), false, true));
    // Compressed's place, while zswap is empty.
    assert_eq!(menu[8], (String::new(), false, false));
}

#[test]
fn every_row_the_summaries_give_has_a_place() {
    let busy = statsbar_core::cpu::CpuLoad {
        system_percent: 1.0,
        user_percent: 1.0,
        idle_percent: 98.0,
        per_core_busy: vec![2.0],
    };
    assert_eq!(summary::cpu_rows(&busy).len(), CPU_MENU_ROWS);
    let zswap = statsbar_core::memory::MemoryBuckets {
        compressed: 1,
        ..Default::default()
    };
    assert_eq!(summary::memory_rows(&zswap).len(), MEMORY_MENU_ROWS);
}

#[test]
fn open_details_sends_its_page_and_other_items_have_none() {
    for (reader, page) in [
        (ReaderId::Cpu, statsbar_core::ipc::Page::Cpu),
        (ReaderId::Memory, statsbar_core::ipc::Page::Memory),
    ] {
        let (mut item, receiver) = item(reader);
        let menu = item.menu();
        let MenuItem::Standard(open) = &menu[0] else {
            panic!("expected Open details");
        };
        (open.activate)(&mut item);
        assert_eq!(receiver.try_recv(), Ok(Command::OpenPage(page)));
    }
    for reader in [ReaderId::Battery, ReaderId::Fan, ReaderId::Network] {
        let (item, _receiver) = item(reader);
        assert!(item.menu().is_empty(), "{reader:?}");
    }
}

#[test]
fn a_full_command_queue_drops_open_details_too() {
    let (mut item, receiver) = item(ReaderId::Cpu);
    let menu = item.menu();
    let MenuItem::Standard(open) = &menu[0] else {
        panic!("expected Open details");
    };
    (open.activate)(&mut item);
    (open.activate)(&mut item);
    assert_eq!(
        receiver.try_recv(),
        Ok(Command::OpenPage(statsbar_core::ipc::Page::Cpu))
    );
    assert!(receiver.try_recv().is_err());
}
