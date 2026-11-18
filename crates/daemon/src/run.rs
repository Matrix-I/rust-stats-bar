//! The daemon's event loop, on the main thread. One timer serves every reader. Menu clicks
//! arrive from the hub, and power-profile changes from the system bus. The single `zbus`
//! connection the daemon owns is ticked here, not on a thread of its own (spike S2).

use std::error::Error;
use std::future::Future;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use async_channel::{Receiver, Sender};
use futures_lite::{StreamExt, future};
use ksni::{Handle, TrayMethods};
use statsbar_core::cpu::CoreTicks;
use statsbar_core::label::Metric;
use statsbar_core::memory::MemInfo;
use statsbar_core::schedule::ReaderId;
use statsbar_readers::cpu::CpuReader;
use statsbar_readers::memory::MemoryReader;

use crate::app::{App, Command};
use crate::power::{ProfileWatch, is_power_saver};
use crate::tray::{self, Hub, MetricItem};
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

pub async fn run() -> Result<(), BoxError> {
    let config_home = paths::config_home(|key| std::env::var_os(key))
        .ok_or("neither XDG_CONFIG_HOME nor HOME is an absolute path")?;
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
        Files {
            root: PathBuf::from("/"),
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

enum Wake {
    Timer,
    Command(Command),
    CommandsClosed,
    Profile(Result<String, BoxError>),
    ProfileEnded,
}

struct Daemon {
    app: App,
    files: Files,
    hub: Handle<Hub>,
    hub_shown: Option<(Vec<String>, bool, bool, bool)>,
    cpu_item: Option<Handle<MetricItem>>,
    memory_item: Option<Handle<MetricItem>>,
    cpu_reader: Option<CpuReader>,
    memory_reader: Option<MemoryReader>,
}

impl Daemon {
    async fn start(app: App, commands: Sender<Command>, files: Files) -> Result<Daemon, BoxError> {
        let hub = Hub::new(
            commands,
            app.config.items.cpu,
            app.config.items.memory,
            app.launch_at_login,
        );
        let hub = bounded("hub item spawn", SETUP_TIMEOUT, hub.spawn()).await??;
        let mut daemon = Daemon {
            app,
            files,
            hub,
            hub_shown: None,
            cpu_item: None,
            memory_item: None,
            cpu_reader: None,
            memory_reader: None,
        };
        for id in [ReaderId::Cpu, ReaderId::Memory] {
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

            // Bound first, so the three futures (and their borrows of `profiles`) are gone
            // before the arms below may reset it.
            let wake = future::or(timer, future::or(command, profile)).await;
            match wake {
                Wake::Timer => {}
                Wake::Command(Command::Quit) | Wake::CommandsClosed => break,
                Wake::Command(command) => self.handle(command).await,
                Wake::Profile(Ok(profile)) => self.app.power_saver = is_power_saver(&profile),
                Wake::Profile(Err(error)) => {
                    eprintln!("statsbard: reading the power profile failed: {error}");
                }
                Wake::ProfileEnded => {
                    eprintln!("statsbard: power profile updates ended; keeping the last value");
                    profiles = None;
                }
            }
        }
        self.shutdown().await;
        Ok(())
    }

    async fn sample(&mut self, id: ReaderId, now: u64) {
        match id {
            ReaderId::Cpu => match self.read_cpu() {
                Ok(ticks) => {
                    let label = self.app.record_cpu(now, ticks);
                    self.refresh_item(Metric::Cpu, label).await;
                }
                Err(error) => self.read_failed(id, now, &error),
            },
            ReaderId::Memory => match self.read_memory() {
                Ok(info) => {
                    let label = self.app.record_memory(now, &info);
                    self.refresh_item(Metric::Ram, label).await;
                }
                Err(error) => self.read_failed(id, now, &error),
            },
        }
    }

    fn read_cpu(&mut self) -> io::Result<Vec<CoreTicks>> {
        let reader = match self.cpu_reader.take() {
            Some(reader) => reader,
            None => CpuReader::open(&self.files.root)?,
        };
        self.cpu_reader.insert(reader).read()
    }

    fn read_memory(&mut self) -> io::Result<MemInfo> {
        let reader = match self.memory_reader.take() {
            Some(reader) => reader,
            None => MemoryReader::open(&self.files.root)?,
        };
        self.memory_reader.insert(reader).read()
    }

    /// Drops the reader, so the next attempt reopens the file, and logs the first failure in a row.
    fn read_failed(&mut self, id: ReaderId, now: u64, error: &io::Error) {
        match id {
            ReaderId::Cpu => self.cpu_reader = None,
            ReaderId::Memory => self.memory_reader = None,
        }
        if self.app.record_error(id, now) {
            eprintln!("statsbard: reading {id:?} failed: {error}");
        }
    }

    async fn refresh_item(&self, metric: Metric, label: Option<u8>) {
        let (handle, rows) = match metric {
            Metric::Cpu => (&self.cpu_item, self.app.cpu().map(summary::cpu_rows)),
            Metric::Ram => (
                &self.memory_item,
                self.app.memory().map(summary::memory_rows),
            ),
        };
        let Some(handle) = handle else { return };
        let rows = rows.unwrap_or_default();
        let redraw = label.map(|value| {
            (
                tray::icon(metric, value),
                summary::title(tray::name(metric), Some(value)),
            )
        });
        // A missed update is not retried. The rows are resent on the next sample, but the icon and
        // title only when the percent changes again, because `App` has already recorded the new
        // value. A 1 s stall on `ksni`'s lock is itself worth investigating, and the log shows it.
        update(
            handle,
            "metric item update",
            move |item: &mut MetricItem| {
                item.rows = rows;
                if let Some((icon, title)) = redraw {
                    item.icon = icon;
                    item.title = title;
                }
            },
        )
        .await;
    }

    async fn refresh_hub(&mut self) {
        let config = &self.app.config;
        let shown = (
            summary::hub_rows(
                self.app.cpu(),
                self.app.memory(),
                config.items.cpu,
                config.items.memory,
            ),
            config.items.cpu,
            config.items.memory,
            self.app.launch_at_login,
        );
        if self.hub_shown.as_ref() == Some(&shown) {
            return;
        }
        let (rows, show_cpu, show_memory, launch_at_login) = shown.clone();
        let delivered = update(&self.hub, "hub update", move |hub: &mut Hub| {
            hub.rows = rows;
            hub.show_cpu = show_cpu;
            hub.show_memory = show_memory;
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
            Command::ToggleCpu => self.toggle(ReaderId::Cpu).await,
            Command::ToggleMemory => self.toggle(ReaderId::Memory).await,
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
        let metric = match id {
            ReaderId::Cpu => Metric::Cpu,
            ReaderId::Memory => Metric::Ram,
        };
        match bounded(
            "metric item spawn",
            SETUP_TIMEOUT,
            MetricItem::new(metric).spawn(),
        )
        .await
        {
            Ok(Ok(handle)) => match id {
                ReaderId::Cpu => self.cpu_item = Some(handle),
                ReaderId::Memory => self.memory_item = Some(handle),
            },
            Ok(Err(error)) => eprintln!("statsbard: showing {id:?} failed: {error}"),
            Err(error) => eprintln!("statsbard: {error}"),
        }
    }

    /// Removes the item and closes its file, so a hidden metric costs nothing.
    async fn hide_item(&mut self, id: ReaderId) {
        let handle = match id {
            ReaderId::Cpu => {
                self.cpu_reader = None;
                self.cpu_item.take()
            }
            ReaderId::Memory => {
                self.memory_reader = None;
                self.memory_item.take()
            }
        };
        if let Some(handle) = handle {
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
        for handle in [self.cpu_item.take(), self.memory_item.take()]
            .into_iter()
            .flatten()
        {
            shut_down(&handle, "metric item shutdown").await;
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
