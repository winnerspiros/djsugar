# DJ Sugar YouTube Streaming — Behaviour Specification

This document captures the exact runtime behaviour of the C++/Qt YouTube integration in DJ Sugar (Mixxx fork) so it can be reimplemented in Rust. Every fact includes a file:line reference to the source.

---

## 1. Architecture Overview

The YouTube integration is split across these components:

| Component | File | Responsibility |
|-----------|------|----------------|
| **YouTubeService** | `src/library/youtube/youtubeservice.{h,cpp}` | Core extractor/downloader: search, stream resolution, download via InnerTube → Piped → yt-dlp |
| **YouTubeFeature** | `src/library/youtube/youtubefeature.{h,cpp}` | Library feature: track table, sidebar, home feed, download queue, cache mgmt, genre lookup |
| **YouTubeTrackModel** | `src/library/youtube/youtubetrackmodel.{h,cpp}` | SQL-backed track model for `youtube_library` table; handles placeholders (`youtube://VID`) |
| **YouTubeAccount** | `src/library/youtube/youtubeaccount.{h,cpp}` | Signed-in YouTube Music account (cookies from Google sign-in page); liked songs, playlists |
| **SponsorBlockController** | `src/library/youtube/sponsorblockcontroller.{h,cpp}` | Runtime skipping of SponsorBlock segments + analysis-based silence/lead-in skip |
| **YouTubeAudioCutter** | `src/library/youtube/youtubeaudiocutter.{h,cpp}` | libavformat stream-copy to physically cut SponsorBlock ranges from cached file |
| **YouTubeGenreLookup** | `src/library/youtube/youtubegenrelookup.{h,cpp}` | Genre inference via iTunes Search API (US → GR → artist-only) → Deezer fallback |
| **YouTubeView.qml** | `res/qml/Library/Streaming/YouTubeView.qml` | Booth streaming hub UI: search, infinite scroll, account, playlists, per-result actions |

---

## 2. HTTP Endpoints

### 2.1 InnerTube API (Primary)

All requests: `POST https://www.youtube.com/youtubei/v1/{endpoint}` with JSON body, `Content-Type: application/json`.

**Headers set on every request** (`youtubeservice.cpp:139-158` `applyYouTubeRequestAttributes`):
- `User-Agent`: client-specific (see client list below)
- `X-YouTube-Client-Name`: client ID (e.g., `28` for ANDROID_VR)
- `X-YouTube-Client-Version`: client version (e.g., `1.65.10`)
- `X-Origin`: `https://www.youtube.com`
- `X-Goog-Visitor-Id`: `m_visitorData` (fetched once at startup from `https://www.youtube.com/youtubei/v1/log_event` or fallback random)
- `X-Goog-AuthUser`: `0`
- `X-YouTube-Page-Label`: `youtube.ytdesktop_20240101_01_RC00` (search) or `youtube.ytdesktop_20240101_01_RC00` (player)
- `X-YouTube-Page-Cl`: client-specific page CL
- `X-YouTube-Timezone`: `Etc/UTC`
- `Accept-Language`: `en-US,en;q=0.9`
- `Accept`: `application/json`

**Search** (`youtubeservice.cpp:967-1040` `searchVideos`):
- Endpoint: `youtubei/v1/search`
- Body fields:
  - `query`: search string
  - `params`: `EgIQAQ%3D%3D` (music filter) — `youtubeservice.cpp:978`
  - `context.client`: client context (name, version, id, userAgent, deviceMake, deviceModel, androidSdk, osName, osVersion)
  - `context.request.useSsl`: `true`
  - `context.request.internalExperimentFlags`: `[{key: "force_music_enable_music_tab", value: "true"}]`
- Timeout: 10 s (`kSearchTimeoutMs` = 10_000 ms, `youtubeservice.cpp:69`)
- Clients tried in order (`youtubeservice.cpp:290-345` `innerTubeSearchClients`):
  1. **ANDROID_VR** (id `28`, v `1.65.10`) — compactVideoRenderer, ~130 KB response
  2. **TVHTML5_SIMPLY** (id `75`, v `1.0`) — compactVideoRenderer, ~400 KB
  3. **WEB** (id `1`, v `2.20260708.00.00`) — videoRenderer, ~1.3 MB, no poToken needed for search
- Continuation token extracted from `nextContinuationData.continuation` (initial) or `continuationCommand.token` (continuation response) — `youtubeservice.cpp:650-719`

**Player / Stream Resolution** (`youtubeservice.cpp:2646-2981` `downloadViaInnerTubeClient`):
- Endpoint: `youtubei/v1/player`
- Body fields:
  - `videoId`
  - `context.client`: download client context (see client list below)
  - `context.request.useSsl`: `true`
  - `playbackContext.contentPlaybackContext.visitorData`: visitorData
  - `playbackContext.contentPlaybackContext.signatureTimestamp`: from player response if needed
