# Proposal

Planned for: **v0.7 · Your library, everywhere** (from a user request, 2026-11-09, after the lyrics, karaoke and waveform lyrics changes)

## Why

Most lyrics a DJ can get are timed by line (LRCLIB, most `.lrc` files) or not timed at all. The karaoke screen, Player mode and the waveform lane are at their best when each word has its time, so the word being sung fills as it is sung. Whisper can hear each word in the vocal stem with its time, on the DJ's own computer.

## What Changes

- **Time Words** in the Lyrics tab, for lyrics not yet timed by word: Whisper (whisper.cpp through whisper-rs) listens to the track's vocal stem, separating it with HTDemucs first when the track has no stems yet, and the lyrics the DJ has are lined up with the words it heard (a global alignment of the two word lists, so missed and extra words don't throw it off). Words it heard take its times; the words between are spread over the gap by length. The result is saved as an enhanced `.lrc` beside the track, like Find, and every view picks it up.
- The model, OpenAI Whisper large-v3-turbo (5-bit, about 550 MB, MIT), is downloaded the first time it is needed from the `stem-models` release, checked against the SHA-1 whisper.cpp publishes for it. On a Mac it runs on the GPU (Metal).
- The tab says what is happening: downloading the model, separating the vocals, timing words with a percentage, or why it failed (too few words heard to trust).
- Later in this change: **transcribe** a track with no lyrics, marked as made by DJ Sugar.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `lyrics`: word timing from the audio

## Out of Scope

- Timing words for every track in the collection at once.
- Choosing another Whisper model.

## Difficulty

**Medium: two PRs.** whisper.cpp builds with cmake through whisper-rs; the alignment is a small dynamic program over the two word lists.

## Performance

Runs only when the DJ presses Time Words, on a thread of its own, one track at a time. Whisper turbo on a recent Mac's GPU takes well under the length of the track; on a CPU it is slower but nothing waits for it. The model is loaded for the run and freed after.
