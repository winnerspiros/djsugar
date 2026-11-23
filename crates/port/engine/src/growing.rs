//! A file that is still being written, read as if it were whole: a read
//! past what has arrived waits for more. For hearing a download before it
//! finishes.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use symphonia::core::io::MediaSource;

/// How long a read waits for bytes that don't come before it calls it the
/// end of the file.
const STALL: Duration = Duration::from_secs(60);
/// How often a waiting read looks again.
const POLL: Duration = Duration::from_millis(50);

/// A file growing to `size` bytes, front to back.
pub struct GrowingFile {
    file: File,
    size: u64,
    pos: u64,
    /// Whether anyone still wants the bytes: once not, reads end at once.
    wanted: Arc<dyn Fn() -> bool + Send + Sync>,
    stall: Duration,
}

impl GrowingFile {
    /// Opens `path`, which will hold `size` bytes once it's whole. The file
    /// stays readable when the writer moves it once it's done.
    pub fn open(path: &Path, size: u64, wanted: Arc<dyn Fn() -> bool + Send + Sync>) -> io::Result<GrowingFile> {
        Ok(GrowingFile { file: File::open(path)?, size, pos: 0, wanted, stall: STALL })
    }

    /// How many bytes have arrived.
    fn arrived(&self) -> u64 {
        self.file.metadata().map_or(0, |m| m.len())
    }
}

impl Read for GrowingFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let mut since = Instant::now();
        let mut seen = self.arrived();
        loop {
            // A writer that wrote more than it said: read it all.
            let end = self.size.max(seen);
            if self.pos >= end {
                return Ok(0);
            }
            if seen > self.pos {
                self.file.seek(SeekFrom::Start(self.pos))?;
                let room = (seen - self.pos).min(buf.len() as u64) as usize;
                let n = self.file.read(&mut buf[..room])?;
                self.pos += n as u64;
                return Ok(n);
            }
            if !(self.wanted)() || since.elapsed() >= self.stall {
                return Ok(0);
            }
            std::thread::sleep(POLL);
            let now = self.arrived();
            if now > seen {
                seen = now;
                since = Instant::now();
            }
        }
    }
}

impl Seek for GrowingFile {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let pos = match to {
            SeekFrom::Start(n) => Some(n),
            SeekFrom::End(d) => self.size.max(self.arrived()).checked_add_signed(d),
            SeekFrom::Current(d) => self.pos.checked_add_signed(d),
        };
        let pos = pos.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "seek before 0"))?;
        self.pos = pos;
        Ok(pos)
    }
}

impl MediaSource for GrowingFile {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        Some(self.size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn waits_for_bytes_to_arrive() {
        let dir = std::env::temp_dir().join(format!("djsugar-growing-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("song.mp3");
        let mut writer = File::create(&path).unwrap();
        writer.write_all(b"abc").unwrap();
        let mut file = GrowingFile::open(&path, 6, Arc::new(|| true)).unwrap();
        assert_eq!(file.byte_len(), Some(6));
        let feeder = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            writer.write_all(b"def").unwrap();
        });
        let mut all = Vec::new();
        file.read_to_end(&mut all).unwrap();
        feeder.join().unwrap();
        assert_eq!(all, b"abcdef");
        assert_eq!(file.seek(SeekFrom::End(-2)).unwrap(), 4);
        let mut two = [0; 2];
        file.read_exact(&mut two).unwrap();
        assert_eq!(&two, b"ef");

        // Nobody wants the rest: the read ends instead of waiting.
        let wanted = Arc::new(AtomicBool::new(true));
        let w = wanted.clone();
        let mut file = GrowingFile::open(&path, 100, Arc::new(move || w.load(Ordering::Relaxed))).unwrap();
        file.seek(SeekFrom::Start(6)).unwrap();
        wanted.store(false, Ordering::Relaxed);
        assert_eq!(file.read(&mut two).unwrap(), 0);

        // Bytes that stop coming end it too.
        let mut file = GrowingFile::open(&path, 100, Arc::new(|| true)).unwrap();
        file.stall = Duration::from_millis(120);
        file.seek(SeekFrom::Start(6)).unwrap();
        assert_eq!(file.read(&mut two).unwrap(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
