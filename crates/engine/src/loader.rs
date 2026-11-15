//! Loads tracks off the audio thread, the way Mixxx does: a track goes on its
//! deck as soon as its file is opened, and plays while the rest decodes. The
//! waveform fills in as the audio is analysed, and the tempo and beat grid
//! follow when the analysis is done. Results are cached on disk, so a track
//! loaded again has its waveform and tempo at once.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::analysis::{analyse, detect_key, Analyser, Analysis, TempoHint};
use crate::beats::BeatGrid;
use crate::beats::GridEdit;
use crate::beatthis;
use crate::cache;
use crate::cover::{self, Cover};
use crate::decode::{decode, Decoder};
use crate::engine::{Command, EngineInput, Returned};
use crate::key;
use crate::phrases;
use crate::scan;
use crate::separation;
use crate::stems::{self, StemSource};
use crate::synth;
use crate::track::{Audio, LibraryInfo, Source, Track};
use crate::{LoadedTracks, NUM_DECKS};

/// How often a growing waveform is handed to the UI while analysing.
const WAVEFORM_UPDATE: Duration = Duration::from_millis(200);

/// Produces the playable copy of `track` in one go: audio, waveform, tempo
/// and first beat.
pub fn prepare(track: &Track) -> Result<Track, String> {
    let audio = match &track.source {
        Source::Synth { seed } => synth::render(track, *seed),
        Source::File(path) => decode(path)?,
    };
    let analysis = analyse(&audio, analysis_hint(track));
    if let Some(l) = analysis.loudness {
        audio.set_loudness(l);
    }
    let detected = key_is_detected(track);
    Ok(analysed(track, Arc::new(audio), analysis, detected))
}

/// The tempo analysis keeps for `track` (see [`crate::analysis::tag_tempo`]):
/// the library's, unless an earlier analysis found it. A tempo the analysis
/// got wrong would otherwise hold every later one to it, Beat This!'s too.
fn tempo_hint(track: &Track) -> Option<f64> {
    (track.bpm > 0.0 && !track.info.grid_analysed).then_some(track.bpm)
}

/// What analysis is told about `track`: the tempo it keeps and the genre
/// that picks the octave a detected tempo is read in.
fn analysis_hint(track: &Track) -> TempoHint {
    TempoHint {
        bpm: tempo_hint(track),
        genre: track.genre.clone(),
    }
}

/// Whether `track`'s key is for analysis to find: it has none, or an
/// earlier analysis found it. A key from tags or another app stays.
fn key_is_detected(track: &Track) -> bool {
    let Source::File(path) = &track.source else {
        return false;
    };
    if track.key.is_empty() {
        return true;
    }
    match track.info.key_detected {
        Some(d) => d,
        // Not recorded: a key the file's tags don't carry came from analysis.
        None => scan::read_key(path).is_empty(),
    }
}

/// `track` with its audio and analysis. Unless `detected` (see
/// [`key_is_detected`]), its own key wins, as the tempo from tags does.
fn analysed(track: &Track, audio: Arc<Audio>, analysis: Analysis, detected: bool) -> Track {
    let mut t = with_analysis(track, &analysis, detected);
    t.duration = audio.duration();
    t.waveform = Arc::new(analysis.waveform);
    t.audio = Some(audio);
    t
}

/// `track` with the tempo, grid and key `analysis` found, as [`analysed`]
/// gives them.
fn with_analysis(track: &Track, analysis: &Analysis, detected: bool) -> Track {
    let mut info = track.info.clone();
    let key = match analysis.key {
        Some(k) if detected => {
            info.key_detected = Some(true);
            k.camelot()
        }
        _ => {
            if !track.key.is_empty() && info.key_detected.is_none() {
                info.key_detected = Some(detected);
            }
            track.key.clone()
        }
    };
    let mut t = Track {
        key,
        info,
        ..track.clone()
    };
    with_grid(&mut t, track, analysis);
    t
}

