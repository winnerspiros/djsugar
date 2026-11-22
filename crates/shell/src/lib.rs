#[cfg(target_os = "linux")]
mod x11;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Resize { width: u32, height: u32, scale: f32 },
    Redraw,
    Pointer { x: f32, y: f32 },
    Button { button: u8, pressed: bool },
    Key { code: u32, pressed: bool },
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

impl Error {
    fn from(e: impl std::fmt::Display) -> Self {
        Self(e.to_string())
    }
}

#[cfg(target_os = "linux")]
pub fn run(title: &str, width: u32, height: u32, app: &mut impl App) -> Result<(), Error> {
    x11::run(title, width, height, app)
}
