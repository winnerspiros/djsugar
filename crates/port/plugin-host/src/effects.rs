//! What a helper runs. Real plugin formats (CLAP, then VST3 and AU) will
//! implement [`Effect`] too; for now there are test effects that misbehave
//! on purpose, so the isolation can be tested without third-party plugins.

use crate::client::Transport;
use crate::shm::{Midi, CHANNELS};

/// One loaded effect, inside the helper, on its audio thread.
pub trait Effect: Send {
    fn process(
        &mut self,
        input: [&[f32]; CHANNELS],
        output: [&mut [f32]; CHANNELS],
        params: &[f32],
        transport: &Transport,
        events: &[Midi],
    );
}

/// The part of a plugin that must stay on the helper's main thread
/// (CLAP's instance, the editor). The helper calls `idle` regularly.
pub trait MainThread {
    fn idle(&mut self);

    /// The plugin's whole state (its sound and settings), as bytes only it
    /// reads, for `load_state` to bring back later.
    fn save_state(&mut self) -> Result<Vec<u8>, String> {
        Err("it can't save its sound".into())
    }

    /// Brings the plugin back to a state `save_state` gave.
    fn load_state(&mut self, data: &[u8]) -> Result<(), String> {
        let _ = data;
        Err("it can't load a saved sound".into())
    }

    /// Loads one of the plugin's own presets, as a preset list gave it
    /// (see `crate::client::presets`).
    fn load_preset(&mut self, reference: &str) -> Result<(), String> {
        let _ = reference;
        Err("it can't load its own presets".into())
    }

    /// The parameters' values now, in [`Info::params`] order and their own
    /// ranges, into `out` (as long as the list). False when it can't tell.
    fn param_values(&mut self, out: &mut [f64]) -> bool {
        let _ = out;
        false
    }

    /// DJ Sugar set parameter `index` to `value` (its own range), for a
    /// plugin whose editor keeps its own copy of the values, as a VST3
    /// controller does.
    fn param_set(&mut self, index: usize, value: f64) {
        let _ = (index, value);
    }

    /// Puts the plugin's own window (its editor) inside `parent` and returns
    /// its size, in the units `parent` uses (points on macOS, pixels
    /// elsewhere). Errors when the plugin has none.
    fn open_editor(&mut self, parent: ParentWindow, scale: f64) -> Result<EditorSize, String> {
        let _ = (parent, scale);
        Err("it has no window of its own".into())
    }

    /// Takes the editor out of its window again.
    fn close_editor(&mut self) {}

    /// The window was resized to `size`; returns the size the editor took.
    fn editor_resized(&mut self, size: EditorSize) -> EditorSize {
        size
    }

    /// The window now has the editor's size: a chance to lay the editor
    /// out again inside it (some editors place themselves by the window's
    /// size when they are attached, before the window took theirs).
    fn editor_placed(&mut self) {}

    /// Whether the editor can be resized by dragging the window's edge.
    fn editor_can_resize(&self) -> bool {
        false
    }

    /// A size the editor asked its window to take, since the last call.
    fn take_editor_resize(&mut self) -> Option<EditorSize> {
        None
    }

    /// Whether the plugin closed its own editor (or lost it), since the last
    /// call.
    fn take_editor_closed(&mut self) -> bool {
        false
    }

    /// When `idle` next needs calling, at the latest (timers a plugin asked
    /// for, its file descriptors on Linux).
    fn next_idle(&self) -> Option<std::time::Duration> {
        None
    }
}

/// The window an editor is put in, as each platform names it.
#[derive(Clone, Copy, Debug)]
pub enum ParentWindow {
    /// An `NSView *`.
    Cocoa(*mut std::ffi::c_void),
    /// An `HWND`.
    Win32(*mut std::ffi::c_void),
    /// An X11 window id.
    X11(u64),
}

/// An editor's size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EditorSize {
    pub width: u32,
    pub height: u32,
}

/// One parameter, in the plugin's own (plain) units.
#[derive(Clone, Debug, PartialEq)]
pub struct Param {
    /// The plugin's own id for it (CLAP param id, VST3 `ParamID`, AU
    /// parameter id; a test effect's index).
    pub id: u32,
    pub name: String,
    pub min: f64,
    pub max: f64,
    pub default: f64,
    /// Its value when the plugin loaded.
    pub value: f64,
}

/// What DJ Sugar learns about a loaded effect.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Info {
    pub name: String,
    /// Up to [`crate::shm::MAX_PARAMS`]; index `i` is shared parameter `i`.
    pub params: Vec<Param>,
    /// Every parameter a DJ could turn, (id, name) in the plugin's order,
    /// to pick from for a knob (up to [`ALL_PARAMS`]).
    pub all: Vec<(u32, String)>,
    /// Frames the plugin delays its output by.
    pub latency: u32,
}

/// A loaded effect: the audio half, the main-thread half, and its info.
pub struct Loaded {
    pub effect: Box<dyn Effect>,
    pub main: Option<Box<dyn MainThread>>,
    pub info: Info,
}