/// Analyses `track` away from the decks (background analysis): its cached
/// analysis when there is one, else the file is decoded and analysed and
/// the result cached, as loading it on a deck would. A key found by
/// another detector is found again. With `fresh` (the DJ asked), the file
/// is analysed again whatever the cache holds, and a grid the library got
/// from an earlier analysis may be replaced; a grid the DJ edited or an
/// import brought, and a key from tags, an import or the DJ, still stay.
/// The track comes back with its tempo, grid, key and detected phrases
/// (unless it has imported ones), and without audio or waveform.
pub fn analyse_file(track: &Track, fresh: bool) -> Result<Track, String> {
    let Source::File(path) = &track.source else {
        return Err("not a file".into());
    };
    let detected = key_is_detected(track);
    let detector = key::detector();
    let mut base = track.clone();
    if fresh && base.info.grid_analysed && !base.info.grid_locked {
        base.info.fixed_grid = false;
        base.grid = BeatGrid::default();
    }
    // Afresh, the tempo is found without the library's as a hint.
    let hint = TempoHint {
        bpm: tempo_hint(track).filter(|_| !fresh),
        genre: track.genre.clone(),
    };
    let cached = if fresh { None } else { cache::load(path) };
    let mut audio = None;
    let mut analysis = match cached {
        Some(c) if !(detected && c.key_detector != detector) => c,
        cached => {
            let decoded = audio.insert(decode(path)?);
            match cached {
                Some(mut c) => {
                    c.key = detect_key(decoded, detector).or(c.key);
                    c.key_detector = detector;
                    c
                }
                None => analyse(decoded, hint.clone()),
            }
        }
    };
    let mut changed = audio.is_some();
    if beatthis::wanted(&analysis) && grid_replaceable(&base) {
        let decoded = match audio.take() {
            Some(a) => a,
            None => decode(path)?,
        };
        match beatthis::refine(&mut analysis, &decoded, hint) {
            Ok(()) => {
                changed = true;
                base.info.fixed_grid = false;
            }
            Err(e) => eprintln!("no beats from Beat This!: {e}"),
        }
    }
    if changed {
        cache::store(path, &analysis);
    }
    let mut t = with_analysis(&base, &analysis, detected);
    let finest = analysis.waveform.levels.first().map_or(0, Vec::len);
    if t.duration <= 0.0 && analysis.waveform.bins_per_second > 0.0 && finest > 0 {
        t.duration = finest as f64 / analysis.waveform.bins_per_second;
    }
    if !phrases::imported(&t) {
        t.waveform = Arc::new(analysis.waveform);
        let found = phrases::to_markers(&phrases::detect(&t), t.duration);
        t.info.markers = phrases::with_phrases(&t.info.markers, found);
        t.waveform = Arc::default();
    }
    Ok(t)
}

/// Whether a new analysis may replace `track`'s grid: it has none from the
/// library, or the one it has came from an earlier analysis (not the DJ or
/// an import), and the DJ hasn't locked it.
fn grid_replaceable(track: &Track) -> bool {
    !track.info.grid_locked && (!track.info.fixed_grid || track.info.grid_analysed)
}

/// Gives `t` the beat grid it should play with: a grid from the library is
/// kept as the DJ left it, the synthesiser knows exactly where its beats
/// are, and anything else takes the analysis.
fn with_grid(t: &mut Track, base: &Track, analysis: &Analysis) {
    if base.info.fixed_grid || base.info.grid_locked {
        return;
    }
    match base.source {
        Source::Synth { .. } => t.bpm = analysis.bpm,
        Source::File(_) if !analysis.grid.is_empty() => t.set_grid(analysis.grid.clone()),
        Source::File(_) => {
            t.bpm = analysis.bpm;
            t.first_beat = analysis.first_beat;
        }
    }
    t.info.grid_unsure = analysis.grid_unsure;
    t.info.grid_from = "analysis".into();
}

pub(crate) struct LoadRequest {
    pub deck: usize,
    pub track: Arc<Track>,
}

/// What the loading threads share.
struct Shared {
    input: Mutex<EngineInput>,
    loaded: LoadedTracks,
    /// Counts load requests per deck; a load that is no longer the latest stops.
    generation: [AtomicU64; NUM_DECKS],
    alerts: crate::Alerts,
}

/// Whether two copies of a track share their audio, i.e. are the same load.
fn same_load(a: &Track, b: &Track) -> bool {
    match (&a.audio, &b.audio) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        _ => false,
    }
}

impl Shared {
    fn send(&self, cmd: Command) {
        Self::send_locked(&mut self.input.lock().unwrap(), cmd, &self.loaded);
    }

