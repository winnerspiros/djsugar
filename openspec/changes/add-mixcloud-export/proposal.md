# Proposal

Planned for: **v0.5 · Stems, performance and switching over** (2026-11-03, from a user request)

## Why

DJs record a set to put it on Mixcloud. Mixcloud wants a tracklist with start times; typing it in by hand after a two-hour set is the step most DJs skip, and Mixcloud ranks and licenses shows by it. DJ Sugar already knows exactly when each track became audible in a recording (the recorder's `.cue` sheet), so it can hand Mixcloud a finished show. VirtualDJ and Mixxx write a `.cue` file and stop there; rekordbox makes the DJ type the tracklist in.

Separately, a new recording only appeared in the Mixed Sets source after the next library scan, which made it look as if recording had failed.

## What Changes

- A recording joins the Mixed Sets source as soon as it stops.
- The recorder writes `<name>.tracklist.txt` beside the `.cue`: `0:00 Artist - Title` lines, ready to paste into a show's description.
- **File › Export › Mixcloud…** (macOS menu bar; was on the track menu at first) makes `<name> (Mixcloud).mp3`: 320 kbps, all parts of a split recording joined, with an ID3v2.4 tag naming the mix and carrying the tracklist as chapters (`CHAP`/`CTOC`) and as a comment. Long recordings are streamed, never held in memory. The file is shown in Finder when done.
- Step 2: **Upload to Mixcloud**, the only Mixcloud line on a mixed set's track menu. The DJ signs in to their own Mixcloud account in the browser (OAuth); DJ Sugar uploads the show with its name, description, up to five tags, cover and the tracklist as `sections-N-artist/song/start_time`, checking Mixcloud's limits (4 GB, 1000-character description) before sending. Mixcloud's token exchange needs the app's client secret and offers no desktop flow, so the secret lives on github.com/winnerspiros/djsugar: a Cloudflare Pages function swaps the code for a token and hands it to the app on 127.0.0.1 (decided 2026-11-03). The first version uploads the show private and opens it on mixcloud.com to name, tag and publish; an upload dialog in the app follows.

## Capabilities

### New Capabilities

- `mixcloud-export`: a recorded mix as a Mixcloud-ready show with its tracklist

### Modified Capabilities

None.

## Out of Scope

- Mixcloud Live (RTMP), which belongs to add-live-broadcast step 3.
- Tracklists for mixed sets DJ Sugar didn't record (no `.cue`): they export without chapters.
- SoundCloud and YouTube uploads.

## Difficulty

**Small: 2 to 3 days, two PRs.** Step 1 reuses the recorder's cue sheet and the broadcast crate's LAME encoder. Step 2 is OAuth plus one multipart POST, and the secret's home.

## Performance

Nothing new runs on the audio thread. The export decodes and encodes on its own thread, packet by packet.
