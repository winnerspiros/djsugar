# Xone K3 — Mixxx mapping

Custom Mixxx mapping for the Allen & Heath Xone K3.

One runtime, two layer trees, five presets. Four playback presets
share one layer tree and differ only in boot config; colorLab is
a separate editor preset.

`midiChannel` in each preset's boot config controls **input
routing** (MIDI note-on/note-off) and the K2 fallback output
(`sysex: false`). It does **not** control SysEx LED output —
`midi.sendSysexMsg` routes to the MIDI port bound to each
preset's `<controller id="...">` in the `.midi.xml`, regardless
of the SysEx frame's byte-7 value (the `0x0E` there is a fixed
command-header byte, shared by the RGB write and release
commands and invariant across LED indices, not a channel).

Consequence: two presets on the same `midiChannel` share input
bindings. If Cat hardware sends on `0x0E` and both Cat and
oneK3 listen on `0x0E`, both presets respond to Cat's hardware;
oneK3's port receives LED state driven by Cat's buttons. Cat,
oneK3, oneK3 miniFX, and colorLab all ship on `0x0E` — change
one before running them together on separate physical units.
Kitten ships on `0x0D` and pairs with Cat without conflict.

| Preset | Channel | Boot mode | Layer tree | Use case |
|---|---|---|---|---|
| **Cat** | 0x0E | focus | CAT_LAYERS | Main preset. Decks 1/3. Re-pointable single-deck top half. |
| **Kitten** | 0x0D | focus | CAT_LAYERS | Second unit. Decks 2/4. No overlap with Cat. |
| **oneK3** | 0x0E | twoDeck | CAT_LAYERS | Single K3, both decks visible. |
| **oneK3 miniFX** | 0x0E | twoDeckMini | CAT_LAYERS | Both decks, one focused FX unit in the middle. |
| **colorLab** | 0x0E | n/a | lab | No playback. LED colour ramp editor. |

---

## Install

1. Copy the `xone-k3-*.js` files and the `*.midi.xml` files into
   `~/.mixxx/controllers/`.
2. Preferences → Controllers → pick the K3 → Preset.
3. Pick a preset.
4. After editing any file: untick / re-tick the preset checkbox.

---

## Focus: active deck and active FX

Two independent axes decide what the top half drives at any
moment:

- **`activeDeck`** — `'A'` or `'B'`. Which of two deck slots is live.
- **`activeFx`** — `'A'` or `'B'`. Which of two FX slots is live.

Each slot holds a *pinned* assignment (`selectedDeckA/B`,
`selectedFxA/B`). Switching activeDeck or activeFx changes which
slot's channel is targeted by focus-following controls. The
pinned assignments themselves don't change. The `selectAB` chord
overlay reassigns what each slot points at.

- Press `press0` / `press3` → activeDeck = A / B.
- Press `press1` / `press2` → activeFx = A / B.
- Slip button (`grid2_1`) tap → toggle slip.
- Slip button + shift → flip activeDeck.
- `inspect()` (console) shows active deck, FX, and current
  selection.

Controls that follow focus use `$DECK_ACTIVE$` / `$FX_ACTIVE$` in
their group string. When focus changes, those controls silently
retarget to the newly-active slot's channel — EQ knobs, play/cue,
slip LED, QFX buttons, and so on.

---

## Where to go

| I want to… | Read |
|---|---|
| Understand how the system works | `CONCEPTS.md` |
| See the physical layout and what each control does | this file |
| Look up an FX cell by unit and mode | this file, §FX cell layouts |
| Change a colour, rebind a button, add a mode | `CUSTOMISE.md` |
| Understand the runtime, tune internals | `INTERNALS.md` |
| Tune performance, see caching strategy | `PERFORMANCE.md` |
| Build a colour ramp on the hardware | `LAB.md` |
| Look up a term | `GLOSSARY.md` |
| Ask an AI to edit for me | `AI-HELPER.md` |

If you only read one thing, read `CONCEPTS.md`. Every other doc
assumes it.

---

## Physical layout and behaviour

