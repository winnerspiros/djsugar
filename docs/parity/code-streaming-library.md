# DJ Sugar (C++ Mixxx Fork) vs djsugar-next (Rust Rewrite) — Streaming & Library Feature Parity

> **Legend**: ✅ DONE · 🟡 PARTIAL · ❌ MISSING · N/A = not applicable
> **PARTIAL** = exact behavioral difference noted in-line.

---

## 1. STREAMING SERVICES

### 1.1 YouTube

| Feature | DJ Sugar (C++) | djsugar-next (Rust) | Status | Notes |
|---------|----------------|---------------------|--------|-------|
| **InnerTube search clients** | 3 clients: ANDROID_VR → TVHTML5_SIMPLY → WEB (hardcoded + remote JSON refresh every 24h from GitHub) | 3 clients: ANDROID_VR → TVHTML5 (PlayStation) → WEB (hardcoded const arrays) | 🟡 PARTIAL | C++ fetches remote config at startup; Rust has no remote update. C++ skips iOS for search (payload too large); Rust keeps it only for player. |
| **InnerTube player/download clients** | 8 clients: ANDROID_VR → iOS → ANDROID → TVHTML5_SIMPLY + 4 TVHTML5 variants (order from yt-dlp + empirical testing) | 6 clients: TVHTML5 (2 versions) → ANDROID_VR → iOS → ANDROID → PLAYSTATION | 🟡 PARTIAL | C++ order puts ANDROID_VR first (no poToken for many videos); Rust puts TVHTML5 first. C++ includes 4 TVHTML5 fallbacks; Rust has 2. |
| **Piped fallback (search + streams)** | Round-robin 10 public instances, 6s timeout each, failover on any error | 2 hardcoded instances (`api.piped.private.coffee`, `pipedapi.ducks.party`), 6s timeout | 🟡 PARTIAL | C++ has 10 instances + dynamic failover; Rust only 2. C++ uses Piped for both search *and* streams; Rust uses Piped only for streams (search is InnerTube-only). |
| **Rate limiting / pacing** | 8 req / 10s sliding window, min 200ms gap, exponential backoff on bot flag (30s → 10m) | 8 req / 10s, 200–500ms jittered gap, exponential backoff (30s → 10m) | ✅ DONE | Logic nearly identical; C++ uses `QElapsedTimer`, Rust uses `Instant`. |
| **poToken (Proof-of-Origin) support** | Android: JNI call to bundled youtubedl-android AAR (Python 3.11 + yt-dlp) returns player+streaming tokens + visitor; Desktop: external yt-dlp binary | Platform-injected `PoTokenSource` fn (`set_po_token_source`); Android expects Kotlin/JNI to call it; no bundled AAR yet | 🟡 PARTIAL | C++ has full Android JNI integration with youtubedl-android AAR; Rust only provides the callback hook — platform layer must supply tokens. |
| **Visitor ID persistence** | Stored in `QSettings` (`YouTube/visitorId`), updated from every InnerTube response | In-memory only (`YouTube.visitor` field), updated from responses; no disk persistence | 🟡 PARTIAL | C++ persists across restarts; Rust loses visitor on restart. |
| **Signature / n-parameter decipher** | Handled transparently by InnerTube clients (no cipher on mobile/TV clients); yt-dlp fallback handles cipher | Same — mobile/TV clients return pre-signed URLs; no cipher logic in Rust | ✅ DONE | Both avoid cipher by using clients that return clear URLs. |
| **Download pipeline order** | 1. InnerTube (all 8 clients, sequential, token mint per video) → 2. Piped (round-robin) → 3. yt-dlp (desktop) / bundled AAR (Android) | 1. InnerTube (6 clients, token from `PoTokenSource`) → 2. Piped (2 instances) → **no yt-dlp fallback** | 🟡 PARTIAL | **Critical gap**: Rust has no yt-dlp fallback at all. C++ tries yt-dlp last on desktop and bundled AAR on Android. |
| **Parallel chunked download** | ≥2 MB → 16 parallel Range requests (8 on C++? wait: code says `kParallelDownloadChunks = 16`) | ≥1 MB (`CHUNK_FROM = 1<<20`) → 16 chunks (`CHUNKS = 16`) | ✅ DONE | Thresholds differ (2 MB vs 1 MB); both use 16 chunks. |
| **SponsorBlock integration** | Prefetch segments on search/download; cut non-music segments during download if enabled | **MISSING** | ❌ MISSING | C++ has `SponsorBlockController` + `YouTubeAudioCutter`; Rust has nothing. |
| **Trending / regional charts** | `fetchTrending(regionCode)` → InnerTube `/browse` with `musicCharts` params; default region GR | **MISSING** | ❌ MISSING | C++ supports country-specific trending; Rust only has search. |
| **Search pagination (continuation)** | InnerTube continuation token extracted from two JSON shapes; `fetchMoreSearchResults` chains pages until `minResults` met | `Page.more` token from `continuation()` parser; `Hub::more()` fetches next page | ✅ DONE | Both handle initial + continuation response shapes. |
| **Result parsing** | `compactVideoRenderer` (ANDROID_VR/TVHTML5) + `videoRenderer` (WEB); skips live/upcoming (no `lengthText`) | Same two renderer types; `parse_search` filters live (`lengthText` missing) | ✅ DONE | Parsing logic equivalent. |
| **Genre inference** | `YouTubeVideoInfo::inferGenre(title, uploader, categoryId)` — keyword + channel heuristics | **MISSING** | ❌ MISSING | C++ infers genre for library display; Rust `Hit` has no genre field. |
| **Android yt-dlp (bundled AAR)** | `youtubedl-android` AAR (Python 3.11 + yt-dlp) called via JNI; runs in background thread; returns file path or error | **MISSING** | ❌ MISSING | Major Android gap. C++ ships zero-dependency yt-dlp; Rust relies on platform `PoTokenSource` only. |
| **Desktop yt-dlp fallback** | Bundled PyInstaller `yt-dlp` binary next to executable; also checks PATH, `/opt/homebrew/bin`, `~/.local/bin` | **MISSING** | ❌ MISSING | Rust has no external-process fallback. |
| **Cookie / auth support** | `QNetworkCookieJar` persisted; Piped/InnerTube can send cookies; yt-dlp uses `--cookies` | No cookie jar; InnerTube requests are stateless beyond visitor ID | 🟡 PARTIAL | C++ maintains cookie session; Rust does not. |
| **Audio format preference** | AAC (m4a) first, then Opus/WebM; `bestaudio` via yt-dlp | AAC (m4a) first, then WebM; `each_source` tries formats in order | ✅ DONE | Both prefer m4a. |
| **Live stream handling** | Detected via missing `lengthText` / `isLive` flag; skipped in search, download fails gracefully | `Hit` requires `seconds` (parsed from `lengthText`); live streams filtered out in `parse_search` | ✅ DONE | Both exclude live from search results. |

