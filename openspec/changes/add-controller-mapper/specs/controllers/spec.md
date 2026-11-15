## ADDED Requirements

### Requirement: Own mappings
Each controller SHALL have its Original Mapping, read only, and the DJ's own mappings made with Duplicate, which can be renamed and deleted. One mapping per controller SHALL be in use; Reload Original SHALL switch back to the original.

#### Scenario: Duplicate and use
- **WHEN** the DJ duplicates the FLX4's Original Mapping as "My FLX4" and picks it
- **THEN** the FLX4 runs "My FLX4", and Reload Original puts the original back

### Requirement: Edit a mapping
In a mapping of the DJ's own, a row's function, target and advanced options (control type, button behaviour, encoder mode, invert, takeover, range) SHALL be editable, and script rows SHALL be replaceable by a plain binding.

#### Scenario: Change a pad
- **WHEN** Pad 8 in Hot cue mode is changed to Beat loop 4 on the left deck
- **THEN** pressing it starts a 4-beat loop on that deck

### Requirement: Map mode
Map mode SHALL tint the main window; clicking an on-screen control and then moving a hardware control SHALL bind them, with Learn staying armed, the last binding undoable, and a choice to replace or add when the hardware control is already used.

#### Scenario: Learn a knob
- **WHEN** Map mode is on, a filter knob is clicked and a controller knob turned
- **THEN** that knob now drives the filter

### Requirement: Quick map
For a connected MIDI controller with no mapping, DJ Sugar SHALL offer Quick map: it highlights the usual deck, mixer and library controls one at a time, binds each to the next hardware control the DJ moves, lets the DJ skip any, and saves the result as the DJ's own mapping.

#### Scenario: New controller
- **WHEN** an unmapped controller is plugged in and the DJ runs Quick map, moving a control for each step
- **THEN** a mapping named after the controller is in use, and every step not skipped works

#### Scenario: Which controller
- **WHEN** the DJ starts the Mapping wizard from the Map mode bar with more than one MIDI device plugged in
- **THEN** it asks which controller, with a button for each and a press on one picking it, and never offers the computer's own ports such as macOS's Network Session

### Requirement: Map mode reaches effects, pad modes and headphone cue
Map mode SHALL let the DJ map the FX bar's ON, D/W, second-control slider and effect stepping, each deck's pad mode tabs, and the headphone cue buttons.

#### Scenario: Pad mode tab
- **WHEN** the DJ clicks deck 1's BEAT JUMP tab in Map mode and presses a controller button
- **THEN** that button opens deck 1's third pad tab from then on
