# Keyboard shortcuts

| Key | Does |
|---|---|
| <kbd>Space</kbd> | Play or pause the master deck |
| <kbd>←</kbd> <kbd>→</kbd> | Bend the deck you started last slower or faster while held, 2% (4% with <kbd>Shift</kbd>); see [pitch bend](decks.md#slip-bend-and-censor) |
| <kbd>⌘</kbd> <kbd>1</kbd> to <kbd>⌘</kbd> <kbd>5</kbd> (Linux and Windows: <kbd>Ctrl</kbd>) | Rate the tracks picked in the browser 1 to 5 stars |
| <kbd>⌘</kbd> <kbd>I</kbd> (Linux and Windows: <kbd>Ctrl</kbd> <kbd>I</kbd>) | Show the picked track in the side panel's Info tab, as **Show Info** on the track menu does |
| <kbd>⌘</kbd> <kbd>A</kbd> (Linux and Windows: <kbd>Ctrl</kbd> <kbd>A</kbd>) | Pick every track in the track list or side list you last clicked, to drag or analyze them together |
| <kbd>⌘</kbd> <kbd>,</kbd> (Linux and Windows: <kbd>Ctrl</kbd> <kbd>,</kbd>) | Open or close Settings |
| <kbd>⇧</kbd> <kbd>⌥</kbd> <kbd>←</kbd> / <kbd>⇧</kbd> <kbd>⌥</kbd> <kbd>→</kbd> (Linux and Windows: <kbd>Shift</kbd> <kbd>Alt</kbd>) | Unload deck 1 or deck 2 (`deck 1 eject`), as ⏏ on the deck's cover does; not while it plays |
| <kbd>Esc</kbd> | Close Settings |
| Media keys (play/pause, next, previous) | In [Player](layouts.md), play or pause, mix into the next track in Up Next, or go back to the start of the track; headphone buttons and the system's Now Playing do the same. Other layouts only with **Media keys in every layout** on in Settings › Player |

## DJSugarScript

Each shortcut, apart from Space, ⌘A, ⌘, and Esc (and ← and →, unless you
give them shortcuts of your own), runs a line of DJSugarScript,
a short action language in the style of VirtualDJ's VDJscript. ⌘3 runs
`rating 3`.

```text
rating 3                          rate the tracks picked in the browser
deck 1 rating 5                   rate the track on deck 1
deck 2 play_pause                 a verb on one deck; alone, it's the master deck
sync & play                       & runs one action after another
play ? pause : play               condition ? when true : when false
(sync & key_lock) ? play : pause  parentheses group actions
toggle 'slip_enabled'             any control by its Mixxx name
set '[Master],gain' 1             a control of another group
```

| Verb | Does |
|---|---|
| `play`, `pause`, `play_pause` | Start or stop the deck |
| `goto_start` | Jump to the start of the track |
| `sync`, `key_lock`, `quantize`, `slip` | Turn that on or off |
| `edit_grid` | Start or finish editing the beat grid (G) |
| `loop 4`, `loop_exit` | Loop that many beats (4 if left out), end the loop |
| `hot_cue 1` | Play from a hot cue, or set it |
| `eject` | Unload the deck |
| `volume 80%` | Set the channel fader |
| `crossfader` | Turn the crossfader on or off |
| `rating 0` to `rating 5` | Rate tracks; 0 clears the rating |
| `show_info` | Open the side panel's Info tab on the track picked in the browser |
| `set`, `toggle`, `press`, `get` | Any control; `get` only asks, in a condition |
| `adjust 'rate' 50%` | Any control by its 0 to 1 position; a controller's knob gives the level when none is written |
| `jump 4`, `loop_move` | Jump beats, move the loop by its length; on an encoder, each click (back when turned left) |
| `expand_browser` | Expand the browser over the decks, or back (the button at the bottom left of the browser) |
| `value` | What the controller sent, 0 to 1; only in a controller's line, such as `value > 50% ? play : pause` |

In a condition a verb asks rather than does: `play ? …` asks whether the
deck is playing.

## Changing shortcuts

Open **Settings → Keyboard**. Each row is a key and the line it runs:

- Click the key, then press the new one (Esc leaves it as it was). If that
  key already did something, this shortcut takes it over and the page says
  what it replaced.
- Type in the line, or pick a verb from **Actions**; it's added with `&`
  when the line already has something. A mistake shows in red under it.
- **Reset** puts a changed default back, **Turn Off** stops a default key,
  and **Remove** deletes one of your own.
- **Add a key…** at the end starts a new shortcut, and **Reset All** goes
  back to DJ Sugar's own.

Every verb is listed under ACTIONS at the bottom of the page.

## Shortcut sets

Shortcuts are kept in sets, chosen from the **Set** menu at the top of
**Settings → Keyboard**. DJ Sugar starts with **Default**.

- **New set from this one** copies the set in use under a new name; type
  over the name to rename it.
- **Import…** reads a VirtualDJ keyboard mapping (`.xml` from
  `Documents/VirtualDJ/Mappers`) into a new set. Keys whose action DJ Sugar
  can't do yet are left out, and the page says how many.
- **Delete this set** removes it and goes back to Default, which can't be
  deleted.
