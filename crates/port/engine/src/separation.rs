//! Stem separation inside DJ Sugar (`crates/separation`), without Python: Spleeter whenever a track loads, so its stems are there within a second or two, starting where
//! the DJ will play from; HTDemucs when the DJ asks for high-quality stems, kept in the cache. The model files are downloaded to [`models_dir`] the first time they are
//! needed, through the app's [`set_fetch`]; without them a track simply has no stems on load, and high-quality stems fall back to the Demucs command if it is installed.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use djsugar_separation::spleeter;

use crate::dsp::Lr4;
use crate::stems::{
    default_info, split, split_drums, to_i16, Quality, StemFrame, StemSource, Stems, CHUNK, KICK_SPLIT_HZ, MAX_STEMS,
};
use crate::track::Audio;

/// The Spleeter weights (`scripts/models/convert_spleeter.py`).
pub const SPLEETER: &str = "spleeter-4stems.safetensors";
/// The HTDemucs weights.
pub const HTDEMUCS: &str = "htdemucs.safetensors";

/// Each model file and its SHA-256, as the `stem-models` release of DJ Sugar
/// has them (`.github/workflows/stem-models.yml`).
pub const MODELS: [(&str, &str); 2] = [
    (SPLEETER, "4f3c1011feda8f2cccaef9735fad04f272c87bd5ad0a93dd53a3d622c4ed31c5"),
    (HTDEMUCS, "8193504cdfb3943adaf039b8acb524a46e87ebf232c383ac7a32c80a6578423e"),
];

/// Downloads a model file: `(name, sha256, to)`. The app sets it, so the
/// engine itself never goes online.
pub type Fetch = dyn Fn(&str, &str, &Path) -> Result<(), String> + Send + Sync;

static FETCH: OnceLock<Box<Fetch>> = OnceLock::new();

/// Sets how missing model files are downloaded.
pub fn set_fetch(fetch: Box<Fetch>) {
    let _ = FETCH.set(fetch);
}

/// The model file `name`, downloaded first if it isn't yet (one download
/// at a time), checked against its SHA-256 in `models`.
pub(crate) fn model_or_fetch(name: &str, models: &[(&str, &str)]) -> Result<PathBuf, String> {
    static DOWNLOADING: Mutex<()> = Mutex::new(());
    let _turn = DOWNLOADING.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(p) = model(name) {
        return Ok(p);
    }
    let fetch = FETCH.get().ok_or("the models aren't downloaded")?;
    let (_, sha) = models.iter().find(|(n, _)| *n == name).ok_or("unknown model")?;
    let dir = models_dir().ok_or("no folder for the models")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(name);
    let part = path.with_extension("part");
    let result = fetch(name, sha, &part).and_then(|()| std::fs::rename(&part, &path).map_err(|e| e.to_string()));
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result.map(|()| path).map_err(|e| format!("couldn't download {name}: {e}"))
}

/// Has the model that splits tracks on hand, downloading it if it isn't yet.
pub fn fetch() -> Result<(), String> {
    model_or_fetch(SPLEETER, &MODELS).map(drop)
}

/// Where the separation models are kept: `DJSUGAR_MODELS`, else `models`
/// in the cache folder (they can always be downloaded again).
pub fn models_dir() -> Option<PathBuf> {
    match std::env::var_os("DJSUGAR_MODELS") {
        Some(d) => Some(PathBuf::from(d)),
        None => crate::cache::models_dir(),
    }
}

/// The model file `name`, if it has been downloaded.
pub fn model(name: &str) -> Option<PathBuf> {
    let p = models_dir()?.join(name);
    p.is_file().then_some(p)
}

/// Whether loaded tracks are separated with Spleeter (Settings › Analysis ›
/// Stems on load); on unless the app turns it off.
static ON_LOAD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// The most bytes of high-quality stems kept in the cache; 0 keeps all.
static CACHE_LIMIT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(DEFAULT_CACHE_LIMIT);

/// 10 GB: about 150 tracks of five minutes.
pub const DEFAULT_CACHE_LIMIT: u64 = 10 << 30;

