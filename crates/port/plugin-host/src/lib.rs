//! Audio plugins run outside DJ Sugar, in `djsugar-plugin-host` helper processes, so a plugin that crashes, hangs or runs slow costs one effect slot, never the decks. - [`shm`]: the block of memory DJ Sugar and a helper share. - [`client`]: DJ Sugar's side. [`spawn`] starts a
//! helper; the audio thread hands it blocks with [`Connection::process`], which plays the block dry whenever the helper misses its deadline or is gone. - [`policy`]: restart once, then disable; hangs count as crashes; a plugin that stays over its CPU budget is switched off. -
//! [`effects`]: what a helper can run. For now only test effects (gain, crash, hang, slow) that prove the isolation; CLAP, VST3 and AU plugins follow, as planned in the `add-audio-plugin-hosting` change. - [`window`] (in the helper): the window a plugin's own editor opens in.

#[cfg(all(feature = "formats", target_os = "macos"))]
pub mod au;
#[cfg(feature = "formats")]
pub mod clap;
pub mod client;
pub mod descriptor;
pub mod effects;
pub mod policy;
pub mod presets;
pub mod shm;
#[cfg(feature = "formats")]
pub mod vst3;
#[cfg(feature = "formats")]
pub mod window;

pub use client::{
    presets, scan, scan_audio_units, spawn, Connection, Editor, Helper, Options, Outcome, ScanError, Status, Transport,
};
pub use descriptor::Descriptor;
pub use effects::{Info, Param, Plugin};
pub use policy::{Action, Limits, Policy};
pub use shm::Midi;
