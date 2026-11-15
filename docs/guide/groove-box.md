# Groove Box

The Groove Box is a small drum machine that takes the place of a track on a
deck. It plays in time with the mix, so you can add a beat under a track,
keep a groove going between two tracks, or practise mixing without a second
track.

## Start a groove

1. Click **Jam** in the rail at the far left (the keyboard icon)
   and drag a kit from **Groove Box** onto a deck. A track on that deck is
   stopped and ejected first.
2. Press **PLAY** in the Groove Box. It starts on the next downbeat of the
   playing deck, so it is in time from the first hit.
3. Pick a genre (Basics, Techno, House, Drum & Bass and more) and then a
   groove from the menu beside it. Each groove has a line saying what it is.

To go back to a normal deck, load a track onto the deck, or press EJECT
on the stopped Groove Box.

## Sequencer, Synth and Mixer

The Groove Box fills the whole deck. Along the top are PLAY, the kit menu,
the genre, the groove, **Chain ▾**, **A** to **D**, the tempo and three
tabs.

**SEQUENCER** shows eight tracks, each in its own color, with 16 squares
for the 16 steps of a bar: kick, snare, closed hat, open hat, clap, rim,
cowbell and cymbal. The cymbal track plays the crash or the ride, whichever
the groove you load uses. The icon on the left shows the track's sound; click it
to hear the sound, and hover over it to see its name. A lit square plays the
sound on that step. Click a square to add or remove a hit; when the Groove
Box is stopped, the click also plays the sound. Right-click a lit square to
make the hit accented (brighter, a little louder), then soft (paler, half as
loud), then **50%** (it plays about every other time round), then normal again. A track with no hits stays
dimmed, and a track's icon lights up as it plays. The brighter column is the
step that's playing. **M** mutes a track, **S** solos it.

### Dice: new ideas from the groove

The **⚄** button in the top bar varies every track that isn't locked; right-click a
track's icon to vary only that track, or to lock it. Each change waits for the next bar
while the Groove Box plays, so it always lands in time:

- **New groove** (or **New track**): fresh hits in the style of the genre picked, learnt
  from how that genre's grooves use each sound step by step.
- **Mutate**: moves a hit or two to a nearby step, and now and then adds or drops one.
  The kick's downbeat and the snare's backbeat stay put.
- **Busier** and **Calmer**: one hit more on the strongest free step, or one fewer, the
  weakest first.
- **Even out**: the same number of hits spread as evenly as they go (a Euclidean rhythm).
- **Shift later** and **Shift earlier**: every hit a step later or earlier.

**VARY**, at the top of the ⚄ menu, lets the groove change by itself as it plays: turned
up, weak hits sometimes drop out, soft ghost notes come in where the genre puts them, and
every hit wavers a little in loudness. The kick's downbeat and the snare's backbeat always
play. The ⚄ button is outlined while VARY is up. **Basics** has two grooves made to breathe
this way: **Living house** and **Rolling breaks**.

**Undo** in the ⚄ menu puts back the groove as it was, one change at a time.

**SYNTH** shows each track's sound. Click the sound's name to pick another
from the kit. **PITCH** tunes it up or down by up to an octave, **DECAY**
makes it shorter, and **TONE** darkens it (turned left) or thins it (turned
right).

Under the kick, **KEY** tunes the kick to the playing track's key. Click it
to go from **OFF** to **ROOT** (the key's root note) to **5TH** (its fifth)
and back. The kick moves to the nearest such note, never more than half an
octave, and the button shows the note it plays. This suits long, boomy
kicks most (Boom, Analog); a short, punchy kick has little pitch to tune.

**MIXER** has a strip for each track: **PAN** places it left or right, the
fader sets its level beside a meter, and **M** and **S** mute and solo it.

**4x4**, at the bottom left, shows the Sequencer as 16 square pads laid out like a pad
controller: step 1 at the bottom left, step 16 at the top right, four steps to a row. The
tracks down the left pick which track the pads show, the same track a pad controller
edits. Click a pad to add or remove a hit, right-click it to change its level. Click
**4x4** again for the eight rows of 16.

### Fills and Remix

The bar under the tracks plays fills:

- **AUTO 1** plays a fill in the last beat of every 4th bar, the last two beats of every
  8th bar and the whole of every 16th bar. **AUTO 2** does the same twice as often (every
  2nd, 4th and 8th bar). The bars count with the leading track's beat, so the fills land at
  the end of its phrases. After a fill of two beats or more, the cymbal crashes on the
  next downbeat. **NO FILLS** turns them off.
- Hold **FILL** to play a fill now, for as long as you hold it.
- Hold the **REMIX** strip and slide along it: the groove plays one of 16 figures while you
  hold it, and goes straight back when you let go. From the left: hat rolls, rolls over
  two or three sounds, the tracks' patterns swapped between sounds, and at the far right
  16th rolls over everything.

On a Maschine Mikro MK3, **PERFORM** turns the touch strip into the REMIX strip (press it
again for the knob's setting), hold **NOTE REPEAT** for FILL, and **AUTO** steps through
NO FILLS, AUTO 1 and AUTO 2.

## Kits and grooves

The kit menu at the top left has six kits, all synthesised, so they are
free to use anywhere:

- **Analog** and **Synthetic**: kick, snare, clap and hats from the drum
  models of Mutable Instruments Plaits, with two toms each.
- **Boom**: a deep, long kick and soft analogue drums, 808-style.
- **Punch**: a short, punchy kick and bright hats, 909-style, for house and
  techno.
- **Tin**: small, ticking rhythm-box drums, 606-style.
- **Dub**: a soft kick, a rimshot and a chord stab, for dub techno.

Every kit plays the same eight tracks, so any groove sounds right on any kit.
Kits with more sounds (toms, shaker, chord) offer them in each track's sound
menu in **SYNTH**.

**A**, **B**, **C** and **D** are four grooves you can switch between. A new
one starts at the next bar, so the switch is always in time. Picking a
groove loads it into the one you picked.

The grooves come in genres. **Basics** has DJ Sugar's own six; the other
genres (techno, house, drum & bass, breakbeat, ambient, dubstep and more, 400
grooves in all) are drum patterns from
[mpump](https://github.com/gdamdam/mpump), used under its licence (AGPL-3.0).

**Chain ▾** picks a second groove from the same genre. It goes into the
groove after the one you picked (A's partner is B, C's is D), and the two
take turns, a bar each, until you set Chain to Off.

Your grooves, kit, sounds and mixer settings are kept when you quit.

## Timing

The Groove Box follows, in this order:

1. The deck that SYNC follows, or else the first playing deck with a tempo.
2. Ableton Link, when it is on (see [Ableton Link](link.md)).
3. Its own tempo, 120 BPM, when nothing else is playing.

The tempo button in the top bar shows the BPM, outlined while it follows the
mix. Click it for:

- **Follow the mix**: on by default. Turn it off to keep the Groove Box at
  its own tempo whatever plays.
- **−1**, **−.1**, **+.1** and **+1**: change the BPM. This turns following
  off, so the tempo stays where you put it.
- **Half time**, **Normal** and **Double**: follow the mix at half or twice
  its tempo (and turn following back on).
- **SWING**: delays every other step for a looser feel.

If
the leading deck jumps (a hot cue or beat jump), the Groove Box moves to the
new beat at its next bar rather than mid-bar.

The Groove Box plays through its deck's channel, so the EQ, filter, effects,
channel fader and crossfader work on it as they do on a track.

## In 4-Deck

In the 4-Deck layout a Groove Box deck shows its name, kit and tempo, and its
PLAY works, but the step grid is only in the 2-Deck layout for now.
