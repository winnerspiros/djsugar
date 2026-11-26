//! HID on macOS through IOKit: an HID manager to list devices, and each
//! device's input reports delivered on a run-loop mode of its own, run by
//! whichever thread reads.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ffi::{c_char, c_void, CString};
use std::ptr::null_mut;

use core_foundation_sys::array::{kCFTypeArrayCallBacks, CFArrayCreate, CFArrayRef};
use core_foundation_sys::base::{kCFAllocatorDefault, CFGetTypeID, CFIndex, CFRelease, CFTypeRef};
use core_foundation_sys::dictionary::{
    kCFTypeDictionaryKeyCallBacks, kCFTypeDictionaryValueCallBacks, CFDictionaryCreate, CFDictionaryRef,
};
use core_foundation_sys::number::{
    kCFNumberSInt32Type, kCFNumberSInt64Type, CFNumberCreate, CFNumberGetTypeID, CFNumberGetValue, CFNumberRef,
};
use core_foundation_sys::runloop::{CFRunLoopGetCurrent, CFRunLoopRef, CFRunLoopRunInMode, CFRunLoopStop};
use core_foundation_sys::set::{CFSetGetCount, CFSetGetValues, CFSetRef};
use core_foundation_sys::string::{
    kCFStringEncodingUTF8, CFStringCreateWithCString, CFStringGetCString, CFStringGetTypeID, CFStringRef,
};

use crate::{DeviceInfo, BUFFER};

type IOHIDManagerRef = *mut c_void;
type IOHIDDeviceRef = *mut c_void;
type IOReturn = i32;
type IOHIDReportType = u32;
type IOHIDReportCallback = extern "C" fn(
    context: *mut c_void,
    result: IOReturn,
    sender: *mut c_void,
    kind: IOHIDReportType,
    id: u32,
    report: *mut u8,
    len: CFIndex,
);

const REPORT_INPUT: IOHIDReportType = 0;
const REPORT_OUTPUT: IOHIDReportType = 1;
const REPORT_FEATURE: IOHIDReportType = 2;
const SUCCESS: IOReturn = 0;

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOHIDManagerCreate(allocator: *const c_void, options: u32) -> IOHIDManagerRef;
    fn IOHIDManagerSetDeviceMatchingMultiple(manager: IOHIDManagerRef, multiple: CFArrayRef);
    fn IOHIDManagerCopyDevices(manager: IOHIDManagerRef) -> CFSetRef;
    fn IOHIDDeviceGetProperty(device: IOHIDDeviceRef, key: CFStringRef) -> CFTypeRef;
    fn IOHIDDeviceGetService(device: IOHIDDeviceRef) -> u32;
    fn IOHIDDeviceCreate(allocator: *const c_void, service: u32) -> IOHIDDeviceRef;
    fn IOHIDDeviceOpen(device: IOHIDDeviceRef, options: u32) -> IOReturn;
    fn IOHIDDeviceClose(device: IOHIDDeviceRef, options: u32) -> IOReturn;
    fn IOHIDDeviceScheduleWithRunLoop(device: IOHIDDeviceRef, run_loop: CFRunLoopRef, mode: CFStringRef);
    fn IOHIDDeviceUnscheduleFromRunLoop(device: IOHIDDeviceRef, run_loop: CFRunLoopRef, mode: CFStringRef);
    fn IOHIDDeviceRegisterInputReportCallback(
        device: IOHIDDeviceRef,
        report: *mut u8,
        len: CFIndex,
        callback: Option<IOHIDReportCallback>,
        context: *mut c_void,
    );
    fn IOHIDDeviceSetReport(
        device: IOHIDDeviceRef,
        kind: IOHIDReportType,
        id: CFIndex,
        report: *const u8,
        len: CFIndex,
    ) -> IOReturn;
    fn IOHIDDeviceGetReport(
        device: IOHIDDeviceRef,
        kind: IOHIDReportType,
        id: CFIndex,
        report: *mut u8,
        len: *mut CFIndex,
    ) -> IOReturn;
    fn IORegistryEntryGetRegistryEntryID(entry: u32, id: *mut u64) -> i32;
    fn IORegistryEntryIDMatching(id: u64) -> CFDictionaryRef;
    fn IOServiceGetMatchingService(main_port: u32, matching: CFDictionaryRef) -> u32;
    fn IOObjectRelease(object: u32) -> i32;
}

