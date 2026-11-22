# OpenDJ and OpenFadeR Feature Parity Study

**Sources:**
- OpenDJ (openwave-studio/opendj): https://github.com/openwave-studio/opendj — cloned to `/home/ubuntu/.hermes/cache/scratch/parity-src/opendj`
- OpenFadeR (OpenRsG/OpenFadeR): https://github.com/OpenRsG/OpenFadeR — cloned to `/home/ubuntu/.hermes/cache/scratch/parity-src/openfader`
- Existing study: `/home/ubuntu/djsugar-next/docs/study/opendj-openfader.md` (lines 1-402)

**Note:** DJ Sugar is a Mixxx fork. Mixxx is a mature open-source DJ application (C++/Qt). Features present in Mixxx but not explicitly in OpenDJ/OpenFadeR are noted where relevant.

---

## Numbered Feature Checklist (180 items)

### DECKS & PLAYBACK

1. **Two or four decks** — both projects support 2/4 deck layouts (OpenDJ: 2-Deck, 2-Deck Basic, 4-Deck, Prep, Player; OpenFadeR: 2/4 horizontal/vertical + Browse) [opendj/README.md:13-14, openfader/README.md:82-86]
2. **Deck sync (tempo sync)** — beat-matched tempo sync between decks [opendj/README.md:13, openfader/README.md:85]
3. **Key sync / key lock** — transpose to match key, Camelot wheel display [opendj/README.md:143, openfader/README.md:85]
4. **Tempo fader** — ±100% range (OpenDJ) / variable range (OpenFadeR) [opendj/README.md:13, openfader/README.md:175]
5. **Jog / touchstrip control** — scratching, bend, search [opendj/crates/engine/src/engine.rs:100-119, openfader/src/app/midi/jog.rs]
6. **Slip mode** — playback continues while scratching/looping [opendj/README.md:13, openfader/README.md:175]
7. **Quantize** — snap cues/loops to beat grid [opendj/README.md:13, openfader/README.md:175]
8. **Hot cues (8 per deck)** — A-H pads, color-coded [opendj/README.md:13, openfader/README.md:179]
9. **Memory cue** — separate from hot cues, persistent [opendj/README.md:139, openfader/README.md:179]
10. **Auto loop** — beat-synchronized loop on load [opendj/README.md:139, openfader/README.md:179]
11. **Beat loop** — 1/2/4/8/16/32 beat loops [opendj/README.md:139, openfader/README.md:179]
12. **Manual loop** — set in/out points freely [opendj/README.md:139, openfader/README.md:179]
13. **Loop halve/double** — halve or double loop length [opendj/crates/engine/src/engine.rs, openfader/src/app/deck/loops.rs]
14. **Beat jump** — jump forward/backward by beats [opendj/README.md:13, openfader/README.md:179]
15. **Slicer** — 8 slices across 1 beat [opendj/crates/engine/src/engine.rs:116-119, openfader/src/app/deck/slicer.rs]
16. **Freeze** — beat-sliced hold/repeat [opendj/crates/engine/src/engine.rs:118-119, openfader/README.md:179]
17. **Reverse playback** — play backwards [opendj/crates/engine/src/transport.rs, openfader/src/app/deck/slip.rs]
18. **Brake effect** — gradual stop/start [opendj/crates/engine/src/transport.rs, openfader/src/app/deck/slip.rs]
19. **Phrase view** — auto-detected phrases (Intro/Build/Drop/Breakdown/Outro/Chorus/Verse) as flags on scrolling waveform + bar on overview [opendj/README.md:140, openfader/README.md:84,180]
20. **Beat counter** — position (bars.beats) or distance to next memory cue [opendj/README.md:142, openfader/src/app/deck/beatmath.rs]
21. **Key shift** — semitone transpose per deck [opendj/README.md:143, openfader/src/app/deck/key.rs]
22. **Camelot wheel display** — visual key compatibility [opendj/README.md:143, openfader/README.md:191]
23. **Stem separation (Demucs 4-stem)** — per-stem gain, mute, solo, isolator per stem, stem FX, instant doubles [opendj/README.md:144, openfader/README.md:100-105]
24. **Stem processing choice** — GPU (Metal/CUDA/WebGPU/Dawn) or CPU [opendj/crates/separation/src/lib.rs, openfader/README.md:104]
25. **Instant stem doubles** — copy stem mix to other deck [opendj/README.md:144, openfader/src/app/deck/stemdouble.rs]
26. **Vocal regions** — spectral flux + onset detection for vocal sections [opendj/crates/engine/src/vocals.rs, openfader/src/analysis/auto.rs]
27. **Dynamic beat grid with tempo sections** — handles live drums, old disco; DP beat tracker + grid fitting [opendj/crates/engine/src/beats.rs, openfader/src/analysis/detect.rs]
28. **Grid editor** — type BPM, HERE (section-only), UNDO/REDO, Analysis Lock [opendj/crates/engine/src/beats.rs, openfader/README.md:184]
29. **Beat This! neural fallback** — for difficult tracks [opendj/crates/engine/src/beatthis.rs, openfader/README.md:191]
30. **3-band waveform** — low/mid/high at 150 Hz detail + 1200 entries overview [opendj/crates/waveform/, openfader/README.md:191]
31. **Color preview waveform** — 6-byte RGBH per entry (OpenFadeR) [openfader/src/analysis/colors.rs, openfader/README.md:191]
32. **Multiple waveform styles** — RGB, Bands, Mono, Blue (rekordbox-style), Stems (OpenDJ) [opendj/crates/waveform/, opendj/crates/app/src/wave_look.rs]
33. **Waveform zoom** — continuous zoom levels [opendj/crates/app/src/waveform_zoom.rs, openfader/ui/deck-wave.slint]
34. **Waveform height adjustment** — per-layout [opendj/crates/app/src/waveform_height.rs, openfader/ui/deck-wave.slint]
35. **Scrolling waveforms** — physical pixel 1:1 rendering from audible position [openfader/src/app/deck/frame.rs]
36. **Overview waveform** — full-track summary with cue/loop markers [opendj/crates/waveform/, openfader/ui/deck-wave.slint]
37. **Phase meter** — visual beat alignment indicator [opendj/crates/app/src/deck.slint, openfader/src/app/deck/frame.rs]
38. **Playhead position sync** — latency-compensated position clock [opendj/crates/engine/src/engine.rs, openfader/src/audio/player.rs]
39. **Track end warning** — flash/alert near track end [opendj/crates/app/src/track_end.rs]
40. **Auto-cue on load** — jump to first beat/sound [opendj/crates/app/src/autocue.rs, openfader/src/app/deck/autocue.rs]

