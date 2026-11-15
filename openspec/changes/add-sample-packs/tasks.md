# Tasks

## 1. Engine

- [x] 1.1 16 sampler slots, `[App],num_samplers` = 16; Default DJ Sample Pack sounds on slots 1-8
- [x] 1.2 Load audio files onto a slot off the audio thread (decode, resample, stereo, 60-second limit, errors back to the app)
- [x] 1.3 `[SamplerN],play_mode` (one shot, hold, on/off, loop) applied to `cue_gotoandplay` press and release; `repeat`, `cue_gotoandstop`; short fades
- [x] 1.4 Level on `pregain` (silent to +6 dB)
- [x] 1.5 `[ChannelN],sampler_page`; DDJ-FLX4 sampler pads and LEDs follow the deck's page
- [x] 1.6 Regenerate website/book/supported-controls.txt

## 2. Packs and pads

- [x] 2.1 Pack store in `<config>/samples/` (`pack.json`), Default pack, overrides in `settings.json` under `sampler`
- [x] 2.2 SAMPLER page: 1/2 page button beside the pads, SAMPLE PACK strip under the pads with pack menu (covers, +, Get more Packs placeholder)
- [x] 2.3 Edit mode: level knob and "…" on each pad; right-click menu (Choose Sample, Load file, Rename, Play mode, Reset, Clear); drag a pad onto another to swap them
- [x] 2.4 Choose Sample popover with headphone preview and lengths
- [x] 2.5 Drag and drop from the file manager and the track list onto pads; missing-file state and Locate file
- [x] 2.6 Update the user guide (website/book/src/guide/decks.md)

## 3. Share packs

- [x] 3.1 New pack (built with phase 2)
- [x] 3.1b Rename, delete (to `.deleted`, two clicks) and show a pack in the file manager
- [x] 3.2 `.djsugarpack` export (the pads as they are, to Downloads; built-in sounds stay built in) and import (menu and drop on a pad; only files pack.json names, by plain name, 1 GB cap)

## 3b. Audio thread (v0.5)

- [x] 3b.1 Levels read once per buffer and glide (about 5 ms) while a pad plays; a buffer with no pad sounding skips the sixteen players

## 4. Record and sync

- [ ] 4.1 Record New Sample from a deck, the mic or the master into "My Recordings"; loop-length capture
- [ ] 4.2 Loop samples follow the master tempo and start quantised

## 5. Get more Packs

- [ ] 5.1 Design the dialog and the pack source (see design.md), then build it

## Workflow follow-up

- Archive the change once its last PR is merged, so the main specs take its requirements.
