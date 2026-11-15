# Proposal

Planned for: **v0.5 · Stems and performance** (asked for in issue #60; phases 1 and 2 can be built earlier, they don't depend on anything else in v0.5)

## Why

The sampler plays eight sounds that are built into the app and can't be changed. Every DJ has their own drops, air horns, rewinds and name tags, and issue #60 (a dancehall DJ) says that without them they need a separate sampler app beside DJ Sugar. djay, VirtualDJ, Serato, rekordbox and Mixxx all let DJs load their own samples; djay and rekordbox group them in packs and banks.

There is also a bug to fix on the way: the DDJ-FLX4 mapping plays `[Sampler1]` to `[Sampler16]` (left deck pads 1-4 and 9-12, right deck 5-8 and 13-16, like rekordbox), but DJ Sugar only has eight samplers, so the lower pad row on the controller does nothing and the right deck's pads don't play what the screen shows.

## What Changes

- 16 sampler slots instead of 8, in two pages of 8 on each deck with a 1/2 page button on the side of the pads. The DDJ-FLX4 pads follow the deck's page.
- **Sample packs.** A pack is a named set of up to 16 samples. The current sounds become the read-only **Default DJ Sample Pack**. A SAMPLE PACK menu under the pads switches packs, with a + to make a new pack or import one, and **Get more Packs** at the bottom (its dialog is designed later).
- **Choose Sample** popover on a pad: a pack menu, the pack's samples with a preview button and length, **From file…**, and later **Record New Sample** from deck 1, the mic or deck 2.
- Load a file onto a pad by dragging it from Finder or the track list, or with right-click › Load file…. WAV, AIFF, FLAC, MP3, M4A and OGG.
- Per pad: a level knob, a name you can rename, a play mode (one shot, hold, on/off, loop), Reset to pack sample and Clear.
- Pad settings (sample, name, level, mode) are saved per pack in `settings.json`; pack files are never changed by editing pads.
- Export a pack (with its files) as one `.djsugarpack` file and import it on another computer.
- Later phases: record a sample from a deck, the mic or the master; loop samples that stay in sync with the master deck; Get more Packs.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `performance-pads`: the sampler gains 16 slots, sample packs, own samples, per-pad level, name and play mode, and recording.

## Out of Scope

- A sample editor (trim, fades, waveform editing). Samples play from their start to their end.
- Video samples (VirtualDJ) and stem-swap banks.
- The Get more Packs dialog and where its packs come from: designed later (see design.md for the options found so far).
- Importing VirtualDJ `.vdjsample` banks: the format isn't documented; looked at later with the VirtualDJ import add-on.
- A pattern sequencer (separate change, `add-groove-box`).

## Impact

engine sampler (16 slots, file loading off the audio thread, stereo, play modes), app (pack store, settings, Choose Sample popover, pad edit mode, drag and drop), DDJ-FLX4 mapping (pads follow the deck's sampler page), user guide (performance pads page), supported-controls.txt.
