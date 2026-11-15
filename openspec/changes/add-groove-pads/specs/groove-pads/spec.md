## Purpose

Driving the Groove Box from a pad controller: entering steps, playing and recording the sounds live, and picking grooves and kits, the same way on every controller.

## ADDED Requirements

### Requirement: One pad surface for every controller
Each Groove Box SHALL have controls `[GrooveN],pad1` to `pad16` (numbered as the controller prints them, on a Maschine left to right from the bottom row; the value is how hard the pad is pressed, 0 when released) and `[GrooveN],pad1_color` to `pad16_color` (what the pad shows). A controller mapping SHALL only pass presses in and turn colours into the device's own. A press and release between two audio buffers SHALL still count as a press.

#### Scenario: Quick tap
- **WHEN** a mapping sets `pad5` to 1 and back to 0 within a millisecond
- **THEN** the Groove Box acts on one press of pad 5

### Requirement: Steps mode
With `pad_mode` 0, pad K SHALL add or remove step K of the selected row (`row_selected`) of the groove on screen, and the pads SHALL show that row's hits in the row's colour and the step playing. While stopped, adding a hit SHALL play the sound once.

#### Scenario: Add a hat
- **WHEN** row 3 is selected and pad 3 is pressed
- **THEN** step 3 of row 3 plays from the next time round, and pad 3 lights in row 3's colour

### Requirement: Play mode with recording
With `pad_mode` 1, pads 1-8 (and again 9-16) SHALL play rows 1-8 at once (row 9, Ride, is edited in Steps mode), louder the harder it is hit. With `record` on while the Groove Box plays, the hit SHALL also be written into the nearest step of the groove playing, and that step SHALL NOT play the sound again on this pass.

#### Scenario: Record a kick
- **WHEN** REC is on and pad 1 is hit just before step 3
- **THEN** the kick sounds once now and from then on plays on step 3

### Requirement: Grooves mode
With `pad_mode` 2, pads 1-4 SHALL pick groove A-D (switching on the next bar), pads 5-8 the kit, and pads 9-16 SHALL load the first eight grooves of the genre picked into the groove playing. The groove playing, the one waiting and the kit SHALL be lit.

#### Scenario: Switch groove
- **WHEN** pad 2 is pressed mid-bar
- **THEN** pad 2 shows waiting, groove B starts on the next bar, and pad 2 then shows playing

### Requirement: Maschine Mikro MK3
DJ Sugar SHALL drive a Maschine Mikro MK3 over USB HID without NI software: pads with pressure, STEP / PAD MODE / PATTERN for the modes, LEFT and RIGHT for the row, PLAY, STOP, RESTART, REC, ERASE, MUTE and SOLO, the encoder and touch strip for the row's level, pitch and decay and the swing and tempo, the button and pad lights, and the screen showing deck, kit, groove, tempo, row and mode.

#### Scenario: Plug in
- **WHEN** a Mikro MK3 is plugged in and deck 2 is a Groove Box
- **THEN** its screen shows deck 2's kit and groove and its pads show the selected row's steps