---

### 1.2 Spotify

| Feature | DJ Sugar (C++) | djsugar-next (Rust) | Status | Notes |
|---------|----------------|---------------------|--------|-------|
| **Authentication** | OAuth via `librespot` bridge (Android: `SpotifyLoginActivity`; Desktop: browser); token cached in `QSettings` | **MISSING** | ❌ MISSING | No Spotify code in Rust crate. |
| **Browse: search / liked songs / playlists / top tracks** | `spotify_browse` bridge calls (Web API with access token) | **MISSING** | ❌ MISSING | |
| **Audio: Ogg Vorbis 320 kbps download + decrypt** | `spotifybridge` (librespot) downloads/decrypts to cache; loads to deck as local file | **MISSING** | ❌ MISSING | |
| **Premium requirement** | Enforced (librespot needs Premium) | N/A | N/A | |
| **Download progress / caching** | Per-track progress 0..1; cached files reused | N/A | N/A | |

---

### 1.3 SoundCloud

| Feature | DJ Sugar (C++) | djsugar-next (Rust) | Status | Notes |
|---------|----------------|---------------------|--------|-------|
| **Authentication** | OAuth token stored in `AppData/soundcloud/account` (from Android `AccountLoginActivity`); Go+ token enables full-quality via yt-dlp `--username oauth --password <token>` | **MISSING** | ❌ MISSING | Only `soundcloudaccount.h` exists in C++ (token storage); no browsing/download logic found. |
| **Browse / search / streaming** | **Not implemented** (only token helper) | **MISSING** | ❌ MISSING | C++ has token storage only; no SoundCloud service class. |

