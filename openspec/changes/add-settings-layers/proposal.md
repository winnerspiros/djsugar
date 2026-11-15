# Proposal

Planned for: **v1.0 · Ready for any DJ switching over** (agreed 2026-10-31; split on 2026-11-05: the first steps are
`add-settings-table` and `add-settings-deck-choices` in v0.6, and this change keeps the rest)

## Split (2026-11-05)

This change holds the full map in `design.md`. Its first steps became their own changes, so each can ship and be ticked
off on its own:

- `add-settings-table` (v0.6): one option table, Show modified, reset on each row, Reset all, search words, help in ⓘ.
- `add-settings-deck-choices` (v0.6): Decks sections Loading / Playing / Tempo and key, Hot cue on a playing deck,
  Track end warning and Back up the collection rows.
- The track end warning and the collection backup are features of their own.

What stays here, for later: Settings › Advanced with the layer-3 rows, the remaining layer-1 behaviours and layer-2
choices, View › Reset layout and Show all hints again. Profiles stay out of scope; the VirtualDJ profile and
`settings.xml` import are dropped now that VirtualDJ is no longer the starting point.

## Why

Small behaviours (what loading a track resets, where it starts, whether echo tails ring out) shape a DJ's flow more than
big features. VirtualDJ exposes about 330 of them in one flat list; most DJs never find the ones that matter, and DJs
switching over expect their few favourite tweaks to exist. DJ Sugar favours convention over configuration, so it sorts
every candidate into three layers instead of copying the list.

## What Changes

- **Layer 1, no setting**: behaviours DJ Sugar simply has, with no switch. New ones: key shift and stems reset on load,
  instant doubles (loading a track that plays on another deck starts it at the same place, in step), loops trimmed to
  the grid, killed EQ bands grey on the waveform.
- **Layer 2, Settings pages grouped by what the DJ is doing**: Settings › Decks gets sections Loading, Playing and Tempo
  and key. New choices: Tempo on load (Reset / Keep), Load at (first hot cue or first sound / main cue / start), Match
  tempo on load (Off / Close tempos / Always), Hot cue on a playing deck (Jump / Stutter), Track end warning, Jog touch
  (Scratch / Nudge), Preview starts at, Write changes to tags, Back up the collection, Recording file name, Get beta
  versions, Knobs that don't match the screen (Pick up / Jump). Choices that affect a deck are also on that control's
  right-click menu.
- **Layer 3, Advanced**: a Settings page with one searchable list of every tweak, plain-English names with the stable key
  shown small, a description line, **Show modified**, a reset on each row and **Reset all**. Layer-2 choices appear here
  too, so Show modified is the full picture of how this DJ Sugar differs from the defaults.
- Search in Settings finds rows by meaning as well as name ("pitch" finds tempo, "PFL" finds headphones cue).
- Window and panel state (sizes, columns, last folder, zooms) is never a setting: it is saved quietly and **View › Reset
  layout** puts it back. Help gets **Show all hints again**.

The full option-by-option map from VirtualDJ's list, with layers and defaults, is in `design.md`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `app-layout`: Settings pages by task, Advanced list, Show modified, reset, search by meaning, Reset layout
- `deck-transport`: tempo and key on load, load position, instant doubles, hot cue stutter, track end warning

## Out of Scope

- Profiles (Club, Long blends, Scratch, "My VirtualDJ setup") that set many options at once: a later change, built on
  Show modified.
- Importing VirtualDJ's `settings.xml`: needs its own import, later, with profiles.
- MP3 recording (needs an encoder), fluid-grid BPM stabiliser (belongs with beatgrids), per-deck settings lists.
- Pre/post-fader effects themselves stay in `add-fx-pre-post-and-headphone`; this change only places the choice.
- Video, karaoke, timecode, broadcast, online services, skins.

## Impact

app Settings window (`prefs.rs`, `preferences.slint`), settings.json keys, engine load path (tempo, key, load point,
instant doubles), right-click menus on deck controls, user guide Settings page.
