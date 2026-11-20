//! The audio engine.
//!
//! It owns the named controls (`[Channel1],play`,
//! `[Master],crossfader`, ...), plays the decks through a 3-band isolator EQ,
//! a filter and the effect units, mixes them with the crossfader and sends the
//! result to the sound card. The UI only talks to controls.
//!
//! Threads: the audio callback runs [`engine::Engine`]; loader threads put
//! tracks on decks over a wait-free queue as soon as their files are open,
//! then decode and analyse them while they play.

pub mod analysis;
pub mod arp;
mod auto_trim;
pub mod background;
pub mod beats;
pub mod beatthis;
pub mod cache;
pub mod clock_in;
mod controls;
pub mod cover;
pub mod decode;
pub mod dsp;
mod effects;
pub mod engine;
mod gridfit;
pub mod groove;
pub mod groove_dice;
pub mod groove_fills;
pub mod groove_pads;
pub mod growing;
pub mod import;
pub mod inputs;
/// Timecode vinyl and CD (DVS), for the settings that pick a record.
pub use djsugar_timecode as timecode;
pub mod key;
pub mod keynet;
mod keyshift;
pub mod kits;
pub mod link;
pub mod live_wave;
pub mod loader;
pub mod looper;
pub mod loudness;
pub mod lyrics;
pub mod meter;
pub mod midi_clock;
mod output;
pub mod paths;
pub mod patterns;
pub mod phrases;
pub mod plugins;
pub mod preview;
pub mod process;
pub mod recorder;
#[cfg(test)]
mod rt_guard;
pub mod sampler;
pub mod scale;
pub mod scan;
pub mod separation;
pub mod stems;
pub mod synth;
pub mod tags;
pub mod track;
mod visual;
pub mod vocals;
pub mod word_timing;

use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, RwLock};

use djsugar_control::ControlRegistry;

pub use beats::{BeatGrid, GridMarker};
pub use controls::{
    deck_group, default_orientation, effect_group, effect_unit_group, eq_group, pad_fx_unit,
    quick_effect_group, size_name, stem_group, BEATJUMP_SIZES, BEATLOOP_SIZES, BEAT_FX_UNIT,
    DEFAULT_AUTO_GAIN_TARGET, EFFECTS_PER_UNIT, KEYBOARD_SEMITONES, MIC_FX_UNIT, NUM_DECKS,
    NUM_EFFECT_UNITS, NUM_HOTCUES, XFADER_CUT_IN,
};
pub use effects::{
    color_fx_name, effect_name, parameter_names, plugin_index, InstrumentView, NoteInput,
    PluginLink, AVAILABLE_EFFECTS, COLOR_FX, DEFAULT_EFFECTS, MAX_BUTTON_PARAMETERS,
    MAX_PARAMETERS, MAX_PLUGINS, PLUGIN_BASE, SCOPE_BINS,
};
pub use output::{
    channel_names, deck_outputs_split, default_output_device, input_devices, output_devices,
    AudioSetup, InputRole, InputRoute, OutputDevice, Role, Route, BUFFER_SIZES, SAMPLE_RATES,
};
pub use recorder::{RecordFormat, RecordSettings, Recorder};
pub use synth::sample_library;
pub use track::{
    hashtags, same_tag, split_tags, tag_name, Audio, Hotcue, LibraryInfo, Marker, MarkerKind,
    OverviewSlice, Source, Track, TrackTag, Waveform, WaveformBin, MEMORY_CUE, WAVEFORM_RATE,
};
pub use visual::VisualPositions;

/// The tracks currently loaded on each deck, with their audio, readable by the
/// UI and the waveform renderers. While a track is being analysed its entry is
/// replaced with fuller versions (more waveform, then tempo and first beat).
pub type LoadedTracks = Arc<RwLock<[Option<Arc<Track>>; NUM_DECKS]>>;

/// Messages for the person at the decks, such as why a track didn't load,
/// oldest first. The UI shows and empties them.
pub type Alerts = Arc<Mutex<Vec<String>>>;

pub(crate) fn alert(alerts: &Alerts, message: String) {
    if let Ok(mut a) = alerts.lock() {
        a.push(message);
    }
}

