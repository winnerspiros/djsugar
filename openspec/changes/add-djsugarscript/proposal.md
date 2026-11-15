# Proposal

Planned for: **v0.4**

## Why

Keyboard shortcuts, and later controller rows, pads, OSC and the MCP server, all need a way to say what a key or button does. VirtualDJ users know VDJscript's one-liners; a user request asked for a basic DJ Sugar equivalent (2026-10-28), with ⌘1 to ⌘5 rating tracks as its first use.

## What Changes

- New crate `crates/script` (djsugar-script): parser and runner for one-line actions: verbs, `deck N` / `deck master`, `&` chains, `? :` conditions, parentheses. Mistakes are reported with where they are, before anything runs.
- Verbs: play, pause, play_pause, goto_start, sync, key_lock, quantize, slip, loop, loop_exit, hot_cue, eject, volume, crossfader, rating, and set / toggle / press / get for any control.
- Keyboard shortcuts run DJSugarScript: defaults in `crates/app/src/shortcuts.rs` (⌘1-⌘5 → `rating 1`-`rating 5`), the DJ's own in settings `keyboard_shortcuts` as `[{key, script}]`, replacing the default key by key; an empty script turns a key off. Keys are written `cmd+alt+shift+k` (`cmd` is Ctrl on Linux).

## Capabilities

### New Capabilities

- `scripting`: DJSugarScript and the keyboard shortcuts that run it

### Modified Capabilities

None.

## Out of Scope

- The shortcuts editor UI (separate change, builds on this).
- Variables, `holding` / `while_pressed`, `repeat`, and using scripts in controller mappings, pads and OSC.
- Full VDJscript compatibility.

## Impact

New crate; app: shortcuts module, browser rating by rows or deck, a Slint callback for keys nothing else took.
