# Design

## Where phrases come from

`djsugar_engine::phrases::detect(track)` once the waveform is ready, otherwise `phrases::stored(track)` (imported phrases are kept as they are). This is what Quick review's Tab already does (`browser/cues.rs::jump_phrase`). A `Phrase` has `start` and `kind`; a phrase ends where the next one starts, the last one at the track's end.

Imported automix points are `markers` with `MarkerKind::Automix`: VirtualDJ's `fadeStart` (out) and `realStart` (in), Traktor's FADE OUT and FADE IN (imported with the same names).

## The mix plan (Phrases length)

Worked out when the next track is cued, and again if the outgoing deck's grid changes.

1. **Out point** on the outgoing track, first that applies:
   - an imported `fadeStart` point;
   - the start of the detected Outro, if it is at least 8 bars before the sound end;
   - the last phrase start that is 8 to 32 bars before the sound end;
   - none: fall back.
2. **In point** on the incoming track, first that applies:
   - an imported `realStart` point;
   - the bar nearest `sound_start` (Automix's current cue point), moved back to bar 1 of the phrase it falls in when that is within 2 bars;
   - none: the current cue point.
3. **Length** in bars: the bars from the out point to the sound end, capped by the bars of the incoming intro (from the in point to its first phrase change), rounded down to a multiple of 4, clamped to 8-32. The mix runs from the out point for that many bars, so the incoming track's first phrase change lands as it ends; the rest of a longer outro is no longer heard. An imported fade-out point mixes from there to the sound end. Built in `app/src/automix_phrases.rs`; the plan is worked out once per pair of tracks (by address) while the next track waits cued.
4. **Beatmatch**: as the 16/32-bar lengths do today (`beat_match`, within the tempo fader range). Without it, the Phrases length falls back to 15 s from the sound end, which is today's behaviour.

The mix still starts on a bar (`planned_start`). `Mix` gains nothing new beyond the chosen start and end; `spans()` keeps showing it.

## Bass swap on a phrase

`apply()` swaps over the middle tenth of the mix today. With a phrase plan, the swap is centred on the outgoing phrase change nearest the middle, or the incoming one if the outgoing track has none inside the mix, still lasting a tenth of the mix.

## Start on phrase

Quantize sizes live in the SYNC quantize popover. Adding Phrase there: with a synced deck paused and its leader playing with phrases, PLAY arms the deck (`play_armed`), the engine starts it on the sample where the leader's next phrase starts, phase-locked as usual. Without leader phrases, Phrase acts as 8 bars on the leader's grid. Pressing PLAY again, or CUE, disarms. A controller's PLAY does the same; its LED blinks as for paused.

The engine needs the leader's phrase starts: the app sends them with the track when it loads (as it does the grid), so the audio thread only compares positions.

## Countdown

The deck header shows "<next kind> · N bars left" from 32 bars before a phrase change, counting whole bars on the deck's grid; nothing without phrases or on the last phrase. Small, muted text under the title (no new colours), hidden in Player layout.

## Settings

`automix_length` stores an index into `LENGTHS`. Phrases goes at the end so saved choices keep their meaning; the panel and phone remote list it after 32 bars. New installs default to Phrases.