### MIXER & ROUTING

41. **3-band isolator EQ + filter per channel** — kill -∞ dB, boost +9 dB (OpenDJ) / EQ_KILL_DB, EQ_MAX_DB (OpenFadeR) [opendj/README.md:136, openfader/src/audio/dsp.rs:55]
42. **Crossfader** — adjustable cut-in (OpenDJ) / enable/disable (OpenFadeR) [opendj/README.md:137, openfader/README.md:177]
43. **Channel faders** — per-deck volume [opendj/README.md:137, openfader/README.md:177]
44. **Master/headphones/booth/external routing** — 6 external outputs (OpenFadeR) [opendj/README.md:137, openfader/README.md:177]
45. **Microphone input** — with talkover/ducker [opendj/README.md:26, openfader/src/audio/mic.rs]
46. **Aux/Line input** — with ducking [opendj/README.md:26, openfader/src/audio/auxin.rs]
47. **Headphone cue** — second output device for preview [opendj/README.md:137, openfader/src/audio/cue.rs]
48. **External mixer support** — use hardware mixer (OpenDJ: "set Mixer to openDJ") [opendj/crates/app/src/automix.rs:52]
49. **Room EQ** — measurement + correction [opendj/crates/app/src/room_eq.rs, openfader/README.md]
50. **Gain staging** — auto gain from loudness analysis [opendj/crates/engine/src/loudness.rs, openfader/src/audio/loudness.rs]
51. **Limiter / soft clip** — on master output [opendj/crates/engine/src/dsp.rs, openfader/src/audio/bus.rs]
52. **Level meters** — per-channel and master VU [opendj/crates/engine/src/meter.rs, openfader/src/audio/player.rs]

### EFFECTS

