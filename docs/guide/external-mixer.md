# External mixer

With an external mixer (a Pioneer DJM, Allen & Heath Xone, Rane and the
like), the hardware does the EQ, faders, crossfader and headphone cue. DJ Sugar
only plays the decks, each into its own mixer channel. This is also how
timecode vinyl rigs are wired.

You need an audio interface with an output pair for each deck: many mixers
have one built in (USB on the mixer itself), or use a 4- or 8-output
interface.

## Set it up

1. Open **Settings › Mixer** and set **Mixer** to **External**.
2. The outputs change to a row for each deck, the headphones and the sampler.
   Pick the interface for **Deck 1** and **Deck 2** (and Deck 3 and 4 for a
   four-channel mixer), each on its own channels, for example Deck 1 on
   **ch 1-2** and Deck 2 on **ch 3-4**. Leave a row on **None** to turn it
   off. The channel menu only shows on devices with more than two channels.
3. Wire each output pair to a line input of the mixer: Deck 1 to channel 1,
   Deck 2 to channel 2.
4. To record or go on air, plug the mixer's record (or booth) out into an
   input of the interface, set it up as an Auxiliary input under the inputs,
   and pick it as the **Recording input** under Recording.

## What changes

- Each deck goes out at full level, after its deck effects, pads, loops,
  stems and sync. DJ Sugar's EQ, filter, channel fader, gain knob, crossfader
  and headphone cue are left out; auto gain still levels the tracks.
- The mixer in the middle of 2-Deck, and the channel controls of each 4-Deck
  strip, give way to each deck's level meter.
- Master and booth are silent. The headphones only play the browser's preview
  player.
- Controllers keep working. EQ, fader and crossfader moves they send do
  nothing, so a deck-only controller pairs best.
- Automix and the master limiter are off: they need DJ Sugar's mixer. Each
  deck's output still bends its loudest peaks (above −1.5 dBFS) under full
  scale instead of clipping them.
- The mic and Auxiliary inputs aren't mixed into anything (the mixer has its
  own). The sampler plays on its own output if it has one, else on Deck 1's.
- Master and booth are not used, so their channels are free for the decks.
- Recording and going on air take the **Recording input** (see step 4).

Set **Mixer** back to **DJ Sugar** to mix on screen again.
