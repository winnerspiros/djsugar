# Proposal

Planned for: **v0.5 · Stems, performance and switching over** (step 1; 2026-11-01, main use is Automix control)

## Why

DJs running Automix at a party, a wedding or a bar want to walk away from the laptop and still change what plays next, skip a track or turn the music down. Mixxx has had a request for a remote open since 2013; VirtualDJ sells a remote app; Serato's and djay's iPad remotes were dropped. A page DJ Sugar serves itself needs no app and is always the right version. Research: notes/remote-control-review.md in the project files.

## What Changes

- REMOTE in the top bar switches a phone remote on and off (off at first) and shows a QR code to pair a phone, a PIN for typing instead, and the paired phones with Remove.
- DJ Sugar serves a web page on the local network. A paired phone sees both decks (title, tempo, key, time left, the next mix shaded), Automix on/off and its status, Mix now and Skip, mix length, style and Order (with Undo), and the Automix list, which it can reorder and remove from.
- Master and booth levels, and Fade out / Bring the sound back.
- Search the collection from the phone (the browser's search syntax) and add tracks to Automix, at the end or next.
- A WebSocket API with the add-on message shape, for later steps and other tools.

## Capabilities

### New Capabilities

- `phone-remote`: control Automix and the main levels from a phone on the same network

### Modified Capabilities

None.

## Out of Scope

- Browsing crates and playlists from the phone, and a Bonjour advert (rest of step 2).
- Guest song requests (step 3), a now-playing page and public API docs (step 4), add-on panels (step 5).
- HTTPS, deck controls such as jog, pads or EQ, and audio to the phone.
- Access from outside the local network.

## Impact

New crate `crates/remote` (std threads, `tungstenite`, `qrcode`, `if-addrs`); app module `remote.rs`; `ui/remote.slint`; settings `remote_on` and `remote_devices`.
