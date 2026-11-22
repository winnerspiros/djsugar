# Roadmap

Goal: more than every other DJ app. Everything Mixxx, the DJ Sugar fork, OpenDJ and OpenFadeR
do, everything Serato, rekordbox, VirtualDJ, Traktor, djay and Engine DJ are loved for, then
our own ideas on top. Other projects are studied for behaviour and ideas; every line here is
our own Rust (docs/study holds the studies).

This roadmap merges the earlier plans: the Mixxx fork backlog and the first Rust rewrite
(`docs/djsugar-rust-roadmap.md` on the old base). Work already done there in our own crates is
carried over and brought up to the rules below; nothing of it is lost.

Status: [x] done, [~] in progress, [>] done in the old Rust base, to port, [ ] not started.

## Rules (apply to every milestone)

- Real-time audio thread: no allocation, locks, syscalls, logging or panics; lock-free rings
  only; checked by an allocation-failing guard on the callback.
- Measured, not assumed: benchmarks for DSP, decoding, analysis, library and waveforms;
  regressions fail CI. Profile before optimising. SIMD where it pays.
- Rust for everything, dependencies included; C only where an OS ABI forces it (AAudio,
  CoreAudio, WASAPI, Vulkan loader). Every dependency has a written reason.
- One app everywhere: same features and layouts on Android, iOS, Linux, Windows and macOS.
  Layouts adapt to size and input, never to the OS. Resizing is fluid on every platform,
  Samsung DeX first-class.
- OLED: true black base; only live elements light up.
- No build reaches the user before the developer has checked it at phone landscape and
  portrait, DeX, tablet, desktop, QHD and 1024x600.

## M0 Foundations
- [x] Workspace, layout solver (no allocation per solve), property tests
- [x] Own wgpu renderer: instanced rounded rects, glyph atlas, own coverage rasterizer
- [x] Linux window over pure-Rust X11, offscreen GPU render
- [x] Android APK: native activity, freely resizable (DeX, split screen), 16 KB aligned
- [x] True black OLED palette
- [~] Input on every shell: pointer, multi-touch, wheel, keyboard (X11 keysyms, winit); stylus (S Pen) open
- [ ] Live resize verified on DeX (S23 Ultra) and every desktop

## M1 Study and spec
- [x] Studies: DJ Sugar Mixxx fork, OpenDJ and OpenFadeR, commercial apps
- [ ] docs/SPEC.md: the full feature list with source and priority per item, screen anatomy
      per form factor, palette, interaction rules
- [ ] Mockups of every form factor approved before M3

## M2 Audio engine
- [x] Engine: 4 decks, cubic resampling at any rate, gain, 3-band EQ with kills, filter,
      crossfader, lock-free commands, triple-buffered telemetry, tracks freed off the audio thread
- [x] Decode: symphonia, every codec and container; all 192 DJ Sugar samples decode in 1.5 s
- [x] Time-stretch, key lock and key shift: own phase vocoder ported, allocation-free test
- [~] io: AAudio (written, needs a device run), with low-latency/exclusive mode (Android), CoreAudio (macOS, iOS),
      WASAPI, pure-Rust ALSA over ioctls (Linux)
- [ ] Headphone cue and split cue, booth output, any mix to any output pair
- [ ] Streaming decode into page-aligned buffers for long tracks on phones
- Acceptance: two tracks mixed on the S23 Ultra, under 10 ms output, one hour without a dropout

## M3 Pro screen
- [~] Landscape pro layout and portrait vinyl ring (platter waveforms, hub mixer), working; spec pending
- [ ] Menu bar (File, Edit, View, ...) on every platform, plus command search over everything
- [ ] Widgets: knob, fader, jog platter, pad, button, list, menu, text field, sheet
- [~] Scrolling RGB and 3-band waveforms (RGB scrolling, overview, beat ticks, loop shading done);, stacked and side by side, overview, beat grid
- [ ] Spring motion, 120 Hz, no frame over 8 ms
- Acceptance: every control works by touch, mouse and keyboard

