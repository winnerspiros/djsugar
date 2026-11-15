# Design

## Research: how other apps do it

| App | Slots | Packs / banks | Per sample | Load your own | Record |
|---|---|---|---|---|---|
| djay Pro | 16 (8 x 2) | Sample packs from a menu above the pads, + to add, Get more Packs (store, mostly PRO). Pencil opens edit mode | Level knob, name, "…" menu | Choose Sample popover: pack menu, list with preview and length | Record New Sample from deck 1, mic, deck 2 |
| VirtualDJ | up to 64 per bank | Banks (XML file + folder of `.vdjsample` files in `Documents/VirtualDJ/Sampler`), unlimited, in folders, lockable | Volume, group, own trigger mode per group | Drag from the file list | From the pads (drop, manual or 4-32 beat loop), a deck, the master or the mic, into a Recordings folder |
| Serato DJ Pro | 4 to 8 (SP-6 style) | Banks A-D | Gain, play mode: Trigger, Hold, On/Off; Repeat; Sync | Drag onto a slot | — |
| rekordbox | 16 per bank, 4 banks + 4 OSC slots | Banks; free Sampler Plus Pack imported via File › Import | Gain, mode: One Shot, Gate, Loop; Sync | Drag onto a slot | 4-bar sequencer |
| Mixxx | Up to 64 sampler decks | Save and Load Sampler Bank (XML listing file paths) | Each sampler is a mini deck (pitch, sync, repeat, gain) | Drag or load the selected track | — |

What we take from them:

