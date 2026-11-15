## Purpose

Keeping hardware and other apps in time with DJ Sugar by sending MIDI clock, and keeping DJ Sugar in time with them by following it.

## ADDED Requirements

### Requirement: Clock out
With CLOCK OUT on in the MASTER panel (`[MidiClock],enabled`), DJ Sugar SHALL send MIDI timing clock (24 pulses per beat) to every output ticked in Settings › Controllers › MIDI Clock, at the tempo and beat of the deck Link follows. Pulses SHALL be timed for when that deck's audio is heard at the master output, plus the output's offset (-50 to +50 ms, default 0). With no deck playing, the clock SHALL keep the last tempo.

#### Scenario: Drum machine follows
- **WHEN** CLOCK OUT is on, a drum machine on a USB MIDI interface is ticked and deck 1 plays at 126 BPM
- **THEN** the drum machine shows 126 BPM and its kick lands on deck 1's beats

#### Scenario: Tempo change
- **WHEN** the DJ moves deck 1's tempo from 126 to 128 BPM
- **THEN** the drum machine follows smoothly without restarting its pattern

### Requirement: Start, stop and bars
When the deck the clock follows starts, DJ Sugar SHALL send Song Position and Start so the first pulse falls on its next downbeat. RESTART ON BAR (`[MidiClock],restart`) SHALL do the same while playing. When that deck stops, DJ Sugar SHALL send Stop, unless "Keep clock running" is ticked.

#### Scenario: Pattern starts on the bar
- **WHEN** the DJ presses RESTART ON BAR halfway through a bar
- **THEN** the drum machine's pattern restarts from step 1 on the next bar of the deck

### Requirement: Virtual clock port
On macOS and Linux, DJ Sugar SHALL offer a virtual MIDI output named "DJ Sugar Clock" that other apps on the same computer can take clock from.

#### Scenario: Another app on the Mac
- **WHEN** "DJ Sugar Clock" is ticked and a synth app takes clock from it
- **THEN** the app plays in time with the master deck

### Requirement: Steady timing
Pulses SHALL be scheduled ahead and sent at their time (a clock thread that sleeps until just before each pulse and spins the rest, or the operating system's timestamps where it has them: Core MIDI on macOS, the ALSA sequencer on Linux), so their spacing varies by less than 0.5 ms on an idle computer.

#### Scenario: Clock display stays still
- **WHEN** clock is sent at a steady 124 BPM for five minutes
- **THEN** a drum machine showing tempo to one decimal stays at 124.0

### Requirement: Clock in
An input chosen under Settings › Controllers › MIDI Clock SHALL be read for clock, Start, Stop and Song Position. Its tempo SHALL be smoothed (a line fitted through up to four bars of pulses, starting over from the last two beats when they show a tempo change) and shown to two decimals.

#### Scenario: Jittery source
- **WHEN** a hardware clock at 120 BPM arrives with pulses up to 2 ms early or late
- **THEN** DJ Sugar shows 120.00 BPM and decks following it don't wobble