    fn send_locked(input: &mut EngineInput, mut cmd: Command, loaded: &LoadedTracks) {
        // The queue only fills if the audio callback has stalled; wait it out.
        while let Err(c) = input.send(cmd) {
            cmd = c;
            std::thread::sleep(Duration::from_millis(5));
            Self::collect_locked(input, loaded);
        }
    }

    /// Frees tracks the engine is done with, and takes ejected tracks off the
    /// list the UI shows (unless a newer track has already replaced them).
    fn collect(&self) {
        Self::collect_locked(&mut self.input.lock().unwrap(), &self.loaded);
    }

    fn collect_locked(input: &mut EngineInput, loaded: &LoadedTracks) {
        for r in input.collect_garbage() {
            match r {
                Returned::Ejected { deck, track } => {
                    let mut l = loaded.write().unwrap();
                    if l[deck].as_ref().is_some_and(|t| same_load(t, &track)) {
                        l[deck] = None;
                    }
                }
                Returned::GridEdit {
                    deck,
                    track,
                    edit,
                    seconds,
                } => {
                    if let Some(t) = edit_grid(loaded, deck, &track, edit, seconds) {
                        Self::send_locked(input, Command::Regrid { deck, track: t }, loaded);
                    }
                }
                Returned::Done(_) => {}
            }
        }
    }

    /// Puts `track` on its deck, unless a newer load for the deck has started.
    fn put_on_deck(&self, deck: usize, generation: u64, track: Arc<Track>) -> bool {
        // Holding the engine's queue keeps loads reaching it in order. (Always
        // take the queue before the list of loaded tracks, never the reverse.)
        let mut input = self.input.lock().unwrap();
        if !self.is_latest(deck, generation) {
            return false;
        }
        self.loaded.write().unwrap()[deck] = Some(track.clone());
        Self::send_locked(&mut input, Command::Load { deck, track }, &self.loaded);
        true
    }

    /// Replaces the UI's copy of a track already on `deck` with a newer one
    /// (a fuller waveform, or the finished analysis). False if the deck has
    /// moved on.
    fn update(&self, deck: usize, track: &Arc<Track>) -> bool {
        let mut l = self.loaded.write().unwrap();
        match &l[deck] {
            Some(t) if same_load(t, track) => {
                // A cover found or changed (see `AudioEngine::set_cover`)
                // since `track` was made stays.
                l[deck] = Some(match &t.cover {
                    Some(c) if !track.cover.as_ref().is_some_and(|n| Arc::ptr_eq(c, n)) => {
                        Arc::new(Track {
                            cover: Some(c.clone()),
                            ..(**track).clone()
                        })
                    }
                    _ => track.clone(),
                });
                true
            }
            _ => false,
        }
    }

    /// Changes the UI's copy of the track loaded as `load` on `deck`, if it
    /// is still there.
    fn modify(&self, deck: usize, load: &Track, change: impl FnOnce(&mut Track)) {
        let mut l = self.loaded.write().unwrap();
        if let Some(t) = l[deck].as_ref().filter(|t| same_load(t, load)) {
            let mut t = (**t).clone();
            change(&mut t);
            l[deck] = Some(Arc::new(t));
        }
    }

    fn is_latest(&self, deck: usize, generation: u64) -> bool {
        self.generation[deck].load(Ordering::Acquire) == generation
    }
}

/// Grids replaced by edits, newest last, per deck, for undo. Cleared when
/// another track is loaded.
static UNDO: Mutex<[Vec<(usize, BeatGrid)>; NUM_DECKS]> =
    Mutex::new([const { Vec::new() }; NUM_DECKS]);

/// When each deck's grid was last dragged, so a drag is one undo step.
static LAST_DRAG: Mutex<[Option<Instant>; NUM_DECKS]> = Mutex::new([None; NUM_DECKS]);
/// A pause this long ends a drag.
const DRAG_PAUSE: Duration = Duration::from_millis(600);

/// Most edits kept for undo per deck.
const UNDO_DEPTH: usize = 64;

