# DJ Sugar Streaming Hub — Exact Behavior Specification

> Source: `/home/ubuntu/djsugar/res/qml/Library/Streaming/{StreamingHub.qml,StreamingControl.qml,SubsonicView.qml,OnlineStreamView.qml}`, `/home/ubuntu/djsugar/src/qml/qmlstreamingproxy.{cpp,h}`
> Hub entry points: `+android/Library.qml:286`, `Skins/Booth/BoothBrowse.qml:850`, `QuickLoadDrawer.qml:199`

---

## 1. Hub Shell — `StreamingHub.qml`

### 1.1 Properties
| Property | Type | Default | Meaning |
|---|---|---|---|
| `targetDeck` | string | `"[Channel1]"` | Deck group to load into; toggled by clicking the deck indicator in the selector bar |
| `activePlatform` | int | `0` | Index of visible platform view (0=Navidrome/URL/Radio, 1=Audius, 2=YouTube, 3=SoundCloud, 4=Spotify) |
| `acceptsController` | bool | `false` | Enables controller mapping when true and hub is visible |
| `platformViews` | var | `[webView, onlineStreamView, youTubeView, soundCloudView, spotifyView]` | Ordered view components |

### 1.2 Platform Selector Bar (lines 40–225)
- **Container**: `Rectangle` height 26, `Theme.backgroundColor`, border `Theme.darkGray3`
- **Tabs (left→right, fixed widths)**:
  1. **NAVIDROME · URL · RADIO** (196px) — active `#00e5ff`, inactive `#80d8ff`, icon `ic_library_navidrome.svg` (18×18)
  2. **AUDIUS** (96px) — active `#ea80fc`, inactive `#e1bee7`, icon `ic_library_audius.svg`
  3. **YOUTUBE** (96px) — active `#ff5252`, inactive `#ff8a80`, inline red rounded rect + white `Skin.Triangle` (6×7, pointsRight)
  4. **SOUNDCLOUD** (128px) — active `#ff7733`, inactive `#ffab80`, icon `ic_library_soundcloud.svg` (20×20)
  5. **SPOTIFY** (104px) — active `#1ed760`, inactive `#a7e8bd`, icon `ic_library_spotify.svg` (18×18)
- **Deck Target Indicator** (right side, 90×20):
  - Blue (`#0d47a1`/`#00b0ff`) for `[Channel1]` → "DECK 1"
  - Orange (`#bf360c`/`#ff6d00`) for `[Channel2]` → "DECK 2"
  - Click toggles `targetDeck` between `[Channel1]` and `[Channel2]`

### 1.3 Platform Views Container (lines 228–270)
Each view fills parent, `visible` bound to `activePlatform` index:
- `webView` (WebView) — index 0, gets `targetDeck`
- `onlineStreamView` (OnlineStreamView) — index 1
- `youTubeView` (YouTubeView) — index 2, gets `targetDeck`
- `soundCloudView` (SoundCloudView) — index 3, gets `targetDeck`
- `spotifyView` (SpotifyView) — index 4, gets `targetDeck`

> **Note**: `YouTubeView`, `SoundCloudView`, `SpotifyView` are referenced but not found in the current QML tree; they are likely stubs or external components.

### 1.4 Controller Mapping (embedded `StreamingControl`, lines 17–33)
Enabled when `root.visible && root.acceptsController`. Forwards:
- `moveTab(offset)` → cycles `activePlatform` modulo 5
- `moveResult(offset)` → calls `controllerMove(offset)` on active view if it has the function
- `loadResult(group, play)` → calls `controllerLoad(group || targetDeck, play)` on active view

---

## 2. Controller Adapter — `StreamingControl.qml`

### 2.1 Focus Modes
- `tabsFocused` (line 16): true when `focusedWidgetControl.value == Skin.FocusedWidgetControl.WidgetKind.Sidebar`
- Two modes: **tab switching** (BROWSE knob while sidebar focused) and **result navigation** (BROWSE knob while results focused)