/// What a sampler slot plays; see [`AudioEngine::set_sample`].
#[derive(Clone, Debug, PartialEq)]
pub enum SampleSource {
    Empty,
    /// One of [`sampler::SAMPLE_NAMES`].
    Builtin(usize),
    File(std::path::PathBuf),
}

/// Handle to the running engine. Dropping it stops playback.
pub struct AudioEngine {
    registry: Arc<ControlRegistry>,
    alerts: Alerts,
    requests: Sender<loader::LoadRequest>,
    loaded: LoadedTracks,
    visual: Arc<VisualPositions>,
    output: RefCell<output::Outputs>,
    recorder: Recorder,
    preview: Arc<Mutex<preview::PreviewInput>>,
    /// Counts preview requests, so a file still decoding for an earlier
    /// one stops.
    preview_generation: Arc<AtomicU64>,
    samplers: Arc<Mutex<sampler::SamplerInput>>,
    /// Counts each slot's sample requests, so a file still decoding for an
    /// earlier one is dropped.
    sample_generation: Arc<[AtomicU64; sampler::NUM_SAMPLERS]>,
    rate: f64,
    /// Runs audio plugins in the effect slots, when the helper is installed.
    plugins: Option<plugins::PluginHost>,
    /// Notes for the Instrument input.
    notes: Arc<Mutex<NoteInput>>,
    /// What the Instrument input plays: notes held and a scope.
    instrument_view: Arc<InstrumentView>,
    /// Each deck's live sound as waveform slices, until the app takes them.
    live_waves: Mutex<Option<[rtrb::Consumer<live_wave::LiveSlice>; NUM_DECKS]>>,
    /// Each Groove Box's hits as played, until the app takes them.
    groove_hits: Mutex<Option<[rtrb::Consumer<groove::GrooveHit>; NUM_DECKS]>>,
    /// The master for broadcasting, until the broadcaster takes it.
    broadcast_feed: Mutex<Option<recorder::Feed>>,
    /// Picks the outputs MIDI clock goes to.
    midi_clock: midi_clock::MidiClock,
}

/// The master output, for broadcasting: a copy of what the speakers get,
/// after the limiter. The audio thread only copies while it is active.
pub struct MasterFeed(recorder::Feed);

impl MasterFeed {
    /// Moves every sample waiting (interleaved stereo, at the engine's
    /// rate) onto the end of `out`.
    pub fn drain_into(&mut self, out: &mut Vec<f32>) {
        self.0.drain_into(out);
    }

    /// Starts or stops the copying. While it is on, the limiter keeps true
    /// peaks under the air ceiling.
    pub fn set_active(&self, on: bool) {
        self.0.set_active(on);
    }
}

/// The plugin helper, `djsugar-plugin-host`, installed beside the app.
pub fn plugin_helper() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let name = format!("djsugar-plugin-host{}", std::env::consts::EXE_SUFFIX);
    let path = exe.parent()?.join(name);
    path.is_file().then_some(path)
}

/// Creates every engine control in `registry` without starting audio, for
/// tools that only need to know which controls exist.
pub fn create_controls(registry: &Arc<ControlRegistry>) {
    let rate = 44_100.0;
    let (mut engine, _input) = engine::Engine::new(registry, rate);
    let feed = engine.take_record_feed().expect("record feed");
    Recorder::new(registry, rate, feed, LoadedTracks::default());
    meter::start(
        registry,
        rate,
        engine.take_meter_feed().expect("meter feed"),
    );
    output::Health::new(registry);
}

