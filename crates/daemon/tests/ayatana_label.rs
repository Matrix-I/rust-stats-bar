//! The vendored `ksni` patch: an item publishes `XAyatanaLabel` and `XAyatanaLabelGuide`, and
//! signals `XAyatanaNewLabel` once per update that changes either, never for one that changes
//! neither. It runs on a private `dbus-daemon`, so the desktop's panel never sees the test item.

use std::future::Future;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use futures_lite::{StreamExt, future};
use ksni::TrayMethods;
use zbus::names::InterfaceName;
use zbus::{MatchRule, MessageStream};

const SNI_PATH: &str = "/StatusNotifierItem";
const SNI_INTERFACE: &str = "org.kde.StatusNotifierItem";
/// `ksni` names each item `org.kde.StatusNotifierItem-<pid>-<n>`.
const ITEM_PREFIX: &str = "org.kde.StatusNotifierItem-";
/// Generous for a local bus; it only has to tell "slow" from "hung".
const TIMEOUT: Duration = Duration::from_secs(5);
/// Lines a child may print before the one awaited (libtest's own); bounds every wait.
const MAX_SKIPPED_LINES: usize = 16;
/// Set only in the re-run that plays the item; see `label_item_child`.
const CHILD_ENV: &str = "STATSBAR_TEST_LABEL_CHILD";
/// Printed by the item once it is on the bus. libtest may print `test … ` on the same line
/// when tests run on one thread, so the parent looks for it with `contains`.
const READY: &str = "statsbar-label-item-ready";
/// Printed by the item after each command is applied.
const APPLIED: &str = "statsbar-label-item-applied";

struct LabelTray {
    label: String,
    guide: String,
}

impl ksni::Tray for LabelTray {
    fn id(&self) -> String {
        "statsbar-test-label".to_owned()
    }

    fn x_ayatana_label(&self) -> String {
        self.label.clone()
    }

    fn x_ayatana_label_guide(&self) -> String {
        self.guide.clone()
    }
}

/// Kills the process on drop, so a failed assertion never leaks a bus or an item.
struct Reaped(Child);