- Clients tried in order (`youtubeservice.cpp:172-288` `innerTubeClients`):
  1. **TVHTML5** (id `7`, v `7.20260707.07.00`) — Cobalt UA, no GVS PO-token policy upstream, returns plain stream URLs
  2. **TVHTML5** (id `7`, v `5.20260707`) — older Cobalt fallback
  3. **ANDROID_VR** (id `28`, v `1.65.10`) — Oculus Quest 3 UA, 403'd since 2026-08-17, kept as fallback
  4. **IOS** (id `5`, v `21.26.4`) — iPhone 16,2 UA, may require poToken, fallback
  5. **ANDROID** (id `3`, v `21.26.364`) — yt-dlp's primary download client, requires poToken for most videos
  6. **TVHTML5_SIMPLY** (id `75`, v `1.0`) — PlayStation 4 UA
- On client failure (HTTP error, no adaptive formats, no audio-only streams, all streams cipher-encrypted): fall over to next client (`youtubeservice.cpp:2659-2980`)
- **Android only**: fetches PO tokens (`playerPot`, `streamingPot`, `visitorData`) via JNI `PoTokenManager.fetchTokens(videoId)` before player request; cached per videoId (`youtubeservice.cpp:2984-3060`)
- Streams with `signatureCipher` (no `url` field) are **skipped** — only non-cipher adaptive streams used (`youtubeservice.cpp:2927-2933`)
- Stream URL gets `&pot=<streamingPot>` appended on Android (`youtubeservice.cpp:2935-2939` `withPoTokenParam`)

### 2.2 Piped API (Fallback)

Round-robin over hardcoded public instances (`youtubeservice.cpp:167-178`):
```
https://api.piped.private.coffee
https://pipedapi.ducks.party
https://piped-api.aymed.org
https://piped.mha.fi
https://piped.kavin.rocks
https://piped.snopyta.org
https://piped.edupage.org
https://piped-project.org
https://piped.rishimo.rocks
https://piped.nics.li
```
- Per-request timeout: 6 s (`kPipedHttpTimeoutMs` = 6_000 ms, `youtubeservice.cpp:74`)
- Search: `GET /search?q={query}&filter=music&page={page}` → returns `{items: [...], nextpage: ...}`
- Trending: `GET /trending?region={region}` → returns `items[]`
- Stream resolve: `GET /streams/{videoId}` → returns `{audioStreams: [...], ...}`
- Audio stream selection: prefer `mimeType` containing `mp4`/`m4a` (AAC) > `webm` (Opus), then highest bitrate (`youtubeservice.cpp:2233-2245`)
- On per-instance failure (HTTP error, timeout, empty results): try next instance (`youtubeservice.cpp:1163-1179`, `2158-2213`)
- Max 5 search pages via Piped (`kMaxPipedSearchPages` = 5, `youtubeservice.cpp:81`)

### 2.3 yt-dlp (Last Resort)

- Desktop: bundled self-contained PyInstaller binary next to executable (`youtubeservice.cpp:348-378` `ytDlpFallbackBins`)
- Android: `youtubedl-android` AAR (Python 3.11 + yt-dlp) via JNI; also works with Termux-installed yt-dlp
- Invoked with: `yt-dlp -f "bestaudio[ext!=webm]/bestaudio" --no-playlist --no-warnings --no-progress -o "{cacheDir}/%(id)s.%(ext)s" --write-info-json --write-thumbnail --embed-thumbnail --add-metadata --parse-metadata "uploader:%(artist)s" "https://www.youtube.com/watch?v={videoId}"`
- Timeout: 10 min (`kDownloadTimeoutMs` = 600_000 ms, `youtubeservice.cpp:70`)
- Output: audio file + `.info.json` metadata sidecar in cache dir

### 2.4 SponsorBlock API

- `GET https://sponsor.ajay.app/api/skipSegments?videoID={videoId}&categories=sponsor,intro,outro,selfpromo,interaction,music_offtopic,preview,poi_highlight,chapter,filler`
- Timeout: 3 s (`kSponsorBlockTimeoutMs` = 3_000 ms, `youtubeservice.cpp:86`)
- Response: array of `{segment: [start, end], category, ...}`
- Sidecar cached at `<cacheDir>/{videoId}.sponsorblock.json` (`sponsorblockcontroller.cpp:24`)

### 2.5 Genre Lookup APIs

**iTunes Search API** (primary):
- `GET https://itunes.apple.com/search?term={normalized_artist} {normalized_title}&entity=song&limit=15&country={US|GR}`
- If no match in US → retry GR → retry GR artist-only (`GR-artist` pass) (`youtubegenrelookup.cpp:285-327`)
- Matching: normalized artist/title + duration within 40 s (`kMaxDurationDiffSec` = 40, `youtubegenrelookup.cpp:25`)
- Genre from `primaryGenreName`; useless genres filtered: `music`, `all`, `other`, `unknown` (`youtubegenrelookup.cpp:28-32`)

