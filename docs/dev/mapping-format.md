# Controller mapping format

DJ Sugar keeps a DJ's own controller mappings, and the ones it imports from
other DJ apps, in one documented format: a JSON file with the extension
`.djsugar-mapping`. This page is the reference for version 1.

The format is a **superset of Mixxx's** `.midi.xml`: anything a Mixxx
mapping says can be written in it without loss, and it adds what other apps
have and Mixxx doesn't (one-line DJSugarScript actions, shift layers, long
press, LED scripts with colors, variables). A mapping can stand alone or be
**based on** a Mixxx mapping and hold only what it changes.

## A small example

```json
{
  "format": "djsugar-mapping",
  "version": 1,
  "name": "DDJ-FLX4 Nahuel 2026",
  "author": "Your Name",
  "controller": { "name": "Pioneer DDJ-FLX4", "ports": ["DDJ-FLX4"], "layout": "flx4" },
  "based_on": "Pioneer-DDJ-FLX4.midi.xml",
  "imported": { "app": "VirtualDJ", "file": "Pioneer DDJ-FLX4 - DDJ-FLX4 Nahuel 2026.xml", "date": "2026-10-29" },
  "rows": [
    { "midi": "97 04", "name": "HOTCUE_PAD5", "script": "deck 1 play_pause" },
    { "midi": "98 05", "name": "SHIFT_HOTCUE_PAD6", "script": "deck 1 pitch_zero" },
    { "midi": "90 58", "name": "SYNC", "when": "long", "script": "deck 1 master_deck" },
    { "midi": "B0 21", "name": "JOG", "off": true, "original": "touchwheel",
      "note": "kept the jog from DJ Sugar's own FLX4 mapping" }
  ],
  "lights": [
    { "midi": "97 04", "script": "deck 1 play ? on : off" }
  ]
}
```

## Top level

