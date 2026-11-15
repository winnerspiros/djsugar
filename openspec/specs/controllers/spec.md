# controllers Specification

## Purpose
MIDI controller support using Mixxx's own mappings unchanged, how controllers are found and shown, and how two-deck controllers reach decks 3 and 4.

## Requirements

### Requirement: Mixxx mappings run unchanged
DJ Sugar SHALL read Mixxx `.midi.xml` mappings and run their JavaScript in QuickJS against Mixxx's `engine` and `midi` API (soft takeover, timers, scratch and brake filters, MIDI out), using the same control names.

#### Scenario: Supported controller
- **WHEN** a controller Mixxx supports is plugged in
- **THEN** its mapping loads and it works with nothing to set up

### Requirement: Hot plugging
Plugging a controller in or out while DJ Sugar runs SHALL be picked up within about a second, each controller on its own thread, on both macOS and Linux. On macOS the MIDI device thread SHALL run its run loop so CoreMIDI can report device changes. LEDs SHALL be sent again 1.5 s after start.

#### Scenario: Plug in while running
- **WHEN** a DDJ-FLX4 is plugged in while DJ Sugar runs
- **THEN** it works within about a second, with nothing to press

#### Scenario: Unplug while running
- **WHEN** a running controller is unplugged
- **THEN** its mapping stops and it leaves Settings › Controllers within about a second

### Requirement: Find controllers again
Settings › Controllers SHALL have a Refresh button that closes every controller, reads the system's MIDI device list again and reconnects them, as a fallback when a controller does not show up or stops answering.

#### Scenario: Controller not showing
- **WHEN** a plugged-in controller is missing and Refresh is pressed
- **THEN** DJ Sugar looks for devices at once and starts the controller with its mapping

### Requirement: Controllers settings
Settings › Controllers SHALL list the connected controllers DJ Sugar has a mapping for, with the mapping in use, let one be set to Not used, and say when mappings are missing or a mapping failed. Other MIDI devices (virtual ports, apps, unmapped gear) SHALL stay off the list until picked from Add a controller. Remove SHALL take a controller off the list and stop using it until it is added again; both are kept by device key. Each controller SHALL have an Edit button that opens the Controller Editor on its mapping.

#### Scenario: Turn off a controller
- **WHEN** a controller is set to Not used
- **THEN** DJ Sugar ignores it

#### Scenario: Unmapped device
- **WHEN** a MIDI device with no mapping is plugged in
- **THEN** it is not listed, and shows under Add a controller

#### Scenario: Remove a controller
- **WHEN** Remove is pressed on a controller
- **THEN** it leaves the list, its mapping stops, and it stays off the list after a restart until added again

#### Scenario: Edit a controller
- **WHEN** Edit is pressed on a controller
- **THEN** the Controller Editor opens on the mapping that controller runs

### Requirement: Decks 3 and 4 on two-deck controllers
A mapping without decks 3 and 4 SHALL get `[Controller],deck3`/`deck4` switches. In 4-Deck, clicking deck 3's (or 4's) number SHALL move the left (or right) side there, LEDs included; leaving 4-Deck SHALL move both back.

#### Scenario: FLX4 on deck 3
- **WHEN** deck 3's number is clicked in 4-Deck
- **THEN** the FLX4's left side plays deck 3 and the number is lit

### Requirement: Controls list and docs
`djsugar --list-controls` SHALL print every control; the website SHALL have a page per mapped controller. `DJSUGAR_MIDI_DEBUG` SHALL log MIDI both ways.

#### Scenario: CI catches new controls
- **WHEN** a control is added without regenerating supported-controls.txt
- **THEN** CI fails

### Requirement: HID not supported
HID controllers SHALL NOT be supported yet, and the docs SHALL say so.

#### Scenario: HID controller
- **WHEN** a HID-only controller is plugged in
- **THEN** DJ Sugar does not use it