### 2.2 Signals
| Signal | Payload | Emitted by |
|---|---|---|
| `moveTab(int offset)` | +1/−1 | BROWSE knob when `tabsFocused` |
| `moveResult(int offset)` | +1/−1 | BROWSE knob when not `tabsFocused`; `SelectTrackKnob` |
| `loadResult(string group, bool play)` | deck group, autoplay flag | `GoToItem`, `LoadSelectedIntoFirstStopped`, per-deck `LoadSelectedTrackHandler` |

### 2.3 ControlProxy Bindings (lines 39–119)
| Mixxx Control | Group | Action |
|---|---|---|
| `MoveVertical` | `[Library]` | `move(value)` |
| `MoveUp` | `[Library]` | `move(-1)` on non-zero |
| `MoveDown` | `[Library]` | `move(+1)` on non-zero |
| `SelectTrackKnob` | `[Playlist]` | `showResults()` + `moveResult(value)` |
| `GoToItem` | `[Library]` | if `tabsFocused` → `showResults()` else `loadResult("", false)` |
| `LoadSelectedIntoFirstStopped` | `[Playlist]` | `loadResult("", false)` |
| `LoadSelectedTrack` | `[ChannelN]` (Instantiator over `num_decks`) | `loadResult("[ChannelN]", play)` |

`showResults()` sets `focusedWidgetControl.value = Skin.FocusedWidgetControl.WidgetKind.LibraryView`.

---

## 3. Subsonic/Navidrome View — `SubsonicView.qml`

### 3.1 Properties
| Property | Type | Default |
|---|---|---|
| `showSettings` | bool | `!Mixxx.StreamingManager.isSubsonicConnected` |
| `activeMode` | string | `"search"` (values: `"search"`, `"playlists"`, `"playlist_tracks"`) |
| `tracksModel` | var | `[]` |
| `playlistsModel` | var | `[]` |
| `currentPlaylistName` | string | `""` |
| `statusMessage` | string | `""` |
| `isBusy` | bool | `false` |
| `downloadProgressMap` | var | `{}` |

### 3.2 Signals from `Mixxx.StreamingManager` (Connections, lines 37–114)
| Signal | Handler |
|---|---|
| `subsonicConnectionTested(success, message, version)` | Sets `isBusy=false`, `statusMessage` with ✔/❌ prefix; on success hides settings drawer |
| `subsonicSearchFinished(success, songs, errorString)` | On success: `tracksModel=songs`, `activeMode="search"`, status with count |
| `subsonicPlaylistsFinished(success, playlists, errorString)` | On success: `playlistsModel=playlists`, `activeMode="playlists"`, status with count |
| `subsonicPlaylistSongsFinished(success, songs, errorString)` | On success: `tracksModel=songs`, `activeMode="playlist_tracks"`, status with count + playlist name |
| `downloadProgress(trackId, progress, bytesReceived, bytesTotal)` | Updates `downloadProgressMap[trackId]=progress` |
| `downloadFinished(trackId, localPath, targetDeck, autoPlay)` | Removes from progress map; marks `isCached=true` in `tracksModel`; status "✔ Track ready and loaded into Deck N/cache" |
| `downloadFailed(trackId, errorMessage)` | Removes from progress map; status "❌ Download error: …" |

### 3.3 UI Layout (ColumnLayout)

#### 3.3.1 Header & Status Bar (lines 121–180)
- Height 33, dark bg `#1c202a`, border `#282d3b`
- Left: "☁️ NAVIDROME / SUBSONIC" bold 11px `#00e5ff`
- Connection pill: green (`#00e676`) when connected "● Connected", red when not "● Disconnected"
- Status message text (elided, max 280px)
- Settings toggle button: "⚙ Settings" / "▲ Hide", toggles `showSettings`

