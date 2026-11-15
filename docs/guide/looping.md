# Looping

A loop plays a stretch of the track over and over, on the beat, so you can
hold a groove while you set up the next track, or stretch an intro or outro.
Loops land on the beat grid, so the track needs a tempo first (it gets one
the first time it loads; see [When tracks are analysed](music.md#when-tracks-are-analysed)).

## The LOOP controls

In 2-Deck the **LOOP** column sits between the pads and the jog. Under its
title:

- **The loop length** in the middle box, such as **4**. Press it to start a
  loop of that many beats from where the track is playing; it turns green
  while the loop plays. Press it again to leave the loop and play on.
- **‹** and **›** either side of it halve and double the length, from 1/32
  of a beat up to 512 beats. While a loop plays they halve or double that
  loop too.
- **IN** and **OUT** make a loop of any length. Press **IN** where it should
  start (it lights grey while the waveform marks out the loop), then **OUT**
  where it should end. The loop plays at once, in lilac.

In 4-Deck each deck has the same loop box, smaller, beside PLAY and CUE.

With [quantize](decks.md#sync-and-quantize) on, IN, OUT and the start of a
loop snap to the nearest beat.

## Beat loop pads

The **BEAT LOOP** pad page has a loop of each length, as on the DDJ-FLX4:
**1/4**, **1/2**, **1**, **2**, **4**, **8**, **16** and **32** beats. Press a
pad to loop, and press it again (or another one) to leave or change it. See
[Pads](pads.md) for how to open a page.

## Loop rolls

A loop roll loops only while you hold the pad. Let go and the track carries
on from where it would have been had it never looped, so the mix stays in
phrase. Right-click a pad tab and choose **LOOP ROLL** to put the page
there. Its pads go from **1/16** to **8** beats; short rolls make stutters
and build-ups.

## Saved loops

A loop you bring in from VirtualDJ, or one saved on a hot cue, shows on its
hot cue pad with a loop sign instead of a play mark. Pressing it jumps to
the loop and plays it.

## Keyboard and controllers

- DJSugarScript has `loop 4` (or any length) and `loop_exit`, so you can put
  loops on keys in **Settings › Keyboard** (see
  [Keyboard shortcuts](shortcuts.md)).
- On a DDJ-FLX4, **LOOP IN** and **LOOP OUT** set a loop, **RELOOP/EXIT**
  starts a loop of the current length or leaves one, and the **CUE/LOOP CALL**
  buttons halve and double it. See [DDJ-FLX4](controllers/ddj-flx4.md).
