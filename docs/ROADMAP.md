# Roadmap

Goal: the best DJ app on every device. Everything Mixxx, the DJ Sugar fork, OpenDJ and
OpenFadeR do, everything Serato, rekordbox, VirtualDJ, Traktor, djay and Engine DJ are loved
for, then our own ideas on top. Other projects are studied for behaviour and ideas only; every
line in this repository is our own Rust, dependencies included (docs/study holds the studies).

Targets: Android (Samsung DeX first-class), iOS and iPadOS, Linux, Windows, macOS.
Branches: `main` (the Mixxx-based app) and `rust` (this rewrite). Nothing else.

Status: [x] done, [~] in progress or partial, [>] done in the old Rust base, to port,
[ ] not started. Every [x] is backed by a test, a screenshot in docs/screens or a CI check.

## Rules (apply to every milestone)

- Real-time audio thread: no allocation, locks, syscalls, logging or panics; lock-free rings
  only; an allocation-failing guard checks the callback in tests.
- Measured, not assumed: benchmarks for DSP, decoding, analysis, library and waveforms;
  regressions fail CI. Profile before optimising. SIMD where it pays.
- Rust for everything, dependencies included; C only where an OS ABI forces it (AAudio,
  CoreAudio, WASAPI, Vulkan loader). Every dependency has a written reason.
- Latest stable dependencies; majors are taken as soon as they release, after tests pass.
- One app everywhere: same features and layouts on every platform. Layouts adapt to size
  and input, never to the OS. Resizing is fluid everywhere, Samsung DeX first.
- OLED: true black base; only live elements light up.
- No code or UI identical to any other app; own look, own names.
- Docs hold facts only: no emoji, no symbols, no HTML, no comment walls in code.
- No build reaches the user before it has been checked at phone landscape (915x412),
  phone portrait (412x915), DeX (1280x800 and up), tablet, desktop, QHD and 1024x600.

## Done so far (rust branch)

Foundations
- [x] Cargo workspace, edition 2024, fat LTO, one codegen unit, panic abort, stripped release
- [x] sugar-core: shared types
- [x] sugar-layout: constraint layout solver, no allocation per solve, priorities hide
      modules that do not fit, property tests across sizes
- [x] sugar-ui: own wgpu renderer, instanced rounded rects and outlines, own glyph
      rasterizer and atlas (ttf-parser), text measuring, clipping, draw layers for overlays
- [x] sugar-shell: pure-Rust X11 window (x11rb), keyboard via keysyms with shift and Greek
      layouts, offscreen GPU render for screenshots
- [x] Android: native activity, freely resizable (DeX, split screen, pop-up), 16 KB page
      aligned, signed APK, bundled samples, build script, permission request for audio files
      through JNI, library rescan on resume
- [x] True black OLED palette: deck A pink, deck B cyan, accent yellow, play green

Engine (sugar-engine)
- [x] 4 decks, any output rate, cubic resampling, gain, 3-band EQ with kills, filter,
      crossfader, channel volumes, peak meters
- [x] Lock-free command ring, triple-buffered telemetry, retired tracks freed off the audio
      thread
- [x] Play, cue, seek, rate, nudge
- [x] Key lock and key shift through sugar-stretch, allocation-free test
- [x] Beat loops from 1/4 to 32 beats, sample-exact, loop exit
- [x] Sampler: 16 voices, one-shot pads
- [x] Echo per channel: beat-synced time, ping-pong feedback, preallocated 4 s line, tail keeps
      ringing after stop (echo out)
- [x] Vinyl brake and backspin with tested stop distance
- [x] 12 engine tests including allocation-free render

Decode, stretch, analysis
- [x] sugar-decode: symphonia, every codec and container it supports; all bundled samples
- [x] sugar-stretch: own phase vocoder ported from the earlier crate, presets, 9 tests
- [x] sugar-analysis: BPM by onset flux over 70 to 180 BPM, beat grid with first beat
- [x] Key detection: chroma by Goertzel bins, major and minor profiles, Camelot and note names,
      tested on synthetic progressions
- [~] sugar-io: AAudio exclusive low-latency output written, silent sink elsewhere; not yet
      run on a device

App (djsugar)
- [x] Landscape pro screen: top bar, two scrolling waveforms, overview, two decks, mixer,
      library, side panel; reflows from 780x360 up to desktop
- [x] Portrait vinyl face: platter ring holding both waveforms, BPM and crossfader in the hub,
      stacked decks for narrow widths
- [x] Frequency-coloured waveforms: low, mid and high peaks per bin, played part dimmed
- [x] Beat ticks with bar markers, loop shading, playhead
- [x] Jog platters, play and cue, rate fader, BPM and key per deck
- [x] Pad modes: CUE (8 hot cues snapped to beats), LOOP, SMP (samples), FX (echo, echo out,
      brake, spin)
