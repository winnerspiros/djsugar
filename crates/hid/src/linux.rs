//! HID on Linux through the kernel's hidraw devices: sysfs to find them,
//! reads and writes on `/dev/hidrawN`, and its ioctls for feature and input
//! reports.

use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use crate::{DeviceInfo, BUFFER};

pub struct Hid;

impl Hid {
    pub fn new() -> Result<Self, String> {
        Ok(Self)
    }

    /// The devices from these vendors plugged in now, one entry per
    /// top-level collection.
    pub fn scan(&mut self, vendors: &[u16]) -> Vec<DeviceInfo> {
        let Ok(dir) = fs::read_dir("/sys/class/hidraw") else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for e in dir.flatten() {
            let name = e.file_name();
            let Some(name) = name.to_str() else { continue };
            let sys = e.path().join("device");
            let Ok(uevent) = fs::read_to_string(sys.join("uevent")) else {
                continue;
            };
            let Some(id) = Uevent::parse(&uevent) else {
                continue;
            };
            if !vendors.contains(&id.vendor) {
                continue;
            }
            let usb = fs::canonicalize(&sys).ok();
            let interface = usb
                .as_deref()
                .and_then(|d| read_trimmed(&d.join("../bInterfaceNumber")))
                .and_then(|n| i32::from_str_radix(&n, 16).ok())
                .unwrap_or(-1);
            let product = usb
                .as_deref()
                .and_then(|d| read_trimmed(&d.join("../../product")))
                .unwrap_or(id.name);
            let Ok(path) = CString::new(format!("/dev/{name}")) else {
                continue;
            };
            let descriptor = fs::read(sys.join("report_descriptor")).unwrap_or_default();
            let mut usages = top_level_usages(&descriptor);
            if usages.is_empty() {
                usages.push((0, 0));
            }
            for (usage_page, usage) in usages {
                out.push(DeviceInfo {
                    path: path.clone(),
                    vendor_id: id.vendor,
                    product_id: id.product,
                    usage_page,
                    usage,
                    interface,
                    product: product.clone(),
                    serial: id.serial.clone(),
                });
            }
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }

    pub fn open(&self, d: &DeviceInfo) -> Result<Device, String> {
        let path = d.path.to_str().map_err(|e| e.to_string())?;
        OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(path)
            .map(Device)
            .map_err(|e| format!("{path}: {e}"))
    }
}

fn read_trimmed(p: &Path) -> Option<String> {
    fs::read_to_string(p)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

struct Uevent {
    vendor: u16,
    product: u16,
    name: String,
    serial: String,
}

impl Uevent {
    /// `HID_ID=0003:000017CC:00001720`, `HID_NAME=...`, `HID_UNIQ=...`.
    fn parse(s: &str) -> Option<Self> {
        let mut id = None;
        let (mut name, mut serial) = (String::new(), String::new());
        for line in s.lines() {
            match line.split_once('=') {
                Some(("HID_ID", v)) => {
                    let mut p = v.split(':').skip(1);
                    let vendor = u32::from_str_radix(p.next()?, 16).ok()?;
                    let product = u32::from_str_radix(p.next()?, 16).ok()?;
                    id = Some((vendor as u16, product as u16));
                }
                Some(("HID_NAME", v)) => name = v.to_string(),
                Some(("HID_UNIQ", v)) => serial = v.to_string(),
                _ => {}
            }
        }
        let (vendor, product) = id?;
        Some(Self {
            vendor,
            product,
            name,
            serial,
        })
    }
}

/// The usage page and usage of each top-level collection in a report
/// descriptor.
fn top_level_usages(d: &[u8]) -> Vec<(u16, u16)> {
    let mut out = Vec::new();
    let (mut page, mut usage, mut depth) = (0u16, None::<u32>, 0u32);
    let mut i = 0;
    while i < d.len() {
        let prefix = d[i];
        if prefix == 0xFE {
            // Long item: size in the next byte, then tag and data.
            let size = *d.get(i + 1).unwrap_or(&0) as usize;
            i += 3 + size;
            continue;
        }
        let size = [0, 1, 2, 4][(prefix & 3) as usize];
        let Some(data) = d.get(i + 1..i + 1 + size) else {
            break;
        };
        let value = data
            .iter()
            .rev()
            .fold(0u32, |v, &b| (v << 8) | u32::from(b));
        match prefix & 0xFC {
            0x04 => page = value as u16,
            0x08 if usage.is_none() => {
                usage = Some(if size == 4 {
                    value
                } else {
                    value | u32::from(page) << 16
                })
            }
            0xA0 => {
                if depth == 0 {
                    let u = usage.unwrap_or(0);
                    out.push(((u >> 16) as u16, u as u16));
                }
                depth += 1;
            }
            0xC0 => depth = depth.saturating_sub(1),
            _ => {}
        }
        // Local items last until the next main item.
        if matches!(prefix & 0x0C, 0x00) {
            usage = None;
        }
        i += 1 + size;
    }
    out
}

pub struct Device(File);

const IOC_READ_WRITE: libc::c_ulong = 3;

const fn ioc(nr: libc::c_ulong, len: usize) -> libc::c_ulong {
    (IOC_READ_WRITE << 30) | ((len as libc::c_ulong) << 16) | ((b'H' as libc::c_ulong) << 8) | nr
}

const SET_FEATURE: libc::c_ulong = 0x06;
const GET_FEATURE: libc::c_ulong = 0x07;
const GET_INPUT: libc::c_ulong = 0x0A;

impl Device {
    /// Waits up to `timeout_ms` for an input report; `Ok(0)` when none came.
    pub fn read(&self, buf: &mut [u8], timeout_ms: i32) -> Result<usize, String> {
        let mut fd = libc::pollfd {
            fd: self.0.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one valid pollfd for the duration of the call.
        let n = unsafe { libc::poll(&mut fd, 1, timeout_ms) };
        if n < 0 {
            let e = std::io::Error::last_os_error();
            return if e.kind() == std::io::ErrorKind::Interrupted {
                Ok(0)
            } else {
                Err(e.to_string())
            };
        }
        if n == 0 {
            return Ok(0);
        }
        if fd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
            return Err("device disconnected".into());
        }
        match (&self.0).read(buf) {
            Ok(n) => Ok(n),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(0),
            Err(e) => Err(e.to_string()),
        }
    }

    fn ioctl(&self, nr: libc::c_ulong, buf: &mut [u8]) -> std::io::Result<usize> {
        // SAFETY: the request's encoded length is buf's, so the kernel
        // reads and writes within it.
        let n = unsafe {
            libc::ioctl(
                self.0.as_raw_fd(),
                ioc(nr, buf.len()) as _,
                buf.as_mut_ptr(),
            )
        };
        if n < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(n as usize)
        }
    }

    fn get(&self, nr: libc::c_ulong, id: u8) -> Option<Vec<u8>> {
        let mut buf = [0u8; BUFFER + 1];
        buf[0] = id;
        let n = self.ioctl(nr, &mut buf).ok()?.min(buf.len());
        (n > 1).then(|| buf[1..n].to_vec())
    }
}

impl Device {
    pub fn write(&self, report: &[u8]) -> Result<(), String> {
        (&self.0).write_all(report).map_err(|e| e.to_string())
    }

