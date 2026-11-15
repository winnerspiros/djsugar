//! Album art thumbnails for the browser's first column. Covers are found
//! the same way the decks find them (see [`djsugar_engine::cover`]) on a
//! background thread, and kept once per album folder, so a library of
//! whole albums decodes each cover once.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use djsugar_engine::cover::{self, Cover};
use djsugar_engine::{Source, Track};

/// Longest side of a thumbnail: twice the column, for HiDPI screens.
const THUMB_SIZE: u32 = 56;

pub struct Art {
    /// Longest side of the images, in pixels.
    size: u32,
    requests: Sender<(String, PathBuf, String)>,
    found: Receiver<(String, Option<Cover>)>,
    /// `None` while being looked for or when the track has none.
    images: RefCell<HashMap<String, Option<slint::Image>>>,
}

impl Art {
    pub fn start() -> Self {
        Self::sized(THUMB_SIZE)
    }

    /// Covers at most `size` pixels on their longest side, such as the
    /// Albums source's covers.
    pub fn sized(size: u32) -> Self {
        let (requests, rx) = mpsc::channel::<(String, PathBuf, String)>();
        let (tx, found) = mpsc::channel();
        std::thread::Builder::new()
            .name("album-art".into())
            .spawn(move || {
                for (key, path, album) in rx {
                    let cover = cover::find_sized(&path, &album, size);
                    if tx.send((key, cover)).is_err() {
                        break;
                    }
                }
            })
            .expect("spawn album art thread");
        Self {
            size,
            requests,
            found,
            images: RefCell::default(),
        }
    }

    /// The key a track's art is kept under: its folder and album, or the
    /// file itself when it has no album.
    pub fn key(track: &Track) -> Option<String> {
        let Source::File(path) = &track.source else {
            return None;
        };
        Some(if track.album.is_empty() {
            path.to_string_lossy().into_owned()
        } else {
            format!(
                "{}\u{0}{}",
                path.parent().unwrap_or(Path::new("")).to_string_lossy(),
                track.album
            )
        })
    }

    /// The track's thumbnail if it has been found; otherwise it is looked
    /// for, and [`Art::poll`] reports when it arrives.
    pub fn get(&self, track: &Track) -> Option<slint::Image> {
        let key = Self::key(track)?;
        let mut images = self.images.borrow_mut();
        if let Some(image) = images.get(&key) {
            return image.clone();
        }
        if let Source::File(path) = &track.source {
            images.insert(key.clone(), None);
            let _ = self.requests.send((key, path.clone(), track.album.clone()));
        }
        None
    }

    /// Shows `data`, an image file's bytes, as the track's thumbnail from
    /// now on (after its cover changed).
    pub fn set(&self, track: &Track, data: &[u8]) {
        if let Some(key) = Self::key(track) {
            let image = cover::decode(data, self.size).map(|c| image(&c));
            self.images.borrow_mut().insert(key, image);
        }
    }

    /// Takes in covers found since the last call; true if any arrived.
    pub fn poll(&mut self) -> bool {
        let mut any = false;
        while let Ok((key, cover)) = self.found.try_recv() {
            let Some(c) = cover else { continue };
            // Only while still looked for: a cover set since then is newer.
            if let Some(slot @ None) = self.images.borrow_mut().get_mut(&key) {
                *slot = Some(image(&c));
                any = true;
            }
        }
        any
    }
}

fn image(c: &Cover) -> slint::Image {
    let pixels =
        slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&c.rgba, c.width, c.height);
    slint::Image::from_rgba8(pixels)
}
