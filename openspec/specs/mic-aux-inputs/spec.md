# mic-aux-inputs Specification

## Purpose
The microphone and auxiliary inputs: their channels, the microphone's own effect unit, talkover ducking, and how they are set up and recorded.

## Requirements

### Requirement: Input channels
`[Microphone]` and `[Auxiliary1]` SHALL each take a device channel chosen in Settings › Audio › Inputs, through gain, a 3-band EQ and their fader, and SHALL be cueable in the headphones.

#### Scenario: Pick the mic input
- **WHEN** a device channel is chosen for Microphone
- **THEN** `[Microphone],input_configured` is 1 and the MIC panel is no longer dimmed

### Requirement: Talkover
The microphone SHALL be heard while `talkover` is on, switched by the MIC button in the top bar. The arrow on the MIC button SHALL open the MIC panel with the microphone only: ON, cue, level slider, gain and EQ, and one Mic FX slot (no routing of the Mic FX to other inputs there); a click outside closes it. The aux and instrument inputs are played from the Jam source on a deck. It SHALL join the master after the master's effects.

#### Scenario: Master FX don't touch the voice
- **WHEN** an echo is on the master and the DJ talks with talkover on
- **THEN** the voice is heard without the echo

### Requirement: Talkover ducking
`[Master],talkover_ducking` SHALL be Off, Auto (music goes down while the mic has signal) or Manual (while talkover is on), by "Ducking depth" (`duckStrength`, default -12 dB).

#### Scenario: Auto ducking
- **WHEN** ducking is Auto and the DJ speaks into the mic
- **THEN** the music drops by the ducking depth until they stop

### Requirement: Aux input
The aux input SHALL join the music while `main_mix` is on, as an extra channel such as a turntable, a phone or another mixer.

#### Scenario: Phone in
- **WHEN** a phone is on the aux input and its main_mix is on
- **THEN** it is heard in the master

### Requirement: Mic effects
The microphone SHALL have its own effect unit (the last unit, MIC FX) with Reverb, Echo and Filter, which can only be routed to the inputs.

#### Scenario: Reverb on the voice
- **WHEN** MIC FX reverb is turned on
- **THEN** the voice has reverb and the decks don't

### Requirement: Silent input warning
On macOS DJ Sugar SHALL ask for microphone access, and Settings › Audio › Inputs SHALL warn when a chosen input sends only silence.

#### Scenario: Permission missing
- **WHEN** an input is chosen but receives only silence
- **THEN** the setting shows a warning
