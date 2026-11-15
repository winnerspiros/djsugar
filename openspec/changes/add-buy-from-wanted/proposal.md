# Proposal

Planned for: **v0.6 · Buy it from the artist**

Status: direction chosen 2026-11-05 (Beatport partner access being requested; no affiliate links; stores ranked by coverage and speed to the library). Built; the remaining steps (Odesli, Match to Wanted…, other store-app folders, Beatport) moved to `add-stores-and-purchasing` for v0.8 on 2026-11-12.

## Why

The Wanted list holds tracks the DJ means to own. Getting from there to a file in the Inbox stage takes many manual steps per track: find the store, find the track, buy, download, unzip, import. Selecting several Wanted tracks should open them for purchase in as few checkouts as the stores allow, and the bought files should arrive in Inbox on their own.

## What Changes

- **Buy… on Wanted rows** (one or many): every store that can be checked is searched at once, and the Buy sheet shows, per track, which stores have it. Among stores that have it, artist-direct comes first, then the store that gets the file into the library fastest.
- **Coverage first**: stores strong on ambient, experimental and independent releases (Bandcamp, Bleep, Boomkat, Qobuz, Apple, Mirlo) are checked alongside Beatport, Juno and Traxsource, so a track missing from one catalogue is still found.
- **Open by store**: track pages open in the browser (or the Music app for Apple) in small batches, so the store's own cart collects them for one checkout. DJ Sugar never takes payment.
- **Track links, not just searches**, where a store id can be found: Bandcamp search, the public iTunes Search API, cross-links by ISRC (MusicBrainz, Odesli). Other stores fall back to their search page.
- **Pending purchases**: the Wanted rows sent to a store are remembered for a while so arriving files can be matched back to them.
- **More inbox folders**: besides Downloads, the inbox can watch store-app download folders (Music app, Qobuz, Amazon), taking only new files.
- **Match on arrival**: ISRC, then store tags, then artist + title + length; a match files the track into Inbox and ticks off its Wanted row. Unmatched files still land in Inbox with "Match to Wanted…".
- Beatport partner cart, if access is granted: "Add N to cart", then Checkout on beatport.com, and purchased tracks pulled from the Beatport library.
- No affiliate or referral tags on any store link.

Builds on the purchase inbox from PR #27. Switching the stores on, Bandcamp account downloads and Buy… outside Wanted are in `add-stores-and-purchasing`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `library-sources`: Buy from Wanted, pending purchases, more inbox folders, match on arrival

## Out of Scope

- Taking payment or storing payment details in the app.
- Automatic purchasing without the DJ checking out on the store.
- DRM-protected downloads or any DRM workaround.
- Affiliate, referral or tracking tags on store links.
- Store sign-in scraping beyond what an add-on's own change covers.

## Impact

`crates/plugins` (links.rs track links, a cross-link lookup, inbox.rs multiple folders and matching), `crates/app` (browser/wanted.rs Buy action and sheet, inbox.rs, settings), library schema (pending purchase rows on `wanted`).
