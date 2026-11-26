//! Hardware controllers. Each MIDI or HID device with a mapping drives the engine's controls; what a mapping asks of the
//! library (move the selection, load the selected track) is handed to the app here.

use std::sync::Arc;

use djsugar_control::{ControlHandle, ControlRegistry};
use djsugar_controllers::Controllers;

/// What a controller asked of the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ask {
    /// Move the selection this many rows down.
    Move(isize),
    /// Load the selected track on this deck.
    Load(usize),
}

/// Knobs that send steps, and which way each turns the selection.
const KNOBS: [(&str, &str); 4] = [
    ("[Library]", "MoveVertical"),
    ("[Library]", "ScrollVertical"),
    ("[Playlist]", "SelectTrackKnob"),
    ("[Library]", "SelectTrackKnob"),
];
/// Buttons that move the selection one row.
const BUTTONS: [(&str, &str, isize); 6] = [
    ("[Library]", "MoveDown", 1),
    ("[Library]", "MoveUp", -1),
    ("[Playlist]", "SelectNextTrack", 1),
    ("[Playlist]", "SelectPrevTrack", -1),
    ("[Library]", "SelectNextTrack", 1),
    ("[Library]", "SelectPrevTrack", -1),
];

pub struct Desk {
    devices: Option<Controllers>,
    knobs: Vec<ControlHandle>,
    buttons: Vec<(ControlHandle, isize)>,
    loads: Vec<ControlHandle>,
}

impl std::fmt::Debug for Desk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Desk").field("watching", &self.devices.is_some()).finish()
    }
}

/// What `h` has counted since it was last taken.
fn take(h: &ControlHandle) -> isize {
    let n = h.get();
    if n != 0.0 {
        h.set(h.get() - n);
    }
    n as isize
}

impl Desk {
    /// The library's controls, with devices watched when the mappings are found.
    pub fn start(reg: &Arc<ControlRegistry>, decks: usize) -> Self {
        // Beside the samples when the app is told where its files are, else where the mappings are looked for.
        let told = std::env::var_os("DJSUGAR_RES").map(|d| std::path::PathBuf::from(d).join("controllers"));
        let dir = told.filter(|d| d.is_dir()).or_else(djsugar_controllers::default_mappings_dir);
        let devices = dir.map(|dir| Controllers::start(reg.clone(), dir, Default::default(), Default::default()));
        Self { devices, ..Self::controls(reg, decks) }
    }

    /// The library's controls alone.
    fn controls(reg: &Arc<ControlRegistry>, decks: usize) -> Self {
        let knobs = KNOBS.iter().map(|(g, k)| reg.free(g, k)).collect();
        let buttons = BUTTONS.iter().map(|(g, k, by)| (reg.free(g, k), *by)).collect();
        let loads = (1..=decks).map(|d| reg.free(&format!("[Channel{d}]"), "LoadSelectedTrack")).collect();
        Self { devices: None, knobs, buttons, loads }
    }

    /// What the controllers asked for since the last call.
    pub fn poll(&self) -> Vec<Ask> {
        let turned: isize = self.knobs.iter().map(take).sum();
        let pressed: isize = self.buttons.iter().map(|(h, by)| take(h) * by).sum();
        let mut asks = Vec::new();
        if turned + pressed != 0 {
            asks.push(Ask::Move(turned + pressed));
        }
        asks.extend(self.loads.iter().enumerate().filter(|(_, h)| take(h) > 0).map(|(d, _)| Ask::Load(d)));
        asks
    }

    /// Each device plugged in and what it runs, for the settings page.
    pub fn ports(&self) -> Vec<(String, String)> {
        let Some(devices) = &self.devices else { return Vec::new() };
        let ports = devices.ports().into_iter().filter(|p| !djsugar_controllers::is_system_port(&p.name));
        ports
            .map(|p| {
                let runs = match (p.problem, p.running, p.keys) {
                    (Some(why), ..) => why,
                    (None, Some(mapping), _) => mapping,
                    (None, None, true) => "Keyboard".into(),
                    (None, None, false) => "No mapping".into(),
                };
                (p.name, runs)
            })
            .collect()
    }

    /// How many mappings the app knows.
    pub fn known(&self) -> usize {
        self.devices.as_ref().map_or(0, |d| d.mappings().len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_and_presses_become_asks_once() {
        let reg = ControlRegistry::new();
        let desk = Desk::controls(&reg, 2);
        reg.get("[Library]", "MoveVertical").unwrap().set(3.0);
        reg.get("[Playlist]", "SelectPrevTrack").unwrap().set(1.0);
        reg.get("[Channel2]", "LoadSelectedTrack").unwrap().set(1.0);
        assert_eq!(desk.poll(), vec![Ask::Move(2), Ask::Load(1)]);
        assert_eq!(desk.poll(), vec![]);
    }

    /// A stock mapping against the engine's own controls: its play button starts a deck, its browse knob and load
    /// button reach the library.
    #[test]
    fn a_stock_mapping_drives_the_engine_and_the_library() {
        let reg = ControlRegistry::new();
        let _engine = djsugar_engine::engine::Engine::new(&reg, 48_000.0);
        let desk = Desk::controls(&reg, 2);
        let dir = djsugar_controllers::default_mappings_dir().unwrap();
        let mapping = djsugar_controllers::Mapping::load(&dir.join("Pioneer-DDJ-FLX4.midi.xml")).unwrap();
        let mut c = djsugar_controllers::Controller::new("DDJ-FLX4", mapping, reg.clone()).unwrap();
        assert!(c.errors().is_empty(), "{:?}", c.errors());

        let play = reg.get("[Channel2]", "play").unwrap();
        c.receive(&[0x91, 0x0B, 0x7F]);
        c.receive(&[0x91, 0x0B, 0x00]);
        assert!(play.get_bool());

        // The browse knob one step clockwise, then LOAD on deck 1.
        c.receive(&[0xB6, 0x40, 0x01]);
        c.receive(&[0x96, 0x46, 0x7F]);
        c.receive(&[0x96, 0x46, 0x00]);
        assert_eq!(desk.poll(), vec![Ask::Move(1), Ask::Load(0)]);
    }
}
