//! Decodes a whole audio file into memory with Symphonia.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::codecs::CodecParameters;
use symphonia::core::errors::Error;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;

use crate::track::{Audio, AudioWriter};

/// File extensions the scanner offers and the decoder can read.
pub const EXTENSIONS: &[&str] = &[
    "mp3", "flac", "wav", "wave", "ogg", "oga", "m4a", "mp4", "aac", "aif", "aiff", "caf", "mka",
];

/// Decodes `path` to stereo frames. Mono is duplicated to both sides; channels
/// beyond the first two are dropped.
pub fn decode(path: &Path) -> Result<Audio, String> {
    let audio = Decoder::open(path)?.run(|_| true)?;
    Ok(Arc::try_unwrap(audio).unwrap_or_else(|_| unreachable!("the writer is gone")))
}

/// An opened file, ready to decode into an [`Audio`] that can play while the
/// rest is still decoding.
pub struct Decoder {
    path: PathBuf,
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    writer: AudioWriter,
    /// Whether the file says how long it is; if not, the length is only an
    /// estimate and the audio shouldn't play until decoding is done.
    length_known: bool,
}

impl Decoder {
    /// Opens `path` and reads its headers; doesn't decode any audio yet.
    pub fn open(path: &Path) -> Result<Decoder, String> {
        let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Decoder::open_source(path, Box::new(file))
    }

