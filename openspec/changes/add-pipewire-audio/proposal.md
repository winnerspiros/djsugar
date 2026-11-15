# Native PipeWire audio on Linux

Planned for v0.8. Issue #402.

## Why

On Linux the engine reaches sound cards through cpal's ALSA host only. On PipeWire distros (Fedora, Ubuntu 22.10+, Debian 12, Mint 22, Arch, SteamOS) that plays through `pipewire-alsa`, but only the system default output is shared. A specific output, such as a controller's card or headphones that aren't the default, can't be picked. Raw `hw:` cards bypass PipeWire and either fail as busy or lock other apps out. A second device (headphones, booth) relies on DJ Sugar's own drop-or-pad drift handling.

## What changes

- cpal's `pipewire` host on Linux (cpal 0.18, already a dependency). It is used when a PipeWire server is running, ALSA otherwise.
- Settings › Audio lists PipeWire outputs and inputs by name with their real channel counts; the status line says PipeWire; raw ALSA cards stay available under an "ALSA: " prefix, as ASIO is listed on Windows.
- Real-time priority through rtkit (`realtime-dbus`).
- All DJ Sugar streams share one PipeWire node group, so one driver clocks every device; drop-or-pad drift handling is skipped when every route is on PipeWire.

## Packaging

The `pipewire` feature links `libpipewire-0.3.so.0`, calls `pw_stream_get_time_n` (PipeWire 0.3.50+) and needs 0.3.53+ headers. Release builds run on Ubuntu 22.04 (0.3.48). So the release workflow builds the PipeWire client library from source and ships it, with its modules and client config, inside the AppImage, tarball, .deb and .rpm. The Flatpak gets `--filesystem=xdg-run/pipewire-0`. A CI smoke test runs against PipeWire 0.3.48 on Ubuntu 22.04.

## Not planned

A separate JACK host: PipeWire already serves JACK clients.

## Later

With add-deck-outputs: one unconnected multichannel output with a channel pair per deck, to wire into a DAW with qpwgraph or Helvum.
