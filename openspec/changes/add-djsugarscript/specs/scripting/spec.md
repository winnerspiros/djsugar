## ADDED Requirements

### Requirement: DJSugarScript lines
DJ Sugar SHALL run one-line actions made of verbs, an optional `deck 1`-`deck 4` or `deck master` in front, `&` chains, `condition ? then : otherwise` and parentheses. A verb with no deck acts on the master deck. In a condition a verb is asked, not done.

#### Scenario: Toggle play on a deck
- **WHEN** `deck 1 play ? pause : play` runs while deck 1 is stopped
- **THEN** deck 1 starts, and running it again stops it

#### Scenario: A mistake is reported before anything runs
- **WHEN** the line is `rating 6 & play`
- **THEN** nothing runs and the error says the number of stars goes from 0 to 5, pointing at `6`

### Requirement: Keyboard shortcuts run DJSugarScript
Every key except Space, ⌘, and Esc SHALL look up a shortcut: the DJ's own for that key from settings `keyboard_shortcuts`, else the default. An empty script turns a key off.

#### Scenario: Rate with the keyboard
- **WHEN** two tracks are picked in the browser and the DJ presses ⌘4 (Ctrl+4 on Linux)
- **THEN** both tracks get 4 stars, saved to the library
