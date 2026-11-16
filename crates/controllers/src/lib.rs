//! MIDI and HID controller support driven by controller mappings.
//!
//! Mixxx ships a mapping for each supported controller: a `.midi.xml` file
//! binding MIDI messages to controls or script functions, plus JavaScript
//! that implements anything beyond a direct binding (jog wheels, shift
//! layers, LEDs). This crate runs those files unchanged: the XML is read as
//! Mixxx reads it, and the scripts run in QuickJS against the same `engine`
//! and `midi` API, talking to the shared control registry. USB HID
//! controllers (the Traktor Kontrol range) run `.hid.xml` mappings
//! the same way, with `controller.sendOutputReport` and its kin; see [`hid`].

pub mod catalog;
mod controller;
pub mod edit;
pub mod export;
pub mod format;
pub mod hid;
mod host;
pub mod import;
pub mod layout;
pub mod learn;
mod manager;
pub mod mapping;
pub mod own;
pub mod pickup;
pub mod probe;
pub mod quick;
pub mod routing;
mod rows;
mod script;
pub mod support;
pub mod view;

use std::path::PathBuf;

pub use controller::Controller;
pub use host::{LAYER_GROUP, LAYER_KEYS};
pub use manager::{
    device_key, is_own_file, list_mappings, match_mapping, ConnectedController, Controllers,
    MappingChoice, NoteSink, Port,
};
pub use mapping::Mapping;

/// Whether `port` is a MIDI port the computer makes rather than a
/// controller: macOS's Network Session and IAC Driver, Windows' GS
/// Wavetable synth, Linux's Midi Through. They're never mapped on their
/// own, and the mapping wizard doesn't offer them.
pub fn is_system_port(port: &str) -> bool {
    const SYSTEM: [&str; 4] = [
        "Network Session",
        "IAC Driver",
        "Midi Through",
        "Microsoft GS Wavetable",
    ];
    SYSTEM.iter().any(|s| port.contains(s))
}

/// Where Mixxx's mappings are: `DJSUGAR_CONTROLLERS` if set, otherwise next
/// to the executable (`../Resources/controllers` in a macOS bundle,
/// `controllers/` beside it, or `../share/djsugar/controllers` in a Linux
/// install or AppImage), otherwise the repository's `res/controllers`.
pub fn default_mappings_dir() -> Option<PathBuf> {
    let valid = |p: &PathBuf| p.join(mapping::REQUIRED_SCRIPT).is_file();
    if let Some(dir) = std::env::var_os("DJSUGAR_CONTROLLERS") {
        return Some(PathBuf::from(dir)).filter(valid);
    }
    let mut candidates = Vec::new();
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(PathBuf::from))
    {
        candidates.push(exe_dir.join("../Resources/controllers"));
        candidates.push(exe_dir.join("controllers"));
        candidates.push(exe_dir.join("../share/djsugar/controllers"));
    }
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../res/controllers"));
    candidates.into_iter().find(valid)
}

#[cfg(test)]
mod system_port_tests {
    use super::is_system_port;

    #[test]
    fn tells_system_ports_from_controllers() {
        assert!(is_system_port("Network Session 1"));
        assert!(is_system_port("IAC Driver Bus 1"));
        assert!(is_system_port("Midi Through:Midi Through Port-0 14:0"));
        assert!(!is_system_port("DDJ-FLX4:DDJ-FLX4 MIDI 1 20:0"));
        assert!(!is_system_port("XONE:K2"));
    }
}
