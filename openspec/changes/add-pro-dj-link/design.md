# Design

## Protocol pieces used

| Port | Packet | DJ Sugar |
| --- | --- | --- |
| UDP 50000 | keep-alive (0x06), number claim | Sent while joined (Follow, Lead, show tools); not in Watch |
| UDP 50002 | CDJ status (0x0a), every 200 ms, unicast to each peer | Read always; sent while joined |
| UDP 50001 | beat (0x28); master takeover request (0x26) and response (0x27); sync commands (0x2a) | Read always; hand-off and beats sent in Lead |
| TCP 12523, then the advertised port | dbserver | Queried in Watch for players' tracks; answered for DJ Sugar's own decks (show tools) |
| UDP 50002 from a DJM | mixer status: on air, fader start | Read |

Layouts follow djl-analysis and Beat Link; nothing is copied from either.

## Facts from real units (Deckbench notes) to honour

- Status needs the header after the name (0x1f-0x24 with the length that follows), the master-valid word 0x90 = 0x8000, flag bits 7 and 2 always set, and byte 0xb6 = 1.
- A unit sends its status to a player's own address, so the listener binds to that address, not to any address.
- BPM (0x92) is the track's; pitch (0x8c, 0x100000 = none) is the change. Tempo heard = BPM × pitch.
- A unit answers a takeover request and names the requester in 0x9f only if the requester's status says it is synced; cancel after 3 s and log it.
- Requests can be 0x28 bytes; don't drop packets shorter than a status.
- Old players (NXS2, XDJ-1000MK2) answer dbserver and NFS only to numbers 1 to 4. DJ Sugar uses 5 and above, so Watch may get no metadata from them; show the track id then, not an error.

## Player number

Auto picks the lowest free number from 5 upward after listening 5 s (CDJ-3000 networks use 1 to 6, so 7 and above when 5 and 6 are taken). Numbers 1 to 4 are never taken: they belong to the players. Watch sends nothing and needs no number.

## Library choice

Start on `prodjlink-rs` (MIT, MSRV 1.75, no tokio) for parsing and joining, or a small in-tree codec if its API does not fit Lead; keep it behind `djsugar-prodjlink`'s own trait. `prolink` (GPL-3.0-only, tokio) is the candidate for the later serving step. Both are young single-maintainer crates, so pin versions.

## Testing

- Unit tests: packet encode/decode against fixtures taken from Deckbench output and djl-analysis examples.
- Integration (Linux CI, `test` job or a separate job): start a pinned Deckbench release with `--numbers 2,3 --on`, then check Watch sees both, Follow tracks a tempo change within two beats with bars aligned, Lead takes and returns the master with Deckbench's scenario expectations, and the dbserver answers Deckbench's metadata request. Deckbench needs only Python 3.8+.
- macOS: Deckbench needs a bridge interface rather than loopback there, so CI covers Linux; macOS is checked by hand.
- Hardware: Beta until a user confirms Follow and Lead on CDJs.
