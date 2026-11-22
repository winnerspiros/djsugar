# Roadmap

Every milestone ends with a build the developer has checked at the target sizes (phone
landscape and portrait, Samsung DeX, tablet, desktop, QHD, 1024x600) before anything is shipped.
A milestone is done only when its acceptance list passes on real hardware where noted.

Status: [x] done, [~] in progress, [ ] not started.

## M0 Foundations
- [x] Workspace, layout solver with zero allocation per solve, property tests
- [x] Own wgpu renderer: instanced rounded rects, glyph atlas, own coverage rasterizer
- [x] Linux window over a pure-Rust X11 connection, offscreen GPU render, presented with PutImage
- [x] Android APK: native activity, freely resizable (DeX, split screen), 16 KB aligned
- [x] True black OLED base palette
- [ ] Input: pointer, touch, multi-touch, wheel, keyboard on every shell
- [ ] Live resize verified on DeX (S23 Ultra) and every desktop

## M1 Study and spec
- [~] Studies in docs/study: DJ Sugar Mixxx fork, OpenDJ and OpenFadeR, commercial apps
- [ ] docs/SPEC.md: one feature list with source and priority per item, screen anatomy per
      form factor, OLED palette, interaction rules
- [ ] Mockups of every form factor approved by the user before UI work in M3

## M2 Audio engine
- [ ] decode: symphonia to f32, streaming decode into page-aligned buffers
- [ ] engine: decks (play, cue, seek, rate, nudge), mixer (gain, 3-band EQ, filter, crossfader,
      headphone cue), lock-free command and telemetry rings, no allocation on the audio thread
- [ ] io: AAudio (Android), pure-Rust ALSA over ioctl (Linux), CoreAudio (macOS, iOS), WASAPI
- [ ] time-stretch and key lock
- Acceptance: two tracks play and mix on the S23 Ultra with under 10 ms output latency and no
  dropouts over a one-hour run

## M3 Pro screen
- [ ] Landscape pro layout per the approved spec: menu bar, decks, waveforms, mixer, pads, library
- [ ] Portrait vinyl ring layout
- [ ] Widgets: knob, fader, jog platter, pad, button, list, menu, text field; all touch and mouse
- [ ] Scrolling RGB waveforms and overview at display refresh rate
- Acceptance: every control on screen works; 120 Hz on the S23 Ultra with no dropped frames

## M4 Analysis and library
- [ ] BPM, beat grid, key, ReplayGain, waveform pyramid; worker pool
- [ ] Library database, folders, playlists, crates, history, search syntax, cover art
- [ ] Sync, quantize, hot cues, loops, beat jump, rolls, slip

## M5 DJ Sugar additions
- [ ] Sample library (195 samples) with picker and pad banks
- [ ] Streaming: YouTube, SoundCloud, Spotify search and load
- [ ] Synced lyrics: tags, .lrc, online sources
- [ ] Listen mode

## M6 Effects and hardware
- [ ] Effect units with every built-in effect
- [ ] MIDI, HID, BLE MIDI, mappings, MIDI learn; DDJ-FLX4 first
- [ ] Recording and broadcasting

## M7 Advanced
- [ ] Stems
- [ ] Vinyl control
- [ ] Importers and exporters, USB export
- [ ] Automix (only after its design is agreed)

## M8 Platforms
- [ ] Pure-Rust Wayland and zero-copy dmabuf presentation on Linux
- [ ] iOS and iPadOS build (needs a Mac)
- [ ] Windows and macOS builds checked
