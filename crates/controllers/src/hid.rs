//! USB HID controllers, such as Native Instruments' Traktor Kontrol range.
//!
//! An HID controller sends its whole state as an input report (a block of
//! bytes) whenever something on it moves, and takes its lights as output
//! reports. There are no MIDI messages to bind, so an HID mapping is all
//! script: `.hid.xml` files name the device (`<info><devices>`) and
//! the scripts, which get every input report in `incomingData` and send
//! reports with `controller.sendOutputReport`. This module finds the
//! devices, matches them to mappings, and keeps the output reports.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::ffi::CString;

/// A device an HID mapping is for: `<product protocol="hid" vendor_id=...>`.
/// Missing fields match any device.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Product {
    pub vendor_id: u16,
    pub product_id: u16,
    pub usage_page: Option<u16>,
    pub usage: Option<u16>,
    pub interface: Option<i32>,
}

/// Parses `0x17cc` or `17cc`: Mixxx reads these as hexadecimal either way.
fn hex(s: &str) -> Option<i64> {
    let s = s.trim();
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    i64::from_str_radix(s, 16).ok()
}

impl Product {
    /// Reads a `<product>` element's attributes; `None` unless it is an HID
    /// product with a vendor and product id.
    pub fn from_attributes<'a>(attr: impl Fn(&str) -> Option<&'a str>) -> Option<Self> {
        if attr("protocol").is_some_and(|p| !p.eq_ignore_ascii_case("hid")) {
            return None;
        }
        let num = |name: &str| attr(name).and_then(hex);
        Some(Product {
            vendor_id: u16::try_from(num("vendor_id")?).ok()?,
            product_id: u16::try_from(num("product_id")?).ok()?,
            usage_page: num("usage_page").and_then(|v| u16::try_from(v).ok()),
            usage: num("usage").and_then(|v| u16::try_from(v).ok()),
            interface: num("interface_number").and_then(|v| i32::try_from(v).ok()),
        })
    }

    /// `matchProductInfo`: vendor and product must match; then the
    /// USB interface where the system says which it is, otherwise the
    /// usage page and usage. A field the mapping leaves out matches anything.
    pub fn matches(&self, d: &DeviceInfo) -> bool {
        if d.vendor_id != self.vendor_id || d.product_id != self.product_id {
            return false;
        }
        if d.interface >= 0 {
            self.interface.is_none_or(|i| i == d.interface)
        } else {
            self.usage_page.is_none_or(|p| p == d.usage_page)
                && self.usage.is_none_or(|u| u == d.usage)
        }
    }
}

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
    /// The device's own name for itself ("Traktor Kontrol S2 MK3"), if any.
    pub product: String,
    pub serial: String,
}

/// Talks to an open HID device. Reports written start with their report
/// ID (0 for devices that don't number their reports); reports read back
/// don't.
pub trait Io {
    fn write(&self, report: &[u8]) -> Result<(), String>;
    fn send_feature(&self, report: &[u8]) -> Result<(), String>;
    /// `controller.getInputReport`: asks the device for an input report.
    /// Optional in the HID standard; not every device answers.
    fn get_input(&self, id: u8) -> Option<Vec<u8>>;
    fn get_feature(&self, id: u8) -> Option<Vec<u8>>;
}

/// Output reports waiting to be sent: a report
/// sent in the default mode replaces any of the same ID not yet sent, and
/// isn't sent at all if it's what the device already has. One sent with
/// `useNonSkippingFIFO` is always sent, in order, before the others.
#[derive(Default)]
pub(crate) struct Outputs {
    fifo: VecDeque<Vec<u8>>,
    cached: BTreeMap<u8, Vec<u8>>,
    last: HashMap<u8, Vec<u8>>,
}

impl Outputs {
    pub(crate) fn push(&mut self, id: u8, data: &[u8], fifo: bool) {
        let mut report = Vec::with_capacity(data.len() + 1);
        report.push(id);
        report.extend_from_slice(data);
        if fifo {
            // What the device has is no longer known.
            self.cached.remove(&id);
            self.last.remove(&id);
            // Mixxx's FIFO holds 32 reports; a script flooding it loses the oldest.
            if self.fifo.len() >= 32 {
                self.fifo.pop_front();
            }
            self.fifo.push_back(report);
        } else {
            self.cached.insert(id, report);
        }
    }

    /// The reports to write now, oldest first.
    pub(crate) fn take(&mut self) -> Vec<Vec<u8>> {
        let mut out: Vec<Vec<u8>> = self.fifo.drain(..).collect();
        for (id, report) in std::mem::take(&mut self.cached) {
            if self.last.get(&id) != Some(&report) {
                self.last.insert(id, report.clone());
                out.push(report);
            }
        }
        out
    }

    /// A write failed: send that report again next time even if unchanged.
    pub(crate) fn forget(&mut self, id: u8) {
        self.last.remove(&id);
    }
}

