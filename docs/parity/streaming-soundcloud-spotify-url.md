# DJ Sugar Streaming Parity Spec: SoundCloud, Spotify, Custom URL / Web

This document specifies the exact behaviour of DJ Sugar's three streaming sources for reimplementation in Rust. Every fact includes file:line references to the C++/Qt source.

---

## 1. SoundCloud (via YouTube Proxy)

### 1.1 Architecture
- SoundCloud is **not a separate service** — it is implemented inside `QmlYouTubeProxy` (`Mixxx.YouTube` singleton) and reuses the YouTube download/cache/deck-load pipeline.
- Track IDs presented to QML: `sc-<numeric_track_id>` (e.g. `sc-123456789`).
- Download uses **yt-dlp** with the user's OAuth token (when signed in) passed as `--username oauth --password <token>`.
- Go+ (paid) tracks: SoundCloud returns only a 30 s preview without a valid token; UI marks them `preview: true` and blocks loading.

**Refs:** `qmlyoutubeproxy.h:76-82`, `qmlyoutubeproxy.cpp:575-736`, `youtubeservice.cpp:420-431`, `youtubeservice.cpp:3234-3239`

---

### 1.2 HTTP Endpoints

| Operation | URL | Method | Query Params | Headers |
|-----------|-----|--------|--------------|---------|
| Search tracks | `https://api-v2.soundcloud.com/search/tracks` | GET | `q=<query>`, `limit=20`, `client_id=<web_player_client_id>` | `User-Agent: Mozilla/5.0 (Linux; Android 14; Mobile; Mixxx-DJ) AppleWebKit/537.36` |
| User likes (paginated) | `https://api-v2.soundcloud.com/users/{userId}/track_likes` | GET | `limit=20`, `client_id=<web_player_client_id>` | same |
| Next page | value of `next_href` from previous response | GET | already contains `client_id` | same |
| Current user ("me") | `https://api-v2.soundcloud.com/me` | GET | `client_id=<web_player_client_id>` | same + `Authorization: OAuth <token>` when signed in |

**Refs:** `qmlyoutubeproxy.cpp:579-584` (search), `qmlyoutubeproxy.cpp:587-599` (likes), `qmlyoutubeproxy.cpp:602-609` (more), `qmlyoutubeproxy.cpp:611-637` (request builder), `qmlyoutubeproxy.cpp:530-572` (`fetchSoundCloudMe`)

---

### 1.3 Client ID Discovery
- The web player's public `client_id` is **scraped from `https://soundcloud.com`** on first use.
- `fetchSoundCloudClientId()` loads the homepage HTML, extracts `client_id` via regex `/client_id["\s:]+([a-z0-9]{32})/i`.
- Cached in memory (`m_scClientId`); on 401/403 it is cleared and re-fetched once.
- No persistent storage — re-discovered each app start.

**Refs:** `qmlyoutubeproxy.cpp:512-548` (`fetchSoundCloudClientId`)

---

### 1.4 Auth / OAuth Token
- **Sign-in**: Android only. Opens `https://soundcloud.com/signin` in `AccountLoginActivity` (WebView). On success, the activity returns the `oauth_token` cookie value.
- **Storage**: `~/.local/share/djsugar/soundcloud/account` (plain text, `600` perms).
- **Access**: `mixxx::soundcloud::token()` reads the file; empty string = not signed in.
- **Usage**: Passed to yt-dlp as `--username oauth --password <token>` for Go+ tracks.
- **Sign-out**: Deletes the account file; clears `m_scUserId`, `m_scName`, `m_scAvatar`; emits `accountsChanged`.
- **Token refresh**: None — user must sign in again when token expires (detected via 401/403 on API calls).

**Refs:** `soundcloudaccount.h:11-39`, `qmlyoutubeproxy.cpp:38-41` (startup load), `qmlyoutubeproxy.cpp:757-778` (`signIn`), `qmlyoutubeproxy.cpp:785-800` (`onSignedIn`), `qmlyoutubeproxy.cpp:801-820` (`signOut`)

---

### 1.5 Search / Likes Flow
1. User enters query → `doSearch()` → `Mixxx.YouTube.soundCloudSearch(query)`.
2. `soundCloudSearch()` ensures network + client ID, clears `m_scNextHref`, increments generation counter, calls `soundCloudRequest()`.
3. Response JSON parsed:
   - `collection[]` array of track objects (or `{track: {...}}` for likes).
   - Each track → `YouTubeVideoInfo` with `id = "sc-" + track.id`, title, uploader.username, durationSec (ms→s), genre (track.genre or first tag from tag_list), artwork_url (fallback user.avatar_url), plays (playback_count), preview (policy == "SNIP").
   - `next_href` stored for pagination.
