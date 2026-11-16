//! Encoders: the master's interleaved stereo in, stream bytes out.
//!
//! - Opus in Ogg, at 48 kHz (resampled when the engine runs at another
//!   rate). A new title starts a new chained Ogg stream with the title in its
//!   tags, which is how Ogg streams carry now-playing.
//! - MP3 (LAME), at the engine's rate up to 48 kHz. Its now-playing goes to
//!   the server separately (see `server.rs`).

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
/// The encoder's delay at 48 kHz (2.5 ms), which players skip;
/// `opus_delay_is_pre_skip` measures it.
const PRE_SKIP: u16 = 120;

pub struct OggOpus {
    encoder: rusty_opus::OpusEncoder,
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
            rusty_opus::OpusEncoder::new(OPUS_RATE as i32, 2, rusty_opus::Application::Audio)
                .map_err(|e| e.to_string())?;
        encoder.bitrate_bps = bitrate as i32 * 1000;
        encoder.complexity = 10;
        let pre_skip = PRE_SKIP;
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
                .encode(
                    &self.pending[used..used + frame],
                    OPUS_FRAME,
                    &mut self.packet,
                )
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
    encoder: rusty_mp3::encode::Mp3Encode,
    header: rusty_mp3::header::FrameHeader,
    resampler: Option<Resampler>,
    resampled: Vec<f32>,
    /// Samples per channel waiting for a whole frame.
    pending: [Vec<f32>; 2],
}

impl Mp3 {
    pub fn new(bitrate: u32, rate: u32) -> Result<Self, String> {
        // MP3 goes up to 48 kHz; 88.2 and 96 kHz are halved.
        let mp3_rate = if rate > 48_000 { rate / 2 } else { rate };
        Ok(Self {
            encoder: rusty_mp3::encode::Mp3Encode::new(),
            header: rusty_mp3::encoder_header(mp3_rate, 2, bitrate.clamp(32, 320))
                .map_err(|e| e.to_string())?,
            resampler: Resampler::new(rate, mp3_rate),
            resampled: Vec::new(),
            pending: [Vec::new(), Vec::new()],
        })
    }

    /// The last frames, for the end of a file: the rest of the input and the
    /// encoder's delay, padded with silence.
    pub fn finish(&mut self) -> Result<Vec<u8>, String> {
        let frame = self.header.version.samples_per_frame();
        let len = (self.pending[0].len() + rusty_mp3::encode::lametag::ENCODER_DELAY)
            .div_ceil(frame)
            * frame;
        for c in &mut self.pending {
            c.resize(len, 0.0);
        }
        self.frames()
    }

    /// Encodes every whole frame waiting, one MP3 frame each, with no bit
    /// reservoir between them so each can go out as soon as it's made.
    fn frames(&mut self) -> Result<Vec<u8>, String> {
        let frame = self.header.version.samples_per_frame();
        let whole = self.pending[0].len() / frame * frame;
        let mut out = Vec::new();
        for at in (0..whole).step_by(frame) {
            let block = [
                &self.pending[0][at..at + frame],
                &self.pending[1][at..at + frame],
            ];
            out.extend(
                self.encoder
                    .encode_frame(&self.header, &block, None)
                    .map_err(|e| e.to_string())?,
            );
        }
        for c in &mut self.pending {
            c.drain(..whole);
        }
        Ok(out)
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
        for s in input.as_chunks::<2>().0 {
            self.pending[0].push(s[0]);
            self.pending[1].push(s[1]);
        }
        self.frames()
    }

    fn content_type(&self) -> &'static str {
        "audio/mpeg"
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

    /// What goes in comes back out: as long, at the same level, with no tag
    /// frame in a live stream.
    #[test]
    fn mp3_decodes_back_to_the_tone() {
        let input = tone(44_100, 3.0);
        let mut e = Mp3::new(192, 44_100).unwrap();
        let mut bytes = Vec::new();
        for chunk in input.chunks(1024) {
            bytes.extend(e.encode(chunk).unwrap());
        }
        bytes.extend(e.finish().unwrap());
        assert!(!bytes[..400]
            .windows(4)
            .any(|w| w == b"Info" || w == b"Xing"));
        let mut d = rusty_mp3::Mp3Decoder::new();
        d.push(&bytes);
        d.flush();
        let mut out = Vec::new();
        while let Ok(f) = d.next_frame() {
            assert_eq!((f.sample_rate, f.channels), (44_100, 2));
            out.extend(f.samples);
        }
        let frames = out.len() as f64 / input.len() as f64;
        assert!((0.98..1.08).contains(&frames), "{frames}");
        let rms = |v: &[f32]| (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
        let (a, b) = (rms(&input[44_100..88_200]), rms(&out[44_100..88_200]));
        assert!((b / a - 1.0).abs() < 0.05, "{a} {b}");
    }

    /// A click encoded and decoded comes back `PRE_SKIP` samples late.
    #[test]
    fn opus_delay_is_pre_skip() {
        let mut enc =
            rusty_opus::OpusEncoder::new(OPUS_RATE as i32, 2, rusty_opus::Application::Audio)
                .unwrap();
        enc.bitrate_bps = 256_000;
        let mut dec = rusty_opus::OpusDecoder::new(OPUS_RATE as i32, 2).unwrap();
        let at = 5 * OPUS_FRAME + 100;
        let mut input = vec![0.0f32; 10 * OPUS_FRAME * 2];
        for c in 0..2 {
            input[at * 2 + c] = 0.9;
        }
        let mut out = Vec::new();
        let mut packet = [0u8; 4000];
        let mut frame = vec![0.0f32; OPUS_FRAME * 2];
        for chunk in input.chunks(OPUS_FRAME * 2) {
            let n = enc.encode(chunk, OPUS_FRAME, &mut packet).unwrap();
            dec.decode(&packet[..n], OPUS_FRAME, &mut frame).unwrap();
            out.extend_from_slice(&frame);
        }
        let peak = (0..out.len() / 2)
            .max_by(|&a, &b| out[a * 2].abs().total_cmp(&out[b * 2].abs()))
            .unwrap();
        assert_eq!(peak - at, PRE_SKIP as usize);
    }
}
