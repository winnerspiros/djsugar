//! The quality tag on the Player bar, as music players show theirs: the
//! loaded file's format with its bit depth and sample rate when lossless
//! ("FLAC 24-BIT 96 KHZ"), or its bitrate when not ("MP3 320 KBPS").

use djsugar_engine::tags;
use djsugar_engine::track::{Source, Track};

/// The tag for each deck's track, read from the file once per track.
#[derive(Default)]
pub struct Qualities {
    read: Vec<(usize, String, bool)>,
}

impl Qualities {
    /// The tag and whether the file is lossless; empty for a track that
    /// isn't a file on disk.
    pub fn get(&mut self, deck: usize, track: Option<&Track>) -> (String, bool) {
        let Some(t) = track else {
            return (String::new(), false);
        };
        if self.read.len() <= deck {
            self.read
                .resize(deck + 1, (usize::MAX, String::new(), false));
        }
        if self.read[deck].0 != t.id {
            let (tag, lossless) = match &t.source {
                Source::File(path) => describe(&tags::file_info(path)),
                _ => (String::new(), false),
            };
            self.read[deck] = (t.id, tag, lossless);
        }
        let (_, tag, lossless) = &self.read[deck];
        (tag.clone(), *lossless)
    }
}

/// The tag for a file's details.
pub fn describe(info: &tags::FileInfo) -> (String, bool) {
    let format = info.format.to_uppercase();
    if format.is_empty() {
        return (String::new(), false);
    }
    let lossless = djsugar_library::dupes::lossless(&format, info.bit_depth);
    let khz = |rate: u32| {
        let k = rate as f64 / 1000.0;
        if k.fract() == 0.0 {
            format!("{k:.0} KHZ")
        } else {
            format!("{k:.1} KHZ")
        }
    };
    let tag = if lossless {
        let mut parts = vec![format];
        parts.extend(info.bit_depth.map(|b| format!("{b}-BIT")));
        parts.extend(info.sample_rate.map(khz));
        parts.join(" ")
    } else {
        match info.bitrate {
            Some(kbps) => format!("{format} {kbps} KBPS"),
            None => format,
        }
    };
    (tag, lossless)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(
        format: &str,
        bitrate: Option<u32>,
        rate: Option<u32>,
        depth: Option<u8>,
    ) -> tags::FileInfo {
        tags::FileInfo {
            format: format.into(),
            bitrate,
            sample_rate: rate,
            bit_depth: depth,
            ..Default::default()
        }
    }

    #[test]
    fn lossless_shows_depth_and_rate() {
        let (tag, lossless) = describe(&info("flac", Some(900), Some(44_100), Some(16)));
        assert_eq!(tag, "FLAC 16-BIT 44.1 KHZ");
        assert!(lossless);
        let (tag, _) = describe(&info("WAV", None, Some(96_000), Some(24)));
        assert_eq!(tag, "WAV 24-BIT 96 KHZ");
    }

    #[test]
    fn lossy_shows_bitrate() {
        let (tag, lossless) = describe(&info("MP3", Some(320), Some(44_100), None));
        assert_eq!(tag, "MP3 320 KBPS");
        assert!(!lossless);
        assert_eq!(describe(&info("", None, None, None)).0, "");
    }
}
