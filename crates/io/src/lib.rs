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
    pub silent: bool,
    #[cfg(target_os = "android")]
    _stream: aaudio::Stream,
    #[cfg(not(target_os = "android"))]
    _null: Null,
}

#[cfg(not(target_os = "android"))]
#[derive(Debug)]
struct Null(std::sync::Arc<std::sync::atomic::AtomicBool>);

#[cfg(not(target_os = "android"))]
impl Drop for Null {
    fn drop(&mut self) {
        self.0.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(not(target_os = "android"))]
fn null(mut engine: Engine, format: Format) -> Null {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    let stop = std::sync::Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    std::thread::spawn(move || {
        let mut buf = vec![0.0f32; format.burst * 2];
        let period = Duration::from_secs_f64(format.burst as f64 / f64::from(format.rate));
        let mut next = Instant::now();
        while !flag.load(Ordering::Relaxed) {
            engine.process(&mut buf);
            next += period;
            std::thread::sleep(next.saturating_duration_since(Instant::now()));
        }
    });
    Null(stop)
}

pub fn open(make: impl FnOnce(Format) -> Engine) -> Result<Output, Error> {
    #[cfg(target_os = "android")]
    {
        let (stream, format) = aaudio::open(make)?;
        Ok(Output { format, silent: false, _stream: stream })
    }
    #[cfg(not(target_os = "android"))]
    {
        let format = Format { rate: 48_000, burst: 256, buffer: 512 };
        Ok(Output { format, silent: true, _null: null(make(format), format) })
    }
}