/// Whether an input report is the same as the last one: some devices send
/// their state over and over, and Mixxx skips the repeats rather than run
/// the script for each.
#[derive(Default)]
pub(crate) struct Repeats {
    last: Vec<u8>,
}

impl Repeats {
    pub(crate) fn is_repeat(&mut self, report: &[u8]) -> bool {
        if self.last == report {
            return true;
        }
        self.last.clear();
        self.last.extend_from_slice(report);
        false
    }
}

/// An HID mapping: its name, file and the devices it is for.
pub type HidMapping = (String, std::path::PathBuf, Vec<Product>);

/// The HID mappings in `dir` (`*.hid.xml`).
pub fn list_mappings(dir: &std::path::Path) -> Vec<HidMapping> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut v: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".hid.xml"))
        .filter_map(|p| {
            let m = crate::Mapping::load(&p).ok()?;
            (!m.products.is_empty()).then_some((m.name, p, m.products))
        })
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

/// The mapping for a device: the first whose products match it.
pub fn match_mapping<'a>(
    device: &DeviceInfo,
    mappings: &'a [HidMapping],
) -> Option<&'a HidMapping> {
    mappings
        .iter()
        .find(|(_, _, products)| products.iter().any(|p| p.matches(device)))
}

/// Finds the HID devices plugged in, and opens them.
#[cfg(not(target_os = "android"))]
pub(crate) struct Hid {
    api: hidapi::HidApi,
}

#[cfg(not(target_os = "android"))]
impl Hid {
    pub(crate) fn new() -> Result<Self, String> {
        hidapi::HidApi::new()
            .map(|api| Self { api })
            .map_err(|e| e.to_string())
    }

    /// The devices from these vendors plugged in now. Only they are
    /// listed, so keyboards, mice and the like are never looked at.
    pub(crate) fn scan(&mut self, vendors: &[u16]) -> Vec<DeviceInfo> {
        let _ = self.api.reset_devices();
        for &v in vendors {
            let _ = self.api.add_devices(v, 0);
        }
        self.api
            .device_list()
            .map(|d| DeviceInfo {
                path: d.path().to_owned(),
                vendor_id: d.vendor_id(),
                product_id: d.product_id(),
                usage_page: d.usage_page(),
                usage: d.usage(),
                interface: d.interface_number(),
                product: d.product_string().unwrap_or("").trim().to_string(),
                serial: d.serial_number().unwrap_or("").to_string(),
            })
            .collect()
    }

    pub(crate) fn open(&self, d: &DeviceInfo) -> Result<Device, String> {
        let dev = self.api.open_path(&d.path).map_err(|e| e.to_string())?;
        // Reads are polled with a timeout, not left to block.
        let _ = dev.set_blocking_mode(false);
        Ok(Device(dev))
    }
}

/// The largest report read or asked for.
pub(crate) const BUFFER: usize = 255;

/// An open HID device.
#[cfg(not(target_os = "android"))]
pub(crate) struct Device(hidapi::HidDevice);

#[cfg(not(target_os = "android"))]
impl Device {
    /// Waits up to `timeout_ms` for an input report; `Ok(0)` when none came.
    pub(crate) fn read(&self, buf: &mut [u8], timeout_ms: i32) -> Result<usize, String> {
        self.0
            .read_timeout(buf, timeout_ms)
            .map_err(|e| e.to_string())
    }
}

#[cfg(not(target_os = "android"))]
impl Io for Device {
    fn write(&self, report: &[u8]) -> Result<(), String> {
        self.0.write(report).map(|_| ()).map_err(|e| e.to_string())
    }

    fn send_feature(&self, report: &[u8]) -> Result<(), String> {
        self.0
            .send_feature_report(report)
            .map_err(|e| e.to_string())
    }

    fn get_input(&self, id: u8) -> Option<Vec<u8>> {
        let mut buf = vec![0u8; BUFFER + 1];
        buf[0] = id;
        let n = self.0.get_input_report(&mut buf).ok()?;
        // At least the ID and one byte of data.
        (n > 1).then(|| buf[1..n].to_vec())
    }

    fn get_feature(&self, id: u8) -> Option<Vec<u8>> {
        let mut buf = vec![0u8; BUFFER + 1];
        buf[0] = id;
        let n = self.0.get_feature_report(&mut buf).ok()?;
        (n > 1).then(|| buf[1..n].to_vec())
    }
}

/// Android has no hidraw access for apps; USB HID goes through the
/// platform's USB host API (roadmap C.1), so none are found for now.
#[cfg(target_os = "android")]
pub(crate) struct Hid;

#[cfg(target_os = "android")]
impl Hid {
    pub(crate) fn new() -> Result<Self, String> {
        Ok(Self)
    }

    pub(crate) fn scan(&mut self, _vendors: &[u16]) -> Vec<DeviceInfo> {
        Vec::new()
    }

