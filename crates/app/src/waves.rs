//! Mini waveforms for the browser's Waveform column: each analysed track's
//! overview (see [`djsugar_engine::cache::overview`]), read on a background
//! thread and drawn once into a small image. Only the rows in view hold an
//! image, so a long list costs no more than a screenful; the list draws
//! them like album art, with no work of its own between changes.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Weak};

use djsugar_engine::cache::{self, OVERVIEW_SLICES};
use djsugar_engine::{OverviewSlice, Source, Track, Waveform};

/// The image's size in pixels: a pixel column per slice, and tall enough
/// for a row on a HiDPI screen. The list stretches it to the column.
const WIDTH: usize = OVERVIEW_SLICES;
const HEIGHT: usize = 44;

/// How the mini waveforms are colored (Settings › Appearance): 0 in
/// shades of lilac, 1 in the deck waveforms' full color.
pub const STYLE_KEY: &str = "list_wave_color";
pub const STYLES: [&str; 2] = ["Lilac shades", "Full color"];
/// The style chosen, read from the settings when first asked for.
static STYLE: AtomicU8 = AtomicU8::new(u8::MAX);
/// Set when the style changes, so the rows in view are drawn again.
static RESTYLED: AtomicBool = AtomicBool::new(false);

/// The style chosen: an index into [`STYLES`].
pub fn style() -> usize {
    match STYLE.load(Ordering::Relaxed) {
        u8::MAX => {
            let s = (crate::settings::number(STYLE_KEY, 0.0).max(0.0) as u8).min(1);
            STYLE.store(s, Ordering::Relaxed);
            s as usize
        }
        s => s as usize,
    }
}

/// Chooses a style and has the rows in view redrawn in it.
pub fn choose_style(index: usize) {
    let s = index.min(STYLES.len() - 1) as u8;
    crate::settings::set(STYLE_KEY, serde_json::json!(s));
    STYLE.store(s, Ordering::Relaxed);
    RESTYLED.store(true, Ordering::Relaxed);
}

/// True once after the style changed.
pub fn take_restyled() -> bool {
    RESTYLED.swap(false, Ordering::Relaxed)
}

pub struct Waves {
    requests: Sender<PathBuf>,
    found: Receiver<(PathBuf, Option<Vec<OverviewSlice>>)>,
    /// `None` while being read, or when the track isn't analysed.
    overviews: RefCell<HashMap<PathBuf, Option<Arc<[OverviewSlice]>>>>,
    /// The deck waveform each overview was last learnt from, so a deck
    /// still analysing (whose waveform grows every 200 ms) is followed to
    /// the end, and a finished one isn't summarised again every frame.
    learnt: RefCell<HashMap<PathBuf, Weak<Waveform>>>,
}

impl Waves {
    pub fn start() -> Self {
        let (requests, rx) = mpsc::channel::<PathBuf>();
        let (tx, found) = mpsc::channel();
        std::thread::Builder::new()
            .name("mini-waveforms".into())
            .spawn(move || {
                // Newest first: the rows asked for last are the ones in view.
                let mut queue = Vec::new();
                while let Ok(first) = rx.recv() {
                    queue.push(first);
                    while let Some(path) = queue.pop() {
                        queue.extend(rx.try_iter());
                        let overview = cache::overview(&path);
                        if tx.send((path, overview)).is_err() {
                            return;
                        }
                    }
                }
            })
            .expect("spawn mini waveform thread");
        Self {
            requests,
            found,
            overviews: RefCell::default(),
            learnt: RefCell::default(),
        }
    }

    /// The track's mini waveform if its overview has been read; otherwise
    /// it is read, and [`Waves::poll`] reports when it arrives.
    pub fn get(&self, track: &Track) -> Option<slint::Image> {
        let Source::File(path) = &track.source else {
            return None;
        };
        let mut overviews = self.overviews.borrow_mut();
        match overviews.get(path) {
            Some(o) => o.as_deref().map(|o| image(o, style() == 1)),
            None => {
                overviews.insert(path.clone(), None);
                let _ = self.requests.send(path.clone());
                None
            }
        }
    }

    /// Takes the overview of a track on a deck or just analysed in the
    /// background, so its row shows it without reading it back. A deck
    /// that is still analysing hands over a fuller waveform every 200 ms,
    /// and each is taken in turn, so the row fills in as the deck's does.
    /// True if the overview changed.
    pub fn learn(&self, track: &Track) -> bool {
        let Source::File(path) = &track.source else {
            return false;
        };
        if track.waveform.is_empty() {
            return false;
        }
        let mut learnt = self.learnt.borrow_mut();
        let seen = learnt
            .get(path)
            .is_some_and(|w| w.ptr_eq(&Arc::downgrade(&track.waveform)));
        if seen {
            return false;
        }
        learnt.insert(path.clone(), Arc::downgrade(&track.waveform));
        let overview = track.waveform.overview(OVERVIEW_SLICES);
        let mut overviews = self.overviews.borrow_mut();
        if overviews
            .get(path)
            .is_some_and(|o| o.as_deref() == Some(&overview[..]))
        {
            return false;
        }
        overviews.insert(path.clone(), Some(overview.into()));
        true
    }