impl AudioEngine {
    /// Creates all engine controls in `registry` and starts audio output to
    /// the devices `setup` names. The engine runs at the main device's rate.
    pub fn start(registry: Arc<ControlRegistry>, setup: &AudioSetup) -> Self {
        let rate = output::sample_rate(setup);
        let (mut engine, input) = engine::Engine::new(&registry, rate);
        let feed = engine.take_record_feed().expect("record feed");
        meter::start(
            &registry,
            rate,
            engine.take_meter_feed().expect("meter feed"),
        );
        let broadcast_feed = engine.take_broadcast_feed();
        let preview = engine.take_preview_input().expect("preview input");
        let samplers = engine.take_sampler_input().expect("sampler input");
        let visual = engine.visual_positions();
        let plugin_link = engine.take_plugin_link().expect("plugin link");
        let notes = Arc::new(Mutex::new(engine.take_note_input().expect("note input")));
        let instrument_view = engine.instrument_view();
        let live_waves = engine.take_live_waves().expect("live waves");
        let groove_hits = engine.take_groove_hits().expect("groove hits");
        let plugins = plugin_helper().map(|helper| {
            plugins::PluginHost::start(
                &registry,
                plugin_link,
                plugins::HostOptions::new(helper, rate as u32),
            )
        });
        link::watch(engine.link_handle(), Arc::downgrade(&registry));
        looper::supply(
            engine.take_looper_supply().expect("looper supply"),
            Arc::downgrade(&registry),
            NUM_DECKS,
            rate,
        );
        let midi_clock = midi_clock::start(
            engine.beat_clock(),
            engine.clock_in(),
            engine.link_handle(),
            &registry,
        );
        let output = output::Outputs::new(engine, setup, output::Health::new(&registry));

        let loaded: LoadedTracks = Arc::new(RwLock::new(Default::default()));
        let recorder = Recorder::new(&registry, rate, feed, loaded.clone());
        let (requests, rx) = mpsc::channel();
        let alerts = Alerts::default();
        {
            let (loaded, alerts) = (loaded.clone(), alerts.clone());
            std::thread::Builder::new()
                .name("track-loader".into())
                .spawn(move || loader::run(rx, input, loaded, alerts))
                .expect("spawn track loader");
        }
        Self {
            registry,
            alerts,
            requests,
            loaded,
            visual,
            output: RefCell::new(output),
            recorder,
            preview: Arc::new(Mutex::new(preview)),
            preview_generation: Arc::default(),
            samplers: Arc::new(Mutex::new(samplers)),
            sample_generation: Arc::default(),
            rate,
            plugins,
            notes,
            instrument_view,
            live_waves: Mutex::new(Some(live_waves)),
            groove_hits: Mutex::new(Some(groove_hits)),
            broadcast_feed: Mutex::new(broadcast_feed),
            midi_clock,
        }
    }

    /// MIDI clock out: which outputs it sends to.
    pub fn midi_clock(&self) -> midi_clock::MidiClock {
        self.midi_clock.clone()
    }

    /// The master output for broadcasting; there is only one, so later
    /// calls get `None`.
    pub fn take_master_feed(&self) -> Option<MasterFeed> {
        self.broadcast_feed.lock().ok()?.take().map(MasterFeed)
    }

    /// The rate the engine runs at, in Hz.
    pub fn sample_rate(&self) -> f64 {
        self.rate
    }

    /// The instruments the Instrument input can load
    /// (`[Instrument1],instrument` 1 is the first).
    pub fn set_instruments(&self, entries: Vec<plugins::PluginEntry>) {
        if let Some(p) = &self.plugins {
            p.set_instruments(entries);
        }
    }

    /// Keeps each plugin's sound in `folder` between uses and sessions (see
    /// [`plugins::PluginHost::set_sound_folder`]).
    pub fn set_plugin_sound_folder(&self, folder: std::path::PathBuf) {
        if let Some(p) = &self.plugins {
            p.set_sound_folder(folder);
        }
    }

    /// The Instrument input's plugin key and its whole sound now, for a
    /// preset (see [`plugins::PluginHost::instrument_sound`]).
    pub fn instrument_sound(&self) -> Result<(String, Vec<u8>), String> {
        match &self.plugins {
            Some(p) => p.instrument_sound(),
            None => Err("this build hosts no plugins".into()),
        }
    }

    /// Gives instrument plugin `key` a preset's sound (see
    /// [`plugins::PluginHost::load_instrument_sound`]).
    pub fn load_instrument_sound(&self, key: &str, data: Vec<u8>) -> Result<(), String> {
        match &self.plugins {
            Some(p) => p.load_instrument_sound(key, data),
            None => Err("this build hosts no plugins".into()),
        }
    }