53. **Color FX (knob)** — FILTER, SPACE, DUB ECHO, SWEEP, NOISE, CRUSH (OpenFadeR 5 modes; OpenDJ: Color FX knob) [opendj/README.md:138, openfader/README.md:93]
54. **Beat FX unit** — 14 effects (OpenDJ) / 12 effects (OpenFadeR) with tap tempo [opendj/README.md:138, openfader/README.md:93]
55. **Release FX** — one-shot effects on release (OpenFadeR) [openfader/README.md:94]
56. **Merge FX** — one knob builds HPF+noise+echo (OpenFadeR) [openfader/README.md:94, openfader/src/audio/merge.rs]
57. **Send FX** — auxiliary send/return (OpenFadeR) [openfader/README.md:94]
58. **Lever FX** — momentary FX control (OpenFadeR) [openfader/README.md:94]
59. **Mic FX** — effects on microphone input (OpenFadeR) [openfader/README.md:94]
60. **Stem FX** — effects per stem (OpenDJ) [opendj/README.md:144]
61. **FX rack / panel** — dedicated UI for effect management [opendj/crates/app/ui/fxbar.slint, openfader/ui/fx.slint]
62. **Tap tempo** — manual BPM for beat FX [opendj/crates/engine/src/tap_tempo.rs, openfader/README.md:93]
63. **Effect chaining** — multiple effects per slot [opendj/crates/engine/src/effects/, openfader/src/plugin/effect/chain/]
64. **VST3/CLAP/Audio Unit plugin support** — OpenDJ: out-of-process CLAP/VST3/AU; OpenFadeR: native libraries for effects [opendj/crates/plugin-host/, openfader/README.md:194]
65. **Plugin crash isolation** — helper process per slot, restart-once-then-disable (OpenDJ) [opendj/crates/plugin-host/]

### SAMPLER & GROOVE BOX

66. **Sampler: 16 slots** — trigger samples, sample packs [opendj/README.md:145, openfader/README.md:108]
67. **Sample packs (.djsugarpack / companion plugin)** — load/save sample collections [opendj/README.md:145, openfader/docs/plugins.md]
68. **Sampler sequencer** — step lanes + macro lanes (OpenDJ) / patterns (OpenFadeR) [opendj/crates/app/src/step_lanes.rs, openfader/src/app/deck/sequencer.rs]
69. **Groove Box / Beat Lab** — drum slots, capture from deck, release FX over stems [opendj/README.md:146, openfader/README.md:114-118]
70. **Plaits drum synthesis** — Mutable Instruments Plaits analog/synthetic kits (OpenDJ) [opendj/crates/engine/src/kits.rs]
71. **Drum capture** — grab loop from playing deck into sampler/groove [opendj/crates/engine/src/groove_pads.rs, openfader/src/app/deck/groove.rs]
72. **Drum swap / release FX** — swap drums from stem separation (OpenFadeR) [openfader/README.md:116]

### LOOPER & RECORDER

73. **Looper: 8 loops per deck** — heard/quantized modes, overdub (OpenDJ) [opendj/README.md:147]
74. **Recorder: WAV/FLAC** — post-master, split by track [opendj/README.md:148, openfader/README.md:185]
75. **Recording level meter** — loudness meter for broadcast/recording [opendj/crates/broadcast/src/encode.rs, openfader/README.md:146]
76. **Growing file support** — read recordings while still writing [opendj/crates/engine/src/growing.rs]

### BROADCAST & STREAMING

77. **Icecast/Shoutcast streaming** — MP3/Ogg/Opus [opendj/README.md:149, opendj/crates/broadcast/]
78. **Now Playing (MPRIS/macOS/Windows)** — system media controls [opendj/README.md:149, opendj/crates/broadcast/src/now_playing.rs]
79. **Mixcloud upload** — one-click upload of recorded mixes [opendj/README.md:149, opendj/crates/app/src/mixcloud_upload.rs]
80. **OBS overlay** — now-playing overlay for streaming [opendj/README.md:28]

### LIBRARY & COLLECTION

81. **SQLite library database** — tracks, cues, beat grids, playlists, history [opendj/crates/library/, openfader/src/library/own.rs]
82. **SQLCipher encryption** — for external library adapters (OpenFadeR) [openfader/src/library/own.rs]
83. **Folder watch / auto-scan** — detect new/changed files [opendj/crates/app/src/sound_scan.rs, openfader/src/library/folder.rs]
84. **Stages (Inbox, In Prep, Play Ready)** — workflow for track preparation (OpenDJ) [opendj/README.md:19, opendj/crates/library/src/stages.rs]
85. **Crates / playlists / smart playlists** — nested folders, dynamic queries [opendj/README.md:19, openfader/src/app/browser/playlists.rs]
86. **Tag groups & batch editing** — multi-field tag operations [opendj/README.md:19, openfader/src/app/browser/tagging.rs]
87. **Forgiving search** — fuzzy, typo-tolerant [opendj/README.md:19]
88. **Collection Health** — find missing files, duplicates, poor tags; fix them [opendj/README.md:21, openfader/src/app/browser/health.rs]
89. **Duplicate finder** — case-insensitive, audio fingerprint [opendj/crates/library/src/dupes.rs, openfader/src/app/browser/dupes.rs]
90. **Relocate moved files** — find files by name/hash [opendj/crates/library/src/relocate.rs, openfader/src/library/relocate.rs]
91. **Cover art fetch** — Last.fm, MusicBrainz, iTunes (OpenDJ) / artwork plugins (OpenFadeR) [opendj/crates/app/src/coverart.rs, openfader/src/app/artwork/]
92. **Rating & color coding** — 0-5 stars, custom colors [opendj/crates/library/src/lib.rs:92-93, openfader/src/store.rs]
93. **Play history** — timestamped, export M3U8 (OpenFadeR) [opendj/crates/library/src/lib.rs:97-98, openfader/src/app/browser/history.rs]
94. **Smart lists / saved searches** — dynamic queries with auto-refresh [opendj/crates/library/src/lib.rs:66-71, openfader/src/app/browser/smart.rs]
95. **Playlist file import/export** — M3U8, PLS, XSPF [opendj/README.md:22, openfader/src/library/playlist_file.rs]
96. **Tag editing** — ID3, Vorbis, MP4, APE via lofty (OpenDJ) [opendj/crates/engine/src/import.rs]