/// Which effect a helper runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Plugin {
    /// One of [`TEST_EFFECTS`].
    Test(String),
    /// Plugin `id` in the `.clap` file `file`.
    Clap { file: std::path::PathBuf, id: String },
    /// Class `id` (32 hex digits) in the `.vst3` bundle `file`.
    Vst3 { file: std::path::PathBuf, id: String },
    /// The Audio Unit `type:subtype:manufacturer` (macOS only).
    Au { id: String },
}

/// The test gain's "editor": an empty window of a set size, so the editor
/// window can be tested without third-party plugins. Its "sound" is
/// whatever bytes it was last given.
#[derive(Default)]
struct TestEditor {
    state: Vec<u8>,
}

impl MainThread for TestEditor {
    fn idle(&mut self) {}

    fn save_state(&mut self) -> Result<Vec<u8>, String> {
        Ok(self.state.clone())
    }

    fn load_state(&mut self, data: &[u8]) -> Result<(), String> {
        self.state = data.to_vec();
        Ok(())
    }

    fn open_editor(&mut self, _parent: ParentWindow, _scale: f64) -> Result<EditorSize, String> {
        Ok(EditorSize { width: 320, height: 200 })
    }
}

/// The most parameters listed to pick from (see [`Info::all`]).
pub const ALL_PARAMS: usize = 4096;

/// Puts a plugin's parameters in knob order and keeps the first [`MAX_PARAMS`](crate::shm::MAX_PARAMS): each picked
/// one (knob slot, param id) in its slot, then those in `preferred` (its maker's remote controls pages) in that order,
/// then the rest in its own order. Returns every parameter as (id, name), in its own order, for [`Info::all`].
pub fn arrange<T>(params: &mut Vec<(T, Param)>, preferred: &[u32], picks: &[(usize, u32)]) -> Vec<(u32, String)> {
    use crate::shm::MAX_PARAMS;
    let all = params.iter().take(ALL_PARAMS).map(|(_, p)| (p.id, p.name.clone())).collect();
    params.sort_by_key(|(_, p)| preferred.iter().position(|&id| id == p.id).unwrap_or(usize::MAX));
    let mut slots: Vec<Option<(T, Param)>> = (0..MAX_PARAMS).map(|_| None).collect();
    let mut rest = Vec::with_capacity(params.len());
    for (t, p) in params.drain(..) {
        match picks.iter().find(|&&(slot, id)| id == p.id && slot < MAX_PARAMS) {
            Some(&(slot, _)) if slots[slot].is_none() => slots[slot] = Some((t, p)),
            _ => rest.push((t, p)),
        }
    }
    let mut rest = rest.into_iter();
    for slot in &mut slots {
        if slot.is_none() {
            *slot = rest.next();
        }
    }
    // A pick past the plugin's last knob leaves gaps: close them up.
    params.extend(slots.into_iter().flatten());
    all
}

/// Ids of the test effects.
pub const TEST_EFFECTS: [&str; 5] = ["test-gain", "test-crash", "test-hang", "test-slow", "test-synth"];

/// Loads `plugin`, or says why not.
pub fn load(plugin: &Plugin, sample_rate: u32, max_frames: usize, picks: &[(usize, u32)]) -> Result<Loaded, String> {
    #[cfg(not(feature = "formats"))]
    let _ = picks;
    // Each test effect has one parameter, the one it acts on.
    let test = |effect: Box<dyn Effect>, name: &str, param: &str, max: f64, value: f64| Loaded {
        effect,
        main: None,
        info: Info {
            name: name.into(),
            params: vec![Param { id: 0, name: param.into(), min: 0.0, max, default: value, value }],
            all: vec![(0, param.into())],
            ..Info::default()
        },
    };
    match plugin {
        Plugin::Test(id) => match id.as_str() {
            "test-gain" => {
                Ok(Loaded { main: Some(Box::<TestEditor>::default()), ..test(Box::new(Gain), id, "Gain", 2.0, 1.0) })
            }
            "test-crash" => Ok(test(Box::new(Crash { blocks: 0 }), id, "Crash after", 1000.0, 0.0)),
            "test-hang" => Ok(test(Box::new(Hang), id, "Hang", 1.0, 0.0)),
            "test-slow" => Ok(test(Box::new(Slow), id, "Milliseconds", 100.0, 0.0)),
            "test-synth" => Ok(test(Box::<Synth>::default(), id, "Volume", 1.0, 1.0)),
            _ => Err(format!("unknown effect {id:?}")),
        },
        #[cfg(feature = "formats")]
        Plugin::Clap { file, id } => crate::clap::load(file, id, sample_rate, max_frames, picks),
        #[cfg(feature = "formats")]
        Plugin::Vst3 { file, id } => crate::vst3::load(file, id, sample_rate, max_frames, picks),
        #[cfg(all(feature = "formats", target_os = "macos"))]
        Plugin::Au { id } => crate::au::load(id, sample_rate, max_frames, picks),
        #[cfg(all(feature = "formats", not(target_os = "macos")))]
        Plugin::Au { .. } => Err("Audio Units only run on macOS".into()),
        #[cfg(not(feature = "formats"))]
        _ => {
            let _ = (sample_rate, max_frames);
            Err("this build hosts no plugin formats".into())
        }
    }
}

