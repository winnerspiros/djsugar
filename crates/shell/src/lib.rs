#[cfg(target_os = "linux")]
mod x11;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Resize {
        width: u32,
        height: u32,
        scale: f32,
    },
    Redraw,
    Pointer {
        x: f32,
        y: f32,
    },
    Button {
        button: u8,
        pressed: bool,
    },
    /// A key; `mods` is the X modifier mask: 1 Shift, 4 Control, 8 Alt.
    Key {
        sym: u32,
        pressed: bool,
        mods: u16,
    },
    Close,
}

#[derive(Debug)]
pub struct Frame<'a> {
    pub bytes: &'a [u8],
    pub stride: usize,
    pub width: u32,
    pub height: u32,
}

pub trait App {
    fn event(&mut self, event: Event) -> bool;
    fn animating(&self) -> bool;
    /// The app asked to close.
    fn closing(&self) -> bool {
        false
    }
    fn draw(&mut self, present: &mut dyn FnMut(Frame<'_>));
}

#[derive(Debug)]
pub struct Error(String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

#[cfg(target_os = "linux")]
impl Error {
    fn from(e: impl std::fmt::Display) -> Self {
        Self(e.to_string())
    }
}

#[cfg(target_os = "linux")]
pub fn run(title: &str, width: u32, height: u32, app: &mut impl App) -> Result<(), Error> {
    x11::run(title, width, height, app)
}

pub mod keysym {
    pub const BACKSPACE: u32 = 0xff08;
    pub const TAB: u32 = 0xff09;
    pub const RETURN: u32 = 0xff0d;
    pub const ESCAPE: u32 = 0xff1b;
    pub const HOME: u32 = 0xff50;
    pub const LEFT: u32 = 0xff51;
    pub const UP: u32 = 0xff52;
    pub const RIGHT: u32 = 0xff53;
    pub const DOWN: u32 = 0xff54;
    pub const PAGE_UP: u32 = 0xff55;
    pub const PAGE_DOWN: u32 = 0xff56;
    pub const END: u32 = 0xff57;
    pub const DELETE: u32 = 0xffff;
    /// F1; F2 to F12 follow it.
    pub const F1: u32 = 0xffbe;

    pub fn char(sym: u32) -> Option<char> {
        match sym {
            0x20..=0x7e | 0xa0..=0xff => char::from_u32(sym),
            0x0100_0000..=0x0110_ffff => char::from_u32(sym - 0x0100_0000),
            0x7c1..=0x7d9 => char::from_u32(sym - 0x7c1 + 0x391),
            0x7e1..=0x7f9 => char::from_u32(sym - 0x7e1 + 0x3b1),
            _ => None,
        }
    }
}
