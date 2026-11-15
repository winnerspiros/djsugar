## ADDED Requirements

### Requirement: Buy from Wanted
Selecting one or more Wanted tracks and choosing Buy… SHALL search every store that can be checked and show, per track, the stores that have it, ranked artist-direct first, then by how fast the file reaches the library, then lossless first; stores that can only be searched SHALL follow. Store links SHALL carry no affiliate or referral tags. Opening a store group SHALL open its track pages (or its search page when no track link is known) one at a time, with Next for the following track; clicking a track SHALL open only its page. DJ Sugar SHALL never take payment.

#### Scenario: Several tracks on Bandcamp
- **WHEN** three Wanted tracks are bought and all are found on Bandcamp
- **THEN** Open shows the first Bandcamp track page and Next the second and third, so the store's cart can hold them for one checkout

#### Scenario: Only on Boomkat
- **WHEN** an ambient track is found on Boomkat and Bandcamp but not on Beatport
- **THEN** Bandcamp is first, Boomkat second, and Beatport only appears as "Try on Beatport"

#### Scenario: No track link
- **WHEN** a track has no known page on Traxsource
- **THEN** Traxsource's search page opens filled with artist and title

### Requirement: Pending purchases
Wanted rows sent to a store SHALL be marked as pending with the store and time, and the mark SHALL clear when a matching file arrives or after 7 days.

#### Scenario: Expired
- **WHEN** a pending row sees no matching file for 7 days
- **THEN** the pending mark is cleared and the row stays on Wanted

### Requirement: Match purchases to Wanted
A file arriving in any inbox folder SHALL be matched to pending Wanted rows by ISRC, then store tags, then artist, title and length within 2 seconds. A match SHALL add the track to Inbox and remove its Wanted row. An unmatched file SHALL still be added to Inbox.

#### Scenario: Matched by ISRC
- **WHEN** a file with the ISRC of a pending Wanted row lands in Downloads
- **THEN** the track is in Inbox and no longer on Wanted

### Requirement: Purchase inbox off until turned on
The purchase inbox SHALL be off until the DJ turns it on (on the Buy sheet), and SHALL take only files that arrive after it was last turned on.

#### Scenario: Turned on later
- **WHEN** a zip was in Downloads before the inbox was turned on
- **THEN** it is left alone

### Requirement: Music app purchases
On macOS the inbox SHALL optionally watch the Music app's media folder, its Artist/Album subfolders included. From it, only files matching Wanted tracks being bought SHALL be taken, and they SHALL be added where they are, never moved or copied.

#### Scenario: The DJ's own music
- **WHEN** a song the DJ imported into the Music app appears in its folder and isn't being bought
- **THEN** DJ Sugar leaves it alone