```
                    ┌───────────┬───────────┬───────────┬───────────┐
  TOP               │ encoder0  │ encoder1  │ encoder2  │ encoder3  │
                    │ press0    │ press1    │ press2    │ press3    │
                    ├───────────┼───────────┼───────────┼───────────┤
                    │ knob0_0   │ knob1_0   │ knob2_0   │ knob3_0   │  row 0
                    │ button0_0 │ button1_0 │ button2_0 │ button3_0 │
                    │ knob0_1   │ knob1_1   │ knob2_1   │ knob3_1   │  row 1
                    │ button0_1 │ button1_1 │ button2_1 │ button3_1 │
                    │ knob0_2   │ knob1_2   │ knob2_2   │ knob3_2   │  row 2
                    │ button0_2 │ button1_2 │ button2_2 │ button3_2 │
                    ├───────────┼───────────┼───────────┼───────────┤
  FADERS            │ fader0    │ fader1    │ fader2    │ fader3    │
                    ├───────────┼───────────┼───────────┼───────────┤
  GRID              │ grid0_0   │ grid1_0   │ grid2_0   │ grid3_0   │  row 0
                    │ grid0_1   │ grid1_1   │ grid2_1   │ grid3_1   │  row 1
                    │ grid0_2   │ grid1_2   │ grid2_2   │ grid3_2   │  row 2
                    │ grid0_3   │ grid1_3   │ grid2_3   │ grid3_3   │  row 3
                    ├───────────┼───────────┼───────────┼───────────┤
  BOTTOM            │ layer     │ encoderL  │ encoderR  │ shift     │
                    │           │ pressL    │ pressR    │           │
                    └───────────┴───────────┴───────────┴───────────┘
```

`encoderL`/`encoderR` and `pressL`/`pressR` are separate from
`encoder0`–`encoder3` and `press0`–`press3` — the L/R pair sit at
the bottom, beside layer and shift.

```
════════════════════════════════════════════════════════════
TOP — encoder0-3 / press0-3
════════════════════════════════════════════════════════════

The encoder + press row is identical across every mode. Only
the middle FX cells differ. Encoders target pinned decks A / B
(not the active deck). Press row selects active deck and active
FX slot.

QFX cycle differs between families:
  focus (Cat / Kitten):  enc1 cycles the active deck's QFX chain,
                         enc2 is off.
  oneK3 / mini:          enc1 cycles deck A's chain,
                         enc2 cycles deck B's chain.

focus (Cat / Kitten)
enc0: pregain A              +shift: keySync  A
enc1: QFX cycle active deck  +shift: keyReset A
enc2: —                      +shift: keyReset B
enc3: pregain B              +shift: keySync  B

oneK3 / oneK3 miniFX
enc0: pregain A              +shift: keySync  A
enc1: QFX cycle A            +shift: keyReset A
enc2: QFX cycle B            +shift: keyReset B
enc3: pregain B              +shift: keySync  B

press row (identical in every mode)
press0: active deck ← A      +shift: sync A      LED: theme deckA, bright=active
press1: active fx ← A        +shift: keylock A   LED: theme fxA,   bright=active
press2: active fx ← B        +shift: keylock B   LED: theme fxB,   bright=active
press3: active deck ← B      +shift: sync B      LED: theme deckB, bright=active

Middle cells per mode
focus:        fxLayout<n>       one focused FX unit (unit 1-4)
twoDeck:      fxLayoutA<n>      FX-A column, left  (pinned)
              fxLayoutB<n>      FX-B column, right (pinned)
twoDeckMini:  fxMini<n>         one focused FX unit (same shape as focus)

Low-EQ route cells (button0_2 / button3_2)
focus:        button0_2         active FX → active deck  (LED follows selectedFxCurrent)
twoDeck/mini: button0_2         FX-A → deck A        (LED follows selectedFxA)
              button3_2         FX-B → deck B        (LED follows selectedFxB)

In focus mode, button3_2 is FX3 enable (from the layout patch);
only twoDeck / mini have button3_2 as a route.

QFX enable cells (button1_2 / button2_2)
focus:        button1_2         QFX-A enable + chain LED
              button2_2         display-only (echoes knob2_2)
twoDeck/mini: button1_2         QFX-A enable + chain LED
              button2_2         QFX-B enable + chain LED
```

