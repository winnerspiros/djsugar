# effects Specification

## Purpose
Beat-synced effects: the deck effect units, the Beat FX unit, the pad FX units and the microphone's own unit, with the effects they can load.

## Requirements

### Requirement: Effect list
DJ Sugar SHALL offer these effects: Echo, Reverb, Filter, Flanger, Bitcrusher, Phaser, Gate, Roll, Echo Out, Brake, Noise and Spiral, each with up to four parameters.

#### Scenario: Effects listed
- **WHEN** an effect slot's menu is opened
- **THEN** it lists the twelve effects by name

### Requirement: Effect units
There SHALL be ten units of three slots each (`[EffectRack1_EffectUnitN]`): units 1 to 4 are the decks' FX, unit 5 the Beat FX, 6 to 9 the pad FX, and unit 10 the microphone's. Each unit SHALL have `enabled`, `mix` (dry/wet), `super1` (meta knob) and per-slot effect choice and parameters.

#### Scenario: Deck FX on deck 1
- **WHEN** unit 1 is routed to deck 1 and its first slot is turned on
- **THEN** deck 1 is heard through that effect

### Requirement: Routing
A unit SHALL be routed per target (`group_[ChannelN]_enable`, `group_[Master]_enable`). The microphone unit SHALL only process the microphone and aux inputs, and the other units only the decks and the main mix.

#### Scenario: Mic unit can't take a deck
- **WHEN** the mic unit's routing to a deck is turned on
- **THEN** the deck is not processed by it

### Requirement: Beat-synced timing
Time-based effects SHALL follow the routed deck's beat length. Each unit SHALL have `bpm`, `bpm_tap` (average of four taps, a gap over 2 s starts again) and `bpm_auto` (follow the deck).

#### Scenario: Tap tempo
- **WHEN** TAP is pressed four times at 0.5 s intervals
- **THEN** the unit's tempo is 120 BPM and AUTO turns off

### Requirement: Release tails
A unit's `release` SHALL let the effect's tail ring on for 8 beats after it is turned off.

#### Scenario: Echo tail
- **WHEN** an echo is turned off with release on
- **THEN** its repeats fade out over about 8 beats instead of stopping

### Requirement: Beat FX
Two rows under the mixer's crossfader SHALL show the Beat FX unit Pioneer style: one effect, a channel selector (1, 2, 1&2, MST), LEVEL/DEPTH, beat length, TAP and AUTO, ON/OFF. The DDJ-FLX4's BEAT FX section SHALL drive it.

#### Scenario: FLX4 Beat FX
- **WHEN** BEAT FX ON is pressed on a DDJ-FLX4
- **THEN** unit 5 turns on and the on-screen ON lights

### Requirement: Deck FX defaults
Each deck's own unit (units 1 to 4) SHALL start with Reverb, Echo and Gate in its three slots.

#### Scenario: Fresh start
- **WHEN** DJ Sugar starts
- **THEN** each deck's FX show Reverb, Echo and Gate, all off

### Requirement: Pad FX
The PAD FX 1 pad page SHALL play an effect while a pad is held: Echo, Reverb, Low Pass, High Pass, Flanger, Phaser, Crush and Echo Max, using that deck's pad FX unit. PAD FX 2 SHALL do the same with Roll, Gate, Echo Out, Brake, Noise, Spiral, Gate Fast and Reverb Max. The DDJ-FLX4's PAD FX1 and PAD FX2 pads SHALL play the same effects.

#### Scenario: Hold for reverb
- **WHEN** the REVERB pad is held
- **THEN** the deck plays through reverb until the pad is released
