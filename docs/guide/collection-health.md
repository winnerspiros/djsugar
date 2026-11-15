# Collection Health

**Collection Health**, the cross under Collection in the rail, checks the
whole collection for what needs fixing. **Overview** shows how much of your
collection is clean, with Inbox, In Prep and Play Ready on their own bars, and
the checks grouped into five areas in the order they're best fixed: **Files**,
**Out of place**, **Duplicates**, **Info** and **Prep**. The first area with
work is marked Start here.

An area lists its tracks with a **Problem** and a **Fix** column and a progress
bar. Its bar has:

- **Auto-fix**, for the fixes with one clear answer: relinking a missing file
  to the only file that matches it, merging easy duplicates, and taking the
  junk the clean-up rules know out of names. Undo takes the whole run back.
  On Prep the button reads **Analyze** and queues every track not analysed
  yet.
- **Review**, which goes through the area one track at a time on a card.
  **Return** applies the fix shown, **→** skips, **I** ignores the track for
  that check, **E** opens its Info tab, **Space** plays it in the headphones,
  **⌘Z** (Ctrl+Z) undoes and **Esc** goes back to the list. When the area is
  done, **Next** carries on in the next one with work.

To fix only some tracks from the list, pick them (shift-click for a range,
⌘-click or Ctrl-click for single tracks) and right-click. Tracks a check was
told to leave alone are under **Ignored**, below the areas; **Bring back**
lets every check see them again.

