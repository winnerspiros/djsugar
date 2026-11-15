# Proposal

Planned for: **v0.7 · Your library, everywhere** (from a user request, 2026-11-10; issue #570)

## Why

Every deck draws one waveform look: RGB, where one shape is red, amber or blue by whichever band is loudest. DJs coming from rekordbox read its 3Band waveform (bass, mids and highs stacked in their own colours) and its half overview at a glance, and VirtualDJ users are used to seeing where the vocals are. A stacked-bands look already exists in the shader, but only behind a developer switch.

## What Changes

- Settings › Decks › **Waveform color**, a dropdown: **RGB** (default, as today), **3-Band** (bass, mids and highs stacked, bass widest at the back), **Blue** (one blue shape, lighter where the highs are loud), and **Stems** (each stem in its pad color, stacked, when the track has stems; the RGB look otherwise).
- Settings › Decks › **Palette**, shown for 3-Band: **Blue-Orange-White** (as on rekordbox) or **Lilac-Cyan-White** (DJ Sugar's accent for the bass, cyan mids, white highs).
- Settings › Decks › **Overview**: **Full** or **Half**. Half draws the overview under each deck from its bottom edge up, as rekordbox does; the big scrolling waveform stays whole.
- Settings › Decks › **Show vocals**: tints the parts of a track where someone sings green on the RGB, 3-Band and Blue looks. A light vocal-detection pass at analysis gives every track a vocal level, so it works without separating stems; tracks with stems use the vocal stem.
- Maybe: **Size follows gain**, off at first: the deck's gain knob makes its waveforms taller or shorter, as on VirtualDJ. Kept only if it feels right on a controller.
- One choice for every layout (2-Deck, 4-Deck, Prep). The browser's mini waveforms and the Player's scrubber keep their looks. Loops, bar lines, cues, phrases and lyrics keep their colors on every look.
- No drawing-rate option and no color pickers.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `waveforms`: color schemes, palettes, half overview, vocals

## Out of Scope

- Custom colors per band.
- Half shape on the big scrolling waveform.

## Difficulty

**Medium: four PRs.** The looks, palettes and half overview are shader parameters (one PR); the vocal pass adds a per-slice level to the analysis and the waveform texture (one PR); stems need per-stem levels on the GPU (one PR); gain is one uniform already in the shader (small).

## Performance

The looks change only uniforms; nothing is uploaded again when a setting changes. The vocal level is worked out once at analysis, at a fraction of the cost of key or beat detection, and stored with the waveform.