```
════════════════════════════════════════════════════════════
FADERS
════════════════════════════════════════════════════════════

FOCUS  (Cat / Kitten)
fader0: volume act.deck
fader1: FX-act mix
fader2: —
fader3: —

twoDeck  (oneK3)
fader0: deck A volume
fader1: FX-A mix
fader2: FX-B mix
fader3: deck B volume

twoDeckMini  (oneK3 miniFX)
inherits twoDeck — faders do NOT follow focus. fader1/fader2 always mix the
pinned A/B units, never $FX_ACTIVE_N$.

Shift / chord
No shift companion, no chord override. Faders are identical in every state.
```

```
════════════════════════════════════════════════════════════
GRID — rows 0-1
════════════════════════════════════════════════════════════

grid0_0:  jump −1 beat             +shift: inherit       LED: ramp fader0
grid1_0:  jump +1 beat             +shift: inherit       LED: ramp fader1
grid2_0:  cycle timeline           +shift: LED only      LED: ramp fader2 · bright=track, dim=loop · shift=dim
grid3_0:  CUE: cue                 +shift: clear cue     LED: ramp fader3, tracks cue_indicator
          LOOP/ODD: —              +shift: —             LED: ramp fader3, always lit

grid0_1:  CUE: jump −16            +shift: inherit       LED: purple [69,0,127]
          LOOP/ODD: jump −size     +shift: inherit       LED: magenta
grid1_1:  CUE: jump +16            +shift: inherit       LED: purple
          LOOP/ODD: jump +size     +shift: inherit       LED: magenta
grid2_1:  toggle slip              +shift: flip deck     LED: slipScheme · theme of active-deck ch
          pressR held: load onto active deck
grid3_1:  toggle play              +shift: LED only      LED: green=track, yellow=loop
```

```
════════════════════════════════════════════════════════════
GRID — rows 2-3
════════════════════════════════════════════════════════════

                         row 2                row 3
                     g0_2 g1_2 g2_2 g3_2   g0_3 g1_3 g2_3 g3_3
CUE     (hotcues)      1    2    3    4      5    6    7    8
  +shift: clear 1-8    —    —    —    —      —    —    —    —

LOOP    (loop sizes)   8    4    2    1     1/16 1/8  1/4  1/2
  +shift: beatjump
  size (same cell map) 8    4    2    1     1/16 1/8  1/4  1/2

LEDs
  CUE hotcues         per-hotcue colour read from Mixxx
  LOOP sizes          green · teal2 (grid3_2 / grid3_3)
  LOOP shift sizes    teal2 · green (grid3_2 / grid3_3)

Layer overlays (chord + slip, per layer, toggle again to exit):

  hotcue2 (cue layer)
    row 2: hotcues 9, 10, 11, 12
           press unset: creates with PHRASE_MARKER_COLORS[0]
           +shift:      clears the hotcue
    row 3: intro_start / intro_end / outro_start / outro_end
           press:       activate (jump if set, set at playhead if unset)
           +shift:      clear
           LED lit while the marker is set
           Colours: green / lime / amber / orange
           Shift colour: red (same as hotcue clear)
    grid0_1 / grid1_1 (beatjump16): recolour yellow

  loopOdd (loop layer)
    row 2-3: loop sizes 7, 5, 3, 1 / 3/32, 3/16, 3/8, 1/2
    +shift:  beatjump sizes (same cell map)
    grid0_1 / grid1_1 (beatjump direction): recolour yellow
```

```
════════════════════════════════════════════════════════════
LAYER BUTTON  (cycles bottom layer)
════════════════════════════════════════════════════════════

press:  CUE ↔ LOOP  (replacePatch swaps the active layer)
LED:    track-end countdown on DECK A (4 phases: green/yellow/amber/red)
+shift: enters the chord (layer press is intercepted)
```

```
════════════════════════════════════════════════════════════
BOTTOM — encoderL / pressL / encoderR / pressR
════════════════════════════════════════════════════════════

CUE  (base)
encL:  BPM ±0.1                +shift: master gain
       pressL held → head mix
pressL: hold: encL = head mix
       +shift release (no turn): toggle headSplit
       +shift release (with turn): toggle headSplit
       (shift+pressL+encL: encoderL is master gain, not head mix,
       so the turn isn't registered and the toggle fires)
encR:  playlist prev/next       +shift: headphone volume
pressR: hold: arm track load
       while held: TOP press0/3, selectAB cells, and grid2_1 (slip)
       load onto that channel instead of their normal action

LOOP  (diffs from CUE)
encL:  loop resize (IN point moves)    +shift: beatjump
pressL: reloop (anchor→0)              +shift: inherit
encR:  loop resize (OUT point moves)   +shift: halve/double beatjump_size
pressR: beatloop activate              +shift: inherit

ODD  (inherits LOOP, no diffs for these four)
```