    pub(crate) fn open(&self, _d: &DeviceInfo) -> Result<Device, String> {
        Err("USB HID controllers are not supported on Android yet".into())
    }
}

/// Never made on Android: [`Hid::scan`] finds nothing.
#[cfg(target_os = "android")]
pub(crate) enum Device {}

#[cfg(target_os = "android")]
impl Device {
    pub(crate) fn read(&self, _buf: &mut [u8], _timeout_ms: i32) -> Result<usize, String> {
        match *self {}
    }
}

#[cfg(target_os = "android")]
impl Io for Device {
    fn write(&self, _report: &[u8]) -> Result<(), String> {
        match *self {}
    }

    fn send_feature(&self, _report: &[u8]) -> Result<(), String> {
        match *self {}
    }

    fn get_input(&self, _id: u8) -> Option<Vec<u8>> {
        match *self {}
    }

    fn get_feature(&self, _id: u8) -> Option<Vec<u8>> {
        match *self {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(interface: i32, usage_page: u16, usage: u16) -> DeviceInfo {
        DeviceInfo {
            vendor_id: 0x17cc,
            product_id: 0x1710,
            usage_page,
            usage,
            interface,
            ..Default::default()
        }
    }

    #[test]
    fn matches_devices_as_mixxx_does() {
        let attrs = [
            ("protocol", "hid"),
            ("vendor_id", "0x17cc"),
            ("product_id", "0x1710"),
            ("usage_page", "0xff01"),
            ("usage", "0x1"),
            ("interface_number", "0x3"),
        ];
        let get = |n: &str| attrs.iter().find(|(k, _)| *k == n).map(|(_, v)| *v);
        let p = Product::from_attributes(get).unwrap();
        assert_eq!(p.interface, Some(3));
        // The interface decides where it's known…
        assert!(p.matches(&device(3, 0, 0)));
        assert!(!p.matches(&device(0, 0xff01, 1)));
        // …the usage where it isn't.
        assert!(p.matches(&device(-1, 0xff01, 1)));
        assert!(!p.matches(&device(-1, 0x01, 6)));
        let mut other = device(3, 0, 0);
        other.product_id = 0x1720;
        assert!(!p.matches(&other));

        // Only vendor and product given (the Z1): any interface.
        let loose = Product {
            vendor_id: 0x17cc,
            product_id: 0x1710,
            ..Default::default()
        };
        assert!(loose.matches(&device(2, 0, 0)));
        assert!(loose.matches(&device(-1, 0xff00, 0)));

        let bulk = |n: &str| match n {
            "protocol" => Some("bulk"),
            "vendor_id" => Some("0x17cc"),
            "product_id" => Some("0x1720"),
            _ => None,
        };
        assert_eq!(Product::from_attributes(bulk), None);
    }

    #[test]
    fn output_reports_skip_what_the_device_has() {
        let mut o = Outputs::default();
        o.push(0x80, &[1, 2], false);
        o.push(0x80, &[1, 3], false);
        o.push(0x81, &[9], false);
        // The newer 0x80 replaced the older one.
        assert_eq!(o.take(), vec![vec![0x80, 1, 3], vec![0x81, 9]]);
        o.push(0x80, &[1, 3], false);
        assert!(o.take().is_empty(), "unchanged reports aren't sent again");
        // FIFO reports always go, first, and the next cached one is sent
        // even if it matches what was sent before them.
        o.push(0x80, &[1, 3], false);
        o.push(0x31, &[5], true);
        o.push(0x31, &[5], true);
        assert_eq!(o.take(), vec![vec![0x31, 5], vec![0x31, 5]]);
        o.push(0x31, &[5], false);
        assert_eq!(o.take(), vec![vec![0x31, 5]]);
        o.forget(0x31);
        o.push(0x31, &[5], false);
        assert_eq!(o.take(), vec![vec![0x31, 5]]);
    }

    #[test]
    fn repeated_input_reports_are_skipped() {
        let mut r = Repeats::default();
        assert!(!r.is_repeat(&[1, 2, 3]));
        assert!(r.is_repeat(&[1, 2, 3]));
        assert!(!r.is_repeat(&[1, 2, 4]));
        assert!(!r.is_repeat(&[1, 2, 3]));
    }

    #[test]
    fn lists_the_bundled_hid_mappings() {
        let dir = crate::default_mappings_dir().unwrap();
        let m = list_mappings(&dir);
        let names: Vec<&str> = m.iter().map(|(n, _, _)| n.as_str()).collect();
        assert!(names.contains(&"Traktor Kontrol S2 MK3"), "{names:?}");
        assert!(names.contains(&"Traktor Kontrol S4 MK3"), "{names:?}");
        let s2 = device(3, 0xff01, 1);
        let (name, _, _) = match_mapping(&s2, &m).unwrap();
        assert_eq!(name, "Traktor Kontrol S2 MK3");
    }
}