/// Applies a beat grid edit to the track loaded on `deck`, if it is still
/// `track`. The edited grid is the DJ's from now on: re-analysis keeps it.
/// A locked grid takes no edits but unlocking; resetting gives back the
/// grid the analysis found (from the cache).
fn edit_grid(
    loaded: &LoadedTracks,
    deck: usize,
    track: &Track,
    edit: GridEdit,
    seconds: f64,
) -> Option<Arc<Track>> {
    let mut l = loaded.write().unwrap();
    let current = l[deck].as_ref().filter(|t| same_load(t, track))?.clone();
    let mut t = (*current).clone();
    match edit {
        GridEdit::Lock(on) => {
            if on == current.info.grid_locked {
                return None;
            }
            t.info.grid_locked = on;
            let t = Arc::new(t);
            l[deck] = Some(t.clone());
            return Some(t);
        }
        // A locked grid stays as it is.
        _ if current.info.grid_locked => return None,
        GridEdit::Reset => {
            let Source::File(path) = &current.source else {
                return None;
            };
            let analysed = cache::load(path).filter(|a| !a.grid.is_empty())?;
            if current.info.grid_analysed && analysed.grid == current.grid {
                return None;
            }
            t.set_grid(analysed.grid);
            t.info.fixed_grid = false;
            t.info.grid_analysed = true;
            t.info.grid_from = "analysis".into();
            t.info.grid_unsure = analysed.grid_unsure;
            let t = Arc::new(t);
            l[deck] = Some(t.clone());
            return Some(t);
        }
        _ => {}
    }
    let mut undo = UNDO.lock().unwrap();
    let history = &mut undo[deck];
    history.retain(|(id, _)| *id == current.id);
    let grid = match edit {
        GridEdit::Undo => history.pop()?.1,
        _ => {
            let before = current.beat_grid();
            if before.is_empty() {
                return None;
            }
            let after = before.edited(edit, seconds);
            if after == before {
                return None;
            }
            // A drag moves the grid a little many times: the moves that
            // follow one another closely are one step to undo.
            let mut last_drag = LAST_DRAG.lock().unwrap();
            let dragging = matches!(edit, GridEdit::Drag(_))
                && last_drag[deck].is_some_and(|t| t.elapsed() < DRAG_PAUSE)
                && history.last().is_some_and(|(id, _)| *id == current.id);
            last_drag[deck] = matches!(edit, GridEdit::Drag(_)).then(Instant::now);
            if !dragging {
                history.push((current.id, before));
                if history.len() > UNDO_DEPTH {
                    history.remove(0);
                }
            }
            after
        }
    };
    t.set_grid(grid);
    t.info.fixed_grid = true;
    t.info.grid_from = "user".into();
    t.info.grid_unsure = false;
    let t = Arc::new(t);
    l[deck] = Some(t.clone());
    Some(t)
}

/// Separates `audio` into stems with Spleeter in the background (see
/// [`separation::separate_fast`]), one track at a time, stopping if the
/// deck loads another track meanwhile.
fn separate_on_load(
    shared: &Arc<Shared>,
    deck: usize,
    generation: u64,
    audio: Arc<Audio>,
    from: usize,
) {
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());
    let shared = shared.clone();
    let spawned = std::thread::Builder::new()
        .name(format!("stems-deck-{}", deck + 1))
        .spawn(move || {
            let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
            let latest = || shared.is_latest(deck, generation);
            if !latest() {
                return;
            }
            if let Err(e) = separation::separate_fast(&audio, from, latest) {
                eprintln!("no stems on load: {e}");
            }
        });
    if let Err(e) = spawned {
        eprintln!("could not start separating stems: {e}");
    }
}

/// Decodes the stems from `source` and gives them to `audio` once they are
/// all decoded, so a deck switches to them with nothing missing (taking
/// over from stems separated on load). Stops if `keep_going` turns false.
pub(crate) fn load_stems(
    audio: &Audio,
    source: &StemSource,
    keep_going: impl Fn() -> bool,
) -> Result<(), String> {
    let stems = stems::Decode::open(source)?.run(&keep_going)?;
    // Better stems take over from the ones Spleeter made on load.
    let given = match audio.stems() {
        Some(s) => s.set_upgrade(stems),
        None => audio.set_stems(stems),
    };
    if keep_going() && !given {
        return Err("the stems don't match the track".into());
    }
    Ok(())
}