#### 3.3.2 Settings Drawer (lines 184–302)
Animated height (200ms), visible when `showSettings`, bg `#141720`
- **Row 1**: Server URL field (placeholder "http://192.168.1.50:4533", bound to `StreamingManager.subsonicUrl`, saves on `editingFinished`)
- **Row 1**: Username field (bound to `subsonicUsername`)
- **Row 1**: Password field (Password echo, bound to `subsonicPassword`)
- **Row 1**: Connect button — copies fields to manager, sets `isBusy=true`, calls `testSubsonicConnection()`
- **Row 2**: Hint text listing compatible servers
- **Row 2**: "🧹 Clear cache" button → `StreamingManager.clearCache()`

#### 3.3.3 Search & Navigation Toolbar (lines 306–377)
Height 33, bg `#181b24`
- "🔍 Search" button → `activeMode="search"`, triggers search if input non-empty
- "📜 Playlists" button → `isBusy=true`, calls `getSubsonicPlaylists()`
- Search `TextField` (placeholder "Search artists, tracks or albums in your cloud…") — `onAccepted` triggers search
- Search icon button (🔍) triggers `searchInput.accepted()`

#### 3.3.4 Playlist Header (lines 381–411)
Visible only in `playlist_tracks` mode, height 34
- Back button (← "Back") → `activeMode="playlists"`
- Playlist name "📁 " + `currentPlaylistName`

#### 3.3.5 Tracks ListView (lines 419–638)
Visible when `activeMode !== "playlists"`
- Delegate height 40, alternating `#171a22` / `#13161e`
- **TrackDrag** (line 459–465): `loadTo` and `fetch` call `StreamingManager.downloadAndLoad(streamUrl, "subsonic", id, title, artist, group, false)`
- Cover art: 40×29, async, sourceSize 96×96, `coverArtUrl` from model
- Title (bold 11px white), Artist (9px yellow)
- Download progress bar (3px `#00e5ff`) at bottom when `downloadProg >= 0`
- Empty placeholder: "🔍 Type an artist…" if connected, else "☁️ Connect your Navidrome…"

#### 3.3.6 Playlists ListView (lines 642–710)
Visible when `activeMode === "playlists"`
- Delegate height 37, 📜 icon, name bold 12px, track count 9px
- "Show tracks" button → sets `currentPlaylistName`, `isBusy=true`, calls `getSubsonicPlaylistSongs(id)`

#### 3.3.7 Loading Overlay (lines 714–748)
Centered 140×29, spinning "🔄" + "Loading…" / "Connecting…" when `isBusy`

### 3.4 Helper Functions
- `search(query)` (line 22): sets input, mode, busy, calls `searchSubsonic`
- `openServer(url)` (line 29): prefills URL field, shows settings, focuses username

---

## 4. Audius / Online Stream View — `OnlineStreamView.qml`

### 4.1 Properties
| Property | Type | Default |
|---|---|---|
| `currentGenre` | string | `"Electronic"` |
| `isCustomUrlMode` | bool | `false` |
| `tracksModel` | var | `[]` |
| `statusMessage` | string | `""` |
| `isBusy` | bool | `false` |
| `downloadProgressMap` | var | `{}` |

### 4.2 Signals from `Mixxx.StreamingManager` (Connections, lines 19–70)
| Signal | Handler |
|---|---|
| `audiusSearchFinished(success, tracks, errorString)` | On success: `tracksModel=tracks`, status with count |
| `audiusTrendingFinished(success, tracks, errorString)` | On success: `tracksModel=tracks`, status with count + genre |
| `downloadProgress` / `downloadFinished` / `downloadFailed` | Same pattern as SubsonicView (updates progress map, marks `isCached`, status messages) |

### 4.3 Initialization
`Component.onCompleted` (line 72): `isBusy=true`, calls `getAudiusTrending("Electronic")`

### 4.4 UI Layout

#### 4.4.1 Header (lines 82–145)
Height 33, bg `#1c202a`
- "🔥 AUDIUS STREAMING (FREE MUSIC)" bold 11px activeColor
- Green "● Free online" pill
- Spacer
- Genre selector: dropdown-style button showing `currentGenre`, on click cycles through `["Electronic","Hip Hop","House","Techno","Drum & Bass","Ambient","Pop","Rock"]` (not explicitly listed but implied), calls `getAudiusTrending(newGenre)`
- "🔍 Search" button → shows search input row
- "⚙ Settings" button (placeholder, no action wired)
- Search row (visible when `isCustomUrlMode`): TextField + "Go" button → calls `searchAudius(text)`

