# Proposal

Planned for: **v0.5 · Stems, performance and switching over** (2026-10-29)

## Why

Most DJs keep their playlists in a streaming service. Even without playing streamed audio, seeing those playlists and tracks inside DJ Sugar shows which ones are already in the collection, lets a DJ prepare from them, and is the starting point for buying what's missing. Spotify browsing is already built (sign-in, playlists, liked songs) but switched off with the stores; the other streaming sources are placeholders that say "Coming soon".

## What Changes

- Split `SERVICES_ENABLED` per source, so streaming sources go live in v0.5 while the stores and purchase inbox wait for v0.6 (now `add-stores-and-purchasing`, v0.8).
- **Spotify**: switch on the built browse view (sign-in, playlists, liked songs, tracks), with a clearer sign-in in Settings.
- **TIDAL**: sign in, playlists, favourite tracks, tracks.
- **Apple Music**: sign in, library playlists and songs (MusicKit with DJ Sugar's Apple developer account).
- **SoundCloud**: likes, playlists and the DJ's own uploads. Works today: app registration is self-serve with a SoundCloud Artist Pro subscription (checked 2026-10-29). Its API terms rule out deck playback (no modifying, no mixing with other sources' audio, no stem separation, session-only caching), so it stays browse only like the rest.
- **Mixcloud**: the DJ's shows, favourites and Listen Later. Works today with a free API app; the API gives no audio streams at all (checked 2026-10-29).
- **YouTube Music**: playlists where the API allows; Beatport's streaming playlists if partner access comes through.
- In every streaming list, tracks the collection already holds are marked and load the local file; the rest show greyed out and can't be loaded. Copy a streaming playlist into an DJ Sugar list (only the matched tracks, the rest kept as wanted).
- **Export to Spotify** (2026-11-01): send one of the DJ's lists, crates or a history session to Spotify as a new private playlist, in the list's order (a session as played). Tracks are found by Spotify id, then ISRC, then artist, title and length; those Spotify doesn't have are named. Writing playlists needs the `playlist-modify-private` scope, so a sign-in from before asks to sign in again. Nothing is played from Spotify.
- API access, per service (SoundCloud and Mixcloud first, since they need no approval): Spotify extended quota (until then the DJ supplies a client ID, as today), TIDAL developer app, Apple MusicKit key, SoundCloud app (Artist Pro), Mixcloud app, YouTube API app, Beatport partner request. Each source stays "Coming soon" until its access is in place.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `library-sources`: streaming sources go live, browse only, with matching to the collection

## Out of Scope

- Playing streamed audio of any kind, partner SDKs and DRM workarounds (still decided against).
- Buying missing tracks (`add-stores-and-purchasing`, v0.8).

## Impact

app browser (per-source switches; Mixcloud joins the rail sources), spotify.rs and one module per new service, track matching (artist, title, duration, ISRC when given), settings sign-ins.