pub(crate) fn run(
    requests: Receiver<LoadRequest>,
    input: EngineInput,
    loaded: LoadedTracks,
    alerts: crate::Alerts,
) {
    let shared = Arc::new(Shared {
        input: Mutex::new(input),
        loaded,
        generation: Default::default(),
        alerts,
    });
    loop {
        shared.collect();
        // Short enough that an ejected deck empties on screen promptly.
        let req = match requests.recv_timeout(Duration::from_millis(50)) {
            Ok(r) => r,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        let generation = shared.generation[req.deck].fetch_add(1, Ordering::AcqRel) + 1;
        let shared = shared.clone();
        // One thread per load, so loading one deck never waits for another.
        let spawned = std::thread::Builder::new()
            .name(format!("load-deck-{}", req.deck + 1))
            .spawn(move || {
                if let Err(e) = load(&shared, req.deck, generation, &req.track) {
                    eprintln!("could not load track: {e}");
                    let name = if req.track.title.is_empty() {
                        "The track".to_string()
                    } else {
                        format!("“{}”", req.track.title)
                    };
                    crate::alert(
                        &shared.alerts,
                        format!("{name} couldn't be loaded on deck {}: {e}", req.deck + 1),
                    );
                }
            });
        if let Err(e) = spawned {
            eprintln!("could not start loading a track: {e}");
        }
    }
}

/// Loads `base` on `deck`: on the deck as soon as the file is open, then
/// decoded and analysed, with the UI and engine kept up to date.
fn load(shared: &Arc<Shared>, deck: usize, generation: u64, base: &Track) -> Result<(), String> {
    let latest = || shared.is_latest(deck, generation);
    let path = match &base.source {
        Source::File(p) => Some(p.as_path()),
        Source::Synth { .. } => None,
    };
    let cached = path.and_then(cache::load);
    let detected = key_is_detected(base);
    // A key found by another detector is found again once the audio is in.
    let detector = key::detector();
    let redetect = detected && cached.as_ref().is_some_and(|c| c.key_detector != detector);
    let mut decoder = None;
    let audio = match (&base.source, path) {
        (Source::Synth { seed }, _) => Arc::new(synth::render(base, *seed)),
        (_, Some(path)) => {
            let d = Decoder::open(path)?;
            if d.length_known() {
                let audio = d.audio().clone();
                decoder = Some(d);
                audio
            } else {
                // Without a length the overview can't be laid out; decode first.
                d.run(|_| latest())?
            }
        }
        (Source::File(_), None) => unreachable!(),
    };

    // A cached loudness levels the track from the moment it is loaded.
    if let Some(l) = cached.as_ref().and_then(|c| c.loudness) {
        audio.set_loudness(l);
    }
    let mut analyser = cached
        .is_none()
        .then(|| Analyser::new(audio.sample_rate, audio.len()));
    let waveform = match &cached {
        Some(c) => c.waveform.clone(),
        None => analyser.as_ref().unwrap().waveform(),
    };
    let mut first = Track {
        key: match cached.as_ref().and_then(|c| c.key) {
            Some(k) if detected && !redetect => k.camelot(),
            _ => base.key.clone(),
        },
        duration: audio.duration(),
        waveform: Arc::new(waveform),
        audio: Some(audio.clone()),
        ..base.clone()
    };
    match &cached {
        Some(c) => with_grid(&mut first, base, c),
        // Unknown until analysed; the beat grid starts at zero meanwhile.
        None if !base.info.fixed_grid && matches!(base.source, Source::File(_)) => {
            first.first_beat = 0.0
        }
        None => {}
    }
    let first = Arc::new(first);
    if !shared.put_on_deck(deck, generation, first.clone()) {
        return Ok(());
    }

    // Album art is looked up alongside decoding, so neither waits for the other.
    let cover: OnceLock<Arc<Cover>> = OnceLock::new();
    let with_cover = |t: Track| match cover.get() {
        Some(c) => Track {
            cover: Some(c.clone()),
            ..t
        },
        None => t,
    };
    let analysis = std::thread::scope(|scope| -> Result<Analysis, String> {
        if let Some(source) = path.and_then(stems::find) {
            if let StemSource::Files(files) = &source {
                // Used now, so the cache limit keeps them longest.
                if let Some(dir) = files.first().and_then(|f| f.parent()) {
                    cache::touch_stems(dir);
                }
            }
            let audio = audio.clone();
            scope.spawn(move || {
                if let Err(e) = load_stems(&audio, &source, latest) {
                    eprintln!("could not load stems: {e}");
                }
            });
        }
        if let Some(path) = path {
            scope.spawn(|| {
                if let Some(c) = cover::find(path, &base.album) {
                    let c = cover.get_or_init(|| Arc::new(c));
                    shared.modify(deck, &first, |t| t.cover = Some(c.clone()));
                }
            });
        }

        // Feeds the analyser the chunks decoded since it last ran, and now
        // and then shows the UI the waveform so far.
        let mut fed = 0;
        let mut last_update = Instant::now();
        let mut feed = |audio: &Audio, analyser: &mut Option<Analyser>| {
            let Some(a) = analyser.as_mut() else { return };
            for chunk in audio.decoded_chunks().skip(fed) {
                a.feed(chunk);
                fed += 1;
            }
            if last_update.elapsed() >= WAVEFORM_UPDATE {
                last_update = Instant::now();
                let partial = Arc::new(with_cover(Track {
                    waveform: Arc::new(a.waveform()),
                    ..(*first).clone()
                }));
                shared.update(deck, &partial);
            }
        };
        if let Some(d) = decoder {
            d.run(|audio| {
                feed(audio, &mut analyser);
                latest()
            })?;
        }
        // A file without stems is separated now, from the cue point.
        if separation::on_load() && path.is_some_and(|p| stems::find(p).is_none()) {
            let from = (first.first_beat.max(0.0) * audio.sample_rate as f64) as usize;
            separate_on_load(shared, deck, generation, audio.clone(), from);
        }
        if let Some(a) = analyser.as_mut() {
            // The last, partial chunk only appears once decoding is done.
            for chunk in audio.decoded_chunks().skip(fed) {
                a.feed(chunk);
            }
        }
        Ok(match (cached, analyser) {
            (Some(mut c), _) if redetect => {
                c.key = detect_key(&audio, detector).or(c.key);
                c.key_detector = detector;
                if let Some(path) = path {
                    cache::store(path, &c);
                }
                c
            }
            (Some(c), _) => c,
            (None, Some(a)) => {
                let analysis = a.finish(analysis_hint(base));
                if let Some(path) = path {
                    cache::store(path, &analysis);
                }
                analysis
            }
            (None, None) => unreachable!("an analyser runs whenever nothing is cached"),
        })
    })?;
    if let Some(l) = analysis.loudness {
        audio.set_loudness(l);
    }
    if !latest() {
        return Ok(());
    }
    // Beat This! takes a few seconds more: the deck plays on the DSP grid
    // meanwhile, then takes the grid fitted to its beats.
    let refine = beatthis::wanted(&analysis) && grid_replaceable(base) && path.is_some();
    let mut later = refine.then(|| analysis.clone());
    let done = Arc::new(with_cover(analysed(
        base,
        audio.clone(),
        analysis,
        detected,
    )));
    if shared.update(deck, &done) {
        shared.send(Command::Analysed {
            deck,
            track: done.clone(),
        });
    }
    if let (Some(analysis), Some(path)) = (later.as_mut(), path) {
        if let Err(e) = beatthis::refine(analysis, &audio, analysis_hint(base)) {
            eprintln!("no beats from Beat This!: {e}");
            return Ok(());
        }
        cache::store(path, analysis);
        if !latest() {
            return Ok(());
        }
        // An earlier analysis's grid from the library gives way too, and
        // the track comes back as analysed (not edited) so the library
        // takes the new grid.
        let mut t = (*done).clone();
        t.info.fixed_grid = false;
        let unfixed = Track {
            info: LibraryInfo {
                fixed_grid: false,
                ..base.info.clone()
            },
            ..base.clone()
        };
        with_grid(&mut t, &unfixed, analysis);
        let t = Arc::new(t);
        if shared.update(deck, &t) {
            shared.send(Command::Analysed { deck, track: t });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::tests::write_wav;
    use crate::engine::Engine;
    use crate::key::Key;
    use djsugar_control::ControlRegistry;
    use std::sync::mpsc;

    #[test]
    fn a_locked_grid_takes_no_edits_but_unlocking() {
        let loaded: LoadedTracks = Default::default();
        let mut t = (*synth::sample_library()[0]).clone();
        t.set_grid(BeatGrid::constant(124.0, 0.2));
        t.audio = Some(Arc::new(Audio::from_frames(44_100, vec![[0.0; 2]; 44_100])));
        let t = Arc::new(t);
        loaded.write().unwrap()[0] = Some(t.clone());
        let locked = edit_grid(&loaded, 0, &t, GridEdit::Lock(true), 0.0).unwrap();
        assert!(locked.info.grid_locked);
        assert!(edit_grid(&loaded, 0, &locked, GridEdit::Shift(0.01), 0.0).is_none());
        assert!(edit_grid(&loaded, 0, &locked, GridEdit::Lock(true), 0.0).is_none());
        let open = edit_grid(&loaded, 0, &locked, GridEdit::Lock(false), 0.0).unwrap();
        let moved = edit_grid(&loaded, 0, &open, GridEdit::Shift(0.01), 0.0).unwrap();
        assert!((moved.first_beat - 0.21).abs() < 1e-9);
        assert_eq!(moved.info.grid_from, "user");
    }

    #[test]
    fn a_drag_is_one_undo_step() {
        // Deck 4: the undo history is shared with the other tests.
        let deck = NUM_DECKS - 1;
        let loaded: LoadedTracks = Default::default();
        let mut t = (*synth::sample_library()[0]).clone();
        t.set_grid(BeatGrid::constant(124.0, 0.2));
        t.audio = Some(Arc::new(Audio::from_frames(44_100, vec![[0.0; 2]; 44_100])));
        let mut t = Arc::new(t);
        loaded.write().unwrap()[deck] = Some(t.clone());
        for _ in 0..3 {
            t = edit_grid(&loaded, deck, &t, GridEdit::Drag(0.01), 0.0).unwrap();
        }
        assert!((t.first_beat - 0.23).abs() < 1e-9);
        let back = edit_grid(&loaded, deck, &t, GridEdit::Undo, 0.0).unwrap();
        assert!((back.first_beat - 0.2).abs() < 1e-9);
    }

    #[test]
    fn only_analysed_or_missing_grids_give_way() {
        let mut t = (*synth::sample_library()[0]).clone();
        t.info.fixed_grid = false;
        assert!(grid_replaceable(&t));
        t.info.fixed_grid = true;
        assert!(!grid_replaceable(&t), "edited or imported");
        t.info.grid_analysed = true;
        assert!(grid_replaceable(&t), "from an earlier analysis");
        t.info.grid_locked = true;
        assert!(!grid_replaceable(&t), "locked");
    }

    /// Runs the engine like an audio callback until `done` holds.
    fn run_until(engine: &mut Engine, mut done: impl FnMut() -> bool) {
        let mut buf = vec![0.0f32; 512 * 2];
        let start = Instant::now();
        while !done() {
            assert!(start.elapsed() < Duration::from_secs(60), "timed out");
            engine.process(&mut buf, 2);
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn keys_from_tags_stay_and_detected_keys_follow_the_analysis() {
        let audio = Arc::new(synth::render_with(124.0, 0.25, 4.0, 3));
        let analysis = || Analysis {
            key: Key::parse("8A"),
            ..analyse(&audio, None)
        };
        let mut t = Track {
            key: "3B".into(),
            source: Source::File("/nowhere/track.mp3".into()),
            ..(*synth::sample_library()[0]).clone()
        };
        t.info.key_detected = Some(false);
        assert!(!key_is_detected(&t));
        let kept = analysed(&t, audio.clone(), analysis(), false);
        assert_eq!(
            (kept.key.as_str(), kept.info.key_detected),
            ("3B", Some(false))
        );

        t.info.key_detected = Some(true);
        assert!(key_is_detected(&t));
        let found = analysed(&t, audio.clone(), analysis(), true);
        assert_eq!(
            (found.key.as_str(), found.info.key_detected),
            ("8A", Some(true))
        );

        // Not recorded, and the file has no key tag: analysis found it.
        t.info.key_detected = None;
        assert!(key_is_detected(&t));
    }

    #[test]
    fn plays_before_analysis_then_reloads_from_cache() {
        let dir = std::env::temp_dir().join(format!("djsugar-loader-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        cache::TEST_DIR.set(dir.join("cache")).unwrap();
        let path = dir.join("beats.wav");
        let audio = synth::render_with(124.0, 0.25, 40.0, 9);
        let samples: Vec<i16> = audio
            .decoded_chunks()
            .flatten()
            .flat_map(|f| f.map(|v| (v * 32_000.0) as i16))
            .collect();
        write_wav(&path, audio.sample_rate, 2, &samples);
        let track = Arc::new(Track {
            bpm: 0.0,
            first_beat: 0.0,
            source: Source::File(path),
            ..(*synth::sample_library()[0]).clone()
        });

        let reg = ControlRegistry::new();
        let (mut engine, input) = Engine::new(&reg, 48_000.0);
        let loaded: LoadedTracks = Default::default();
        let (requests, rx) = mpsc::channel();
        {
            let loaded = loaded.clone();
            std::thread::spawn(move || run(rx, input, loaded, Default::default()));
        }
        let deck = |l: &LoadedTracks| l.read().unwrap()[0].clone();
        let bpm = reg.get("[Channel1]", "bpm").unwrap();

        // On the deck straight away, tempo unknown; then analysed.
        requests
            .send(LoadRequest {
                deck: 0,
                track: track.clone(),
            })
            .unwrap();
        let mut first = None;
        run_until(&mut engine, || {
            first = first.take().or_else(|| deck(&loaded));
            first.is_some()
        });
        assert_eq!(first.unwrap().bpm, 0.0);
        run_until(&mut engine, || (bpm.get() - 124.0).abs() < 0.05);
        let analysed = deck(&loaded).unwrap();
        assert!((analysed.first_beat - 0.25).abs() < 0.02);
        assert!(analysed.audio.as_ref().unwrap().is_complete());
        let cue = reg.get("[Channel1]", "cue_point").unwrap();
        assert_eq!(cue.get(), analysed.first_beat);

        // Loaded again: the cached tempo and waveform come with it.
        requests.send(LoadRequest { deck: 0, track }).unwrap();
        run_until(&mut engine, || {
            deck(&loaded).is_some_and(|t| {
                !Arc::ptr_eq(t.audio.as_ref().unwrap(), analysed.audio.as_ref().unwrap())
            })
        });
        let again = deck(&loaded).unwrap();
        assert_eq!(again.bpm, analysed.bpm);
        assert_eq!(again.waveform.levels, analysed.waveform.levels);

        // The library's mini waveform was kept with the analysis, and is
        // made again from it when its file is gone.
        let Source::File(path) = &again.source else {
            unreachable!()
        };
        let overview = cache::overview(path).unwrap();
        assert_eq!(overview, analysed.waveform.overview(cache::OVERVIEW_SLICES));
        assert!(overview.iter().any(|s| s[0] > 100));
        std::fs::remove_dir_all(dir.join("cache/overviews")).unwrap();
        assert_eq!(cache::overview(path).unwrap(), overview);
        assert!(dir.join("cache/overviews").is_dir());

        // Analysed on request: past the cache, a grid from an earlier
        // analysis is replaced, and one the DJ edited stays.
        let mut stale = Track {
            audio: None,
            waveform: Arc::default(),
            ..(*again).clone()
        };
        stale.set_grid(BeatGrid::constant(100.0, 0.0));
        stale.info.fixed_grid = true;
        stale.info.grid_analysed = true;
        assert_eq!(analyse_file(&stale, false).unwrap().bpm, 100.0);
        let fresh = analyse_file(&stale, true).unwrap();
        assert!((fresh.bpm - 124.0).abs() < 0.05, "{}", fresh.bpm);
        assert!(fresh.waveform.is_empty() && fresh.audio.is_none());
        stale.info.grid_analysed = false;
        assert_eq!(analyse_file(&stale, true).unwrap().bpm, 100.0);
        // The library's tempo holds analysis to it only when it didn't come
        // from an earlier analysis, which may have got it wrong.
        assert_eq!(tempo_hint(&stale), Some(100.0));
        stale.info.grid_analysed = true;
        assert_eq!(tempo_hint(&stale), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