#### 4.4.2 Tracks ListView (lines 250–479)
- Delegate height 42, alternating `Theme.darkGray3` / `Theme.darkGray4`
- **TrackDrag** (lines 286–292): `loadTo`/`fetch` call `downloadAndLoad(streamUrl, "audius", id, title, artist, group, false)`
- Cover art 42×30, sourceSize 96×96, fallback "🔥" emoji
- Title bold 11px, Artist 9px yellow
- Duration (9px `#90a4ae`), BPM (9px `#00e5ff`), Key (9px `#ff8a80`)
- Download progress bar (3px `#ea80fc`) at bottom when downloading
- Cache indicator: "✅ Cached" (9px `#00e676`) when `track.isCached`

---

## 5. QML Streaming Proxy — `qmlstreamingproxy.{h,cpp}`

### 5.1 QML Registration
- `QML_NAMED_ELEMENT(StreamingManager)` + `QML_SINGLETON` (line 47–48)
- Exposed as `Mixxx.StreamingManager` in QML

### 5.2 Stored Settings (persisted via `UserSettings` under group `[Streaming]`)
| Key | Property | Default |
|---|---|---|
| `SubsonicUrl` | `subsonicUrl` | `""` |
| `SubsonicUsername` | `subsonicUsername` | `""` |
| `SubsonicPassword` | `subsonicPassword` | `""` |

Saved on every setter (lines 87–122).

### 5.3 Cache Directory
- **Android**: `<settingsPath or AppDataLocation or CacheLocation>/streaming_cache`
- **Desktop**: `CacheLocation/streaming`
Created on construction (lines 35–52).

### 5.4 Properties (Q_PROPERTY)
| Property | Type | Notify |
|---|---|---|
| `subsonicUrl` | QString | `subsonicUrlChanged` |
| `subsonicUsername` | QString | `subsonicUsernameChanged` |
| `subsonicPassword` | QString | `subsonicPasswordChanged` |
| `isSubsonicConnected` | bool | `subsonicConnectionChanged` |
| `subsonicServerVersion` | QString | `subsonicConnectionChanged` |
| `cacheDirectory` | QString | CONSTANT |
| `cacheSizeBytes` | qint64 | `cacheSizeChanged` |
| `activeDownloadCount` | int | `activeDownloadsChanged` |

### 5.5 Subsonic / Navidrome API (Q_INVOKABLE)

#### `testSubsonicConnection()` (lines 197–246)
- Endpoint: `ping`
- Auth: token = MD5(password + salt), salt = random hex
- URL template: `{base}/rest/{endpoint}.view?u={user}&t={token}&s={salt}&v=1.16.1&c=MixxxAndroid&f=json`
- On success: sets `m_isSubsonicConnected=true`, extracts `serverVersion`, emits `subsonicConnectionTested(true, "Connection established successfully!", version)`
- On failure: emits `subsonicConnectionTested(false, errorString, "")`

#### `searchSubsonic(query)` (lines 248–316)
- Endpoint: `search3` with `query=` param
- Parses `subsonic-response.searchResult3.song[]`
- Returns `QVariantList` of `QVariantMap` with keys:
  `id, title, artist, album, duration, suffix, bitRate, size, coverArt, coverArtUrl, streamUrl, service="subsonic", isCached`

#### `getSubsonicPlaylists()` (lines 318–358)
- Endpoint: `getPlaylists`
- Parses `playlists.playlist[]` → `id, name, songCount, duration`

#### `getSubsonicPlaylistSongs(playlistId)` (lines 360–421)
- Endpoint: `getPlaylist` with `id=`
- Parses `playlist.entry[]` → same track map as search