---

### 1.4 Tidal / Beatport / Others

| Service | DJ Sugar | djsugar-next | Status |
|---------|----------|--------------|--------|
| Tidal | No implementation found | **MISSING** | ❌ MISSING |
| Beatport | No implementation found | **MISSING** | ❌ MISSING |
| Other (Bandcamp, etc.) | No implementation found | **MISSING** | ❌ MISSING |

> **Summary**: C++ has **YouTube (full) + Spotify (full via librespot) + SoundCloud (token only)**. Rust has **YouTube (InnerTube + Piped, no yt-dlp fallback, no SponsorBlock, no trending)** only.

---

## 2. LIBRARY CORE

### 2.1 Database & Schema

| Feature | DJ Sugar (C++) | djsugar-next (Rust) | Status | Notes |
|---------|----------------|---------------------|--------|-------|
| **SQL engine** | SQLite (via `QSqlDatabase`), schema versioned, upgrade path in `TrackCollection::repairDatabase` | **No SQL** — flat files: `tracks.tsv` (per-track metadata), `playlists.tsv`, `pads.txt`, `samples.rs` (embedded) | 🟡 PARTIAL | C++ has full relational schema (tracks, playlists, crates, cues, analyses, directories, hashes); Rust uses TSV + in-memory indices. |
| **Track table** | `TrackDAO` — columns: id, title, artist, album, genre, year, bpm, key, duration, bitrate, samplerate, file paths (multiple locations), playcount, lastplayed, rating, cue points, waveform IDs, analysis IDs, hash, file mtime, hidden flag | `Entry` (in-memory) — source (file/sample), title, artist, folder, key (search fold), loose (fold w/o 'h'); `Saved` (persisted) — grid (bpm+first_beat), locked, 8 cues, 8 colours | 🟡 PARTIAL | C++ stores *all* metadata in DB; Rust splits: search index in `Entry`, runtime metadata in `Saved` (TSV), audio analysis computed on-load. |
| **Crate / Playlist tables** | `CrateStorage` + `PlaylistDAO` — crates (id, name, locked, autoDJ flag), playlist tracks (position, hidden type: normal/autoDJ/setlog), auto-DJ crates DB | `playlists.tsv` — name + tab-separated track keys; **no crates**, **no auto-DJ crates**, **no hidden types** | 🟡 PARTIAL | C++ has rich crate/playlist model; Rust has simple playlist list only. |
| **Cue points** | `CueDAO` — 8 hot cues + loop in/out per track, stored in DB | `Saved.cues: [Option<f64>; 8]` + `Saved.colours: [u8; 8]` in TSV | ✅ DONE | Equivalent; Rust stores colours too. |
| **Waveform / analysis** | `AnalysisDao` — waveform (full) + wavesummary (overview) blobs per track, versioned, disk usage tracking | **Computed on-load** (`sugar_analysis::grid`, `key`); `Loaded` struct holds overview (`Box<[u8]>`) + detail (`Box<[[u8;4]]>`) generated in worker | 🟡 PARTIAL | C++ persists waveforms; Rust recomputes (fast) and caches only in memory. |
| **Directory watch / scanner** | `LibraryScanner` (thread) + `ScannerTask` pool — hashes directories, detects added/removed/moved files, cover art extraction, MusicBrainz tag fetch | `library::scan(roots)` — one-shot walk, no persistence, no hash DB, no move detection, no cover art, no tag fetch | 🟡 PARTIAL | C++ has incremental, background scanner with hash DB; Rust has synchronous full rescan only. |
| **File hash / move detection** | `LibraryHashDAO` — SHA256 of first/last 64 KB + size + mtime; `detectMovedTracks` matches by hash | **MISSING** | ❌ MISSING | C++ reliably tracks moved/renamed files; Rust re-imports as new. |
| **Import / tag reading** | `ImportFilesTask` + `TagLib` (via `TrackLoader`) — reads ID3, Vorbis, FLAC, MP4 tags; `DlgTagFetcher` for MusicBrainz/AcoustID | **MISSING** — `Entry::new` splits `"Artist - Title"` from filename only | ❌ MISSING | C++ reads embedded tags; Rust uses filename convention. |
| **Cover art** | `CoverArtCache` + `CoverArtUtils` — embedded (ID3 APIC, Vorbis METADATA_BLOCK_PICTURE) + folder guess (`cover.jpg`, `folder.jpg`, `front.*`, `album.*`); async, cached, multiple sizes | **MISSING** | ❌ MISSING | No cover art extraction or caching in Rust. |
| **Metadata export** | `TrackMetadataExportThread` — exports tags to file (CSV, etc.) | **MISSING** | ❌ MISSING | |
| **ReplayGain** | `ReplayGain` lib (libreplaygain) — track/album gain/peak stored in DB | **MISSING** | ❌ MISSING | |