    /// Forgets a track's overview, as its analysis was redone: it is read
    /// again when next asked for.
    pub fn forget(&self, path: &std::path::Path) {
        self.overviews.borrow_mut().remove(path);
        self.learnt.borrow_mut().remove(path);
    }

    /// Takes in overviews read since the last call; true if any arrived.
    pub fn poll(&mut self) -> bool {
        let mut any = false;
        while let Ok((path, overview)) = self.found.try_recv() {
            let Some(o) = overview else { continue };
            // Only while still being read: one learnt since then is newer.
            if let Some(slot @ None) = self.overviews.borrow_mut().get_mut(&path) {
                *slot = Some(o.into());
                any = true;
            }
        }
        any
    }
}

/// The lilac shades a slice is drawn in by its tone: deep where the bass
/// leads, the brand lilac (#B69CFF) for mids, pale where the highs lead,
/// as the deck waveforms color red, amber and blue.
const DEEP: [f32; 3] = [122.0, 92.0, 214.0]; // #7A5CD6
const LILAC: [f32; 3] = [182.0, 156.0, 255.0]; // #B69CFF
const PALE: [f32; 3] = [232.0, 224.0, 255.0]; // #E8E0FF

/// The deck waveforms' colors for the lows, mids and highs.
const LOW: [f32; 3] = [1.0, 0.22, 0.16];
const MID: [f32; 3] = [1.0, 0.72, 0.12];
const HIGH: [f32; 3] = [0.30, 0.62, 1.0];

/// The full color for a slice, mixed as the deck waveforms mix it: each
/// band's color by its share of the power, brought up to full brightness.
fn full(mid: u8, high: u8) -> [f32; 3] {
    let (mid, high) = (mid as f32 / 255.0, high as f32 / 255.0);
    let low = (1.0 - mid - high).max(0.0);
    let c: [f32; 3] = std::array::from_fn(|i| low * LOW[i] + mid * MID[i] + high * HIGH[i]);
    let top = c[0].max(c[1]).max(c[2]).max(0.35);
    c.map(|v| (v / top).min(1.0) * 255.0)
}

/// The shade for a slice whose mids and highs have these shares of its
/// power (0 to 255): its tone runs from 0 (all bass) through a half (all
/// mids) to 1 (all highs).
fn shade(mid: u8, high: u8) -> [f32; 3] {
    let t = ((0.5 * mid as f32 + high as f32) / 255.0).clamp(0.0, 1.0);
    let (from, to, k) = if t < 0.5 {
        (DEEP, LILAC, t * 2.0)
    } else {
        (LILAC, PALE, (t - 0.5) * 2.0)
    };
    std::array::from_fn(|i| from[i] + (to[i] - from[i]) * k)
}

