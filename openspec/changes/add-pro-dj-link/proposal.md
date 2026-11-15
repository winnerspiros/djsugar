# Proposal

Planned for: **v0.9** (2026-11-13, a user request after a Pro DJ Link review; issue #674)

## Why

Pro DJ Link is the Ethernet protocol that ties Pioneer (AlphaTheta) CDJs, XDJs and DJM mixers together. Clubs run it everywhere: players sync beats over it, rekordbox serves tracks over it, and show tools (PRO DJ LINK Bridge, ShowKontrol, Beat Link Trigger, SoundSwitch, lighting desks, VJ apps) listen to it to follow the decks. No mainstream DJ app besides rekordbox joins it. A laptop running DJ Sugar in a b2b with CDJs, or DJ Sugar's Groove Box and instruments played next to them, today has no way to stay in time with the players except by ear.

The protocol is unpublished but well analysed ([djl-analysis](https://djl-analysis.deepsymmetry.org/)), and two Rust crates now speak it (`prodjlink-rs`, MIT; `prolink`, GPL-3.0-only). There is no Pioneer player to test on, but [Deckbench](https://github.com/commit-collective/deckbench) (MIT or Apache-2.0, Python with no dependencies) runs simulated CDJs and a DJM on loopback with an HTTP API, so every step here can be tested in Linux CI. Its notes also record what real units insist on (master-valid word, flag bit 7, unicast status, player numbers), which are the traps an implementation hits.

## What Changes

- **Settings › Controllers › Pro DJ Link**, off by default: a switch, the network interface, the player number (auto picks 5 or above), Lead allowed, and a list of players and mixers found with their model, number and what they have loaded. Marked Beta.
- **Watch:** with the switch on, DJ Sugar listens without sending anything: every CDJ's loaded track (title, artist, artwork, from its dbserver where it answers), tempo, pitch, play state, master and on-air flags, and beats. The tempo master's beats drive OSC `/beat`; each player's track drives OSC `/deck/N/track`-style events under `/prodjlink/N/...`, the now-playing feed (as an option) and History (tracks played on the players are logged as such).
- **Follow:** `[Master],sync_leader_source` gains **Pro DJ Link**. Decks with SYNC on follow the network's tempo master in tempo, beat and bar, smoothed the way MIDI clock in is. DJ Sugar joins as a virtual player for this (status and keep-alives) so players treat it as linked, using a number they don't need.
- **Lead:** with Lead allowed, DJ Sugar's master deck can become the network's tempo master and CDJs set to SYNC follow it. DJ Sugar takes the role only through the hand-off the players use (request, accept, successor named in status) and hands it back the same way, so there are never two masters.
- **Show tools:** DJ Sugar answers metadata queries for its own decks' tracks (title, artist, album, BPM, key, artwork, duration) on a small dbserver, so tools that follow CDJs show DJ Sugar's tracks too.
- **DJM:** on-air flags and fader start from a DJM mixer are read; a deck routed to a mixer channel (external mixer mode) can start on fader start.

## Capabilities

### New Capabilities

- `pro-dj-link`: join a Pro DJ Link network as a virtual player; watch players, follow and lead the tempo master, be visible to show tools, read the DJM

### Modified Capabilities

- `sync`: the sync leader can also be the Pro DJ Link tempo master

## Out of Scope

- Serving DJ Sugar's library to CDJs over the network (rekordbox Link Export's job: NFS and dbserver over a rekordbox medium). It needs a real player to test, so it waits until the USB export (add-pioneer-usb-export) has played on hardware. The `prolink` crate and the export in `crates/export` are the planned base.
- Loading a track from a CDJ's USB or SD into an DJ Sugar deck.
- Taking a player number from 1 to 4, or a number assigned by a mixer's channel port.
- StagelinQ (Denon), TCNet (ShowKontrol), and any bridging between protocols beyond following one leader.
- Waveforms, grids and cues read from players.

## Difficulty

**Medium-large: about 2 weeks, five PRs** (rig and Watch, Follow, Lead, show tools, DJM). Watch and Follow are UDP parsing and the same smoothing MIDI clock in already has. Lead is a small state machine but the riskiest part, because a wrong hand-off leaves two masters on a live network; Deckbench's DJS-1000 findings and its scenarios cover it. dbserver answering is the largest single piece.

## Performance

Everything runs on a Pro DJ Link thread of its own, with nothing new on the audio thread beyond reading one more leader snapshot through the lock-free path Link and MIDI clock in already use. Status goes out every 200 ms, beats once a beat; that is a few packets a second. With the switch off, no socket is opened.

## Impact

new `crates/prodjlink` (djsugar-prodjlink: discovery, status, beats, hand-off, dbserver answers; behind a trait so the protocol crate underneath can change), engine (`LeaderSource::ProDjLink`, leader snapshot), osc (events from players), remote (now-playing option), library (History rows for player tracks), app (Settings › Controllers › Pro DJ Link, leader choice), controls `[ProDjLink],enabled`, `[ProDjLink],lead`, supported-controls.txt, CI (Deckbench integration tests on Linux), user guide (Pro DJ Link page). Research: the doc "Pro DJ Link for DJ Sugar" in the project notes.
