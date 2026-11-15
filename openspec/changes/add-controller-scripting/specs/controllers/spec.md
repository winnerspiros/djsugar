## ADDED Requirements

### Requirement: DJSugarScript rows in the Controller Editor
The Controller Editor SHALL let the DJ give a control an DJSugarScript line instead of a function, checking the line as it is typed and saving it to the DJ's own mapping only when it parses.

#### Scenario: Conditional line
- **WHEN** the DJ picks "An DJSugarScript line" for PLAY and enters `var 'shift' ? sync : play_pause`
- **THEN** PLAY syncs while the shift variable is on and plays or pauses otherwise

#### Scenario: Mistake
- **WHEN** the line does not parse
- **THEN** the parser's message shows under it and the mapping is not changed

### Requirement: Modifiers and layers
The Controller Editor SHALL let the DJ make a button a shift (a variable on while held) or a mode button (a variable flipped on each press), and give any row an "Only when" condition on the mapping's variables, moving or copying it to that layer.

#### Scenario: Own shift
- **WHEN** the DJ makes a button "Shift: on while held" and copies CUE's row to Only when `shift`, then changes that copy to Sync
- **THEN** CUE cues, and holding the shift button while pressing CUE syncs
