# deck-transport Specification

## Purpose
How a DJ loads a track on one of the four decks and moves around in it: play and CUE, hot cues, saved loops, loops, beat jumps, slip, reverse and the jog wheel.

## Requirements

### Requirement: Four decks
DJ Sugar SHALL have four decks, `[Channel1]` to `[Channel4]`, each able to hold and play one track. Decks 1 and 2 SHALL be shown in every layout; decks 3 and 4 SHALL be shown in the 4-Deck layout.

#### Scenario: Four deck groups exist
- **WHEN** `djsugar --list-controls` is run
- **THEN** it lists controls for `[Channel1]` through `[Channel4]` and `[App],num_decks` reads 4

### Requirement: Loading a track
A track SHALL be loaded by dragging it onto a deck, from a track's menu, or with `LoadSelectedTrack`. The deck SHALL take the track as soon as its file opens and SHALL be playable while the rest decodes; the waveform fills in as the audio is analysed.

#### Scenario: Track plays before decoding finishes
- **WHEN** a long track is dropped on a stopped deck and PLAY is pressed at once
- **THEN** playback starts without waiting for the whole file to decode

#### Scenario: Analysed track loads instantly
- **WHEN** a track that was analysed before is loaded
- **THEN** its waveform, tempo and beat grid show at once from the cache

### Requirement: Protect playing decks
While "Protect playing decks" is on (the default), loading onto a playing deck SHALL be refused and the DJ told why. The setting maps to `[Controls],AllowTrackLoadToPlayingDeck` (inverted).

#### Scenario: Load refused while playing
- **WHEN** protection is on and a track is dropped on a playing deck
- **THEN** the deck keeps playing its track and an alert says the deck is playing

### Requirement: Auto reset on load
Loading a track SHALL put the deck back to normal, as chosen in Settings > Decks under "Auto reset on load", each on by default:
- **Effects** (`[Controls],auto_reset_fx`): every effect unit sent to the deck SHALL switch its effects off, keeping each effect's beats and parameters and the unit's dry/wet. A unit that also sounds on the main mix, or on another deck that is playing, SHALL be left alone. The microphone's unit is never touched.
- **EQ and filter** (`[Controls],auto_reset_eq`): the deck's EQ knobs SHALL return to unity with their kills off, the filter to the middle and the sculpt knobs to flat.
- **Gain** (`[Controls],auto_reset_gain`): the gain knob SHALL return to 0 dB, or to the track's own gain when its info has one. Auto gain SHALL still level the new track on top.

With a reset switched off, loading SHALL leave those controls as they were.

#### Scenario: Track loaded after a heavy mix
- **WHEN** deck 1 has its low EQ cut, its gain at +6 dB and its echo on, and a new track is loaded on it
- **THEN** the EQ and gain are back at unity, auto gain levels the new track and the echo is off, with its beats and dry/wet as they were

#### Scenario: Beat FX shared with a playing deck
- **WHEN** the Beat FX is on for decks 1 and 2, deck 2 plays and a track is loaded on deck 1
- **THEN** the Beat FX stays on

### Requirement: Play and CUE
PLAY SHALL toggle playback. CUE SHALL follow the "CUE button" setting: "Hold to preview" (default, Mixxx `cue_mode` 0) plays while held from the cue point and returns on release, and sets the cue when pressed while paused; "Set and return" (`cue_mode` 3) only sets and returns.

#### Scenario: Hold to preview
- **WHEN** a paused deck's CUE is held and then released
- **THEN** the deck plays from the cue point while held and jumps back to it, paused, on release

#### Scenario: CUE while playing returns
- **WHEN** CUE is pressed on a playing deck
- **THEN** the deck stops and returns to the cue point

### Requirement: Paused indicators blink
The PLAY and CUE indicators SHALL blink in step on a paused deck with a track loaded on controller LEDs. On screen only PLAY blinks: its deck-colour outline (the same outline PLAY shows while playing), with the background plain; CUE, SLIP and every other deck button stay steady while paused.

#### Scenario: Blink while paused
- **WHEN** a track is loaded and paused
- **THEN** on screen only PLAY blinks until the deck plays, and on a controller PLAY and CUE blink together

#### Scenario: Nothing blinks without a track
- **WHEN** a deck has no track loaded
- **THEN** PLAY does not blink

### Requirement: Hot cues
Each deck SHALL have 8 hot cues with a position, colour and optional name, saved with the track in the library. Pressing an empty hot cue sets it at the playhead (snapped to the beat grid); pressing a set one jumps to it; Shift with the pad, or `hotcue_N_clear`, deletes it.

#### Scenario: Set and recall a hot cue
- **WHEN** an empty hot cue pad is pressed while playing and later pressed again
- **THEN** the first press stores the nearest beat, and the second jumps playback there

#### Scenario: Hot cues survive a restart
- **WHEN** hot cues are set, DJ Sugar quits and the track is loaded again
- **THEN** the same hot cues are there

### Requirement: Saved loops
A hot cue MAY be a saved loop (Mixxx hot cue type 4 with an end position). Pressing it SHALL jump to its start and turn the loop on.

