//! The interface the default route leaves by: `/proc/net/route` text in, an interface name out.

use std::fmt;

/// IFNAMSIZ is 16, including the terminating NUL.
const MAX_INTERFACE_NAME: usize = 15;
/// `RTF_UP` in `<linux/route.h>`.
const RTF_UP: u32 = 0x1;

/// A line of `/proc/net/route` that does not parse, 1-based, counting the header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteError {
    pub line: usize,
}

impl fmt::Display for RouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "proc/net/route line {} is malformed", self.line)
    }
}

impl std::error::Error for RouteError {}

/// Whether `name` can be an interface name and is safe to join onto `/sys/class/net/`. This is
/// the kernel's `dev_valid_name`, narrowed to ASCII.
pub fn is_interface_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_INTERFACE_NAME
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && byte != b'/' && byte != b':')
}

/// The interface of the up IPv4 default route with the lowest metric, and the first such line
/// on a tie. `Ok(None)` when there is none. The first line is the header and is skipped.
///
/// A line that does not parse, or names an impossible interface, is an error rather than
/// skipped: skipping it could hide the real default route behind a worse one. IPv6 default
/// routes live in `/proc/net/ipv6_route`, so a machine with only those reads as having none.
pub fn default_route_interface(route: &str) -> Result<Option<String>, RouteError> {
    let mut best: Option<(&str, u32)> = None;
    for (index, line) in route.lines().enumerate().skip(1) {
        let error = RouteError { line: index + 1 };
        let fields: Vec<&str> = line.split_ascii_whitespace().collect();
        let [interface, destination, _, flags, _, _, metric, mask, ..] = fields.as_slice() else {
            return Err(error);
        };
        let hex = |field: &str| u32::from_str_radix(field, 16).map_err(|_| error);
        let (destination, flags, mask) = (hex(destination)?, hex(flags)?, hex(mask)?);
        let metric: u32 = metric.parse().map_err(|_| error)?;
        if !is_interface_name(interface) {
            return Err(error);
        }
        let is_default = destination == 0 && mask == 0 && flags & RTF_UP != 0;
        if is_default && best.is_none_or(|(_, lowest)| metric < lowest) {
            best = Some((*interface, metric));
        }
    }
    Ok(best.map(|(interface, _)| interface.to_owned()))
}