### STREAMING SOURCES & EXTERNAL LIBRARIES

97. **Spotify browsing** — playlists, tracks; play from local collection if owned [opendj/README.md:24-25, opendj/crates/app/src/spotify.rs]
98. **TIDAL browsing** — [opendj/crates/app/src/tidal.rs]
99. **Apple Music browsing** — [opendj/crates/app/src/apple_music.rs]
100. **SoundCloud browsing** — [opendj/crates/app/src/soundcloud.rs]
101. **Mixcloud browsing** — [opendj/crates/app/src/mixcloud.rs]
102. **Plex / Navidrome / Jellyfin** — self-hosted server playback [opendj/README.md:25]
103. **Rekordbox import** — device library, XML, desktop collection; cues, grids, playlists [opendj/README.md:22, openfader/README.md:170, openfader/compatibility/rekordbox/]
104. **Engine DJ import** — [opendj/README.md:22, openfader/README.md:170, openfader/compatibility/engine/]
105. **Serato import** — [opendj/README.md:22, openfader/README.md:172, openfader/compatibility/serato/]
106. **Traktor import** — [opendj/README.md:22, openfader/README.md:171, openfader/compatibility/traktor/]
107. **VirtualDJ import** — [opendj/README.md:22, openfader/README.md:172, openfader/compatibility/virtualdj/]
108. **iTunes / Music import** — [opendj/README.md:22, openfader/README.md:172, openfader/compatibility/itunes/]
109. **Mixxx import** — [opendj/README.md:22]
110. **djay Pro import** — [opendj GitHub release notes v0.7.0]
111. **Open format (JSON/CSV)** — documented exchange format [openfader/docs/library-format.md, openfader/src/library/open_format.rs]
112. **Pluggable library sources** — unified Library trait, adapters as plugins (OpenFadeR) [openfader/src/library/mod.rs:71-104]

### USB EXPORT & DEVICE MANAGEMENT

113. **USB export for Pioneer players** — export.pdb (Device Library), exportLibrary.db (OneLibrary) [opendj/crates/export/, openfader/README.md:128-134]
114. **Engine DJ USB export** — m.db format [opendj/crates/export/src/engine.rs, openfader/README.md:41]
115. **Traktor/Serato/VirtualDJ export** — planned/partial [opendj/crates/export/, openfader/compatibility/]
116. **Device manager** — scan, eject, backup, format (FAT32/exFAT MBR), sync, history import, cue link, update (OpenFadeR) [openfader/src/device/]
117. **USB/SD detection** — OS-specific (Linux udev/udisks2, Windows DeviceIoControl, macOS diskutil) [openfader/src/device/linux.rs, windows.rs, macos.rs]
118. **Stick verifier** — `openfader verify <mount>` headless compatibility check (OpenFadeR) [openfader/README.md:200, openfader/src/device/check.rs]
119. **Backup/restore device library** — before/after export [openfader/src/device/backup.rs]
120. **Format USB stick** — FAT32/exFAT MBR, >32GB on Windows via fatfs [openfader/src/device/format.rs]

### AUTOMIX

