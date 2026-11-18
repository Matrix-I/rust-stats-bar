//! The active power profile, from power-profiles-daemon on the system bus. Only `power-saver`
//! changes the daemon's behaviour: it stretches every interval to 5 s.

use zbus::proxy::{Builder, CacheProperties, PropertyStream};

const DESTINATION: &str = "org.freedesktop.UPower.PowerProfiles";
const PATH: &str = "/org/freedesktop/UPower/PowerProfiles";
const INTERFACE: &str = "org.freedesktop.UPower.PowerProfiles";
const PROPERTY: &str = "ActiveProfile";

pub fn is_power_saver(profile: &str) -> bool {
    profile == "power-saver"
}

/// A subscription to `ActiveProfile`. The stream yields the current value first, then each
/// change. It holds its own clone of the proxy, which keeps the property cache alive.
pub struct ProfileWatch {
    pub changes: PropertyStream<'static, String>,
}

impl ProfileWatch {
    /// Needs the connection's executor to be ticking, because the proxy subscribes to
    /// `PropertiesChanged` while it is built.
    ///
    /// `CacheProperties::Yes` fetches the properties before returning. With the default
    /// (`Lazily`), a missing power-profiles-daemon fails silently in the background and the stream
    /// simply never yields; fetching up front turns that into an error the caller logs.
    pub async fn start(connection: &zbus::Connection) -> zbus::Result<ProfileWatch> {
        let proxy: zbus::Proxy<'static> = Builder::new(connection)
            .destination(DESTINATION)?
            .path(PATH)?
            .interface(INTERFACE)?
            .cache_properties(CacheProperties::Yes)
            .build()
            .await?;
        let changes = proxy.receive_property_changed::<String>(PROPERTY).await;
        Ok(ProfileWatch { changes })
    }
}
