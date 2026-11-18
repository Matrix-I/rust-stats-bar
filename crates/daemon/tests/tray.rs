use ksni::Tray;
use ksni::menu::MenuItem;
use statsbar_core::label::{self, Metric};
use statsbard::app::Command;
use statsbard::tray::{self, FALLBACK_ICON, Hub, MetricItem};

fn hub(capacity: usize) -> (Hub, async_channel::Receiver<Command>) {
    let (sender, receiver) = async_channel::bounded(capacity);
    (Hub::new(sender, true, false, true), receiver)
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
            "---",
            "[x] Launch at login",
            "---",
            "Quit",
        ]
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
        ("Show CPU", Command::ToggleCpu),
        ("Show Memory", Command::ToggleMemory),
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
fn a_metric_item_shows_a_themed_icon_until_its_first_value() {
    let mut item = MetricItem::new(Metric::Cpu);
    assert_eq!(item.icon_name(), FALLBACK_ICON);
    assert!(item.icon_pixmap().is_empty());

    item.icon = tray::icon(Metric::Cpu, 42);
    assert_eq!(item.icon_name(), "");
    let pixmaps = item.icon_pixmap();
    assert_eq!(pixmaps.len(), 1);
    assert_eq!((pixmaps[0].width, pixmaps[0].height), (20, 20));
    assert_eq!(
        pixmaps[0].data,
        label::render(Metric::Cpu, 42).expect("in range").to_vec()
    );
}

#[test]
fn an_icon_above_100_is_refused() {
    assert!(tray::icon(Metric::Ram, 101).is_none());
}

#[test]
fn metric_item_ids_are_stable_and_rows_are_not_clickable() {
    assert_eq!(MetricItem::new(Metric::Cpu).id(), "statsbar-cpu");
    let mut memory = MetricItem::new(Metric::Ram);
    assert_eq!(memory.id(), "statsbar-memory");
    memory.rows = vec!["App 6.00 GB".to_owned()];
    match &memory.menu()[0] {
        MenuItem::Standard(item) => {
            assert_eq!(item.label, "App 6.00 GB");
            assert!(!item.enabled);
        }
        other => panic!("expected a row, got {other:?}"),
    }
}
