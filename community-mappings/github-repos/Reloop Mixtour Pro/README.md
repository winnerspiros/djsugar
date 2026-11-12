```
 ███  ████  █████ █   █ █████ █   █ █   █
█   █ █   █ █   █ ██  █ █     █  █   █ █
█████ ████  █   █ █ █ █ █████ ███    █
█   █ █  █  █   █ █  ██     █ █  █    █
█   █ █   █ █████ █   █ █████ █   █   █
```

# Reloop Mixtour Pro — Mixxx Controller Mapping

A full **4-deck** Mixxx mapping for the **Reloop Mixtour Pro**, built and verified on real
hardware. Covers the controller's entire labeled surface with djay-quality LED feedback —
and it's **self-sufficient** (it switches the controller into its advanced mode on its own,
no other DJ app required).

> Mapping by **Aronsky** · MIT-spirited, share freely.

---

## Features

- **4 decks** — flip each side between decks 1/3 (left) and 2/4 (right); everything follows
  with blue (1/2) / red (3/4) LED coloring.
- **Transport** — play, cue, sync, and a 3-press In-Out-Ex loop, all with blinking/coloured LEDs.
- **8 performance pads** with hardware mode-select + on-pad picker: Hot Cue, Auto Loop,
  Bounce Loop, Sampler, Instant FX *(Pitch Cue / Saved Loops / Neural Mix pending)*.
- **FX paddles** → Effect Unit 1 (momentary or latched) + **backspin** on shift.
- **Filter**, full 3-band EQ, gain, faders, crossfader, headphone, browse.
- **VU meters**, hot-cue colours, and quiet MIDI output (LEDs only transmit on change).

---

## Installation

1. Copy both files into your Mixxx controllers folder:

   ```
   Reloop-Mixtour-Pro.midi.xml
   Reloop-Mixtour-Pro.scripts.js
   ```

   - **macOS (App Store / sandboxed):**
     `~/Library/Containers/org.mixxx.mixxx/Data/Library/Application Support/Mixxx/controllers/`
   - **macOS (regular):** `~/Library/Application Support/Mixxx/controllers/`
   - **Windows:** `%LOCALAPPDATA%\Mixxx\controllers\`
   - **Linux:** `~/.mixxx/controllers/`

2. Mixxx → **Preferences → Controllers → Reloop Mixtour Pro** → enable, pick this preset.
3. **Preferences → Decks → Decks = 4** (or use a 4-deck skin).
4. **Preferences → Effects → Quick Effect chain presets** — *this list's order matters:*
   - **Preset #1** is the deck's **default filter** (what the filter knobs control). Put
     your filter here, e.g. **Moog Filter** or the standard **Filter** (LPF/HPF).
   - **Presets #2–#9** become the **8 Instant-FX pads**, in order (pad 1 = #2 … pad 8 = #9).
     Drop in whatever effects you want — Echo, Reverb, Flanger, BitCrusher, etc.

   The mapping resets every deck to preset #1 on load, so rearranging this list is how you
   choose both your default filter and your instant effects — no remapping needed.

> Tip: editing the **`.js`** hot-reloads in Mixxx; editing the **`.xml`** needs a preset
> re-apply or a Mixxx restart.

---

## Control reference

### Transport (per deck) — modifier matrix

| Button | Plain | + Shift | + Mode | + Shift + Mode |
|--------|-------|---------|--------|----------------|
| **Play** | play / pause | pitch bend up | key up | tempo **+0.1 BPM** |
| **Cue** | set / jump to cue | pitch bend down | key down | tempo **−0.1 BPM** |
| **Sync** | beat sync | key match (harmonic) | loop **double** | loop **out / move** |
| **In-Out-Ex** | loop in → out → exit | auto loop | loop **halve** | loop **in / move** |

### Performance pads

- **Pick a mode:** hold a **Mode** button and tap the pad with the printed function.
- **Split** button: 4 pads → left deck, 4 → right deck. Otherwise all 8 act on the active deck.

| Mode | Pads do… | Shift + pad |
|------|----------|-------------|
| **Hot Cue** | set / jump (lit in the cue's colour) | clear the cue |
| **Auto Loop** | beat loop — pads 1→8 = ¼,½,1,2,4,8,16,32 | — |
| **Bounce Loop** | beatjump: left col **back** (orange), right col **fwd** (green), 4/8/16/32 | invert direction |
| **Sampler** | load / play sample (pink, bright while playing) | stop → eject |
| **Instant FX** | hold a pad = Quick Effect preset #2–#9 while held (configure the list order — see setup) | — |
| **Pitch Cue** | pad 1 resets key; pads 2–8 set +1…+7 semitones | jump to hot cue 1–8 |
| **Saved Loops** | save current loop to a slot / recall a saved loop | clear the slot |
| *Neural Mix* | *not yet implemented* | |

### FX section

| Control | Function |
|---------|----------|
| **FX paddle** (push) | engage Effect Unit 1 on that side's deck (down = momentary, up = latch) |
| **Shift + paddle** | backspin |
| **Dry/Wet knob** | effect mix (both decks) |
| **FX param ◀ / ▶** | effect parameter down / up |
| **Shift + param ◀ / ▶** | select previous / next effect |

### Mixer & browse

| Control | Function |
|---------|----------|
| Channel faders / crossfader | volume / blend |
| 3-band EQ · Filter · Gain | per channel |
| Master / headphone vol / cue mix | master section |
| Browse encoder | scroll library · press = load/open |
| Shift + Browse press | back one step |
| Shift + Load (L / R) | library section back / forward |
| Mode + Load | switch the side between decks 1↔3 / 2↔4 |
| Mode + Browse turn | beat-snapped seek (4 beats per click) |

---

## How it works (for the curious)

- **Channel = deck.** The controller re-transmits a side's controls on the deck's channel
  when you flip 1↔3 / 2↔4, so the mapping just addresses all four decks directly.
- **Modifiers are encoded by the hardware** into distinct MIDI notes (shift/mode change the
  note sent), so the script maps notes rather than tracking modifier state.
- **Advanced ("Serato") mode** is enabled by two SysEx messages the script sends in `init()`
  — without them the mode macro, pad picker and combo notes are inert after a power-cycle.
- The script **owns every LED** and caches them so nothing is sent while idle.

Full design notes and the reverse-engineered protocol live in
[`docs/superpowers/specs/`](docs/superpowers/specs/).

---

## License

[MIT](LICENSE) © Aron Birkir (Aronsky). Share freely — pull requests and other
Mixtour Pro tweaks welcome.

*Built for the love of mixing. 🎧*
