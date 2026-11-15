# Pioneer DDJ-FLX4

The DDJ-FLX4 is a two-deck controller, and the one DJ Sugar is tested with
most. Plug it in over USB and it works: DJ Sugar loads its mapping by itself,
and the pads, sync and effects follow what's on screen.

On Windows, quit rekordbox or any other DJ app first, since only one app
can use the controller at a time.

## Decks

- **PLAY/PAUSE** and **CUE** work as on any Pioneer deck. **SHIFT** + **CUE**
  jumps to the start of the track. **SHIFT** + **PLAY/PAUSE** plays backwards
  while held, and the track carries on where it would have been (censor).
- **The jog wheel**: the top of the platter scratches, the rim bends the
  tempo to nudge a mix back in line. **SHIFT** + jog searches through the
  track quickly.
- **TEMPO**: up is slower, as on Pioneer gear. To have it the other way, turn
  on **Tempo fader direction** in **Settings › Decks**.
- **BEAT SYNC** turns [sync](../decks.md#sync-and-quantize) on, and a second
  press turns it off, as SYNC on screen does. Hold it to make that deck the
  master. **SHIFT** + **BEAT SYNC** changes the tempo range.
- **SHIFT** + a channel's **CUE** turns quantize on or off for that deck.

## Loops

- **LOOP IN** and **LOOP OUT** make a loop of any length. With **SHIFT**,
  the jog wheel moves the loop's start or end.
- **RELOOP/EXIT** starts a loop of the length shown on screen where the track
  is playing, or leaves the loop that's playing.
- **CUE/LOOP CALL ‹** and **›** halve and double the loop.

See [Looping](../looping.md).

## Pads

The pad mode buttons open the deck's [pad tabs](../pads.md#pad-pages) on
screen, and the pads play whatever page is in that tab:

| Button | Opens | With SHIFT |
|---|---|---|
| **HOT CUE** | Hot cues (SHIFT + pad deletes one) | **KEYBOARD** |
| **PAD FX1** | Pad FX 1 | **PAD FX2** |
| **BEAT JUMP** | Beat jump, 4 to 32 beats | **BEAT LOOP** |
| **SAMPLER** | Sampler | **KEY SHIFT** |

**Stems**: press **KEYBOARD** twice to turn its pads into the
[stems pads](../stems.md#stems-pads), in the same layout as on screen, and
again to go back. **SHIFT** + a stem's pad plays that stem alone. You can
also right-click a tab on screen and put **STEMS** in it, for example in
place of the sampler, and that button then gives you stems.

In **SAMPLER** mode each side's pads play the eight sounds on screen for
that deck (see [Sampler](../sampler.md)).

## Mixer

**TRIM**, **EQ HI/MID/LOW**, the channel faders and the crossfader work as
expected. **CUE** sends a channel to the headphones, **HEADPHONES MIXING**
blends cue and main in the headphones, and **MASTER LEVEL** and **HEADPHONES
LEVEL** set those levels.

When the mixer on screen is switched to **STEMS** or **SCULPT** (the menu
over the meters), the controller's EQ knobs follow it.

## Effects

- **The CFX knobs** play each channel's [Color FX](../fx.md#color-fx).
  **SHIFT** + **SMART CFX** steps through them.
- **The BEAT FX section** plays DJ Sugar's [Beat FX](../fx.md#beat-fx):
  **BEAT FX SELECT** steps through the effects (SHIFT goes back), **BEAT ‹ ›**
  halve and double its length, **CH SELECT** picks deck 1, deck 2 or both,
  **LEVEL/DEPTH** sets how much you hear, and **ON/OFF** turns it on.
  **SHIFT** + **ON/OFF** releases it, letting its tail ring out.

## Browser

- **Turn BROWSE** to move through the track list, and **press** it to move
  between the list and the tree on the left. **SHIFT** + press zooms the
  waveforms.
- **LOAD** loads the selected track onto that deck.
- **SHIFT** + left **LOAD** shows the tracks linked to the selected one.
- **SHIFT** + right **LOAD** goes to the next of your layouts (VIEW; see
  [Your layouts](../layouts.md#your-layouts)).

## Decks 3 and 4

In the 4-Deck layout, click deck 3's number to move the controller's left
side to deck 3, and deck 1's number to move it back. Deck 4 and deck 2 do
the same for the right side. See
[Layouts](../layouts.md#decks-3-and-4-on-a-two-deck-controller).

## Every control

[Pioneer DDJ-FLX4 in Supported controllers](../../controllers/pioneer-ddj-flx4.md)
lists every control of the mapping and what it does, with and without
SHIFT. The [Controller Editor](../controllers.md#controller-editor) in
DJ Sugar shows the same with a picture of the controller: press a control and
the list jumps to it.

Something not right on your FLX4?
[Tell us](https://github.com/winnerspiros/djsugar/issues/new?template=controller.yml).
