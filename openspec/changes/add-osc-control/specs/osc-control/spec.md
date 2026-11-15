## Purpose

Letting tablets, visuals, lighting and show software control DJ Sugar and follow what it does, over Open Sound Control (UDP).

## ADDED Requirements

### Requirement: Off until switched on
OSC SHALL be off at first. When switched on in Settings › Controllers › OSC, DJ Sugar SHALL listen on the chosen UDP port (default 9000), only for this computer unless "Only this computer" is unticked.

#### Scenario: First launch
- **WHEN** DJ Sugar starts for the first time
- **THEN** no OSC port is open

### Requirement: Controls by address
Every control in supported-controls.txt SHALL have the address `/<group>/<key>`, where the group is the control's group lower-cased without brackets (`[Channel1],play` is `/channel1/play`). A message with one number SHALL set the control; a message with no arguments SHALL make DJ Sugar reply to the sender with the control's value. Unknown addresses SHALL be ignored. With "Send only" ticked, incoming messages other than subscriptions SHALL be ignored.

#### Scenario: Tablet plays a deck
- **WHEN** TouchOSC sends `/channel1/play 1`
- **THEN** deck 1 plays, exactly as if the PLAY button had been pressed

#### Scenario: Ask for a value
- **WHEN** a client sends `/master/crossfader` with no arguments
- **THEN** it receives `/master/crossfader` with the crossfader's position

### Requirement: Feedback
DJ Sugar SHALL send a control's new value to each send target (and each client that set, asked for or subscribed to it) when it changes, at most 30 times a second per control. `/subscribe` with an address pattern SHALL add matching controls for that client; `/unsubscribe` SHALL remove them.

#### Scenario: Tablet shows the fader
- **WHEN** a TouchOSC layout subscribed to `/channel1/*` and the DJ moves deck 1's fader on a MIDI controller
- **THEN** the tablet's fader moves with it

### Requirement: Events for visuals and lights
While on, DJ Sugar SHALL send to every send target: `/beat` with beat in bar, bar number and BPM on each beat of the master deck, timed for when it is heard; `/phrase` with the phrase name and length in bars when the master deck enters a new phrase; `/deck/N/track` with title, artist, BPM and key when a track loads; and `/deck/N/cue` with the cue number when a hot cue is triggered.

#### Scenario: Visuals on the drop
- **WHEN** Resolume listens for `/phrase` and the master deck reaches its chorus
- **THEN** Resolume receives `/phrase "Chorus" 16` on the chorus's first beat

### Requirement: Found on the network
While OSC is on and open to the network, DJ Sugar SHALL advertise it over Bonjour as `_osc._udp`.

#### Scenario: TouchOSC finds DJ Sugar
- **WHEN** the DJ opens TouchOSC's connection list on a tablet on the same Wi-Fi
- **THEN** DJ Sugar is listed
