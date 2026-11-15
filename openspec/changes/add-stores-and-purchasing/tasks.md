# Tasks

## 0. Add-on repositories

- [ ] 0.1 Parse and verify `djsugar-addons.json` with signed and unsigned fixtures
- [ ] 0.2 Install, update and remove into `<config>/addons/` with tests
- [ ] 0.3 Settings › Add-ons with Installed, Available and Repositories and the token field with keychain storage; screenshot
- [ ] 0.4 Restart crashed add-ons with backoff and show the reason; test with a crashing stub
- [ ] 0.5 New add-ons page in website/book/src/guide

## 1. Stores on

- [ ] 1.1 Split `STORES_ENABLED` into per-store switches in Settings › Sources (Bandcamp, Beatport, Shopping), hidden on a fresh install; test the inbox default is off
- [ ] 1.2 Bandcamp results in the list with price and format where shown; Buy opens the page
- [ ] 1.3 Check the Bandcamp, MusicBrainz and iTunes lookups against the live services on a machine with network access (from add-buy-from-wanted 1.4)

## 2. Buy… everywhere

- [ ] 2.1 Buy… on streaming rows, import placeholders and Up Next rows without a collection match, using the Buy sheet; tests for which rows offer it
- [ ] 2.2 A purchase started outside Wanted adds the track to Wanted as being bought; test

## 3. Track links

- [ ] 3.1 Odesli as a second cross-link source with a day's cache and sample answers (from add-buy-from-wanted 1.5)
- [ ] 3.2 Prices on the Buy sheet from every lookup that returns one

## 4. Arrival

- [ ] 4.1 Match to Wanted… on unmatched Inbox tracks (from add-buy-from-wanted 3.3)
- [ ] 4.2 Purchase inbox folder list in Settings with Take per folder, new files only (from add-buy-from-wanted 3.4)
- [ ] 4.3 Just bought list in Inbox from a purchase store on the row; test it empties when tracks move on

## 5. Bandcamp sync

- [ ] 5.1 Bandcamp sign-in kept in the keychain; collection listing with recorded responses
- [ ] 5.2 Download new purchases once each into the purchase inbox; de-duplication test
- [ ] 5.3 Sync now and daily sync, spaced downloads, format choice

## 6. More artist-direct stores

- [ ] 6.1 Check Mirlo, Ampwall and Faircamp APIs and terms; note the outcome in design.md
- [ ] 6.2 Store add-ons for those that allow it, ranked as artist-direct

## 7. Beatport (only if partner access is granted)

- [ ] 7.1 Beatport sign-in (OAuth PKCE, loopback callback), catalogue lookup by ISRC (from add-buy-from-wanted 5.1)
- [ ] 7.2 Add selected tracks to the Beatport cart, open checkout (from add-buy-from-wanted 5.2)
- [ ] 7.3 Pull purchased tracks from the Beatport library into the inbox (from add-buy-from-wanted 5.3)

## 8. Docs

- [ ] 8.1 Update the store, Wanted and Inbox pages in website/book/src/guide

## Workflow follow-up

- Archive the change once its last PR is merged, so the main specs take its requirements.
