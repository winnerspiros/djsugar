# Hercules DJControl Inpulse T7: Mixxx mapping

A controller mapping for the **Hercules DJControl Inpulse T7** for [Mixxx](https://mixxx.org) **2.5.6 or later**.

**Status: beta (0.9.0-beta).** It works on the author's T7, but it has had limited testing. Please read "Known limits" below.

It covers two decks with the motorized platters, the eight performance pads on each deck, the browse knobs, the effects section and the mixer section (crossfader, curve switch, headphone split cue).

## Install

You need three files from the `res/controllers` folder of this repository:

- `Hercules_DJControl_Inpulse_T7.midi.xml`
- `Hercules-DJControl-Inpulse-T7-script.js`
- `Hercules-DJControl-Inpulse-T7-platter.js`

Copy all three into the **user controllers folder** of Mixxx, then restart Mixxx:

| System | User controllers folder |
|---|---|
| Windows | `%LOCALAPPDATA%\Mixxx\controllers` (paste this into the File Explorer address bar) |
| macOS | `~/Library/Containers/org.mixxx.mixxx/Data/Library/Application Support/Mixxx/controllers` |
| Linux | `~/.mixxx/controllers` |

If the `controllers` folder does not exist yet, create it. On macOS the `Library` folder is hidden: in Finder use Go > Go to Folder and paste the path. (On macOS, a Mixxx that was installed in the past and never ran in the sandbox may keep its settings in `~/Library/Application Support/Mixxx` instead; Mixxx moves them to the path above when it starts.)

These folders were taken from the Mixxx 2.5 source code; the Windows one is confirmed on a real install, macOS and Linux are not tried yet.

## Turn it on in Mixxx

1. Connect the T7 and start Mixxx.
2. Open **Preferences > Controllers** and select the **DJControl Inpulse T7**.
3. Tick **Enabled**, and in the **Load Mapping** list pick **Hercules DJControl Inpulse T7**.
4. Click **Apply** (or OK).

## Audio setup

The T7 has its own sound card, and the headphone output of the T7 plays whatever Mixxx sends to it, so Mixxx has to be told where to send the sound:

1. Open **Preferences > Sound Hardware**.
2. Under **Output**, set **Main** to the T7, channels **1-2**.
3. Set **Headphones** to the T7, channels **3-4**.
4. Click **Apply**.

The **cue/mix** knob and the master headphone button work in Mixxx, because Mixxx makes the headphone mix.

## What the controls do

Hold **SHIFT** (on a deck) for the second function. Both decks work the same way.

| Control | Plain | With SHIFT |
|---|---|---|
| SYNC | Sync on / off | Make this deck the sync leader |
| CUE | Cue (hold to preview) | Go to the cue point and stop |
| PLAY / PAUSE | Play or pause (also starts and stops the platter) | Stutter play from the cue point |
| IN / OUT | Set loop start / end | Halve / double the loop |
| AUTOLOOP knob | Turn: loop size (or double/halve a running loop). Push: start / leave loop | Turn: move the loop. Push: delete the loop |
| PARAM `<` `>` | Hold to slow down / speed up the deck a little | none |
| RANGE | Tempo range 8 %, 16 %, 50 % | Key lock |
| Browse knob | Turn: move in the track list. Push: load the track | Turn: ten rows at a time |
| ASSIST / BACK | Open the selected item / go back to the list | Add to Auto DJ / big library view (some skins only) |
| Platter | Scratch and nudge | same |
| Crossfader curve switch | mix (smooth), scratch (sharp), disabled (fader off) | |
| Master headphone button | Split cue on / off | |
| FX 1/2 buttons | Arm effect slots 1-3 | Load the next effect into the slot |
| FX levers | Effect unit on while held | |
| FX `-` / `+` | Effect knob down / up | |

Channel faders, gain, EQ, filter, tempo faders, the headphone buttons of each deck and the effect depth knob work as printed.

### Pad pages

The four buttons under the pads choose the page. SHIFT plus the same four buttons gives pages 5 to 8. Each deck remembers its page. The pad lamps are coloured.

| Page | Pads, plain | Pads, with SHIFT | Lamp colours |
|---|---|---|---|
| 1 HOT CUE | Jump to hot cue 1-8, or set it | Delete the hot cue | The colour of the cue; dark if empty |
| 2 LOOP | Loops of 1/4 to 32 beats, on or off | Loop roll of the same size | Blinks teal while that loop runs |
| 3 (stems) | nothing | nothing | dark |
| 4 SAMPLER | Hold to play sampler 1-8 from its cue point; let go and it stops and goes back to the cue point | same | Pink if loaded (blinks while playing); dark if empty |
| 5 BEAT JUMP | Jump back 1, 2, 4, 8 beats (pads 1-4) or forward (pads 5-8) | Four times as far | Dim green; light green while held |
| 6 ROLL | Loop roll of 1/16 to 8 beats while held | same | Cyan while rolling |
| 7 FX | Pads 1-3: arm effect slot 1-3. Pad 4: effect unit on while held | same | Dim red off, red on |
| 8 KEY | Key down, key up, key reset, key sync | same | Dim white |

The **browse knob ring** flashes with the beat of a playing deck: red on beat 1, blue on beats 2 to 4.

## Known limits

- **No stems in Mixxx 2.5.** The INSTRUMENTAL and VOCAL buttons and pad page 3 do nothing and stay dark.
- **The motor is always on**, as it is in Serato and DJUCED. There is no setting to turn it off.
- **Ring beat 1 is counted from the main CUE point.** Mixxx 2.5 has no "bar" information, so the red flash is every fourth beat counted from the main cue point. With no cue point, every beat is blue.
- **Sampler pads add samplers on demand.** If you press a sampler pad above the number of samplers Mixxx has, Mixxx gets more samplers (up to that number).
- **Beta, tested on Windows only so far.** Linux and macOS are untested.
- The beatmatch-guide lights (tempo arrows, beat align) are not used.

### Hardware test status

On Windows with stock Mixxx 2.5.6, the author has checked playback, sync, scratching, mixing, hot cue/loop/sampler pads, pad colours and the browse-ring flash. Later checks confirmed that held rolls and samplers release when the mapping is disabled, PARAM releases across SHIFT on the left deck, and FX pad 4 is audible through headphones.

The latest FX-lever and FX-light fixes pass offline tests, but still need a combined check in Mixxx on the T7. Headphone behavior after the new FX-light setup, shutdown lights, and USB reconnect also need checking. The latest session used headphones; main speaker output was not checked. Please treat this as a beta and try your setup before a live set.

## Report a problem

Open an issue at https://github.com/Infinite-Grok/mixxx-hercules-t7/issues and tell me:

- your operating system and the Mixxx version (Help > About),
- what you pressed and what you expected,
- if possible, the Mixxx log (`mixxx.log` in the Mixxx settings folder). The normal log is usually enough. Use `--controller-debug` only for a short diagnostic session, never while playing: in testing it caused PortMidi buffer overflows and irregular platter timing.

## How this was made

This mapping was written with AI assistance (Claude and Codex) under the author's direction, and it was tested on real T7 hardware by the author. The setup message that makes the T7 show the FX button lights was measured on the USB wire from the T7's own host session.

## Credits

The crossfader curve switch and the channel layout follow the approach of the **Hercules DJControl Inpulse 500** mapping in Mixxx, by Ev3nt1ne, DJ Phatso and resetreboot. Thanks to them and to the Mixxx team. The T7's MIDI messages were measured from the hardware.

## Tests

An offline test suite (no Mixxx and no T7 needed) is in `tests/`; see `tests/README.md`. It runs on every push (`.github/workflows/tests.yml`).

## Licence

GPL-2.0-or-later, the same as Mixxx. The licence text (GPL version 2) is in `LICENSE`. The files in `tests/vendor/` are unmodified copies from Mixxx and keep Mixxx's licence, except `tests/vendor/lodash.mixxx.js`, which is MIT (see `tests/vendor/LICENSE-lodash.txt`).
