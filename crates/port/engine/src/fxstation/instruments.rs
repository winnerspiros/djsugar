//! X-Pad sounds: four banks of kick, snare, clap and hi-hat. Bank 1 is the app's
//! own kit, banks 2 to 4 come from folders the user fills. A loader thread decodes
//! them; the audio thread gets them through a queue and never locks or frees.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use djsugar_control::ControlHandle;

use crate::decode::decode;

pub const BANKS: usize = 4;
pub const INSTRUMENTS: usize = 4;
/// User sounds are cut after this long.
const MAX_USER_SECONDS: f64 = 4.0;
const NAME_PREFIXES: [&str; INSTRUMENTS] = ["kick", "snare", "clap", "hat"];
/// Bank 1, from the app's samples. The kick has a click: phone speakers drop
/// nearly all of a plain kick, which sits below 150 Hz.
pub const FACTORY_FILES: [&str; INSTRUMENTS] = ["hardstyle-kick.ogg", "snare.ogg", "clap.ogg", "hi-hat.ogg"];
const EXTENSIONS: [&str; 9] = ["wav", "flac", "mp3", "ogg", "opus", "m4a", "aac", "aif", "aiff"];

/// A sound of the X-Pad: interleaved stereo samples.
#[derive(Clone, Debug, Default)]
pub struct Instrument {
    pub samples: Vec<f32>,
    pub sample_rate: f32,
}

impl Instrument {
    pub fn frames(&self) -> usize {
        self.samples.len() / 2
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

/// A bank of the four instruments; a slot without a sound plays bank 1's.
#[derive(Clone, Debug, Default)]
pub struct Bank {
    pub instruments: [Option<Arc<Instrument>>; INSTRUMENTS],
}

type Update = (usize, Option<Arc<Bank>>);

/// The banks as the audio thread sees them.
pub struct Instruments {
    banks: [Option<Arc<Bank>>; BANKS],
    updates: rtrb::Consumer<Update>,
}

impl std::fmt::Debug for Instruments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Instruments").field("banks", &self.banks).finish_non_exhaustive()
    }
}

impl Instruments {
    /// The banks and the loader that fills them; nothing loads until
    /// [`Loader::start`].
    pub fn new(reload: ControlHandle, sounds: [ControlHandle; BANKS]) -> (Self, Loader) {
        let (tx, updates) = rtrb::RingBuffer::new(BANKS * 4);
        let shared = Arc::new(Shared {
            factory: Mutex::new([None, None, None, None]),
            wake: Mutex::new(true),
            cv: Condvar::new(),
        });
        let loader = Loader { shared, tx: Some(tx), reload, sounds };
        (Self { banks: [None, None, None, None], updates }, loader)
    }

    /// Takes the banks the loader published; once per audio block.
    pub fn poll(&mut self) {
        while let Ok((index, bank)) = self.updates.pop() {
            // The loader keeps every bank it published, so this drop never frees.
            self.banks[index] = bank;
        }
    }

    /// A bank's sound for an instrument, else bank 1's.
    pub fn sound(&self, bank: usize, instrument: usize) -> Option<&Arc<Instrument>> {
        let of = |b: usize| self.banks[b].as_ref().and_then(|b| b.instruments[instrument].as_ref());
        of(bank % BANKS).or_else(|| of(0))
    }
}

struct Shared {
    factory: Mutex<[Option<PathBuf>; INSTRUMENTS]>,
    wake: Mutex<bool>,
    cv: Condvar,
}

/// Loads the banks off the audio thread, at start, on `xpad_reload` and when
/// the app's own kit is given.
pub struct Loader {
    shared: Arc<Shared>,
    tx: Option<rtrb::Producer<Update>>,
    reload: ControlHandle,
    sounds: [ControlHandle; BANKS],
}

/// Hands the loader new sounds from other threads.
#[derive(Clone)]
pub struct LoaderHandle(Arc<Shared>);

impl LoaderHandle {
    /// The app's own kit for bank 1, in the order of [`FACTORY_FILES`].
    pub fn set_factory(&self, files: [Option<PathBuf>; INSTRUMENTS]) {
        if let Ok(mut f) = self.0.factory.lock() {
            *f = files;
        }
        if let Ok(mut w) = self.0.wake.lock() {
            *w = true;
        }
        self.0.cv.notify_one();
    }
}

impl Loader {
    pub fn handle(&self) -> LoaderHandle {
        LoaderHandle(self.shared.clone())
    }

