## ADDED Requirements

### Requirement: Mapping format
DJ Sugar SHALL read and write controller mappings in the documented `.djsugar-mapping` format, a superset of Mixxx's `.midi.xml`, and an exported file SHALL carry the scripts it needs.

#### Scenario: Round trip
- **WHEN** a mapping is exported as `.djsugar-mapping` and imported on another computer
- **THEN** it has the same rows, lights and scripts

### Requirement: Import from other DJ apps
The Controller Editor SHALL import VirtualDJ, Traktor, rekordbox and djay Pro mappings, each file as a mapping of the DJ's own based on the controller's original. Rows it can't translate SHALL be kept greyed with the reason, the original binding SHALL keep working underneath, and a report SHALL say what came across.

#### Scenario: Xone K2 from VirtualDJ
- **WHEN** a DJ imports their VirtualDJ Xone K2 mapping
- **THEN** a new mapping lists its rows, its centre LEDs show the deck levels as in VirtualDJ, and the report lists the greyed rows

### Requirement: Export for Mixxx
A mapping SHALL export as a Mixxx `.midi.xml` with its script files, and the report SHALL list the rows Mixxx can't do.

#### Scenario: Script row left out
- **WHEN** a mapping with a `holding` row is exported for Mixxx
- **THEN** its plain bindings are written and the row is listed as left out

### Requirement: Keyboard shortcut sets
Settings › Keyboard SHALL pick the shortcut set in use from a menu, make a new set from the current one, rename and delete sets (not Default), and import a VirtualDJ keyboard mapping as a set.

#### Scenario: Import keys
- **WHEN** the DJ imports a VirtualDJ keyboard mapping
- **THEN** a new set named after the file is in use and the page says how many keys were left out

### Requirement: Two-deck and four-deck use
A mapping with a layout for two decks and one for four SHALL be set to one of them per controller in Settings › Controllers, with a deck order for four decks, and only the rows for that use SHALL run and be listed.

#### Scenario: K2 with four decks
- **WHEN** the DJ sets their imported Xone K2 mapping to 4 decks with deck order 3 1 2 4
- **THEN** the first column works deck 3 and the second deck 1, and the two-deck rows are gone from the editor
