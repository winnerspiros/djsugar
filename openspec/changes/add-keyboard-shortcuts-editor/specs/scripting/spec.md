## ADDED Requirements

### Requirement: Keyboard shortcuts editor
Settings SHALL have a Keyboard page listing every shortcut with its key and DJSugarScript line, where the DJ changes a key by pressing it, edits or picks the action, resets or turns off a default, removes their own, adds new ones, and resets all. A line that doesn't parse SHALL show its mistake while it's typed.

#### Scenario: Move a rating to another key
- **WHEN** the DJ clicks ⌘1's key on the Keyboard page and presses F1
- **THEN** F1 rates the picked tracks 1 star, ⌘1 does nothing, and ⌘1's row offers Reset

#### Scenario: Taking a key that's in use
- **WHEN** the DJ moves a shortcut onto ⌘2
- **THEN** ⌘2 runs the moved action, and the page says ⌘2 ran “rating 2” before

#### Scenario: Fixed keys
- **WHEN** the DJ presses Space or ⌘, on a key button
- **THEN** nothing changes and the page says that key can't be a shortcut