```
════════════════════════════════════════════════════════════
SHIFT BUTTON  (top right)
════════════════════════════════════════════════════════════

press:   shift transaction captured → modifiers.shift becomes true
         → shift companions join the stack
release: modifiers.shift becomes false; if chord was active it
         exits first (via the chord modifier)
LED:     track-end countdown on DECK B (same phases as layer button)

The shift layer for the top row is a single shared patch (topShift)
used by all three modes — encoders drive keySync/keyReset on
pinned deck A / B, presses toggle sync/keylock on pinned deck A / B.
```

```
════════════════════════════════════════════════════════════
CHORD  (shift + layer, both held)
════════════════════════════════════════════════════════════

Enter:  hold shift, press layer, keep both held
Exit:   release shift OR layer, OR trigger shutdown timer

Chord activates when both shift and layer are held, regardless of
press order.

Overrides (CHORD patch declares just these):
press0: toggle fxRouting overlay (clears selectAB)
press1: toggle selectAB overlay  (clears fxRouting)
press2: —
press3: hold 2 s → shutdown
pressR: toggleTopMode  (focus ↔ twoDeck-family; mini → focus)
pressL: toggleMiniFx   (twoDeck ↔ mini; focus → no-op)
grid2_1 (slip): dispatched via CUE_SHIFT.grid2_1 / LOOP_SHIFT.grid2_1
                (each branches on modifiers.chord). Under chord:
                toggle layer overlay (cue → hotcue2, loop → loopOdd).
                Under shift only: flip active deck. NOT declared on
                CHORD itself — the shift companion reaches through
                the chord stack.

Passes through unchanged:
enc0-3:  whatever the active top mode bound them to (chord holds shift, so
         shift-layer applies if that mode defines one)
encL/R:  whatever the active bottom layer bound them to
grids:   chord requires shift held, so grid cells behave as shifted

Overlays (enter via chord; exit via layer press or toggle again):

selectAB
  grid row 0: deck A ← channel 1-4
  grid row 1: deck B ← channel 1-4
  grid row 2: fx A   ← unit 1-4
  grid row 3: fx B   ← unit 1-4
  pressR held + tap a deck cell: load onto that channel

fxRouting
  gridN_M: toggle FX(N+1) → channel(M+1) group-enable

Layer overlays (hotcue2 / loopOdd) — see GRID rows 2-3 above.
Both can be on at once; each is visible only in its own layer.
```

---

## FX cell layouts

Every mode's middle cells come from a per-unit layout table. When
the focused FX unit changes, the runtime swaps the live layout
patch automatically. You don't rebind by hand.

| Mode | Table | Layout cells | Emitter | Patch family |
|---|---|---|---|---|
| focus | `FX_LAYOUTS` | 16 | `fxCells(n)` | `fxLayout1..4` |
| twoDeck | `TWO_DECK_FX_LAYOUTS` | 4 per column | `twoDeckFxColumn(side, n)` | `fxLayoutA1..4`, `fxLayoutB1..4` |
| mini | `TWO_DECK_MINI_FX_LAYOUTS` | 8 | `twoDeckMiniFxCells(n)` | `fxMini1..4` |

QFX is **not** in these tables. `knob1_2` / `button1_2` (and, in
twoDeck, `knob2_2` / `button2_2`) are plain bindings in the
top-half base patch.

Notation: `FX<n>.p<m>` = effect `n` parameter `m` on the focused
unit. `meta` = the effect's meta knob. `btn<k>` =
`button_parameter<k>`. `LED→` = display-only cell, no press action.

### Focus mode — `FX_LAYOUTS`

16 cells: rows 0–1 cols 1–3 (12 cells), plus row 2 cols 2–3 (4
cells). Row 2 col 1 is QFX; col 0 is the EQ/PFL column.

Unit 1 is the base table, units 2–4 add overrides. **Bold** =
differs from unit 1.

**Row 0**