    /// Opens `path` while it's still arriving: it will hold `size` bytes,
    /// and reads wait for them while `wanted` says so.
    pub fn open_growing(
        path: &Path,
        size: u64,
        wanted: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> Result<Decoder, String> {
        let file = crate::growing::GrowingFile::open(path, size, wanted)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Decoder::open_source(path, Box::new(file))
    }

    /// Reads the headers of `source`, the bytes of `path`.
    fn open_source(path: &Path, source: Box<dyn MediaSource>) -> Result<Decoder, String> {
        let err = |e: Error| format!("{}: {e}", path.display());
        let mss = MediaSourceStream::new(source, MediaSourceStreamOptions::default());
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }
        let format = symphonia::default::get_probe()
            .probe(
                &hint,
                mss,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .map_err(err)?;
        let track = format
            .default_track(TrackType::Audio)
            .ok_or_else(|| format!("{}: no audio track", path.display()))?;
        let track_id = track.id;
        let n_frames = track.num_frames;
        let Some(CodecParameters::Audio(params)) = track.codec_params.clone() else {
            return Err(format!("{}: unknown codec", path.display()));
        };
        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(&params, &AudioDecoderOptions::default())
            .map_err(err)?;
        let sample_rate = params.sample_rate.unwrap_or(0);
        let length_known = n_frames.is_some() && sample_rate > 0;
        // Without a length, guess ten minutes; the audio grows past it as needed.
        let expected = n_frames.map_or(600 * sample_rate.max(44_100) as usize, |n| n as usize);
        let (_, writer) = Audio::writer(sample_rate, expected);
        Ok(Decoder {
            path: path.to_owned(),
            format,
            decoder,
            track_id,
            writer,
            length_known,
        })
    }

    /// The audio being decoded; frames appear in it as [`Decoder::run`] goes.
    pub fn audio(&self) -> &Arc<Audio> {
        self.writer.audio()
    }

    pub fn length_known(&self) -> bool {
        self.length_known
    }

    /// The file's length in seconds, when its headers say.
    pub fn seconds(&self) -> Option<f64> {
        let a = self.writer.audio();
        (self.length_known && a.sample_rate > 0).then(|| a.len() as f64 / a.sample_rate as f64)
    }

    /// Decodes to the end without keeping the audio: `each` gets the sample
    /// rate and each packet as interleaved stereo, and stops decoding early
    /// by returning false. For files too long to hold in memory, such as a
    /// recorded set. Returns the sample rate and the frames decoded.
    pub fn stream(
        mut self,
        mut each: impl FnMut(u32, &[f32]) -> bool,
    ) -> Result<(u32, u64), String> {
        let path = self.path.clone();
        let err = |e: Error| format!("{}: {e}", path.display());
        let mut buf: Vec<f32> = Vec::new();
        let mut stereo: Vec<f32> = Vec::new();
        let mut rate = 0;
        let mut frames = 0u64;
        loop {
            let packet = match self.format.next_packet() {
                Ok(Some(p)) => p,
                Ok(None) => break,
                Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(err(e)),
            };
            if packet.track_id != self.track_id {
                continue;
            }
            let decoded = match self.decoder.decode(&packet) {
                Ok(d) => d,
                Err(Error::DecodeError(_)) => continue,
                Err(e) => return Err(err(e)),
            };
            let spec = decoded.spec();
            if rate == 0 {
                rate = spec.rate();
            } else if rate != spec.rate() {
                return Err(format!("{}: sample rate changes", path.display()));
            }
            let channels = spec.channels().count().max(1);
            decoded.copy_to_vec_interleaved(&mut buf);
            stereo.clear();
            for f in buf.chunks_exact(channels) {
                stereo.extend_from_slice(&[f[0], if channels > 1 { f[1] } else { f[0] }]);
            }
            frames += (stereo.len() / 2) as u64;
            if !each(rate, &stereo) {
                return Err(format!("{}: decoding cancelled", path.display()));
            }
        }
        if frames == 0 || rate == 0 {
            return Err(format!("{}: no audio decoded", path.display()));
        }
        Ok((rate, frames))
    }

    /// Decodes to the end, calling `progress` with the audio after each
    /// packet; decoding stops early if it returns false.
    pub fn run(mut self, mut progress: impl FnMut(&Audio) -> bool) -> Result<Arc<Audio>, String> {
        let path = self.path.clone();
        let err = |e: Error| format!("{}: {e}", path.display());
        let mut buf: Vec<f32> = Vec::new();
        loop {
            let packet = match self.format.next_packet() {
                Ok(Some(p)) => p,
                Ok(None) => break,
                Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(err(e)),
            };
            if packet.track_id != self.track_id {
                continue;
            }
            let decoded = match self.decoder.decode(&packet) {
                Ok(d) => d,
                // A corrupt packet is skipped, as players do.
                Err(Error::DecodeError(_)) => continue,
                Err(e) => return Err(err(e)),
            };
            let spec = decoded.spec();
            if self.writer.audio().sample_rate != spec.rate() {
                if self.writer.audio().decoded() > 0 {
                    return Err(format!("{}: sample rate changes", path.display()));
                }
                // The headers didn't give the rate; start over with the real one.
                let expected = self.writer.audio().len();
                self.writer = Audio::writer(spec.rate(), expected).1;
                self.length_known = false;
            }
            let channels = spec.channels().count().max(1);
            decoded.copy_to_vec_interleaved(&mut buf);
            for f in buf.chunks_exact(channels) {
                self.writer
                    .push([f[0], if channels > 1 { f[1] } else { f[0] }]);
            }
            if !progress(self.writer.audio()) {
                return Err(format!("{}: decoding cancelled", path.display()));
            }
        }
        let audio = self.writer.finish();
        if audio.is_empty() || audio.sample_rate == 0 {
            return Err(format!("{}: no audio decoded", path.display()));
        }
        Ok(audio)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Write;

    /// Writes a 16-bit PCM WAV file.
    pub(crate) fn write_wav(path: &Path, rate: u32, channels: u16, samples: &[i16]) {
        let data_len = (samples.len() * 2) as u32;
        let mut f = File::create(path).unwrap();
        let block = channels * 2;
        f.write_all(b"RIFF").unwrap();
        f.write_all(&(36 + data_len).to_le_bytes()).unwrap();
        f.write_all(b"WAVEfmt ").unwrap();
        f.write_all(&16u32.to_le_bytes()).unwrap();
        f.write_all(&1u16.to_le_bytes()).unwrap();
        f.write_all(&channels.to_le_bytes()).unwrap();
        f.write_all(&rate.to_le_bytes()).unwrap();
        f.write_all(&(rate * block as u32).to_le_bytes()).unwrap();
        f.write_all(&block.to_le_bytes()).unwrap();
        f.write_all(&16u16.to_le_bytes()).unwrap();
        f.write_all(b"data").unwrap();
        f.write_all(&data_len.to_le_bytes()).unwrap();
        for s in samples {
            f.write_all(&s.to_le_bytes()).unwrap();
        }
    }

    #[test]
    fn decodes_mono_wav_to_stereo() {
        let dir = std::env::temp_dir().join(format!("djsugar-decode-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tone.wav");
        let samples: Vec<i16> = (0..48_000)
            .map(|i| ((i as f32 / 48.0).sin() * 16_000.0) as i16)
            .collect();
        write_wav(&path, 48_000, 1, &samples);
        let audio = decode(&path).unwrap();
        assert_eq!(audio.sample_rate, 48_000);
        assert_eq!(audio.len(), 48_000);
        assert!(audio.is_complete());
        let f = audio.frame(100);
        assert_eq!(f[0], f[1]);
        assert!((f[0] - samples[100] as f32 / 32_768.0).abs() < 1e-3);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn audio_plays_while_decoding() {
        let dir = std::env::temp_dir().join(format!("djsugar-stream-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("long.wav");
        // Ten seconds, so it spans several chunks.
        let samples: Vec<i16> = (0..441_000)
            .flat_map(|i| {
                let v = ((i as f32 / 20.0).sin() * 16_000.0) as i16;
                [v, -v]
            })
            .collect();
        write_wav(&path, 44_100, 2, &samples);
        let d = Decoder::open(&path).unwrap();
        assert!(d.length_known());
        let audio = d.audio().clone();
        assert_eq!(audio.len(), 441_000);
        assert_eq!(audio.decoded(), 0);
        let mut seen_partial = false;
        let done = d
            .run(|a| {
                if a.decoded() > 0 && !a.is_complete() {
                    seen_partial = true;
                    assert_eq!(a.frame(100)[0], samples[200] as f32 / 32_768.0);
                    // Not decoded yet: silence.
                    assert_eq!(a.frame(a.len() - 1), [0.0; 2]);
                }
                true
            })
            .unwrap();
        assert!(seen_partial);
        assert!(Arc::ptr_eq(&done, &audio) && audio.is_complete());
        let last = audio.frame(440_999);
        assert_eq!(last[1], samples[2 * 440_999 + 1] as f32 / 32_768.0);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
