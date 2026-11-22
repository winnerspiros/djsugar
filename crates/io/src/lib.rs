#[cfg(target_os = "android")]
mod aaudio;

use sugar_engine::Engine;

#[derive(Debug)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Format {
    pub rate: u32,
    pub burst: usize,
    pub buffer: usize,
}

#[derive(Debug)]
pub struct Output {
    pub format: Format,
    #[cfg(target_os = "android")]
    _stream: aaudio::Stream,
}

pub fn open(make: impl FnOnce(Format) -> Engine) -> Result<Output, Error> {
    #[cfg(target_os = "android")]
    {
        let (stream, format) = aaudio::open(make)?;
        Ok(Output { format, _stream: stream })
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = make;
        Err(Error("no audio output on this platform yet".into()))
    }
}
