# Proposal

Planned for: **v0.7 · Your library, everywhere** (from a user request, issue #730, 2026-11-14; moved from v0.8 at a user request)

## Why

The waveforms' look is split: the height is in the waveforms button's drop-down, as a row of ten numbers, while the color scheme, palette and Full/Half sit deep in Settings › Appearance, where trying one means opening Settings and closing it again to see the decks. The large waveforms can't be half, and the play line is always in the middle, so only half the strip shows what is coming.

## What Changes

- The waveforms button's drop-down gets the look, in this order: **Waveform height**, **Type**, **Color** (and 3-Band's palette under it), **Needle**, **Vocals**, **Phrases**. It stays open after a pick, so choices can be tried out.
- **Waveform height** becomes a slider: the waveforms follow it while it is dragged and settle on the nearest of the ten steps when it is let go, with ticks at the steps (3, the default, taller) and the step's number beside it. Arrow keys move a step.
- **Type**: Full (the default) or Half, now for the large scrolling waveforms as well as the overviews. A live deck showing lanes of notes, steps or knobs stays Full.
- **Color**: RGB (the default), 3-Band or Stems. The Blue scheme is dropped; a saved Blue comes back as RGB. 3-Band's Blue-Orange / Lilac-Cyan row shows under it.
- **Needle**: Center (the default) or Left, the large waveforms' play line a quarter of the way in so three quarters of the strip shows what is coming. The beat grid, cues, loops, lyrics, knob labels and live decks' lanes follow it.
- **Vocals** (Off, Bar, Lyrics) picks one cyan lane along the bottom of the large waveforms: a bar where someone sings (a vocal pass, or the vocal stem when a track has HQ stems) or the timed lyrics, never both. It replaces the Show vocals and Show lyrics switches and the strip along the top. **Phrases** (Off, Overview, Waveform, Both) replaces the right-click Show/Hide phrases on the overviews and the large waveforms.
- Waveform color, Palette, Overview, Show vocals and Lyrics on waveforms leave Settings › Appearance.
- **Size follows gain** stays in Settings › Appearance, now on by default, and sizes only the large waveforms: the overviews keep their height.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `waveforms`: the look picked from the waveforms button, Half on the large waveforms, a needle position

## Out of Scope

- Other needle positions, or a draggable needle.
- Moving Size follows gain into the drop-down.
- How vocals are detected and drawn (a separate change).

## Difficulty

**Small: one PR.** The shader already has Half for overviews and takes the play line's position from one place; the drop-down reuses the choices that Settings had.

## Performance

Nothing new per frame: one more number in the waveform shader's parameters.
