//! HID on Windows through the HID class driver: SetupAPI to list device
//! interfaces, overlapped file reads and writes for reports, and `HidD_*`
//! for feature and input reports.

use std::cell::{Cell, UnsafeCell};
use std::ffi::{c_void, CString};
use std::ptr::{null, null_mut};

use windows_sys::core::GUID;
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW,
    DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, HDEVINFO, SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
};
use windows_sys::Win32::Devices::HumanInterfaceDevice::{
    HidD_FreePreparsedData, HidD_GetAttributes, HidD_GetFeature, HidD_GetHidGuid, HidD_GetInputReport,
    HidD_GetPreparsedData, HidD_GetProductString, HidD_GetSerialNumberString, HidD_SetFeature, HidP_GetCaps,
    HIDD_ATTRIBUTES, HIDP_CAPS, HIDP_STATUS_SUCCESS, PHIDP_PREPARSED_DATA,
};
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_IO_PENDING, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
    WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, ReadFile, WriteFile, FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::Threading::{CreateEventW, ResetEvent, WaitForSingleObject};
use windows_sys::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};

use crate::{interface_in_path, DeviceInfo, BUFFER};

/// How long a write may take before it's given up on.
const WRITE_TIMEOUT_MS: u32 = 1000;

pub struct Hid;

impl Hid {
    pub fn new() -> Result<Self, String> {
        Ok(Self)
    }