/// Draws an overview as a waveform mirrored about its middle, each column
/// in its lilac shade (or, `in_full`, its full color), a touch brighter
/// towards the centre. Rows that color it (the selection, missing
/// tracks) use it as a mask.
fn image(overview: &[OverviewSlice], in_full: bool) -> slint::Image {
    let mut pixels =
        slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(WIDTH as u32, HEIGHT as u32);
    let buf = pixels.make_mut_slice();
    let half = HEIGHT as f32 / 2.0;
    for x in 0..WIDTH {
        // One solid shape, sized as on the deck waveforms: the body (RMS)
        // doubled, never past the peaks, on the same gentle curve. Slices
        // hold square roots of the levels, so they're squared first;
        // doubling the stored value instead would fill loud tracks to the
        // edges as flat blocks.
        let [peak, rms, mid, high] = overview[x * overview.len() / WIDTH];
        let color = if in_full {
            full(mid, high)
        } else {
            shade(mid, high)
        };
        let linear = |b: u8| (b as f32 / 255.0).powi(2);
        let body = (2.0 * linear(rms)).min(linear(peak));
        let rms = body.powf(0.8) * 0.94 * half;
        for y in 0..HEIGHT {
            // How far this pixel's middle is from the centre line, and how
            // much of it each part covers, for soft edges.
            let d = (y as f32 + 0.5 - half).abs() - 0.5;
            let cover = |reach: f32| (reach - d).clamp(0.0, 1.0);
            let a = cover(rms.max(0.5));
            let lift = 0.88 + 0.12 * (1.0 - d / rms.max(1.0)).clamp(0.0, 1.0);
            let c = |i: usize| (color[i] * lift).round().clamp(0.0, 255.0) as u8;
            buf[y * WIDTH + x] = slint::Rgba8Pixel {
                r: c(0),
                g: c(1),
                b: c(2),
                a: (a * 255.0).round() as u8,
            };
        }
    }
    slint::Image::from_rgba8(pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_a_deck_analysing_the_track() {
        let waves = Waves::start();
        let mut t = (*djsugar_engine::sample_library()[0]).clone();
        t.source = Source::File("/music/a.wav".into());
        // Not analysed: nothing to show, and nothing learnt.
        assert!(!waves.learn(&t));
        // Halfway through: what is analysed so far, the rest silent.
        let mut bins = vec![[0u8; 8]; 1000];
        bins[..500].fill([200; 8]);
        t.waveform = Arc::new(Waveform::from_finest(1000.0, bins.clone()));
        assert!(waves.learn(&t));
        assert!(!waves.learn(&t), "the same waveform isn't taken twice");
        let half = waves.overviews.borrow()[&PathBuf::from("/music/a.wav")].clone();
        assert_eq!(half.unwrap()[OVERVIEW_SLICES - 1][..2], [0, 0]);
        // Finished: the row shows the whole track.
        bins.fill([200; 8]);
        t.waveform = Arc::new(Waveform::from_finest(1000.0, bins));
        assert!(waves.learn(&t));
        assert!(waves.get(&t).is_some());
        let full = waves.overviews.borrow()[&PathBuf::from("/music/a.wav")].clone();
        assert_eq!(full.unwrap()[OVERVIEW_SLICES - 1][..2], [200, 200]);
    }

    #[test]
    fn draws_the_body_about_the_middle() {
        let quiet = [[0u8, 0, 255, 0]; 4];
        let loud = [[255u8, 128, 255, 0]; 4];
        let alpha = |o: &[OverviewSlice], y: usize| {
            let img = image(o, false);
            let buf = img.to_rgba8().unwrap();
            buf.as_slice()[y * WIDTH].a
        };
        // A silent track is a hairline; a loud one's body (twice its RMS)
        // is solid, with nothing drawn beyond it for the peaks.
        assert_eq!(alpha(&quiet, 0), 0);
        assert!(alpha(&quiet, HEIGHT / 2) > 0);
        assert_eq!(alpha(&loud, HEIGHT / 2), 255);
        assert_eq!(alpha(&loud, 0), 0);
        assert_eq!(alpha(&loud, 1), alpha(&loud, HEIGHT - 2));
        // Even a track as loud as can be keeps a margin: no flat blocks.
        let loudest = [[255u8, 255, 255, 0]; 4];
        assert_eq!(alpha(&loudest, 0), 0);
        assert_eq!(alpha(&loudest, HEIGHT - 1), 0);
    }

    #[test]
    fn shades_follow_the_tone() {
        let color = |(mid, high): (u8, u8)| {
            let img = image(&[[255u8, 128, mid, high]; 4], false);
            let buf = img.to_rgba8().unwrap();
            let p = buf.as_slice()[HEIGHT / 2 * WIDTH];
            (p.r, p.g, p.b)
        };
        // Bass deep, mids the brand lilac, highs pale.
        let (bass, mids, highs) = (color((0, 0)), color((255, 0)), color((0, 255)));
        assert!(
            bass.0 < mids.0 && mids.0 < highs.0,
            "{bass:?} {mids:?} {highs:?}"
        );
        assert!(
            (mids.0 as i32 - 182).abs() <= 2 && mids.2 >= 250,
            "{mids:?}"
        );
    }

    #[test]
    fn full_color_follows_the_bands() {
        let color = |(mid, high): (u8, u8)| {
            let img = image(&[[255u8, 128, mid, high]; 4], true);
            let buf = img.to_rgba8().unwrap();
            let p = buf.as_slice()[HEIGHT / 2 * WIDTH];
            (p.r, p.g, p.b)
        };
        // Bass red, mids amber, highs blue, as on the decks.
        let (bass, mids, highs) = (color((0, 0)), color((255, 0)), color((0, 255)));
        assert!(bass.0 > 200 && bass.1 < 80 && bass.2 < 80, "{bass:?}");
        assert!(mids.0 > 200 && mids.1 > 140 && mids.2 < 60, "{mids:?}");
        assert!(highs.2 > 200 && highs.0 < 100, "{highs:?}");
    }
}
