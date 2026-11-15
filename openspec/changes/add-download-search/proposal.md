# Proposal

Planned for: **v0.8 · From the booth to the studio** (2026-11-14, a user request after the yt-dlp add-on was built)

## Why

Download sources are add-ons, one per rail entry: Soulseek today, and YouTube and SoundCloud from the yt-dlp add-on (dj-nahuel/ytdlp-djsugar). Each has its own search, so finding one track means typing it into each source in turn and comparing the results by eye. The sources are good at different things: Soulseek for lossless copies of released music, YouTube for the widest catalog of released songs, SoundCloud for edits, bootlegs and free downloads. A DJ wants to ask once and see the best copy wherever it is.

Wanted has the same gap. Auto-download is set per source, so a track Soulseek can't find is never tried on YouTube, and "Add to Downloads" only routes Spotify links to the one source that takes them.

## What Changes

1. **One search over every download source.** A **Find** entry at the top of the add-on sources in the rail, with the same search bar and cog as a source. A search goes to every installed source that downloads, at once.
2. **Source chips.** Under the search bar, chips for **Any** and each download source by name and icon (Soulseek, YouTube, SoundCloud…). Any is on by default; picking sources narrows the search to them, and the choice stays for the session.
3. **One ranked list.** Results from all sources in one list, each row with a small source chip. Ranked by quality tier first (lossless, then 320 kbps, then 256 kbps, then lower), then by how well artist, title and length match, then by the source's own order. Preview and Download act through the source the row came from; downloads show in each source's Downloads and in one combined Downloads row under Find.
4. **Source order.** Settings › Sources lists the download sources in an order the DJ drags; it breaks ties in ranking and is the order Wanted tries them in.
5. **Wanted tries every source.** Get on Wanted rows (and Wanted's auto-download) searches Find. When the best result meets the Prefer setting (Lossless, MP3 320 or Any) it is downloaded; when no source has one, the row says so and stays on Wanted. A track only found below Prefer can be downloaded by hand from the row's results.
6. **Lossy marked.** A download below 256 kbps, or from a source that says its audio is lossy, carries a Lossy tag in Health, so it can be replaced by a bought copy later.

## Capabilities

### Modified Capabilities

- `library-sources`: a Find search over all download sources, source chips, one ranked list, a source order, Wanted trying every source, lossy downloads marked

## Out of Scope

- Merging the add-ons themselves: each source stays its own add-on with its own sign-in, settings and updates.
- Store sources (Bandcamp, Beatport) in Find: they don't download; Buy… covers them (add-stores-and-purchasing).
- Streaming playback, or any DRM workaround.
- Browsing a source's own playlists (that stays in the source's own rail entry).

## Impact

`crates/app` sources.rs (a Find source fanning out to the download sources and merging their snapshots), the source panel UI (chips, source chip on rows, combined Downloads), browser/wanted.rs (Get and auto-download through Find), Settings › Sources (order), library schema (Lossy flag, or a tag), Health; `crates/plugins` source.rs only if a numeric bitrate on `Found` proves better than reading `quality`. User guide page in `website/book/src/guide`.