#### `getSubsonicStreamUrl(songId)` (line 185–187)
- Returns `buildSubsonicUrl("stream", "id="+songId)`

#### `getSubsonicCoverArtUrl(coverArtId, size=160)` (lines 189–195)
- Returns `buildSubsonicUrl("getCoverArt", "id=...&size=...")`

#### `buildSubsonicUrl(endpoint, extraParams)` (lines 151–183)
- Ensures `http://` prefix, strips trailing `/`
- Generates fresh `salt` (random 32-bit → hex) and `token = MD5(password + salt)`
- Returns full URL with `u`, `t`, `s`, `v=1.16.1`, `c=MixxxAndroid`, `f=json`

### 5.6 Audius API (Q_INVOKABLE)

#### `searchAudius(query)` (lines 423–497)
- Empty query → falls back to `getAudiusTrending()`
- URL: `https://discoveryprovider.audius.co/v1/tracks/search?query={enc}&app_name=MixxxAndroid`
- Parses `data[]` → track map with:
  `id` (falls back to `track_id`), `title`, `artist` (from `user.name`), `duration`, `genre`, `bpm` (default 126), `key` (`musical_key`), `coverArtUrl` (from `artwork["150x150"]` or `480x480`), `suffix="MP3"`, `streamUrl="https://discoveryprovider.audius.co/v1/tracks/{id}/stream?app_name=MixxxAndroid"`, `service="audius"`, `isCached`

#### `getAudiusTrending(genre="Electronic")` (lines 499–565)
- URL: `https://discoveryprovider.audius.co/v1/tracks/trending?genre={enc}&app_name=MixxxAndroid`
- Same parsing as search

### 5.7 Track Cache & Download Engine

#### `isTrackCached(service, trackId)` (line 567–569)
- Returns `!getCachedTrackPath(service, trackId).isEmpty()`

#### `getCachedTrackPath(service, trackId)` (lines 571–591)
- Marker pattern: `[{service}_{safeTrackId}]` (safe = sanitized)
- Legacy prefix: `{service}_{safeTrackId}`
- Scans `m_cacheDir` for files (ignoring `.part`), returns first match with size > 1024 bytes

#### `downloadAndLoad(streamUrl, service, trackId, title, artist, targetDeck, autoPlay)` (lines 593–761)
1. **Cache hit**: if `getCachedTrackPath` returns path → emits `downloadProgress(1.0)`, creates security token, queues `PlayerManager::slotLoadLocationToPlayer(cachedPath, targetDeck, autoPlay)` on main thread, emits `downloadFinished`, returns
2. **Already downloading**: if `m_activeDownloads.contains(trackId)` → returns
3. **Extension detection**: from URL (`.flac`, `.m4a`/`.aac`, `.ogg`, `.wav`, default `.mp3`)
4. **Filename**: `buildTrackFileName(service, trackId, title, artist, ext)` → `"{Artist - Title} [{service}_{trackId}].{ext}"` (sanitized)
5. Creates `.part` file, starts `QNetworkAccessManager::get(streamUrl)` with:
   - `NoLessSafeRedirectPolicy`
   - User-Agent: `Mozilla/5.0 (Linux; Android 14; Mobile; Mixxx-DJ) AppleWebKit/537.36`
   - SSL errors ignored
6. **Progress**: `downloadProgress` signal on `downloadProgress` (qreal 0–1, bytesReceived, bytesTotal)
7. **ReadyRead**: appends to `.part` file
8. **Finished**:
   - On error: removes `.part`, emits `downloadFailed(trackId, errorString)`
   - On success: renames `.part` → final, writes ID3 tags (Title, Artist) via `MetadataSourceTagLib`, emits `refreshCacheSize()`
   - If `targetDeck` non-empty: creates security token, queues `slotLoadLocationToPlayer(finalPath, targetDeck, autoPlay)`
   - Emits `downloadFinished(trackId, finalPath, targetDeck, autoPlay)`