4. Results emitted via `soundCloudResults(QVariantList, append)` signal.
5. QML `ListModel` receives `vid`, `title`, `uploader`, `durationSec`, `genre`, `artwork`, `plays`, `preview`, `bpm`, `musicKey`.

**Refs:** `qmlyoutubeproxy.cpp:575-585`, `qmlyoutubeproxy.cpp:666-736` (response parsing), `SoundCloudView.qml:102-128` (signal handler), `SoundCloudView.qml:90-122` (model fields)

---

### 1.6 Pagination
- `soundCloudHasMore()` returns `!m_scNextHref.isEmpty()`.
- `soundCloudMore()` calls `soundCloudRequest(m_scNextHref, true, false)`.
- QML `maybeLoadMore()` triggers when list scrolled near bottom (`contentHeight - contentY - height > 900`).

**Refs:** `qmlyoutubeproxy.h:80-82`, `qmlyoutubeproxy.cpp:602-609`, `SoundCloudView.qml:44-51`

---

### 1.7 Audio Acquisition (Download)
- **Path**: Same as YouTube — `YouTubeService::download()` → yt-dlp.
- **Media URL**: `https://api.soundcloud.com/tracks/<trackId>` (constructed by `mediaUrlForId()` stripping `sc-` prefix).
- **yt-dlp args** (desktop):
  ```
  -f bestaudio --no-playlist --no-warnings --no-progress --no-cache-dir
  --ignore-config --no-mtime --concurrent-fragments 8 --http-chunk-size 1M
  --no-check-formats -o <template> --print after_move:filepath -- <media_url>
  ```
  + anti-bot args (cookies, extractor-args, sleep intervals).
  + **If signed in**: `--username oauth --password <token>`.
- **Android**: bundled `youtubedl-android` (JNI) with same args.
- **Output**: cached under YouTube cache dir (`<cache>/youtube/<videoId>.*`).
- **Completion**: `downloadFinished(videoId, localPath)` → `YouTubeFeature::finalizeDownload()` → loads to deck if `videoId` mapped to a deck group.

**Refs:** `youtubeservice.cpp:420-431` (`mediaUrlForId`), `youtubeservice.cpp:3220-3313` (`download` yt-dlp path), `youtubeservice.cpp:3315+` (Android path), `qmlyoutubeproxy.cpp:83-111` (deck mapping)

---

### 1.8 UI: SoundCloudView.qml

| Element | Behaviour |
|---------|-----------|
| Header row | ☁ logo (orange), search field (placeholder "Search SoundCloud: remixes, edits, DJ mixes"), SEARCH button, Account button |
| Account button | Not signed in: orange "SIGN IN" → `Mixxx.YouTube.signIn("soundcloud")`. Signed in: shows avatar + name; tap opens account menu (Sign out, View profile). |
| Results list | Infinite scroll `ListView`; each row: artwork (60×60), title, uploader, duration, genre, play count (formatted K/M), BPM/key chips (if match deck ±6%), preview badge (⚠ Go+). |
| Row tap / controller LOAD | `controllerLoad()` → if `preview` true: show "Go+ track: SoundCloud only gives a 30 s preview"; else `Mixxx.YouTube.downloadToDeck(vid, group)`. |
| Controller BROWSE | `controllerMove(±1)` moves highlight, triggers `maybeLoadMore()`. |
| Status message | "Searching...", "%1 results", "No results", "%1 liked tracks", "No likes yet", errors. |
| Auto-load likes | On `Component.completed`, if `soundCloudSignedIn` → `openLikes()`. |

**Refs:** `SoundCloudView.qml:14-88` (properties/functions), `SoundCloudView.qml:151-400` (UI), `SoundCloudView.qml:68-88` (controller)

---

### 1.9 Errors / Edge Cases
- No client ID → "SoundCloud is not reachable".
- 401/403 on request with token → token cleared, "SoundCloud wants a new sign-in".
- 401/403 without token → client ID re-fetched once.
- Empty query → no-op.
- Generation counter ignores stale responses.

**Refs:** `qmlyoutubeproxy.cpp:611-665`

---

## 2. Spotify (via librespot Bridge)

