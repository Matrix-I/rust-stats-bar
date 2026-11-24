//! The daemon's event loop, on the main thread. One timer serves every reader. Menu clicks
//! arrive from the hub, power-profile changes from the system bus, and the panel's requests from
//! the socket. The single `zbus` connection the daemon owns is ticked here, not on a thread of its
//! own (spike S2).

use std::error::Error;
use std::future::Future;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use async_channel::{Receiver, Sender};
use futures_lite::{StreamExt, future};
use ksni::{Handle, TrayMethods};
use statsbar_core::ipc::{Event, Page};
use statsbar_core::schedule::ReaderId;
use statsbar_readers::battery::BatteryReader;
use statsbar_readers::cpu::CpuReader;
use statsbar_readers::fan::FanReader;
use statsbar_readers::memory::MemoryReader;
use statsbar_readers::network::NetworkReader;

use crate::app::{App, Command, PAGES, READERS, page_of, reader_of, slot};
use crate::config::Items;
use crate::ipc::{Change, Options, Server};
use crate::pages::{Pages, System};
use crate::power::{ProfileWatch, is_power_saver};
use crate::tray::{Hub, Look, MetricItem, battery_look, network_look, percent_look};
use crate::{autostart, config, paths, summary};

pub type BoxError = Box<dyn Error>;

/// Connecting and registering an item only has to tell "slow" from "hung"; the value S2 ran with.
const SETUP_TIMEOUT: Duration = Duration::from_secs(5);
/// How long one tray update or property read may hold up the loop; the value S2 ran with.
const CALL_TIMEOUT: Duration = Duration::from_secs(1);
/// Clicks come at human speed; a few slots absorb a double click while the loop is reading.
const COMMAND_CAPACITY: usize = 8;

/// Runs `call`, giving up after `timeout`, so a hung bus or watcher is reported by name instead
/// of stalling the loop.
async fn bounded<T>(
    what: &str,
    timeout: Duration,
    call: impl Future<Output = T>,
) -> Result<T, BoxError> {
    future::or(async { Ok(call.await) }, async {
        async_io::Timer::after(timeout).await;
        Err(format!("{what} timed out after {timeout:?}").into())
    })
    .await
}

