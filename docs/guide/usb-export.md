# Export to USB

> **Experimental.** USB export hasn't been tried on a Pioneer or Engine DJ
> player yet.
> Check the stick on a player before you rely on it at a gig, and tell us
> how it went (see [Feedback and logs](feedback.md)).

DJ Sugar can put your lists on a USB stick that Pioneer DJ players read, the
way rekordbox's USB export does, or that Engine DJ players (Denon DJ Prime
and SC, Numark, Rane) read, the way Engine DJ does: the music, its beat
grid, cues, loops and waveforms, album art, and your playlists and folders.
One stick can carry both.

## Exporting

1. Plug in a USB stick formatted **FAT32** or **exFAT**.
2. Pick a playlist, folder, smart list or stage in the tree on the left and
   choose **File › Export › USB…** (or right-click it and choose
   **Export to USB…**). A folder takes its lists and
   subfolders with it.
3. Pick the stick under **Drive** (press **Refresh** if it isn't there yet).
4. Pick which **Players** the stick is for:
   - **All Pioneer DJ players** writes both Pioneer libraries below; it's
     right for Pioneer gear unless you know you only need one.
   - **CDJ-3000, NXS2 and older, XDJ-XZ, XDJ-RX3** read the Device Library
     (`export.pdb`).
   - **CDJ-3000X, XDJ-AZ, OPUS-QUAD, OMNIS-DUO** read OneLibrary
     (`exportLibrary.db`).
   - **Denon DJ Prime and SC, Numark, Rane** read the Engine DJ library
     (`Engine Library/Database2/m.db`). It's written the way Engine OS 4
     and 5 read it.
   - **Pioneer DJ and Engine DJ players** writes all three, for a stick you
     take to any booth.
5. Press **Export**. You can press **Stop** at any time; the stick then
   keeps the library it had.

Tracks that were never analysed are analysed first, so the first export of
a big list takes a while. Music already on the stick isn't copied again.

## What ends up on the stick

- The audio, under `Contents/Artist/Album/` on the stick, shared by the
  Pioneer and Engine DJ libraries.
- Each track's analysis: beat grid, hot cues A–H with their names and
  colors, memory cues and loops, and the waveforms players draw (blue,
  RGB and three-band).
- Title, artist, album, genre, label, key, BPM, rating, color, comment,
  year and play count. Keys are written the way DJ Sugar shows them (Camelot
  or notes, from **Settings › Analysis**).
- Album art.
- The lists you exported, in their folders.

For Engine DJ players each track gets its beat grid, hot cues 1–8 with
their names and colors, up to eight saved loops (loops on hot cue pads keep
their pad; memory loops fill the rest), the overview waveform, the main cue
(your first memory cue), key, BPM, rating and album art. The players draw
the scrolling waveform from the music themselves.

## Good to know

- **The stick's playlists are replaced.** Each export writes the stick's
  library afresh with the lists you picked. To have several lists on one
  stick, put them in a folder and export the folder.
- **A stick Engine DJ already exported to** is left alone: DJ Sugar only
  replaces an Engine Library it wrote itself. Use another stick, or move
  the `Engine Library` folder off it first.
- **A stick rekordbox already exported to** gets DJ Sugar's library in the
  same place, in place of rekordbox's, so players never see two.
- **Hot cues past H** are left out, as players have eight. The result says
  how many.
- **Tracks that can't be read** (a missing file, say) are left out, and the
  result lists them.
- Exporting the same lists again is quick: only new music is copied.

## Credits

USB export stands on the work of others:

- **[rbox](https://github.com/dylanljones/rbox)** by Dylan Jones (GPL-3.0)
  writes the Device Library, OneLibrary and the analysis files. While
  building on it we found three small bugs in its OneLibrary schema (a
  column name, a missing column and its foreign keys), which DJ Sugar works
  around and has reported upstream.
- **[Bake'n Deck](https://github.com/M-Igashi/baken)** by Masanari Higashi
  (MIT) worked out, and tested on real CDJs, how rekordbox encodes
  waveforms, cue colors, analysis folders and album art. DJ Sugar follows it.
- **[rbxport](https://github.com/chrisle/rbxport)** by Chris Le
  (GPL-2.0-or-later) documents the stick format in depth; DJ Sugar checks its
  exports against that reference.
- **[EngineX-OS](https://github.com/Cd1s/EngineX-OS)** (MIT) measured
  Engine DJ's library on Engine OS players: the `m.db` schema and how beat
  grids, waveforms, cues and loops are encoded. DJ Sugar's Engine DJ export
  follows it.
- **[rekordcrate](https://github.com/Holzhaus/rekordcrate)** (MPL-2.0) and
  the [Deep Symmetry](https://djl-analysis.deepsymmetry.org/) format notes
  were used to check DJ Sugar's sticks.
- **[EngineX OS](https://github.com/Cd1s/EngineX-OS)** (MIT) reads
  rekordbox sticks for Denon, Numark and Rane players, and its
  [OpenProLink](https://github.com/Cd1s/OpenProLink) CDJ-3000 simulator
  loads them with the same reader. DJ Sugar's tests read every export back
  with a port of that reader, which shares no code with rbox.