- [x] SYNC: tempo match with half and double tempo handling, beat phase alignment
- [x] KEY lock toggle
- [x] Mixer: gain, 3-band EQ, filter knob, channel faders, crossfader
- [x] Library: bundled samples plus a background scan of music folders (DJSUGAR_MUSIC,
      Android Music and Download)
- [x] Search: type anywhere, words, negation with a minus sign, accent folding, Greek and
      Greeklish both ways, Enter loads the top result
- [x] Browser overlay on phones where the library is hidden
- [x] Synced lyrics from .lrc beside the track: repeated stamps, offset tag, current line
      follows playback
- [x] Auto DJ (basic): plays the shown list, syncs and crossfades over 16 beats before the
      end, reloads the idle deck; smart version below needs the owner's design approval
- [x] F1 and F2 play decks A and B

Quality
- [x] 55 tests across the workspace, clippy clean with all warnings denied
- [x] CI on every push and pull request: hygiene (no HTML, no other app names in code, no
      emoji or symbols, no comment walls, no leftover markers, no file over 2 MB), typos,
      rustfmt, cargo-deny (advisories, licenses, sources, bans), unused dependency check,
      locked lockfile, clippy per target (Linux, Windows, macOS, iOS, Android), debug and
      release tests, docs with warnings denied, miri on core and layout, release builds with
      size budgets, APK alignment and signature checks, artifacts for every platform
- [x] Studies: DJ Sugar Mixxx fork, OpenDJ and OpenFadeR, commercial apps (not fact-checked)

## Parity track (approved direction)
The user approved the DJ Sugar based landscape screen. Every remaining screen and feature follows the same method: read the DJ Sugar QML or C++ source, the OpenDJ and OpenFadeR sources and the parity specs in docs/parity, then rebuild it in our own code and verify it with screenshots at 915x412 and 1280x800.

Done:
- Landscape screen from the DJ Sugar Android main window: toolbar with DJ, WAVE, LIB and CTL views, LOAD 1 and 2, FX bar, sampler bar, Auto DJ, search.
- Stacked colour waveforms with beat lines, cue flags and loop region.
- Deck: info bar with remaining time, BPM, Camelot key and pitch; overview; REV, SLIP, Q, EJECT; PLAY and CUE; HOT, LOOP, SMPL and FX pad modes; loop IN, OUT, size, toggle; beat jump with size; SYNC, pitch fader, key lock and semitone steps.
- Mixer: HI, MID, LOW, filter, gain, faders, segmented VU, PFL, crossfader.
- Engine: reverse, slip, unload.
- Renderer: vertical and radial gradients, soft glow and shadow, arcs and pie wedges, lines. Decks use a platter with a glowing position ring, candy transport, shaded pads with press feedback, arc knobs, shaded fader caps.

Next, in order:
- Portrait layout from the DJ Sugar portrait QML.
- Done: hot cue popup on long press (8 colours, clear, close), colours on pads, waveform and overview.
- Done: beat grid popup from the BPM readout: half beat, downbeat here, 10 ms earlier and later, BPM halve and double, match other deck phase, undo stack, lock.
- Quick load drawer with local and streaming tabs; re-analyze from the grid popup.
- Headphone cue output for PFL, cue mix and split cue.
- File, Edit and View menus and the settings pages.
- Done: library with title, artist, BPM, Camelot key and time columns filled by a background analyzer, tap-to-sort headers, select then DECK 1 or DECK 2 bar.
- Library rating, crates, playlists, history, sidebar.
- Sampler grid with banks, FX station with chained effects and the XY pad.
- Stems overlay, microphone ducking panel.
- OpenDJ and OpenFadeR features and QoL items from docs/parity, item by item.
- Commercial QoL items from docs/parity/commercial.md.

## M0 Foundations, remaining
- [ ] Run the APK on the S23 Ultra: launch, touch, resize in DeX, rotation
- [ ] Multi-touch on every shell: two faders at once, pinch zoom on waveforms
- [ ] Stylus (S Pen): hover, pressure for scratch, side button
- [ ] Mouse wheel on every control; right-click and long-press menus
- [ ] Pure-Rust Wayland shell with fractional scaling and dmabuf presentation
- [ ] Windows and macOS shells checked by hand, HiDPI, window state saved
- [ ] Settings storage: own format, atomic writes, versioned migrations
- [ ] Crash-safe state: decks, loops and mixer restored after a kill
- [ ] Logging off the audio thread, ring buffered, exportable from the Help menu

