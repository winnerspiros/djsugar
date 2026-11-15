//! Encoders: the master's interleaved stereo in, stream bytes out.
//!
//! - Opus in Ogg, at 48 kHz (resampled when the engine runs at another
//!   rate). A new title starts a new chained Ogg stream with the title in its
//!   tags, which is how Ogg streams carry now-playing.
//! - MP3 (LAME), at the engine's rate up to 48 kHz. Its now-playing goes to
//!   the server separately (see `server.rs`).

use std::mem::MaybeUninit;

use mp3lame_encoder::{Builder, FlushNoGap, InterleavedPcm};
use ogg::{PacketWriteEndInfo, PacketWriter};

use crate::resample::Resampler;
use crate::Format;

/// Turns audio into bytes for the server.
pub trait Encoder: Send {
    /// Encodes interleaved stereo at the engine's rate; returns the bytes
    /// ready to send (maybe none yet).
    fn encode(&mut self, samples: &[f32]) -> Result<Vec<u8>, String>;
    /// Puts a new title into the stream itself, where the format can carry
    /// it; returns whether it did.
    fn set_title(&mut self, _artist: &str, _title: &str) -> bool {
        false
    }
    /// The MIME type the server is told.
    fn content_type(&self) -> &'static str;
}

/// An encoder for `format` at `bitrate` kb/s, fed at `rate` Hz.
pub fn encoder(format: Format, bitrate: u32, rate: u32) -> Result<Box<dyn Encoder>, String> {
    Ok(match format {
        Format::Opus => Box::new(OggOpus::new(bitrate, rate)?),
        Format::Mp3 => Box::new(Mp3::new(bitrate, rate)?),
    })
}

/// Opus frames of 20 ms at 48 kHz.
const OPUS_FRAME: usize = 960;
const OPUS_RATE: u32 = 48_000;
/// Ogg pages are closed after this many packets (100 ms), so listeners
/// don't wait for a full page.
const PACKETS_PER_PAGE: u32 = 5;

pub struct OggOpus {
    encoder: opus::Encoder,
    resampler: Option<Resampler>,
    input_rate: u32,
    /// Resampled audio waiting for a full frame.
    pending: Vec<f32>,
    resampled: Vec<f32>,
    writer: PacketWriter<'static, Vec<u8>>,
    serial: u32,
    /// Samples (per channel, at 48 kHz) in the current chained stream,
    /// counting the pre-skip.
    granule: u64,
    pre_skip: u16,
    in_page: u32,
    /// Headers not written yet: at the start, and after a new title.
    headers_due: Option<(String, String)>,
    /// Whether anything of the current stream has been written.
    started: bool,
    /// The last packet and its granule, held back one packet so a stream
    /// can end on it (Ogg marks the end on the last page, and an empty
    /// packet there is invalid Opus).
    held: Option<(Vec<u8>, u64)>,
    packet: Vec<u8>,
}

impl OggOpus {
    pub fn new(bitrate: u32, rate: u32) -> Result<Self, String> {
        let mut encoder =
            opus::Encoder::new(OPUS_RATE, opus::Channels::Stereo, opus::Application::Audio)
                .map_err(|e| e.to_string())?;
        encoder
            .set_bitrate(opus::Bitrate::Bits(bitrate as i32 * 1000))
            .map_err(|e| e.to_string())?;
        let pre_skip = encoder.get_lookahead().map_err(|e| e.to_string())?.max(0) as u16;
        Ok(Self {
            encoder,
            resampler: Resampler::new(rate, OPUS_RATE),
            input_rate: rate,
            pending: Vec::new(),
            resampled: Vec::new(),
            writer: PacketWriter::new(Vec::new()),
            serial: first_serial(),
            granule: pre_skip as u64,
            pre_skip,
            in_page: 0,
            headers_due: Some((String::new(), String::new())),
            started: false,
            held: None,
            packet: vec![0; 4000],
        })
    }

    fn write_headers(&mut self, artist: &str, title: &str) -> Result<(), String> {
        let mut head = b"OpusHead".to_vec();
        head.push(1);
        head.push(2);
        head.extend(self.pre_skip.to_le_bytes());
        head.extend(self.input_rate.to_le_bytes());
        head.extend(0i16.to_le_bytes());
        head.push(0);
        self.write(head, PacketWriteEndInfo::EndPage, 0)?;
        let vendor = format!("DJ Sugar {}", env!("CARGO_PKG_VERSION"));
        let mut comments = Vec::new();
        if !artist.is_empty() {
            comments.push(format!("ARTIST={artist}"));
        }
        if !title.is_empty() {
            comments.push(format!("TITLE={title}"));
        }
        let mut tags = b"OpusTags".to_vec();
        tags.extend((vendor.len() as u32).to_le_bytes());
        tags.extend(vendor.as_bytes());
        tags.extend((comments.len() as u32).to_le_bytes());
        for c in &comments {
            tags.extend((c.len() as u32).to_le_bytes());
            tags.extend(c.as_bytes());
        }
        self.write(tags, PacketWriteEndInfo::EndPage, 0)
    }