/// Multiplies by parameter 0.
struct Gain;

impl Effect for Gain {
    fn process(
        &mut self,
        input: [&[f32]; CHANNELS],
        output: [&mut [f32]; CHANNELS],
        params: &[f32],
        _: &Transport,
        _: &[Midi],
    ) {
        let g = params[0];
        for (i, o) in input.iter().zip(output) {
            for (x, y) in i.iter().zip(o.iter_mut()) {
                *y = x * g;
            }
        }
    }
}

/// Passes audio, then aborts once it has done parameter 0's number of
/// blocks (when that is above 0), like a plugin with a memory bug.
struct Crash {
    blocks: u32,
}

impl Effect for Crash {
    fn process(
        &mut self,
        input: [&[f32]; CHANNELS],
        output: [&mut [f32]; CHANNELS],
        params: &[f32],
        t: &Transport,
        _: &[Midi],
    ) {
        if params[0] > 0.0 && self.blocks as f32 >= params[0] {
            std::process::abort();
        }
        self.blocks += 1;
        Gain.process(input, output, &[1.0], t, &[]);
    }
}

/// Passes audio until parameter 0 goes above 0.5, then never returns.
struct Hang;

impl Effect for Hang {
    fn process(
        &mut self,
        input: [&[f32]; CHANNELS],
        output: [&mut [f32]; CHANNELS],
        params: &[f32],
        t: &Transport,
        _: &[Midi],
    ) {
        if params[0] > 0.5 {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
        Gain.process(input, output, &[1.0], t, &[]);
    }
}

/// Passes audio after sleeping parameter 0 milliseconds per block.
struct Slow;

impl Effect for Slow {
    fn process(
        &mut self,
        input: [&[f32]; CHANNELS],
        output: [&mut [f32]; CHANNELS],
        params: &[f32],
        t: &Transport,
        _: &[Midi],
    ) {
        if params[0] > 0.0 {
            std::thread::sleep(std::time::Duration::from_secs_f32(params[0] / 1000.0));
        }
        Gain.process(input, output, &[1.0], t, &[]);
    }
}

/// An instrument for tests: from a note-on it plays a steady level of
/// `key / 127 * volume` on both channels, from the note's frame, until its
/// note-off. It ignores its input.
#[derive(Default)]
struct Synth {
    key: Option<u8>,
}

impl Effect for Synth {
    fn process(
        &mut self,
        _: [&[f32]; CHANNELS],
        output: [&mut [f32]; CHANNELS],
        params: &[f32],
        _: &Transport,
        events: &[Midi],
    ) {
        let [l, r] = output;
        let mut next = events.iter().peekable();
        for f in 0..l.len() {
            while let Some(e) = next.next_if(|e| e.frame as usize <= f) {
                if let Some((_, key, _)) = e.as_note_on() {
                    self.key = Some(key);
                } else if let Some((_, key)) = e.as_note_off() {
                    if self.key == Some(key) {
                        self.key = None;
                    }
                }
            }
            let v = self.key.map_or(0.0, |k| k as f32 / 127.0 * params[0]);
            l[f] = v;
            r[f] = v;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(names: &[&str]) -> Vec<((), Param)> {
        names
            .iter()
            .enumerate()
            .map(|(i, n)| {
                ((), Param { id: i as u32 * 10, name: n.to_string(), min: 0.0, max: 1.0, default: 0.0, value: 0.0 })
            })
            .collect()
    }

    fn names(p: &[((), Param)]) -> Vec<&str> {
        p.iter().map(|(_, p)| p.name.as_str()).collect()
    }

    #[test]
    fn picks_take_their_knobs_then_the_makers_pages_then_the_rest() {
        let mut p = params(&["a", "b", "c", "d", "e"]);
        let all = arrange(&mut p, &[], &[]);
        assert_eq!(names(&p), ["a", "b", "c", "d", "e"]);
        assert_eq!(all[2], (20, "c".to_string()));
        // The maker's pages put d and b first.
        let mut p = params(&["a", "b", "c", "d", "e"]);
        arrange(&mut p, &[30, 10], &[]);
        assert_eq!(names(&p), ["d", "b", "a", "c", "e"]);
        // The DJ put e on knob 1 and a on knob 4.
        let mut p = params(&["a", "b", "c", "d", "e"]);
        let all = arrange(&mut p, &[30, 10], &[(0, 40), (3, 0)]);
        assert_eq!(names(&p), ["e", "d", "b", "a", "c"]);
        // Listed in the plugin's own order, whatever the knobs.
        assert_eq!(all.iter().map(|(id, _)| *id).collect::<Vec<_>>(), [0, 10, 20, 30, 40]);
        // A pick past the last knob, or of a parameter gone, closes up.
        let mut p = params(&["a", "b"]);
        arrange(&mut p, &[], &[(5, 0), (0, 99)]);
        assert_eq!(names(&p), ["b", "a"]);
    }
}