/// A CoreFoundation string, released when dropped.
struct CfString(CFStringRef);

impl CfString {
    fn new(s: &str) -> Self {
        let c = CString::new(s).unwrap_or_default();
        // SAFETY: a NUL-terminated UTF-8 string.
        Self(unsafe { CFStringCreateWithCString(kCFAllocatorDefault, c.as_ptr(), kCFStringEncodingUTF8) })
    }
}

impl Drop for CfString {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: created above, released once.
            unsafe { CFRelease(self.0.cast()) };
        }
    }
}

fn number(device: IOHIDDeviceRef, key: &str) -> Option<i64> {
    let key = CfString::new(key);
    // SAFETY: the value is borrowed from the device (a Get rule).
    let v = unsafe { IOHIDDeviceGetProperty(device, key.0) };
    if v.is_null() || unsafe { CFGetTypeID(v) != CFNumberGetTypeID() } {
        return None;
    }
    let mut out = 0i64;
    // SAFETY: a CFNumber read into an i64.
    unsafe { CFNumberGetValue(v as CFNumberRef, kCFNumberSInt64Type, (&mut out as *mut i64).cast()) }.then_some(out)
}

fn string(device: IOHIDDeviceRef, key: &str) -> String {
    let key = CfString::new(key);
    // SAFETY: the value is borrowed from the device (a Get rule).
    let v = unsafe { IOHIDDeviceGetProperty(device, key.0) };
    if v.is_null() || unsafe { CFGetTypeID(v) != CFStringGetTypeID() } {
        return String::new();
    }
    let mut buf = [0 as c_char; 256];
    // SAFETY: the length given is the buffer's.
    if 0 == unsafe {
        CFStringGetCString(v as CFStringRef, buf.as_mut_ptr(), buf.len() as CFIndex, kCFStringEncodingUTF8)
    } {
        return String::new();
    }
    // SAFETY: NUL-terminated by the call above.
    unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) }.to_string_lossy().into_owned()
}

pub struct Hid {
    manager: IOHIDManagerRef,
}

// SAFETY: the manager is only used through `&mut self`.
unsafe impl Send for Hid {}

impl Hid {
    pub fn new() -> Result<Self, String> {
        // SAFETY: a new manager, released in Drop.
        let manager = unsafe { IOHIDManagerCreate(kCFAllocatorDefault.cast(), 0) };
        if manager.is_null() {
            return Err("IOHIDManagerCreate failed".into());
        }
        Ok(Self { manager })
    }

    /// The devices from these vendors plugged in now. Only they are
    /// listed, so keyboards, mice and the like are never looked at.
    pub fn scan(&mut self, vendors: &[u16]) -> Vec<DeviceInfo> {
        let key = CfString::new("VendorID");
        let dicts: Vec<CFDictionaryRef> = vendors
            .iter()
            .map(|&v| {
                let v = i32::from(v);
                // SAFETY: one key and one value, retained by the dictionary.
                unsafe {
                    let num = CFNumberCreate(kCFAllocatorDefault, kCFNumberSInt32Type, (&v as *const i32).cast());
                    let keys = [key.0.cast::<c_void>()];
                    let values = [num.cast::<c_void>()];
                    let d = CFDictionaryCreate(
                        kCFAllocatorDefault,
                        keys.as_ptr(),
                        values.as_ptr(),
                        1,
                        &kCFTypeDictionaryKeyCallBacks,
                        &kCFTypeDictionaryValueCallBacks,
                    );
                    CFRelease(num.cast());
                    d
                }
            })
            .collect();
        // SAFETY: the array retains the dictionaries; all released below.
        let set = unsafe {
            let array = CFArrayCreate(
                kCFAllocatorDefault,
                dicts.as_ptr().cast(),
                dicts.len() as CFIndex,
                &kCFTypeArrayCallBacks,
            );
            IOHIDManagerSetDeviceMatchingMultiple(self.manager, array);
            CFRelease(array.cast());
            for d in &dicts {
                CFRelease(d.cast());
            }
            IOHIDManagerCopyDevices(self.manager)
        };
        if set.is_null() {
            return Vec::new();
        }
        // SAFETY: the set holds this many device refs, borrowed while it
        // lives.
        let devices = unsafe {
            let n = CFSetGetCount(set) as usize;
            let mut v = vec![std::ptr::null::<c_void>(); n];
            CFSetGetValues(set, v.as_mut_ptr());
            v
        };
        let mut out: Vec<DeviceInfo> = devices
            .into_iter()
            .filter_map(|d| describe(d as IOHIDDeviceRef))
            .filter(|d| vendors.contains(&d.vendor_id))
            .collect();
        // SAFETY: from a Copy call.
        unsafe { CFRelease(set.cast()) };
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }

    pub fn open(&self, d: &DeviceInfo) -> Result<Device, String> {
        let id = d
            .path
            .to_str()
            .ok()
            .and_then(|p| p.strip_prefix("DevSrvsID:"))
            .and_then(|n| n.parse::<u64>().ok())
            .ok_or("not an IOKit device path")?;
        // SAFETY: the matching dictionary is consumed by the lookup; the
        // service is released once the device holds it.
        let device = unsafe {
            let service = IOServiceGetMatchingService(0, IORegistryEntryIDMatching(id));
            if service == 0 {
                return Err("device not found".into());
            }
            let device = IOHIDDeviceCreate(kCFAllocatorDefault.cast(), service);
            IOObjectRelease(service);
            device
        };
        if device.is_null() {
            return Err("IOHIDDeviceCreate failed".into());
        }
        // SAFETY: a device just made, closed and released in Drop.
        let r = unsafe { IOHIDDeviceOpen(device, 0) };
        if r != SUCCESS {
            // SAFETY: made above, used no further.
            unsafe { CFRelease(device.cast()) };
            return Err(format!("cannot open the device (IOReturn {r:#x})"));
        }
        let input_len = number(device, "MaxInputReportSize").unwrap_or(BUFFER as i64);
        let device = Device {
            device,
            mode: CfString::new("org.djsugar.hid"),
            inbox: Box::new(Inbox {
                reports: RefCell::new(VecDeque::new()),
                buf: RefCell::new(vec![0; (input_len.clamp(1, BUFFER as i64)) as usize]),
                run_loop: Cell::new(null_mut()),
            }),
            scheduled: Cell::new(null_mut()),
        };
        // SAFETY: the buffer and context are boxed and live until Drop
        // unregisters the callback.
        unsafe {
            let mut buf = device.inbox.buf.borrow_mut();
            IOHIDDeviceRegisterInputReportCallback(
                device.device,
                buf.as_mut_ptr(),
                buf.len() as CFIndex,
                Some(on_report),
                (&*device.inbox as *const Inbox).cast_mut().cast(),
            );
        }
        Ok(device)
    }
}

impl Drop for Hid {
    fn drop(&mut self) {
        // SAFETY: created in new, released once.
        unsafe { CFRelease(self.manager.cast_const()) };
    }
}

fn describe(device: IOHIDDeviceRef) -> Option<DeviceInfo> {
    let mut entry = 0u64;
    // SAFETY: the service is borrowed from the device.
    if unsafe { IORegistryEntryGetRegistryEntryID(IOHIDDeviceGetService(device), &mut entry) } != 0 {
        return None;
    }
    Some(DeviceInfo {
        path: CString::new(format!("DevSrvsID:{entry}")).ok()?,
        vendor_id: number(device, "VendorID")? as u16,
        product_id: number(device, "ProductID").unwrap_or(0) as u16,
        usage_page: number(device, "PrimaryUsagePage").unwrap_or(0) as u16,
        usage: number(device, "PrimaryUsage").unwrap_or(0) as u16,
        interface: -1,
        product: string(device, "Product").trim().to_string(),
        serial: string(device, "SerialNumber"),
    })
}

/// Where the device's input callback leaves reports for `read`.
struct Inbox {
    reports: RefCell<VecDeque<Vec<u8>>>,
    buf: RefCell<Vec<u8>>,
    run_loop: Cell<CFRunLoopRef>,
}

/// Most reports held for a reader that has fallen behind.
const QUEUE: usize = 64;

extern "C" fn on_report(
    context: *mut c_void,
    _result: IOReturn,
    _sender: *mut c_void,
    _kind: IOHIDReportType,
    _id: u32,
    report: *mut u8,
    len: CFIndex,
) {
    // SAFETY: the context is the device's boxed inbox, alive while the
    // callback is registered; the report is valid for the call.
    let (inbox, report) =
        unsafe { (&*(context as *const Inbox), std::slice::from_raw_parts(report, len.max(0) as usize)) };
    let mut q = inbox.reports.borrow_mut();
    if q.len() == QUEUE {
        q.pop_front();
    }
    q.push_back(report.to_vec());
    // A report is in: `read` can return.
    // SAFETY: the reading thread's run loop, running this callback.
    unsafe { CFRunLoopStop(inbox.run_loop.get()) };
}