## M1 Spec and design
- [ ] docs/SPEC.md: every feature with source, priority and status; screen anatomy per form
      factor; interaction rules; palette and type scale
- [ ] Mockups for phone landscape, phone portrait, foldable, tablet, DeX, desktop, QHD,
      1024x600 and booth view; owner approval before polish
- [ ] Own icon set drawn in code, no icon fonts
- [ ] Theme system: OLED black default, dim, high contrast, accent colours per deck
- [ ] UI scale and Lite, Pro and Booth densities

## M2 Audio engine, remaining
- [ ] Real output on every OS: AAudio verified on device, CoreAudio (macOS, iOS), WASAPI
      exclusive and shared, pure-Rust ALSA over ioctls, PipeWire native
- [ ] Device picker, sample rate and buffer size choice, latency shown in ms
- [ ] Headphone cue per deck, cue mix and split cue, booth output, any bus to any output pair
- [ ] Master limiter and clip meter, master EQ, balance, mono switch
- [ ] Streaming decode into page-aligned buffers for long tracks; memory budget per device
- [ ] Gapless decode, resample quality settings (cubic, sinc), dither on 16-bit outputs
- [ ] 4 decks in the UI, deck layering on 2-deck screens
- [ ] Slip mode, reverse, censor (reverse roll), vinyl stop speed setting
- [ ] Pitch bend and nudge with ramp, pitch range 4, 8, 16, 50, 100 percent
- [ ] Scratch: platter drag with inertia, scratch filter, touch-down stop
- [ ] Quantize for cues, loops, samples and play
- [ ] Beat jump forward and back by 1/4 to 64 beats; loop move and loop halve and double
- [ ] Loop rolls, saved loops, loop in and out by hand with fine adjust
- [ ] Hot cues up to 36 per track, colours, names, cue to loop
- [ ] Sampler: banks, loops, sync to master, per-pad gain and colour, sample recording
- [ ] Metronome, master clock, master deck choice, tempo lock to master
- [ ] Microphone and aux inputs with ducking and talkover
- [ ] Allocation-free guard installed on the real callback in debug builds
- [ ] Acceptance: two tracks mixed on the S23 Ultra, under 10 ms output, one hour without a
      dropout

## M3 Pro screen
- [ ] Menu bar (File, Edit, View, Deck, Library, Mix, Tools, Window, Help) wired to commands
- [ ] Command palette searching every action, track and setting
- [ ] Widgets: knob, fader, jog platter, pad, button, list, menu, text field, sheet, dialog,
      tooltip, toast
- [ ] Waveform styles: RGB, 3-band stacked, mono, stems; zoom, stacked and side by side,
      beat grid edit by drag
- [ ] Vertical waveforms option for portrait and booth
- [ ] Spring motion, 120 Hz, no frame over 8 ms, redraw only on change when idle
- [ ] Keyboard map for every control, editable
- [ ] Undo and redo for cues, loops, grid edits and library edits
- [ ] Acceptance: every control works by touch, mouse and keyboard at every checked size

## M4 Analysis and library
- [ ] Better BPM: tempo changes, half and double resolution, confidence, manual tap
- [ ] Dynamic beat grid with tempo sections, downbeat and bar detection
- [ ] Key: detection confidence, key change sections, open key and classic notation choice
- [ ] ReplayGain and loudness (EBU R128), auto gain on load
- [ ] Phrases, energy, intro and outro, vocal detection
- [ ] Waveform pyramid cached on disk, analysis queue with progress, background priority
- [ ] Accuracy checks against public datasets (GiantSteps tempo and key) in CI
- [ ] Library database in our own Rust store: tracks, tags, play counts, ratings, colours,
      comments, cue and loop data
- [ ] Tag reading and writing (ID3, Vorbis comments, MP4, FLAC, APE), cover art
- [ ] Playlists, crates, smart crates with rules, history per session, prep list
- [ ] Search field syntax: bpm:120-128, key:8A, artist:, genre:, year:, rating:, played:
- [ ] Sort by any column, column layout per size, compatible keys and BPM highlight
- [ ] Library health: relink moved files, duplicates by fingerprint, missing files report
- [ ] Watch folders for changes; Android media store; iOS Music library and Files

## M5 DJ Sugar features
- [ ] All 195 DJ Sugar samples bundled with a picker, favourites and pad banks
- [ ] Sample editor: trim, fade, gain, loop points
- [ ] FX Station: scenes, isolate, X-Pad, release; XY touch pad overlay
- [ ] Synced lyrics: embedded tags (SYLT, LYRICS), online sources, karaoke view, word timing
- [ ] Listen mode: a calm player view for listening, no DJ controls
- [ ] Streaming: YouTube (own extractor, SponsorBlock), SoundCloud, Spotify, Audius,
      Subsonic, Jellyfin, Plex; streaming hub, smart URL box, next track prefetched