    /// The devices from these vendors plugged in now, one entry per
    /// top-level collection (Windows gives each its own interface).
    pub fn scan(&mut self, vendors: &[u16]) -> Vec<DeviceInfo> {
        let mut out = Vec::new();
        for path in interface_paths() {
            let Some(handle) = Handle::open(&path, 0, 0) else {
                continue;
            };
            let Some(info) = describe(&handle, &path) else {
                continue;
            };
            if vendors.contains(&info.vendor_id) {
                out.push(info);
            }
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }

    pub fn open(&self, d: &DeviceInfo) -> Result<Device, String> {
        let path = d.path.to_str().map_err(|e| e.to_string())?;
        let wide = wide(path);
        let handle = Handle::open(&wide, GENERIC_READ | GENERIC_WRITE, FILE_FLAG_OVERLAPPED)
            .ok_or_else(|| format!("{path}: {}", std::io::Error::last_os_error()))?;
        let caps = caps(&handle).ok_or_else(|| format!("{path}: no report descriptor"))?;
        Device::new(handle, &caps)
    }
}

/// Every HID device interface present, as NUL-terminated UTF-16 paths.
fn interface_paths() -> Vec<Vec<u16>> {
    let mut guid: GUID = unsafe { std::mem::zeroed() };
    // SAFETY: writes one GUID.
    unsafe { HidD_GetHidGuid(&mut guid) };
    // SAFETY: no enumerator or window; the set is freed below.
    let set: HDEVINFO =
        unsafe { SetupDiGetClassDevsW(&guid, null(), null_mut(), DIGCF_PRESENT | DIGCF_DEVICEINTERFACE) };
    if set == -1 {
        return Vec::new();
    }
    let mut paths = Vec::new();
    for index in 0.. {
        let mut iface: SP_DEVICE_INTERFACE_DATA = unsafe { std::mem::zeroed() };
        iface.cbSize = size_of::<SP_DEVICE_INTERFACE_DATA>() as u32;
        // SAFETY: `iface` is sized for the call.
        if unsafe { SetupDiEnumDeviceInterfaces(set, null(), &guid, index, &mut iface) } == 0 {
            break;
        }
        let mut need = 0u32;
        // SAFETY: a size query; nothing is written but `need`.
        unsafe { SetupDiGetDeviceInterfaceDetailW(set, &iface, null_mut(), 0, &mut need, null_mut()) };
        if (need as usize) < size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() {
            continue;
        }
        // u32 storage keeps the detail struct aligned.
        let mut buf = vec![0u32; (need as usize).div_ceil(4)];
        let detail = buf.as_mut_ptr().cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
        // SAFETY: `buf` holds `need` bytes, aligned for the struct, whose
        // size field the call wants as the fixed part's size.
        let ok = unsafe {
            (*detail).cbSize = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
            SetupDiGetDeviceInterfaceDetailW(set, &iface, detail, need, null_mut(), null_mut())
        };
        if ok == 0 {
            continue;
        }
        // SAFETY: the path is NUL-terminated within `buf`.
        let path = unsafe {
            let start = std::ptr::addr_of!((*detail).DevicePath).cast::<u16>();
            let max = (need as usize - 4) / 2;
            let len = (0..max).find(|&i| *start.add(i) == 0).unwrap_or(max);
            std::slice::from_raw_parts(start, len + 1).to_vec()
        };
        paths.push(path);
    }
    // SAFETY: made above, used no further.
    unsafe { SetupDiDestroyDeviceInfoList(set) };
    paths
}

fn describe(handle: &Handle, path: &[u16]) -> Option<DeviceInfo> {
    let mut attrs: HIDD_ATTRIBUTES = unsafe { std::mem::zeroed() };
    attrs.Size = size_of::<HIDD_ATTRIBUTES>() as u32;
    // SAFETY: `attrs` is sized for the call.
    if !unsafe { HidD_GetAttributes(handle.0, &mut attrs) } {
        return None;
    }
    let caps = caps(handle)?;
    let path = String::from_utf16_lossy(&path[..path.len() - 1]);
    Some(DeviceInfo {
        interface: interface_in_path(&path),
        path: CString::new(path).ok()?,
        vendor_id: attrs.VendorID,
        product_id: attrs.ProductID,
        usage_page: caps.UsagePage,
        usage: caps.Usage,
        product: string(handle, HidD_GetProductString).trim().to_string(),
        serial: string(handle, HidD_GetSerialNumberString),
    })
}

fn caps(handle: &Handle) -> Option<HIDP_CAPS> {
    let mut data: PHIDP_PREPARSED_DATA = 0;
    // SAFETY: writes one pointer, freed below.
    if !unsafe { HidD_GetPreparsedData(handle.0, &mut data) } {
        return None;
    }
    let mut caps: HIDP_CAPS = unsafe { std::mem::zeroed() };
    // SAFETY: `data` is the device's, `caps` sized for the call.
    let status = unsafe { HidP_GetCaps(data, &mut caps) };
    // SAFETY: from HidD_GetPreparsedData.
    unsafe { HidD_FreePreparsedData(data) };
    (status == HIDP_STATUS_SUCCESS).then_some(caps)
}

type StringFn = unsafe extern "system" fn(HANDLE, *mut c_void, u32) -> bool;

fn string(handle: &Handle, get: StringFn) -> String {
    let mut buf = [0u16; 128];
    // SAFETY: the length given is the buffer's, in bytes.
    if !unsafe { get(handle.0, buf.as_mut_ptr().cast(), size_of_val(&buf) as u32) } {
        return String::new();
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

struct Handle(HANDLE);

impl Handle {
    /// Opens a device path (NUL-terminated UTF-16), sharing it with other
    /// programs. Access 0 is enough to describe it.
    fn open(path: &[u16], access: u32, flags: u32) -> Option<Self> {
        // SAFETY: `path` is NUL-terminated.
        let h = unsafe {
            CreateFileW(
                path.as_ptr(),
                access,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null(),
                OPEN_EXISTING,
                flags,
                null_mut(),
            )
        };
        (h != INVALID_HANDLE_VALUE && !h.is_null()).then_some(Self(h))
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: owned, closed once.
        unsafe { CloseHandle(self.0) };
    }
}

/// An overlapped operation's state, kept at a fixed address while the
/// system may write to it.
struct Pending {
    ov: UnsafeCell<OVERLAPPED>,
    event: Handle,
}

impl Pending {
    fn new() -> Result<Box<Self>, String> {
        // SAFETY: an unnamed manual-reset event.
        let event = unsafe { CreateEventW(null(), 1, 0, null()) };
        if event.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let mut ov: OVERLAPPED = unsafe { std::mem::zeroed() };
        ov.hEvent = event;
        Ok(Box::new(Self { ov: UnsafeCell::new(ov), event: Handle(event) }))
    }

    fn start(&self) -> *mut OVERLAPPED {
        // SAFETY: no operation is in flight on it.
        unsafe {
            ResetEvent(self.event.0);
            let ov = &mut *self.ov.get();
            ov.Internal = 0;
            ov.InternalHigh = 0;
            ov.Anonymous = std::mem::zeroed();
        }
        self.ov.get()
    }
}

pub struct Device {
    handle: Handle,
    input_len: usize,
    output_len: usize,
    feature_len: usize,
    read: Box<Pending>,
    read_buf: Box<UnsafeCell<[u8; BUFFER + 1]>>,
    reading: Cell<bool>,
    write: Box<Pending>,
}

// SAFETY: the handle and the buffers it fills belong to this device alone;
// it is used from one thread at a time.
unsafe impl Send for Device {}

impl Device {
    fn new(handle: Handle, caps: &HIDP_CAPS) -> Result<Self, String> {
        Ok(Self {
            handle,
            input_len: (caps.InputReportByteLength as usize).min(BUFFER + 1),
            output_len: caps.OutputReportByteLength as usize,
            feature_len: caps.FeatureReportByteLength as usize,
            read: Pending::new()?,
            read_buf: Box::new(UnsafeCell::new([0; BUFFER + 1])),
            reading: Cell::new(false),
            write: Pending::new()?,
        })
    }

    /// Waits up to `timeout_ms` for an input report; `Ok(0)` when none came.
    /// A read left waiting carries on into the next call.
    pub fn read(&self, buf: &mut [u8], timeout_ms: i32) -> Result<usize, String> {
        if self.input_len == 0 {
            return Ok(0);
        }
        if !self.reading.get() {
            let ov = self.read.start();
            // SAFETY: the buffer and OVERLAPPED are boxed and outlive the
            // read (Drop cancels and waits for it).
            let ok =
                unsafe { ReadFile(self.handle.0, self.read_buf.get().cast(), self.input_len as u32, null_mut(), ov) };
            if ok == 0 && unsafe { GetLastError() } != ERROR_IO_PENDING {
                return Err(std::io::Error::last_os_error().to_string());
            }
            self.reading.set(true);
        }
        // SAFETY: a valid event handle.
        match unsafe { WaitForSingleObject(self.read.event.0, timeout_ms.max(0) as u32) } {
            WAIT_OBJECT_0 => {}
            WAIT_TIMEOUT => return Ok(0),
            _ => return Err(std::io::Error::last_os_error().to_string()),
        }
        self.reading.set(false);
        let mut n = 0u32;
        // SAFETY: the read has finished.
        if unsafe { GetOverlappedResult(self.handle.0, self.read.ov.get(), &mut n, 0) } == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        // SAFETY: no read in flight now.
        let data = unsafe { &(&*self.read_buf.get())[..n as usize] };
        // Reports from devices that don't number them come with a 0 first.
        let data = match data {
            [0, rest @ ..] => rest,
            d => d,
        };
        let n = data.len().min(buf.len());
        buf[..n].copy_from_slice(&data[..n]);
        Ok(n)
    }

    pub fn write(&self, report: &[u8]) -> Result<(), String> {
        // Windows takes only whole reports of the device's output length.
        let mut buf = report.to_vec();
        buf.resize(buf.len().max(self.output_len), 0);
        let ov = self.write.start();
        // SAFETY: `buf` and the OVERLAPPED live until the write is done or
        // cancelled below.
        let ok = unsafe { WriteFile(self.handle.0, buf.as_ptr(), buf.len() as u32, null_mut(), ov) };
        if ok == 0 && unsafe { GetLastError() } != ERROR_IO_PENDING {
            return Err(std::io::Error::last_os_error().to_string());
        }
        // SAFETY: a valid event handle.
        if unsafe { WaitForSingleObject(self.write.event.0, WRITE_TIMEOUT_MS) } != WAIT_OBJECT_0 {
            // SAFETY: cancels this write and waits for it to let go of `buf`.
            unsafe {
                CancelIoEx(self.handle.0, ov);
                let mut n = 0;
                GetOverlappedResult(self.handle.0, ov, &mut n, 1);
            }
            return Err("write timed out".into());
        }
        let mut n = 0u32;
        // SAFETY: the write has finished.
        if unsafe { GetOverlappedResult(self.handle.0, ov, &mut n, 0) } == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(())
    }

    pub fn send_feature(&self, report: &[u8]) -> Result<(), String> {
        let mut buf = report.to_vec();
        buf.resize(buf.len().max(self.feature_len), 0);
        // SAFETY: the length given is the buffer's.
        if unsafe { HidD_SetFeature(self.handle.0, buf.as_ptr().cast(), buf.len() as u32) } {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error().to_string())
        }
    }

    pub fn get_input(&self, id: u8) -> Option<Vec<u8>> {
        self.get(HidD_GetInputReport, id, self.input_len)
    }

    pub fn get_feature(&self, id: u8) -> Option<Vec<u8>> {
        self.get(HidD_GetFeature, id, self.feature_len)
    }

    fn get(&self, call: StringFn, id: u8, len: usize) -> Option<Vec<u8>> {
        if len < 2 {
            return None;
        }
        let mut buf = vec![0u8; len];
        buf[0] = id;
        // SAFETY: the length given is the buffer's.
        unsafe { call(self.handle.0, buf.as_mut_ptr().cast(), len as u32) }.then(|| buf.split_off(1))
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        if self.reading.get() {
            // SAFETY: cancels the read in flight and waits for it to let go
            // of the buffer before it's freed.
            unsafe {
                CancelIoEx(self.handle.0, self.read.ov.get());
                let mut n = 0;
                GetOverlappedResult(self.handle.0, self.read.ov.get(), &mut n, 1);
            }
        }
    }
}