| Cell | Unit 1 | Unit 2 | Unit 3 | Unit 4 |
|---|---|---|---|---|
| `knob1_0` | FX2.p1 | FX2.p1 | **FX2.p2** | **FX2.p2** |
| `knob2_0` | FX1.p2 | FX1.p2 | FX1.p2 | **FX1.p1** |
| `knob3_0` | FX1.meta | FX1.meta | FX1.meta | **FX1.p3** |
| `button1_0` | LED→`knob1_0` | LED→`knob1_0` | **LED→`knob1_0` (lo)** | **LED→`knob1_0` (mid)** |
| `button2_0` | LED→`knob2_0` (mid) | LED→`knob2_0` (mid) | LED→`knob2_0` (mid) | **LED→`knob2_0`** |
| `button3_0` | FX1.enable | FX1.enable | FX1.enable | **FX1.enable (lo)** |

**Row 1**

| Cell | Unit 1 | Unit 2 | Unit 3 | Unit 4 |
|---|---|---|---|---|
| `knob1_1` | FX2.p2 | FX2.p2 | **FX2.p3** | **FX2.p3** |
| `knob2_1` | FX2.p3 | FX2.p3 | **FX2.p4** | **FX2.p4** |
| `knob3_1` | FX2.p4 | FX2.p4 | **FX2.p5** | **FX2.p5** |
| `button1_1` | FX2.btn1 | FX2.btn1 | **FX2.btn1 (mid)** | **FX2.btn1 (mid)** |
| `button2_1` | FX2.btn2 | FX2.btn2 | **LED→`knob2_1` (mid)** | **FX2.btn2 (mid)** |
| `button3_1` | FX2.enable (lo) | FX2.enable (lo) | FX2.enable (lo) | FX2.enable (lo) |

**Row 2 (cols 2–3)**

| Cell | Unit 1 | Unit 2 | Unit 3 | Unit 4 |
|---|---|---|---|---|
| `knob2_2` | FX3.p2 | FX3.p2 | FX3.p2 | FX3.p2 |
| `knob3_2` | FX3.meta | FX3.meta | FX3.meta | FX3.meta |
| `button2_2` | LED→`knob2_2` | LED→`knob2_2` | LED→`knob2_2` | LED→`knob2_2` |
| `button3_2` | FX3.enable | FX3.enable | FX3.enable | FX3.enable |

`button0_2` (active FX route) comes from `TOP_SHARED`, not this
table — it routes the focused FX unit into the active deck.
`button2_2` is display-only on every unit — echoes `knob2_2`.

### twoDeck — `TWO_DECK_FX_LAYOUTS`

4 cells per column, per unit, in rows 0–1. Column 1 emits into
`knob1_0` / `knob1_1` / `button1_0` / `button1_1` and follows
`$FX_A_N$`. Column 2 emits into the `*2_*` cells and follows
`$FX_B_N$`.

```
              col 1        col 2
row 0  knob   knob1_0      knob2_0
       btn    button1_0    button2_0
row 1  knob   knob1_1      knob2_1
       btn    button1_1    button2_1
```

All four units share the same table:

| Cell in col 1 | Cell in col 2 | Content |
|---|---|---|
| `knob1_0` | `knob2_0` | FX1.meta |
| `knob1_1` | `knob2_1` | FX2.meta |
| `button1_0` | `button2_0` | FX1.enable (lo) |
| `button1_1` | `button2_1` | FX2.enable (lo) |

Row 2 comes from `TWO_DECK`, not this table: `knob1_2` /
`button1_2` = QFX A super/enable, `knob2_2` / `button2_2` = QFX B
super/enable. `button0_2` / `button3_2` are the low-EQ route
cells: `button0_2` = FX-A → deck A, `button3_2` = FX-B → deck B.

### twoDeck mini — `TWO_DECK_MINI_FX_LAYOUTS`

8 cells in rows 0–1, all pointing at `$FX_ACTIVE_N$`:

```
              col 1        col 2
row 0  knob   knob1_0      knob2_0
       btn    button1_0    button2_0
row 1  knob   knob1_1      knob2_1
       btn    button1_1    button2_1
```

All four units share the same table:

| Cell | Content |
|---|---|
| `knob1_0` | FX1.p1 |
| `knob2_0` | FX2.p1 |
| `knob1_1` | FX1.meta |
| `knob2_1` | FX2.meta |
| `button1_0` | FX1.btn1 (lo) |
| `button2_0` | FX2.btn1 (lo) |
| `button1_1` | FX1.enable (lo) |
| `button2_1` | FX2.enable (lo) |

