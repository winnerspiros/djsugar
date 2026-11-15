## ADDED Requirements

### Requirement: What counts as a play
With scrobbling on, the track the audience hears (the deck loudest in the master mix) SHALL be sent as playing now after 5 seconds, and as a listen, stamped with when it started being heard, once it has been heard for half its length or four minutes, whichever is less. A track under 30 seconds SHALL never count, and a deck heard only in the headphones SHALL never count.

#### Scenario: A three-minute track
- **WHEN** a 3:00 track has been the loudest deck for 1:30
- **THEN** a listen is sent to ListenBrainz with the time it started

#### Scenario: Cueing
- **WHEN** the DJ cues a track in the headphones with its channel fader down
- **THEN** nothing is sent for it

### Requirement: Sending later
A listen that can't be sent SHALL be kept in a file and sent, in batches, once the service answers again, also after a restart.

#### Scenario: Offline set
- **WHEN** the DJ plays a set without a connection and then reconnects
- **THEN** the set's listens reach ListenBrainz with the times they were heard

### Requirement: ListenBrainz settings
Settings › Broadcast SHALL have a switch to send plays to ListenBrainz (off at first) and a field for the DJ's user token, kept out of backups, and SHALL show who the token belongs to and how many plays were sent, or that the token was refused.

#### Scenario: A wrong token
- **WHEN** the DJ pastes a token ListenBrainz refuses
- **THEN** Settings says the token was not accepted

### Requirement: Last.fm
Settings › Broadcast SHALL have a switch to send the same plays to Last.fm (off at first), fields for the DJ's own API key and shared secret (DJ Sugar ships none), and a Sign in button that opens Last.fm in the browser and keeps the session it returns out of backups. Plays SHALL be sent as now playing and as scrobbles by the same rule as ListenBrainz, and SHALL wait in a file when Last.fm can't be reached.

#### Scenario: Signing in
- **WHEN** the DJ pastes their key and secret, presses Sign in and allows DJ Sugar on last.fm
- **THEN** Settings says "Signed in as" their Last.fm name, and the next track heard scrobbles