    fn write(
        &mut self,
        data: Vec<u8>,
        end: PacketWriteEndInfo,
        granule: u64,
    ) -> Result<(), String> {
        self.writer
            .write_packet(data, self.serial, end, granule)
            .map_err(|e| e.to_string())
    }
}

/// A stream serial that differs from run to run, as Ogg asks.
fn first_serial() -> u32 {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    (t.as_nanos() as u32) ^ std::process::id().rotate_left(16)
}

impl Encoder for OggOpus {
    fn encode(&mut self, samples: &[f32]) -> Result<Vec<u8>, String> {
        if let Some((artist, title)) = self.headers_due.take() {
            self.write_headers(&artist, &title)?;
            self.started = true;
        }
        match &mut self.resampler {
            Some(r) => {
                self.resampled.clear();
                r.process(samples, &mut self.resampled);
                self.pending.extend_from_slice(&self.resampled);
            }
            None => self.pending.extend_from_slice(samples),
        }
        let frame = OPUS_FRAME * 2;
        let mut used = 0;
        while self.pending.len() - used >= frame {
            let n = self
                .encoder
                .encode_float(&self.pending[used..used + frame], &mut self.packet)
                .map_err(|e| e.to_string())?;
            used += frame;
            self.granule += OPUS_FRAME as u64;
            self.in_page += 1;
            let end = if self.in_page >= PACKETS_PER_PAGE {
                self.in_page = 0;
                PacketWriteEndInfo::EndPage
            } else {
                PacketWriteEndInfo::NormalPacket
            };
            if let Some((data, granule)) =
                self.held.replace((self.packet[..n].to_vec(), self.granule))
            {
                self.write(data, end, granule)?;
            }
        }
        self.pending.drain(..used);
        Ok(std::mem::take(self.writer.inner_mut()))
    }

    fn set_title(&mut self, artist: &str, title: &str) -> bool {
        if !self.started {
            // Nothing sent yet: the first headers carry the title.
            self.headers_due = Some((artist.to_string(), title.to_string()));
            return true;
        }
        // End this stream on its last packet, then chain a new one with the
        // new tags.
        if let Some((data, granule)) = self.held.take() {
            let _ = self.write(data, PacketWriteEndInfo::EndStream, granule);
        }
        self.started = false;
        self.serial = self.serial.wrapping_add(1);
        self.granule = self.pre_skip as u64;
        self.in_page = 0;
        // The encoder carries on, so there's no gap; a fresh pre-skip only
        // trims a few milliseconds the player has already heard.
        self.headers_due = Some((artist.to_string(), title.to_string()));
        true
    }

    fn content_type(&self) -> &'static str {
        "audio/ogg"
    }
}

pub struct Mp3 {
    encoder: mp3lame_encoder::Encoder,
    resampler: Option<Resampler>,
    resampled: Vec<f32>,
    out: Vec<u8>,
    flushed: bool,
}

impl Mp3 {
    pub fn new(bitrate: u32, rate: u32) -> Result<Self, String> {
        // MP3 goes up to 48 kHz; 88.2 and 96 kHz are halved.
        let mp3_rate = if rate > 48_000 { rate / 2 } else { rate };
        let brate = match bitrate {
            0..=64 => mp3lame_encoder::Bitrate::Kbps64,
            65..=96 => mp3lame_encoder::Bitrate::Kbps96,
            97..=128 => mp3lame_encoder::Bitrate::Kbps128,
            129..=160 => mp3lame_encoder::Bitrate::Kbps160,
            161..=192 => mp3lame_encoder::Bitrate::Kbps192,
            193..=256 => mp3lame_encoder::Bitrate::Kbps256,
            _ => mp3lame_encoder::Bitrate::Kbps320,
        };
        let mut b = Builder::new().ok_or("the MP3 encoder could not start")?;
        let fail = |e: mp3lame_encoder::BuildError| e.to_string();
        b.set_num_channels(2).map_err(fail)?;
        b.set_sample_rate(mp3_rate).map_err(fail)?;
        b.set_brate(brate).map_err(fail)?;
        b.set_quality(mp3lame_encoder::Quality::Good)
            .map_err(fail)?;
        b.set_to_write_vbr_tag(false).map_err(fail)?;
        Ok(Self {
            encoder: b.build().map_err(fail)?,
            resampler: Resampler::new(rate, mp3_rate),
            resampled: Vec::new(),
            out: Vec::new(),
            flushed: false,
        })
    }

