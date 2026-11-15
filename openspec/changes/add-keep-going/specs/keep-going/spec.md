## ADDED Requirements

### Requirement: Keep going switch
In Player mode, Up Next's header SHALL offer a Keep going switch, off at first. While it is on and Automix plays, DJ Sugar SHALL add the next pick to the end of Up Next whenever fewer than two tracks wait. Switching it on SHALL switch Automix on. Outside Player mode it SHALL add nothing.

#### Scenario: The list runs out
- **WHEN** Keep going is on in Player mode and the last track the DJ queued starts
- **THEN** the next pick goes on Up Next and Automix mixes into it when the track ends

#### Scenario: Off at first
- **WHEN** the DJ has never switched Keep going on and Up Next runs out
- **THEN** playback stops after the last track, as before

### Requirement: Picks that mix
Each pick SHALL come from the pool (everything, a list, or an artist and their genres) and SHALL be among the tracks that mix best with the track before it: fewest Camelot steps, then the smallest tempo change counting half and double time. Tracks played in the last 3 hours, on a deck, in Up Next or already picked SHALL be left out, and the same artist twice in a row SHALL be avoided where another track fits.

#### Scenario: Harmonic pick
- **WHEN** Up Next ends on a track in 8A at 124 BPM and the pool holds tracks in 8A at 123 BPM and 3B at 140 BPM
- **THEN** the 8A track is picked

### Requirement: Picks shown ahead
While Keep going is on, Up Next SHALL show the next 3 picks greyed after its tracks; they SHALL NOT play until they are added to the list.

#### Scenario: Greyed picks
- **WHEN** Keep going is switched on with two tracks in Up Next
- **THEN** three greyed picks show under them

### Requirement: Start Radio
In Player mode, the track menu SHALL offer Start Radio for the track and for its artist, and the list menu SHALL offer Start Radio for a crate or playlist. Each SHALL switch Keep going and Automix on with that seed: a track goes next in Up Next (or starts if nothing plays) with picks from everything; an artist's radio picks from their tracks and those sharing their genres; a list's radio picks from that list.

#### Scenario: Crate radio
- **WHEN** the DJ right-clicks the crate "Sunday Morning" and picks Start Radio with nothing playing
- **THEN** a track from that crate starts and the next picks come from that crate
