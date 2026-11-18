//! Spike S2: how many threads, and how much memory, do N tray items plus one zbus connection
//! driven from the main thread cost?
//!
//! Usage: `threads <items>`. Prints its pid; measure it with `measure.sh <pid>`.

use std::time::Duration;

use futures_lite::{StreamExt, future};
use ksni::TrayMethods;
use statsbar_tray_spike::{SETUP_TIMEOUT, bounded};

/// The upper bound on how long a D-Bus call may hold up the loop, per the global constraint
/// that every external call has a timeout.
const CALL_TIMEOUT: Duration = Duration::from_secs(1);
const TICK: Duration = Duration::from_secs(2);

struct Item {
    index: usize,
    value: u32,
}

impl ksni::Tray for Item {
    fn id(&self) -> String {
        format!("statsbar-spike-threads-{}", self.index)
    }
    fn title(&self) -> String {
        format!("Item {}: {}%", self.index, self.value)
    }
    fn icon_name(&self) -> String {
        "utilities-system-monitor".into()
    }
}

type BoxError = Box<dyn std::error::Error>;

fn main() -> Result<(), BoxError> {
    let items: usize = match std::env::args().nth(1) {
        Some(arg) => arg.parse()?,
        None => 3,
    };
    async_io::block_on(async {
        let mut handles = Vec::new();
        for index in 0..items {
            handles.push(
                bounded(
                    "tray item spawn",
                    SETUP_TIMEOUT,
                    Item { index, value: 0 }.spawn(),
                )
                .await??,
            );
        }
        // internal_executor(false): zbus does not start its own thread; the loop below ticks the
        // connection's executor on this thread instead.
        let builder = zbus::connection::Builder::session()?.internal_executor(false);
        let conn = bounded("session bus connect", SETUP_TIMEOUT, builder.build()).await??;
        let executor = conn.executor().clone();
        println!(
            "pid {}: {items} items and one zbus connection",
            std::process::id()
        );
        let work = async {
            let dbus = bounded(
                "DBus proxy",
                SETUP_TIMEOUT,
                zbus::fdo::DBusProxy::new(&conn),
            )
            .await??;
            let mut ticks = async_io::Timer::interval(TICK);
            let mut count: u32 = 0;
            loop {
                ticks.next().await;
                count = count.wrapping_add(1);
                let reply = future::or(async { Some(dbus.get_id().await) }, async {
                    async_io::Timer::after(CALL_TIMEOUT).await;
                    None
                })
                .await;
                match reply {
                    Some(Ok(id)) => println!("tick {count}: bus id {id}"),
                    Some(Err(error)) => println!("tick {count}: GetId failed: {error}"),
                    None => println!("tick {count}: GetId timed out"),
                }
                for handle in &handles {
                    bounded(
                        "tray item update",
                        CALL_TIMEOUT,
                        handle.update(|item: &mut Item| item.value = count % 101),
                    )
                    .await?
                    .ok_or("tray service shut down during update")?;
                }
            }
        };
        let drive = async {
            loop {
                executor.tick().await;
            }
        };
        future::or(work, drive).await
    })
}
