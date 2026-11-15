# Proposal

Planned for: **v0.7 · Your library, everywhere** (split from the lyrics and karaoke research, 2026-11-06; `add-karaoke` stays in v1.x for CDG and MP4 karaoke files and a singer queue)

## Why

With lyrics read and timed (`add-synced-lyrics`) and stems separated on load, DJ Sugar has what a karaoke night needs: the words, when they are sung, and the track without its voice. VirtualDJ has a karaoke screen and a vocal stem; djay, Serato and rekordbox have no karaoke screen. A DJ playing a party, a bar or a wedding wants the words on a TV or a projector while the music plays on.

## What Changes

- **Karaoke screen**: a window of its own, opened from the Lyrics tab or from Player mode's large lyrics, that opens full screen on another display when there is one. It shows the line to sing big, the word being sung filling with lilac as it is sung, the line before faded, and the next two lines under it. Lyrics timed only by line have their words spread over the line by length. Before a line that follows a break, dots count in the last four beats at the deck's tempo. Double-click or F toggles full screen, Esc leaves it.
- It follows the track playing, as the Player's large lyrics do, so it moves over with Automix.
- **Guide vocal**: while the screen is open the Lyrics tab has a Vocals slider, the vocal stem's level on the deck followed (none to full). The level set there is held on the next track the screen follows, so a whole night can run with the voice down.
- **Karaoke page**: the same screen as a page on DJ Sugar's local web server (the now-playing overlay's, port 7487), for a TV's browser, a tablet or a laptop on the network. Share in the Lyrics tab serves it on the network while the karaoke screen is open and shows its address.

## Capabilities

### New Capabilities

- `karaoke-screen`: the words of the track playing on a screen of their own, with the voice turned down

### Modified Capabilities

None.

## Out of Scope

- CDG and MP4 karaoke files, a singer queue and key per singer: `add-karaoke` (v1.x).
- Word timing from the audio (Whisper on the vocal stem): a later change.
- Video behind the words.

## Difficulty

**Medium: two PRs.** The window is a second Slint window fed from the Lyrics tab's state; the page reuses the overlay server and polls JSON like the overlay does.

## Performance

The screen is updated from the Lyrics tab's poll on the UI thread, a few string compares a frame; nothing runs while it is closed. The second window is drawn only when what it shows changes, apart from the filling word.