#### `cancelDownload(trackId)` (lines 763–775)
- Aborts reply, closes file, removes `.part`, removes from `m_activeDownloads`, emits `activeDownloadsChanged`

#### `cacheSizeBytes()` (lines 777–785)
- Sums size of all files in `m_cacheDir`

#### `clearCache()` (lines 791–798)
- Removes all files in `m_cacheDir`, emits `cacheSizeChanged`

#### `refreshCacheSize()` (line 787–789)
- Emits `cacheSizeChanged`

### 5.8 Signals (Q_SIGNALS)
| Signal | Payload |
|---|---|
| `subsonicConnectionTested` | `bool success, QString message, QString serverVersion` |
| `subsonicSearchFinished` | `bool success, QVariantList songs, QString errorString` |
| `subsonicPlaylistsFinished` | `bool success, QVariantList playlists, QString errorString` |
| `subsonicPlaylistSongsFinished` | `bool success, QVariantList songs, QString errorString` |
| `audiusSearchFinished` | `bool success, QVariantList tracks, QString errorString` |
| `audiusTrendingFinished` | `bool success, QVariantList tracks, QString errorString` |
| `downloadProgress` | `QString trackId, qreal progress, qint64 bytesReceived, qint64 bytesTotal` |
| `downloadFinished` | `QString trackId, QString localPath, QString targetDeck, bool autoPlay` |
| `downloadFailed` | `QString trackId, QString errorMessage` |
| `subsonicUrlChanged` / `subsonicUsernameChanged` / `subsonicPasswordChanged` | — |
| `subsonicConnectionChanged` | — |
| `cacheSizeChanged` | — |
| `activeDownloadsChanged` | — |

### 5.9 Sanitization & Naming
- `sanitizeFileName(input)` (lines 125–130): replaces `[\\/:*?"<>|]` and control chars with `_`, trims
- `buildTrackFileName(service, trackId, title, artist, ext)` (lines 132–149): `"{Artist - Title} [{service}_{trackId}]{ext}"` (falls back to title only, then `service_trackId`)

---

## 6. Hub Open / Close Behavior

### 6.1 Android Library (`+android/Library.qml:286–291`)
```qml
StreamingComponent.StreamingHub {
    id: streamingHubArea
    anchors.fill: parent
    visible: root.librarySourceMode === 1
    acceptsController: root.visible && root.acceptsController
}
```
- Shown when `librarySourceMode == 1` (streaming mode)
- Controller enabled when library visible + acceptsController

### 6.2 Booth Skin (`Skins/Booth/BoothBrowse.qml:842–852`)
```qml
Loader {
    anchors.bottom: parent.bottom
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.top: header.bottom
    active: root.streaming
    visible: root.streaming
    sourceComponent: StreamingComponent.StreamingHub { }
}
```
- Loaded/unloaded by `root.streaming` boolean

### 6.3 QuickLoadDrawer (`QuickLoadDrawer.qml:199–203`)
```qml
StreamingComponent.StreamingHub {
    anchors.fill: parent
    targetDeck: root.targetDeck
    visible: root.activeTab === 1
}
```
- Shown when drawer's `activeTab == 1` (second tab)
- Receives `targetDeck` from drawer

---

## 7. Track Model Fields (QVariantMap)

### Subsonic / Navidrome
| Key | Type | Source |
|---|---|---|
| `id` | string | `song.id` |
| `title` | string | `song.title` |
| `artist` | string | `song.artist` |
| `album` | string | `song.album` |
| `duration` | int (seconds) | `song.duration` |
| `suffix` | string (uppercase) | `song.suffix` |
| `bitRate` | int | `song.bitRate` |
| `size` | long long | `song.size` |
| `coverArt` | string | `song.coverArt` |
| `coverArtUrl` | string | `getSubsonicCoverArtUrl(coverArt, 160)` |
| `streamUrl` | string | `getSubsonicStreamUrl(id)` |
| `service` | string | `"subsonic"` |
| `isCached` | bool | `isTrackCached("subsonic", id)` |

