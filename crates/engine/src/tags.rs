//! Reading and writing the tags the track info panel edits, the front
//! cover, and what the file itself is (format, bitrate, sample rate).
//!
//! Tags are written through lofty into the file's main tag (ID3v2 for MP3,
//! Vorbis comments for FLAC and Ogg, iTunes atoms for M4A), creating one if
//! the file has none. The rating goes in as a popularimeter, MusicBee's
//! scale, which rekordbox, VirtualDJ and Mixxx read too.

use std::path::Path;

use lofty::config::WriteOptions;
use lofty::picture::{Picture, PictureType};
use lofty::prelude::*;
use lofty::tag::items::popularimeter::{Popularimeter, StarRating};
use lofty::tag::Tag;

/// The tags the info panel shows and edits.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TagFields {
    pub title: String,
    pub artist: String,
    /// The remixer or mix name, e.g. "Club Mix".
    pub remix: String,
    pub album: String,
    /// A year, or a full date as the file has it.
    pub year: String,
    pub genre: String,
    /// 0 for none.
    pub track_number: u32,
    pub grouping: String,
    /// 0 for none.
    pub bpm: f64,
    pub key: String,
    /// Stars, 0 for none.
    pub rating: u8,
    pub comment: String,
}

/// What the file is, for the info panel.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileInfo {
    /// The container or codec, e.g. "MP3", "FLAC".
    pub format: String,
    /// Kilobits per second of audio, when known.
    pub bitrate: Option<u32>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u8>,
    pub channels: Option<u8>,
    /// Bytes on disk.
    pub size: u64,
}

fn text(tag: &Tag, key: ItemKey) -> String {
    tag.get_string(key).unwrap_or_default().trim().to_string()
}

