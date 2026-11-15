# Filtering tracks

Two things narrow the track list down: the **search box** and the
**filter bar**. They work together, on whatever list, crate, set or stage
is picked on the left, and **Save** keeps both as a smart list.

## The filter bar

The filter bar above the track list narrows it down with chips, as djay Pro
does. The **filter button** next to the search box shows or hides it. The
bar starts hidden, and the button lights up while any filter is on.

The bar starts with chips for **Grouping**, **Genre**, your first two tag
groups (see [Your tags](music.md#your-tags)), **BPM** and
**Key**. Click a chip to pick values:

- **Genre, Grouping, Artist, Album**: tick the values you want. Each one
  shows how many tracks of the list have it, and a search box at the top
  finds one in a long list.
- **Tag groups** such as Mood: tick tags, and choose **Match Any** (a track
  needs one of them) or **Match All** (it needs all of them).
- **BPM**: type a range, **From** and **To**, or turn on **Near the playing
  track** to follow the tempo of what's playing, within a few percent.
- **Key**: tick keys, or turn on **Mixes with the playing track** to list
  only the keys that mix with it (see [Keys](decks.md#keys)).
- **Rating, Color, Year, Play Count, Date Added** are in **+**.

A chip that's on is lit and says what it picked. **Clear** in a chip empties
it, and **Clear** at the end of the bar empties them all.

**+** adds any other field to the bar for now. Turn on **Keep on the bar**
in a chip to pin it there for next time.

With **Near the playing track** or **Mixes with the playing track** on, the
list changes by itself when the next track starts: a quick way to find what
mixes next.

## Search

Type in the search box to find words in any field. Rest the pointer on the
box for a moment to see everything search understands:

| Search | Finds |
|---|---|
| `daft punk` | Both words, in any field |
| `artist:"daft punk"` | In one field: title, artist, album, genre, a tag group… |
| `mood:uplifting tag2:synth` | Your tag groups, by name or place |
| `title:=glue album:""` | The whole field; an empty field |
| `bpm:120-128 rating:>=4` | Numbers: `>`, `>=`, `<`, `<=` and ranges |
| `duration:>6m played:0` | Length; times played |
| `added:<7d added:>=2026-01-31` | Days or weeks ago, or a date |
| `~key:8A ~bpm:124` | Keys that mix with 8A; a tempo within 6% of 124 |
| `~deck` | Tracks that mix with the one playing |
| `color:red` | A track color |
| `crate:yoga crate:""` | In a crate with "yoga" in its name; in no crate |
| `stage:inbox` `stage:prep` `stage:playready` | A [stage](music.md#stages-inbox-in-prep-and-play-ready) |
| `is:prepped` | Tracks with tags, rating, tempo and key |
| `is:duplicate is:missing is:nocover` | A [Collection Health](collection-health.md) check |
| `-genre:techno` | Leaves those out |
| `glue \| daft` | Either one |

The chips are searches too, so a chip and the same typed search always
find the same tracks.

## Smart lists

**Save**, beside the search box, turns the search and the chips into a smart
list in the tree. It updates by itself as tracks come in or change.
VirtualDJ's filter folders come in as smart lists too.