    /// Runs the loader on its own thread, with the user banks under `root`.
    pub fn start(mut self, root: PathBuf) {
        let Some(mut tx) = self.tx.take() else { return };
        let spawned = std::thread::Builder::new().name("xpad-loader".into()).spawn(move || {
            let mut published: Vec<Arc<Bank>> = Vec::new();
            let mut seen = self.reload.version();
            loop {
                let wake = {
                    let Ok(w) = self.shared.wake.lock() else { return };
                    let Ok((mut w, _)) = self.shared.cv.wait_timeout(w, Duration::from_millis(250)) else { return };
                    std::mem::take(&mut *w)
                };
                let version = self.reload.version();
                let pressed = version != seen && self.reload.get() > 0.0;
                seen = version;
                if !(wake || pressed) {
                    continue;
                }
                let factory = self.shared.factory.lock().map(|f| f.clone()).unwrap_or_default();
                for index in 0..BANKS {
                    let bank = load_bank(index, &factory, &root);
                    let count = bank.instruments.iter().filter(|i| i.is_some()).count();
                    let bank = (count > 0).then(|| Arc::new(bank));
                    if let Some(b) = &bank {
                        published.push(b.clone());
                    }
                    let _ = tx.push((index, bank));
                    self.sounds[index].set(count as f64);
                }
            }
        });
        if let Err(e) = spawned {
            eprintln!("X-Pad: cannot start the sound loader: {e}");
        }
    }
}

/// The folder of a user bank (2 to 4).
pub fn bank_directory(root: &Path, index: usize) -> PathBuf {
    root.join(format!("Bank {}", index + 1))
}

fn load_bank(index: usize, factory: &[Option<PathBuf>; INSTRUMENTS], root: &Path) -> Bank {
    let chosen = if index == 0 { factory.clone() } else { folder_sounds(&bank_directory(root, index), root) };
    let mut bank = Bank::default();
    for (slot, path) in bank.instruments.iter_mut().zip(chosen) {
        *slot = path.and_then(|p| decode_sound(&p)).map(Arc::new);
    }
    bank
}

/// The sounds of a bank folder: by name ("kick.wav", "01_kick.wav"), else in
/// order. Creates the folder empty, so users find where their sounds go.
fn folder_sounds(dir: &Path, root: &Path) -> [Option<PathBuf>; INSTRUMENTS] {
    if !dir.exists() {
        let _ = std::fs::create_dir_all(dir);
    }
    // The sounds stay out of the phone's music apps.
    let no_media = root.join(".nomedia");
    if cfg!(target_os = "android") && !no_media.exists() {
        let _ = std::fs::write(&no_media, b"");
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|r| r.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.is_file() && supported(p)).collect())
        .unwrap_or_default();
    files.sort();
    let mut chosen: [Option<PathBuf>; INSTRUMENTS] = Default::default();
    for (slot, prefix) in chosen.iter_mut().zip(NAME_PREFIXES) {
        *slot = files.iter().find(|f| bare_name(f).starts_with(prefix)).cloned();
    }
    let mut next = 0;
    for i in 0..INSTRUMENTS {
        while chosen[i].is_none() && next < files.len() {
            if !chosen.contains(&Some(files[next].clone())) {
                chosen[i] = Some(files[next].clone());
            }
            next += 1;
        }
    }
    chosen
}

/// The file name in lower case without a leading number ("01_", "2 ", "3-").
fn bare_name(path: &Path) -> String {
    let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    let digits = name.bytes().take_while(u8::is_ascii_digit).count();
    match name.as_bytes().get(digits) {
        Some(b'_' | b' ' | b'-') if digits > 0 => name[digits + 1..].to_owned(),
        _ => name,
    }
}

fn supported(path: &Path) -> bool {
    path.extension().map(|e| e.to_string_lossy().to_lowercase()).is_some_and(|e| EXTENSIONS.contains(&e.as_str()))
}

/// Decodes a sound to stereo, at most [`MAX_USER_SECONDS`] long.
fn decode_sound(path: &Path) -> Option<Instrument> {
    let audio = decode(path).ok()?;
    let frames = audio.len().min((MAX_USER_SECONDS * f64::from(audio.sample_rate)) as usize);
    if frames == 0 {
        return None;
    }
    let mut samples = Vec::with_capacity(frames * 2);
    for f in 0..frames {
        samples.extend_from_slice(&audio.frame(f));
    }
    Some(Instrument { samples, sample_rate: audio.sample_rate as f32 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_with_or_without_a_number() {
        assert_eq!(bare_name(Path::new("/x/01_Kick.wav")), "kick.wav");
        assert_eq!(bare_name(Path::new("/x/2 snare.ogg")), "snare.ogg");
        assert_eq!(bare_name(Path::new("/x/808.wav")), "808.wav");
    }

    #[test]
    fn a_folder_fills_by_name_then_in_order() {
        let root = std::env::temp_dir().join(format!("xpad-test-{}", std::process::id()));
        let dir = bank_directory(&root, 1);
        std::fs::create_dir_all(&dir).unwrap();
        for n in ["a.wav", "01_snare.wav", "b.wav", "notes.txt"] {
            std::fs::write(dir.join(n), b"").unwrap();
        }
        let got = folder_sounds(&dir, &root);
        let name = |i: usize| got[i].as_ref().map(|p| p.file_name().unwrap().to_string_lossy().into_owned());
        assert_eq!(name(1).as_deref(), Some("01_snare.wav"));
        assert_eq!(name(0).as_deref(), Some("a.wav"));
        assert_eq!(name(2).as_deref(), Some("b.wav"));
        assert_eq!(name(3), None);
        let _ = std::fs::remove_dir_all(root);
    }
}