- **djay's look and flow** (as in reference screenshots): SAMPLE PACK menu with covers, + and Get more Packs; pencil for edit mode with level knobs and "…"; the Choose Sample popover with preview buttons, lengths and Record New Sample. It matches DJ Sugar's djay-style look.
- **Serato/rekordbox play modes**: One shot (Serato's Trigger), Hold (Gate), On/Off, Loop (rekordbox). That covers issue #60's one-shot / hold / loop.
- **16 slots, like rekordbox and djay**, shown as two pages of 8 on each deck with a 1/2 page button beside the pads (2026-10-27), so a deck's 8 pads always read in order.
- **VirtualDJ's recordings folder**: recordings land in their own "My Recordings" pack.
- Not taken: VirtualDJ's 64-slot banks (too many for pads), Mixxx's samplers-as-full-decks (pitch, EQ per sampler; more than drops need).

## Decisions

### Packs and pads

- A **pack** is a folder `<config>/samples/<pack-id>/` with `pack.json` and its audio files:

  ```json
  {
    "name": "Dancehall Drops",
    "author": "Kerron",
    "licence": "CC-BY-4.0",
    "url": "https://…",
    "cover": "cover.jpg",
    "samples": [
      { "file": "pull-up.wav", "name": "Pull Up", "level": 1.0, "mode": 0 }
    ]
  }
  ```

  Sample `i` sits on slot `i + 1` (`null` leaves a slot empty); a pack may have fewer than 16 (empty pads show "+"). `level` is the pad's gain (0 silent, 1 as recorded, 2 = +6 dB) and `mode` its play mode number. Licence is an SPDX id; a sample may override the pack's licence and author.
- The **Default DJ Sample Pack** (id `default`) is not a folder: its eight sounds stay synthesised by the engine as today, so the app still needs no sample files. Slots 9-16 start empty; we could add eight more synthesised sounds (rewind, scratch, gunshot, bass drop, …) later.
- **Editing a pad never changes the pack's files.** What you change (sample, name, level, mode) is saved as an override for that pack and slot in `settings.json`, so Reset to pack sample just removes it:

  ```json
  "sampler": {
    "pack": "dancehall-drops",
    "packs": {
      "default": { "3": { "file": "/Users/…/pull-up.wav", "name": "Rewind", "level": 0.5, "mode": 1 } }
    }
  }
  ```

  `file` is a path on disk (a sample borrowed from another pack with Choose Sample is saved by its path), `builtin` one of the built-in sounds, and `empty: true` a cleared pad. Only what differs from the pack is saved. This matches a user request ("store levels in settings") and keeps packs shareable.
- **New pack** makes an empty user pack folder. A file loaded onto a pad of a user pack is copied into that pack's folder on export only, so we don't duplicate audio on disk.
- **`.djsugarpack`** is a zip of the pack folder with the overrides folded into `pack.json` and the referenced files copied in. Import unzips into `<config>/samples/`, keeping both if the name is taken.
- Loose files dropped on the Default pack's pads work too (as overrides); nobody is forced to make a pack first.

### Engine

- `[Sampler1]` to `[Sampler16]`; `[App],num_samplers` reports 16 (the FLX4 script already asks for it).
- Each deck has `[ChannelN],sampler_page` (0 = slots 1-8, 1 = slots 9-16). The 1/2 button and controllers set it; the DDJ-FLX4 script plays slot `page * 8 + pad` on that deck instead of the rekordbox split (left 1-4/9-12, right 5-8/13-16), so the hardware pads always play what the screen shows.
- Samples are decoded with Symphonia on a loader thread, resampled to the engine rate, kept stereo, and handed to the audio thread through the same lock-free hand-over the decks use; the old buffer is dropped off the audio thread. Nothing allocates on the audio thread.
- An empty pad pressed on a controller asks for `LoadSelectedTrack`, as Mixxx mappings do: the app puts the track selected in the library on it. `eject` (FLX4 Shift + pad on a stopped pad) clears it.
- Length limit 60 seconds, held as 16-bit stereo in memory: about 11 MB a slot at 48 kHz, under 200 MB if all 16 slots are full-length. Drops and loops are far shorter; anything longer belongs on a deck. Over the limit: refused with "Longer than a minute, load it on a deck instead".
- Level is the existing `[SamplerN],pregain` (now 0 to 2, i.e. silent to +6 dB, default 1 = 0 dB); `volume` stays as Mixxx's sampler fader at 1.0.
- Play mode is a new `[SamplerN],play_mode` (0 one shot, 1 hold, 2 on/off, 3 loop), applied by the engine to `cue_gotoandplay` press and release, so controller mappings keep sending `cue_gotoandplay` and get the pad's mode for free. `repeat` mirrors Loop for Mixxx compatibility. `cue_gotoandstop` stops (FLX4 Shift + pad).
- A short 2 ms fade on stop and restart, so cutting a sample doesn't click.
- Preview in Choose Sample goes through the headphone preview player (`add-browser-preview-player`); until that lands, a single preview voice on the cue bus.

### Screen

- **Page button**: a narrow 1/2 button on the side of the pads (outer edge of each deck) shows which page the deck is on and flips it; page 2 lights it lilac.
- **SAMPLE PACK strip**: when a tab shows the SAMPLER page, a 22 px strip under the pads holds `SAMPLE PACK  [Default DJ Sample Pack ▾]  [✎]` like djay. It sits under the pads, not between the page tabs and the pads (2026-10-27). All decks share the pack.
- **Pack menu**: popover with each pack's cover (or a lilac initial tile), the chosen one highlighted in Electric Lilac #B69CFF, + at the top right (New pack…, Import pack…), and a full-width Get more Packs button at the bottom. Right-click a pack: Rename, Export…, Show in Finder, Delete (not on Default).
- **Edit mode (✎)**: each pad gets a small level knob on its left and a "…" on its right, as in djay. In edit mode a pad can be dragged onto another to swap them (2026-10-27). Out of edit mode pads stay big, clean triggers. Right-click works in both modes.
- **Choose Sample popover**: anchored above the pad; pack menu at the top; scrolling list of samples with ▶ preview and length (`0:00.7`); a From file… row; and (phase 4) Record New Sample with buttons for deck 1, mic and deck 2.
- **Drag and drop**: files from Finder or tracks from the track list onto any pad; the pad outline turns lilac while hovering. Dropping onto the pack menu imports `.djsugarpack` files.
- **Missing files**: name dimmed with a small warning mark; the "…" menu offers Locate file….

### Get more Packs (later)

Options to look at when that dialog is designed:

1. **Curated DJ Sugar packs** (recommended): a small `djsugar-sample-packs` repository, each pack built from CC0 (and CC-BY with credits in `pack.json`) sounds, published as `.djsugarpack` release assets. The dialog lists them from a JSON index, like add-on repositories. No account needed, licences checked by us, fast downloads.
2. **Freesound** directly: huge CC catalogue and real "packs", but downloading originals needs OAuth2 (a Freesound login) and is limited to 500 a day; previews work with an API key. Licences are mixed (CC0, CC-BY, CC-BY-NC), so we'd filter out NC since DJs play paid gigs.
3. **Add-on packs**: sample-pack add-ons from add-on repositories (`add-stores-and-purchasing` step 0), for third parties to publish their own.

## Phases (one PR each)

1. **Engine**: 16 slots, file loading, stereo, play modes, level, `num_samplers`, FLX4 check.
2. **Packs and pads UI**: pack store, Default pack, settings overrides, SAMPLE PACK strip, edit mode (with pad swap), "…"/right-click menu, Choose Sample popover, drag and drop, rename, reset.
3. **Share**: new pack, `.djsugarpack` export and import.
4. **Record** New Sample (deck, mic, master, loop-length capture) and synced loops.
5. **Get more Packs** dialog (after its own design).

## Answered

- 16 slots, with a 1/2 page button on the side of the pads (2026-10-27).
- Build phases 1-2 now (2026-10-27).

## Open questions

- Should Default DJ Sample Pack get eight more synthesised sounds for slots 9-16, or stay at eight?
