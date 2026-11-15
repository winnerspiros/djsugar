## ADDED Requirements

### Requirement: Your layouts
DJ Sugar SHALL keep the DJ's layouts as an ordered list (`layouts`), at first Player, 2-Deck, 2-Deck Pro and 4-Deck. The layout menu at the top left SHALL list only these, in this order. The layout in use SHALL always be in the list. Layouts added to DJ Sugar later SHALL NOT join the list by themselves.

#### Scenario: A layout taken out
- **WHEN** the DJ takes 2-Deck Pro out of their layouts
- **THEN** the layout menu lists Player, 2-Deck and 4-Deck only

#### Scenario: The layout in use
- **WHEN** the DJ is on 2-Deck
- **THEN** Settings › Layouts offers no way to take 2-Deck out

### Requirement: Settings › Layouts
Settings SHALL have a Layouts page: Your layouts (each with a preview, up and down to reorder, and − to take it out) and More layouts (a card with a preview, a line on what it is for and + Add for each layout not in the list). Clicking a row or card SHALL switch to that layout. A cog at the top right of the layout menu SHALL open this page.

#### Scenario: Adding a layout
- **WHEN** the DJ presses + Add on 4-Deck under More layouts
- **THEN** 4-Deck goes to the end of Your layouts and the layout menu lists it last

### Requirement: VIEW steps through your layouts
`[Skin],layout`'s VIEW press on a controller SHALL switch to the next layout in the DJ's list, after the last going back to the first.

#### Scenario: Two layouts
- **WHEN** the DJ's layouts are Player and 2-Deck, 2-Deck is showing, and VIEW is pressed
- **THEN** Player shows