**Deezer API** (fallback):
- Search: `GET https://api.deezer.com/search?q=artist:"{artist}" track:"{title}"&limit=10`
- Album lookup: `GET https://api.deezer.com/album/{albumId}`
- Genres from album's `genres.data[].name`, joined by `/`, max 2 (`youtubegenrelookup.cpp:254-272`)

### 2.6 MyInstants (Greek meme sounds)

- `GET https://www.myinstants.com/en/index/gr/` (HTML scrape)
- Regex patterns for `onclick="play('/media/sounds/FILE.mp3')">TITLE<` or `data-url="/media/sounds/FILE.mp3"` (`youtubefeature.cpp:2768-2775`)
- Direct MP3 download from `https://www.myinstants.com{path}` with Referer header

---

## 3. Stream URL Selection & Format Preference

**Preference order** (`youtubeservice.cpp:2225-2245` `downloadAudioStream`):
1. **Container**: M4A (AAC/MP4) > WebM (Opus) — `extFromMime` returns `m4a` for `mp4`/`m4a` mime or `mp4a` codec, `webm` for `webm` mime or `opus` codec
2. **Bitrate**: highest first (within same container)

**Cipher handling**: Streams exposing `signatureCipher` (no direct `url`) are **discarded** at parse time (`youtubeservice.cpp:2927-2933`). Only streams with a ready-to-use `url` field are kept.

**n-parameter / signatureCipher**: Not handled — InnerTube clients chosen specifically to return non-cipher URLs. If a client starts returning cipher streams, it fails over to the next client.

**Piped/Invidious fallbacks**: Piped is the only federated proxy used. No Invidious integration. Piped audio streams are ranked same as InnerTube (M4A > WebM, then bitrate).

**PO tokens (Android only)**: `&pot=<streamingPot>` appended to stream URL for token-gated clients (ANDROID, IOS). TVHTML5 clients do not require PO tokens. Tokens fetched via JNI `PoTokenManager` and cached per videoId (`youtubeservice.cpp:2984-3060`).

---

## 4. Bot-Check / LOGIN_REQUIRED Handling

- **InnerTube**: On HTTP 429 or `botguard`/`LOGIN_REQUIRED` in response: mark client as failed, fail over to next client (`youtubeservice.cpp:1020-1022`, `2659-2980`). Emits `botFlagged(detail)` signal (`youtubeservice.h:404`).
- **Piped**: Per-instance failure (any HTTP error, timeout, empty) → try next instance. No bot-check specific logic beyond generic failover.
- **Rate limiting**: Sliding window 10 s, max 8 requests (`kRateLimitWindowMs` = 10_000, `kRateLimitBurst` = 8, `youtubeservice.cpp:116-117`). Minimum 200 ms gap between requests (`kMinRequestGapMs` = 200, `youtubeservice.cpp:120`). When limit hit, request deferred or caller skips to yt-dlp.
- **Remote config update**: At startup, fetches `https://raw.githubusercontent.com/winnerspiros/djsugar/main/res/youtube/inner_tube_clients.json` (24 h TTL, 5 s timeout) to update client signatures without app update (`youtubeservice.cpp:103-110`).

---

## 5. Sign-In / Account Flow

- **OAuth device code**: NOT used. Sign-in opens Google's sign-in page in a `YouTubeLoginActivity` (Android) or system browser (desktop). Cookies captured on success.
- **Cookie storage**: `QStandardPaths::AppDataLocation/youtube/account` (`youtubeaccount.cpp:37-43`)
- **Cookies used for**: YouTube Music web player requests (liked songs, playlists, channel switching) — signed with `SAPISID`/`__Secure-3PAPISID` hash (`youtubeaccount.cpp:56-60`)
- **Downloads**: Always anonymous (InnerTube/Piped/yt-dlp) — account cookies NOT used for stream resolution.
- **Account endpoints** (YouTube Music InnerTube):
  - `browse` for liked songs (`playlistId` = `LM`), playlists (`VL...`), channel pages
  - Client: web player (`clientVersion` from `music.youtube.com`, UA Chrome 142, `youtubeaccount.cpp:27-32`)
  - Signed requests include `X-Goog-AuthUser: 0`, `Authorization: SAPISIDHASH <timestamp>_<hash>` (`youtubeaccount.cpp:71-113` `post`)
- **Channel switching**: Multiple brand accounts supported; `selectAccount(pageId, name)` switches the active channel for liked/playlist fetches (`youtubeaccount.cpp:50`)

---

## 6. Caching & Download to Disk

**Cache directory** (`youtubefeature.cpp:904-912`):
- Base: `[UserSettingsPath]` or `QStandardPaths::CacheLocation`
- Subdir: `youtube_cache/`
- Thumbnails: `youtube_cache/thumbnails/{videoId}.jpg` (mqdefault.jpg, 320×180)
- MyInstants: `youtube_cache/myinstants/{filename}.mp3`

