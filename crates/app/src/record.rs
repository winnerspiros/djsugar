//! Mix recorder: 32-bit float stereo WAV, written on its own thread.

use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::mpsc::{Sender, channel};

#[derive(Debug)]
pub struct Recorder {
    tx: Sender<Vec<f32>>,
    pub path: PathBuf,
    pub frames: u64,
}

fn header(rate: u32, bytes: u32) -> [u8; 44] {
    let mut h = [0u8; 44];
    let mut put = |at: usize, v: &[u8]| h[at..at + v.len()].copy_from_slice(v);
    put(0, b"RIFF");
    put(4, &(36 + bytes).to_le_bytes());
    put(8, b"WAVEfmt ");
    put(16, &16u32.to_le_bytes());
    put(20, &3u16.to_le_bytes());
    put(22, &2u16.to_le_bytes());
    put(24, &rate.to_le_bytes());
    put(28, &(rate * 8).to_le_bytes());
    put(32, &8u16.to_le_bytes());
    put(34, &32u16.to_le_bytes());
    put(36, b"data");
    put(40, &bytes.to_le_bytes());
    h
}

impl Recorder {
    pub fn start(dir: PathBuf, rate: u32) -> std::io::Result<Self> {
        std::fs::create_dir_all(&dir)?;
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let path = dir.join(format!("mix-{secs}.wav"));
        let mut file = std::io::BufWriter::new(std::fs::File::create(&path)?);
        file.write_all(&header(rate, 0))?;
        let (tx, rx) = channel::<Vec<f32>>();
        std::thread::spawn(move || {
            for block in rx {
                for v in block {
                    let _ = file.write_all(&v.to_le_bytes());
                }
            }
            let Ok(mut f) = file.into_inner() else { return };
            let len = f.seek(SeekFrom::End(0)).unwrap_or(44).saturating_sub(44) as u32;
            let _ = f.seek(SeekFrom::Start(0)).and_then(|_| f.write_all(&header(rate, len)));
        });
        Ok(Self { tx, path, frames: 0 })
    }

    pub fn push(&mut self, block: Vec<f32>) {
        self.frames += block.len() as u64 / 2;
        let _ = self.tx.send(block);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_is_float_stereo() {
        let h = header(48000, 800);
        assert_eq!(&h[0..4], b"RIFF");
        assert_eq!(u16::from_le_bytes([h[20], h[21]]), 3);
        assert_eq!(u32::from_le_bytes([h[40], h[41], h[42], h[43]]), 800);
    }
}