| Check | Finds | Fixes |
|---|---|---|
| Not music | Audiobooks, podcasts and spoken word: Apple Books and Podcasts folders, `.m4b` files, genres such as Audiobook, "(Unabridged)" albums, and chapters (several tracks in one folder sharing a title) | Remove from library, It's music |
| Strays | Files outside your Inbox, In Prep and Play Ready folders | Move to In Prep, Remove from collection |
| Outside folders | Files outside your music folders and stage folders, such as ones that came in with a VirtualDJ import | Move to In Prep, Remove from collection |
| Duplicates | The same track more than once: same artist, title and remix | Auto-fix, Keep this copy, Link as versions (see [Duplicates](#duplicates)) |
| Not analyzed | No tempo or key | Analyze, in the background |
| Missing or poor metadata | No artist, title, album, year or genre, or one that says nothing, like "Unknown Artist", "Untitled" or "Track 01" | Look up online (see [Online lookup](#online-lookup)); Fill from file name… (`Artist - Title.mp3`, shown in the review table before anything is written). The page says what the flagged tracks lack, such as "80 no genre, 12 no artist or title" |
| Low bitrate | Compressed files under 192 kb/s, which can sound dull on a big system | None yet: replace them with a better copy |
| Junk in names | Track numbers, web sites, "(Official Video)", underscores, or a remix or guest in the title | Clean up… (see [Clean-up](#clean-up)), Look up online |
| No cover | No picture in the file's tags or its folder | Find covers online, or one by one from the track's info panel |
| No rating | No stars | Rate them in the list |
| Missing files | The file isn't where your collection expects it | Auto-fix relinks a file found under the same name in your stage folders and music folders, when there is only one; Remove from collection |
| Long tracks | Over 20 minutes: recorded sets and long pieces | Tag as #mix |

**Lists** at the bottom of the page shows up when crates are empty or lists
still hold tracks you removed from your collection. **Tidy up** deletes the
empty crates and takes those tracks out of the lists. Playlists are left
alone, since a new one starts empty.

**Ignore for** in the right-click menu tells a check to leave a track
alone. A track that isn't music is left out of the other checks, so an
audiobook doesn't also show up as missing its cover.

**Remove from collection** keeps the file where it is. The track no longer
shows, and a rescan or a VirtualDJ import doesn't bring it back; adding the
file again does, and so does **Bring back** at the bottom of the page.

Each check is also a search: `is:notmusic`, `is:stray`, `is:outside`, `is:unanalyzed`,
`is:noinfo`, `is:lowbitrate`, `is:junk`, `is:nocover`, `is:unrated`, `is:missing`, `is:long` and
`is:duplicate`.

Covers are looked for in the background, and a file is looked at again
only when it changes. On a Mac, files that iCloud has taken off the disk
are skipped, so the check doesn't download your whole collection.

**Find covers** looks online (Cover Art Archive, iTunes, and Discogs with
a Discogs token in Settings › Collection) for each track, one at a time in the
background, and embeds the largest cover found, scaled down to 1024 pixels
when it is bigger. Tracks it finds nothing for stay in the list: search
those from the track's info panel.

## Clean-up

**Clean up…** on Junk in names (or a right-click on some of its tracks)
shows every change before anything happens: the track, the field, what it
says now and what it becomes. Untick a change to leave it, then **Apply**.
The changes go to your collection and to the files' tags. **Undo** in the bar
or the track menu takes the whole batch back.

| Rule | Example |
|---|---|
| Track numbers | `03 - Road` becomes `Road`; `Lane 8 - Fingerprint` by Lane 8 becomes `Fingerprint` |
| Web sites | `Glue www.free-mp3.com` becomes `Glue` |
| Video words | `Awake (Official Video) [HD]` becomes `Awake` |
| Underscores and spaces | `bicep_glue` becomes `bicep glue` |
| Remix to its field | `Bodhi Mandala (Drumspyder Remix)` puts `Drumspyder Remix` in Remix |
| Featured artists | `Road (feat. Arctic Lake)` by Lane 8 becomes `Road` by `Lane 8 feat. Arctic Lake` |
| Title Case (off) | `CARBON BASED LIFEFORMS` becomes `Carbon Based Lifeforms` |

Each rule can be turned off in Settings › Collection › Health, under
Clean-up. Titles that start with a number, such as `99 Luftballons`, stay
as they are.

**Genres…** at the bottom of the Collection Health page lists every genre with
how many tracks have it. Genres spelled like another but for capitals,
spaces or punctuation (`Electronica` and `electronica`) are lilac. Tick the
ones that mean the same, type the name to keep (the most used one is
filled in), and **Merge**. Undo takes a merge back too.

## Online lookup

**Look up online** on Missing or poor metadata or Junk in names works on
every track listed (or a right-click on some of them). It looks each track
up on MusicBrainz in the background, paced as MusicBrainz asks, and opens
the same review table as Clean-up with what it found: artist, title, album,
year (of the recording's first release) and genre, and with a Discogs token
also the label and style (as the genre). Next to each track is where the
match came from and how sure it is.

To match more of a DJ collection, DJ Sugar searches for the title as tagged
first, then without bracketed parts and guests ("Kerala (Original Mix)
[Ninja Tune]" by "Bonobo feat. X" is also searched as "Kerala" by
"Bonobo"), then by the words in any order. "Unknown Artist", "Track 01" and
the like count as empty: they aren't searched by, and a file called
`Artist - Title.mp3` stands in for them. When the match gives no name, or
nothing is found, the file name's artist and title are offered as
"File name".

Only changes that fill an empty field (or one that says nothing) from a
match at least 80% sure are ticked, and file-name suggestions for an empty
artist or title. A genre that is the artist's rather than the track's is
shown unticked. A change to something the track already has is shown
unticked, so nothing you named yourself is overwritten unless you tick it.
Fields you typed in the track's info panel are never proposed. **Apply**
writes the ticked changes to your collection and the files, and Undo takes
them back.

When it's done, the status line says how many were found (and how many by
their sound), how many aren't on MusicBrainz (often DJ edits, promos and
bootlegs), how many couldn't be looked up because there was no connection
or MusicBrainz was busy (try those again later), and how many are in iCloud
only, so they were looked up by name and not by their sound.

With an **AcoustID key** (free on acoustid.org; paste it in Settings ›
Collection › Health under Online lookup) DJ Sugar recognises tracks by their sound, so
files called `track01.mp3` are found too. Without one, tracks are looked up
by artist and title (from the tags or the file name), and those with
neither are skipped.

**Fill from file name…** opens the review table with the artist and title
each file's name gives, for those that have none. Check them before
**Apply**: a file called `Title - Artist.mp3` gives them the wrong way
round.

## Duplicates

Copies are tracks with the same artist, title and remix, ignoring case,
punctuation and "Original Mix". Under **Duplicates** in the tree, DJ Sugar
sorts them into groups by what tells them apart:

| Group | What it is | What happens |
|---|---|---|
| Same file | The same file, byte for byte, in two places | One is kept |
| Better copy | The same recording, one lossless or at a higher bitrate | The better file is kept |
| Same format | The same recording in the same format, in different folders | One is kept |
| Different lengths | More than 3 seconds apart: an edit and an extended mix, say | You decide: **Link as versions** or **Keep the best copies** |

In each group the copy kept comes first and the others are dimmed. DJ Sugar
keeps the copy in your Play Ready folder, then a lossless file over a lossy
one, then the higher bitrate, then the one with cues or a checked grid,
then the most played. To keep another one, right-click it and choose
**Keep this copy**. The **Location** column (right-click a heading) shows
which folder each copy is in.

**Auto-fix** on Duplicates merges every group but Different lengths at once. Merging
loses nothing: the kept copy takes the others' lists and history, cues and
grid (when it has none), links, the highest rating, all their plays, the
earliest date added, and any tags it lacks. The other copies leave your
collection and their files go to `_Quarantine/<date>/`, next to your
Play Ready folder. With **Move files** off in Settings › Collection, the moves
wait until you turn it on. DJ Sugar never deletes anything there: the bottom
of the Collection Health page shows how many files are waiting, and
**Show folder** opens it, so you can empty it once you're sure.

**Undo** in the bar (or the track menu) brings the copies back and moves
their files back. The merged plays and tags stay on the kept copy.

**Link as versions** keeps copies of different lengths and links them, so
each shows the others on its Related tab, and they leave the Duplicates list.

## Quarantine

To put away a track you don't want, without deleting it, right-click it and
choose **Move to Quarantine**. It leaves your collection and its file goes to the
same `_Quarantine/<date>/` folder as duplicates, and **Undo** brings it back.
