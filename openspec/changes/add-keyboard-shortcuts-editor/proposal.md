# Proposal

Planned for: **v0.4**

## Why

A user request asked for a keyboard shortcuts editor (v0.4 list, 2026-10-28). Since add-djsugarscript, every shortcut is a key plus an DJSugarScript line, but the only way to change one was editing settings.json.

## What Changes

- Settings → Keyboard, a new page between Mixer and Feedback. It lists every shortcut: the defaults (⌘1-⌘5 → `rating 1`-`rating 5`) and then the DJ's own.
- Each row has a key button (click, then press the new key; Esc leaves it), the DJSugarScript line, which is checked as you type and shows the verbs' help or the mistake, an Actions menu that inserts a verb (or adds it with `&`), and Reset (changed default), Turn Off (default) or Remove (own).
- Moving a key onto one already in use takes it over and says what it replaced. Space, ⌘, and Esc can't be used.
- "Add a key…" starts a new shortcut, and Reset All goes back to the defaults.
- A list of every verb with its arguments and help, under ACTIONS.
- Edits keep settings `keyboard_shortcuts` minimal: a row that's back to its default drops its entry.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `scripting`: the DJ edits shortcuts in Settings.

## Out of Scope

- Presets (rekordbox, VirtualDJ or Mixxx style), and more default keys. Both wait on a decision.
- Recording a shortcut's key while the app plays: the editor only lives in Settings.

## Impact

App only: prefs.rs (Keyboard page), shortcuts.rs (row editing, key display), preferences.slint (shortcut row, key capture). djsugar-script exports ArgKind, ArgSpec and TakesDeck for editors.