/// Sets whether tracks loaded from now on get stems at once.
pub fn set_on_load(on: bool) {
    ON_LOAD.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// Whether tracks get stems when they load.
pub fn on_load() -> bool {
    ON_LOAD.load(std::sync::atomic::Ordering::Relaxed)
}

/// Sets the most bytes of high-quality stems kept (0 for no limit); the
/// cache is trimmed to it after each separation.
pub fn set_cache_limit(bytes: u64) {
    CACHE_LIMIT.store(bytes, std::sync::atomic::Ordering::Relaxed);
}

/// The most bytes of high-quality stems kept; 0 for no limit.
pub fn cache_limit() -> u64 {
    CACHE_LIMIT.load(std::sync::atomic::Ordering::Relaxed)
}

/// Trims the kept stems to [`cache_limit`], keeping those of `keep`.
pub fn trim_cache(keep: &[PathBuf]) -> usize {
    match cache_limit() {
        0 => 0,
        limit => crate::cache::trim_stems(limit, keep),
    }
}

/// Samples of the drums before a chunk that the kick/hi-hat split runs
/// over first, so its filters have settled where the chunk starts.
const SPLIT_LEAD: usize = 8192;

/// Spleeter's stems ([`spleeter::STEMS`]: vocals, drums, bass, other) for
/// each of ours (drums, bass, other, vocals).
const FROM_SPLEETER: [usize; 4] = [1, 2, 3, 0];

/// The Spleeter networks, loaded once.
pub(crate) fn spleeter_net() -> Result<Arc<spleeter::cpu::Net>, String> {
    static NET: Mutex<Option<Arc<spleeter::cpu::Net>>> = Mutex::new(None);
    let mut net = NET.lock().unwrap();
    if let Some(n) = net.as_ref() {
        return Ok(n.clone());
    }
    let path = model_or_fetch(SPLEETER, &MODELS)?;
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let n = Arc::new(spleeter::cpu::Net::load(&bytes)?);
    // Half the cores, so a playing deck keeps its share.
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    spleeter::cpu::set_threads((cores / 2).max(1));
    *net = Some(n.clone());
    Ok(n)
}

/// The track's channels, whole.
pub(crate) fn channels(audio: &Audio) -> [Vec<f32>; 2] {
    let mut l = Vec::with_capacity(audio.len());
    let mut r = Vec::with_capacity(audio.len());
    for f in audio.decoded_chunks().flatten() {
        l.push(f[0]);
        r.push(f[1]);
    }
    [l, r]
}

/// Separates the decoded `audio` with Spleeter and gives it the stems, which play at once: the stretch
/// around frame `from` is separated first, in about a second, and the original plays wherever the stems
/// aren't there yet. Does nothing if the audio has stems already. Stops when `keep_going` turns false.
pub fn separate_fast(audio: &Audio, from: usize, keep_going: impl Fn() -> bool) -> Result<(), String> {
    if audio.stems().is_some() || audio.is_empty() {
        return Ok(());
    }
    let net = spleeter_net()?;
    let rate = audio.sample_rate;
    let mut info = default_info();
    split_drums(&mut info);
    let stems = Arc::new(Stems::separated(info, rate, audio.len(), Quality::Fast));
    if !audio.set_stems(stems.clone()) {
        return Ok(());
    }
    let [l, r] = channels(audio);
    let model_rate = djsugar_separation::SAMPLE_RATE;
    // Our stems, with the drums split, for `n` frames from `lead` on.
    #[allow(clippy::needless_range_loop)] // frame by frame across the stems
    let frames = |parts: &[[&[f32]; 2]], lead: usize| -> Box<[StemFrame]> {
        let mut low = Lr4::lowpass(rate as f64, KICK_SPLIT_HZ);
        let n = parts[0][0].len();
        let mut out = Vec::with_capacity(n - lead);
        for i in 0..n {
            let at = |s: usize| [parts[FROM_SPLEETER[s]][0][i], parts[FROM_SPLEETER[s]][1][i]];
            let drums = at(0);
            let (k, h) = split(&mut low, drums);
            if i < lead {
                continue;
            }
            let mut f: StemFrame = [[0; 2]; MAX_STEMS];
            f[0] = k.map(to_i16);
            for s in 1..4 {
                f[s] = at(s).map(to_i16);
            }
            f[4] = h.map(to_i16);
            out.push(f);
        }
        out.into_boxed_slice()
    };
    if rate == model_rate {
        let complete = spleeter::separate_in_chunks(&*net, [&l, &r], from, CHUNK, SPLIT_LEAD, |k, parts| {
            let lead = (k * CHUNK).min(SPLIT_LEAD);
            stems.fill(k, frames(&parts, lead));
            keep_going()
        });
        if complete {
            stems.finish();
        }
        return Ok(());
    }
    // Another rate: separate at the model's and bring the stems back, all
    // at once.
    let resample = |x: &[f32], from_rate, to_rate| djsugar_separation::resample(x, from_rate, to_rate);
    let (l, r) = (resample(&l, rate, model_rate)?, resample(&r, rate, model_rate)?);
    let from = (from as f64 * model_rate as f64 / rate as f64) as usize;
    let separated = spleeter::separate(&*net, [&l, &r], from, |_, _| {});
    if !keep_going() {
        return Ok(());
    }
    let back: Vec<[Vec<f32>; 2]> = separated
        .iter()
        .map(|[sl, sr]| {
            let fit = |x: &[f32]| -> Result<Vec<f32>, String> {
                let mut y = resample(x, model_rate, rate)?;
                y.resize(audio.len(), 0.0);
                Ok(y)
            };
            Ok([fit(sl)?, fit(sr)?])
        })
        .collect::<Result<_, String>>()?;
    for (k, start) in (0..audio.len()).step_by(CHUNK).enumerate() {
        let end = (start + CHUNK).min(audio.len());
        let lead = start.min(SPLIT_LEAD);
        let parts: Vec<[&[f32]; 2]> =
            back.iter().map(|[sl, sr]| [&sl[start - lead..end], &sr[start - lead..end]]).collect();
        stems.fill(k, frames(&parts, lead));
    }
    stems.finish();
    Ok(())
}

/// Separates the file at `path` into high-quality stems with HTDemucs, leaving four FLAC files at
/// the track's own rate where [`crate::stems::find`] looks. Uses the GPU (Metal on macOS, Vulkan on
/// Linux). Without the model, or when it fails, the Demucs command does it if it is installed.
pub fn separate_hq(path: &Path) -> Result<StemSource, String> {
    let weights = match model_or_fetch(HTDEMUCS, &MODELS) {
        Ok(w) => w,
        Err(e) => return crate::stems::run_demucs(path).map_err(|_| e),
    };
    let result = std::panic::catch_unwind(|| htdemucs(path, &weights))
        .unwrap_or_else(|_| Err("the GPU separation crashed".into()));
    match result {
        Ok(s) => Ok(s),
        Err(e) => {
            eprintln!("HTDemucs: {e}; trying the Demucs command");
            crate::stems::run_demucs(path).map_err(|_| e)
        }
    }
}

fn htdemucs(path: &Path, weights: &Path) -> Result<StemSource, String> {
    use djsugar_separation::htdemucs::HtDemucs;
    let audio = crate::decode::decode(path)?;
    let rate = audio.sample_rate;
    let [l, r] = channels(&audio);
    let bytes = std::fs::read(weights).map_err(|e| format!("{}: {e}", weights.display()))?;
    let model = HtDemucs::load(&bytes, &djsugar_separation::gpu())?;
    let stems = model.separate([&l, &r])?;
    let model_rate = djsugar_separation::SAMPLE_RATE;
    let dir = crate::cache::stems_dir(path).ok_or("no cache folder")?;
    let work = dir.with_extension(format!("work{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let written = (|| -> Result<(), String> {
        // HTDemucs's order is ours: drums, bass, other, vocals.
        for (name, [sl, sr]) in djsugar_separation::htdemucs::STEMS.iter().zip(&stems) {
            let fit = |x: &[f32]| -> Result<Vec<f32>, String> {
                let mut y = djsugar_separation::resample(x, model_rate, rate)?;
                y.resize(audio.len(), 0.0);
                Ok(y)
            };
            let (sl, sr) = (fit(sl)?, fit(sr)?);
            crate::recorder::write_flac(&work.join(format!("{name}.flac")), rate, [&sl, &sr])
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_dir_all(&work);
        return Err(e);
    }
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::rename(&work, &dir).map_err(|e| e.to_string())?;
    crate::stems::find(path).ok_or_else(|| "the stems weren't written".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs the Spleeter weights: `DJSUGAR_MODELS=<folder with
    /// spleeter-4stems.safetensors> cargo test -p djsugar-engine -- --ignored spleeter_on`.
    #[test]
    #[ignore]
    fn spleeter_on_load_fills_the_stems_from_the_cue() {
        let audio = crate::synth::render_with(124.0, 0.25, 60.0, 5);
        let audio =
            Audio::from_frames(djsugar_separation::SAMPLE_RATE, audio.decoded_chunks().flatten().copied().collect());
        let t = std::time::Instant::now();
        separate_fast(&audio, 30 * 44_100, || true).unwrap();
        println!("60 s separated in {:.2} s", t.elapsed().as_secs_f64());
        let stems = audio.stems().unwrap();
        assert!(stems.is_complete());
        assert_eq!(stems.count(), 5);
        assert_eq!(stems.quality, Quality::Fast);
        // The stems add up to the track.
        let i = 20 * 44_100;
        let (orig, all) = (audio.frame(i), stems.mix(i, &[0.0; MAX_STEMS], || [0.0; 2]));
        assert!((orig[0] + all[0]).abs() < 2e-3, "{orig:?} {all:?}");
        // The synth's drums are mostly kick and hats: muting both takes out most of it.
        let rms = |g: [f32; MAX_STEMS]| {
            let e: f32 = (0..44_100 * 10).map(|k| stems.mix(i + k, &g, || audio.frame(i + k))[0].powi(2)).sum();
            e.sqrt()
        };
        let full = rms([1.0; MAX_STEMS]);
        let no_drums = rms([0.0, 1.0, 1.0, 1.0, 0.0]);
        println!("full {full:.1}, without kick and hats {no_drums:.1}");
        assert!(no_drums < full);
    }

    #[test]
    fn checksums_match_the_published_models() {
        let sums = include_str!("../../models/SHA256SUMS");
        for (name, sha) in MODELS {
            assert!(sums.lines().any(|l| l == format!("{sha}  {name}")), "{name}");
        }
    }

    #[test]
    fn spleeter_stems_map_onto_ours() {
        // Ours: drums, bass, other, vocals; Spleeter's: vocals, drums, bass, other.
        for (ours, theirs) in FROM_SPLEETER.iter().enumerate() {
            let name = spleeter::STEMS[*theirs];
            let want = ["drums", "bass", "other", "vocals"][ours];
            assert_eq!(name, want);
        }
    }
}