**File naming**: `{videoId}.{ext}` where ext from `extFromMime` (m4a or webm). No subdirectories.

**Sidecars** (written alongside audio file):
- `.info.json` — yt-dlp metadata (title, uploader, duration, etc.)
- `.meta.json` — raw YouTube title only (`{"title": "..."}`) written by feature at download (`youtubefeature.cpp:576-584`)
- `.sponsorblock.json` — SponsorBlock segments array `[{segment: [start,end], category}]` or `{cut: 1}` if physically cut (`sponsorblockcontroller.cpp:24`, `youtubeaudiocutter.h:10-20`)
- `.part` — temporary download file (renamed on completion)

**Partial download / progressive play**:
- Single-connection: streams to `.part` file, renamed on success (`youtubeservice.cpp:2321-2370`)
- Chunked (parallel Range requests): when `contentLength >= 2 MB` (`kChunkedDownloadThresholdBytes` = 2 MB, `youtubeservice.cpp:79`), uses 16 parallel chunks (`kParallelDownloadChunks` = 16, `youtubeservice.cpp:80`) via `downloadAudioStreamChunked` (`youtubeservice.cpp:2398-2570`). Each chunk writes to a segment file, concatenated on completion.
- **No progressive playback during download** — track only loadable after download finishes and file renamed.

**Download queue** (`youtubefeature.cpp:1452-1517`):
- Max parallel downloads: 3 desktop, 2 Android (`kMaxParallelDownloads`, `youtubefeature.cpp:56` — not shown in excerpt but referenced)
- Player-requested tracks jump to front of queue (`priority = !m_pendingPlayerLoads.value(videoId).isEmpty()`)
- Cache hit: immediate `onDownloadFinished` without network request

**Auto-cleanup** (`youtubefeature.cpp:2580-2719` `cleanCache`):
- Removes cached audio + sidecars for tracks not loaded on any deck
- Removes `youtube_library` rows whose local file is gone
- Triggered manually from UI

---

## 7. SponsorBlock & Audio Cutter Behaviour

**SponsorBlockController** (`sponsorblockcontroller.cpp`):
- One instance per app, watches all decks (`[Channel1]..[Channel8]`)
- On track load: if file in YouTube cache dir → extract videoId from filename
- Segments loaded from sidecar `<cacheDir>/{videoId}.sponsorblock.json` (written at download or on first network fetch)
- If sidecar has `"cut": 1` → segments physically removed from file, `sponsorBlockCovered = true`, no runtime skip
- Else if segments exist → `segmentsKnown = true`, `sponsorBlockCovered = true`, runtime skip active
- Else → fetch from SponsorBlock API (`YouTubeService::fetchSponsorSegments`), persist sidecar on reply

**Runtime skipping** (`sponsorblockcontroller.cpp:255-343` `onPlayPositionChanged`):
- Only while **playing** (`play > 0`)
- If playhead **inside** a SponsorBlock segment → seek to segment end (`playposition = seg.end / duration`)
- Segments checked on every position update (not just crossing start) because segments arrive async
- **Analysis fallback** (when no SponsorBlock segments):
  - Wait 5 s (`kWaitForSponsorBlockSec` = 5.0) for SponsorBlock reply before allowing analysis skip
  - **Lead-in skip**: `N60dBSound` cue (first audible sound) or `Intro` cue (music videos). Music video lead-in uses `Intro` cue end if 8–120 s. Skip once per load (`leadSilenceSkipped` flag). Decision locked after 120 s (`kLeadInDecidedSec`).
  - **Tail skip**: if silence ≥ 1 s at end (`duration - soundEnd >= 1 s`) and playhead past `soundEnd + 0.25 s` → seek to end.

**YouTubeAudioCutter** (`youtubeaudiocutter.cpp`):
- Called at download finish if `autoSkipEnabled` and SponsorBlock segments exist (`youtubefeature.cpp:1620-1660`)
- Uses libavformat stream-copy (no re-encode) to cut ranges from file
- Ranges merged/normalised (overlapping segments combined) (`youtubeaudiocutter.cpp:30-54`)
- PTS shifted so output timeline is contiguous
- On success: writes `{cut: 1}` to `.sponsorblock.json` sidecar, original file replaced
- On failure: falls back to runtime skipping via SponsorBlockController

---

## 8. Genre Lookup

**YouTubeGenreLookup** (`youtubegenrelookup.cpp`):
- Triggered at download finish for new tracks (`youtubefeature.cpp:1660-1690`)
- Normalisation: lower-case, NFKD decompose, strip diacritics, Greek→Greeklish, `&`→`and`, remove `[...]`, `(feat...)`, non-word chars → spaces (`youtubegenrelookup.cpp:117-139`)
- **Same-song matching** (`isSameSong`): artist credit overlap + title prefix match + duration diff ≤ 40 s (`youtubegenrelookup.cpp:142-174`)
- Order: iTunes US → iTunes GR → iTunes GR artist-only → Deezer
- Result cached in `youtube_library` row `genre` column

