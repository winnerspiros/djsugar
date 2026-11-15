# Proposal

Planned for: **v0.6 · Buy it from the artist** (moved from v0.5 at the 0.5.0 release) (2026-11-02, from the ecosystem standards review)

## Why

Open Sound Control is how the rest of the live world talks: TouchOSC and Open Stage Control on tablets, Resolume, VDMX, TouchDesigner and Synesthesia for visuals, QLab for shows, many lighting desks, Max and Pure Data. DJ Sugar's controls already have names (`[Channel1],play`), so every control can have an OSC address for free, and DJ Sugar knows things visuals and lights want (beats, bars, phrase changes, the track playing). Research: notes/ecosystem-standards-review.md in the project files.

## What Changes

- **Settings › Controllers › OSC** (off at first): a receive port (default 9000), a list of send targets (address and port), "Only this computer" (on at first) and "Send only, ignore incoming".
- **Every control by name:** `[Channel1],play` is `/channel1/play`, `[Master],crossfader` is `/master/crossfader`, `[EffectRack1_EffectUnit1],mix` is `/effectrack1_effectunit1/mix` (group lower-cased, brackets dropped, since `[` `]` are OSC pattern characters). A float or int sets the value; a message with no value asks for it.
- **Feedback:** changes are sent to the targets for controls a target has set, asked for or subscribed to (`/subscribe /channel1/*`), at most 30 times a second per control.
- **Events for visuals and lights:** `/beat` (beat in bar, bar, BPM) on every beat of the master deck, `/phrase` (name and length) on phrase changes, `/deck/N/track` (title, artist, BPM, key) on load, `/deck/N/cue` when a hot cue fires.
- The OSC service is advertised on the network (Bonjour, `_osc._udp`) so TouchOSC and others find it.
- A starter TouchOSC layout for two decks in the user guide.

## Capabilities

### New Capabilities

- `osc-control`: control DJ Sugar and follow what it does over Open Sound Control

### Modified Capabilities

None.

## Out of Scope

- Controller mappings that send OSC from MIDI controllers' buttons.
- OSC over TCP and OSC 1.1 framing.
- A built-in layout editor (TouchOSC and Open Stage Control do that).

## Difficulty

**Medium: 3 to 5 days, one PR.** The `rosc` crate (MIT/Apache) parses and builds messages. The control registry already lets controller threads set and read controls safely, so OSC is one more such thread. The work is the address table, subscription and throttled feedback, the event messages, and a guide page with a TouchOSC layout.

## Performance

Nothing runs on the audio thread. Incoming messages set controls the same way MIDI does (atomic writes the engine reads at the start of each buffer). Feedback is read on the OSC thread at up to 30 Hz, only for subscribed controls. `/beat` and `/phrase` come from the beat clock snapshot shared with add-midi-clock, read on the OSC thread. With OSC off, nothing listens and nothing is read.

## Impact

new `crates/osc` (an OSC thread with `rosc`, depending only on the control registry), engine (beat clock snapshot shared with add-midi-clock; phrase change event), app (Settings › Controllers › OSC), `mdns-sd` advert shared with the phone remote's planned Bonjour step, user guide (new OSC page with an address list generated from supported-controls.txt and a TouchOSC layout).
