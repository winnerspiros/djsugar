# Proposal

Planned for: **v0.6 · Buy it from the artist** (moved from v1.x 2026-11-06: phrases already ship, and phrase mixing is a top gap for new DJs)

The tasks still open moved to **v0.8 · From the booth to the studio** on 2026-11-12, so v0.7 ships with what is built.

## Why

New DJs judge a DJ app by how good its help sounds. djay's Automix AI and rekordbox's phrase-based mixing bring the next track in where the music changes: its intro over the outgoing outro, with the next drop landing as the old track ends. DJ Sugar Automix still mixes by time or by a fixed number of bars counted back from the end, so it often starts in the middle of a vocal, or brings a drop in over a breakdown.

DJ Sugar already finds phrases (Intro, Build, Drop, Chorus, Breakdown, Outro) on every loaded track since 0.3.5 (`add-phrase-analysis`), keeps them in `markers`, and imports VirtualDJ and Traktor automix points (`fadeStart`, `realStart`). Nothing uses them to mix yet. This change does, for Automix first and then for a DJ mixing by hand.

## What Changes

- **Phrases, a new Automix length.** The mix starts on a phrase change of the outgoing track (its outro, or the last phrase start far enough from the end) and brings the incoming track in from the first bar of its intro, beatmatched, so the incoming track's first phrase change lands on a phrase change of the outgoing one. The mix lasts as long as the shorter of outgoing outro and incoming intro, 8 to 32 bars.
- **Imported points win.** A VirtualDJ or Traktor fade-out point on the outgoing track, and a start point on the incoming one, are used before detected phrases.
- **Falls back quietly.** No phrases, a tempo out of the tempo fader's reach, or a track too short: the mix is 16 bars, or 15 s when the tempos can't be matched. The status line says which.
- **Bass swap on a phrase.** With Bass swap, the bass changes over at a phrase change inside the mix, not at its halfway point.
- **The plan on screen.** The Automix shading on the deck overviews already shows where the mix starts and ends; it now follows the phrase points, and the status line reads "Mixing into <title> at the outro in 0:42".
- **Phrase countdown on the deck**: builds the "Drop · 8 bars left" already specified in `add-phrase-analysis` (its task 1.5b), so a DJ mixing by hand sees the next change coming.
- **Start on phrase.** A new quantize size, Phrase, in the SYNC quantize popover: PLAY on a paused synced deck waits and starts on its leader's next phrase change (PLAY blinks fast while it waits; pressing it again cancels).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `automix`: the Phrases length, imported points, fallbacks, bass swap on a phrase
- `sync`: Phrase quantize, start on the leader's next phrase

## Out of Scope

- An "Auto" style that picks Smooth, Filter, Bass swap or Cut from the phrases on each side, and energy-aware ordering: after energy per phrase (`add-phrase-analysis` 1.2b) and a benchmark against rekordbox phrases.
- Stem-based transitions (vocals out before the incoming vocals come in): with `add-stem-fx-and-acapella`.
- Automix on decks 3 and 4.
- Changing how phrases are found; that stays in `add-phrase-analysis`.

## Impact

app `automix.rs` (length, mix plan), `remote.rs` (phone remote lengths), settings `automix_length` (new index at the end, so saved choices keep their meaning), engine sync/quantize (Phrase size, armed start), deck header in `deck.slint`, user guide `automix.md` and `decks.md` (Sync and quantize).
