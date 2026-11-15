# Proposal

Planned for: **v0.6 · Buy it from the artist** (moved from v0.5 at the 0.5.0 release) (2026-11-02, from the ecosystem standards review)

## Why

Ableton Link keeps apps in time, but most hardware (drum machines, grooveboxes, synths with arpeggiators, Elektron and Roland boxes, modular through a clock converter) only follows MIDI clock. DJs who play live gear over their decks today buy a Link-to-MIDI box or run a second app to do it. Taking clock in lets a DJ lock the decks to a live act or a drum machine. Today DJ Sugar throws incoming clock bytes away (`crates/controllers/src/controller.rs`). Research: notes/ecosystem-standards-review.md in the project files.

## What Changes

- **MIDI clock out**, in the MASTER panel beside the Link switch: a CLOCK OUT switch and, in Settings › Controllers › MIDI Clock, the outputs to send to (any MIDI output, plus a virtual "DJ Sugar Clock" port on macOS and Linux for other apps on the same computer). The clock follows the same deck Link follows (the master deck), at 24 pulses per beat, timed for when the audio is heard, not when it is rendered.
- **Start, Stop and bar alignment:** Start is sent on the next downbeat when the master deck starts (or when the DJ presses RESTART ON BAR), with Song Position so pattern-based gear lands on the bar. Stop when the master deck stops, or keep running at the last tempo (setting).
- **Per-output offset** (-50 to +50 ms) to make up for gear that sounds late or early.
- **MIDI clock in:** choose an input in Settings › Controllers › MIDI Clock. It becomes a sync leader called **CLOCK**: decks with SYNC on follow its tempo and its beat (from Start and Song Position). The tempo is smoothed, so a jittery hardware clock still gives a steady BPM.
- **Link follow:** the same leader choice gains **LINK**, so the decks can follow a Link session (a drummer's or a live act's Live set) instead of leading it.

## Capabilities

### New Capabilities

- `midi-clock`: send MIDI clock to gear and other apps, and follow incoming MIDI clock

### Modified Capabilities

- `sync`: the sync leader can be the master deck (as now), the Link session or an incoming MIDI clock

## Out of Scope

- MIDI Time Code (MTC) and SMPTE/LTC timecode for show control.
- MIDI notes or CC out to gear (the Groove Box sending notes is a separate step).
- MIDI 2.0 clock and UMP.

## Difficulty

**Medium: 3 to 5 days, two PRs (out first, then in + Link follow).** Sending the bytes is trivial; keeping them steady is the work. `midir` sends immediately with no timestamps, so a sleeping thread alone gives 1 ms or more of jitter, which drum machines' tempo displays show as wobble. Plan: on macOS, send through Core MIDI with future timestamps so the OS schedules each pulse; on Linux, the ALSA sequencer's scheduled queue; on Windows, a real-time thread with a high-resolution timer and a short spin before each pulse. Clock in needs a smoothing filter (a line fit over the last two beats of pulses) and tests with a real jittery source. Testing needs one piece of hardware with a clock display (any drum machine or a phone app over a MIDI interface).

## Performance

Nothing new runs inside the audio callback beyond publishing one small "beat clock" snapshot per buffer (host time the buffer is heard, beat, tempo), written to three atomics the way Link's state is already captured. The clock thread reads that snapshot, schedules the next pulses ahead and sleeps; at 128 BPM that's 51 pulses a second, negligible CPU. Clock in runs on the MIDI input thread and hands a tempo and phase to the engine through the same lock-free path SYNC already uses for Link. No locks are shared with the audio thread, and the engine works the same when clock is off.

The beat clock snapshot is shared with add-osc-control (beat messages) and with the Groove Box clock.

## Impact

engine (beat clock snapshot after `sync`, leader choice in `sync.rs`), controllers (clock port reader instead of dropping `0xF8`-`0xFC`, Ignore settings for the clock port), new clock sender module with Core MIDI / ALSA sequencer / Windows back ends, app (MASTER panel CLOCK OUT and RESTART ON BAR, Settings › Controllers › MIDI Clock, leader choice), controls `[MidiClock],enabled`, `[MidiClock],restart`, `[Master],sync_leader_source`, supported-controls.txt, user guide (Sync page: Link and MIDI clock).