| Field | | What it is |
|---|---|---|
| `format` | required | Always `"djsugar-mapping"`. |
| `version` | required | `1`. A reader refuses a higher major version it doesn't know. |
| `name` | required | Shown in the Controller Editor's Mapping menu. |
| `author`, `description` | optional | Free text. |
| `controller` | optional | `name` (shown), `ports` (MIDI port names it matches, as Mixxx's `<info>`), `layout` (DJ Sugar's drawing of the controller: `flx4`, `xone-k2`, `twister`). |
| `based_on` | optional | A Mixxx mapping file name. Its bindings, lights, scripts and settings run first; `rows` and `lights` here replace or add to them. Without it the file is the whole mapping. |
| `imported` | optional | Where it came from: `app` (`VirtualDJ`, `Traktor`, `rekordbox`, `djay Pro`, `Mixxx`), `file`, `date`. |
| `scripts` | optional | JavaScript files run as Mixxx runs them: `[{ "file": "...", "prefix": "..." }]`. A shared file carries them in `sources` (below). |
| `settings` | optional | The mapping's options: Mixxx's `<settings>` definitions as `[{ "variable", "type", "label", "default", "choices" }]`, plus `values` with the DJ's picks. |
| `decks` | optional | `2` or `4`: whether the controller is used with two decks or four, for a mapping with a layout for each (VirtualDJ's Xone K2). Rows and lights on the layer `decks=2` or `decks=4` work only in that use; the rest of their layer still applies. Chosen in Settings › Controllers. |
| `deck_order` | optional | With four decks, the deck at each position from left to right, `[3, 1, 2, 4]` as on DJ Sugar's 4-Deck screen. Rows and lights marked `by_position` have their decks (`deck 2`, `[Channel2]`, effect unit 2) moved to it. |
| `variables` | optional | Variables the scripts use: `{ "k2mode": { "default": 0, "saved": true } }`. `saved` keeps the value across restarts (VirtualDJ's `@$`). |
| `rows` | optional | What the controls do, below. |
| `lights` | optional | What the LEDs show, below. |
| `sources` | optional | Only in a shared file: `{ "file name": "file text" }` for each script and for `based_on` when it isn't one DJ Sugar ships. |

## Rows

A row says what one MIDI message does, on one layer.

| Field | | What it is |
|---|---|---|
| `midi` | required | Status and number in hex: `"90 0B"` (note 0B, channel 1), `"B0 13"` (CC 13). A 14-bit pair is `"B0 00+B0 20"` (MSB then LSB). Pitch bend is `"E0"`. |
| `name` | optional | The control's name in the app it came from (`PLAY_PAUSE`). Shown in the editor. |
| `layer` | optional | Variables the row needs, joined with `&`: `"shift"` acts only while the variable `shift` is above 0, `"m1=2 & m2=0"` only while `m1` is 2 and `m2` is 0 (Traktor's modifiers). Hardware shift (controllers that send other notes with SHIFT held) needs no layer: it's another `midi`. |
| `when` | optional | `"press"` (default; for a fader or knob, every move), `"release"`, `"long"` (held 500 ms), `"tap"` (let go before 500 ms), `"double"` (two presses within 300 ms). VirtualDJ's `holding ? a : b` becomes a `long` row and a `tap` row. |
| one action | required unless `off` | `bind`: a control, `"[Channel1],play"`, with Mixxx's `options` (`["button"]`, `["soft-takeover", "invert"]`, `["rot64"]` …). `js`: a script function, `"PioneerDDJFLX4.jog"`. `script`: an DJSugarScript line, `"deck 1 loop 4"`. |
| `takeover` | optional | For a `bind` to a fader or knob: `"jump"` (default), `"pickup"` (waits for the control to reach the value, Mixxx's soft takeover) or `"scale"` (moves towards the value as you turn). |
| `range` | optional | For a `bind`: the part of the control's range the hardware covers, `[0.25, 0.75]` (0 to 1). |
| `off` | optional | `true`: the row is kept but does nothing (greyed in the editor). Imports use it for actions DJ Sugar can't do yet. |
| `original` | optional | The action as the other app wrote it (`"touchwheel 'search'"`). |
| `note`, `description` | optional | Why it's off; what the author wrote about it. |
| `remove` | optional | `true`: take the `based_on` mapping's bindings for this message away. |
| `steps` | optional | For a `script` row on a relative encoder (1 a click right, 127 a click left): the line gets the clicks, `1`, `-1` or more, so `deck 1 jump 1` jumps a beat a click and `deck 1 jump 4` four, `deck 1 jog` turns the deck as a jog wheel, and a level verb with no level written (`deck 1 filter`, `adjust '[Master],headGain'`) moves 2.5% a click from where it is (`song_pos` 0.5%). `value > 0` asks which way it turned. |
| `by_position` | optional | The row's deck is where its control sits (a column of the K2), so `deck_order` picks it. |

**Layers.** When several rows of one message hold, the one on the
most conditions wins: with SHIFT held a row on `shift` acts and the row on
no layer doesn't, and `shift & m1=2` wins over `shift`. Rows on a Note On
(`"9E 0F"`) also hear the controller's Note Off for it (`8E 0F`) as the
release, unless the file has rows of its own for the Note Off.

**Merging with `based_on`.** Rows are matched to the base mapping by `midi`,
`layer` and `when`. A row replaces every base binding of that message on that
layer; a row with `remove` deletes them; anything else is added. In an
imported mapping (one with `imported`) the rows say everything a message
does, so a row replaces the base's bindings of that message on every layer.
An `off` row replaces nothing: when all of a message's rows are `off`, the
base binding keeps working underneath, so an import can't make a control go
dead that worked before.

**Values.** A `script` row on a fader, knob or encoder gets the control's
value: verbs that set a level (`volume`, `eq_high`, `filter`) take it when
no level is written, as a 0 to 1 number (7-bit or 14-bit), or, for an
encoder (`options: ["rot64"]` or `["diff"]`), as steps.

## Lights

| Field | | What it is |
|---|---|---|
| `midi` | required | The message sent: `"90 0B"`. |
| `bind` + `min`, `max`, `on_value`, `off_value` | one of | Mixxx's `<output>`: send `on_value` (127 unless given) while the control is between `min` and `max`, else `off_value` (0). |
| `js` | one of | Lights driven by the scripts (nothing to do here; listed so the editor can show them). |
| `script` | one of | An DJSugarScript line asked several times a second; it ends in `on`, `off`, `blink` (with a time and a color: `blink 500 'green'`), `color 'red'` or a number 0 to 127. A condition picks between them: `get_vu_meter > 75% ? off : color 'orange'`. A line that is only a level (`vu_meter`, `adjust 'rate'`, `value`) sends that level, 0 to 127, for LED rings and meters. | |
| `colors` | optional | What `color` sends on this controller: `{ "red": 15, "orange": 51, "green": 60 }`. |
| `color_notes` | optional | `true` when the controller picks a color by note, not velocity (the Xone K2: red is the button's note, amber 36 above it, green 72 above). `color` then sends the note plus the color's number at full velocity. |
| `layer`, `by_position` | optional | As for rows; a `script` light on a layer is off while the layer doesn't hold. |
| `name`, `off`, `original`, `note` | optional | As for rows. |

## Imports

Each file imported becomes one mapping of the DJ's own with `based_on` set
to the controller's Mixxx mapping, so anything the import can't do falls back
to what worked before.

| App | File | What comes across |
|---|---|---|
| VirtualDJ | `Mappers/*.xml` | VDJscript is read and written as DJSugarScript: `holding` becomes `long` and `tap` rows, deck and side words are settled for the control, `LED_` keys become lights (the Xone K2 VU meters and colors too). VirtualDJ keeps its controller definitions encrypted, so the keys are placed with DJ Sugar's own tables for the FLX4, Xone K2 and Midi Fighter Twister. A keyboard mapping becomes a shortcut set. |
| Traktor | `.tsi` | Every device in the file on the same port, with its MIDI names, encoder modes, modifier conditions (as `layer`s like `m1=2`), Hold/Toggle/Direct modifier buttons and LED outputs. Native Instruments controllers in their own mode (S2, S4) can't come across. |
| rekordbox | `.midi.csv` | The functions and their deck columns, with SHIFT rows and LEDs. |
| djay Pro | `.djayMidiMapping` | Its actions on each message, with the deck it names. |

Rows that don't translate are `off`, with `original` and a `note` saying
why. The Controller Editor shows them greyed and lists them in its report.

## How it maps to Mixxx

| Mixxx `.midi.xml` | DJ Sugar mapping |
|---|---|
| `<info><name>`, `<author>`, `<description>` | `name`, `author`, `description` |
| `<info>` device matching | `controller.ports` |
| `<settings>` options | `settings` |
| `<scriptfiles><file filename functionprefix>` | `scripts` |
| `<control>` with a control `key` and `<options>` | row with `bind` and `options` |
| `<control>` with `<Script-Binding/>` | row with `js` |
| `<control>` pair with `fourteen-bit-msb/lsb` | one row, `midi: "B0 00+B0 20"` |
| `<output>` | light with `bind`, `min`, `max`, `on_value`, `off_value` |
| — | rows with `script`, `layer`, `when`, `off`; lights with `script`; `variables`; `based_on`; `imported` |

So a Mixxx mapping converts to an DJ Sugar mapping with nothing lost. Going
back, **Export as Mixxx** writes every `bind` and `js` row and every `bind`
light; an DJSugarScript row that is just one control (`deck 1 sync`) becomes a
plain binding; the rest are left out and listed in the export report. A
`based_on` mapping is exported with its base merged in and its script files
copied next to it.

## Files and sharing

- DJ Sugar's own mappings ship in this format next to Mixxx's, in
  `res/controllers/*.djsugar-mapping` (the Xone K2's two-deck mapping is the
  first). They are listed and matched to devices like Mixxx's, and a DJ's
  copy can be `based_on` one of them.

- The DJ's mappings live in `<config>/controller-mappings/<id>.json`, in this
  format. Files from DJ Sugar 0.4 (`name`, `based_on` and `changes`, no
  `format`) are read as version 1 with `changes` as `rows`.
- **Export** writes one self-contained `.djsugar-mapping`: scripts that don't
  ship with DJ Sugar go in `sources`, so the file works on another computer.
- A reader keeps fields it doesn't know when it writes a file back, so newer
  files survive older versions of DJ Sugar.