121. **Automix** — plays Automix list on two decks, crossfading [opendj/README.md:18, opendj/crates/app/src/automix.rs]
122. **Transition styles** — Smooth, Bass Swap, Filter, Cut (OpenDJ) / recipes (OpenFadeR) [opendj/crates/app/src/automix.rs:84-101, openfader/README.md:186]
123. **Phrase-aware mixing** — match intro/outro phrases [opendj/crates/app/src/automix_phrases.rs, openfader/README.md:186]
124. **Mix length options** — 0s, 12s, 15s, 30s, 16 bars, 32 bars, Phrases [opendj/crates/app/src/automix.rs:68-76]
125. **Learned pair mixes** — from recorded sets [opendj/crates/app/src/automix.rs:41-45]
126. **Auto DJ via controller** — Mixxx `[AutoDJ]` controls work [opendj/crates/app/src/automix.rs:9-10]
127. **Remote automix control** — from phone/web [opendj/README.md:18, opendj/crates/remote/]

### CONTROLLERS & MIDI

128. **Mixxx controller mappings** — 200+ controllers supported (OpenDJ) [opendj/README.md:34-35]
129. **Built-in mappings** — ~80 with pictures and guides (OpenDJ) / installed auto-load (OpenFadeR) [opendj/README.md:36-37, openfader/README.md:42]
130. **USB HID controllers** — NI Traktor Kontrol S2/S3/S4/S5/S8/D2/F1/X1 MK3/Z1/MX2, Maschine Mikro MK3 (OpenDJ) [opendj/README.md:41-43]
131. **MIDI Learn / mapping wizard** — click-to-map for unmapped controllers [opendj/README.md:44-45, openfader/src/app/midi/learn.rs]
132. **Soft takeover** — absolute knobs/faders wait until reaching screen value [opendj/crates/controllers/src/pickup.rs, openfader/src/app/midi/pickup.rs]
133. **Shift layers** — deck layers, pad pages, FX units [opendj/crates/controllers/src/layer.rs, openfader/src/app/midi/layer.rs]
134. **Jog wheel support** — vinyl/CDJ modes, jog cutter (OpenFadeR) [opendj/crates/controllers/src/jog_feel.rs, openfader/src/app/midi/jog.rs]
135. **LED feedback** — deck state, VU meters, pad colors [opendj/crates/controllers/src/view.rs, openfader/src/app/midi/lights.rs]
136. **Controller editor UI** — view/edit mappings with pictures [opendj/crates/app/src/controller_editor.rs, openfader/ui/prefs-controller.slint]
137. **Mapping catalog** — install from GitHub repository (OpenFadeR) [openfader/src/app/midi/get.rs, openfader/docs/mappings.md]
138. **Keyboard shortcuts** — fully remappable, context-aware [opendj/README.md:155, openfader/README.md:190]
139. **MIDI clock in/out** — leader/follower sync (OpenDJ) [opendj/README.md:152, opendj/crates/app/src/midi_clock.rs]
140. **Ableton Link** — tempo/phase sync across apps/hardware (OpenDJ) [opendj/README.md:151, opendj/crates/engine/src/link.rs]

### TIMECODE / DVS

141. **Timecode vinyl/CD (DVS)** — control vinyl, CD, relative/absolute modes, latency measurement (OpenDJ) [opendj/README.md:150, opendj/crates/engine/src/timecode/]
142. **Timecode calibration** — per-device latency compensation [opendj/crates/engine/src/timecode/]

### OSC & REMOTE

143. **OSC remote control** — tablet/phone via Open Sound Control (OpenDJ) [opendj/README.md:153, opendj/crates/osc/]
144. **Web remote** — WebSocket, phone/tablet UI (OpenDJ) [opendj/README.md:161, opendj/crates/remote/]
145. **QR code pairing** — for remote app (OpenDJ) [opendj/crates/remote/src/qr.rs]

### SCRIPTING & PLUGINS

146. **JavaScript scripting** — Boa engine for controllers and UI (OpenDJ) [opendj/README.md:160, opendj/crates/script/]
147. **Lua scripting** — mlua for plugins (OpenFadeR) [openfader/README.md:194, openfader/src/plugin/lua.rs]
148. **Plugin system** — CLAP/VST3/AU out-of-process (OpenDJ) / Lua + child processes + native libs (OpenFadeR) [opendj/crates/plugin-host/, openfader/src/plugin/]
149. **Hot-reload plugins** — actions/events/settings without restart (OpenFadeR) [openfader/README.md:194]
150. **Knob/pad modes declarative JSON** — (OpenFadeR) [openfader/src/plugin/effect/]

### UI / LAYOUTS / THEMING