    /// Loads one of instrument plugin `key`'s own presets (see
    /// [`plugins::PluginHost::load_instrument_preset`]).
    pub fn load_instrument_preset(&self, key: &str, reference: &str) -> Result<(), String> {
        match &self.plugins {
            Some(p) => p.load_instrument_preset(key, reference),
            None => Err("this build hosts no plugins".into()),
        }
    }

    /// The playing instrument's parameter names, for its knobs
    /// (`[Instrument1],parameterN` is the N-th); empty while none plays.
    pub fn instrument_parameters(&self) -> Vec<String> {
        self.plugins
            .as_ref()
            .map_or_else(Vec::new, |p| p.instrument_parameters())
    }

    /// Every parameter of the playing instrument, (id, name), to pick
    /// from for a knob; empty while none plays.
    pub fn instrument_all_parameters(&self) -> Vec<(u32, String)> {
        self.plugins
            .as_ref()
            .map_or_else(Vec::new, |p| p.instrument_all_parameters())
    }

    /// The parameter id each of the playing instrument's knobs turns.
    pub fn instrument_knob_ids(&self) -> Vec<u32> {
        self.plugins
            .as_ref()
            .map_or_else(Vec::new, |p| p.instrument_knob_ids())
    }

    /// The playing instrument's knobs the DJ picked a parameter for:
    /// (knob from 0, parameter id).
    pub fn instrument_picks(&self) -> Vec<(usize, u32)> {
        self.plugins
            .as_ref()
            .map_or_else(Vec::new, |p| p.instrument_picks())
    }

    /// Puts parameter `id` on the playing instrument's knob `knob` (from
    /// 0), or (none) gives it back to the plugin's own choice; the
    /// instrument restarts with its sound to take it.
    pub fn pick_instrument_knob(&self, knob: usize, id: Option<u32>) -> Result<(), String> {
        match &self.plugins {
            Some(p) => p.pick_instrument_knob(knob, id),
            None => Err("this build hosts no plugins".into()),
        }
    }

    /// Sets all of instrument plugin `key`'s knob picks, as a preset has them.
    pub fn set_instrument_picks(&self, key: &str, picks: Vec<(usize, u32)>) -> Result<(), String> {
        match &self.plugins {
            Some(p) => p.set_instrument_picks(key, picks),
            None => Err("this build hosts no plugins".into()),
        }
    }

    /// Keeps every running plugin's sound and stops the plugins, waiting
    /// until done. Call when quitting.
    pub fn shut_down_plugins(&self) {
        if let Some(p) = &self.plugins {
            p.shut_down();
        }
    }

    /// Where notes for the Instrument input go in, from any thread.
    pub fn note_input(&self) -> Arc<Mutex<NoteInput>> {
        self.notes.clone()
    }

    /// What the Instrument input plays: the notes held and a scope of its
    /// sound, for the deck playing it to show.
    pub fn instrument_view(&self) -> Arc<InstrumentView> {
        self.instrument_view.clone()
    }

    /// Each deck's sound while it plays a live source (an input, an
    /// instrument or the Groove Box), a slice every millisecond, for its
    /// scrolling waveform. There is one set; later calls get `None`.
    pub fn take_live_waves(&self) -> Option<[rtrb::Consumer<live_wave::LiveSlice>; NUM_DECKS]> {
        self.live_waves.lock().ok()?.take()
    }

    /// Each Groove Box's hits as it plays them, with the beat each fell on,
    /// for its deck's step lanes. There is one set; later calls get `None`.
    pub fn take_groove_hits(&self) -> Option<[rtrb::Consumer<groove::GrooveHit>; NUM_DECKS]> {
        self.groove_hits.lock().ok()?.take()
    }

    /// Whether audio plugins can run: the helper is installed.
    pub fn plugins_available(&self) -> bool {
        self.plugins.is_some()
    }

    /// The plugins the effect slots can load, after the built-in effects.
    pub fn set_plugins(&self, entries: Vec<plugins::PluginEntry>) {
        if let Some(p) = &self.plugins {
            p.set_plugins(entries);
        }
    }

