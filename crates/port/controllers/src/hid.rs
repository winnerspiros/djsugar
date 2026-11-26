//! USB HID controllers, such as Native Instruments' Kontrol range. An HID controller sends its whole state as
//! an input report (a block of bytes) whenever something on it moves, and takes its lights as output reports.

use std::collections::{BTreeMap, HashMap, VecDeque};

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

/// Parses `0x17cc` or `17cc`: the stock app reads these as hexadecimal either way.
fn hex(s: &str) -> Option<i64> {
    let s = s.trim();
    let s = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(s);
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
            self.usage_page.is_none_or(|p| p == d.usage_page) && self.usage.is_none_or(|u| u == d.usage)
        }
    }
}

pub use djsugar_hid::DeviceInfo;

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

/// Output reports waiting to be sent: a report sent in the default mode replaces any of the same ID not yet sent, and
/// isn't sent at all if it's what the device already has. One sent with `useNonSkippingFIFO` is always sent, in order,
/// before the others.
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
            // the stock mappings' FIFO holds 32 reports; a script flooding it loses the oldest.
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
/// their state over and over, and the stock app skips the repeats rather than run
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
pub fn match_mapping<'a>(device: &DeviceInfo, mappings: &'a [HidMapping]) -> Option<&'a HidMapping> {
    mappings.iter().find(|(_, _, products)| products.iter().any(|p| p.matches(device)))
}

/// Finds the HID devices plugged in, and opens them.
pub(crate) use djsugar_hid::{Device, Hid, BUFFER};

impl Io for Device {
    fn write(&self, report: &[u8]) -> Result<(), String> {
        Device::write(self, report)
    }

    fn send_feature(&self, report: &[u8]) -> Result<(), String> {
        Device::send_feature(self, report)
    }

    fn get_input(&self, id: u8) -> Option<Vec<u8>> {
        Device::get_input(self, id)
    }

    fn get_feature(&self, id: u8) -> Option<Vec<u8>> {
        Device::get_feature(self, id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(interface: i32, usage_page: u16, usage: u16) -> DeviceInfo {
        DeviceInfo { vendor_id: 0x17cc, product_id: 0x1710, usage_page, usage, interface, ..Default::default() }
    }

    #[test]
    fn matches_devices_by_their_ids() {
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
        let loose = Product { vendor_id: 0x17cc, product_id: 0x1710, ..Default::default() };
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
        assert!(names.iter().any(|n| n.ends_with("Kontrol S2 MK3")), "{names:?}");
        assert!(names.iter().any(|n| n.ends_with("Kontrol S4 MK3")), "{names:?}");
        let s2 = device(3, 0xff01, 1);
        let (name, _, _) = match_mapping(&s2, &m).unwrap();
        assert!(name.ends_with("Kontrol S2 MK3"), "{name}");
    }
}