fn elapsed_nanos(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

/// Reuses the open reader, or opens one, then reads it. A failed read leaves the reader in its
/// slot; the caller's `read_failed` closes it, so the next attempt reopens its files.
fn read_with<R, T>(
    slot: &mut Option<R>,
    open: impl FnOnce() -> io::Result<R>,
    read: impl FnOnce(&mut R) -> io::Result<T>,
) -> io::Result<T> {
    let reader = match slot.take() {
        Some(reader) => reader,
        None => open()?,
    };
    read(slot.insert(reader))
}

pub async fn run() -> Result<(), BoxError> {
    let config_home = paths::config_home(|key| std::env::var_os(key))
        .ok_or("neither XDG_CONFIG_HOME nor HOME is an absolute path")?;
    let runtime_dir = paths::runtime_dir(|key| std::env::var_os(key))
        .ok_or("XDG_RUNTIME_DIR is not an absolute path")?;
    let system_facts = System {
        own_uid: rustix::process::geteuid().as_raw(),
        page_size: u64::try_from(rustix::param::page_size())?,
        ticks_per_second: rustix::param::clock_ticks_per_second(),
    };
    // Before any tray item: a second daemon must leave no trace.
    let server = Server::bind(
        &runtime_dir,
        Options {
            allowed_uid: system_facts.own_uid,
            handshake_timeout: SETUP_TIMEOUT,
        },
    )?;
    let root = PathBuf::from("/");
    let pages = Pages::new(&root, system_facts);
    let config_file = paths::config_file(&config_home);
    let autostart_file = paths::autostart_file(&config_home);

    let loaded = config::load(&config_file);
    if let Some(warning) = &loaded.warning {
        eprintln!("statsbard: {warning}");
    }
    let launch_at_login = autostart::is_enabled(&autostart_file).unwrap_or_else(|error| {
        eprintln!("statsbard: {}: {error}", autostart_file.display());
        false
    });

    let (sender, receiver) = async_channel::bounded(COMMAND_CAPACITY);
    let app = App::new(loaded.config, launch_at_login);
    let mut daemon = Daemon::start(
        app,
        sender,
        server,
        pages,
        Files {
            root,
            config: config_file,
            config_writable: loaded.writable,
            autostart: autostart_file,
        },
    )
    .await?;

    let system = connect_system_bus().await;

    let work = daemon.run(receiver, system.as_ref());
    let drive = async {
        match &system {
            Some(connection) => loop {
                connection.executor().tick().await;
            },
            None => future::pending().await,
        }
    };
    future::or(work, drive).await
}

/// The daemon's one `zbus` connection, used for power profiles. Any failure, even a malformed
/// `DBUS_SYSTEM_BUS_ADDRESS`, leaves power saver off (requirement 6) instead of stopping the daemon.
async fn connect_system_bus() -> Option<zbus::Connection> {
    let builder = match zbus::connection::Builder::system() {
        Ok(builder) => builder.internal_executor(false),
        Err(error) => {
            eprintln!("statsbard: no system bus address, power saver stays off: {error}");
            return None;
        }
    };
    match bounded("system bus connect", SETUP_TIMEOUT, builder.build()).await {
        Ok(Ok(connection)) => Some(connection),
        Ok(Err(error)) => {
            eprintln!("statsbard: no system bus, power saver stays off: {error}");
            None
        }
        Err(error) => {
            eprintln!("statsbard: {error}; power saver stays off");
            None
        }
    }
}

struct Files {
    root: PathBuf,
    config: PathBuf,
    config_writable: bool,
    autostart: PathBuf,
}

/// The open readers. A hidden metric's reader is dropped, which closes its files.
#[derive(Default)]
struct Readers {
    cpu: Option<CpuReader>,
    memory: Option<MemoryReader>,
    battery: Option<BatteryReader>,
    fan: Option<FanReader>,
    network: Option<NetworkReader>,
}

impl Readers {
    fn close(&mut self, id: ReaderId) {
        match id {
            ReaderId::Cpu => self.cpu = None,
            ReaderId::Memory => self.memory = None,
            ReaderId::Battery => self.battery = None,
            ReaderId::Fan => self.fan = None,
            ReaderId::Network => self.network = None,
        }
    }
}

enum Wake {
    Timer,
    Command(Command),
    CommandsClosed,
    Profile(Result<String, BoxError>),
    ProfileEnded,
    Panel(Change),
}

struct Daemon {
    app: App,
    files: Files,
    hub: Handle<Hub>,
    hub_shown: Option<(Vec<String>, Items, bool)>,
    /// Indexed by `app::slot`.
    items: [Option<Handle<MetricItem>>; 5],
    readers: Readers,
    server: Server,
    pages: Pages,
    /// Each page as last published, indexed by `page_slot`, for a watcher that joins later.
    published: [Option<Event>; 2],
}

fn page_slot(page: Page) -> usize {
    match page {
        Page::Cpu => 0,
        Page::Memory => 1,
    }
}

impl Daemon {
    async fn start(
        app: App,
        commands: Sender<Command>,
        server: Server,
        pages: Pages,
        files: Files,
    ) -> Result<Daemon, BoxError> {
        let hub = Hub::new(commands, app.config.items.clone(), app.launch_at_login);
        let hub = bounded("hub item spawn", SETUP_TIMEOUT, hub.spawn()).await??;
        let mut daemon = Daemon {
            app,
            files,
            hub,
            hub_shown: None,
            items: Default::default(),
            readers: Readers::default(),
            server,
            pages,
            published: Default::default(),
        };
        for id in READERS {
            if daemon.app.is_visible(id) {
                daemon.show_item(id).await;
            }
        }
        Ok(daemon)
    }

    async fn run(
        &mut self,
        commands: Receiver<Command>,
        system: Option<&zbus::Connection>,
    ) -> Result<(), BoxError> {
        let mut profiles = match system {
            Some(connection) => {
                match bounded(
                    "power profile watch",
                    SETUP_TIMEOUT,
                    ProfileWatch::start(connection),
                )
                .await
                {
                    Ok(Ok(watch)) => Some(watch),
                    Ok(Err(error)) => {
                        eprintln!("statsbard: power profiles unavailable: {error}");
                        None
                    }
                    Err(error) => {
                        eprintln!("statsbard: {error}");
                        None
                    }
                }
            }
            None => None,
        };

        let start = Instant::now();
        loop {
            let now = elapsed_nanos(start);
            self.sync_pages();
            for id in self.app.due(now) {
                self.sample(id, now).await;
            }
            self.refresh_hub().await;

            let wake_at = self.app.next_wake_nanos();
            let timer = async {
                match wake_at {
                    Some(at) => {
                        let wait = at.saturating_sub(elapsed_nanos(start));
                        async_io::Timer::after(Duration::from_nanos(wait)).await;
                        Wake::Timer
                    }
                    None => future::pending().await,
                }
            };
            let command = async {
                match commands.recv().await {
                    Ok(command) => Wake::Command(command),
                    Err(_) => Wake::CommandsClosed,
                }
            };
            let profile = async {
                let Some(watch) = profiles.as_mut() else {
                    return future::pending().await;
                };
                match watch.changes.next().await {
                    Some(change) => {
                        match bounded("ActiveProfile read", CALL_TIMEOUT, change.get()).await {
                            Ok(Ok(value)) => Wake::Profile(Ok(value)),
                            Ok(Err(error)) => Wake::Profile(Err(error.into())),
                            Err(error) => Wake::Profile(Err(error)),
                        }
                    }
                    None => Wake::ProfileEnded,
                }
            };

            let panel = async { Wake::Panel(self.server.next().await) };

            // Bound first, so the four futures (and their borrows of `profiles` and the server)
            // are gone before the arms below change either.
            let wake = future::or(timer, future::or(command, future::or(profile, panel))).await;
            match wake {
                Wake::Timer => {}
                Wake::Command(Command::Quit) | Wake::CommandsClosed => break,
                Wake::Command(command) => self.handle(command).await,
                Wake::Profile(Ok(profile)) => self.app.power_saver = is_power_saver(&profile),
                Wake::Profile(Err(error)) => {
                    eprintln!("statsbard: reading the power profile failed: {error}");
                }
                Wake::Panel(change) => self.handle_panel(change, elapsed_nanos(start)),
                Wake::ProfileEnded => {
                    eprintln!("statsbard: power profile updates ended; keeping the last value");
                    profiles = None;
                }
            }
        }
        self.shutdown().await;
        Ok(())
    }

    /// Reads one metric, records it, and redraws its item.
    async fn sample(&mut self, id: ReaderId, now: u64) {
        let root = &self.files.root;
        let readers = &mut self.readers;
        let app = &mut self.app;
        let result = match id {
            ReaderId::Cpu => read_with(&mut readers.cpu, || CpuReader::open(root), CpuReader::read)
                .map(|ticks| {
                    app.record_cpu(now, ticks)
                        .map(|value| percent_look(id, value))
                }),
            ReaderId::Memory => read_with(
                &mut readers.memory,
                || MemoryReader::open(root),
                MemoryReader::read,
            )
            .map(|info| {
                app.record_memory(now, &info)
                    .map(|value| percent_look(id, value))
            }),
            ReaderId::Battery => read_with(
                &mut readers.battery,
                || BatteryReader::open(root),
                BatteryReader::read,
            )
            .map(|reading| {
                app.record_battery(now, &reading.info, reading.adapter_online)
                    .map(battery_look)
            }),
            ReaderId::Fan => read_with(&mut readers.fan, || FanReader::open(root), FanReader::read)
                .map(|speed| {
                    app.record_fan(now, speed)
                        .map(|value| percent_look(id, value))
                }),
            ReaderId::Network => read_with(
                &mut readers.network,
                || NetworkReader::open(root),
                NetworkReader::read,
            )
            .map(|sample| {
                let download = app.record_network(now, sample.as_ref())?;
                app.network()
                    .map(|status| network_look(&download, &status.reading))
            }),
        };
        match result {
            Ok(redraw) => {
                self.refresh_item(id, redraw).await;
                if let Some(page) = page_of(id) {
                    self.publish(page, now);
                }
            }
            Err(error) => self.read_failed(id, now, &error),
        }
    }

    /// Follows who watches each page. Run every pass, since a connection also ends when a write
    /// to it fails.
    fn sync_pages(&mut self) {
        for page in PAGES {
            let id = reader_of(page);
            let open = self.server.watched(page);
            self.app.set_page_open(id, open);
            if !open {
                self.pages.close(page);
                self.published[page_slot(page)] = None;
            }
            if !self.app.is_read(id) {
                self.readers.close(id);
            }
        }
    }

    fn handle_panel(&mut self, change: Change, now: u64) {
        match change {
            Change::Subscribed(page) => {
                self.sync_pages();
                // What is known already, so a shown item's page does not open empty. Nothing is
                // read for it: a read here would be outside the cadence.
                if let Some(event) = &self.published[page_slot(page)] {
                    self.server.welcome(page, event);
                }
            }
            Change::Refresh(page) => {
                // An ignored refresh must not make the next reading a full scan either.
                if self.app.refresh(reader_of(page), now) {
                    self.pages.refresh(page);
                }
            }
            Change::Unwatched => {}
        }
    }

    /// Sends a page to its watchers. A page nobody watches costs nothing here.
    fn publish(&mut self, page: Page, now: u64) {
        if !self.server.watched(page) {
            return;
        }
        let (event, problems) = match page {
            Page::Cpu => {
                let (snapshot, problems) =
                    self.pages.cpu(now, self.app.cpu(), self.app.power_saver);
                (Event::Cpu(snapshot), problems)
            }
            Page::Memory => {
                // Nothing to show until the first reading, which is due at once.
                let Some(&buckets) = self.app.memory() else {
                    return;
                };
                let (snapshot, problems) = self.pages.memory(now, buckets);
                (Event::Memory(snapshot), problems)
            }
        };
        for (what, error) in problems {
            eprintln!("statsbard: reading {what} for the {page:?} page failed: {error}");
        }
        self.server.publish(page, &event);
        self.published[page_slot(page)] = Some(event);
    }

    /// Closes the reader, so the next attempt reopens its files, and logs the first failure in a
    /// row.
    fn read_failed(&mut self, id: ReaderId, now: u64, error: &io::Error) {
        self.readers.close(id);
        if self.app.record_error(id, now) {
            eprintln!("statsbard: reading {id:?} failed: {error}");
        }
    }

    async fn refresh_item(&self, id: ReaderId, redraw: Option<Look>) {
        let Some(handle) = &self.items[slot(id)] else {
            return;
        };
        let rows = self.app.rows(id);
        // A missed update is not retried. The rows are resent on the next sample, but the icon,
        // label and title only when the label changes again, because `App` has already recorded
        // the new value. A 1 s stall on `ksni`'s lock is itself worth investigating, and the log
        // shows it.
        update(
            handle,
            "metric item update",
            move |item: &mut MetricItem| {
                item.rows = rows;
                if let Some(look) = redraw {
                    item.icon = look.icon;
                    item.label = look.label;
                    item.title = look.title;
                }
            },
        )
        .await;
    }

    async fn refresh_hub(&mut self) {
        let shown = (
            summary::hub_rows(&self.app.overview()),
            self.app.config.items.clone(),
            self.app.launch_at_login,
        );
        if self.hub_shown.as_ref() == Some(&shown) {
            return;
        }
        let (rows, items, launch_at_login) = shown.clone();
        let delivered = update(&self.hub, "hub update", move |hub: &mut Hub| {
            hub.rows = rows;
            hub.items = items;
            hub.launch_at_login = launch_at_login;
        })
        .await;
        // Only a delivered update counts as shown, so a timed-out one is retried next pass.
        if delivered {
            self.hub_shown = Some(shown);
        }
    }

    async fn handle(&mut self, command: Command) {
        match command {
            Command::Toggle(id) => self.toggle(id).await,
            Command::ToggleLaunchAtLogin => self.toggle_launch_at_login(),
            Command::Quit => {}
        }
    }

    async fn toggle(&mut self, id: ReaderId) {
        if self.app.toggle(id) {
            self.show_item(id).await;
        } else {
            self.hide_item(id).await;
        }
        let files = &self.files;
        if let Err(error) = config::persist(&files.config, &self.app.config, files.config_writable)
        {
            eprintln!("statsbard: {}: {error}", files.config.display());
        }
    }

    async fn show_item(&mut self, id: ReaderId) {
        match bounded(
            "metric item spawn",
            SETUP_TIMEOUT,
            MetricItem::new(id).spawn(),
        )
        .await
        {
            Ok(Ok(handle)) => self.items[slot(id)] = Some(handle),
            Ok(Err(error)) => eprintln!("statsbard: showing {id:?} failed: {error}"),
            Err(error) => eprintln!("statsbard: {error}"),
        }
    }

    /// Removes the item and, unless its page is open, closes its files: a hidden metric nobody
    /// watches costs nothing.
    async fn hide_item(&mut self, id: ReaderId) {
        if !self.app.is_read(id) {
            self.readers.close(id);
        }
        if let Some(handle) = self.items[slot(id)].take() {
            shut_down(&handle, "metric item shutdown").await;
        }
    }

    fn toggle_launch_at_login(&mut self) {
        let path = &self.files.autostart;
        match autostart::toggle(path, self.app.launch_at_login, std::env::current_exe) {
            Ok(enabled) => self.app.launch_at_login = enabled,
            Err(error) => eprintln!("statsbard: {}: {error}", path.display()),
        }
    }

    async fn shutdown(&mut self) {
        for item in &mut self.items {
            if let Some(handle) = item.take() {
                shut_down(&handle, "metric item shutdown").await;
            }
        }
        shut_down(&self.hub, "hub shutdown").await;
    }
}

/// Applies `change` to the item and lets `ksni` emit the D-Bus signals. Returns whether the
/// update was delivered.
async fn update<T: ksni::Tray>(
    handle: &Handle<T>,
    what: &str,
    change: impl FnOnce(&mut T),
) -> bool {
    match bounded(what, CALL_TIMEOUT, handle.update(change)).await {
        Ok(Some(())) => true,
        Ok(None) => {
            eprintln!("statsbard: {what}: the tray service has shut down");
            false
        }
        Err(error) => {
            eprintln!("statsbard: {error}");
            false
        }
    }
}

async fn shut_down<T>(handle: &Handle<T>, what: &str) {
    if let Err(error) = bounded(what, CALL_TIMEOUT, handle.shutdown()).await {
        eprintln!("statsbard: {error}");
    }
}