    /// What happened to plugins since the last call (crashes, failures).
    fn plugin_events(&self) -> Vec<plugins::PluginEvent> {
        self.plugins
            .as_ref()
            .map(|p| p.events())
            .unwrap_or_default()
    }

    /// Puts `track` in the preview player (`[PreviewDeck1]`, heard in the
    /// headphones only) at its start, playing if `play`, or empties it. The
    /// file is opened and decoded in the background; it plays while decoding.
    pub fn preview(&self, track: Option<&Track>, play: bool) {
        let generation = self.preview_generation.fetch_add(1, Ordering::AcqRel) + 1;
        let Some(track) = track else {
            if let Ok(mut p) = self.preview.lock() {
                p.send(None, false);
            }
            return;
        };
        let source = track.source.clone();
        let base = track.clone();
        let input = self.preview.clone();
        let current = self.preview_generation.clone();
        let spawned = std::thread::Builder::new()
            .name("preview".into())
            .spawn(move || {
                let latest = || current.load(Ordering::Acquire) == generation;
                let send = |audio: Arc<Audio>| {
                    if latest() {
                        if let Ok(mut p) = input.lock() {
                            p.send(Some(audio), play);
                        }
                    }
                };
                match &source {
                    Source::Synth { seed } => send(Arc::new(synth::render(&base, *seed))),
                    Source::File(path) => match decode::Decoder::open(path) {
                        Ok(d) if d.length_known() => {
                            send(d.audio().clone());
                            let _ = d.run(|_| latest());
                        }
                        Ok(d) => {
                            if let Ok(audio) = d.run(|_| latest()) {
                                send(audio);
                            }
                        }
                        Err(e) => eprintln!("can't preview {}: {e}", path.display()),
                    },
                }
            });
        if let Err(e) = spawned {
            eprintln!("could not start the preview: {e}");
        }
    }

    /// Puts the file at `path` in the preview player, playing, while it's
    /// still being downloaded: it will hold `size` bytes, and plays as far
    /// as they've come. Reading stops once something else is previewed.
    pub fn preview_download(&self, path: std::path::PathBuf, size: u64) {
        let generation = self.preview_generation.fetch_add(1, Ordering::AcqRel) + 1;
        let input = self.preview.clone();
        let current = self.preview_generation.clone();
        let spawned = std::thread::Builder::new()
            .name("preview".into())
            .spawn(move || {
                let latest = {
                    let current = current.clone();
                    move || current.load(Ordering::Acquire) == generation
                };
                let wanted: Arc<dyn Fn() -> bool + Send + Sync> = Arc::new(latest.clone());
                match decode::Decoder::open_growing(&path, size, wanted) {
                    // Plays at once, even with no length yet: it's only a
                    // taste, and waiting for the whole file would defeat it.
                    Ok(d) => {
                        if latest() {
                            if let Ok(mut p) = input.lock() {
                                p.send(Some(d.audio().clone()), true);
                            }
                        }
                        let _ = d.run(|_| latest());
                    }
                    Err(e) => eprintln!("can't preview {}: {e}", path.display()),
                }
            });
        if let Err(e) = spawned {
            eprintln!("could not start the preview: {e}");
        }
    }

