# Design

## How a tutorial runs

- Built on the `add-first-run-setup` tour engine (`crates/app/src/tour.rs`): each tutorial is a `Lesson` in `crates/app/src/tutorials.rs`. The tip rings the real control (its `TourMark`) and goes on once the step's `Wait` is met; **Skip** and ✕ are always there.
- Waits read the same control values the engine and controllers use, so doing a step on screen or on a controller both count. The engine gained:
  - `HasTrack(deck)`: any track on the deck, so a tutorial doesn't make a DJ who already has one load another.
  - `Changed(group, key)`: a control differs from when the step began (picking another effect, setting a hot cue).
  - `Any(&[Wait])`: one of several, such as any of the eight pads.
  - `App(fn(&AppWindow) -> bool)`: something on screen that isn't a control, such as the FX bar showing, a filter chip on or the Automix tab open.
  - Tip buttons `Action::Set` (open a pad page through `pad_panel`) and `Action::Open` (the guide page).
- Starting one switches to 2-Deck and sets the few controls it needs (`prepare`); closing or finishing puts the layout back. Finishing the last tip marks it done (`tutorial_done_<id>` in settings).
- **Settings › Help › Tutorials** lists each with what it covers, its length, ✓ once done, and **Start**. The DDJ-FLX4 tutorial shows "Plug in a DDJ-FLX4 to start" instead of Start until one is plugged in.
- The last tip of each has **Open the guide**: `guide/looping`, `guide/fx`, `guide/filtering`, `guide/automix`, `guide/controllers/ddj-flx4` on github.com/winnerspiros/djsugar/docs.

## Steps

### Looping (deck A)

1. Have a track on deck A (or **Load demo track**). 2. PLAY.
3. Press 4 in the LOOP box (`loop_enabled`).
4. ‹ halves it (`beatloop_size` under 3). 5. › doubles it back.
6. Press the length again to let go.
7. IN, then OUT: your own loop.
8. While it loops, press an empty HOT CUES pad: a saved loop (`hotcue_N_type` 4). **Show hot cues** opens the page.
9. Leave the loop. 10. Press the pad: back into the saved loop.
11. Hold a LOOP ROLL pad (**Show loop rolls**).
12. Done, **Open the guide**.

### Using FX (deck A)

1. Track on deck A. 2. PLAY.
3. FX button: the FX bar shows.
4. ON on deck A's second effect. 5. D/W up.
6. Another effect from its name or arrows (`loaded_effect` changes).
7. Another length in beats (skip for a slider effect). 8. Off again.
9. Beat FX on deck 1, ON. 10. Off.
11. Color FX ▾ beside deck A's filter knob: Space.
12. Knob left. 13. Right. 14. Back to the middle.
15. Done, **Open the guide**.

### Filtering tracks

1. Track on deck A. 2. PLAY.
3. Type a search. 4. Clear it.
5. BPM chip on (match deck A's tempo). 6. Key chip on (keys that mix).
7. **+** and a field, such as Genre.
8. **Save** as a smart playlist (it opens).
9. **Clear** the filter bar.
10. Done, **Open the guide**.

### Automix

1. Automix tab in the side panel. 2. Three tracks queued.
3. Length 16 bars. 4. Style Bass swap.
5. AUTOMIX on (`[AutoDJ],enabled`).
6. **Mix now** (the crossfader moves). 7. AUTOMIX off: take over.
8. Done, **Open the guide**.

### DDJ-FLX4 (only with one plugged in)

Each tip names the controller's own button and rings the matching control on screen.

1. Browse knob and LOAD on deck 1. 2. PLAY/PAUSE. 3. CUE. 4. PLAY/PAUSE.
5. Scratch on the jog's top (`scratch2_enable`).
6. LOAD on deck 2. 7. Channel 2 headphones CUE. 8. BEAT SYNC on deck 2.
9. Channel 2 LOW down. 10. PLAY deck 2. 11. Channel 2 fader up.
12. Bass swap. 13. Channel 1 CFX knob to an end. 14. Channel 1 fader down or crossfader right.
15. HOT CUE: set a cue on deck 2. 16. SHIFT + BEAT JUMP (BEAT LOOP): a pad loop.
17. SAMPLER: play a pad. 18. Beat FX ON/OFF and LEVEL/DEPTH. 19. Beat FX off.
20. Done, **Open the guide**.

Not in it until tried on hardware: the master and headphones level knobs (their MIDI numbers are unverified).
