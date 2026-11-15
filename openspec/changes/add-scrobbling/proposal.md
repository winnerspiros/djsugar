# Proposal

Planned for: **v0.7 · Your library, everywhere** (from a gap review of Player mode against Spotify and TIDAL, a user request 2026-11-09; issue #536)

## Why

Spotify, TIDAL and most desktop players send what you play to Last.fm or ListenBrainz, so listening history and recommendations follow the listener from app to app. DJ Sugar keeps its own history but sends nothing out, so plays in DJ Sugar are missing from those profiles.

## What Changes

- **ListenBrainz first**: a **Send plays to ListenBrainz** switch in Settings › Broadcast (off at first) and the DJ's own user token. The track the audience hears (the same rule as the now-playing feed: the deck loudest in the master mix; headphone cueing never counts) goes as "playing now" after a few seconds and as a listen once half of it, or four minutes, has played. Tracks under 30 seconds never count.
- Listens that can't be sent wait in a file in the config folder and go in batches when ListenBrainz answers again.
- Settings shows who the token belongs to and how many plays were sent, or why sending failed.
- **Then Last.fm**, with the DJ's own API key and secret (DJ Sugar ships none), signed in through the browser once.

## Capabilities

### New Capabilities

- `scrobbling`: send the tracks heard to ListenBrainz and Last.fm

### Modified Capabilities

None.

## Out of Scope

- Loving or banning tracks on the services, or reading recommendations back.
- Scrobbling tracks heard only in the headphones or in the browser's preview player.

## Difficulty

**Small: two PRs.** ListenBrainz needs one POST with a token; Last.fm needs the signed auth flow and a session key, kept in the settings and out of backups.

## Performance

Nothing runs on the audio thread. The UI thread checks the audible deck once a second with values it already has; requests go on a thread of their own.
