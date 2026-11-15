//! Album art for the players: two thumbnails per picture, 80 and 240 pixels
//! square, the picture fitted and centred on black, baseline JPEG, the way
//! rekordbox makes them (as Bake'n Deck's `expressport` found, MIT licence,
//! Copyright (c) Masanari Higashi). rekordbox uses 4:2:0 chroma; `image`
//! writes 4:2:2, which any JPEG decoder reads too.

use std::path::Path;

use djsugar_engine::cover;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::{self, FilterType};
use image::{Rgb, RgbImage, RgbaImage};

const SMALL: u32 = 80;
const MEDIUM: u32 = 240;
const QUALITY: u8 = 85;

pub struct Thumbnails {
    pub small: Vec<u8>,
    pub medium: Vec<u8>,
}

/// The thumbnails of the track's album art (in its tags or its folder).
pub fn for_track(path: &Path, album: &str) -> Option<Thumbnails> {
    let c = cover::find_sized(path, album, MEDIUM)?;
    let rgba = RgbaImage::from_raw(c.width, c.height, c.rgba)?;
    let rgb = image::DynamicImage::ImageRgba8(rgba).to_rgb8();
    Some(Thumbnails {
        small: square(&rgb, SMALL)?,
        medium: square(&rgb, MEDIUM)?,
    })
}

fn square(image: &RgbImage, side: u32) -> Option<Vec<u8>> {
    let (w, h) = image.dimensions();
    if w == 0 || h == 0 {
        return None;
    }
    let scale = f64::from(side) / f64::from(w.max(h));
    let fit = |n: u32| ((f64::from(n) * scale).round() as u32).clamp(1, side);
    let scaled = imageops::resize(image, fit(w), fit(h), FilterType::Lanczos3);
    let mut canvas = RgbImage::from_pixel(side, side, Rgb([0, 0, 0]));
    let (x, y) = ((side - scaled.width()) / 2, (side - scaled.height()) / 2);
    imageops::overlay(&mut canvas, &scaled, i64::from(x), i64::from(y));
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, QUALITY)
        .encode_image(&canvas)
        .ok()?;
    Some(jpeg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wide_picture_is_letterboxed() {
        let img = RgbImage::from_pixel(200, 100, Rgb([200, 40, 120]));
        let jpeg = square(&img, SMALL).unwrap();
        let back = image::load_from_memory(&jpeg).unwrap().to_rgb8();
        assert_eq!(back.dimensions(), (SMALL, SMALL));
        assert!(back.get_pixel(40, 2)[0] < 30, "top band is black");
        assert!(back.get_pixel(40, 40)[0] > 150, "middle has the picture");
    }
}
