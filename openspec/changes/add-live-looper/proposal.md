# Proposal

Planned for: **v0.7**, after the instrument deck work in `rework-instruments` (live channels with TRIM, PAN and the safety limiter)

The tasks still open moved to **v0.8 · From the booth to the studio** on 2026-11-12, so v0.7 ships with what is built.

## Why

DJ Sugar can play instruments, the Groove Box and interface inputs through decks, but nothing it plays can be kept and built on in the moment. A DJ who hears a great four bars in the mix has no way to grab them, and someone playing a synth over the mix can't layer a bass line, then a pad, then a lead on top of each other. Traktor's Loop Recorder and loop stations (Boss RC-505 style) show the simple model DJs reach for: a few layers, record, overdub, play, undo, locked to the beat. Loopy Pro shows the far end of the range (a clip canvas with groups, follow actions and scenes), which is a whole app of its own, overlaps the DAW many DJs run alongside, and was assessed and left out of the Instruments rework. It already plays along with DJ Sugar over Ableton Link.

## What Changes

- **Looper on a deck.** A **Looper** item in the Instruments browser, dropped on a deck like the Groove Box. The deck shows four stacked layers in place of a track: each with a state light (empty, armed, recording, overdubbing, playing, muted), a level and mute. Deck PLAY and CUE start and stop all layers. The looper plays through the deck's channel, so fader, EQ, filter, FX, crossfader, the main-mix recording and the control log all work with it.
- **Tap cycle per layer.** Tap arms recording from the next bar; tap again ends the pass and the layer plays, or goes straight into overdub when **After recording: Overdub** is chosen (Settings, default Play). Tap a playing layer to overdub, hold to clear. UNDO and REDO step through the passes of the last layer touched.
- **Length and sync.** 1, 2, 4, 8 or 16 bars, locked to the sync leader's bars. With no deck playing, the first layer can be free: its length sets the tempo and the looper becomes the leader. Every layer is a whole multiple of the shortest and stays phase-locked. Loops keep their BPM and follow tempo changes through the key-lock stretcher.
- **Input picker.** Each layer records from Aux 1 to 3, Mic, an instrument deck, the Groove Box, any deck, or the main mix (resample). The input is heard through while recording.
- **Capture.** The looper keeps a ring of the recent past of the picked input, so CAPTURE puts the last 1, 2, 4 or 8 bars into the next free layer without pressing record first. Grabbing from the mix and then layering instruments on top is the core flow.
- **Keep it.** Save loop writes a layer, or all four mixed, to the Inbox as a WAV tagged with its BPM and key, so it comes back as a track or a sample.
- **Timing.** Recorded audio is moved back by the input's round-trip latency, measured once in Settings › Audio (a click played out and heard back). The input queue stretches gently instead of dropping or padding audio while a looper records from it, so long loops stay in time without clicks.
- **Controllers.** Looper controls (`[LooperN],layer1_tap` … `layer4_tap`, `layerN_volume`, `layerN_mute`, `layerN_clear`, `capture`, `undo`, `redo`, `length`) are controls like any other, mapped for the Maschine Mikro MK3 (pads 1-4 layers, REC capture, ERASE undo, strip length), Xone K2 (a column per layer) and a DDJ-FLX4 pad page; KeyLab mk3 transport drives the looper on the focused deck. They are controls, not notes, so they stay clear of the rule that DJ controllers never play instruments.
- **Waveform.** The looper deck's big waveform shows each layer as a lane with the play head going round.

## Out of scope

- A clip grid or clip launcher with groups, scenes and follow actions (Loopy Pro style).
- More than four layers, or layers spread across several decks.
- Loops into sampler pads (Capture to the Inbox covers keeping a loop; pads can load it from there).
- Starting on the leader's next phrase, ×2 and ÷2, reverse, half speed and fades: later steps.

## Capabilities

### New Capabilities

- `live-looper`: a four-layer looper on a deck with record, overdub, undo, capture of the recent past, beat-locked lengths, input picking, saving loops, latency compensation and controller mappings.

## Impact

- Engine: a looper source per deck (`crates/engine/src/looper.rs`), a capture ring per input fed off the audio thread like the recorder's, latency offset per input, input queue stretching in `inputs.rs`.
- App: Looper row in the Instruments browser, looper deck UI in 2-Deck and 4-Deck, Settings › Audio latency measure, Save loop to Inbox.
- Controllers: mappings for Mikro MK3, Xone K2, DDJ-FLX4 and KeyLab mk3.
- Memory: about 25 MB per input for 32 bars at 120 BPM (48 kHz stereo); capture rings are fixed size, set in Settings.
