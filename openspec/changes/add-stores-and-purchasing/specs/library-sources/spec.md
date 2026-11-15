## ADDED Requirements

### Requirement: Store sources
Bandcamp SHALL search in the app and list results with price and format where the store shows them; choosing Buy SHALL open the item's page in the browser. Shopping SHALL offer other stores' own search pages filled with the search. Each store source SHALL be switched on and off in Settings › Sources and SHALL be hidden on a fresh install. DJ Sugar SHALL never take payment, and store links SHALL carry no affiliate or referral tags.

#### Scenario: Buy on Bandcamp
- **WHEN** a Bandcamp result's Buy is clicked
- **THEN** its Bandcamp page opens in the browser

#### Scenario: Fresh install
- **WHEN** DJ Sugar runs for the first time
- **THEN** no store source shows in the rail and no file in Downloads is touched

### Requirement: Buy any track without a file
Buy… SHALL be offered on any track row that has no collection match (streaming lists, list placeholders from imports, Up Next and Wanted) and SHALL open the Buy sheet with the same store ranking as Buy from Wanted. A purchase started outside Wanted SHALL add the track to Wanted as being bought.

#### Scenario: From a Spotify playlist
- **WHEN** Buy… runs on three Spotify playlist tracks not in the collection
- **THEN** the Buy sheet lists the stores that have each, artist-direct first, and the three are on Wanted as being bought

### Requirement: Cross-links
Store track pages SHALL be found by ISRC through MusicBrainz, then Odesli, then each store's own lookup, and SHALL fall back to the store's search page. Prices SHALL be shown where a lookup gives one.

#### Scenario: Found through Odesli
- **WHEN** MusicBrainz has no Bandcamp link for a track and Odesli has one
- **THEN** the Buy sheet opens the Bandcamp track page, not a search

### Requirement: Match to Wanted
An Inbox track that arrived without matching a Wanted row SHALL offer Match to Wanted…, listing Wanted rows by artist and title similarity; picking one SHALL remove it from Wanted.

#### Scenario: Matched by hand
- **WHEN** a purchase arrives with different title spelling and the DJ picks its Wanted row
- **THEN** the row leaves Wanted and the track stays in Inbox

### Requirement: Purchase inbox folders
The purchase inbox SHALL watch Downloads and any further folders the DJ adds, each set to move, copy or leave files in place, taking only files newer than when the inbox was turned on.

#### Scenario: Qobuz downloader folder
- **WHEN** the Qobuz download folder is added and an album arrives in it
- **THEN** its tracks are in Inbox

### Requirement: Just bought
Tracks that arrived from a purchase SHALL appear in a Just bought list in Inbox, analysed and tagged, until moved on from Inbox.

#### Scenario: Purchase lands
- **WHEN** a purchase arrives through the inbox
- **THEN** it is in Just bought with tempo and key

### Requirement: Artist-direct store add-ons
Each artist-direct store whose terms and search allow it SHALL be a store add-on whose results rank as artist-direct.

#### Scenario: Mirlo
- **WHEN** the Mirlo add-on is installed and a track is on Mirlo and Beatport
- **THEN** Mirlo is offered before Beatport

### Requirement: Beatport cart
With Beatport partner access and the DJ signed in, Buy… SHALL offer Add to Beatport cart for the tracks found there and Checkout SHALL open beatport.com with them in the cart; purchased tracks SHALL be pulled from the Beatport library into the inbox. Without partner access Beatport SHALL offer search links only.

#### Scenario: No partner access
- **WHEN** Beatport partner access has not been granted
- **THEN** Beatport appears only as "Search on Beatport"
