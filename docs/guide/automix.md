# Automix

Automix mixes through a list of tracks for you, on decks 1 and 2. Everything
it does goes through the same controls you use, so you see the crossfader
and the EQ move, and you can take over at any time.

## Start a mix

1. **Queue your tracks.** The Automix panel is the first tab of the side
   panel on the right of the browser. Drag tracks into it, or right-click a
   track and choose **Add to Automix**. Drag tracks within the list to
   change their order. The top of the panel shows how long the list runs.
2. **Pick a length and a style** from the two menus at the top of the
   panel (see below).
3. **Press AUTOMIX.** It turns green. If nothing is playing, DJ Sugar loads the
   first track and starts it; if a deck is already playing, Automix mixes out
   of it into the list.

Each track leaves the list as it loads. When the list runs out, the last
track plays to its end and Automix stops.

## Mix length

| Length | What happens |
|---|---|
| **0 s (gapless)** | No mix: each track plays to its very end and the next starts at once, with the crossfader in the middle. For albums and live recordings. |
| **12 s**, **15 s**, **30 s** | The mix takes that long, ending where the outgoing track's sound ends. |
| **16 bars**, **32 bars** | The mix takes that many bars, starting on a bar. The incoming track is synced to the outgoing one when its tempo is within reach of the tempo fader. |
| **Intro over outro** (Phrases) | The mix starts where the outgoing track's outro starts and brings the next track in from the first bar of its intro, synced, for as long as that intro lasts (8 to 32 bars). The next track's first big change then lands just as the mix ends. A mix point from VirtualDJ or Traktor wins over the phrases DJ Sugar found. Without phrases the mix is 16 bars, and when the tempos are too far apart to sync, 15 seconds; the status line says so. This is the length new installs start with. |

## Mix style

| Style | What happens |
|---|---|
| **Smooth** | The crossfader moves evenly across the whole mix. |
| **Bass swap** | As Smooth, with the bass of the two tracks swapped halfway, so they never fight. With Intro over outro, the swap lands on a phrase change of the outgoing track when there is one in the mix. |
| **Filter** | As Smooth, while the outgoing track is filtered away. |
| **Cut** | Switches over in one beat, where the mix would start. |

DJ Sugar remembers the length and style you picked.

## While it plays

- **Mix now** starts the next mix straight away.
- **Skip** drops the track waiting on the other deck and loads the next
  one from the list.
- **Move the crossfader** during a mix to take it over: Automix stops and
  leaves the mix to you. Pausing the playing deck stops it too.
- **Start the other deck yourself** and Automix leaves that deck alone.
- **Press AUTOMIX** again to stop. A mix in progress stays where it is, and
  the EQ and filter Automix moved go back to the middle.

The panel says what Automix is doing, such as which track mixes in next and
when.

## Good to know

- Automix always mixes between decks 1 and 2, whatever the layout.
- Tracks mix best once they're analysed: Automix uses the beat grid to start
  bar-length mixes on a bar, and the waveform to find where a track's sound
  ends. Tracks are analysed the first time they load, or ahead of time with
  [Analyze All](music.md#when-tracks-are-analysed).
- Controller mappings that start Mixxx's Auto DJ, fade now or skip a track
  run Automix.