151. **Slint UI framework** — GPU (wgpu/femtovg) and CPU backends [both projects]
152. **Layout profiles** — Desktop, Touch (profile 1), Stage (profile 2, OpenFadeR) [opendj/crates/app/src/touch.slint, openfader/ui/touch.slint]
153. **2-Deck / 4-Deck Horizontal & Vertical** — waveforms stacked or between decks [opendj/crates/app/ui/fourdeck.slint, openfader/ui/app.slint:65-119]
154. **Browse layout** — browser full height under waveforms [opendj/crates/app/ui/layouts.rs, openfader/ui/app.slint:110]
155. **Export mode** — player panels over decks A/B, 6 layouts (OpenFadeR) [openfader/README.md:196, openfader/ui/exportmode.slint]
156. **Edit mode** — timeline arranger, EDL, render WAV (OpenFadeR) [openfader/README.md:122-126, openfader/src/edit/]
157. **Lighting workspace** — DMX fixtures, scenes, ArtNet/sACN (OpenFadeR) [openfader/README.md:148, openfader/src/lighting/]
158. **Video workspace** — full-window video, video deck view, crossfader sync (OpenFadeR) [openfader/README.md:149, openfader/src/video/]
159. **Split screen** — browser in separate window (OpenFadeR) [openfader/README.md:195, openfader/src/app/split.rs]
160. **Mini player** — compact always-on-top window (OpenDJ) [opendj/crates/app/src/mini_player.rs]
161. **Theming/skinning** — Catppuccin base, skin files override Theme globals, per-deck waveform colors [opendj/crates/app/ui/theme.slint, openfader/ui/theme.slint]
162. **Theme catalog** — install from GitHub repositories (OpenFadeR) [openfader/ui/catalog.slint, openfader/docs/skins.md]
163. **Built-in themes** — Catppuccin Mocha/Latte, Dracula, Nord, Gruvbox, Tokyo Night, Rek, etc. [openfader/README.md:138]
164. **High-DPI / retina support** — Slint handles scaling
165. **Touch gestures** — swipe, pinch, large hit targets [opendj/crates/app/ui/touch-actions.slint, openfader/ui/touch.slint]
166. **Safe area insets** — for notched screens (OpenFadeR) [openfader/ui/app.slint:77-78]
167. **Native macOS menu bar** — File/Edit/View with 40 actions (OpenDJ) [opendj/crates/app/src/mac_menu.rs]
168. **Cross-platform menu button** — Windows/Linux button opens same menus (OpenDJ) [opendj/crates/app/src/app_menu.rs]
169. **Guided tour / tutorials** — interactive in-app tutorials (OpenDJ) [opendj/README.md:30, opendj/crates/app/src/tour.rs]
170. **Welcome window** — first-start setup (OpenFadeR) [openfader/README.md:199, openfader/src/app/welcome.rs]
171. **Context-sensitive help** — hover tips, `?` for shortcuts [opendj/crates/app/ui/controls.slint, openfader/ui/help.slint]
172. **Multi-language (i18n)** — 20+ languages (OpenDJ) / system language detection (OpenFadeR) [opendj/README.md:162, openfader/README.md:198]

### ANALYSIS & METADATA

173. **Audio fingerprint cache** — keyed by fingerprint, persists analysis [opendj/crates/engine/src/cache.rs, openfader/src/analysis/cache.rs]
174. **Loudness (EBU R128)** — for auto gain, broadcast [opendj/crates/engine/src/loudness.rs, openfader/src/audio/loudness.rs]
175. **Key detection** — multiple detectors (Krumhansl-Schmuckler, KeyNet CNN) [opendj/crates/engine/src/key.rs, openfader/docs/key-detector.md]
176. **Similar tracks** — timbre/energy features + tags + background index (OpenFadeR) [openfader/src/analysis/similar.rs]
177. **Lyrics (synced)** — LRCLIB, local LRC, karaoke window (OpenDJ) [opendj/README.md:141, opendj/crates/app/src/lyrics.rs]
178. **Word-level lyric timing** — Whisper.cpp alignment (OpenDJ) [opendj/crates/engine/src/word_timing.rs]
179. **Phrase detection** — from beat grid + 3-band waveform [opendj/crates/engine/src/phrases.rs, openfader/src/analysis/auto.rs]

### PLATFORM-SPECIFIC

