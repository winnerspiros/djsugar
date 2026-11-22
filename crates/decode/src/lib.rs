use std::fs::File;
use std::path::Path;

use sugar_engine::Track;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Format(String),
    NoAudio,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Format(e) => f.write_str(e),
            Self::NoAudio => f.write_str("no audio track"),
        }
    }
}

impl std::error::Error for Error {}

pub fn file(path: &Path) -> Result<Track, Error> {
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let source = Box::new(File::open(path).map_err(Error::Io)?);
    let len = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    decode(MediaSourceStream::new(source, Default::default()), &hint, len)
}

pub fn bytes(data: Vec<u8>, ext: &str) -> Result<Track, Error> {
    let mut hint = Hint::new();
    hint.with_extension(ext);
    let len = data.len() as u64;
    decode(MediaSourceStream::new(Box::new(std::io::Cursor::new(data)), Default::default()), &hint, len)
}

fn decode(mss: MediaSourceStream<'static>, hint: &Hint, len: u64) -> Result<Track, Error> {
    let fail = |e: SymError| Error::Format(e.to_string());
    let mut format = symphonia::default::get_probe()
        .probe(hint, mss, FormatOptions::default(), MetadataOptions::default())
        .map_err(fail)?;
    let track = format.default_track(TrackType::Audio).ok_or(Error::NoAudio)?;
    let id = track.id;
    let params = track.codec_params.as_ref().and_then(|p| p.audio()).ok_or(Error::NoAudio)?;
    let rate = params.sample_rate.ok_or(Error::NoAudio)?;
    let channels = params.channels.as_ref().map_or(2, |c| c.count()).max(1);
    let expect = track.num_frames.map_or(len as usize / 2, |n| n as usize);
    let mut decoder =
        symphonia::default::get_codecs().make_audio_decoder(params, &AudioDecoderOptions::default()).map_err(fail)?;
    let mut frames: Vec<[f32; 2]> = Vec::with_capacity(expect.min(1 << 28));
    let mut buf: Vec<f32> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(fail(e)),
        };
        if packet.track_id != id {
            continue;
        }
        let audio = match decoder.decode(&packet) {
            Ok(a) => a,
            Err(SymError::DecodeError(_)) => continue,
            Err(e) => return Err(fail(e)),
        };
        buf.resize(audio.samples_interleaved(), 0.0);
        audio.copy_to_slice_interleaved(&mut buf);
        match channels {
            1 => frames.extend(buf.iter().map(|&s| [s, s])),
            2 => frames.extend(buf.chunks_exact(2).map(|c| [c[0], c[1]])),
            n => frames.extend(buf.chunks_exact(n).map(|c| [c[0], c[1]])),
        }
    }
    Ok(Track::new(rate, frames.into_boxed_slice()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(rate: u32, channels: u16, samples: &[i16]) -> Vec<u8> {
        let data = samples.len() as u32 * 2;
        let mut v = Vec::new();
        v.extend(b"RIFF");
        v.extend((36 + data).to_le_bytes());
        v.extend(b"WAVEfmt ");
        v.extend(16u32.to_le_bytes());
        v.extend(1u16.to_le_bytes());
        v.extend(channels.to_le_bytes());
        v.extend(rate.to_le_bytes());
        v.extend((rate * u32::from(channels) * 2).to_le_bytes());
        v.extend((channels * 2).to_le_bytes());
        v.extend(16u16.to_le_bytes());
        v.extend(b"data");
        v.extend(data.to_le_bytes());
        for s in samples {
            v.extend(s.to_le_bytes());
        }
        v
    }

    #[test]
    fn mono_wav_becomes_stereo() {
        let t = bytes(wav(44100, 1, &[0, 16384, -16384, 0]), "wav").unwrap();
        assert_eq!(t.rate, 44100);
        assert_eq!(t.frames.len(), 4);
        assert_eq!(t.frames[1], [0.5, 0.5]);
        assert_eq!(t.frames[2], [-0.5, -0.5]);
    }

    #[test]
    fn stereo_wav_keeps_channels() {
        let t = bytes(wav(48000, 2, &[16384, -16384]), "wav").unwrap();
        assert_eq!(t.frames.as_ref(), &[[0.5, -0.5]]);
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(bytes(vec![1, 2, 3, 4], "mp3").is_err());
    }
}