/// The first number in "3/12" or "03".
fn leading_number(s: &str) -> u32 {
    s.trim()
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

/// Reads the tags the info panel edits from `tag`.
pub fn fields_of(tag: &Tag) -> TagFields {
    let bpm = tag
        .get_string(ItemKey::Bpm)
        .or_else(|| tag.get_string(ItemKey::IntegerBpm))
        .and_then(|b| b.trim().replace(',', ".").parse::<f64>().ok())
        .filter(|b| (20.0..=400.0).contains(b))
        .unwrap_or(0.0);
    let year = [ItemKey::Year, ItemKey::RecordingDate, ItemKey::ReleaseDate]
        .into_iter()
        .map(|k| text(tag, k))
        .find(|y| !y.is_empty())
        .unwrap_or_default();
    let comment = [ItemKey::Comment, ItemKey::Description]
        .into_iter()
        .map(|k| text(tag, k))
        .find(|c| !c.is_empty())
        .unwrap_or_default();
    TagFields {
        title: text(tag, ItemKey::TrackTitle),
        artist: text(tag, ItemKey::TrackArtist),
        remix: text(tag, ItemKey::Remixer),
        album: text(tag, ItemKey::AlbumTitle),
        year,
        genre: text(tag, ItemKey::Genre),
        track_number: leading_number(&text(tag, ItemKey::TrackNumber)),
        grouping: text(tag, ItemKey::ContentGroup),
        bpm,
        key: text(tag, ItemKey::InitialKey),
        rating: tag.ratings().next().map_or(0, |r| r.rating as u8),
        comment,
    }
}

/// The tags of the file at `path`, or None when it can't be read.
pub fn read(path: &Path) -> Option<TagFields> {
    let file = lofty::read_from_path(path).ok()?;
    Some(
        file.primary_tag()
            .or_else(|| file.first_tag())
            .map(fields_of)
            .unwrap_or_default(),
    )
}

fn set(tag: &mut Tag, key: ItemKey, value: &str) {
    tag.remove_key(key);
    let value = value.trim();
    if !value.is_empty() {
        tag.insert_text(key, value.to_string());
    }
}

fn star(rating: u8) -> Option<StarRating> {
    Some(match rating {
        1 => StarRating::One,
        2 => StarRating::Two,
        3 => StarRating::Three,
        4 => StarRating::Four,
        5 => StarRating::Five,
        _ => return None,
    })
}

/// Opens `path` and hands `change` its main tag, made if it has none, then
/// saves the file.
fn edit(path: &Path, change: impl FnOnce(&mut Tag)) -> Result<(), String> {
    let mut file = lofty::read_from_path(path).map_err(|e| e.to_string())?;
    let tag_type = file.primary_tag_type();
    if file.tag(tag_type).is_none() {
        file.insert_tag(Tag::new(tag_type));
    }
    let tag = file.tag_mut(tag_type).ok_or("the file can't hold tags")?;
    change(tag);
    let cached = crate::cache::before_retag(path);
    let saved = file
        .save_to_path(path, WriteOptions::default())
        .map_err(|e| e.to_string());
    crate::cache::tags_rewritten(path, cached);
    saved
}

/// Writes `fields` into the file at `path`, replacing what its tags had
/// for each of them. Empty fields are taken out.
pub fn write(path: &Path, fields: &TagFields) -> Result<(), String> {
    edit(path, |tag| {
        set(tag, ItemKey::TrackTitle, &fields.title);
        set(tag, ItemKey::TrackArtist, &fields.artist);
        set(tag, ItemKey::Remixer, &fields.remix);
        set(tag, ItemKey::AlbumTitle, &fields.album);
        tag.remove_key(ItemKey::Year);
        set(tag, ItemKey::RecordingDate, &fields.year);
        set(tag, ItemKey::Genre, &fields.genre);
        let number = if fields.track_number > 0 {
            fields.track_number.to_string()
        } else {
            String::new()
        };
        set(tag, ItemKey::TrackNumber, &number);
        set(tag, ItemKey::ContentGroup, &fields.grouping);
        tag.remove_key(ItemKey::IntegerBpm);
        tag.remove_key(ItemKey::Bpm);
        if fields.bpm > 0.0 {
            // Whole tempos as integers, as most apps write them.
            let exact = if fields.bpm.fract() == 0.0 {
                format!("{}", fields.bpm as u32)
            } else {
                format!("{:.2}", fields.bpm)
            };
            let whole = format!("{}", fields.bpm.round() as u32);
            // ID3v2's TBPM is text, so it keeps the decimals; MP4's tmpo is
            // a whole number, with the exact tempo in a BPM atom beside it.
            if tag.tag_type() == lofty::tag::TagType::Id3v2 {
                tag.insert_text(ItemKey::IntegerBpm, exact);
            } else {
                tag.insert_text(ItemKey::IntegerBpm, whole);
                tag.insert_text(ItemKey::Bpm, exact);
            }
        }
        set(tag, ItemKey::InitialKey, &fields.key);
        tag.remove_key(ItemKey::Popularimeter);
        if let Some(stars) = star(fields.rating) {
            tag.insert_text(
                ItemKey::Popularimeter,
                Popularimeter::musicbee(stars, 0).to_string(),
            );
        }
        tag.remove_key(ItemKey::Description);
        set(tag, ItemKey::Comment, &fields.comment);
    })
}

/// The full-size picture embedded in the file's tags: the front cover if
/// there are several.
pub fn embedded_cover(path: &Path) -> Option<Vec<u8>> {
    let file = lofty::read_from_path(path).ok()?;
    let pictures: Vec<_> = file.tags().iter().flat_map(|t| t.pictures()).collect();
    pictures
        .iter()
        .find(|p| p.pic_type() == PictureType::CoverFront)
        .or(pictures.first())
        .map(|p| p.data().to_vec())
}

/// Embeds `image` (JPEG or PNG data) as the file's front cover, replacing
/// the one it had.
pub fn write_cover(path: &Path, image: &[u8]) -> Result<(), String> {
    let mut picture = Picture::from_reader(&mut &image[..]).map_err(|e| e.to_string())?;
    picture.set_pic_type(PictureType::CoverFront);
    edit(path, |tag| {
        tag.remove_picture_type(PictureType::CoverFront);
        // A lone picture of another type was the cover too.
        if tag.picture_count() == 1 && tag.pictures()[0].pic_type() == PictureType::Other {
            tag.remove_picture(0);
        }
        tag.push_picture(picture);
    })
}

/// The format, bitrate and size of the file at `path`.
pub fn file_info(path: &Path) -> FileInfo {
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let format = path
        .extension()
        .map(|e| e.to_string_lossy().to_uppercase())
        .unwrap_or_default();
    let Ok(file) = lofty::read_from_path(path) else {
        return FileInfo {
            format,
            size,
            ..Default::default()
        };
    };
    let p = file.properties();
    FileInfo {
        format,
        bitrate: p.audio_bitrate().or(p.overall_bitrate()).filter(|&b| b > 0),
        sample_rate: p.sample_rate(),
        bit_depth: p.bit_depth(),
        channels: p.channels(),
        size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_and_reads_back_every_field() {
        let dir = std::env::temp_dir().join(format!("djsugar-tags-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("song.wav");
        crate::decode::tests::write_wav(&path, 44_100, 2, &vec![0; 4_410]);
        let fields = TagFields {
            title: "Home".into(),
            artist: "Maribou State".into(),
            remix: "Club Mix".into(),
            album: "Portraits".into(),
            year: "2015".into(),
            genre: "Electronic".into(),
            track_number: 3,
            grouping: "11 YOGA".into(),
            bpm: 100.0,
            key: "10B".into(),
            rating: 4,
            comment: "warm-up".into(),
        };
        write(&path, &fields).unwrap();
        assert_eq!(read(&path).unwrap(), fields);

        // Clearing a field takes it out.
        let cleared = TagFields {
            remix: String::new(),
            rating: 0,
            ..fields
        };
        write(&path, &cleared).unwrap();
        assert_eq!(read(&path).unwrap(), cleared);

        let info = file_info(&path);
        assert_eq!(info.format, "WAV");
        assert_eq!(info.sample_rate, Some(44_100));
        assert_eq!(info.channels, Some(2));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn embeds_a_cover() {
        let dir = std::env::temp_dir().join(format!("djsugar-tagcover-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("song.wav");
        crate::decode::tests::write_wav(&path, 44_100, 2, &vec![0; 4_410]);
        assert!(embedded_cover(&path).is_none());
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(8, 8, image::Rgba([200, 150, 255, 255]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        write_cover(&path, &png).unwrap();
        assert_eq!(embedded_cover(&path).unwrap(), png);
        // Saving again replaces it rather than adding another.
        write_cover(&path, &png).unwrap();
        let file = lofty::read_from_path(&path).unwrap();
        let count: usize = file.tags().iter().map(|t| t.pictures().len()).sum();
        assert_eq!(count, 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reads_track_numbers() {
        assert_eq!(leading_number("3/12"), 3);
        assert_eq!(leading_number("07"), 7);
        assert_eq!(leading_number(""), 0);
    }
}
