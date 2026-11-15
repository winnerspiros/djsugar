## Purpose

Letting a DJ control Automix and the main levels from a phone on the same network, by opening a page DJ Sugar serves, without installing an app.

## ADDED Requirements

### Requirement: Off until switched on
The remote SHALL not listen on the network until the DJ switches it on under REMOTE in the top bar. It SHALL stay on across restarts only if it was on when DJ Sugar quit.

#### Scenario: First launch
- **WHEN** DJ Sugar starts for the first time
- **THEN** REMOTE is off and nothing listens on the network

### Requirement: Pairing
A phone SHALL pair by scanning the QR code under REMOTE, which holds a code that works once and for two minutes, or by typing the 6-digit PIN shown there. After five wrong codes or PINs in a row, pairing SHALL stop for a minute. A paired phone SHALL reconnect without pairing again until the DJ removes it.

#### Scenario: Scan to pair
- **WHEN** the DJ scans the QR code with a phone on the same Wi-Fi
- **THEN** the phone opens the remote, appears under Paired phones as connected, and REMOTE turns green

#### Scenario: Removed phone
- **WHEN** the DJ clicks Remove beside a phone
- **THEN** the phone is disconnected and asked to pair again

### Requirement: Other pages kept out
The remote SHALL refuse requests whose `Host` is not an address or `.local` name of the computer, and WebSocket connections opened by a page from another origin.

#### Scenario: A web page tries to connect
- **WHEN** a web page on another site tries to open the remote's WebSocket
- **THEN** the connection is refused

### Requirement: Automix on the phone
A paired phone SHALL open on a now-playing screen: a large cover of the track you hear, its title, artist, deck, tempo and key, a progress bar with the next mix shaded, Automix's status line, and the next track with the time left to its mix. Its controls SHALL be, left to right, Order, Mix settings (length, style and Skip the next track), play/pause in the centre, Mix now and the sleep timer. A master volume slider SHALL sit under the controls, styled apart from the progress bar (labelled, lilac, in its own box). Pause SHALL pause the decks Automix is playing and switch Automix off; Play SHALL resume them and switch Automix back on (or start Automix when nothing was paused). The phone SHALL NOT switch between decks: it shows one track, the one you hear.

#### Scenario: Pause from the phone
- **WHEN** the DJ taps pause while Automix plays deck 2
- **THEN** deck 2 pauses, Automix stops, and tapping play resumes deck 2 and Automix together

### Requirement: Queue on the phone
A paired phone SHALL show the Automix list as a queue with a small cover on every track, the track playing at the top, and Mix, Order, Levels and Timer at the bottom. Tracks SHALL move by dragging or from a track's menu (Play next, Move to the end, Remove), and in Edit, where several tracks are picked at once for Play next, Move to the end or Remove. The track waiting on a deck SHALL stay movable until 15 seconds before its mix starts; from then, and during the mix, it SHALL show a lock and stay put, as SHALL the track playing.

#### Scenario: Swap the waiting track
- **WHEN** the next mix is a minute away and the DJ drags another track above the waiting one
- **THEN** that track mixes in next instead

#### Scenario: Too close to the mix
- **WHEN** the next mix is 10 seconds away
- **THEN** the waiting track shows a lock and can't be moved or removed

### Requirement: Album art on the phone
DJ Sugar SHALL serve covers to paired phones only, at two sizes (list and now playing), through addresses that carry a key given out at pairing and that changes each time the remote starts.

#### Scenario: A stranger asks for a cover
- **WHEN** a device without the key requests a cover address
- **THEN** DJ Sugar answers 404

### Requirement: Sleep timer
A paired phone SHALL set a sleep timer (15, 30, 45, 60 or 90 minutes) after which the master level fades out over 20 seconds and Automix stops, and SHALL show the time left on the queue's Timer button.

#### Scenario: Timer runs out
- **WHEN** a 15-minute timer is set and 15 minutes pass
- **THEN** the music fades out, Automix stops, and the master level is put back for next time

### Requirement: Levels on the phone
A paired phone SHALL set the master level (`[Master],gain`) and, when a booth output is set up, the booth level (`[Master],booth_gain`), and fade the master level out over eight seconds and bring it back.

#### Scenario: Fade out
- **WHEN** the DJ taps Fade out
- **THEN** the master level slides to silence over eight seconds, and Bring the sound back restores it

### Requirement: Search and add from the phone
A paired phone SHALL search the collection with the same search syntax as the browser (for example `bpm:120-124` or `crate:Warm-up`), show results sorted by title with how many were found, 100 at a time with more fetched as the DJ scrolls (an empty search lists the whole collection), and add a result to the end of the Automix list or to play next (right below the tracks Automix holds). Tracks already on the list SHALL be marked. The search SHALL open full screen from the queue and close with Done, which also clears it.

#### Scenario: Add a request
- **WHEN** the DJ searches "innerbloom" on the phone and taps Next on the result
- **THEN** the track goes on the Automix list right after the waiting track, in the app too
