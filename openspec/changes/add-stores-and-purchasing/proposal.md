# Proposal

Planned for: **v0.8 · From the booth to the studio** (2026-11-12, a user request to put all store and purchase work in one change)

Replaces four changes that were never started (`enable-stores-and-purchase-inbox`, `add-bandcamp-collection-sync`, `add-find-and-buy`, `add-artist-direct-stores`) and `add-addon-repositories` (moved in on a user request the same day), and takes over the steps of `add-buy-from-wanted` that were not built (live lookup checks, Odesli, Match to Wanted…, other store-app folders, Beatport). Issue #358 tracked the built part.

## Why

Buying music should be the easy way to get a track into DJ Sugar, with as much of the money as possible reaching the artist. Much is already built but hidden or half-joined:

- Bandcamp search, store links, the wanted list and the purchase inbox are in the code (PR #27), but the Bandcamp and Beatport sources still say "Coming soon" (`STORES_ENABLED = false`).
- Buy… on Wanted works (PR #359, #422, #476): every store is checked, ranked by who has the track, artist-direct, speed to the library and lossless, and arrivals are matched back to Wanted. It only works from Wanted, and an unmatched arrival can't be tied to its Wanted row by hand.
- Bandcamp purchases must still be downloaded by hand, one zip at a time.
- Artist-direct stores other than Bandcamp (Mirlo, Ampwall, Faircamp) are only search links.
- Add-ons run from the add-ons folder (PR #36) but installing one means copying files by hand, so store add-ons can't be offered to most DJs.
- Beatport's cart needs partner access, which has been asked for and not answered.

One change, in one release, makes these a single path: find a track anywhere, buy it on the store's own checkout, and find it analysed in Inbox.

## What Changes

0. **Add-on repositories.** Settings › Add-ons with Installed, Available and Repositories. A repository is one `djsugar-addons.json` at a URL listing each add-on with a download and checksum per platform; the official one is built in, others are marked unsupported. Private GitHub repositories with a read-only token kept in the keychain. Update checks at start and at most daily, never installing while a deck plays. Crashed add-ons restart with a growing delay. Store add-ons (step 6) and download add-ons install from here.
1. **Stores switched on.** The Bandcamp source searches in the app and lists results with price and format where the store shows them; Buy opens the page in the browser. A Shopping row offers the other stores' own search pages. Each store can be switched on and off in Settings › Sources; the purchase inbox stays off until the DJ turns it on.
2. **Buy… everywhere a track has no file** (was Find and buy): Spotify, TIDAL, Apple Music and SoundCloud rows not in the collection, list placeholders from imports, Wanted rows and Up Next, using the same Buy sheet and ranking as Buy from Wanted. Tracks bought from outside Wanted are put on Wanted as being bought, so their arrival is matched the same way.
3. **Better track links.** Odesli (song.link) as a second cross-link source after MusicBrainz, with a short cache; prices shown where the store's lookup gives one.
4. **Arrival.** Match to Wanted… on unmatched Inbox tracks; a setting to add other store-app download folders (Qobuz, Amazon, Beatport's own downloader), taking only new files; a **Just bought** list in Inbox holding what came from a purchase, analysed and tagged, until it is moved on.
5. **Bandcamp collection sync.** Sign in to Bandcamp, list the purchases, download new ones as FLAC (or the format the DJ picks) into the purchase inbox, once each, so they land in Just bought without opening the download page.
6. **More artist-direct stores.** Mirlo, Ampwall and Faircamp sites as store sources where their search or feeds allow it, checked into the same Buy ranking as artist-direct.
7. **Beatport cart (only if partner access is granted).** Sign in, find tracks by ISRC, "Add N to cart" and Checkout on beatport.com, and pull purchased tracks from the Beatport library into the inbox.

## Capabilities

### New Capabilities

- `bandcamp-sync`: bringing a DJ's Bandcamp purchases into the library

### Modified Capabilities

- `add-ons`: Settings › Add-ons with repositories, install, update and remove
- `library-sources`: stores on, Buy… on any track without a file, Odesli links, Match to Wanted…, store-app folders, Just bought, more artist-direct stores, Beatport cart

## Out of Scope

- Taking payment or keeping payment details in DJ Sugar; buying without the DJ checking out on the store.
- Affiliate, referral or tracking tags on any store link.
- DRM-protected downloads, stream ripping or any DRM workaround.
- Streaming playback.
- Sandboxing add-ons (they run with the user's rights; the trust story is shown, not enforced) and third-party audio plugins.
- The Add to Wanted… paste dialog (filling Wanted from text, CSV and links) is planned on its own; Buy… works on what it adds.

## Impact

`crates/plugins` remote.rs and new repository/index code, the Settings › Add-ons page, keychain access; `crates/app` (browser.rs `STORES_ENABLED` split per store, stores.rs, inbox.rs, browser/buy.rs and the Buy sheet on more lists, Settings › Sources and the inbox folders), `crates/plugins` (links.rs, buy.rs ranking, a new odesli.rs, inbox.rs folders and Just bought, bandcamp.rs sign-in and collection), new store add-ons, library schema (purchase source on Inbox rows), user guide pages in `website/book/src/guide`.