    /// The last frames, for the end of a file.
    pub fn finish(&mut self) -> Result<Vec<u8>, String> {
        self.flushed = true;
        let mut rest: Vec<MaybeUninit<u8>> = vec![MaybeUninit::uninit(); 7200];
        let n = self
            .encoder
            .flush::<FlushNoGap>(&mut rest)
            .map_err(|e| e.to_string())?;
        // SAFETY: the encoder wrote the first `n` bytes.
        Ok(rest[..n]
            .iter()
            .map(|b| unsafe { b.assume_init() })
            .collect())
    }
}

impl Encoder for Mp3 {
    fn encode(&mut self, samples: &[f32]) -> Result<Vec<u8>, String> {
        let input = match &mut self.resampler {
            Some(r) => {
                self.resampled.clear();
                r.process(samples, &mut self.resampled);
                &self.resampled[..]
            }
            None => samples,
        };
        self.out.clear();
        self.out
            .reserve(mp3lame_encoder::max_required_buffer_size(input.len() / 2));
        let n = self
            .encoder
            .encode(InterleavedPcm(input), self.out.spare_capacity_mut())
            .map_err(|e| e.to_string())?;
        // SAFETY: the encoder wrote `n` bytes into the spare capacity.
        unsafe { self.out.set_len(n) };
        Ok(std::mem::take(&mut self.out))
    }

    fn content_type(&self) -> &'static str {
        "audio/mpeg"
    }
}

impl Drop for Mp3 {
    fn drop(&mut self) {
        if self.flushed {
            return;
        }
        let mut rest: Vec<MaybeUninit<u8>> = vec![MaybeUninit::uninit(); 7200];
        let _ = self.encoder.flush::<FlushNoGap>(&mut rest);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(rate: u32, seconds: f64) -> Vec<f32> {
        (0..(rate as f64 * seconds) as usize)
            .flat_map(|i| {
                let v =
                    (0.3 * (std::f64::consts::TAU * 440.0 * i as f64 / rate as f64).sin()) as f32;
                [v, v]
            })
            .collect()
    }

    #[test]
    fn opus_makes_an_ogg_stream_with_headers_and_new_titles() {
        let mut e = OggOpus::new(96, 44_100).unwrap();
        let mut bytes = Vec::new();
        for chunk in tone(44_100, 2.0).chunks(1024) {
            bytes.extend(e.encode(chunk).unwrap());
        }
        assert!(bytes.starts_with(b"OggS"));
        let find =
            |hay: &[u8], needle: &[u8]| hay.windows(needle.len()).filter(|w| *w == needle).count();
        assert_eq!(find(&bytes, b"OpusHead"), 1);
        // About 96 kb/s: two seconds is about 24 kB.
        assert!(
            bytes.len() > 15_000 && bytes.len() < 40_000,
            "{}",
            bytes.len()
        );
        // A new title chains a stream with it in the tags.
        assert!(e.set_title("Daft Punk", "Around the World"));
        let mut more = Vec::new();
        for chunk in tone(44_100, 0.5).chunks(1024) {
            more.extend(e.encode(chunk).unwrap());
        }
        assert_eq!(find(&more, b"OpusHead"), 1);
        assert_eq!(find(&more, b"TITLE=Around the World"), 1);
        assert_eq!(find(&more, b"ARTIST=Daft Punk"), 1);
        // Every page is a valid Ogg page: the reader gets all the packets.
        let mut all = bytes;
        all.extend(more);
        let mut reader = ogg::PacketReader::new(std::io::Cursor::new(all));
        let mut packets = 0;
        while let Ok(Some(_)) = reader.read_packet() {
            packets += 1;
        }
        assert!(packets > 100, "{packets}");
    }

    #[test]
    fn mp3_frames_come_out_at_the_bitrate() {
        let mut e = Mp3::new(128, 48_000).unwrap();
        let mut bytes = Vec::new();
        for chunk in tone(48_000, 2.0).chunks(2048) {
            bytes.extend(e.encode(chunk).unwrap());
        }
        // An MP3 frame starts with 11 set bits.
        assert_eq!(bytes[0], 0xff);
        assert_eq!(bytes[1] & 0xe0, 0xe0);
        // 128 kb/s for about two seconds is about 32 kB.
        assert!(
            bytes.len() > 25_000 && bytes.len() < 36_000,
            "{}",
            bytes.len()
        );
    }
}
