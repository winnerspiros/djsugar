# Decks and mixer

Drag a track from the browser onto a deck to load it, or select it and
press a controller's **LOAD** button. You can also drag an audio file
straight from Finder, Explorer or your file manager onto a deck: it loads
there and joins your library. (On Linux this needs an X11 session; Wayland
doesn't pass file drops to openDJ yet.)

## Decks

In 2-Deck each deck runs, from the outer edge in:

- **The tempo column**, as in djay Pro: **SYNC** on top, the BPM, the
  tempo % (click it for the [tempo panel](#tempo-panel-and-range)), the
  tempo fader, and **−** and **+** under it for small steps. The triangle
  beside the fader puts the tempo back to the track's own. A lilac line
  down the fader means [key lock](#keys) is on; right-click the fader to
  switch it.
- **The pads**, with their tabs on top (see [Pads](pads.md)).
- **The LOOP column** (see [Looping](looping.md)).
- **The jog wheel**, with **CUE** and **PLAY** under it, next to the mixer.
  Drag round its black rim to bend the tempo, scroll over it to nudge, and
  hold **«** or **»** in its lower corners to bend slower or faster.
  **SLIP** sits in its top right corner (see
  [Slip, bend and censor](#slip-bend-and-censor)).

Above them, the deck's header shows the cover, title and artist, the time
left and the track's tempo and key, with the overview waveform of the whole
track. Click the overview to jump; dragging it moves without playing what it
passes.

To empty a deck, point at its cover and click the small **⏏** in the
corner (the Mixer has it at the end of its title bar instead). It works on
whatever the deck holds: a track, the Groove Box, the Looper, an input or
the Mixer. It only shows while the deck isn't playing. Right-clicking the
cover has **Unload deck** too, and <kbd>⇧</kbd> <kbd>⌥</kbd> <kbd>←</kbd> /
<kbd>→</kbd> unload decks 1 and 2. If it was a mistake, click
**↺ Put back** on the empty deck: a track comes back where it was.

### Waveform look

Click the arrow on the right of the waveforms button in the top bar. Under
the height slider, these set how the waveforms look:

- **Type**: **Full** (the default) mirrors the waveforms about their middle;
  **Half** grows them from the bottom edge, as on rekordbox. It applies to
  the large waveforms and the overviews under the decks alike.
- **Color**:
  - **RGB** (the default): one shape, red where the bass is loudest, amber
    for the mids and blue for the highs.
  - **3-Band**: the bass, mids and highs stacked in their own colors, the
    bass widest, as on rekordbox. A second row picks **Blue-Orange**
    (rekordbox's colors) or **Lilac-Cyan**.
  - **Stems**: each stem (kick, hi-hats, bass, instruments, vocals) in the
    color of its stem pad, the loudest at the back, so you can see where
    the vocals come in or the bass drops out. A track shows RGB until its
    stems are ready.
- **Needle**: where the play line sits on the large waveforms. **Center**
  (the default) shows as much of what has played as what is coming;
  **Left** puts it a quarter of the way in, so three quarters of the
  waveform shows what is coming next.
- **Vocals**: what runs along the bottom of the large waveforms, one at a
  time. **Lyrics** (the default) shows a track's timed lyrics in cyan.
  **Bar** draws a cyan bar wherever someone sings, leaving the waveform's
  colors as they are. openDJ checks each track for vocals the first time
  you load it (or when it's analyzed), in the background, so the bar shows
  up a few seconds after the waveform; after that it's there straight
  away. Tracks with HQ stems use their vocal stem instead. **Off** shows
  neither.
- **Phrases**: the colored phrase bar along the bottom of the overviews
  (**Overview**), the phrase flags on the large waveforms (**Waveform**),
  **Both** (the default) or neither (**Off**).

The changes show straight away and the menu stays open, so you can try them
out.

**Settings › Appearance › Size follows gain**, on at first, makes a deck's
large waveform taller or shorter as you turn its gain knob.
The overview keeps its height.

### Waveform zoom

The large scrolling waveforms zoom in twenty steps of 5%, from **5%** (about
40 seconds across) to **100%** (under a second, close enough to line a grid
up with a kick). They start at **40%**, ten seconds across. Every large
waveform zooms together, in every layout, so beats stay the same distance
apart on each deck; the overviews under the decks always show the whole
track.

- Scroll the mouse wheel or swipe on the trackpad over a large waveform: up
  zooms in, down zooms out. The zoom follows smoothly and settles on the
  nearest step when you stop.
- Use the bar at the edge of the waveform under the pointer (the side away
  from the grid pencil), widest at the top: click the grey track to move the lilac marker there, or drag the
  marker up and down.
- **⌘=** and **⌘−** (**Ctrl** on Windows and Linux) step in and out, **⌘0**
  goes back to 40%. They are also in **View** and in a large waveform's
  right-click menu.
- On a controller, map **Zoom waveform in**, **Zoom waveform out**, **Reset
  waveform zoom** or **Waveform zoom** (1 to 20).

The step shows for a moment on the waveform under the pointer (or the top
one). openDJ remembers it for next time. Decks playing an input, an
instrument or the Groove Box keep ten seconds across.

### Waveform height

The large scrolling waveforms have ten heights, **1** to **10**; **3** is the
usual one. Each layout remembers its own, so tall waveforms in 2-Deck don't
make 4-Deck taller.

- Drag the line under the waveforms: a lilac grip shows when the pointer is
  on it. The waveforms stretch with the pointer and settle on the nearest
  height when you let go, with the number in the middle. Double-click the
  line to go back to 3.
- Click the arrow on the right of the waveforms button in the top bar and
  drag the **Waveform height** slider: the waveforms resize as you drag and
  settle on the nearest height when you let go. The arrow keys move it a
  height at a time.
- Right-click a large waveform and use **−** and **+** under Height.
- **⌥⌘↑** and **⌥⌘↓** (**Ctrl+Alt** on Windows and Linux) make them taller
  or shorter. They are also in **View**.
- On a controller, map **Taller waveforms**, **Shorter waveforms**, **Reset
  waveform height** or **Waveform height** (1 to 10).

The library always keeps room for about five tracks. When the window is too
short for the height you chose, the waveforms show the highest one that
fits, and grow back when the window does. The decks never shrink.

### Beat counter

Beside the title, four squares light up one beat at a time, so you can see
where the bar is: the first beat of each bar lights white, the others in the
deck's color. The thin bars under them fill up through each group of four
bars, and all four turn lilac in the last bar before a new phrase starts,
so you know the change is one bar away.

Next to the squares, the count reads **phrase.bar.beat** (for example
**3.2.1**: phrase 3, bar 2, beat 1), and under it **↓ 6 › DROP** says the
next phrase, a drop, starts in 6 bars. The phrases come from the track's
[phrase analysis](music.md#phrases); a track without phrases is counted in phrases of
eight bars from its first bar. If beat 1 doesn't land on the downbeat, the
beat grid is off: click the tempo on the jog and choose **Bar 1 here** on a
downbeat (see [Beat grids](music.md)).

Choose **Off**, **Beats** (squares only) or **Beats and phrases** under
**Settings › Decks › Beat counter**. The 4-Deck strips show the squares
only.

**PLAY**, **CUE** and **SYNC** show an outline in the deck's color while
they're on. **Shift-click** and hold PLAY while the deck plays for
[censor](#slip-bend-and-censor).

The decks' effects are in the [FX bar](fx.md#the-fx-bar) under them.

### Jam on a deck

A deck can play something live instead of a track. Click **Jam** in
the rail at the far left (the keyboard icon) and drag one onto a deck:

- A **Groove Box** kit: a drum machine in time with the mix (see
  [Groove Box](groove-box.md)).
- An **instrument plugin** you ticked in **Settings › Plugins**, played with
  a MIDI keyboard or the **KEYBOARD** pads. **OPEN** on the deck opens its
  own window for its sounds and presets (see
  [Instruments](fx.md#instruments)).
- The **Looper**: four layers you record in time with the mix and play
  back (see [Looper](#looper)).
- The **Microphone**, from the input picked in **Settings › Audio**.
- The **Mixer**: up to eight aux inputs from **Settings › Audio**
  (turntables, a drum machine, a phone, another DJ's mixer) summed into the
  one deck. The numbers on the deck turn each input on and off; a new
  input starts off. The deck's fader is the Mixer's master, and one deck
  holds the Mixer at a time.

It plays through that deck's channel, so the EQ, filter, effects, fader and
crossfader all work on it. A track on the deck is stopped and ejected
first. An input plays on one deck at a time: dropping it on another deck
moves it there, and while a deck has it, its own channel in the MIC panel
stays quiet. The Jam page shows which deck each one is on.

The deck's scrolling waveform draws what it plays as it plays: the sound
comes in at the play line and scrolls away to the left, with nothing ahead
of it. The beat lines and bar numbers are those of the track leading the mix
(the sync leader, or else the first deck playing a track), so you can see
your part land on its beats. The leading track's phrases show too: a thin
strip along the top in each phrase's color, running on ahead of the play
line, and a flag at each new phrase, so you can see the drop coming and
bring your part in on it.

On a deck playing an instrument plugin, the notes you play show as bars on
a note roll over a faint copy of the sound: one row per note, fitted to the
notes played in view, with the notes still held lit up to the play line.
The rows of the scale the instrument plays in (its Scale setting, Auto or a
picked scale) are tinted.

On a Groove Box deck, the scrolling waveform shows eight step lanes, kick
on top, over a faint copy of the sound. The hits it played scroll away to
the left, and the hits still to come are drawn ahead of the play line as
outlines, so you can see the next bars before they play. Fills are drawn in
lilac, and a groove you queue shows from the next bar on.

On an instrument deck, the small sliders button at the waveform's top
right switches to macro lanes: the knobs on the page the deck shows, drawn
as lines up to the play line across the waveform's full height, each in its
own colour, so you can see the filter ride you did a few bars back. Only the
knobs that moved in view get a line. The list just past the play line names
all eight with their values and colours, dimming the ones left alone. Click
the button again to go back to the note roll. Macro lanes are in 2-Deck
only; a Groove Box deck always shows its step lanes.

To go back to a track deck, load a track onto it, or press EJECT while it is
stopped. openDJ remembers what each deck plays for next time.

### Looper

Drag **Looper** from the **Jam** list onto a deck. In 2-Deck the deck shows
four layers, two by two, with **▶ PLAY** and four beat lights under them. The
header shows the tempo the Looper plays at, and whether it is in time with
the mix.

- **FROM** picks what it records: **Mix** (the main mix), **Mic**,
  **Aux**, **Instr.** (the instrument plugin), **Aux 2** or **Aux 3**.
- **BARS** sets how long a new layer is: 1, 2, 4, 8 or 16 bars.
- Tap a layer's **● REC** to record it. It shows **NEXT BAR** until the
  next bar starts, then **RECORDING**, then **▶ PLAY** with its length once
  the bars are done. Tapping a layer on a stopped deck starts the deck.
- Tap a playing layer to **OVERDUB**: what comes in is added on top of
  it. Tap it again to go back to playing. With **THEN OVERDUB** on, a layer
  goes straight on to overdub when it finishes recording.
- **CAPTURE** puts the bars just heard into the first empty layer, even if
  you weren't recording. **LAST** picks how many: 1, 2, 4 or 8 bars.
- Each layer has **M** to mute it, a level knob, and **×** to clear it.
- **UNDO** takes back the last recording or overdub, and **REDO** brings
  it back. While a layer waits for the bar or records, **UNDO** cancels it.
- **▶ PLAY** under the layers starts all of them together, and **■ STOP**
  stops them.

A layer holds up to about 48 seconds, 16 bars at 80 BPM. If what you pick
is longer, the deck says **Too long to fit: pick fewer bars.** To go back
to a track deck, drop a track on it.

## Slip, bend and censor

**SLIP** keeps the track in time while you play with it. Click the word
SLIP in the jog's top right corner (2-Deck only): it turns lilac
when on and light grey when off. While it's on,
scratching, a loop, a hot cue held down, reverse or a pause takes the sound
away, but a silent playhead carries on, drawn as a dashed lilac line on the
waveforms. When the trick ends (you let go of the jog or the pad, end the
loop, turn reverse off or press PLAY) the track carries on from that line,
on the beat and the phrase, as if it had played on. In slip mode a hot cue
plays only while you hold it. SLIP blinks while the track is held elsewhere;
switching it off then also brings the track back. Slip only works on a deck
that's playing: on a paused deck, scratching and hot cues find a spot as
usual.

**Scratching** follows your hand closely. openDJ smooths a controller's jog
the way Mixxx does when you hold the record or push it at a steady speed,
so the sound doesn't wobble. When your hand changes speed quickly, as in a
baby scratch, a quicker filter takes over, so the record keeps up: about
10 ms behind your hand, where Mixxx's smoothing alone is about 26 ms behind.
The audio buffer (Settings › Audio) comes on top of that, so a smaller
buffer feels tighter still.

**Vinyl mode** (Settings › Decks) decides what touching the top of a
controller's jog does. On (the default), it grabs the record so you can
scratch. Off, the top bends the tempo like the rim does, so brushing the jog
doesn't stop the track, as CDJ mode does on a Pioneer player. The DDJ-FLX4
and DDJ-FLX6 follow it, and a controller's own VINYL button still switches
it for its deck.

**Pitch bend** nudges a deck's beat into line by ear: it plays a little
faster or slower while you bend and springs back when you stop. Turn a
controller's jog rim, drag round the on-screen jog's rim, or hold **«** or
**»** beside the jog (2%, or 4% with Shift). <kbd>←</kbd> and <kbd>→</kbd>
bend the deck you started last, the one you're mixing in.

**Censor** plays the deck backwards while you hold it, then carries on as if
it had played forwards, to bleep a word. Shift-click and hold PLAY while the
deck plays, or hold SHIFT and PLAY on the DDJ-FLX4.

## Tempo panel and range

Click a deck's tempo % (under the BPM in 2-Deck and 2-Deck Basic, under
the BPM in a 4-Deck strip) for its tempo menu. In 2-Deck and 2-Deck Basic
it has a short tempo slider with fine **−** and **+**, **KEY LOCK**,
**QUANTIZE** and **RESET**; in 4-Deck, **KEY LOCK**, **QUANTIZE** and
**RESET**. **RESET** puts the tempo back to the track's own.

**Range** at the bottom shows how far the tempo fader goes either way, and a
click lists the ranges: **4%**, **6%**, **8%**, **10%**, **16%**, **24%**,
**50%** and **100%**, the current one ticked. The new range applies to that
deck straight away and the tempo stays where it is: the fader moves to
match, or goes to its end when the tempo doesn't fit the new range.
**Settings › Decks › Tempo fader range** sets the range every deck starts
with, and changing it there also changes every deck's range the same way.
Custom controller mappings can set a deck's range the same way with
**rate_range_set** (for example `0.16` for 16%); **rateRange** stays Mixxx's
raw range control.

## Sync and quantize

**SYNC** sits over the tempo fader. It stays on while it's lit: the deck
follows the **master** deck's tempo and keeps its beats in step, at half or
double tempo when that's closer. Press it again to let go. **Shift-click**
SYNC to match the tempo and beat once, without staying synced, as SYNC does
in VirtualDJ.

The deck that synced decks follow shows a small **MASTER** under its key.
openDJ picks it by itself, the deck that has been playing longest, and hands
over when it stops. Click **MASTER** on a deck to keep that deck the master
(it fills in), and click it again to let openDJ pick.

**Right-click SYNC** for **quantize**. While it's on, cues and loops are set
on the beat, and play, CUE, hot cues and jumps land in step with the other
playing deck. The same menu sets the step: **1/16**, **1/8**, **1/4**,
**1/2** or **1** beat, or **4** to land on the same beat of the bar. Cues and
loops snap to a beat at most. **Settings › Decks › Quantize** sets whether
quantize starts on for every deck.

## Keys

**Key lock** shows on the tempo fader: while its line is lilac, moving
the fader keeps the track's key. It's on from the start; right-click the
tempo fader to switch it off and on (**Settings › Decks › Key lock on at
start** sets how decks start). The **+** and **−**
beside the deck's key shift it a semitone at a time, and clicking the key
lists every shift with the key it gives.

The track list shows in lilac the keys that mix with the track playing now.
**Settings › Decks › Key mixing** picks which ones count:

- **Harmonic** (4 keys): the same Camelot number, or the next number up or
  down with the same letter. 8A mixes with 8A, 8B, 7A and 9A.
- **Fuzzy** (6 keys): the letter doesn't matter, so 7B and 9B mix with 8A
  too. This is [fuzzy keymixing](https://www.digitaldjtips.com/fuzzy-keymixing-the-new-way-to-dj/).

A key one semitone from mixing gets a small **+1** or **−1** in a dimmer
lilac. With **Match key on load** (Off, ±1 or ±2 semitones, ±1 by default), a
track loaded while another deck plays is shifted by that much, with key lock
on, so it mixes. At ±2, keys two semitones away are marked **+2** or **−2**
in grey.

## Auto reset on load

Loading a track puts its deck back to normal, so the last track's settings
don't carry over. In **Settings › Decks**, under **Auto reset on load**, each
of these can be switched off (all are on to start):

- **Effects**: the deck's effects switch off. Their beats and dry/wet stay as
  you set them. An effect that also plays on the main mix or on another
  playing deck, such as a shared Beat FX, keeps going.
- **EQ and filter**: EQ, kills, filter and sculpt go back to the middle.
- **Gain**: the gain knob goes back to 0 dB (or the track's own gain, if its
  info sets one). Auto gain still levels each track.

## Mixer

Between the decks, each channel has four knobs down its outer side, its
gain, and a channel fader beside the main level meter. Under the faders are
the headphone **CUE** buttons and the crossfader.

The menu over the meters picks what the four knobs do:

- **EQ**: high, mid and low, and the [Color FX](fx.md#color-fx) filter knob,
  with a small **▾** beside it to pick the Color FX.
- **STEMS**: the level of each [stem](stems.md).
- **SCULPT**: a low-pass, a sculpt band with its frequency and cut or boost,
  and a high-pass, after PlayDifferently's Model 1.

A controller's EQ knobs follow the same choice.

**Crossfader curve** (Settings › Mixer): **Smooth** for blends, **Linear**,
or **Scratch**, where both sides play at full level until the very end. With
Scratch, **Cut-in** sets how close to the end a side opens fully: 0.5%, 1%,
2% (the default, under a millimetre on most controllers) or 5% of the
crossfader's travel.

**The crossfader** can be switched off, so it no longer fades either side:
right-click it, or use **Settings › Mixer**.

## Master, phones and mic

The **MASTER** button in the top bar opens the output levels: **Master**,
**Phones**, **Booth**, and **Cue / Mix**, which blends cue and main in the
headphones. Its **Ableton Link** switch is described in
[Ableton Link](link.md). **Audio Settings…** opens the
outputs in Settings. Levels are kept between runs.

The arrow on the **MIC** button opens the microphone, with **ON**, a level
and one **MIC FX** slot of its own, which the decks' effects never touch.
Auxiliary inputs play on a deck from the **Jam** source. Choose
the inputs in **Settings › Audio**: **+** under the inputs adds a second
and third auxiliary input, and each auxiliary input takes a pair of
channels (stereo) or one (mono).

**If the sound device goes away** (an audio interface unplugged, or a dock
disconnected), the decks stop where they are and stay usable: you can load,
cue and seek, and a message says what happened. openDJ plays on that device
again as soon as it's back, or pick another one in **Settings › Audio**.
