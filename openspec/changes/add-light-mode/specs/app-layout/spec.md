## ADDED Requirements

### Requirement: Appearance
Settings SHALL offer an Appearance of Dark, Light or Match system, Dark at first and saved. Light SHALL make the panels, library, buttons, menus and Settings light while the large waveforms and overviews keep dark wells and their colours. Match system SHALL follow the computer's light or dark setting, also when it changes while DJ Sugar runs. Text SHALL keep a contrast of at least 4.5:1 against its background in both appearances, so Electric Lilac used as text SHALL use a deeper lilac in Light. The native title bar and text fields SHALL follow the appearance. The karaoke screen SHALL stay dark.

#### Scenario: Light
- **WHEN** the DJ picks Light in Settings › Layouts
- **THEN** the window turns light at once, the deck waveforms stay on black, and DJ Sugar opens in Light next time

#### Scenario: Match system
- **WHEN** Match system is picked and the computer switches to dark mode at sunset
- **THEN** DJ Sugar turns dark without a restart

#### Scenario: Key marks
- **WHEN** the library shows a track whose key mixes with the track playing, in Light
- **THEN** its key is marked in the deeper lilac, readable on the white row
