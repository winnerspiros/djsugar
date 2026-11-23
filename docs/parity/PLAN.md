# Parity plan

The goal: the Rust app works like DJ Sugar (the C++ fork in ~/djsugar), function by
function, not only on screen.

1. Base: the engine and its crates from OpenDJ (~/djsugar-rust), under crates/port.
   C code is replaced with Rust (whisper.cpp gone; SQLite to be replaced).
2. OpenFadeR (github.com/OpenRsG/OpenFadeR): its library importers from other DJ
   apps, and any effect or DSP code closer to DJ Sugar's behaviour than OpenDJ's.
3. For every feature, compare DJ Sugar's code with OpenDJ and OpenFadeR, take the
   closest, and make it behave exactly as DJ Sugar's does.
4. Keep our renderer and the Classic screen; wire them to the ported engine.
5. Order: headphones and continuous sync, YouTube, tags/cover art/library, effect
   racks, menus and settings, controllers, waveforms, stems, Spotify.
6. Screenshots against DJ Sugar are a final check only, not the work itself.

Gap lists: code-engine.md, code-streaming-library.md, code-fx-ui.md, code-helpers.md.
