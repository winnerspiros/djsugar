# Controllers

DJ Sugar uses [Mixxx](https://mixxx.org)'s controller mappings, so most MIDI
controllers that Mixxx supports load their mapping as soon as they're plugged
in. There's nothing to set up.

**Settings › Controllers** lists the controllers plugged in that DJ Sugar has a
mapping for, each with its mapping, **Edit** and **×**. To add another MIDI
device, such as gear without a mapping, press **+** under the list and pick
it in the new row; the computer's own ports (macOS's Network Session and IAC
Driver) come last. **×** takes a controller off the list and stops using it
until you add it again.

If a controller is missing or stops answering (after the computer slept, or a
USB hub was unplugged), press **Reconnect controllers · Refresh** at the
bottom of the list: DJ Sugar closes every controller and connects them again.
When a controller goes away, DJ Sugar lets go of whatever it held on a deck: a
touched jog, a brake or spinback, reverse, or a pitch bend button.

USB HID controllers work too: Native Instruments' Traktor Kontrol S2 (MK1,
MK2 and MK3), S3, S4 (MK2 and MK3), F1, Z1 and MX2 run Mixxx's HID mappings,
picked by the device itself rather than its name. They're listed with the
MIDI controllers, can be turned off there, and have no picture or list in the
Controller Editor: an HID mapping is all script. The S4 MK3's screens stay
dark. On Linux they need DJ Sugar's udev rule first; see the
[Traktor Kontrol S2 MK3](../controllers/traktor-kontrol-s2-mk3.md) page. The
Traktor Kontrol X1 (first model) is a MIDI controller and has a MIDI mapping.

The **Pioneer DDJ-FLX4** has a page of its own: [Pioneer DDJ-FLX4](controllers/ddj-flx4.md).

## MIDI keyboards and decks