### Audius
| Key | Type | Source |
|---|---|---|
| `id` | string | `track.id` or `track.track_id` |
| `title` | string | `track.title` |
| `artist` | string | `track.user.name` |
| `duration` | int | `track.duration` |
| `genre` | string | `track.genre` |
| `bpm` | double | `track.bpm` (default 126) |
| `key` | string | `track.musical_key` |
| `coverArtUrl` | string | `track.artwork["150x150"]` or `["480x480"]` |
| `suffix` | string | `"MP3"` |
| `streamUrl` | string | `https://discoveryprovider.audius.co/v1/tracks/{id}/stream?app_name=MixxxAndroid` |
| `service` | string | `"audius"` |
| `isCached` | bool | `isTrackCached("audius", id)` |

---

## 8. Error Messages (User-Visible)

| Context | Message |
|---|---|
| Subsonic URL empty | "The server URL is empty" |
| Subsonic connection error | Reply `errorString()` or "Authentication or server error" |
| Subsonic search empty query | Returns empty list silently |
| Subsonic search no URL | "Set up the Navidrome/Subsonic URL" |
| Subsonic playlists no URL | "Set up the Navidrome/Subsonic URL" |
| Subsonic playlist songs no URL | "Set up the Navidrome/Subsonic URL" |
| Audius network error | Reply `errorString()` |
| Download invalid URL/ID | "Invalid URL or track ID" |
| Download cannot create temp file | "Cannot create the temporary cache file" |
| Download failed | Reply `errorString()` |
| Download cannot save | "Could not save the downloaded track" |
| Cache clear | "Streaming cache cleared" (via SubsonicView button) |

---

## 9. Platform Selector Order (Fixed)
`activePlatform` values:
| Index | Platform | Tab Label | Color (active) | Icon |
|---|---|---|---|---|
| 0 | Navidrome / URL / Radio | "NAVIDROME · URL · RADIO" | `#00e5ff` | `ic_library_navidrome.svg` |
| 1 | Audius | "AUDIUS" | `#ea80fc` | `ic_library_audius.svg` |
| 2 | YouTube | "YOUTUBE" | `#ff5252` | Inline red rect + white ▶ |
| 3 | SoundCloud | "SOUNDCLOUD" | `#ff7733` | `ic_library_soundcloud.svg` |
| 4 | Spotify | "SPOTIFY" | `#1ed760` | `ic_library_spotify.svg` |

---

## 10. Controller BROWSE/LOAD Mapping Summary

| Hardware Control | Mixxx Control | Group | Condition | Action |
|---|---|---|---|---|
| Browse knob (turn) | `MoveVertical` | `[Library]` | Sidebar focused | `moveTab(±1)` → switch platform tab |
| Browse knob (turn) | `MoveVertical` | `[Library]` | Results focused | `moveResult(±1)` → navigate results |
| Browse knob (press) | `GoToItem` | `[Library]` | Sidebar focused | `showResults()` → enter results |
| Browse knob (press) | `GoToItem` | `[Library]` | Results focused | `loadResult("", false)` → load to first stopped |
| Load button | `LoadSelectedIntoFirstStopped` | `[Playlist]` | — | `loadResult("", false)` |
| Deck Load button | `LoadSelectedTrack` | `[ChannelN]` | — | `loadResult("[ChannelN]", play)` |

---

## 11. File References Quick Index

| File | Key Sections |
|---|---|
| `StreamingHub.qml:1–272` | Hub shell, selector bar, platform views, controller embed |
| `StreamingControl.qml:1–120` | Controller adapter, focus modes, ControlProxy bindings |
| `SubsonicView.qml:1–751` | Settings drawer, search/playlist UI, track/playlist lists, signals |
| `OnlineStreamView.qml:1–520` | Audius header, genre picker, search, track list, signals |
| `qmlstreamingproxy.h:1–185` | Class definition, properties, invokables, signals, job struct |
| `qmlstreamingproxy.cpp:1–801` | All implementations: auth, endpoints, cache, download, metadata |

---

*End of spec*