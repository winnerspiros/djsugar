# Proposal

Planned for: **v0.5 · Stems, performance and switching over** (follow-up to collection maintenance PRs 4 and 5 and improve-metadata-lookup)

## Why

Tidying a collection means changing many tracks at once: one album name on twelve tracks, a mood tag on thirty, "(Original Mix)" off a whole Inbox. Today the track list can only pick a run of rows (shift-click), and the Info tab edits one track. The safe machinery already exists (the clean-up review table, the `tag_changes` log with batch Undo, and `hand_edited` protection), but there is no way in for arbitrary sets of tracks or for typing one value into many.

Every library tool compared (Lexicon, Mp3tag, MusicBrainz Picard, beets, Yate, Serato, rekordbox, VirtualDJ, Traktor, Mixxx) lets the DJ pick any set of tracks and edit them together. Lexicon and Mp3tag show a placeholder for fields that differ; Lexicon and rekordbox treat tags as chips added to or removed from many tracks; Mp3tag, Lexicon and Yate offer case conversion and find and replace. DJ Sugar keeps its own strengths: changes are reviewed before they are written, and a batch can be taken back.

## What Changes

- Cmd-click (Ctrl-click on Windows and Linux) adds or drops one row from the selection; shift-click extends from the last clicked row. The selection is a set of tracks, and every track-menu action, drag, rating and list action takes the set.
- With several tracks selected, the Info tab edits them all: a field they share shows its value, a field that differs shows "N values" with a menu of the values in use (most used first), Keep each track's own, Clear in all, and a new value. Changes collect until Apply (or Discard), go through the change log as one batch that Cmd-Z undoes, and mark typed fields as hand-edited.
- Tag fields show their tags as chips: on all the tracks, on some ("#dreamy 5/12", click to put it on all), removed from all with ×, added with "+ tag" and suggestions from tags already in the library. Rating, colour and stage can be set for all.
- The track menu for several tracks gets Edit Tracks… (⌘I), Add Tag ›, and Fix Names › with Find and Replace, Change Case, Remove Between brackets, Swap Artist and Title and Number in List Order, alongside the existing Clean Up, Fill from File Name and Look Up Online. Every fix fills the review table; nothing is written until Apply.

Collection Health keeps finding the tracks that need fixing; batch editing is for tracks the DJ picks. Both use the same fixes: Health's Clean up, Look up online and Fill from file name join Fix Names for any selection, and the new fixes are offered in Health too. No second review table, log or rule set.

## Capabilities

### Modified Capabilities

- `track-browser`: Selection
- `track-info`: Info tab, editing several tracks
- `library-health`: name fixes in the review table

## Out of Scope

- Editing cells in the track list directly (may follow later).
- Saved chains of fixes, suggested tags from analysis, tag hotkeys.
- Renaming or moving files from tags (waits for the stage-folder move switch).

## Impact

app ui/library.slint (selection), browser.rs (id sets instead of row ranges), browser/info.rs and ui/info.slint (several tracks), browser/cleanup.rs and library cleanup.rs (more fields and fix kinds), browser/track_menu.rs. User guide: Browser, Track info, Collection Health.
