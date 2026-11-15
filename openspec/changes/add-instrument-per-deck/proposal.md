# Proposal

Planned for: **v0.7**, after `rework-instruments` (issue #523)

The tasks still open moved to **v0.8 · From the booth to the studio** on 2026-11-12, so v0.7 ships with what is built.

## Why

DJ Sugar plays one instrument plugin at a time: the Instruments browser can put a synth on deck 3, but loading a second synth for deck 4 replaces the first and cold-starts its helper. Playing a bass on one deck and a pad on another, each from its own keyboard or sequencer track, is the natural next step for live playing over the mix.

The engine cost of several instruments is small in itself: each plugin runs in its own helper process, and a round trip costs about 0.03 to 0.06 ms of a 5.3 ms buffer (256 frames at 48 kHz) and about 3 MB per helper before the plugin's own memory. Two things made several heavy plugins costly, though: the audio thread waited for each plugin before handing the next one its block, so their DSP times added up inside the plugins' share of the buffer even though they run on different cores; and a loaded instrument got a block every buffer, even when silent and on no deck.

## What Changes

- **Plugins side by side.** At each effect slot, every deck's plugin gets its block before the rack waits for any of them, with the built-in effects running meanwhile. The instrument gets its block before the decks are worked out (key lock, stems) and is collected just before the mix. Plugins on different decks, and the instrument, now work at the same time.
- **Sleeping instruments.** An instrument that has played silence for a second, holds no notes, gets none and is on no deck gets no blocks until a note, a knob turn or a deck wakes it; it plays in the very buffer that wakes it.
- **One instrument per deck.** `[InstrumentN]` for each deck, each with its own plugin helper, notes, scale, chord, arpeggiator, knobs, presets, art and view. Loading a plugin on a deck no longer replaces the one on another deck.
- **MIDI from per deck.** Each instrument deck takes notes from its own device and channel (left out of `rework-instruments`).
- **App.** The instrument deck, 4-Deck strip, presets, knob pages, art, macro lanes, note roll and the Instruments browser's "On deck N" follow the deck's own instrument.

## Out of scope

- Several instruments layered on one deck.
- Plugin effects inside an instrument's own chain beyond the deck's FX units.
- A shared helper process for several plugins (each keeps its own, so one crash takes only its plugin down).

## Capabilities

### Modified Capabilities

- `instruments`: several instruments at once, one per deck; plugins processed side by side; idle instruments sleep.

## Impact

- Plugin host: `Connection::send` and `Connection::collect` beside `process`.
- Engine: `EffectRack::process_blocks` for the decks; the instrument split into `start_instrument` and `finish_instrument`; `Input::start` before the decks; then per-deck instrument state in place of the single instrument in `EffectRack`, `PluginMsg::Instrument` with a deck, a Vec of instrument states in the plugin supervisor.
- App: about 36 places that name `[Instrument1]` take the deck's group.