## M4 Analysis and library
- [~] BPM, beat grid and key (Camelot) done; ReplayGain, phrases, energy, intro and outro, waveform pyramid
- [ ] Accuracy against Mixxx as oracle on GiantSteps and synthetic sets
- [~] Library: folder scan done;  playlists, crates, smart crates, history, tags, ratings, colours
- [~] Search: words, negation, Greek and Greeklish, accent folding done; field syntax open
- [~] Sync (tempo + phase), 8 hot cues, beat loops, brake, spinback done; quantize, 36 cues, saved loops, beat jump, rolls, slip, censor open
- [ ] Library health: relink moved files, duplicates by fingerprint [>]

## M5 DJ Sugar features
- [~] Sample library bundled, sampler voices and pad bank done; picker, sample editor, recordings
- [ ] FX Station: scenes, isolate, X-Pad, release; XY touch pad overlay
- [~] Synced lyrics: .lrc done; tags, online sources, karaoke view open
- [ ] Listen mode
- [>] Streaming: YouTube (own extractor, SponsorBlock), SoundCloud, Spotify, Audius,
      Subsonic, Jellyfin, Plex; streaming hub, smart URL box, next track prefetched
- [ ] Smart Auto DJ: energy, key and phrase scoring, beat-matched transitions
- [ ] Microphone unit with ducking; latency calibration with the microphone [>]
- [ ] Touch: Lite and Booth layouts, UI scale

## M6 Effects
- [~] Echo (beat-synced, echo out) done; every Mixxx built-in effect, plus DJ Sugar's effects, chained units, presets
- [ ] Stem FX
- [ ] Plugin hosting on desktop (CLAP, LV2, VST3)

## M7 Controllers and DVS
- [>] HID in Rust: hidraw, Windows HID, IOKit (old `hid` crate)
- [>] JS mapping interpreter in Rust, all 149 bundled mappings load (old `js` crate)
- [ ] USB MIDI on every OS, Android MIDI, Core MIDI, BLE MIDI with pairing
- [ ] Hotplug and auto-detect by VID/PID and BLE name; DDJ-FLX4 first
- [ ] MIDI learn, Map mode, Quick map wizard [>]
- [>] DVS timecode decoder: Serato CV02, Traktor MK1, MixVibes (old `timecode` crate)
- [ ] DVS interfaces: Rane SL1-SL4, Seventy, Twelve; Pioneer DJM; Traktor Audio
- [ ] Controllers with sound cards: routing per device [>]
- [ ] Jog screens and motorised platters
- [ ] Mapping translator: Mixxx XML and JS into our own mapping format

## M8 Library exchange
- [>] Import and export, both ways: rekordbox (XML, master.db, USB PDB and OneLibrary),
      Serato (database, crates, file markers), Traktor NML, Engine DJ, VirtualDJ, Mixxx,
      djay, iTunes/Music
- [>] USB export for CDJ/XDJ and Engine players; two-way sync manager
- [ ] Dry-run preview and conflict report for every import

## M9 Performance and streaming out
- [>] Recording (WAV, FLAC, MP3, Opus, all pure Rust) and Icecast/Shoutcast broadcasting
- [>] Ableton Link in pure Rust (old `link` crate); needs a live test
- [ ] OSC and remote control

## M10 Stems
- [>] Demucs separation on device, STEM files, per-stem volume and mute
- [ ] Real-time separation on the S23 Ultra; stems cached per track

## M11 Mods
- [>] Mod core: manifest, load order, slots with provenance, live reload (old `mods` crate)
- [ ] Look, layout, pad mode, mapping, effect and source mods; sandboxed WASM sources
- [ ] Settings > Mods; store builds ship without source mods

## M12 Per-device tuning
- [ ] Device profile database with first-run benchmark
- [ ] Graphics per device: Vulkan, Metal, DX12; Adreno, Mali, Xclipse workarounds; 120 Hz
- [ ] Audio per device: AAudio MMAP, Samsung low-latency, ADPF hints, Audio Workgroups, MMCSS
- [ ] Runtime SIMD dispatch, big.LITTLE thread placement, power and thermal profiles

## M13 Platforms and release
- [ ] Pure-Rust Wayland with zero-copy dmabuf presentation
- [ ] iOS and iPadOS (needs a Mac to build)
- [ ] Windows and macOS builds checked; AppImage, .deb, .dmg, installer
- [ ] Store builds without download sources; full build on GitHub releases

## Later
- Lighting: DMX, Art-Net, sACN, fixtures, beat-driven scenes
- Visuals and video mixing
- Automix: only after its design is agreed with the owner