    /// Puts the built-in sound `n`, the audio file at a path, or nothing on
    /// sampler slot `slot`. Files are decoded in the background; one that
    /// can't be read, or is too long, leaves the slot as it was, and
    /// `done` hears why. `done` gets the sample's length in seconds, and
    /// isn't called when a later request for the slot overtook this one.
    pub fn set_sample(
        &self,
        slot: usize,
        sample: SampleSource,
        done: impl FnOnce(Result<f64, String>) + Send + 'static,
    ) {
        let Some(counter) = self.sample_generation.get(slot) else {
            return;
        };
        let generation = counter.fetch_add(1, Ordering::AcqRel) + 1;
        let (input, all, rate) = (
            self.samplers.clone(),
            self.sample_generation.clone(),
            self.rate,
        );
        let current = move || all[slot].load(Ordering::Acquire) == generation;
        let send = move |sample: Option<sampler::Sample>| {
            let seconds = sample.as_ref().map_or(0.0, |s| s.seconds());
            let sent = input
                .lock()
                .map(|mut i| i.send(slot, sample.map(Arc::new)))
                .unwrap_or(false);
            if sent {
                Ok(seconds)
            } else {
                Err("The sampler is busy, try again.".to_owned())
            }
        };
        match sample {
            SampleSource::Empty => done(send(None)),
            SampleSource::Builtin(n) => done(send(Some(sampler::Sample::builtin(n, rate)))),
            SampleSource::File(path) => {
                let spawned = std::thread::Builder::new()
                    .name("sample-loader".into())
                    .spawn(move || {
                        let loaded = sampler::Sample::load(&path, rate);
                        if current() {
                            done(loaded.and_then(|s| send(Some(s))));
                        }
                    });
                if let Err(e) = spawned {
                    eprintln!("could not load the sample: {e}");
                }
            }
        }
    }

    /// Plays a sampler sound in the headphones, through the preview player.
    pub fn preview_sample(&self, sample: SampleSource) {
        let generation = self.preview_generation.fetch_add(1, Ordering::AcqRel) + 1;
        let (input, current, rate) = (
            self.preview.clone(),
            self.preview_generation.clone(),
            self.rate,
        );
        let play = move |s: sampler::Sample| {
            if current.load(Ordering::Acquire) == generation {
                if let Ok(mut p) = input.lock() {
                    p.send(Some(Arc::new(s.to_audio())), true);
                }
            }
        };
        match sample {
            SampleSource::Empty => {}
            SampleSource::Builtin(n) => play(sampler::Sample::builtin(n, rate)),
            SampleSource::File(path) => {
                let _ = std::thread::Builder::new()
                    .name("preview".into())
                    .spawn(move || match sampler::Sample::load(&path, rate) {
                        Ok(s) => play(s),
                        Err(e) => eprintln!("can't preview {}: {e}", path.display()),
                    });
            }
        }
    }

    /// Puts `track` on `deck` as soon as its file is open; decoding and
    /// analysis carry on in the background (see [`loader`]).
    /// Unless `[Controls],AllowTrackLoadToPlayingDeck` is on, a deck that is
    /// playing keeps its track and this returns false.
    pub fn load_track(&self, deck: usize, track: Arc<Track>) -> bool {
        let on =
            |group: &str, key: &str| self.registry.get(group, key).is_some_and(|c| c.get_bool());
        if on(&deck_group(deck), "play") && !on("[Controls]", "AllowTrackLoadToPlayingDeck") {
            alert(
                &self.alerts,
                format!(
                    "Deck {} is playing, so the track wasn't loaded. Stop the deck first, \
                     or turn off Protect playing decks in Settings.",
                    deck + 1
                ),
            );
            return false;
        }
        self.recorder.track_loaded(deck, &track);
        let _ = self.requests.send(loader::LoadRequest { deck, track });
        true
    }

    /// Records the master mix; see [`recorder`].
    pub fn recorder(&self) -> &Recorder {
        &self.recorder
    }

    /// Shows `message` to the person at the decks.
    pub fn alert(&self, message: String) {
        alert(&self.alerts, message);
    }

    /// Alerts since the last call, oldest first. Also reopens an audio
    /// output that went away once it is back, so call it every so often.
    pub fn take_alerts(&self) -> Vec<String> {
        let mut alerts: Vec<String> = self
            .alerts
            .lock()
            .map(|mut a| std::mem::take(&mut *a))
            .unwrap_or_default();
        if let Ok(mut output) = self.output.try_borrow_mut() {
            alerts.extend(output.recover());
        }
        alerts.extend(self.plugin_events().into_iter().map(|e| e.to_string()));
        alerts
    }

    /// Whether the system refused the audio thread real-time priority, so
    /// a busy computer is more likely to make it drop out.
    pub fn realtime_denied(&self) -> bool {
        self.output.borrow().realtime_denied()
    }

