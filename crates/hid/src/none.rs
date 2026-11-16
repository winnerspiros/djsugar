use crate::DeviceInfo;

pub struct Hid;

impl Hid {
    pub fn new() -> Result<Self, String> {
        Ok(Self)
    }

    pub fn scan(&mut self, _vendors: &[u16]) -> Vec<DeviceInfo> {
        Vec::new()
    }

    pub fn open(&self, _d: &DeviceInfo) -> Result<Device, String> {
        Err("USB HID controllers are not supported on this system yet".into())
    }
}

/// Never made: [`Hid::scan`] finds nothing.
pub enum Device {}

impl Device {
    pub fn read(&self, _buf: &mut [u8], _timeout_ms: i32) -> Result<usize, String> {
        match *self {}
    }

    pub fn write(&self, _report: &[u8]) -> Result<(), String> {
        match *self {}
    }

    pub fn send_feature(&self, _report: &[u8]) -> Result<(), String> {
        match *self {}
    }

    pub fn get_input(&self, _id: u8) -> Option<Vec<u8>> {
        match *self {}
    }

    pub fn get_feature(&self, _id: u8) -> Option<Vec<u8>> {
        match *self {}
    }
}