    pub fn send_feature(&self, report: &[u8]) -> Result<(), String> {
        let mut buf = report.to_vec();
        self.ioctl(SET_FEATURE, &mut buf)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn get_input(&self, id: u8) -> Option<Vec<u8>> {
        self.get(GET_INPUT, id)
    }

    pub fn get_feature(&self, id: u8) -> Option<Vec<u8>> {
        self.get(GET_FEATURE, id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uevent_ids_name_and_serial() {
        let u = Uevent::parse("DRIVER=hid-generic\nHID_ID=0003:000017CC:00001720\nHID_NAME=Native Instruments Traktor Kontrol S4 MK3\nHID_UNIQ=AB12\n").unwrap();
        assert_eq!((u.vendor, u.product), (0x17cc, 0x1720));
        assert_eq!(u.name, "Native Instruments Traktor Kontrol S4 MK3");
        assert_eq!(u.serial, "AB12");
        assert!(Uevent::parse("HID_NAME=x\n").is_none());
    }

    #[test]
    fn top_level_collections() {
        // A mouse: Generic Desktop / Mouse with a nested Pointer collection.
        let mouse = [
            0x05, 0x01, 0x09, 0x02, 0xa1, 0x01, 0x09, 0x01, 0xa1, 0x00, 0x05, 0x09, 0x19, 0x01,
            0x29, 0x05, 0x15, 0x00, 0x25, 0x01, 0x95, 0x05, 0x75, 0x01, 0x81, 0x02, 0x95, 0x01,
            0x75, 0x03, 0x81, 0x01, 0x05, 0x01, 0x09, 0x30, 0x09, 0x31, 0x09, 0x38, 0x15, 0x81,
            0x25, 0x7f, 0x75, 0x08, 0x95, 0x03, 0x81, 0x06, 0xc0, 0xc0,
        ];
        assert_eq!(top_level_usages(&mouse), vec![(0x01, 0x02)]);
        // Two collections, a vendor page with a 2-byte page and an extended
        // usage.
        let two = [
            0x06, 0x00, 0xff, 0x09, 0x01, 0xa1, 0x01, 0xc0, 0x0b, 0x80, 0x00, 0x0c, 0x00, 0xa1,
            0x01, 0xc0,
        ];
        assert_eq!(top_level_usages(&two), vec![(0xff00, 0x01), (0x0c, 0x80)]);
    }

    /// This machine's hidraw devices, whatever they are, are found with
    /// their USB details.
    #[test]
    fn scans_this_machines_devices() {
        let Ok(dir) = fs::read_dir("/sys/class/hidraw") else {
            return;
        };
        let vendors: Vec<u16> = dir
            .flatten()
            .filter_map(|e| fs::read_to_string(e.path().join("device/uevent")).ok())
            .filter_map(|u| Uevent::parse(&u))
            .map(|u| u.vendor)
            .collect();
        let found = Hid.scan(&vendors);
        assert!(found.len() >= vendors.len());
        for d in &found {
            assert!(d.path.to_str().unwrap().starts_with("/dev/hidraw"));
            assert!(vendors.contains(&d.vendor_id));
        }
        assert!(Hid.scan(&[0xdead]).iter().all(|d| d.vendor_id == 0xdead));
    }
}