impl Drop for Reaped {
    fn drop(&mut self) {
        // Either may fail only because the process has already exited, which is the goal.
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// A child's stdout lines, read on a thread so every wait below has a timeout.
fn lines(out: impl Read + Send + 'static) -> mpsc::Receiver<String> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    receiver
}

/// The first line that `wanted` accepts, skipping at most `MAX_SKIPPED_LINES` others.
fn wait_for(lines: &mpsc::Receiver<String>, wanted: impl Fn(&str) -> bool, what: &str) -> String {
    for _ in 0..=MAX_SKIPPED_LINES {
        match lines.recv_timeout(TIMEOUT) {
            Ok(line) if wanted(&line) => return line,
            Ok(_) => {}
            Err(error) => panic!("{what}: {error}"),
        }
    }
    panic!("{what}: more than {MAX_SKIPPED_LINES} other lines")
}

async fn within<T>(what: &str, call: impl Future<Output = T>) -> T {
    future::or(call, async {
        async_io::Timer::after(TIMEOUT).await;
        panic!("{what} timed out after {TIMEOUT:?}")
    })
    .await
}

async fn text_property(properties: &zbus::fdo::PropertiesProxy<'_>, name: &str) -> String {
    let interface = InterfaceName::from_static_str(SNI_INTERFACE).expect("valid interface");
    let value = within(name, properties.get(interface, name))
        .await
        .unwrap_or_else(|error| panic!("{name}: {error}"));
    String::try_from(value).unwrap_or_else(|error| panic!("{name}: {error}"))
}

/// The item's half of `the_item_publishes_and_signals_its_label`. It does nothing unless that
/// test starts it with `CHILD_ENV` and a private bus address. Each stdin line is one update:
/// `label <text>`, `guide <text>`, `both <label> <guide>`, or `same` (an update that changes
/// nothing). It exits at the end of stdin.
#[test]
#[ignore = "the item half of the_item_publishes_and_signals_its_label"]
fn label_item_child() {
    if std::env::var_os(CHILD_ENV).is_none() {
        return;
    }
    async_io::block_on(async {
        let item = LabelTray {
            label: "37%".to_owned(),
            guide: "100%".to_owned(),
        };
        let handle = item
            .assume_sni_available(true)
            .spawn()
            .await
            .expect("spawn on the private bus");
        println!("{READY}");
        for line in std::io::stdin().lock().lines() {
            let line = line.expect("read a command");
            let words: Vec<String> = line.split(' ').map(str::to_owned).collect();
            handle
                .update(move |tray: &mut LabelTray| match words.as_slice() {
                    [command, label] if command == "label" => tray.label = label.clone(),
                    [command, guide] if command == "guide" => tray.guide = guide.clone(),
                    [command, label, guide] if command == "both" => {
                        tray.label = label.clone();
                        tray.guide = guide.clone();
                    }
                    [command] if command == "same" => {}
                    other => panic!("unknown command {other:?}"),
                })
                .await
                .expect("the item is still running");
            println!("{APPLIED}");
        }
        handle.shutdown().await;
    });
}

/// Sends one command and waits until the item has applied it, so its signal (if any) is on
/// the bus before the next command.
fn send(item: &mut ChildStdin, applied: &mpsc::Receiver<String>, command: &str) {
    writeln!(item, "{command}").unwrap_or_else(|error| panic!("{command}: {error}"));
    wait_for(applied, |line| line.contains(APPLIED), command);
}

#[test]
fn the_item_publishes_and_signals_its_label() {
    let mut bus = Reaped(
        Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("run /usr/bin/dbus-daemon (Ubuntu package dbus-daemon)"),
    );
    let bus_lines = lines(bus.0.stdout.take().expect("piped stdout"));
    let address = wait_for(&bus_lines, |line| line.starts_with("unix:"), "bus address");

    let mut item = Reaped(
        Command::new(std::env::current_exe().expect("test binary path"))
            .args(["--exact", "label_item_child", "--ignored", "--nocapture"])
            .env(CHILD_ENV, "1")
            .env("DBUS_SESSION_BUS_ADDRESS", &address)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("start the item"),
    );
    let item_lines = lines(item.0.stdout.take().expect("piped stdout"));
    let mut commands = item.0.stdin.take().expect("piped stdin");
    wait_for(&item_lines, |line| line.contains(READY), "item ready");

    async_io::block_on(async {
        let connection = within("connect", async {
            zbus::connection::Builder::address(address.as_str())
                .expect("valid address")
                .build()
                .await
        })
        .await
        .expect("connect to the private bus");
        let dbus = within("DBus proxy", zbus::fdo::DBusProxy::new(&connection))
            .await
            .expect("DBus proxy");
        let names = within("ListNames", dbus.list_names())
            .await
            .expect("list names");
        let item_name = names
            .iter()
            .map(|name| name.to_string())
            .find(|name| name.starts_with(ITEM_PREFIX))
            .expect("the item owns its name");
        let properties = within("properties proxy", async {
            zbus::fdo::PropertiesProxy::builder(&connection)
                .destination(item_name.as_str())
                .expect("valid name")
                .path(SNI_PATH)
                .expect("valid path")
                .build()
                .await
        })
        .await
        .expect("properties proxy");
        let rule = MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .interface(SNI_INTERFACE)
            .expect("valid interface")
            .member("XAyatanaNewLabel")
            .expect("valid member")
            .build();
        // Subscribed before the first command, so no signal can be missed.
        let mut signals = within(
            "AddMatch",
            MessageStream::for_match_rule(rule, &connection, None),
        )
        .await
        .expect("subscribe");
        let mut next_signal = async || -> (String, String) {
            let message = within("XAyatanaNewLabel", signals.next())
                .await
                .expect("the stream is open")
                .expect("a signal");
            message.body().deserialize().expect("an (ss) body")
        };

        assert_eq!(text_property(&properties, "XAyatanaLabel").await, "37%");
        assert_eq!(
            text_property(&properties, "XAyatanaLabelGuide").await,
            "100%"
        );

        send(&mut commands, &item_lines, "label 38%");
        assert_eq!(next_signal().await, ("38%".to_owned(), "100%".to_owned()));
        assert_eq!(text_property(&properties, "XAyatanaLabel").await, "38%");

        send(&mut commands, &item_lines, "guide ↓999M");
        assert_eq!(next_signal().await, ("38%".to_owned(), "↓999M".to_owned()));
        assert_eq!(
            text_property(&properties, "XAyatanaLabelGuide").await,
            "↓999M"
        );

        // Both at once, then nothing: if the label check short-circuited the guide's, the
        // guide's stale hash would send a second signal on the update that changes nothing.
        send(&mut commands, &item_lines, "both 39% 100%");
        assert_eq!(next_signal().await, ("39%".to_owned(), "100%".to_owned()));
        send(&mut commands, &item_lines, "same");
        send(&mut commands, &item_lines, "label 40%");
        assert_eq!(
            next_signal().await,
            ("40%".to_owned(), "100%".to_owned()),
            "the update that changed nothing sent no signal"
        );
    });
    // Closing stdin ends the item's loop; `Reaped` then reaps both processes.
    drop(commands);
}
