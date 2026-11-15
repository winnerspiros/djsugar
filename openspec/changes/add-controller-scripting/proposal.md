# Proposal

Planned for: **v0.5** (with the controller mapper, #133)

## Why

A user request (2026-11-01): the Controller Editor should support DJSugarScript, so a
mapping can do things like conditional logic based on the controller's
modes, modifiers and SHIFT. The mapping format and the runtime already have
it (DJSugarScript rows, `layer` conditions on variables, `set_var` /
`toggle_var` / `var`, long press, tap, double press), and VirtualDJ and
Traktor imports already write it; the editor could only give a control a
plain function.

## What Changes

- The row editor gets **Does**: *A function* (as before), *An DJSugarScript
  line*, *Shift: on while held* or *Mode: on/off each press*.
  - A line is checked as it's typed (the parser's own message under it) and
    used on Enter; **Acts on** (press, release, long press, tap, double
    press) applies to it as to a function.
  - Shift makes two rows, `set_var 'shift' 1` on press and `set_var 'shift'
    0` on release; Mode one, `toggle_var 'mode'`. The name can be changed;
    the variable is added to the mapping's `variables`. Everything else the
    button did on that layer goes.
- Every row gets **Only when**: the variables it needs (`shift`, `mode=2 &
  shift`), checked as typed. **Move** puts the row on that layer instead,
  **Copy** as well, so SHIFT + CUE can do something else while CUE still
  cues. The mapping's variables are listed under it.
- Lines can ask anything DJSugarScript asks: `var 'shift' ? sync : play_pause`,
  `deck 1 play ? deck 1 pause : deck 1 play`.

## Capabilities

### Modified Capabilities

- `controllers`: the Controller Editor edits DJSugarScript rows, modifiers and layers

## Out of Scope (later)

- A verb list and completion in the line field (the keyboard page's list
  could be reused).
- Editing variables' defaults and `saved` in Options; LED rows driven by a
  variable (lights already take a `layer`).
- Holding/repeat verbs (`while_pressed`, `repeat`), and scripts for Map mode.

## Impact

`crates/controllers/src/edit.rs` (`Does`, `set_does`, `set_does_at`,
`set_layer`, `check_layer`), the row editor in `controller_editor.slint` and
`crates/app/src/controller_editor.rs`. No format change.
