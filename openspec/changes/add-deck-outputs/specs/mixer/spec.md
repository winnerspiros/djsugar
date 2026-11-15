## ADDED Requirements

### Requirement: External mixer mode
Settings › Audio › Outputs SHALL offer where the mixing happens (`[Master],external_mixer`): **DJ Sugar** (the default) or **an external mixer**. With an external mixer, each deck SHALL go to its own output at the Pre-fader point with EQ, filter and channel fader treated as flat; deck FX, pads, loops, stems and sync SHALL keep working. The on-screen mixer (EQ, faders, crossfader, master section and headphone cue buttons) SHALL be replaced by slim deck strips with level meters in 2-Deck and 4-Deck layouts. Mixer controls sent by controllers SHALL be ignored without error.

#### Scenario: Club mixer
- **WHEN** external mixer mode is on and decks 1 and 2 go to the interface's outputs 1-2 and 3-4, wired to a DJM's channels 1 and 2
- **THEN** each DJM channel gets one deck at full level, its EQ and faders shape the mix, and DJ Sugar shows no mixer

#### Scenario: Controller sends EQ
- **WHEN** a mapped controller turns deck 1's high EQ knob in external mixer mode
- **THEN** deck 1's output does not change

### Requirement: What external mixer mode turns off
In external mixer mode, Automix and the master limiter SHALL be off, each with a line saying it needs DJ Sugar's mixer. The master, headphone and booth routes MAY be empty. Mic and Aux inputs SHALL not be mixed. The sampler and Groove Box SHALL use their own outputs, or Deck 1's when they have none. Recording SHALL offer an input (such as the mixer's record out on Aux) as its source.

#### Scenario: Automix unavailable
- **WHEN** the DJ opens the Automix panel in external mixer mode
- **THEN** it says Automix needs DJ Sugar's mixer and its switch is disabled