    /// Shows `cover` as the album art of every deck with the file at
    /// `path` loaded, after the file's cover was changed.
    pub fn set_cover(&self, path: &std::path::Path, cover: Option<Arc<cover::Cover>>) {
        let mut loaded = self.loaded.write().unwrap();
        for slot in loaded.iter_mut() {
            if let Some(t) = slot
                .as_ref()
                .filter(|t| matches!(&t.source, Source::File(p) if p == path))
            {
                *slot = Some(Arc::new(Track {
                    cover: cover.clone(),
                    ..(**t).clone()
                }));
            }
        }
    }

    pub fn loaded_tracks(&self) -> LoadedTracks {
        self.loaded.clone()
    }

    /// Play positions to draw from, smooth between audio buffers.
    pub fn visual_positions(&self) -> Arc<VisualPositions> {
        self.visual.clone()
    }

    /// Switches the outputs to the routes `setup` lists, and the inputs to
    /// its inputs.
    pub fn set_outputs(&self, setup: &AudioSetup) {
        self.output.borrow_mut().configure(setup);
    }

    /// Switches the microphone and auxiliary inputs to the devices `setup`
    /// lists, leaving the outputs playing.
    pub fn set_inputs(&self, setup: &AudioSetup) {
        self.output.borrow_mut().configure_inputs(setup);
    }

    /// Each input's state, by [`InputRole::index`]: its device and format,
    /// what is wrong with it, or empty when it has none.
    pub fn input_status(&self) -> Vec<String> {
        self.output.borrow().input_status()
    }

    /// The peak (0 to 1 and up, linear) of each channel of each open input
    /// device since the last call, by the device's name as
    /// [`InputRoute::device`] gives it: the input picker's meters.
    pub fn input_levels(&self) -> Vec<(String, Vec<f32>)> {
        self.output.borrow().input_levels()
    }

    /// How far behind each input is heard, in ms, by [`InputRole::index`]:
    /// its queue plus one device callback; `None` while it isn't open.
    pub fn input_delays(&self) -> Vec<Option<f64>> {
        self.output.borrow().input_delays()
    }

    /// Meters every channel of the input device called `device` (empty for
    /// the system default) while the input picker shows it, opening it if
    /// no input uses it; `None` once the picker closes.
    pub fn watch_input(&self, device: Option<&str>) {
        self.output.borrow_mut().watch_input(device);
    }

    /// Prepares high-quality stems for `track` in the background (see
    /// [`stems::separate`]); when they are ready, they take over on any deck
    /// the track is loaded on. Check on it with [`stems::separation`].
    pub fn prepare_hq_stems(&self, track: &Track) {
        let Source::File(path) = &track.source else {
            return;
        };
        if stems::find(path).is_some() {
            return;
        }
        let loaded = self.loaded.clone();
        let file = path.clone();
        stems::separate(path, move |result| {
            let source = match result {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("stem separation failed: {e}");
                    return;
                }
            };
            let decks: Vec<_> = loaded.read().unwrap().iter().flatten().cloned().collect();
            // Room for these: the stems used longest ago go, never ones
            // loaded on a deck.
            let mut keep = loaded_paths(&decks);
            keep.push(file.clone());
            separation::trim_cache(&keep);
            for t in decks {
                let here = matches!(&t.source, Source::File(p) if *p == file);
                if let (true, Some(audio)) = (here, &t.audio) {
                    if let Err(e) = loader::load_stems(audio, &source, || true) {
                        eprintln!("could not load stems: {e}");
                    }
                }
            }
        });
    }

    /// The device running the engine, or why there is no sound.
    pub fn status(&self) -> String {
        self.output.borrow().main_status.clone()
    }

    /// Each route's state, in the setup's order.
    pub fn route_status(&self) -> Vec<String> {
        self.output.borrow().route_status.clone()
    }
}

/// The files of `tracks`.
fn loaded_paths(tracks: &[Arc<Track>]) -> Vec<std::path::PathBuf> {
    tracks
        .iter()
        .filter_map(|t| match &t.source {
            Source::File(p) => Some(p.clone()),
            _ => None,
        })
        .collect()
}
