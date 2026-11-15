## ADDED Requirements

### Requirement: Looper deck
The Instruments browser SHALL offer a Looper that loads onto a deck. A looper deck SHALL hold four layers, each with a state (empty, armed, recording, overdubbing, playing, muted), a level and mute, and SHALL play through the deck's channel so its fader, EQ, effects and crossfader apply and the main-mix recording includes it. Deck PLAY and CUE SHALL start and stop all layers.

#### Scenario: Looper through a deck
- **WHEN** the user drags Looper onto deck 3 and records a layer
- **THEN** the layer plays through deck 3, and moving deck 3's fader changes its level

### Requirement: Layer tap cycle
Tapping an empty layer SHALL arm it and start recording on the next bar of the sync leader. Tapping a recording layer SHALL end the pass on the next bar and play the layer, or overdub it when the After recording setting is Overdub. Tapping a playing layer SHALL overdub it; holding a layer SHALL clear it. UNDO SHALL remove the last pass and REDO SHALL bring it back.

#### Scenario: Record then overdub
- **WHEN** a deck plays at 124 BPM, the user taps layer 1 during bar 7 and again during bar 10, with length set to Free and After recording set to Play
- **THEN** layer 1 records bars 8 to 10 and plays them as a 4-bar loop locked to the deck's bars

#### Scenario: Undo an overdub
- **WHEN** layer 2 is overdubbed and UNDO is pressed
- **THEN** layer 2 plays as it was before the overdub

### Requirement: Beat-locked lengths
A layer's length SHALL be 1, 2, 4, 8 or 16 bars of the sync leader, or free for the first layer when no deck plays, in which case its length SHALL set the tempo and the looper SHALL become the leader. Every layer SHALL be a whole multiple of the shortest layer and SHALL stay in phase with it. Layers SHALL follow tempo changes with key lock.

#### Scenario: Free first loop sets the tempo
- **WHEN** no deck plays and the user records a free first layer of 1.935 seconds and picks 4 beats
- **THEN** the tempo becomes 124 BPM, the looper is the leader and a deck synced to it follows

### Requirement: Input picking
Each layer SHALL record from a chosen input: an auxiliary input, the microphone, an instrument deck, the Groove Box, any deck, or the main mix. The input SHALL be heard while recording.

#### Scenario: Layer a synth over a grabbed loop
- **WHEN** layer 1 holds 4 bars captured from the main mix and the user records layer 2 from Aux 1 with a MiniFreak plugged in
- **THEN** layers 1 and 2 play together in time, and the MiniFreak is heard while layer 2 records

### Requirement: Capture
The looper SHALL keep the recent past of its picked input, and CAPTURE SHALL put the last 1, 2, 4 or 8 bars, ending on the latest bar line, into the next free layer without recording having been started.

#### Scenario: Grab the moment after it happened
- **WHEN** the main mix plays a vocal hook during bars 33 to 36 and the user presses CAPTURE with 4 bars during bar 37
- **THEN** the next free layer holds bars 33 to 36 and plays them from bar 37 on

### Requirement: Saving loops
Save loop SHALL write a layer, or all layers mixed, to the Inbox as a WAV file tagged with its BPM and key.

#### Scenario: Keep a loop
- **WHEN** the user picks Save loop › All layers on a looper deck at 124 BPM
- **THEN** a WAV of the mixed layers appears in the Inbox with BPM 124

### Requirement: Latency compensation
DJ Sugar SHALL measure each input's round-trip latency on request in Settings › Audio and SHALL move recorded audio back by it, so a loop recorded from an input lines up with the beat it was played to. While a looper records from an input, the input's queue SHALL keep in time by stretching rather than by dropping or padding audio.

#### Scenario: A played loop lands on the beat
- **WHEN** the round trip measures 11 ms and the user plays a hit exactly on bar 9's downbeat while recording
- **THEN** the hit sits on the layer's downbeat, not 11 ms after it

### Requirement: Looper controls
Every looper action SHALL be a control (`[LooperN]` layer taps, levels, mutes, clears, capture, undo, redo, length) that controllers, keyboard shortcuts and OSC can use. Mappings SHALL ship for the Maschine Mikro MK3, Xone K2 and DDJ-FLX4.

#### Scenario: Looping from the Mikro
- **WHEN** a looper deck has focus and the user taps pad 2 on a Maschine Mikro MK3
- **THEN** layer 2 arms, and the pad lights to show its state
