DJ Sugar 0.6.0 turns DJ Sugar into an instrument as much as a DJ app. Plugin instruments and the Groove Box get a deck of their own with scale, chord and arpeggiator, a looper layers them over the mix, and live decks draw what they play on the waveforms. Player mode becomes a full music player with Home, artist and album pages, Now Playing, Keep going and scrobbling. Lyrics arrive everywhere: a Lyrics tab, on the waveforms, a karaoke screen, and Whisper to time or write down the words.

## What's new

**Instruments**

- **Jam** (the library rail's source for instruments, formerly Instruments) browses them like Collection: a tree with Recently played, Type and Character tag chips, presets and column sort.
- **Presets**: save, step through and load presets for plugins and the Groove Box, and list a plugin's own presets (VST3 `.vstpreset`, Audio Unit factory presets, CLAP presets).
- **Instrument deck**: eight knobs to a page that follow the plugin's own window (right-click a knob to pick the parameter it turns), a scope, and a keyboard lighting the notes played.
- **Scale** (Off, Auto from the master deck's key, or a picked scale), **chord** (Triad, Seventh, Power or a learned chord) and an **arpeggiator** synced to the mix's beat.
- **MIDI from**: pick the keyboard and channel that play an instrument. DJ controllers no longer play instruments unless you choose **Play instruments**, and each device has a Deck menu.
- **Live decks** (instruments, inputs, Groove Box): TRIM, PAN, a safety limiter, auto trim, and Thru while live. 4-Deck gets a live strip for them.
- **Instrument art**: a plugin's NKS artwork or bundle icon, or an image of your own.
- Plugins keep their sound across restarts and when quitting, run side by side, and sleep when idle.

**Looper**

- A **Looper** on a deck: four layers, record and overdub on the beat from the bar, and **CAPTURE** the last bars of the mix.

**Live decks on the waveforms**

- Live decks draw what they play on the scrolling waveform: a note roll for instruments, step lanes with the hits to come for the Groove Box, and a macro lanes view.
- **Phrase flags** on the scrolling waveforms, and the leading deck's phrases on live decks.

**Groove Box**

- Its own tempo (or follow the mix, half or double), kick in key, eight tracks, Dice and living grooves.
- **Auto fills**, a held **FILL**, the **REMIX** strip and a 4x4 view. Maschine Mikro MK3 pads follow their printed numbers.

**Player mode**

- **Home** with recently played and added albums, your crates and playlists, and Genre and Decade mixes, with covers from their tracks.
- **Artist and album pages** with Play, Shuffle and Mix.
- **Now Playing** view with large lyrics, and a **Favorites** heart.
- **Up Next** shows the track playing, and each track can have its own mix length and style.
- **Keep going** tops up Up Next with tracks that mix, and **Start Radio** from a track, artist or list.
- A bar along the bottom with a quality tag, a square frameless mini player, ＋ Add to crate, and a **sleep timer**.
- **Media keys**, headphone buttons and the system's Now Playing on macOS, Windows and Linux.
- **Scrobbling** to ListenBrainz and Last.fm (Settings › Broadcast, off at first; Last.fm uses your own API key).

**Lyrics and karaoke**

- **Lyrics tab** reading `.lrc` files and lyric tags, and **Find lyrics** on LRCLIB.
- **Lyrics on the waveforms**: each deck's timed lyrics along its big waveform.
- **Karaoke screen** on a second display with word fill, count-in and a Vocals slider, also as a `/karaoke` web page for a TV or another device.
- **Time Words**: Whisper gives each word its time, and **Transcribe** writes down the words of a track with no lyrics.

**Layouts and browsing**

- **Prep layout** for getting In Prep tracks ready on Deck 1: drag the grid, name hot cues and memory cues in a Cues tab, tick your own checklist, and review quickly with the keyboard (Enter marks a track Play Ready and loads the next). It's off at first: add it in Settings › Layouts.
- **Choose your layouts**: Settings › Layouts (or the cog in the layout menu) lists the layouts the menu offers, with previews. Reorder them, remove the ones you don't use, and add others from More layouts. A controller's VIEW button steps through them.
- A lighter **2-Deck** layout with the mixer between the pads, a **JOG** button and a simpler tempo; the previous one is now **2-Deck Pro**, always with pads. A **PADS** button shows or hides the pad grid in 2-Deck and 4-Deck.
- **Filter bar**: chips narrow each other's counts, drag to reorder, a Key chip that matches by Setting, Exact, Harmonic or Fuzzy, a + menu with core fields, your tags and more, and a finder in the search bar.
- **Wanted** sorts by column; the Buy sheet shows prices, a store picker and iTunes Store links on macOS.
- Add-on downloads show their queue place and speed, can be previewed while they download, and Downloads is a place in the side panel.
- Funkwhale works as a Subsonic source.

**Fixes**

- Decks stay usable when the audio device or a controller goes away (Settings › Controllers › Reconnect controllers).
- The UI stays smooth during batch analysis.
- macOS: ⌘, opens Settings, File, Edit, View and Settings… stay in every menu bar, the plugin helper stays out of the app switcher, the updater updates the real app when macOS runs it translocated, and the Mixcloud token is read from the keychain only when uploading.
- M3U8 and rekordbox XML export work on macOS again. Exports live in File › Export.
- Long names in the browser header no longer run under the search box.

**Moved home**

- DJ Sugar now lives at github.com/winnerspiros/djsugar. Old links redirect. On macOS the app has a new bundle ID, so macOS asks once more for microphone, keychain and folder access (Desktop, Documents, removable drives, other apps' data for imports) after this update. System Settings › Privacy & Security › Files and Folders lists DJ Sugar twice until the old copy is gone.

## Install

**macOS** (Apple Silicon and Intel, macOS 11 or newer): if you have 0.3.0 or newer, DJ Sugar offers this update itself when it starts. Otherwise download `DJSugar-…-macos.zip`, unzip it and move DJ Sugar to Applications.

**Windows** (preview; 64-bit Windows 10 and 11):

- `DJSugar-…-windows-x64-setup.exe`: installs for your user, no administrator needed, and updates itself from now on.
- `djsugar-…-windows-x64.zip`: portable. Unzip anywhere and run `DJSugar.exe`.

The Windows build isn't signed yet, so on first run SmartScreen says "Windows protected your PC": click More info, then Run anyway. ASIO drivers show next to Windows Audio devices in Settings › Audio. Windows on ARM runs it through emulation.

**Linux** (x86_64, glibc 2.35 or newer, e.g. Ubuntu 22.04+, Fedora 36+, Debian 12+):

- `djsugar_…_amd64.deb` (Ubuntu, Debian, Mint, Pop!_OS) or `djsugar-…x86_64.rpm` (Fedora, openSUSE): open it to install with your software centre, or run `sudo apt install ./djsugar_…_amd64.deb` / `sudo dnf install ./djsugar-…x86_64.rpm`. DJ Sugar then shows up with your other apps, Traktor Kontrol controllers work without extra setup, and when a new version is out DJ Sugar offers the new package.
- `DJSugar-…-x86_64.AppImage`: make it executable (`chmod +x`) and run it. AppImages from 0.3.0 on update themselves.
- `djsugar-…-linux-x86_64.tar.gz`: unpack anywhere and run `bin/djsugar`.

DJ Sugar needs ALSA and a Vulkan driver, which most desktops already have.

Add your music folders in Settings › Collection, or let the guided tour do it the first time DJ Sugar starts. MIDI controllers that Mixxx supports load their mapping when plugged in.

## Known limits

- Windows is a preview: unsigned, and less tested than macOS and Linux.
- Streaming services are browse only. Buying from stores opens the store's page; DJ Sugar doesn't check out for you.
- Automix uses decks 1 and 2 only.
- Controllers marked Beta, including the Xone K2 and Maschine Mikro MK3 mappings, the Traktor Kontrol controllers and the XDJ-RX3 and Numark V7 mappings, haven't all been tried on the hardware yet.
- Plugins: their latency isn't compensated. On Linux, plugin windows need X11 (XWayland on Wayland).
- The phone remote works on your local network only.
- Broadcasting: Icecast and Shoutcast only for now. Twitch, YouTube and Mixcloud Live streaming come later.
- Some Settings descriptions are still cut short with "…".
- USB export for Pioneer players is experimental.
