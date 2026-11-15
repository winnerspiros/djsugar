# Phone remote

Control Automix from your phone while you're away from the laptop: see
what's playing, change what comes next, skip a track or turn the music
down. Your phone opens a page DJ Sugar serves, so there's no app to install.

## Pair your phone

1. Make sure your phone and the computer are on the same Wi-Fi.
2. Click the phone button in the top bar and switch the remote on.
3. Scan the QR code with your phone's camera and open the link.

That's it: the phone shows the remote, and REMOTE turns green while a phone
is connected. Next time, just open the same page again (add it to your home
screen to make that one tap); the phone stays paired.

The QR code works once, for two minutes; a fresh one appears each time you
open the panel. If your phone can't scan, open the address shown under the
QR code in its browser and type the **PIN**.

The first time, macOS may ask whether DJ Sugar may accept incoming network
connections. Click **Allow**.

## What the phone can do

The remote is built around Automix: it shows what's playing and lets you
manage what comes next.

### Now playing

The phone opens on the track you hear, with its cover, title, artist,
tempo and key. The progress bar shades where the next mix happens, and the
card under the controls shows the next track and how long until it mixes
in. Tap the card to open the queue.

- **Play / pause**: pause stops the music and Automix; play picks up where
  it left off. When Automix is off, play starts it.
- **Mix now** (⏭) starts the next mix straight away.
- **Order** puts the queue in order (Shuffle, BPM low to high, Harmonic
  path and so on), as in the Automix panel (see [Automix](automix.md)),
  with **Undo** afterwards.
- **Mix settings** (left of play) sets the mix **Length** and **Style**,
  and has **Skip the next track**.
- **Timer** (far right) sets the sleep timer.
- **Volume**, under the controls, sets the master level with one drag.
- The speaker button at the top opens **Levels** (booth level and Fade out); **⋯** has the queue,
  Add tracks, the sleep timer and Stop Automix.

### Queue

Tap the queue button at the bottom right. The track playing is at the top,
then everything up next, each with its cover.

- Drag a track by its handle to move it, or tap it for **Play next**,
  **Move to the end** and **Remove from the queue**.
- You can swap the track waiting on a deck for another until 15 seconds
  before its mix. After that, and during the mix, it shows a lock and
  stays put.
- **Edit** lets you pick several tracks at once, then **Play next**, **To
  end** or **Remove** them all.
- **+** opens search (see below).
- The bar at the bottom has **Mix**, **Order**, **Levels** and **Timer**.

### Add tracks

Tap **+** in the queue to see your whole collection, sorted by title; keep
scrolling and more tracks load. Type to search it, with the same search as
the browser (try `bpm:120-124`, `key:8A` or `crate:Warm-up`). Tap **+** on
a result to add it to the end of the queue, or **Next** to play it next.
Tracks already queued show **Added**. Tap **Done** to go back to the queue;
the search clears itself for next time.

### Levels and the sleep timer

- **Levels**: the master level, and the booth level when a booth output is
  set up. **Fade out** turns the master level down to silence over eight
  seconds; **Bring the sound back** puts it back.
- **Timer**: pick 15, 30, 45, 60 or 90 minutes. When time is up the music
  fades out over 20 seconds and Automix stops. The Timer button shows the
  time left.

Everything the phone does shows in DJ Sugar straight away, and the other way
round.

## Phones and safety

- The remote is off until you switch it on, and only works on your local
  network.
- Only paired phones can control DJ Sugar. Under **Paired phones** in the
  REMOTE panel you can see which are connected and **Remove** any of them.
- After five wrong codes or PINs in a row, pairing stops for a minute.
- The remote uses plain HTTP on your network. On a public network, anyone
  watching the traffic could see it; use a network you trust (see below).

## If your phone can't connect

Hotel, venue and café Wi-Fi often keeps devices apart, so the phone can't
reach the computer even on the same network. Either:

- share the computer's connection as its own Wi-Fi network (on a Mac:
  **System Settings › General › Sharing › Internet Sharing**) and join it
  with your phone, or
- turn on your phone's hotspot and join the computer to it.

If the computer is on more than one network, the REMOTE panel lists the
other addresses under **Other networks**.

A phone that locks its screen drops the connection; the page reconnects as
soon as you look at it again.