pub struct Device {
    device: IOHIDDeviceRef,
    mode: CfString,
    inbox: Box<Inbox>,
    /// The run loop the device delivers reports on: the reading thread's.
    scheduled: Cell<CFRunLoopRef>,
}

// SAFETY: the device is used from one thread at a time; its reports are
// delivered on the run loop of whichever thread reads.
unsafe impl Send for Device {}

impl Device {
    /// Waits up to `timeout_ms` for an input report; `Ok(0)` when none came.
    pub fn read(&self, buf: &mut [u8], timeout_ms: i32) -> Result<usize, String> {
        // SAFETY: the current thread's run loop, scheduled on in our own
        // mode so nothing else of the thread's runs here.
        unsafe {
            let here = CFRunLoopGetCurrent();
            if self.scheduled.get() != here {
                if !self.scheduled.get().is_null() {
                    IOHIDDeviceUnscheduleFromRunLoop(self.device, self.scheduled.get(), self.mode.0);
                }
                IOHIDDeviceScheduleWithRunLoop(self.device, here, self.mode.0);
                self.scheduled.set(here);
                self.inbox.run_loop.set(here);
            }
            if self.inbox.reports.borrow().is_empty() {
                let secs = f64::from(timeout_ms.max(0)) / 1000.0;
                CFRunLoopRunInMode(self.mode.0, secs, 1);
            }
        }
        let Some(report) = self.inbox.reports.borrow_mut().pop_front() else {
            return Ok(0);
        };
        let n = report.len().min(buf.len());
        buf[..n].copy_from_slice(&report[..n]);
        Ok(n)
    }

    pub fn write(&self, report: &[u8]) -> Result<(), String> {
        self.set(REPORT_OUTPUT, report)
    }

    pub fn send_feature(&self, report: &[u8]) -> Result<(), String> {
        self.set(REPORT_FEATURE, report)
    }

    pub fn get_input(&self, id: u8) -> Option<Vec<u8>> {
        self.get(REPORT_INPUT, id)
    }

    pub fn get_feature(&self, id: u8) -> Option<Vec<u8>> {
        self.get(REPORT_FEATURE, id)
    }

    fn set(&self, kind: IOHIDReportType, report: &[u8]) -> Result<(), String> {
        let Some((&id, rest)) = report.split_first() else {
            return Err("empty report".into());
        };
        // IOKit takes the report ID byte only from devices that number
        // their reports.
        let data = if id == 0 { rest } else { report };
        // SAFETY: the length given is the slice's.
        let r =
            unsafe { IOHIDDeviceSetReport(self.device, kind, CFIndex::from(id), data.as_ptr(), data.len() as CFIndex) };
        if r == SUCCESS {
            Ok(())
        } else {
            Err(format!("IOReturn {r:#x}"))
        }
    }

    fn get(&self, kind: IOHIDReportType, id: u8) -> Option<Vec<u8>> {
        let mut buf = vec![0u8; BUFFER + 1];
        let mut len = buf.len() as CFIndex;
        // SAFETY: `len` is the buffer's length, updated to what was read.
        let r = unsafe { IOHIDDeviceGetReport(self.device, kind, CFIndex::from(id), buf.as_mut_ptr(), &mut len) };
        if r != SUCCESS {
            return None;
        }
        buf.truncate(len.max(0) as usize);
        // Numbered reports come back with their ID first.
        if id != 0 && buf.first() == Some(&id) {
            buf.remove(0);
        }
        (!buf.is_empty()).then_some(buf)
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        // SAFETY: stops the callback before its context is freed, then
        // closes and releases the device made in `open`.
        unsafe {
            IOHIDDeviceRegisterInputReportCallback(self.device, null_mut(), 0, None, null_mut());
            if !self.scheduled.get().is_null() {
                IOHIDDeviceUnscheduleFromRunLoop(self.device, self.scheduled.get(), self.mode.0);
            }
            IOHIDDeviceClose(self.device, 0);
            CFRelease(self.device.cast_const());
        }
    }
}