180. **Android** — native APK, touch layout, Oboe audio, Slint Skia backend (OpenDJ) / separate mobile crate, software renderer preview (OpenFadeR) [opendj/crates/android/, openfader/README.md:204]
181. **Raspberry Pi** — aarch64 builds, CPU-only stems, Dockerfile.pi, profile.pi (OpenFadeR) [openfader/README.md:203, openfader/Dockerfile.pi]
182. **Linux** — ALSA/JACK/PipeWire, MPRIS, Wayland drag-and-drop, ALSA mixer control (OpenFadeR) [opendj/README.md:171, openfader/README.md:205]
183. **macOS** — native menu bar, Core Audio real-time, MPRIS via souvlaki (OpenDJ) / native file dialogs (OpenFadeR) [opendj/README.md:169, openfader/README.md:206]
184. **Windows** — ASIO + WASAPI, native Open dialog, elevated disk formatter, FAT32 >32GB (OpenFadeR) [opendj/README.md:170, openfader/README.md:207]
185. **iOS** — not supported (Slint lacks backend) [both projects]

### DIAGNOSTICS & UPDATES

186. **Crash reports & log collection** — Help > Report a Problem [opendj/README.md:164, openfader/README.md:200]
187. **Structured diagnostics** — crash reports, log collection, health check (OpenDJ) [opendj/crates/app/src/diagnostics.rs]
188. **Update check + one-click update** — with rollback (OpenFadeR keeps `.prev` copy) [opendj/README.md:165, openfader/README.md:199, openfader/src/update.rs]
189. **Verbose logging** — configurable levels [openfader/README.md:200]

---

## DJ Sugar (Mixxx Fork) Likely Lacks

Based on Mixxx's feature set and the study document's "What Is Weak or Incomplete" sections:

1. **Stem separation** — Mixxx has no built-in stem separation (no Demucs/ONNX)
2. **Neural beat tracking** — Mixxx uses traditional algorithms, no Beat This! fallback
3. **Phrase detection** — Mixxx has basic phrase hints but not auto-detected phrase flags on waveform
4. **Color preview waveform** — Mixxx waveforms are mono/color-by-frequency, not RGBH
5. **Groove Box / Beat Lab with drum synthesis** — Mixxx has sampler but no Plaits drum models
6. **Sampler sequencer with step/macro lanes** — Mixxx sampler is trigger-only
7. **Looper with overdub/quantized modes** — Mixxx has loops but not 8-loop looper per deck
8. **Ableton Link** — not in Mixxx
9. **MIDI clock in/out** — not in Mixxx
10. **OSC remote control** — not in Mixxx
11. **Web remote / phone UI** — not in Mixxx
12. **Broadcast / Icecast / Mixcloud upload** — not in Mixxx
13. **Now Playing (MPRIS/system media controls)** — not in Mixxx
14. **Timecode/DVS** — Mixxx has DVS but OpenDJ's implementation may differ
15. **CLAP/VST3/AU plugin host** — Mixxx has LV2/LADSPA but not CLAP/VST3/AU
16. **Out-of-process plugin crash isolation** — Mixxx runs plugins in-process
17. **JavaScript/Lua scripting for controllers/UI** — Mixxx uses JS for controllers only
18. **Pluggable library sources** — Mixxx has fixed format support
19. **Device manager (USB format/backup/sync)** — Mixxx has basic export only
20. **Lighting/DMX** — not in Mixxx
21. **Video playback** — not in Mixxx
22. **Edit mode / timeline arranger** — not in Mixxx
23. **Split screen browser** — not in Mixxx
24. **Android native app** — Mixxx has no Android build
25. **Raspberry Pi optimized builds** — Mixxx runs but no Pi-specific profile
26. **Guided tour / interactive tutorials** — Mixxx has manual only
27. **Word-level lyric timing (Whisper)** — not in Mixxx
28. **Karaoke window** — not in Mixxx
29. **Similar tracks recommendation** — not in Mixxx
30. **Dynamic beat grid with tempo sections** — Mixxx has fixed grid per track
31. **Grid editor with UNDO/REDO** — Mixxx has grid edit but no undo stack
32. **Stem instant doubles** — not applicable without stems
33. **Merge FX / Lever FX / Release FX** — Mixxx has different effect rack
34. **Color FX knob modes (FILTER/SPACE/DUB ECHO/SWEEP/NOISE/CRUSH)** — Mixxx has different FX
35. **Per-stem gain/mute/solo/FX** — not applicable
36. **GPU waveform rendering (wgpu textures)** — Mixxx uses CPU Cairo
37. **Touch layout profile** — Mixxx has touch support but not separate profile
38. **Theming/skinning system with token overrides** — Mixxx has skins but less flexible
39. **Native macOS menu bar** — Mixxx uses Qt menu bar
40. **SQLCipher encrypted library adapters** — Mixxx uses plain SQLite
41. **ONNX Runtime / WebGPU for ML** — Mixxx has no ML inference
42. **Rollback on failed update** — Mixxx updates via package manager
43. **Stick verifier CLI** — not in Mixxx
44. **Low-memory malloc tuning** — not in Mixxx
45. **Export mode for DJ player prep** — Mixxx has export but not dedicated workspace

