//! Album art, found as: a picture embedded in the file's tags (the front cover if there are several),
//! otherwise an image next to the file. With one image in the folder that one is used; with several, one
//! named after the track, its file, its album, or "cover", "front", "album" or "folder", in that order.

use std::path::Path;

use lofty::picture::PictureType;
use lofty::prelude::*;

/// Longest side of a decoded cover, in pixels: enough for the deck artwork on
/// a HiDPI screen, small enough to keep in memory for every loaded track.
pub const COVER_SIZE: u32 = 320;

/// A decoded cover image.
pub struct Cover {
    pub width: u32,
    pub height: u32,
    /// Pixels in RGBA order, row by row.
    pub rgba: Vec<u8>,
}

impl std::fmt::Debug for Cover {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Cover({}x{})", self.width, self.height)
    }
}

/// The album art for the audio file at `path`, if it has any.
pub fn find(path: &Path, album: &str) -> Option<Cover> {
    find_sized(path, album, COVER_SIZE)
}

/// Like [`find`], scaled down to at most `size` pixels on its longest side,
/// e.g. for the browser's thumbnails.
pub fn find_sized(path: &Path, album: &str, size: u32) -> Option<Cover> {
    embedded(path)
        .and_then(|data| decode(&data, size))
        .or_else(|| decode(&std::fs::read(beside(path, album)?).ok()?, size))
}

/// Whether the file at `path` has album art (in its tags or its folder),
/// without decoding it.
pub fn has_cover(path: &Path, album: &str) -> bool {
    embedded(path).is_some() || beside(path, album).is_some()
}

fn embedded(path: &Path) -> Option<Vec<u8>> {
    let file = lofty::read_from_path(path).ok()?;
    let pictures: Vec<_> = file.tags().iter().flat_map(|t| t.pictures()).collect();
    pictures.iter().find(|p| p.pic_type() == PictureType::CoverFront).or(pictures.first()).map(|p| p.data().to_vec())
}

/// An image file in the same folder that belongs with the track.
pub fn beside(path: &Path, album: &str) -> Option<std::path::PathBuf> {
    let images: Vec<_> = std::fs::read_dir(path.parent()?)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| ["jpg", "jpeg", "png"].contains(&e.to_ascii_lowercase().as_str()))
        })
        .collect();
    if images.len() <= 1 {
        return images.into_iter().next();
    }
    let stem = |p: &Path| p.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    let file_name = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    let wanted =
        [stem(path), file_name, album.to_lowercase(), "cover".into(), "front".into(), "album".into(), "folder".into()];
    wanted.iter().filter(|w| !w.is_empty()).find_map(|w| images.iter().find(|i| stem(i) == *w)).cloned()
}

/// Decodes an image file's bytes into a cover of at most `size` pixels on
/// its longest side.
pub fn decode(data: &[u8], size: u32) -> Option<Cover> {
    let img = image::load_from_memory(data).ok()?;
    let img = if img.width().max(img.height()) > size { img.thumbnail(size, size) } else { img };
    let rgba = img.to_rgba8();
    Some(Cover { width: rgba.width(), height: rgba.height(), rgba: rgba.into_raw() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn png(path: &Path, w: u32, h: u32, color: [u8; 4]) {
        image::RgbaImage::from_pixel(w, h, image::Rgba(color)).save(path).unwrap();
    }

    #[test]
    fn finds_folder_art() {
        let dir = std::env::temp_dir().join(format!("djsugar-cover-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let track = dir.join("01 Opening.wav");
        crate::decode::tests::write_wav(&track, 44_100, 1, &[0; 100]);
        assert!(find(&track, "Night Drive").is_none());

        // The only image in the folder is used, whatever its name, and
        // shrunk to COVER_SIZE.
        png(&dir.join("scan.png"), 1000, 500, [255, 0, 0, 255]);
        let c = find(&track, "Night Drive").unwrap();
        assert_eq!((c.width, c.height), (COVER_SIZE, COVER_SIZE / 2));
        assert_eq!(&c.rgba[..4], &[255, 0, 0, 255]);

        // With several, the best-named one wins.
        png(&dir.join("Folder.png"), 8, 8, [0, 0, 255, 255]);
        assert_eq!(&find(&track, "Night Drive").unwrap().rgba[..4], &[0, 0, 255, 255]);
        png(&dir.join("night drive.png"), 8, 8, [0, 255, 0, 255]);
        assert_eq!(&find(&track, "Night Drive").unwrap().rgba[..4], &[0, 255, 0, 255]);
        png(&dir.join("01 Opening.png"), 8, 8, [9, 9, 9, 255]);
        assert_eq!(&find(&track, "Night Drive").unwrap().rgba[..4], &[9, 9, 9, 255]);
        fs::remove_dir_all(&dir).unwrap();
    }
}
