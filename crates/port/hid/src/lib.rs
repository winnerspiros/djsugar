//! USB HID devices: finding them, reading their input reports, and sending output and feature reports, straight through
//! each system's own HID layer (hidraw on Linux, the HID class driver on Windows, IOKit on macOS). Reports written
//! start with their report ID (0 for devices that don't number their reports).

use std::ffi::CString;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{Device, Hid};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{Device, Hid};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{Device, Hid};

#[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
mod none;
#[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
pub use none::{Device, Hid};

/// The largest report read or asked for.
pub const BUFFER: usize = 255;

/// An HID device plugged in, as the system describes it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeviceInfo {
    /// What the system opens it by.
    pub path: CString,
    pub vendor_id: u16,
    pub product_id: u16,
    pub usage_page: u16,
    pub usage: u16,
    /// The USB interface, or -1 where the system doesn't say.
    pub interface: i32,
    /// The device's own name for itself ("Kontrol S2 MK3"), if any.
    pub product: String,
    pub serial: String,
}

/// The USB interface number in a device path, where it carries one
/// (`...&mi_02#...` on Windows).
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
fn interface_in_path(path: &str) -> i32 {
    let lower = path.to_ascii_lowercase();
    lower.find("&mi_").and_then(|i| lower.get(i + 4..i + 6)).and_then(|n| i32::from_str_radix(n, 16).ok()).unwrap_or(-1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interface_numbers_in_paths() {
        assert_eq!(
            interface_in_path(r"\\?\hid#vid_17cc&pid_1720&mi_02#8&2a1b&0&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}"),
            2
        );
        assert_eq!(interface_in_path(r"\\?\hid#vid_06f8&pid_b105#7&1a"), -1);
    }
}