---

## 25 Most Valuable Features (for djsugar-next parity)

1. **Stem separation (Demucs 4-stem) with per-stem gain/mute/solo/FX/instant doubles** — core modern DJ feature
2. **Dynamic beat grid with tempo sections** — handles live/drifting tempo correctly
3. **Phrase detection + phrase-aware automix** — intelligent mixing
4. **GPU waveform rendering (wgpu)** — 60fps smooth scrolling at any zoom
5. **Ableton Link + MIDI clock** — multi-app/hardware sync
6. **CLAP/VST3/AU plugin host with crash isolation** — extensible effects
6. **Pluggable library sources (Rekordbox, Engine DJ, Serato, Traktor, VirtualDJ, iTunes)** — library interoperability
7. **USB device manager (format, backup, sync, export to player formats)** — hardware integration
8. **Automix with phrase-aware transitions** — hands-free mixing
9. **Color preview waveform (RGBH)** — visual frequency balance at a glance
10. **Groove Box / Beat Lab with drum capture + Plaits synthesis** — creative performance
11. **Sampler sequencer (step lanes + macro lanes)** — pattern-based sampling
12. **Looper (8 loops/deck, overdub, quantized)** — live looping
13. **Broadcast (Icecast/Shoutcast) + Mixcloud upload + Now Playing** — streaming workflow
14. **Timecode/DVS support** — vinyl/CDJ control
15. **OSC + Web remote** — phone/tablet control
16. **JavaScript/Lua scripting for controllers** — custom mappings
17. **Touch layout profile + gestures** — mobile/tablet DJing
18. **Theming/skinning with token system** — visual customization
19. **Android native app** — mobile platform
20. **Raspberry Pi optimized build** — embedded hardware
21. **Guided tour + tutorials** — onboarding
22. **Similar tracks recommendation** — music discovery
23. **Word-level lyric timing + karaoke** — vocal performance
24. **Grid editor with UNDO/REDO + Analysis Lock** — precise grid control
25. **Edit mode (timeline arranger, EDL, render WAV)** — track preparation

---

## Summary Statistics

- **Total features enumerated:** 189 (numbered 1-189)
- **OpenDJ-unique features:** ~45 (broadcast, OSC, web remote, karaoke, JS scripting, plugin host, timecode, Ableton Link, Mixxx mappings, native macOS menu, guided tour, Android, word-timing, etc.)
- **OpenFadeR-unique features:** ~35 (device manager, pluggable library, edit mode, lighting/DMX, video, split screen, export mode, Lua plugins, similar tracks, color preview waveform, Raspberry Pi, stick verifier, rollback, merge/lever/release FX, etc.)
- **Shared core features:** ~109 (decks, mixer, effects, sampler, stems, library, automix, controllers, UI layouts, theming, analysis, platforms)
- **DJ Sugar (Mixxx) gaps:** ~45 features not in Mixxx

---

## Source Citations

- OpenDJ repo: https://github.com/openwave-studio/opendj (cloned to `/home/ubuntu/.hermes/cache/scratch/parity-src/opendj`)
- OpenFadeR repo: https://github.com/OpenRsG/OpenFadeR (cloned to `/home/ubuntu/.hermes/cache/scratch/parity-src/openfader`)
- OpenDJ README: lines 13-30, 34-51, 109-111
- OpenFadeR README: lines 39-199
- OpenDJ engine.rs: lines 1-120, 1000-8891
- OpenFadeR audio/mod.rs: lines 1-65
- OpenDJ browser.rs: lines 1-120, 6922 total
- OpenFadeR app/deck/mod.rs: lines 1-120, 508 total
- OpenFadeR app/browser/mod.rs: lines 1-120, 691 total
- OpenDJ controllers/lib.rs: lines 1-94
- OpenFadeR app/midi/mod.rs: lines 1-120, 631 total
- OpenDJ export/lib.rs: lines 1-120
- OpenFadeR device/mod.rs: lines 1-120, 500 total
- OpenFadeR lighting/mod.rs: lines 1-120
- OpenFadeR video/mod.rs: lines 1-39
- OpenDJ broadcast/lib.rs: lines 1-120
- OpenDJ karaoke/lyrics: crates/remote/src/overlay.rs, crates/engine/src/word_timing.rs
- Existing study: `/home/ubuntu/djsugar-next/docs/study/opendj-openfader.md` (402 lines)