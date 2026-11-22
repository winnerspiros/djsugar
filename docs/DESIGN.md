# DJ Sugar: design from scratch

Status: draft for agreement. Nothing in this repository is derived from openDJ, OpenFadeR, Mixxx
or any other DJ application. Ideas are studied; code, layouts and look are our own.

## 1. What the reference apps do best

| App | What is worth beating |
| --- | --- |
| DJ Sugar (Mixxx fork) | Streaming in the library: YouTube, SoundCloud, Spotify loaded straight to a deck, fast parallel downloads, SponsorBlock. Phones as a real target (Oboe, touch UI). DDJ-FLX4 over USB and BLE MIDI. Mixxx's huge set of controller mappings, recording, broadcasting, vinyl control. Free and open. |
| openDJ | Rust end to end. Groove box, looper and synth decks. Band FX (an effect on only the lows, mids or highs). One Release button that lets every effect go in its own way. A mixed-set editor that can redo a transition. |
| OpenFadeR | Layout and theme are data: one file restyles and rearranges the whole window while the app runs. Several layouts over one shared playback state. Stems computed lazily, only when a stem control is first used, then cached. Reads and writes the libraries of rekordbox, Traktor, Serato, VirtualDJ and Engine DJ. USB export, DMX lighting, an edit timeline. |
| Serato | Calm, uncluttered screen; stable under any load; low CPU. Parallel and stacked waveforms that make beatmatching readable at a glance. Instant stems. Reference DVS. Hardware works the moment it is plugged in. |
| VirtualDJ | Real-time stems with no pre-analysis. Sandbox: rehearse the next transition in the headphones while the audience hears the live mix. Everything scriptable and skinnable. Any controller. Video and karaoke. |
| rekordbox | Library preparation: phrase analysis with coloured sections, My Tag, intelligent playlists, related tracks, cloud library sync, export to CDJs. Lighting driven by phrases. |

## 2. Principles

1. One state, any shape. The screen is a function of window size and what the DJ is doing.
   There are no fixed layouts and no breakpoints. Modules declare their minimum, preferred and
   maximum size and a priority. A layout solver places them for any width and height, from a
   phone in portrait to a 4K desktop or Samsung DeX. Resizing animates continuously.
2. The mix is the subject, not the decks. The main view shows both tracks on one shared beat
   ruler: what plays now, where the next track will come in, and how the phrases line up.
3. Prepare less, play more. Analysis (tempo, key, phrases, energy, waveform) runs in the
   background as soon as a track is seen. Stems come in real time. Streaming tracks behave like
   local files.
4. Never drop audio. The audio thread never allocates, locks or waits on I/O. Every UI and
   analysis load is measured against that rule in CI.
5. Everything is data. Themes, layouts, controller mappings, effect chains and pad modes are
   files. Users can change them without rebuilding, and share them.
6. Calm by default, deep on demand. The first screen shows only what a set needs. Every power
   feature is one gesture or one command away and never clutters the default view.

## 3. Our own ideas

- Mix lane. A full-width, beat-aligned strip that shows both tracks on one ruler. The incoming
  track is drawn at its future position, so the DJ sees which phrase of A meets which phrase of
  B before pressing play. Dragging B along the lane moves its cue-in by whole phrases.
- Transition planner. Choose the next track and DJ Sugar proposes a cue-in point, an EQ swap
  and an effect, aligned to phrases. Rehearse it in the headphones (Sandbox), then perform it
  by hand, let the app perform it, or mix the two. One undo restores the state from before the
  transition.
- Command bar. One input that searches local and streaming tracks and also runs actions:
  "load to B", "loop 8", "echo out", "export USB".
- Up next. Suggestions ranked by key fit, tempo, energy direction, play history and genre.
  Each one says why it was picked. Streaming results appear alongside local ones.
- Two faces, one state. Landscape (phone, tablet, desktop) is a full professional layout:
  stacked scrolling waveforms across the top, two complete decks (jog, pitch, hot cue / roll /
  slicer / sampler / stems / key pad modes, loops, stem mutes, FX unit with band and release)
  around a full mixer (trim, three-band EQ, filter, meters, channel faders, crossfader,
  headphone cue), and the library below with sources, track table and Up next. Small
  landscape screens drop the library and FX rows behind a button and enlarge touch targets.
  Portrait is the vinyl face: one platter ring holding both waveforms, the mixer in its hub,
  decks under it.
- Set memory. Every set is recorded as events: loads, cues, faders, effects. Any set can be
  replayed, edited, rendered to audio, or turned into a playlist.
- Phone-first touch. One-thumb controls in portrait, two-thumb in landscape. Every hit target
  is at least 9 mm. A phone can act as a remote screen for a desktop session.

Carried over from DJ Sugar: streaming sources and SponsorBlock, BLE MIDI and the DDJ-FLX4
mapping, our own JavaScript engine for Mixxx mappings (crates/js, written by us), and our
importers and exporters for other apps' libraries once they have been checked for third-party
code.

## 4. Architecture (Rust only)

```
crates/
  core       time, beats, keys, ids; no dependencies
  dsp        SIMD kernels: time-stretch, resample, EQ, filters, effects
  engine     real-time graph: decks, mixer, FX, sampler; lock-free command and telemetry rings
  io         audio back ends: CoreAudio, WASAPI/ASIO, ALSA/PipeWire/JACK, AAudio
  decode     audio decoding (symphonia)
  analysis   tempo, beat grid, key, phrases, energy, waveform pyramid; worker pool
  stems      real-time source separation, cached per track
  library    track database, smart crates, history, import/export adapters
  sources    YouTube, SoundCloud, Spotify, local folders
  control    MIDI, HID, BLE MIDI; mapping runtime on crates/js
  layout     constraint solver for the fluid layout
  ui         renderer (wgpu), widgets, theme, text, input
  app        wiring, settings, platform shells (desktop, Android)
```

Decisions:
- UI: our own renderer on wgpu and winit, not Slint. Waveforms, the hub and the mix lane are
  GPU geometry at the display's refresh rate. A custom renderer is the only way to get fluid
  layouts and our own look without inheriting a toolkit's look. Text rendering uses a small
  shaping and glyph crate.
- Engine threading: one real-time thread per audio device, with commands in and telemetry out
  over SPSC rings. Analysis, decoding and stems run on a worker pool below real-time priority.
- Memory: pre-allocated pools sized at start. Track audio is decoded into memory-mapped,
  page-aligned buffers, so a 10-minute track does not need its full size in RAM on a phone.
- Dependencies: every crate added to the tree needs a written reason in deny.toml. Pure Rust is
  preferred.
- Tests: an allocation guard on the audio callback, golden-file tests for DSP, layout solver
  property tests across sizes from 320x480 to 3840x2160, and screenshot tests per form factor.

## 5. Order of work

1. core, layout solver and the ui skeleton: one window that renders the mockup's structure
   fluidly at every size on desktop and Android.
2. engine, io, decode: two decks, mixer, time-stretch; allocation guard in CI.
3. analysis, then library with local folders and the command bar.
4. Mix lane and Up next.
5. sources (port DJ Sugar's streaming), control (port crates/js and mappings).
6. stems, Sandbox, transition planner, set memory.
7. Importers and exporters, USB export, recording and broadcast.
8. Video, lighting, Automix (Automix only after its design has been agreed).
