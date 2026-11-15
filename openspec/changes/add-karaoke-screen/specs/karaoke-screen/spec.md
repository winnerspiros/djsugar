## Purpose

Showing the words of the track playing on a screen of their own, for singing along, with the singer's voice turned down.

## ADDED Requirements

### Requirement: Karaoke screen
DJ Sugar SHALL offer a karaoke screen, a window of its own, that follows the track playing and shows the line to sing large, the line before it faded and the next two lines. When another display is connected it SHALL open full screen there.

#### Scenario: Opening on a second display
- **WHEN** a second display is connected and the DJ presses Karaoke in the Lyrics tab
- **THEN** the karaoke screen opens full screen on the second display

### Requirement: Word fill
The word being sung SHALL fill with the accent color from left to right as it is sung. When lyrics are timed only by line, the line's words SHALL be spread over it by their length, ending by the next line's start.

#### Scenario: Filling a timed word
- **WHEN** a line reads `[00:10.00]<00:10.00>Hold <00:10.40>the <00:10.70>line` and the deck is at 10.5 s
- **THEN** "Hold" is filled and "the" is a third filled

### Requirement: Count-in
Before a line that follows a break, the screen SHALL show the line to come with up to four dots, one per beat left at the deck's tempo.

#### Scenario: Counting in
- **WHEN** the next line starts in two seconds and the deck plays at 120 BPM
- **THEN** four dots show above the line

### Requirement: Guide vocal
While the karaoke screen is open, the Lyrics tab SHALL have a Vocals slider setting the vocal stem's level on the deck followed, and SHALL hold the level set there on the next track the screen follows.

#### Scenario: Next track
- **WHEN** the DJ sets Vocals to 0 and Automix moves to the next track
- **THEN** the next track plays with its vocal stem muted

### Requirement: Karaoke page
While the karaoke screen is open and the DJ shares it, DJ Sugar SHALL serve the same screen as a web page at `/karaoke` on its local web server, reachable from other devices on the network, and SHALL show the page's address in the Lyrics tab. The page SHALL keep time itself between updates, so the word fill stays smooth.

#### Scenario: Opening on a TV
- **WHEN** the DJ presses Share in the Lyrics tab and opens the address shown in a TV's browser
- **THEN** the TV shows the line being sung, filling as on the karaoke screen

#### Scenario: Closing
- **WHEN** the DJ closes the karaoke screen
- **THEN** the page stops being served on the network, unless the now-playing overlay is set to allow other devices
