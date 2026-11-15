## ADDED Requirements

### Requirement: Instrument input
DJ Sugar SHALL offer an instrument input that loads an enabled CLAP, VST3 or (macOS) AU instrument plugin, out of process like effect plugins, and plays it as a channel with its own fader, gain, EQ, cue and the microphone's effect unit. Notes SHALL come from a MIDI keyboard, the Keyboard pad page or a controller mapping.

#### Scenario: Play a synth over the mix
- **WHEN** an instrument is loaded and the DJ plays notes on a MIDI keyboard
- **THEN** the instrument is heard through its channel, in time with the master tempo
