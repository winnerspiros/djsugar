# Send decks to a DAW

Each deck, and the sampler, can play on its own outputs as well as in the
mix. A DAW such as Ableton Live, Bitwig or Logic can then record every deck
on its own track, or put its own effects on each one.

## Set it up

1. Open **Settings › Audio**.
2. Under the outputs, press **+** and pick **Deck 1** (then **Deck 2**, and
   so on) in the first column.
3. Give each deck its own channels on the device, after the master's: for
   example master on **1 & 2**, Deck 1 on **3 & 4**, Deck 2 on **5 & 6**.
4. Pick what the deck outputs carry under **Deck outputs**:
   - **Clean**: the deck after tempo, key lock, stems and gain, before EQ,
     filter and deck FX. For adding your own effects in the DAW.
   - **Pre-fader** (the default): after EQ, filter and deck FX, before the
     channel fader and crossfader.
   - **Post-fader**: what each deck adds to the master mix. Together the deck
     and sampler outputs add up to the master mix before the master effects,
     so the DAW can rebuild the mix and run its own master chain.

DJ Sugar still mixes as usual: master, headphones, recording, Automix and Link
all keep working.

Keep the deck outputs on the same device as the master. Outputs on the same
device stay sample-aligned; on two devices each runs on its own clock and they
drift apart a little, which Settings warns about.

## On the same computer

To reach a DAW on the same computer, use a virtual device with enough channels
as the master's device, and pick it as the DAW's input.

- **macOS**: [BlackHole 16ch](https://existential.audio/blackhole/) or
  Loopback. To hear the master too, make an aggregate device in Audio MIDI
  Setup with BlackHole and your speakers or interface, and pick that.
- **Windows**: VB-Cable or VoiceMeeter, or the loopback channels of an ASIO
  interface's own mixer.
- **Linux**: PipeWire (or JACK) shows DJ Sugar's channels as ports to connect to
  the DAW's inputs.

## Ableton Live

1. In Live's **Settings › Audio**, pick the same device as the input device
   and turn on its input channels (3/4, 5/6 and so on) under **Input Config**.
2. On an audio track per deck, set **Audio From** to **Ext. In** and the
   deck's channels, and arm it, or set **Monitor** to **In** to play it
   through Live's effects.