### 2.1 Architecture
- **Rust library** `lib/spotify-bridge` (librespot) compiled as `libspotifybridge.so` / `.dylib` / `.a`.
- C API (`spotifybridge.h`) called from `QmlSpotifyProxy` (`Mixxx.Spotify` singleton).
- **No Spotify Developer App** — uses Spotify's public web client ID (`65b708073fc0480ea92a077233ca87bd`) with PKCE.
- **Session** = librespot `Session` connected via stored credentials; reused across calls.
- **Browsing**: Spotify **client endpoints (spclient)**, not public Web API (avoids rate limits).
- **Audio**: Encrypted Ogg Vorbis from Spotify CDN → decrypted via `librespot_audio::AudioDecrypt` → written as plain `.ogg` → loaded like local file.
- **Requires Premium**; Free accounts get error and fallback to YouTube Music search.

**Refs:** `lib.rs:1-9`, `lib.rs:40-58`, `qmlspotifyproxy.h:21-28`, `qmlspotifyproxy.cpp:330-428`

---

### 2.2 Auth Flow (PKCE OAuth)
1. `login()` → spawns thread → `login_web()`:
   - Binds `127.0.0.1:5588`.
   - Generates `code_verifier` (64 random bytes, URL-safe base64), `code_challenge` = SHA256(verifier), `state` (16 bytes).
   - Opens browser / Android `SpotifyLoginActivity` to:
     ```
     https://accounts.spotify.com/authorize?
       client_id=65b708073fc0480ea92a077233ca87bd&
       response_type=code&
       redirect_uri=http://127.0.0.1:5588/login&
       scope=streaming user-read-private user-library-read playlist-read-private playlist-read-collaborative&
       code_challenge_method=S256&
       code_challenge=<challenge>&
       state=<state>
     ```
   - Waits for redirect to `/login?code=...&state=...` (or `error=...`).
   - Exchanges code for access token via `POST https://accounts.spotify.com/api/token` (form: `grant_type=authorization_code`, `code`, `redirect_uri`, `client_id`, `code_verifier`).
   - Creates librespot `Credentials::with_access_token(token)` → `Session::connect()`.
   - Credentials persisted by librespot in cache dir (`<appdata>/spotify/`).
2. On success: `loggedIn=true`, `userName` set, `loginChanged` emitted.
3. Cancel: `cancelLogin()` → `spotify_cancel_login()` → closes port, aborts.
4. Logout: `spotify_logout(cache_dir)` → clears credentials, `loggedIn=false`.

**Refs:** `lib.rs:285-430` (`login_web`, `wait_for_code`, `token_request`), `qmlspotifyproxy.cpp:103-148` (`login`), `qmlspotifyproxy.cpp:151-193` (`cancelLogin`, `logout`, `onLoginResult`)

---

### 2.3 Credentials Storage
- Directory: `<AppDataLocation>/spotify/` (e.g. `~/.local/share/djsugar/spotify/`).
- librespot `Cache` stores `credentials.json` (encrypted refresh/access tokens) + `device_id` (40 hex chars, generated once).
- `device_id` identifies the "device" in Spotify Connect.

**Refs:** `lib.rs:115-145` (`cache`, `device_id`, `config`)

---

### 2.4 Browsing Endpoints (via `spotify_browse` C API)
All calls go through `spotify_browse(cache_dir, request, offset, limit, out_json)` on a worker thread. Request strings:

| Request | Meaning | Returns |
|---------|---------|---------|
| `profile` | Current user profile | `{name, uri, images[]}` |
| `search:<query>` | Search tracks | `{items: [track...], next: offset}` |
| `liked` | Liked Songs (Saved Tracks) | same |
| `playlists` | User's playlists | `{items: [playlist...]}` |
| `playlist:<22-char-id>` | Playlist tracks | `{items: [track...], next: offset}` |

- Pagination: `next` field = offset for next page (or -1).
- Page size: 50 (hardcoded `kPage`).
- Track JSON fields: `id` (22-char base62), `uri` (`spotify:track:<id>`), `title`, `artist` (comma-joined), `album`, `durationSec`, `artwork` (smallest ≥64px `https://i.scdn.co/image/<hex>`), `explicit`.
- Playlist JSON fields: `id`, `name`, `artwork`, `count` (track count).

**Refs:** `browse.rs:48-125` (`track_json`, `context_tracks`), `browse.rs:126-250` (search/liked/playlists/playlist handlers), `qmlspotifyproxy.cpp:206-290` (`browse`, `fetchList`), `qmlspotifyproxy.cpp:293-314` (`search`, `likedSongs`, `playlists`, `playlistTracks`)