---

### 2.2 Playlists & Crates

| Feature | DJ Sugar (C++) | djsugar-next (Rust) | Status |
|---------|----------------|---------------------|--------|
| **Create / rename / delete playlists** | `PlaylistDAO::createPlaylist`, `renamePlaylist`, `deletePlaylist` | `State` holds `playlists: Vec<(String, Vec<String>)>`; `Target::NewPlaylist` action; `store::save_playlists` | 🟡 PARTIAL (Rust: basic CRUD only) |
| **Lock / unlock playlist** | `setPlaylistLocked`, `isPlaylistLocked` | **MISSING** | ❌ MISSING |
| **Hidden playlist types** | `PLHT_NOT_HIDDEN`, `PLHT_AUTO_DJ`, `PLHT_SET_LOG`, `PLHT_UNKNOWN` | **MISSING** | ❌ MISSING |
| **Auto-DJ queue** | `addPlaylistToAutoDJQueue`, `addTracksToAutoDJQueue`, `clearAutoDJQueue`, `AutoDJCratesDAO` | `State.auto: Option<Auto>` (simple next-track fade) | 🟡 PARTIAL (Rust: minimal) |
| **Playlist track order / reorder** | `insertTrackIntoPlaylist`, `orderTracksByCurrPos`, `getTrackIdsInPlaylistOrder` | Append-only via `playlists.tsv` (no position column) | 🟡 PARTIAL |
| **Copy playlist tracks** | `copyPlaylistTracks` | **MISSING** | ❌ MISSING |
| **Crates (collections)** | `CrateStorage` — create/update/delete, add/remove tracks, auto-DJ source flag, crate summaries (count, duration) | **MISSING** | ❌ MISSING |
| **Smart crates / auto-DJ crates** | `AutoDJCratesDAO` — active-tracks view, play-count weighting, ignore-time, random pick | **MISSING** | ❌ MISSING |

---

### 2.3 History

| Feature | DJ Sugar (C++) | djsugar-next (Rust) | Status |
|---------|----------------|---------------------|--------|
| **Played history (set logs)** | Hidden playlists of type `PLHT_SET_LOG`; `PlaylistDAO::tracksRemovedFromPlayedHistory` signal; `TrackDAO::updatePlayCounterFromPlayedHistory` aggregates playcount/lastplayed | `State.history: Vec<u32>` (entry indices loaded in session only) | 🟡 PARTIAL (Rust: session-only, no persistence, no playcount aggregation) |
| **Play count / last played** | Stored in track table, updated from history | **MISSING** | ❌ MISSING |

---

### 2.4 Search & Sorting

| Feature | DJ Sugar (C++) | djsugar-next (Rust) | Status |
|---------|----------------|---------------------|--------|
| **Full-text search** | `ProxyTrackModel::search(QString)` → `QSortFilterProxyModel` filter on all columns | `library::matches(Entry, query)` — word-based, case-insensitive, Greek→Greeklish fold (`fold`), negation (`-term`) | 🟡 PARTIAL (C++: SQL LIKE on all columns; Rust: custom fold + loose match) |
| **Search columns** | Title, Artist, Album, Genre, Year, Track#, Key, BPM, Duration, Label, Composer, etc. | Title, Artist, Folder (via `key`/`loose`) | 🟡 PARTIAL |
| **Sorting** | `TrackModel::SortColumnId` (20+ columns) → `ProxyTrackModel::sort` | `State.sort: (Column, bool)` — 7 columns (Index, Title, Artist, BPM, Key, Time) | 🟡 PARTIAL |
| **Multi-column sort** | Via proxy model | **MISSING** | ❌ MISSING |
| **Persistent sort per view** | `modelKey()` includes sort state | **MISSING** | ❌ MISSING |