- [ ] Smart Auto DJ: energy, key and phrase scoring, phrase-aligned transitions, transition
      styles; design approved by the owner first
- [ ] Latency calibration with the microphone [>]
- [ ] Touch layouts: Lite and Booth, UI scale

## M6 Effects
- [ ] Every Mixxx built-in effect in our own code: reverb, delay, flanger, phaser, bitcrusher,
      autopan, tremolo, gate, compressor, distortion, filters, pitch shift, white noise,
      loudness contour, metronome, balance, glitch, moog ladder
- [ ] DJ Sugar's own effects
- [ ] Beat FX units with chains, wet and dry, beat fractions, presets
- [ ] Colour FX per channel on the filter knob (filter, noise, space, dub echo, crush)
- [ ] Stem FX
- [ ] Plugin hosting on desktop (CLAP, LV2, VST3)

## M7 Controllers and DVS
- [>] HID in Rust: hidraw, Windows HID, IOKit (old `hid` crate)
- [>] JS mapping interpreter in Rust, all 149 bundled mappings load (old `js` crate)
- [ ] USB MIDI on every OS, Android MIDI, Core MIDI, BLE MIDI with pairing
- [ ] Hotplug and auto-detect by VID/PID and BLE name; DDJ-FLX4 first
- [ ] MIDI learn, Map mode, Quick map wizard [>]
- [>] DVS timecode decoder for common vinyl timecodes (old `timecode` crate)
- [ ] DVS interfaces and controller sound cards: routing per device [>]
- [ ] Jog screens and motorised platters
- [ ] Mapping translator: Mixxx XML and JS into our own mapping format
- [ ] Controller LED feedback, VU meters, jog screen artwork

## M8 Library exchange
- [>] Import and export both ways: rekordbox (XML, master.db, USB PDB and OneLibrary),
      Serato (database, crates, file markers), Traktor NML, Engine DJ, VirtualDJ, Mixxx,
      djay, Apple Music
- [>] USB export for CDJ/XDJ and Engine players; two-way sync manager
- [ ] Dry-run preview and conflict report for every import
- [ ] Cloud sync of library metadata between devices (own server-free format)

## M9 Recording, broadcasting and sync
- [>] Recording (WAV, FLAC, MP3, Opus, all pure Rust)
- [>] Icecast and Shoutcast broadcasting
- [>] Ableton Link in pure Rust (old `link` crate); needs a live test
- [ ] OSC and remote control from a second device
- [ ] Set history with timestamps, export as tracklist

## M10 Stems
- [>] Separation on device, STEM files, per-stem volume and mute
- [ ] Real-time separation on the S23 Ultra; stems cached per track
- [ ] Stem pads: acapella, instrumental, drums only, quick swaps

## M11 Mods
- [>] Mod core: manifest, load order, slots with provenance, live reload (old `mods` crate)
- [ ] Look, layout, pad mode, mapping, effect and source mods; sandboxed WASM sources
- [ ] Settings > Mods; store builds ship without source mods

## M12 Per-device tuning
- [ ] Device profile database with first-run benchmark
- [ ] Graphics per device: Vulkan, Metal, DX12; Adreno, Mali, Xclipse workarounds; 120 Hz
- [ ] Audio per device: AAudio MMAP, Samsung low-latency, ADPF hints, Audio Workgroups, MMCSS
- [ ] Runtime SIMD dispatch, big.LITTLE thread placement, power and thermal profiles
- [ ] Profile-guided optimisation in release builds, binary size tracked per commit

## M13 Platforms and release
- [ ] iOS and iPadOS app bundle, signing, TestFlight (needs a Mac or a macOS CI signer)
- [ ] Windows installer, macOS dmg with notarisation, Linux AppImage and .deb, Flatpak
- [ ] Android release keystore in CI secrets, Play and F-Droid builds
- [ ] Store builds without download sources; full build on GitHub releases
- [ ] Release notes generated from commits; version shown in Help > About

## Quality, ongoing
- [ ] Benchmarks in CI with regression limits (engine render, decode, analysis, layout, paint)
- [ ] Screenshot tests at every checked size compared against approved images
- [ ] Fuzzing for decoders, lyric and playlist parsers, library import
- [ ] Coverage report; every engine command has a test
- [ ] Android emulator run in CI: launch, render a frame, play a sample

## Later
- Lighting: DMX, Art-Net, sACN, fixtures, beat-driven scenes
- Visuals and video mixing
- Karaoke mode with pitch scoring
- Collaborative sets across two devices over the network
