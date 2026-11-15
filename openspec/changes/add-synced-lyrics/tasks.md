# Tasks

## 1. Read and show lyrics

- [x] 1.1 `djsugar_engine::lyrics`: lines with optional word times; LRC, enhanced LRC, `[offset:]`, repeated stamps, plain text; ID3 `SYLT` (by line or by word), `USLT`, Vorbis `LYRICS`, MP4 `©lyr`; `.lrc` beside the track first; tests
- [x] 1.2 Lyrics tab in the side panel: follows Playing, Selected, Deck A or Deck B; source and timing; line sung now lit and kept in view; click a timed line to play from it
- [x] 1.3 User guide: Lyrics page

## 2. Player mode

- [x] 2.1 Lyrics button on the Player bar; large lyrics in place of the browser, sung words lit when timed by word, a ♪ for breaks in the singing, click a line to play from it

## 3. Find lyrics online

- [x] 3.1 Settings › Collection › Health › Find lyrics online: LRCLIB switch, on at first (changed from off by a user request, 2026-11-07)
- [x] 3.2 Find lyrics in the tab: LRCLIB `/api/get` then `/api/search`, matches ranked by length and timing, saved as `.lrc` beside the track (DJ Sugar's own lyrics folder when that folder can't be written)
- [ ] 3.3 Collection Health › Info: find lyrics for many tracks, one request at a time, honouring `Retry-After`

## 4. Fix lyrics

- [ ] 4.1 Edit text, tap to sync, nudge timing, save to `.lrc` or tags (`SYLT` + `USLT` for MP3, `LYRICS` for FLAC/Ogg, `©lyr` for MP4)

## Workflow follow-up

- Archive the change once its last PR is merged, so the main specs take its requirements.