#### Scenario: Recall a saved loop
- **WHEN** a pad holding a saved loop is pressed
- **THEN** the deck jumps to the loop start and loops between its start and end

### Requirement: Memory cues kept
Cue points beyond the 8 hot cues brought in from other apps (memory cues) SHALL be kept in the library with the track and SHALL NOT be lost on a re-import.

#### Scenario: VirtualDJ memory cues kept
- **WHEN** a VirtualDJ track with more than 8 cues is imported
- **THEN** all its cues are stored, the first 8 as hot cues

### Requirement: Beat loops
The loop box SHALL show the loop length; pressing it starts a beat loop of that length at the playhead (`beatloop_activate`), pressing again ends it, and the arrows halve or double it (`loop_halve`, `loop_double`). Lengths SHALL run from 1/2 to 64 beats. IN and OUT SHALL set a loop of any length.

#### Scenario: Four-beat loop
- **WHEN** the loop length is 4 and the loop box is pressed
- **THEN** the deck loops 4 beats from the current beat and the length turns green

#### Scenario: Manual loop
- **WHEN** IN is pressed, then OUT a few beats later
- **THEN** the deck loops between the two points and OUT lights lilac

### Requirement: Loop rolls
Loop rolls (`beatlooproll_N_activate`) SHALL loop while held and, on release, continue from where the track would have been without the roll.

#### Scenario: Roll returns in time
- **WHEN** a 1/2-beat roll is held for two bars and released
- **THEN** playback continues two bars later than where the roll started

### Requirement: Beat jumps
Beat jump SHALL move the playhead a whole number of beats forward or back (`beatjump_N_forward`, `beatjump_N_backward`, N = 1 to 32), keeping its place in the beat.

#### Scenario: Jump 16 beats
- **WHEN** beat jump 16 forward is pressed
- **THEN** the playhead moves exactly 16 beats later on the grid

### Requirement: Click-free jumps
Any jump of a playing deck's playhead (beat jump, hot cue, seek, slip return, sync landing, or a loop wrapping back to its start) SHALL crossfade over about 6 ms from the audio that would have played on to the audio at the new position, so it doesn't click. With key lock or a key shift on, jumps SHALL fade the same way and the audio SHALL continue without a gap.

#### Scenario: Beat jump on a sustained note
- **WHEN** a deck playing a sustained bass note beat jumps to a point mid-cycle
- **THEN** the jump is heard without a click

#### Scenario: Short loop with key lock
- **WHEN** key lock is on and a 3/8-beat loop plays
- **THEN** each wrap back to the loop start is heard without a click

### Requirement: Slip mode
With `slip_enabled` on, loops, reverse and scratching SHALL keep a silent playhead moving, and playback SHALL resume from it when the action ends.

#### Scenario: Slip after a loop
- **WHEN** slip is on and a loop is exited after 8 beats
- **THEN** playback continues 8 beats past where the loop started

### Requirement: Reverse and censor
`reverse` SHALL play the deck backwards while on. `reverseroll` (censor) SHALL play backwards while held and resume as if it had played forwards.

#### Scenario: Censor
- **WHEN** censor is held for one bar
- **THEN** the deck plays backwards and on release continues one bar further on

### Requirement: Jog wheel and scratching
Jog input SHALL nudge a playing deck's speed (`jog`) and scratch when scratching is enabled (`scratch2_enable`, `scratch2`). A deck moved by hand faster than 8x SHALL be muted while it moves. Scrolling over an on-screen jog SHALL nudge the track by 1 ms per scrolled pixel.

Turning a jog's outer rim (`jog_bend`, in ticks of 720 to a turn) SHALL bend a playing deck's speed as in Mixxx: clockwise faster, counterclockwise slower, by 0.08% of normal speed per tick a second, averaged over about 0.15 s so it springs back once the rim stops. A paused deck SHALL move 8 ms per tick instead. The DDJ-FLX4's rim, and its platter when not touched, SHALL send `jog_bend`; dragging round the black rim of an on-screen jog SHALL too.

#### Scenario: Rim bend
- **WHEN** the DJ turns a playing deck's jog rim clockwise a quarter turn a second
- **THEN** the deck plays about 14% faster while the rim turns and returns to its tempo when it stops

#### Scenario: Fast search is silent
- **WHEN** a controller jog spins a deck faster than 8x normal speed
- **THEN** the deck is muted until it slows down

### Requirement: Scrubbing on the waveform
Holding the scrolling waveform SHALL stop the track under the pointer and dragging SHALL scrub through it; releasing SHALL resume the previous state. Clicking the overview waveform SHALL seek there.

#### Scenario: Drag to scrub
- **WHEN** the DJ holds and drags a playing deck's scrolling waveform
- **THEN** audio scrubs with the drag and playback resumes on release

### Requirement: Eject
`eject` SHALL unload the deck's track.

#### Scenario: Eject a stopped deck
- **WHEN** eject is pressed on a stopped deck with a track
- **THEN** the deck is empty