**Query-based inference** (`youtubefeature.cpp:550-562` `inferGenreFromQuery`):
- Matches search query against `kGenreMatchList` (500+ genres, longest-first) case-insensitive substring
- Used for search results before download

---

## 9. Search Kinds & Home Feed

| Kind | Trigger | Backend | Query/Params | Results Handling |
|------|---------|---------|--------------|------------------|
| **Search** | User enters text + Enter | InnerTube → Piped → yt-dlp | `query` + music filter params | `searchResultsReady` → `onSearchResultsReady` → track table + sidebar |
| **Trending (Home)** | First open / region change | InnerTube (browse `FEmusic`) → Piped | Region code (default `GR`, `youtubeservice.cpp:89`) | Home HTML pane with clickable `ytplay:` links |
| **More (infinite scroll)** | Scroll near bottom | InnerTube continuation token | Stored from last search | `searchMoreReady` → appended to track table |
| **Liked songs** | Signed in, tap ♥ LIKED SONGS | YouTube Account (YouTube Music browse `LM`) | Account cookies | `tracksReady` → track table |
| **Playlist** | Signed in, tap playlist | YouTube Account (YouTube Music browse `VL...`) | Account cookies | `tracksReady` → track table |
| **Playlists list** | Signed in, tap PLAYLISTS | YouTube Account fetchPlaylists | Account cookies | `playlistsReady` → playlist ListView |
| **Genres** | Sidebar Genres node | YouTube Music browse `FEmusic_genres` (auto-discovered) | — | `genresReady` → sidebar rebuild |
| **Samples (DJ Tools)** | Sidebar Samples node | YouTube search for genre term | `inferGenreFromQuery` term | Search results |
| **MyInstants** | Sidebar MyInstants node | HTML scrape `myinstants.com/en/index/gr/` | — | Direct MP3 download to sampler |

**Trending/Home feed filtering** (`youtubefeature.cpp:63-70`):
- Max duration 15 min (`kTrendingMaxDurationSec` = 900 s) — drops megamixes
- Unknown duration (0) kept
- Only applies to home feed; explicit search untouched

**Search batching** (`youtubefeature.cpp:58-61`):
- Initial batch: 50 (desktop) / 50 (Android) — `kSearchResultsMax`
- Continuation pages (~20 videos each) chained until batch ≥ `kSearchBatchSize` (50)
- `minResults` param ensures infinite scroll delivers full batches

---

## 10. Error Messages (User-Visible)

| Situation | Message | Source |
|-----------|---------|--------|
| Search failed | `"Search error: <error>"` | `YouTubeView.qml:279-283` `onSearchFailed` |
| Download failed | `"Download error: <error>"` | `YouTubeView.qml:289-291` `onDownloadFailed` |
| Account error | `"YouTube account: <error>"` | `YouTubeView.qml:274-278` `onAccountFailed` |
| No results | `"No results"` | `YouTubeView.qml:209-211` |
| Empty liked songs | `"No liked songs here yet: like songs on YouTube Music or YouTube and they show up"` | `YouTubeView.qml:737` |
| Empty playlist | `"This playlist is empty"` | `YouTubeView.qml:738` |
| Not signed in (home) | `"Search above. Sign in for your liked songs and playlists"` | `YouTubeView.qml:740` |
| Signed in, empty search | `"Search above, or open your liked songs and playlists"` | `YouTubeView.qml:739` |
| Cache clean | `"Removed %n downloaded track(s) from the cache."` | `youtubefeature.cpp:2716-2718` |
| Bot flagging (home HTML) | Warning banner in home pane with detail | `youtubefeature.cpp:846-855` |

---

## 11. Timeouts & Retries

| Operation | Timeout | Retry Behaviour |
|-----------|---------|-----------------|
| InnerTube search | 10 s | Fail over to Piped → yt-dlp |
| InnerTube player | 10 s (per client) | Fail over to next client (6 clients) |
| Piped HTTP | 6 s per instance | Round-robin to next instance (10 instances) |
| yt-dlp | 10 min | No retry (last resort) |
| SponsorBlock | 3 s | Sidecar cached; no retry on failure |
| Thumbnail download | 10 s (transfer) | Max 8 parallel; skipped if exists |
| Genre lookup (iTunes/Deezer) | 8 s | Fallback chain: US → GR → GR-artist → Deezer |
| MyInstants fetch | 10 s | No retry |
| MyInstants download | 15 s | No retry |
| Rate limit | 10 s window, 8 burst | Defer or skip to yt-dlp |
| Min request gap | 200 ms | Enforced between all YouTube requests |

