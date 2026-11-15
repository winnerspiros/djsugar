## MODIFIED Requirements

### Requirement: Settings window
Settings SHALL open with ⌘, (Ctrl+, on Linux) or from the macOS app menu, with pages Audio, Controllers, Collection, Analysis, Decks, Mixer, Sources, Plugins, Broadcast, Keyboard, Help, Advanced and one per add-on with settings. Settings › Decks SHALL be split into Loading, Playing, and Tempo and key. Changes SHALL apply at once. Esc SHALL close it.

#### Scenario: Close with Esc
- **WHEN** Settings is open and Esc is pressed
- **THEN** it closes

## ADDED Requirements

### Requirement: Advanced settings
Settings › Advanced SHALL list every setting DJ Sugar has, including those on the other pages, as one searchable list. Each row SHALL show a plain-English name, its stable key in small grey text, and its value; its help SHALL be in an ⓘ popover as on the other pages.

#### Scenario: Find a tweak
- **WHEN** the DJ opens Advanced and types "tails"
- **THEN** the effect tails row is shown with its description

### Requirement: Layout is not a setting
Window and panel sizes, list columns and sort, the last folder and waveform zoom SHALL be remembered without appearing in Settings; View › Reset layout SHALL put them back to the defaults. Help SHALL offer Show all hints again.

#### Scenario: Reset layout
- **WHEN** the DJ has resized the browser and hidden columns and picks View › Reset layout
- **THEN** sizes and columns return to their defaults, and no setting in Settings changes