---

### 2.5 Track Metadata & Cover Art

| Feature | DJ Sugar (C++) | djsugar-next (Rust) | Status |
|---------|----------------|---------------------|--------|
| **Embedded tag reading** | TagLib (ID3v1/2, Vorbis, FLAC, MP4) — title, artist, album, genre, year, track#, composer, label, BPM, key, cover art | **MISSING** (filename split only) | ❌ MISSING |
| **MusicBrainz / AcoustID lookup** | `DlgTagFetcher` — fingerprint → AcoustID → MusicBrainz → cover art (Cover Art Archive) | **MISSING** | ❌ MISSING |
| **Cover art extraction** | Embedded (APIC, METADATA_BLOCK_PICTURE) + folder guess (`cover`, `folder`, `front`, `album`, `*.jpg/png`) | **MISSING** | ❌ MISSING |
| **Cover art caching** | `CoverArtCache` — async, multi-size, request deduplication | **MISSING** | ❌ MISSING |
| **Key notation** | `Key` enum (Camelot, classical, flat/sharp) | `sugar_analysis::Key` — Camelot (tonic + minor) + `camelot()` string | ✅ DONE |
| **BPM / beat grid** | `libqm-dsp` (TempoTrackV2, BeatSpectrum) → stored in DB | `sugar_analysis::grid` (autocorrelation flux) → computed on-load, cached in `Saved` | 🟡 PARTIAL (C++: persistent; Rust: recomputed) |
| **Waveform / overview** | `AnalysisDao` stores full + summary blobs | Generated in worker (`bands`/`peaks`) → `Loaded.overview` + `Loaded.detail` | 🟡 PARTIAL (C++: persistent; Rust: transient) |
| **Lyrics** | **MISSING** (not found in scanned files) | `Lyrics` struct — LRC parser (timestamps, offset) | ✅ DONE (Rust only) |

---

### 2.6 Analysis Queue

| Feature | DJ Sugar (C++) | djsugar-next (Rust) | Status |
|---------|----------------|---------------------|--------|
| **Background analysis** | `ModalTrackBatchProcessor` + `TrackCollectionManager` — processes tracks modally with progress dialog; analyzes BPM, key, waveform | `State.jobs` channel → worker threads (`Job::Meta`, `Job::Scan`) → `sugar_analysis::grid`/`key` computed per track | 🟡 PARTIAL (C++: modal UI + persistent results; Rust: fire-and-forget, results in `Loaded`) |
| **Analysis persistence** | `AnalysisDao` saves to DB | `Saved` in `tracks.tsv` (grid + lock + cues only); waveform not persisted | 🟡 PARTIAL |
| **Re-analysis** | User-triggered (context menu) | **MISSING** (re-load triggers recompute) | 🟡 PARTIAL |

---

## 3. ANDROID-SPECIFIC GAPS

| Area | DJ Sugar (C++) | djsugar-next (Rust) | Status |
|------|----------------|---------------------|--------|
| **YouTube: bundled yt-dlp (AAR)** | `youtubedl-android` AAR (Python 3.11 + yt-dlp) via JNI; zero external deps | **MISSING** | ❌ CRITICAL |
| **YouTube: poToken via JNI** | JNI call returns player+streaming tokens + visitor | `PoTokenSource` callback hook only; platform must implement | 🟡 PARTIAL |
| **Spotify: Android login** | `SpotifyLoginActivity` (browser tab in-app) | **MISSING** | ❌ |
| **Storage paths** | `QStandardPaths::AppDataLocation` + scoped storage handling | `DJSUGAR_DATA` env / `XDG_DATA_HOME` / `~/Music` + `/storage/emulated/0/{Music,Download}` | 🟡 PARTIAL |
| **USB / mass storage scan** | `AndroidUsbDrive` + `UsbMassStorage` (libusb/SCSI) | **MISSING** | ❌ |
| **Audio output** | `AndroidAudio` (AAudio / OpenSL ES) | `crates/io/src/aaudio.rs` (AAudio) | ✅ DONE |

---

## 4. RANKED TOP 20 GAPS (Most Important First)