**Infinite scroll retry**: Empty batch from `fetchMoreSearchResults` → `retryMoreAfter = now + 4 s` (`YouTubeView.qml:220-222`). Prevents hammering throttled connection.

---

## 12. YouTubeView.qml — UI Elements & Actions

### 12.1 Top Bar
- **YouTube logo** (red triangle) — static
- **Search field** (`TextInput`):
  - Placeholder: `"Search YouTube: songs, sets, remixes"`
  - On `activeFocus` gained: `Mixxx.YouTube.warmUp()` (opens connection early)
  - On `accepted` (Enter): `doSearch()` → `Mixxx.YouTube.search(text)`
  - Clear button? No — text stays
- **SEARCH button**: Disabled while `isBusy`, text `"..."` during search
- **Account button**:
  - Not signed in: red, `"SIGN IN"` → `Mixxx.YouTube.signIn("youtube")`
  - Signed in: dark red, shows account initial + name → opens `accountMenu` popup
  - `accountMenu`:
    - Channel switcher (Repeater over `root.channels`): tap → `Mixxx.YouTube.selectAccount(pageId, name)`
    - Divider
    - `SIGN OUT` → `Mixxx.YouTube.signOut()`

### 12.2 Signed-In Row (visible when `youTubeSignedIn`)
- **♥ LIKED SONGS** button: active when `listKind === "liked"` → `openList("liked", "Liked songs")` → `Mixxx.YouTube.youTubeLiked()`
- **PLAYLISTS** button: active when `listKind === "playlists"` or `"playlist"` → `showPlaylists()` → `Mixxx.YouTube.youTubePlaylists()`
- **Status text**: `"<playlistName>  ·  <statusMessage>"`

### 12.3 Playlists ListView (`listKind === "playlists"`)
- Delegate: artwork (40×40) + name + subtitle
- Tap → `openList(id, name)` → `Mixxx.YouTube.youTubePlaylist(id)`

### 12.4 Empty State (when `resultsModel.count === 0 && !isBusy && listKind !== "playlists"`)
- Large red play triangle
- Text varies by `listKind`:
  - `liked`: "No liked songs here yet..."
  - `playlist`: "This playlist is empty"
  - `search` + signed in: "Search above, or open your liked songs and playlists"
  - `search` + not signed in: "Search above. Sign in for your liked songs and playlists"

### 12.5 Results ListView (`listKind !== "playlists" && (count > 0 || isBusy)`)
- **Delegate** (`videoCard`, 48 px height, alternating row colors):
  - **Thumbnail** (left, 36×36): `image://youtube/<videoId>` (served from `thumbnailDir`)
  - **Title** (bold, white, elided)
  - **Uploader** (gray, elided)
  - **Duration** (gray, MM:SS or HH:MM:SS)
  - **Genre** (accent color, elided) — only if non-empty
  - **BPM/Key badges** (right side, if `bpm > 0` / `musicKey` non-empty):
    - BPM: green if `fitsDeck(bpm)` (within 6% at 1×, 2×, 0.5×), else gray
    - Key: cyan
  - **Versions badge** (if `versions > 1`): `"+N versions"` in accent
  - **Times played** (if `> 0`): `"♺ N"` in accent
  - **Download progress overlay** (while `downloadProg >= 0 && < 1.0`):
    - Translucent pink fill across row (animated width)
    - Bottom 4 px gradient bar (pink→cyan)
  - **Action buttons** (right side, visible on hover/press):
    - **Deck load buttons** (Repeater over `[Channel1]..[Channel4]`): label `"1"`..`"4"` or `"%N"` during download → `Mixxx.YouTube.downloadToDeck(vid, group)`
    - **Save button** (green down-arrow): `Mixxx.YouTube.download(vid)` — downloads to library without loading
  - **Drag handle**: `Library.TrackDrag` → drag to deck loads via `downloadToDeck`

### 12.6 Infinite Scroll
- `onContentYChanged` → `maybeLoadMore()`
- Triggers when `contentHeight - contentY - height ≤ 900 px` from bottom
- Guards: not busy, not loading more, `hasMore`, visible, `allResults.length > 0`, `now > retryMoreAfter`
- Calls `Mixxx.YouTube.loadMore()` → `fetchMoreSearchResults`

### 12.7 Controller Navigation (hardware)
- `controllerMove(offset)`: moves `resultList.currentIndex`, calls `Mixxx.YouTube.prefetchLink(vid)` for highlighted
- `controllerLoad(group, _play)`: loads `currentIndex` to `group` via `downloadToDeck`