`twoDeckMini` inherits everything else from `twoDeck` — encoders,
press row, shift row, faders, low-EQ route cells, QFX. The only
difference between oneK3 and oneK3 mini is this middle layout.

### Editing a layout

Change the table entry in `xone-k3-cat-fx.js`. See CUSTOMISE §1.9
for the field vocabulary and worked examples.

### QFX chain presets

`QFX_CHAIN_LIST` in `xone-k3-cat-schemes.js` must match
Preferences → Effects → Quick Effect Chains. Only the chains
listed there light up correctly on the QFX buttons. To add a new
chain: create it in Mixxx, then add its name to the list. See
CUSTOMISE §2.9.

### Shipped FX presets

`fx-presets/` ships Mixxx effect-chain XMLs that line up with
the layouts here:

**Full-size chains** — pairs with the focus-mode `fxLayout<n>`
layout. Also works with oneK3 twoDeck columns as-is (they only
touch each effect's meta + enable, so any of the four is fine):

| File | Layout |
|---|---|
| `1-EchoMoog.xml` | Moog → Echo → Moog |
| `2-2ndEchoMoog.xml` | Moog → Echo → Moog — second copy for a duplicate FX unit; same chain shape as preset 1 |
| `3-FlangerMoog.xml` | Moog → Flanger → Moog |
| `4-TremeloAutopan.xml` | Autopan → Tremolo → Moog |

**Mini chains** — pairs with the mini `echo` layout (units 2–4):

| File | Layout |
|---|---|
| `1.miniEcho.xml` | Echo — time / feedback / ping-pong / send + quantize / triplets |

**Quick Effect chains** — pairs with `QFX_CHAIN_LIST` in
`xone-k3-cat-schemes.js`, in order:

| File | `QFX_CHAIN_LIST` index |
|---|---|
| `qfx1-Reverb.xml` | 1 `filterReverb` |
| `qfx2-Smooth Growl.xml` | 2 `filterSmoothGrowl` |
| `qfx3-Echo.xml` | 3 `filterEcho` |
| `qfx4-Flanger.xml` | 4 `filterFlanger` |
| `qfx5-Moog.xml` | 5 `filterMoog` |
| `qfx6-Balance.xml` | 6 `filterBalance` |

(`filterNone` is Mixxx's built-in empty chain, index 0 — no
preset file.)

**Import:** copy the XML into Mixxx's user folder — `effects/`
for full-size chains, `quickeffectchains/` for QFX — then
reload and pick the chain from the preset menu. Paths:
`~/.mixxx/` on Linux, `~/Library/Application Support/Mixxx/`
on macOS, `%LOCALAPPDATA%\Mixxx\` on Windows.

### Making a single-effect layout (oneK3 + mini)

Shipped defaults drive two effects per FX unit. To drive one
effect from all the controls instead (e.g. an Echo with time /
feedback / ping-pong / send on knobs plus quantize / triplets /
enable on buttons), override the layout table.

**Find the parameter number:** hover over the knob or button in
Mixxx — the status bar shows the CO name (`parameter2`,
`button_parameter1`). That's the `param:` / `buttonParam:` value.

**oneK3 — column A on effect 1:**

```js
// in xone-k3-cat-fx.js
var TWO_DECK_FX_LAYOUTS = {
    1: twoDeckFxTable({
        knob0:   { effect: 1, param: 1 },             // knob1_0
        knob1:   { effect: 1, param: 2 },             // knob1_1
        button0: { effect: 1, buttonParam: 1 },       // button1_0
        button1: { effect: 1, enabled: true }         // button1_1
    }),
    2: twoDeckFxTable(), 3: twoDeckFxTable(), 4: twoDeckFxTable()
};
```

Column B keeps its default. Column A gives you 2 knobs + 2
buttons for effect 1.

**mini — full Echo on effect 1.** Positions `1 2 / 3 4` (knob
above, button below in each):

| | Knob | Button |
|---|---|---|
| 1 | time | quantize |
| 2 | ping-pong | triplets |
| 3 | feedback | display only |
| 4 | send | enable |

This is the shipped `echo` layout (units 2–4). To use it on a
different unit, or to build your own, add a named layout to
`MINI_LAYOUTS` (in `xone-k3-cat-fx.js`):

```js
// MINI_LAYOUTS:
myEcho: {
    knob0:   { effect: 1, param: 1 },           // pos 1  time
    button0: { effect: 1, buttonParam: 1 },     //        quantize
    knob1:   { effect: 1, param: 3 },           // pos 2  pingpong
    button1: { effect: 1, buttonParam: 2 },     //        triplets
    knob2:   { effect: 1, param: 2 },           // pos 3  feedback
    button2: { displayOnly: true, on: true },   //        display only
    knob3:   { effect: 1, param: 4 },           // pos 4  send
    button3: { effect: 1, enabled: true }       //        enable
}
```

Then pick it per unit:

```js
var TWO_DECK_MINI_FX_LAYOUTS = {
    1: twoDeckMiniFxTable('split'),    // two-effect default
    2: twoDeckMiniFxTable('myEcho'),
    3: twoDeckMiniFxTable('myEcho'),
    4: twoDeckMiniFxTable('myEcho')
};
```

The shipped `echo` layout is already wired to units 2–4 and
pairs with `1.miniEcho.xml` — load that preset on each unit the
layout drives.

No Mixxx effect chain preset is shipped for the `split` default
(unit 1) — its shape assumes a different per-effect arrangement,
so build your own. Same for oneK3: its default is just meta
knobs on effect 1 and 2 with their enables.

The same shape works for focus mode (`FX_LAYOUTS`), with
different cell names. Full field vocabulary: CUSTOMISE §1.9.

---

## LAB layout and behaviour

Same physical layout as above. colorLab replaces the entire layer
tree — none of the playback bindings above apply. Two modes,
swapped by the layer button.

```
════════════════════════════════════════════════════════════
EDIT mode — TOP
════════════════════════════════════════════════════════════

                    unshifted                        +shift
enc0                R ±1                             curve ±0.025
enc1                G ±1                             brightness ±1
enc2                B ±1                             brightnessScale ±0.05
enc3                brightness ±1                    unbound
fader0..fader3      R/G/B/brightness abs (0..127)    unchanged
knob0_0             preview, co-matched to QFX       preview, no co-match
knob1_0             markerPosition                   unchanged
knob2_0             schemePosition                   unchanged
all other knobs     ratio[0..8]                      unchanged
button0_0           toggle preview on/off ramp       unchanged
button1_0           reset markerPosition → 63.5      reset brightness → 127
button2_0           reset schemePosition → 63.5      reset brightnessScale → 1
segment buttons     select segments 0-8              unchanged
press0              print selection                  dump scheme
press1              toggle calibration               toggle white balance
press2              toggle ratio edit                unchanged
press3              toggle interp                    toggle tuning overlay

Press LED convention: green = on, red = off (press1/2/3).
press0 LED: dim white unshifted, dim cyan shifted.
Segment buttons: LED shows that segment's ramp colour.


════════════════════════════════════════════════════════════
EDIT mode — GRID
════════════════════════════════════════════════════════════

Segment → cell map (segments 0–8 on the top button row, 9–22 on
the grid):

button3_0: seg 0   button0_1: seg 1   button1_1: seg 2
button2_1: seg 3   button3_1: seg 4   button0_2: seg 5
button1_2: seg 6   button2_2: seg 7   button3_2: seg 8

grid0_0: seg 9    grid1_0: seg 10   grid2_0: seg 11   grid3_0: seg 12
grid0_1: seg 13   grid1_1: seg 14   grid2_1: seg 15   grid3_1: seg 16
grid0_2: seg 17   grid1_2: seg 18   grid2_2: seg 19   grid3_2: seg 20
grid0_3: seg 21   grid1_3: seg 22   grid2_3: toggle beat edit overlay
                                    grid3_3: select marker

LEDs: each segment cell shows its ramp colour; empty stops
render black. grid3_3 shows the marker colour, or black when no
marker. grid2_3: cyan @60 when the beat overlay is on, white @20
when off.

+shift:
  segment cells: shift+press deletes that segment
  grid3_3:       shift+press clears the marker


════════════════════════════════════════════════════════════
EDIT mode — BOTTOM
════════════════════════════════════════════════════════════

layer:   cycle to SLOTS                       LED: dim yellow
encL:    markerWidth                          +shift: scroll registry
pressL:  fill gaps                            +shift: inherit
encR:    unbound                              +shift: unbound
pressR:  unbound                              +shift: load preview into edit

Grid cells use the same segment map as EDIT GRID, including
grid2_3 (beat edit toggle) and grid3_3 (marker select).


════════════════════════════════════════════════════════════
SLOTS mode
════════════════════════════════════════════════════════════

Entered from EDIT by pressing layer. Top section stays bound but
every edit func is a no-op — the preview LED and preview sweep
still work.

layer:   cycle back to EDIT                   LED: purple (bright when armed)
encL:    unbound                              +shift: scroll registry
pressL:  exit to EDIT, buffer unchanged       +shift: inherit
encR:    unbound                              +shift: unbound
pressR:  arm save                             +shift: load preview into
                                              edit, exit to EDIT

Grid cells: 16 slots, row-major.

grid0_0 = slot 0    grid0_1 = slot 1    grid0_2 = slot 2    grid0_3 = slot 3
grid1_0 = slot 4    grid1_1 = slot 5    grid1_2 = slot 6    grid1_3 = slot 7
grid2_0 = slot 8    grid2_1 = slot 9    grid2_2 = slot 10   grid2_3 = slot 11
grid3_0 = slot 12   grid3_1 = slot 13   grid3_2 = slot 14   grid3_3 = slot 15

Grid press (unarmed) → preview that slot. Grid stays visible.
Grid press (armed)   → save current preview into that slot,
                       load into edit, exit to EDIT.

Cell colours:
  0     green        (on)
  1     red          (off)
  2     white        (co)
  3     soft blue    (co-off)
  4-11  yellow       (saves)
  12    blue         (structural bake)
  13    cyan         (baked + interp)
  14    magenta      (baked, snapped)
  15    orange       (export)

Empty: dark. Populated: dim. Currently previewed: bright.

### The four bake slots

Slots 12–15 aren't yours to save into. They rewrite automatically
every time the preview changes — a compiled copy of whatever
scheme is currently previewed, at four fidelity levels:

| Slot | What it is |
|---|---|
| 12 | Source stops, everything live — still editable |
| 13 | Source stops, brightness pre-solved, interp forced smooth |
| 14 | Source stops, brightness pre-solved, interp forced snapped |
| 15 | Source stops, brightness pre-solved, white balance baked into rgb — export only |

All four keep the source stop count and the source's
`curve` / `schemePosition` / `ratio` as live fields. The
difference is what's been frozen into the stops.

Which one to paste:

- **12 structural** — same stop count as your edit buffer,
  everything still live and re-solvable. Use this when you want
  to keep editing after paste.
- **13 baked+interp** — same stops, brightness pre-solved, interp
  forced smooth. Paste and forget for this runtime.
- **14 baked** — same stops, brightness pre-solved, interp forced
  snapped. Paste and forget for this runtime when you want
  discrete bands.
- **15 export** — same stops, brightness pre-solved, white
  balance baked in. Hand-off to a foreign script or machine that
  sends rgb + brightness directly. Reloading here double-applies
  wb — don't ship export as a runtime scheme.

Full matrix of what each level keeps vs bakes: `LAB.md` §Slots
and bakes.


════════════════════════════════════════════════════════════
OVERLAYS
════════════════════════════════════════════════════════════

TUNING overlay (host: colorLabTop, toggle with shift+press3 in EDIT)

While active, two shift-layer knob overrides:
  enc1: hkBoost (0..2, step 0.02)       ← replaces brightness
  enc2: lumGamma (0.1..2, step 0.02)    ← replaces brightnessScale

BEAT EDIT overlay (host: colorLab, toggle with grid2_3 in EDIT)

encL:  barBeats (1..32)                  +shift: inherit
encR:  phraseBars (1..32)                +shift: prePhraseBeats

grid0_0:  select beat target        grid1_0:  select bar target
grid2_0:  select prePhrase target   grid3_0:  select phrase target
grid0_1:  toggle beat.enabled       green=on, red=off

All other grid cells nulled.

RGB encoders and faders edit the selected target's colour and
brightness.

+shift:
  target cells: shift+press clears that target's colour


════════════════════════════════════════════════════════════
CHORD
════════════════════════════════════════════════════════════

press3: hold 2 s → Mixxx shutdown

Releasing shift, layer, or press3 cancels the timer. Works in
both EDIT and SLOTS.