---

### 2.5 Audio Download (`spotify_download` C API)
1. Validate Premium: `session.get_user_attribute("type") != "premium"` → error.
2. Resolve playable item: `AudioItem::get_file(session, SpotifyUri::Track{id})`; if unavailable, try `item.alternatives` (other releases).
3. Pick best format: `OGG_VORBIS_320` > `160` > `96`.
4. Request audio key: `session.audio_key().request(track_id, file_id)`.
5. Open encrypted stream: `AudioFile::open(session, file_id, 40KB)` → `controller.set_random_access_mode()`.
6. Decrypt + write: `AudioDecrypt::new(key, encrypted)` → seek past Spotify Ogg header (0xA7 bytes) → read 64KB blocks → write to `<cache>/<trackId>.ogg.part` → rename to `.ogg`.
7. Progress callback: 0.05 → 0.95 (linear by bytes), then 1.0.
8. On failure (e.g. `INVALID_CREDENTIALS`, key refusal #1649): error propagated → QML falls back to YouTube Music.

**Refs:** `lib.rs:462-544` (`download`), `qmlspotifyproxy.cpp:358-410` (`downloadTrack`, `onDownloaded`)

---

### 2.6 Cache & Deck Load
- Cache dir: `<AppDataLocation>/spotify_cache/` (e.g. `~/.local/share/djsugar/spotify_cache/`).
- File name: `<spotifyId>.ogg`.
- On `loadToDeck(track, deckGroup)`:
  - If cached → `loadFile()` → `TrackCollectionManager::getOrAddTrack()` → set title/artist/album from Spotify metadata, `comment = uri` → `slotLoadTrackToPlayer()`.
  - If not cached → queue `PendingLoad` per deck → start single `downloadTrack()`.
  - Download progress → `downloadProgressChanged(spotifyId, 0..1)`.
  - On success → `fileReady(spotifyId, path)` → load to all pending decks.
  - On failure → `m_spotifyRefused = true` → `loadFromYouTube()` (search `artist + title` on YouTube, download that, map Spotify ID → YouTube video ID for progress).

**Refs:** `qmlspotifyproxy.cpp:323-356` (`loadToDeck`), `qmlspotifyproxy.cpp:330-338` (cache check), `qmlspotifyproxy.cpp:413-504` (`onDownloaded`, `loadFromYouTube`), `qmlspotifyproxy.cpp:507-534` (`loadFile`)

---

### 2.7 UI: SpotifyView.qml

| Element | Behaviour |
|---------|-----------|
| Not-logged-in view | Centered: Spotify logo, "Log in once with your Spotify account", green "LOG IN TO SPOTIFY" button → `Mixxx.Spotify.login()`. If logging in: "OPEN LOGIN PAGE AGAIN" + "CANCEL". |
| Logged-in header | Search field (placeholder "Search Spotify"), green SEARCH button. |
| Navigation chips | Three chips: "Liked Songs" → `likedSongs()`, "Playlists" → `playlists()`, "Search" (active when search used). |
| Playlists list | Rows: artwork (40×40), name, track count. Tap → `openPlaylist(id, name)` → `playlistTracks(id)`. |
| Tracks list | Rows: artwork (50×50), title, artist, album, duration, download progress ring (while downloading). Tap / LOAD → `loadRow()` → `loadToDeck()`. |
| Controller BROWSE | `controllerMove(±1)` moves highlight, triggers `maybeLoadMore()` (scroll near bottom). |
| Controller LOAD | `controllerLoad(group)` → `loadRow(currentIndex, group)`. |
| Status message | "Loading...", "%1 tracks", "%1 playlists", "No tracks", errors. |
| Auto-load | On `Component.completed` if `loggedIn` → `openList("liked")`. On `loginChanged` if logged in & empty → same. |

**Refs:** `SpotifyView.qml:12-99` (properties/functions), `SpotifyView.qml:105-176` (signals), `SpotifyView.qml:178-499` (UI)

---

### 2.8 Errors / Edge Cases
- Build without `HAVE_SPOTIFY_BRIDGE` → `available() = false`, all ops no-op with "Spotify is not part of this build".
- `INVALID_CREDENTIALS` on browse → auto-logout + "Spotify wants a new login".
- Key refusal (librespot #1649) → "Spotify gives this account no audio keys... tracks can't be loaded" → fallback to YouTube.
- Network errors during login → retry up to 30× with 2 s delay.
- Cancelled login → "cancelled" (no error toast).

**Refs:** `qmlspotifyproxy.cpp:88-94`, `qmlspotifyproxy.cpp:234-248`, `lib.rs:492-498`

---

## 3. Custom URL / Web Streaming (StreamingManager + WebView)

### 3.1 Architecture
- **Smart input box** (`WebView.qml`) classifies pasted text via `webinput.mjs` → routes to:
  - **Navidrome/Subsonic** (source 0): search, server login, playlists.
  - **Direct URL / Radio** (source 1): `CustomUrlView` downloads and loads.
- **StreamingManager** (`QmlStreamingProxy`, singleton `Mixxx.StreamingManager`) handles:
  - Subsonic/Navidrome REST API (ping, search, playlists, stream URLs).
  - Audius search/trending (discovery provider).
  - Generic HTTP(S) / icy:// / rtsp:// / mms:// download + cache + deck load.
- Cache dir: `<CacheLocation>/streaming/` (desktop) or `<AppDataLocation>/streaming_cache/` (Android).

**Refs:** `WebView.qml:9-54` (`feed`, `classify`), `webinput.mjs:1-61`, `qmlstreamingproxy.h:31-60`, `qmlstreamingproxy.cpp:31-52` (cache dir)

---

### 3.2 URL Classification (`webinput.mjs::classify`)

| Input Pattern | Kind | Action |
|---------------|------|--------|
| Words (no scheme, no dot, contains space) | `search` | Navidrome search `query` |
| `http(s)://...` with audio extension (`.mp3`, `.m4a`, `.aac`, `.flac`, `.ogg`, `.oga`, `.opus`, `.wav`, `.aif`, `.aiff`, `.wma`, `.pls`, `.m3u`, `.m3u8`, `.asx`, `.xspf`) | `url` | Direct download |
| `http(s)://...` with stream path (`/stream/`, `/listen/`, `/live/`, `/radio/`, `/icecast/`, `;`) | `url` | Direct download |
| `http(s)://...` with stream host (`stream.`, `streams.`, `listen.`, `live.`, `radio.`, `icecast.`, `shoutcast.`, `ice<N>.`) | `url` | Direct download |
| `icy://`, `rtsp://`, `mms://` | `url` | Direct download |
| `http(s)://...` with empty path `/` or `/app` or `/rest` or host matching `navidrome|subsonic|airsonic|gonic|ampache|funkwhale` | `server` | Navidrome server login |
| `localhost:port` or bare IPv4 | `server` (if no audio/stream markers) | Navidrome server login |
| Anything else with valid domain.tld | `url` | Direct download |

**Refs:** `webinput.mjs:6-30` (`toUrl`), `webinput.mjs:36-60` (`classify`)

---

### 3.3 Subsonic / Navidrome API
Base URL: user-configured server address (stored in settings `[Streaming]SubsonicUrl`).
Auth: **Token auth** (Subsonic 1.16.1) — `t=md5(password + salt)`, `s=salt` (hex), `u=username`, `v=1.16.1`, `c=MixxxAndroid`, `f=json`.
All calls: `GET <base>/rest/<endpoint>.view?u=...&t=...&s=...&v=1.16.1&c=MixxxAndroid&f=json&<extra>`.

| Endpoint | Params | Response |
|----------|--------|----------|
| `ping` | — | `{status: "ok", version?, serverVersion?}` |
| `search3` | `query=<urlencoded>` | `searchResult3.song[]` with `id, title, artist, album, duration, suffix, bitRate, size, coverArt` |
| `getPlaylists` | — | `playlists.playlist[]` with `id, name, songCount, duration` |
| `getPlaylist` | `id=<playlistId>` | `playlist.entry[]` (same song fields) |
| `stream` | `id=<songId>` | **302 redirect** to actual audio URL |
| `getCoverArt` | `id=<coverArtId>, size=160` | Image bytes |

Stream URL from `getSubsonicStreamUrl(id)` → used as `streamUrl` in search results.

**Refs:** `qmlstreamingproxy.cpp:151-195` (`buildSubsonicUrl`, `getSubsonicStreamUrl`), `qmlstreamingproxy.cpp:197-246` (`testSubsonicConnection`), `qmlstreamingproxy.cpp:248-316` (`searchSubsonic`), `qmlstreamingproxy.cpp:318-358` (`getSubsonicPlaylists`), `qmlstreamingproxy.cpp:360-421` (`getSubsonicPlaylistSongs`)

---

### 3.4 Audius Discovery
- Search: `GET https://discoveryprovider.audius.co/v1/tracks/search?query=<urlencoded>&app_name=MixxxAndroid`
- Trending: `GET https://discoveryprovider.audius.co/v1/tracks/trending?genre=<urlencoded>&app_name=MixxxAndroid`
- Response: `data[]` with `id, title, user.name, duration, genre, bpm, musical_key, artwork{150x150,480x480}`.
- Stream URL: `https://discoveryprovider.audius.co/v1/tracks/<id>/stream?app_name=MixxxAndroid` (MP3).
- Results include `service: "audius"`, `suffix: "MP3"`.

**Refs:** `qmlstreamingproxy.cpp:423-497` (`searchAudius`), `qmlstreamingproxy.cpp:499-565` (`getAudiusTrending`)

---

### 3.5 Generic Download (`downloadAndLoad`)
Called from `CustomUrlView` (Deck 1/2 buttons) or Subsonic/Audius result taps.

**Parameters:**
- `streamUrl`: direct audio/stream URL (may be redirect).
- `service`: `"custom"`, `"subsonic"`, `"audius"`, etc.
- `trackId`: unique ID for cache key.
- `title`, `artist`: for metadata tags.
- `targetDeck`: `"[Channel1]"` or `"[Channel2]"` (or empty for cache-only).
- `autoPlay`: bool.

**Flow:**
1. Check cache: `getCachedTrackPath(service, trackId)` scans `<cacheDir>/` for filename containing `[service_trackId]` or legacy `service_trackId` prefix. If found & >1 KB → emit progress 1.0 → `slotLoadLocationToPlayer()` → `downloadFinished`.
2. If already downloading (`m_activeDownloads[trackId]`) → return.
3. Determine extension from URL (`.flac`, `.m4a`/`.aac`, `.ogg`, `.wav`, default `.mp3`).
4. Build final filename: `"Title - Artist [service_trackId].ext"` (sanitized).
5. `QNetworkRequest(streamUrl)` + `User-Agent: Mozilla/5.0 (Linux; Android 14; Mobile; Mixxx-DJ) AppleWebKit/537.36` + **ignore SSL errors**.
6. Stream to `.part` file via `readyRead` → write all.
7. `downloadProgress(trackId, progress, received, total)` emitted.
8. On `finished`:
   - Error → remove `.part`, `downloadFailed`.
   - Success → rename `.part` → final → write ID3 tags (Title, Artist) via `MetadataSourceTagLib` → `refreshCacheSize()` → if `targetDeck` set → `Sandbox::createSecurityToken()` → `slotLoadLocationToPlayer()` → `downloadFinished`.

**Refs:** `qmlstreamingproxy.cpp:593-761` (`downloadAndLoad`), `qmlstreamingproxy.cpp:567-591` (`isTrackCached`, `getCachedTrackPath`), `qmlstreamingproxy.cpp:125-149` (`sanitizeFileName`, `buildTrackFileName`)

---

### 3.6 Cache Management
- `cacheDirectory()` returns cache path.
- `cacheSizeBytes()` sums all files in cache dir.
- `clearCache()` deletes all files.
- File naming: `"Display Name [service_trackId].ext"` (legacy: `service_trackId_...`).

**Refs:** `qmlstreamingproxy.h:58-60`, `qmlstreamingproxy.cpp:777-798`

---

### 3.7 UI: WebView.qml (Smart Input) + CustomUrlView.qml

#### WebView (Smart Box)
| Element | Behaviour |
|---------|-----------|
| TextField `smartInput` | Placeholder: "Search your library, or paste a link: Navidrome / Subsonic server, MP3, radio stream...". On `accepted` / GO button → `feed(text)`. |
| Source chips | Two toggle buttons: "NAVIDROME" (cyan, icon) and "URL / RADIO" (yellow). Tap → switches `source` index, re-classifies current text. |
| Hint label | Shows context: "Searching your Navidrome...", "Connect a Navidrome server first...", "Audio file or stream: pick a deck". |
| StackLayout | Index 0: `SubsonicView` (separate component). Index 1: `CustomUrlView`. |

**Refs:** `WebView.qml:33-54` (`feed`), `WebView.qml:56-164` (UI)

#### CustomUrlView (Direct URL / Radio)
| Element | Behaviour |
|---------|-----------|
| Header bar | Yellow "🔗 DIRECT URL / WEB STREAMING" + status message. |
| URL field | `TextField` with placeholder "https://example.com/dj-set.mp3", default example freesound URL. |
| Title field | Optional "Optional title (e.g. Future Rave Set)". |
| Load buttons | "Load into DECK 1" (left arrow, deck1 color) → `downloadAndLoad(url, "custom", "stream_<timestamp>", title||"Web Stream Audio", "Web Stream", "[Channel1]", false)`. Same for DECK 2. |
| Download progress | Connected to `Mixxx.StreamingManager.downloadProgress/Finished/Failed` → status message updates. |

**Refs:** `CustomUrlView.qml:11-39` (signals), `CustomUrlView.qml:98-182` (UI), `CustomUrlView.qml:142-181` (button handlers)

---

### 3.8 Errors / Edge Cases
- Invalid URL / empty trackId → "Invalid URL or track ID".
- Cannot create temp file → "Cannot create the temporary cache file".
- Network error → `downloadFailed` with `reply->errorString()`.
- SSL errors ignored (self-signed certs on local streams).
- Redirects followed (default Qt redirect policy `NoLessSafeRedirectPolicy`).
- Subsonic: empty server URL → "Set up the Navidrome/Subsonic URL".
- Subsonic auth failure → "Authentication or server error".
- Cache scan skips `.part` files and files ≤ 1 KB.

**Refs:** `qmlstreamingproxy.cpp:601-604`, `qmlstreamingproxy.cpp:650-666`, `qmlstreamingproxy.cpp:719-724`

---

## 4. Cross-Cutting Behaviours

### 4.1 Deck Loading (Common)
- All three paths ultimately call `PlayerManager::slotLoadTrackToPlayer()` (SoundCloud/YouTube) or `slotLoadLocationToPlayer()` (Spotify cached .ogg, Custom URL).
- Security token: `Sandbox::createSecurityToken(&FileInfo)` before load (Android scoped storage).
- Auto-play: passed through from UI (default `false` for streaming loads).

**Refs:** `qmlyoutubeproxy.cpp:86-92`, `qmlspotifyproxy.cpp:528-533`, `qmlstreamingproxy.cpp:611-617`, `qmlstreamingproxy.cpp:750-757`

### 4.2 Background Threading
- SoundCloud search: `QNetworkAccessManager` on Qt network thread.
- Spotify browse/download: `QThreadPool::globalInstance()` → Rust `std::thread` (large stack 16 MB) → Tokio runtime.
- Custom URL download: `QNetworkAccessManager` (async signals).
- yt-dlp: `QProcess` (desktop) or JNI (Android).

### 4.3 Persistent Settings (UserSettings)
| Key | Group | Meaning |
|-----|-------|---------|
| `SubsonicUrl` | `[Streaming]` | Navidrome/Subsonic base URL |
| `SubsonicUsername` | `[Streaming]` | Username |
| `SubsonicPassword` | `[Streaming]` | Password (stored plaintext in config) |
| `Spotify/userName` | `QSettings` | Last logged-in display name |
| `soundcloud/account` file | `AppDataLocation/soundcloud/` | OAuth token |

**Refs:** `qmlstreamingproxy.cpp:24-26`, `qmlstreamingproxy.cpp:55-61`, `soundcloudaccount.h:14-19`, `qmlspotifyproxy.cpp:38-39`

---

## 5. File:Line Reference Index

### SoundCloud
- `qmlyoutubeproxy.h:76-82` — SoundCloud invokables
- `qmlyoutubeproxy.cpp:38-41` — Startup token load
- `qmlyoutubeproxy.cpp:512-548` — Client ID fetch
- `qmlyoutubeproxy.cpp:530-572` — `fetchSoundCloudMe`
- `qmlyoutubeproxy.cpp:575-585` — `soundCloudSearch`
- `qmlyoutubeproxy.cpp:587-599` — `soundCloudLikes`
- `qmlyoutubeproxy.cpp:602-609` — `soundCloudMore`
- `qmlyoutubeproxy.cpp:611-736` — `soundCloudRequest` + response parsing
- `qmlyoutubeproxy.cpp:757-820` — Sign in/out
- `soundcloudaccount.h:11-39` — Token file helpers
- `youtubeservice.cpp:420-431` — `isSoundCloudId`, `mediaUrlForId`
- `youtubeservice.cpp:3220-3313` — yt-dlp download with OAuth
- `SoundCloudView.qml:14-88` — Properties, search/likes/more, controller
- `SoundCloudView.qml:102-128` — Signal handlers
- `SoundCloudView.qml:151-400` — UI layout

### Spotify
- `lib.rs:1-58` — Constants, config, cache, session
- `lib.rs:285-430` — PKCE login flow
- `lib.rs:462-544` — Audio download + decrypt
- `lib.rs:546-763` — C API (`spotify_login_web`, `spotify_browse`, `spotify_download`, `spotify_logout`, `spotify_cancel_login`, `spotify_login_redirect`, `spotify_logged_in`)
- `browse.rs:1-125` — Track/playlist JSON, context pagination
- `browse.rs:126-324` — Request handlers (search, liked, playlists, playlist)
- `qmlspotifyproxy.h:19-93` — Proxy interface
- `qmlspotifyproxy.cpp:59-67` — `accountDir`
- `qmlspotifyproxy.cpp:103-148` — `login` (thread + JNI/Android)
- `qmlspotifyproxy.cpp:151-193` — `cancelLogin`, `logout`, `onLoginResult`, `fetchProfile`
- `qmlspotifyproxy.cpp:206-290` — `browse`, `fetchList`
- `qmlspotifyproxy.cpp:293-314` — `search`, `likedSongs`, `playlists`, `playlistTracks`, `loadMore`
- `qmlspotifyproxy.cpp:323-356` — `loadToDeck` (cache check, pending loads)
- `qmlspotifyproxy.cpp:358-428` — `downloadTrack`, `onDownloaded`, `loadFromYouTube`
- `qmlspotifyproxy.cpp:507-534` — `loadFile` (tags + deck load)
- `SpotifyView.qml:12-99` — Properties, navigation, controller
- `SpotifyView.qml:105-176` — Signal handlers
- `SpotifyView.qml:178-499` — UI (logged-out / logged-in views)

### Custom URL / Web
- `webinput.mjs:1-61` — `toUrl`, `classify`
- `WebView.qml:9-54` — `feed`, classification routing
- `WebView.qml:56-164` — Smart input UI
- `CustomUrlView.qml:11-39` — StreamingManager signals
- `CustomUrlView.qml:98-182` — URL/title fields, deck buttons
- `qmlstreamingproxy.h:31-60` — Proxy interface + properties
- `qmlstreamingproxy.cpp:31-52` — Constructor (cache dir)
- `qmlstreamingproxy.cpp:125-149` — Filename sanitization + building
- `qmlstreamingproxy.cpp:151-195` — Subsonic URL building + stream/cover URLs
- `qmlstreamingproxy.cpp:197-246` — `testSubsonicConnection`
- `qmlstreamingproxy.cpp:248-316` — `searchSubsonic`
- `qmlstreamingproxy.cpp:318-358` — `getSubsonicPlaylists`
- `qmlstreamingproxy.cpp:360-421` — `getSubsonicPlaylistSongs`
- `qmlstreamingproxy.cpp:423-497` — `searchAudius`
- `qmlstreamingproxy.cpp:499-565` — `getAudiusTrending`
- `qmlstreamingproxy.cpp:567-591` — Cache lookup
- `qmlstreamingproxy.cpp:593-761` — `downloadAndLoad` (full flow)
- `qmlstreamingproxy.cpp:763-798` — `cancelDownload`, `cacheSizeBytes`, `clearCache`

---

## 6. Reimplementation Checklist (Rust)

- [ ] SoundCloud: client ID scrape → api-v2 search/likes → `sc-` IDs → yt-dlp with OAuth token → cache → deck load
- [ ] Spotify: PKCE login (port 5588) → librespot session → spclient browse (search/liked/playlists) → Ogg Vorbis 320 download + decrypt → `.ogg` cache → deck load → YouTube fallback
- [ ] Custom URL: `webinput.mjs` classification logic → Subsonic REST (token auth) / Audius / generic HTTP → streaming download with progress → ID3 tag write → cache → deck load
- [ ] Unified cache management (per-service subdirs, size accounting, clear)
- [ ] Android: JNI bridges for Spotify login activity, yt-dlp-android, AccountLoginActivity (SoundCloud)
- [ ] Settings persistence (Subsonic URL/user/pass, Spotify credentials via librespot cache, SoundCloud token file)
- [ ] Controller navigation (browse up/down, load to deck 1/2) mapped to hardware MIDI
- [ ] Error surfaces: user-facing messages for each failure mode above