### 12.8 Signals from Mixxx.YouTube (connected in `Connections`)
- `onSearchFinished(results)`: clears `allResults`, `resultsModel`, appends batch, updates `hasMore`, `statusMessage`
- `onSearchMoreFinished(results, hasMore)`: appends batch, updates `hasMore`, sets `retryMoreAfter = now + 4 s` if 0 added
- `onGenreResolved(videoId, genre)`: updates `allResults` and `resultsModel` row
- `onAccountsChanged()`: handles sign-in/out, opens account on new sign-in
- `onYouTubePlaylistsLoaded(playlists)`: updates `root.playlists`, status
- `onYouTubeAccountsLoaded(accounts)`: maps to `root.channels` with single selected highlight
- `onAccountFailed(error)`: clears busy, shows error
- `onSearchFailed(error)`: clears busy, shows error
- `onDownloadFinished(videoId, path)`: status `"Download finished"`
- `onDownloadFailed(videoId, error)`: status `"Download error: <error>"`

---

## 13. YouTubeFeature — Key Internal Behaviours

**Sidebar** (`rebuildSidebar`):
- Nodes: Search, Trending (home), Genres (auto-discovered or `kDefaultGenres`), Samples (DJ Tools), MyInstants, Cache Clean
- Click handling in `activateChild`:
  - `yt-search:` → `searchAndActivate(query)`
  - `yt-cached:` → load downloaded track
  - `yt-genre:` → search for genre term
  - `myinstant-fetch` → `fetchMyInstantsSounds()`
  - `myinstant:<url>` → `downloadMyInstant(url, name)`
  - `ytplay:<videoId>` → `requestDownloadToPlayer(videoId, group, true)`
  - `ytcached:<videoId>` → load cached
  - `ytautoanalyze:<videoId>` → `requestPrefetch` (background download for analysis)
  - `ytgenre:<name>` → search for genre

**Home HTML pane** (`rebuildHomeHtml`):
- Generated HTML with clickable `<a href="ytplay:VID">` links for trending/top songs
- Shows bot-flag warning banner if `m_lastSearchError` set
- Switches view via `switchToView("YOUTUBE_HOME")` on search error (desktop only)

**Track table** (`replaceTrackTable` / `appendToTrackTable`):
- Writes to `youtube_library` SQL table (schema: id, artist, title, album, genre, location, comment=videoId, duration)
- Placeholder rows: `location = "youtube://<videoId>"` (unique via comment)
- Real rows: `location = <cacheDir>/<videoId>.<ext>`
- `YouTubeTrackModel` serves thumbnails from `thumbnailDir` via `getCoverInfo`

**Download lifecycle**:
1. `requestDownloadFile(videoId)` → checks cache → if hit, `onDownloadFinished` immediately
2. Else queues (priority for player loads) → `m_service.downloadVideo(videoId, cacheDir)`
3. `YouTubeService` tries InnerTube → Piped → yt-dlp
4. On success: `downloadFinished(videoId, localPath)` → `YouTubeFeature::onDownloadFinished`
5. `upsertDownloadedRow` → updates `youtube_library` with real path, parsed metadata
6. `fetchThumbnails` for videoId
7. `YouTubeGenreLookup::lookup` for genre
8. If `autoSkipEnabled` and SponsorBlock segments → `cutAudioRanges` (physical cut)
9. `fileReady(videoId, localPath)` emitted
10. Pending player loads for this videoId → `loadTrackToPlayer` / `loadTrackToNextSampler`
11. `playerLoadsSettled(videoId)` emitted when all decks done

**AutoDJ prefetch** (`prefetchAutoDjQueue`): At startup, re-downloads any AutoDJ-queued YouTube tracks missing from cache.

**Cache cleanup** (`cleanCache`): Removes unreferenced audio files + sidecars + stale DB rows.

---

## 14. YouTubeTrackModel — Placeholder vs Real Rows

- **Placeholder row**: `location = "youtube://<videoId>"` → `getTrack()` returns null, `getTrackUrl()` returns `QUrl("youtube://<videoId>")`
- **Real row**: `location = <absolute path>` → `getTrack()` returns `TrackPointer` from `getOrAddTrack(location)`
- `WTrackTableView` / `WTrackMenu` detect `youtube://` scheme → dispatch `loadTrackLocationToPlayer` → feature downloads + loads
- Columns: id, artist, title, album, year, genre, tracknumber, location, comment, duration, bitrate, bpm, rating
- Searchable columns: artist, title, album, comment
- `search()` override: does NOT dispatch YouTube search (keystroke debounce would spam). Only `searchNow()` (Enter) fires `searchRequested`.

---

## 15. Android-Specific Behaviours

- **Parallel limits**: `kSearchResultsMax = 50` (same as desktop), `kMaxParallelDownloads = 2` (vs 3 desktop)
- **PO tokens**: Required for ANDROID/IOS clients; fetched via JNI `PoTokenManager` (blocks up to ~105 s init + mint). Cached per videoId.
- **yt-dlp**: Bundled `youtubedl-android` AAR (Python 3.11 + yt-dlp) via JNI; no Termux/system Python needed. Falls back to Termux yt-dlp if present.
- **Search error view**: On Android, search error does NOT switch to home HTML pane (stays on track table; error in sidebar/log) — `youtubefeature.cpp:1374-1382`
- **Library scan contention**: `QTimer::singleShot(0)` defer on `select()` after cache clean to avoid SQLite lock contention — `youtubefeature.cpp:2710-2714`
- **Thumbnail parallelism**: 8 max (`kMaxParallelThumbs` = 8) — `youtubefeature.cpp:945`
- **UI**: YouTubeView.qml same; hardware controller navigation via `controllerMove`/`controllerLoad`