A MIDI device with no mapping, such as a keyboard, plays the instrument
plugin on a deck (see [Instruments](fx.md#instruments)). Its row in
**Settings › Controllers** says **Plays instruments**. A controller running
a mapping never plays instruments. Neither do a keyboard's DAW or Mackie
Control port (marked **DAW port: not used**), the computer's own ports, or
the MIDI side of an HID controller running its HID mapping. To make any of
them play instruments, pick **Play instruments** in its mapping menu.

The menu before the mapping picks the deck the device plays:

- **Auto**: follows the instrument for a device that plays instruments, and
  as mapped for the rest. The menu shows which, **Auto · Follow** or
  **Auto · Mapped**.
- **As mapped**: the decks its mapping names.
- **Follow instrument**: the deck that holds the instrument plugin, or else
  the Groove Box. Drag the instrument to another deck and the device goes
  with it.
- **Deck 1** to **Deck 4**: that deck. A keyboard set to a deck plays the
  instrument only while the instrument is on that deck.

With **Follow instrument** or a deck, the mapping moves along so that its
deck 1 is that deck, lights and all. DJ Sugar keeps these choices.

The first time a keyboard plays the instrument, a message names it, so a
device that shouldn't play it is easy to spot. The keyboard button in the
instrument deck's header shows which keyboard played it last.

## Controller Editor

To see what every control does, press **Edit** next to a controller in
**Settings › Controllers**. The editor opens on that controller's mapping;
pick another at the top. Its controls are listed in three columns: the control
you press (with the layer it works in, such as SHIFT, a pad mode or a bank),
what it does, and the MIDI message it sends. They're grouped into sections
(Deck, Pads, Mixer, Effects, Browser, More) with smaller sections inside, such
as Hot cues and Loops under Pads. Click a heading to open or close it. Search
the list by any of those words.

- **Picture.** For the [popular controllers](../controllers/index.html#popular-controllers),
  a drawing of the controller is on the left, with every control in about the
  place it sits. Point at a part to name it, and click it to list only what
  it does, with the messages it sends and the lights it shows.
- **Press to find.** With the controller plugged in, press or turn a control
  and the list jumps to it.
- **Lights** lists what each LED shows, and **Options** how the controller
  plays with this mapping (**Knobs and faders**: Pick-up waits for the
  hardware to reach a knob moved on screen, Scale takes over at once and
  closes the gap) and the mapping's own settings with their defaults.
- **Your own mappings.** The **Mapping** menu next to the controller has the
  original mapping, which never changes, and your own copies of it: choose
  **Duplicate…** to make one, and **Rename…** or **Delete…** to tidy up. The
  controller runs the one picked.
- **Change a control.** Click a row and the panel under the list shows what it
  does. **Change…** opens every function by category, with a search; pick one
  and the controller uses it at once. Pick the deck or effect unit beside it.
  **Does nothing** turns the control off, and **Back to original** puts the
  row back as the original mapping has it. Changed rows are marked in lilac.
  Changing the original makes your own copy first, called "My" and the
  controller's name, so the original stays as it is.
- **Advanced**, folded under the function, sets when the row acts (press,
  release, long press, tap or double press, so one button can do more than one
  thing), how a button is read, the knob type (absolute, or one of the endless
  kinds), pick-up for knobs and faders, and invert. Each has a short hint.
  A row run by the mapping's script can be given a plain function too; the
  script keeps running the other controls.
- **Scripts, shift and modes.** **Does**, at the top of the panel, picks
  what the control does: *A function*, *An DJSugarScript line* (the same
  one-line actions as [keyboard shortcuts](shortcuts.md), checked as you type
  and used when you press Enter, such as `var 'shift' ? sync : play_pause`),
  *Shift: on while held* or *Mode: on/off each press*. A shift or mode button
  sets a variable, `shift` or `mode` unless you name it otherwise. **Only
  when** makes any row work only while variables are on: `shift`, or
  `mode=2 & shift`. Type it and press **Move** to put the row there, or
  **Copy** to keep it where it was too. So to make SHIFT + CUE sync: make a
  button *Shift: on while held*, select CUE, type `shift` in Only when,
  **Copy**, and change the copy's function to Sync.
- **Import…** in the same menu brings a mapping across from another DJ app:
  VirtualDJ (`.xml` from `Documents/VirtualDJ/Mappers`), Traktor (`.tsi`),
  rekordbox (`.midi.csv`), djay Pro (`.djayMidiMapping`) or an DJ Sugar
  `.djsugar-mapping`. Each file becomes a mapping of your own, built on the
  controller's original mapping. Controls DJ Sugar can't do yet are kept but
  greyed, with the reason, and the original keeps working for them; a report
  says how many came across. A VirtualDJ keyboard file becomes a keyboard
  shortcut set instead (see [Keyboard shortcuts](shortcuts.md)).
- **2 or 4 decks.** A mapping with a layout for two decks and one for four,
  such as VirtualDJ's Xone K2, adds **Used with** to the editor's **Options**
  tab, and with four decks **Deck order**: which deck each column is from
  left to right (3 1 2 4 to start, as on the 4-Deck screen).
- **Export…** writes the mapping as one `.djsugar-mapping` file that works on
  another computer, and **Export for Mixxx…** writes a `.midi.xml` with its
  scripts; a report lists anything Mixxx can't do.

## Map mode

The quickest way to change a few controls: open the Controller Editor
(Settings → Controllers, **Edit**) and press **MIDI map**. The editor makes
way for the main window, and every control on screen that can be
mapped turns lilac, brighter where the controller already does it. Click
one, then press, turn or move a control on the controller, and from then on
it does that. The bar says what changed and what the control did before.
**Keep both** makes it do both, **Undo** (or ⌘Z, Ctrl+Z on Windows and
Linux) takes the last one back, and **Done** or Esc ends Map mode and brings
the editor back. While Map mode is on, the controller only teaches:
pressing PLAY doesn't start a deck.

Besides the deck and mixer controls, the effects (ON, D/W, the second
control's slider and the ‹ › that step through effects), the pad mode tabs
over the pads and the headphone cue buttons can all be mapped.

Map mode works out endless encoders and 14-bit faders by itself. The
changes go into your own mapping, as in the editor ("My" and the
controller's name the first time), so the original stays as it is.

Mappings that run a script are tried out in a sandbox to see what each
control does, so even those read in plain words.

## Mapping wizard

For a controller DJ Sugar has no mapping for, or to map one from scratch:
press **Map it** next to it in **Settings › Controllers**, or **Mapping
wizard** in the Map mode bar. From the bar it first asks which controller,
with a button for each one plugged in (or press a button on one that
already runs a mapping); the
computer's own MIDI ports, such as macOS's Network Session, are never
offered. Then it asks whether the controller is used with two decks or four,
then walks through the usual controls one at a time, lighting each up on
screen: deck 1's PLAY, CUE, SYNC, jog wheel, tempo fader, headphone cue,
gain, EQs, filter, channel fader, pads 1 and 2 and LOAD, then the crossfader
and the browse knob. Press or move each on the controller, or **Skip** one
it doesn't have. Pads 3 to 8 follow on from pads 1 and 2.

The other decks aren't walked: pressing PLAY on deck 2 (and 3 and 4) shows
how they differ from deck 1, usually another MIDI channel, and deck 1's
controls are copied over. With four decks, **Used with** and **Deck order**
in the editor's **Options** then work for it as for any four-deck mapping.

The wizard makes a new mapping called "My" and the controller's name and puts
it in use; the mapping the controller ran before stays in the menu next to
it. When the walk is done Map mode stays on, so you can try it and change
anything by clicking a control on screen.

## Popular controllers

DJ Sugar looks after a list of popular controllers itself: the DDJ-FLX4,
DDJ-FLX6, DDJ-400, DDJ-200, DDJ-SB3, Mixtrack Platinum FX, Mixtrack Pro FX,
DJControl Inpulse 500, Xone K2 and Midi Fighter Twister so far, with the
DDJ-REV1, DDJ-1000, DDJ-FLX10 and Traktor Kontrol S2 MK3 coming.
Each has a drawing in the Controller Editor and a guide page.

They're marked **Beta** in **Settings › Controllers** and the editor until
someone has tried the mapping in DJ Sugar on the real controller, and
**Supported** once that's done and the mapping is settled. If you have one
of them, try it and
[tell us how it goes](https://github.com/winnerspiros/djsugar/issues/new?template=controller.yml).

[Supported controllers](../controllers/index.html) lists every controller with
a mapping, and what each control does where the mapping describes it.

If your controller doesn't work right,
[tell us which one](https://github.com/winnerspiros/djsugar/issues/new?template=controller.yml).
Beta testers' controllers are fixed first.