| Rank | Gap | Area | Impact |
|------|-----|------|--------|
| 1 | **No yt-dlp fallback (desktop + Android)** | YouTube | InnerTube/Piped fail → no download at all; C++ always succeeds via yt-dlp |
| 2 | **No Spotify support** | Streaming | Major service missing; C++ has full librespot integration |
| 3 | **No cover art extraction / caching** | Library | Visual browsing broken; C++ has embedded + folder + MusicBrainz |
| 4 | **No embedded tag reading (TagLib)** | Library | Metadata from filename only; C++ reads ID3/Vorbis/FLAC/MP4 |
| 5 | **No MusicBrainz / AcoustID lookup** | Library | No auto-tagging or cover art from metadata |
| 6 | **No crate / smart-crate / auto-DJ crate system** | Library | Core DJ workflow (crates, auto-DJ) missing |
| 7 | **No persistent played history / play counts** | Library | No "recently played", no playcount-based auto-DJ |
| 8 | **No incremental library scanner / hash DB** | Library | Full rescan on every start; no move/rename detection |
| 9 | **No waveform persistence** | Library | Recompute overview/detail on every load; C++ caches to DB |
| 10 | **No Android bundled yt-dlp (AAR + JNI)** | YouTube/Android | Android users need Termux yt-dlp or fail; C++ works out of box |
| 11 | **No poToken source implementation (Android)** | YouTube/Android | Hook exists but no Kotlin/JNI provider; many videos need token |
| 12 | **No SponsorBlock integration** | YouTube | Non-music segments (intros, outros) not auto-skipped |
| 13 | **No YouTube trending / regional charts** | YouTube | "Trending in GR/US/BR" tab missing |
| 14 | **No genre inference for YouTube results** | YouTube | Library view lacks genre column for streamed tracks |
| 15 | **No visitor ID persistence** | YouTube | Visitor reset on restart → faster bot detection |
| 16 | **Only 2 Piped instances (vs 10)** | YouTube | Higher failure rate when instances go down |
| 17 | **Only 6 InnerTube player clients (vs 8)** | YouTube | Fewer fallbacks when clients are throttled |
| 18 | **No cookie jar / session persistence** | YouTube | No auth cookies for age-restricted / region-locked content |
| 19 | **No multi-column sort / persistent sort per view** | Library | UX regression vs C++ |
| 20 | **No SoundCloud / Tidal / Beatport services** | Streaming | C++ has SoundCloud token helper; others missing in both |

---

## 5. BEHAVIORAL DIFFERENCES SUMMARY (PARTIAL Items)

| Component | C++ Behavior | Rust Behavior | Consequence |
|-----------|--------------|---------------|-------------|
| **InnerTube client order (search)** | ANDROID_VR → TVHTML5_SIMPLY → WEB | ANDROID_VR → PLAYSTATION → WEB | Rust tries PlayStation before TVHTML5; different failure profiles |
| **InnerTube client order (player)** | ANDROID_VR (no token) → iOS → ANDROID → TVHTML5_SIMPLY → 3×TVHTML5 | TVHTML5 → TVHTML5 → ANDROID_VR → iOS → ANDROID → PLAYSTATION | Rust tries TVHTML5 first (may need token); C++ tries token-free ANDROID_VR first |
| **Piped instances** | 10, round-robin, dynamic failover | 2, static | Rust more brittle |
| **Download threshold for chunking** | 2 MB | 1 MB | Rust chunks earlier (more overhead on small files) |
| **Genre on YouTube results** | Inferred (title/channel/category) | None | Rust library view missing genre |
| **Library persistence** | SQLite (full relational) | TSV + in-memory | Rust loses move detection, rich queries, multi-column indexes |
| **Scanner** | Background, incremental, hash-based, cover art, tag fetch | Foreground, full walk, filename-only | Rust startup slower on large libraries; no metadata enrichment |
| **Analysis persistence** | Waveform blobs in DB | Recomputed on load (fast) | Rust trades disk for CPU; no waveform sharing across sessions |
| **Playlist model** | Position column, hidden types, locking, auto-DJ queue | Flat list, no positions, no types | Rust playlists less flexible |
| **Search** | SQL LIKE on all tag columns | Custom fold (Greek→Latin, loose match) | Rust: no album/genre/year search; C++: heavier but complete |

---

*Generated by code inspection of `/home/ubuntu/djsugar` (C++) and `/home/ubuntu/djsugar-next` (Rust) on 2026-11-23.*