---

## 16. File:Line Reference Index

| Topic | File | Lines |
|-------|------|-------|
| InnerTube client list (download) | `youtubeservice.cpp` | 172-288 |
| InnerTube client list (search) | `youtubeservice.cpp` | 290-345 |
| InnerTube request headers | `youtubeservice.cpp` | 139-158 |
| Search request body | `youtubeservice.cpp` | 967-1040 |
| Player request + PO tokens | `youtubeservice.cpp` | 2646-3060 |
| Stream parsing (skip cipher) | `youtubeservice.cpp` | 2910-2980 |
| Stream ranking (M4A > WebM, bitrate) | `youtubeservice.cpp` | 2225-2245 |
| Piped instance list | `youtubeservice.cpp` | 167-178 |
| Piped search/download | `youtubeservice.cpp` | 2158-2213 |
| yt-dlp fallback paths | `youtubeservice.cpp` | 348-378 |
| yt-dlp invocation | `youtubeservice.cpp` | 3207-3350 |
| Rate limiting | `youtubeservice.cpp` | 116-120 |
| Remote config fetch | `youtubeservice.cpp` | 103-110 |
| SponsorBlock fetch | `youtubeservice.cpp` | 194-210 (signals), `sponsorblockcontroller.cpp` | 146-150, 200-230 |
| Audio cutting | `youtubeaudiocutter.cpp` | 84-234 |
| Runtime skip logic | `sponsorblockcontroller.cpp` | 255-343 |
| Genre lookup (iTunes/Deezer) | `youtubegenrelookup.cpp` | 274-372 |
| Query genre inference | `youtubefeature.cpp` | 550-562 |
| Cache dir / sidecars | `youtubefeature.cpp` | 904-918, 576-584 |
| Download queue | `youtubefeature.cpp` | 1452-1517 |
| Chunked download | `youtubeservice.cpp` | 2398-2570 |
| Auto-cleanup | `youtubefeature.cpp` | 2580-2719 |
| YouTubeAccount cookies | `youtubeaccount.cpp` | 37-60 |
| YouTubeAccount signed requests | `youtubeaccount.cpp` | 71-113 |
| YouTubeView.qml search | `YouTubeView.qml` | 96-118 |
| YouTubeView.qml infinite scroll | `YouTubeView.qml` | 108-118 |
| YouTubeView.qml result delegate | `YouTubeView.qml` | 776-1052 |
| YouTubeView.qml account menu | `YouTubeView.qml` | 369-540 |
| YouTubeView.qml playlist list | `YouTubeView.qml` | 642-704 |
| YouTubeTrackModel placeholder | `youtubetrackmodel.h` | 30-37, 45 |

---

## 17. Summary of Critical Invariants

1. **Three-tier backend**: InnerTube (primary) → Piped (fallback) → yt-dlp (last resort). Never skip a tier unless it fails.
2. **Client failover order is fixed** and chosen for non-cipher stream URLs. TVHTML5 first (no PO token), then Android VR, iOS, Android, TVHTML5_SIMPLY.
3. **Cipher streams are discarded** — not deciphered. Client failover handles YouTube's tightening.
4. **PO tokens only on Android** for token-gated clients (ANDROID, IOS). TVHTML5 clients need none.
5. **Downloads are anonymous** — account cookies used only for YouTube Music browse (liked/playlists).
6. **M4A (AAC) preferred over WebM (Opus)** for playback compatibility; bitrate secondary.
7. **SponsorBlock segments cut at download** if auto-skip enabled; otherwise skipped at playback.
8. **Cache cleanup removes unreferenced files** — no unbounded disk growth.
9. **Search only on Enter** — keystroke debounce explicitly disabled to avoid spam.
10. **Default region is GR (Greece)** — hardcoded project default, not geo-IP.
11. **Rate limit: 8 req / 10 s, min 200 ms gap** — enforced across all YouTube requests.
12. **Infinite scroll chains continuation pages** until batch ≥ 50; empty batch → 4 s cooldown.
13. **Chunked download (≥ 2 MB) uses 16 parallel Range requests** for cellular throughput.
14. **Genre lookup: iTunes US → GR → GR-artist → Deezer** with strict same-song matching.
15. **Signed-in UI**: account button → channel switcher + sign out; liked songs & playlists via YouTube Music InnerTube.

---

*Generated from DJ Sugar source (commit not recorded). All line numbers approximate to source snapshot.*