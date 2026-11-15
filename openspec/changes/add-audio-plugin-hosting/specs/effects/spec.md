## ADDED Requirements

### Requirement: Plugin effects
Any effect slot (deck FX, Beat FX, pad FX and the microphone's unit, so on decks, the main mix, Mic and Aux) SHALL load an installed CLAP, VST3 or (macOS) AU effect. Instrument plugins SHALL be listed as not supported.

#### Scenario: Plugin in a deck FX slot
- **WHEN** the DJ picks an enabled plugin in a deck FX slot and turns it on
- **THEN** the deck is heard through the plugin, with ON, D/W and the slot's slider working as for built-in effects

#### Scenario: Plugin on the mic
- **WHEN** the DJ picks an enabled plugin in a slot of the microphone's effect unit
- **THEN** the microphone and aux inputs are heard through it, and the decks are not

### Requirement: Out-of-process plugins
Every plugin SHALL run in an `djsugar-plugin-host` helper process, never in DJ Sugar's own process. The audio thread SHALL hand each block to the helper and wait no longer than a deadline; a block that is late, or whose helper is gone, SHALL be played dry.

#### Scenario: Crash
- **WHEN** a plugin crashes
- **THEN** its slot goes dry, the decks keep playing, and the slot says the plugin crashed

#### Scenario: Hang
- **WHEN** a plugin stops returning blocks
- **THEN** every block is played dry within the deadline, and after a few missed blocks the helper is killed as if it had crashed

### Requirement: Crash recovery
A crashed or hung plugin SHALL be restarted bypassed, with its last saved state, and SHALL NOT turn itself back on. After two crashes in a session it SHALL be disabled until the DJ re-enables it in Settings, and a plugin that crashed on load SHALL NOT be loaded at the next startup.

#### Scenario: Second crash
- **WHEN** the same plugin crashes a second time in a session
- **THEN** it is disabled and its slot stays dry until the DJ enables it again

### Requirement: Plugin scanning
DJ Sugar SHALL NOT scan plugins at startup. Scanning SHALL run when the DJ presses Scan in Settings › Plugins, or in the background when a plugin folder changes, each plugin in its own scanner process with a timeout, with results cached by path, size and modification time. A plugin that fails to scan SHALL be listed with the reason and a Retry button.

#### Scenario: Many plugins
- **WHEN** 500 unchanged plugins are installed and DJ Sugar starts
- **THEN** startup takes no longer than without them

### Requirement: Plugin allow-list
Only plugins the DJ has switched on in Settings › Plugins SHALL appear in the FX chooser. A fresh install SHALL have none switched on. A plugin's helper SHALL start only when the plugin is first put in a slot, or in the background after the window shows for plugins already in saved slots.

#### Scenario: Chooser
- **WHEN** 400 plugins are installed and 6 are switched on
- **THEN** the FX chooser lists the built-in effects and those 6

### Requirement: Plugin CPU budget
Each plugin slot SHALL show the plugin's load. A plugin that stays over its CPU budget (default 30% of the block time) SHALL be switched off with a message saying why.

#### Scenario: Heavy plugin
- **WHEN** a plugin keeps using more than its budget
- **THEN** it is bypassed and the slot says it was too slow

### Requirement: Plugin latency
A plugin's reported latency SHALL be compensated by reading its deck ahead by the same amount, so the deck stays on the beat. Plugins reporting more than 20 ms SHALL be marked as high latency in the chooser.

#### Scenario: Lookahead plugin
- **WHEN** a plugin with 10 ms latency is loaded on deck 1 while deck 2 is synced to it
- **THEN** the two decks' beats still line up at the master output

### Requirement: Plugin editors
A plugin's own editor SHALL open in a separate window owned by its helper process, never inside DJ Sugar's window.

#### Scenario: Open editor
- **WHEN** the DJ opens a plugin's editor
- **THEN** it appears in its own window and the waveforms keep running smoothly
