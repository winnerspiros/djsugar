# app-layout Specification

## Purpose
The window: layouts, the top bar, the compact view, the Settings window and keyboard shortcuts.

## Requirements

### Requirement: Layouts
The layout menu at the top left SHALL offer Simple (decks 1 and 2 as one row each, library gets most of the screen), 2-Deck (full decks with pads and FX, mixer between) and 4-Deck (djay style, strips in the order 3, 1, 2, 4, each with EQ, filter, gain, fader, transport, loop, tempo, pads and FX). The choice SHALL be saved (`layout`).

#### Scenario: 4-Deck order
- **WHEN** 4-Deck is chosen
- **THEN** four strips show left to right as decks 3, 1, 2, 4

### Requirement: Deck section order
A 2-Deck deck SHALL place FX, loop and transport in that order, with PLAY on the left and CUE on the right.

#### Scenario: Transport
- **WHEN** a deck is shown in 2-Deck
- **THEN** PLAY is left of CUE

### Requirement: Top bar
The top bar SHALL hold the layout menu, WAVEFORMS, MIC, REC and MASTER (which opens the levels and Ableton Link), at 52 px high, with macOS's title bar hidden into it.

#### Scenario: REC lights
- **WHEN** a recording runs
- **THEN** REC is lit until the recording stops

### Requirement: Compact view
The button at the bottom left SHALL hide the decks in any layout.

#### Scenario: Compact
- **WHEN** the compact view button is pressed
- **THEN** the decks hide and the library fills the window

### Requirement: Settings window
Settings SHALL open with ⌘, (Ctrl+, on Linux) or from the macOS app menu, with pages Audio, Controllers, Library, Analysis, Decks, Mixer, Feedback and one per add-on with settings. Esc SHALL close it.

#### Scenario: Close with Esc
- **WHEN** Settings is open and Esc is pressed
- **THEN** it closes

### Requirement: Space plays
Space SHALL play or pause the master deck when no text field has focus.

#### Scenario: Space
- **WHEN** Space is pressed with deck 1 the master deck
- **THEN** deck 1 plays or pauses

### Requirement: Look
The UI SHALL use djay Pro's colours, widgets and fonts with VirtualDJ's placement, the DJ Sugar logo and Electric Lilac (#B69CFF) for accents.

#### Scenario: Accent colour
- **WHEN** a control is active
- **THEN** its accent is lilac unless it carries a deck or cue colour
