# Install

Download DJ Sugar from the [download page](https://github.com/winnerspiros/djsugar#download) or the
[latest release on GitHub](https://github.com/winnerspiros/djsugar/releases/latest).

## macOS

Apple Silicon and Intel, macOS 11 or newer.

1. Download `DJSugar-…-macos.zip`.
2. Unzip it and move **DJ Sugar** to Applications.

The app is signed and notarized by Apple, so it opens like any other app, and
it offers new versions itself when it starts.

## Linux

x86_64, glibc 2.35 or newer (Ubuntu 22.04+, Fedora 36+, Debian 12+). DJ Sugar
needs ALSA and a Vulkan driver, which most desktops already have.

- **AppImage:** download `DJSugar-…-x86_64.AppImage`, make it executable with
  `chmod +x`, and run it.
- **Tarball:** download `djsugar-…-linux-x86_64.tar.gz`, unpack it anywhere and
  run `bin/djsugar`.

## Windows

Windows is a preview from DJ Sugar 0.4: 64-bit Windows 10 or 11.

- **Setup:** download `DJSugar-…-windows-x64-setup.exe` and run it. It installs
  DJ Sugar for your user only (no administrator needed), adds it to the Start
  menu, and DJ Sugar updates itself from then on.
- **Portable:** download `djsugar-…-windows-x64.zip`, unzip it anywhere and run
  `DJSugar.exe`. It doesn't update itself.

The builds aren't signed yet, so on the first start Windows shows "Windows
protected your PC". Click **More info**, then **Run anyway**.

In Settings → Audio, pick an **ASIO** driver if your controller or audio
interface has one, for the lowest latency. Windows lets only one app use a
MIDI controller at a time, so quit any other DJ app before you start DJ Sugar.

## First start: the guided tour

The first time DJ Sugar opens, a short guided tour sets it up with you. Every step
can be skipped.

1. **Sound:** pick where the master (your speakers) and the headphones play,
   and play a test tone in each. A controller with its own sound card, such as
   the DDJ-FLX4, is its own device: pick it for both, on channels 1 & 2 and
   3 & 4.
2. **Controller** (only when one is plugged in): DJ Sugar shows which mapping it
   runs. Press a button or move a knob and the tour says it heard it.
3. **Your music:** add your music folders. If DJ Sugar finds another DJ app's
   library, such as VirtualDJ's, it offers to bring it in.
4. **Your first mix:** a hands-on lesson on two demo tracks. It switches to
   the 2-Deck layout, rings each control in turn and waits for you to use it,
   on screen or on your controller: load, play and cue, listen in the
   headphones, sync, bring the second track in with its fader and EQ, then a
   bass swap.

**Help › Take the Tour** runs the tour again, and **Help › Tour Part** one part of it. If you used DJ Sugar
before the tour existed, it's offered once in a banner under the top bar.

## Tutorials

**Help › Tutorials** has short hands-on lessons that work the
same way, a few minutes each: **Looping**, **Using FX**, **Filtering
tracks**, **Automix** and, with one plugged in, the **DDJ-FLX4**. Each rings
the real controls and waits for you; the ✕ on the tip leaves at any step and
puts the layout back. A ✓ marks the ones you've finished, and the last tip
links to that part of this guide.
