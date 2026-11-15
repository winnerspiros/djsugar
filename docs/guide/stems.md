# Stems

Every track you load gets stems: DJ Sugar separates it with Spleeter as it
loads, starting with the part around the cue point, so the stems pads work
within a second or two. Nothing needs installing. The first time, DJ Sugar
downloads the stems model (about 80 MB).

The stems play over the original track: with every stem on you hear the track
exactly as it is, and turning a stem down only takes that stem out. Where a
part of the track isn't separated yet, the original plays.

## Stems pads

Open the **STEMS** pad page (right-click a pad tab to put it there). The pads
are laid out as in VirtualDJ:

| Vocal | Instru | Bass | Acapella |
|---|---|---|---|
| **Kick** | **HiHat** | **Stems FX** | **Instrumental** |

- **Vocal**, **Instru**, **Bass**, **Kick** and **HiHat** mute and unmute
  their stem. A pad is lit while its stem plays.
- **Acapella** plays the vocals alone; press it again for every stem.
- **Instrumental** plays everything but the vocals; press it again for every
  stem.
- **Stems FX** switches the pads to echo out (the pad is lit while it's on).
  See below.

Set how loud each stem is with the mixer's knobs in stems mode (the **EQ**
menu above the mixer).

On the DDJ-FLX4, press **KEYBOARD** twice for the stems pads, in the same
layout. **SHIFT** + a stem's pad plays that stem alone. On the Xone K2 and the
FLX4, the EQ knobs turn the stems while the mixer is in stems mode.

## Echo out a stem

Press **Stems FX** and the stem pads echo out instead of cutting:

- A stem's pad sends that stem into an echo and mutes it, so its echo rings
  on and fades while the rest plays dry. Press it again to bring the stem
  back (any echo still ringing fades out by itself).
- **Instrumental** echoes out the vocals; **Acapella** echoes out everything
  but the vocals. Press again to bring them back.
- The echo is half a beat, at the track's tempo. Right-click **Stems FX** to
  pick **Reverb** instead.
- The stem keeps playing for half a beat after the press, so the echo has
  something to repeat.

The pads' corners show the effect, lit on the stems that are in it. Press
**Stems FX** again for the normal pads. On the DDJ-FLX4, **Stems FX** is pad 7
of the stems pads, as on screen.

Controllers can also use Mixxx's
`[QuickEffectRack1_[ChannelN_StemM]],enabled` to send a stem into the echo,
`[ChannelN],stem_fx` (1 Echo, 2 Reverb) and `[ChannelN],stem_fx_mode`.

## High quality stems

Spleeter is quick but lets a little of each stem bleed into the others. For a
cleaner split, right-click a track in the browser, or a stems pad, and choose
**Prepare HQ Stems**. DJ Sugar separates it with HTDemucs in the
background (on the graphics card; the first time it downloads the 84 MB model),
one track at a time. When it's done the deck crossfades to the new stems in a
fiftieth of a second, so nothing clicks even with a stem muted, and the track
loads with them from then on.

The top-right stems pad shows which stems are playing: **INSTANT** for the
quick ones made on load, **HQ…** while high quality ones are being prepared
and **HQ** once they play.

## Settings

**Settings › Analysis › Stems** has:

- **Stems on load**: turn it off to save CPU on an older computer. Tracks then
  only get stems you prepare with Prepare High Quality Stems (or from an NI
  stem file).
- **High quality stems kept**: high quality stems take about 4 times the
  space of the track. Once they pass this size (10 GB at first), the stems
  played longest ago are deleted to make room. Shows how many tracks have
  them and how much space they take.
- To delete them all, use **Manage › Clear high quality stems** under
  **Settings › Collection › Storage**.
- Whether each stems model is downloaded yet.

NI stem files (`.stem.mp4`) play their own stems.
