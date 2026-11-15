# Proposal

Planned for: **v0.5 · Stems, performance and switching over** (follows add-batch-edit's tag chips)

## Why

DJ Sugar's tags were shaped by VirtualDJ: numbered text fields (User 1, User 2, then `user_more` lines) holding space-separated hashtags, with names for the fields kept in the app's settings. That model leaks everywhere: a tag can't have a space ("Peak Time"), a tag can't be renamed or merged across the collection, rekordbox's My Tag categories had nowhere to go (they ended up as loose `labels`), and the settings page talked about "fields" instead of tags.

DJ library tools that do tags well (Lexicon's tag categories, rekordbox's My Tag, Serato's crates-as-tags) treat a tag as an entry of its own, in a category the DJ names, and import other apps' tags into those categories.

## What Changes

- The library keeps **tag groups** (`tag_groups`: name, order, one-or-several per track, color) and **tags** (`tags`: a name per group, case-insensitive, color), linked to tracks by `track_tags`. Names are free text: spaces allowed, no `#` needed.
- On first open, the old fields move into groups ("Tag 1", "Tag 2"… renamed from the names the DJ gave them in settings), hashtags become tags without `#`, and the change log follows.
- Imports map into groups through `tag_sources` (app, where it keeps tags → group): VirtualDJ's User 1 and User 2 fill the migrated groups, each rekordbox My Tag category becomes a group of its name. A first-seen place gets a group of its name (or the DJ's group with that name).
- Settings › Collection › **Tags** (was Fields) lists the groups: rename, Several/One per track, remove (asking first when tracks use it), add. Grouping's name stays on the same tab.
- Search: a group by its name (`mood:`) or place (`tag1:`), `tag:` across all groups; filter chips, columns, Info tab chips and the track menu's Add Tag work per group. Typed tags part on commas (hashtags still work).
- Grouping stays a plain field; **Copy into a tag group** on the Tags tab copies each track's grouping into a group of the grouping's name (say Trip Stage), where a track can have several.
- Next step: manage single tags (rename, merge, Color, delete) from the Tags tab.

## Capabilities

### Modified Capabilities

- `track-info`: tag groups in the Info tab
- `library-search`: searching tag groups
- `music-library`: tag groups and imports

## Out of Scope

- Writing tags into audio files (tags stay in the library, as before).
- Exporting tags back to VirtualDJ or rekordbox.
