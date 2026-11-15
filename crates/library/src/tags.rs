//! The DJ's tags: DJ Sugar's own model, not any other app's.
//!
//! Tags sit in **groups** the DJ makes and names (Mood, Instruments,
//! Vibe…). A group holds as many tags as the DJ likes, and lets a track
//! have several of them or, for a group set to one per track, only one.
//! A **tag** is an entry of its own (a name, later a color), so renaming
//! or merging one changes it on every track. Names may have spaces
//! ("Peak Time") and need no `#`.
//!
//! Other apps' tags are mapped in: each app's place for them (VirtualDJ's
//! User 1 and User 2, each rekordbox My Tag category) goes into one group,
//! remembered in `tag_sources`; a place seen for the first time gets a
//! group of its own name.

use std::collections::HashMap;

use djsugar_engine::{hashtags, same_tag, tag_name, LibraryInfo, TrackTag};
use rusqlite::{params, Connection, OptionalExtension};

use crate::{row_id, Error, Library};

pub(crate) const TAGS_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS tag_groups (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    position INTEGER NOT NULL DEFAULT 0,
    -- A track has at most one tag of the group.
    single INTEGER NOT NULL DEFAULT 0,
    color INTEGER
);
CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY,
    group_id INTEGER NOT NULL REFERENCES tag_groups(id) ON DELETE CASCADE,
    name TEXT NOT NULL COLLATE NOCASE,
    color INTEGER,
    UNIQUE (group_id, name)
);
CREATE TABLE IF NOT EXISTS track_tags (
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    -- The tag's place among the track's tags of its group.
    position INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (track_id, tag_id)
);
CREATE INDEX IF NOT EXISTS track_tags_tag ON track_tags(tag_id);
-- The group another app's tags go into: the app, and where it keeps them
-- ('User 1', a My Tag category).
CREATE TABLE IF NOT EXISTS tag_sources (
    source TEXT NOT NULL,
    field TEXT NOT NULL,
    group_id INTEGER NOT NULL REFERENCES tag_groups(id) ON DELETE CASCADE,
    PRIMARY KEY (source, field)
);
";

/// Groups the migration made from the old numbered tag fields, waiting for
/// the names the DJ gave those fields in the app's settings.
const UNNAMED: &str = "tag_groups_unnamed";

/// One of the DJ's tag groups.
#[derive(Clone, Debug, PartialEq)]
pub struct TagGroup {
    pub id: i64,
    pub name: String,
    /// A track has at most one tag of the group.
    pub single: bool,
    /// Color as 0xRRGGBB.
    pub color: Option<u32>,
}

impl TagGroup {
    /// A group as tests and the samples use it.
    pub fn new(id: i64, name: &str) -> Self {
        TagGroup {
            id,
            name: name.into(),
            single: false,
            color: None,
        }
    }
}

/// How much a group is used.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GroupUse {
    /// Tags in the group.
    pub tags: usize,
    /// Tracks with at least one of them.
    pub tracks: usize,
}

/// One tag of a group, with how many tracks have it.
#[derive(Clone, Debug, PartialEq)]
pub struct TagInfo {
    pub id: i64,
    pub group: i64,
    pub name: String,
    /// Color as 0xRRGGBB.
    pub color: Option<u32>,
    pub tracks: usize,
}

/// Creates the tag tables, and on first run moves the old numbered tag
/// fields (`user1`, `user2`, `user_more`: VirtualDJ's User 1 and User 2
/// and more like them) into groups.
pub(crate) fn setup(db: &Connection) -> Result<(), Error> {
    let had = db
        .prepare("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'tag_groups'")?
        .exists([])?;
    db.execute_batch(TAGS_SCHEMA)?;
    if !had {
        migrate_fields(db)?;
    }
    Ok(())
}

/// The old fields' words, one list per field, by track row.
fn old_fields(db: &Connection) -> Result<Vec<(i64, Vec<String>)>, Error> {
    let mut q = db.prepare("SELECT id, user1, user2, user_more FROM tracks")?;
    let rows = q.query_map([], |r| {
        let mut fields: Vec<String> = vec![r.get(1)?, r.get(2)?];
        let more: String = r.get(3)?;
        if !more.is_empty() {
            fields.extend(more.split('\n').map(str::to_string));
        }
        Ok((r.get::<_, i64>(0)?, fields))
    })?;
    rows.collect()
}

fn migrate_fields(db: &Connection) -> Result<(), Error> {
    let tracks = old_fields(db)?;
    let used = tracks
        .iter()
        .filter_map(|(_, f)| f.iter().rposition(|w| !w.trim().is_empty()))
        .max()
        .map_or(0, |i| i + 1);
    let mut groups = Vec::new();
    for i in 0..used.max(2) {
        db.execute(
            "INSERT INTO tag_groups (name, position) VALUES (?1, ?2)",
            params![format!("Tag {}", i + 1), i as i64],
        )?;
        groups.push(db.last_insert_rowid());
    }
    for (field, &group) in ["User 1", "User 2"].iter().zip(&groups) {
        db.execute(
            "INSERT INTO tag_sources (source, field, group_id) VALUES ('virtualdj', ?1, ?2)",
            params![field, group],
        )?;
    }
    for (row, fields) in &tracks {
        for (words, &group) in fields.iter().zip(&groups) {
            for (n, name) in hashtags(words).iter().enumerate() {
                let tag = tag_id(db, group, name)?;
                db.execute(
                    "INSERT OR IGNORE INTO track_tags (track_id, tag_id, position)
                     VALUES (?1, ?2, ?3)",
                    params![row, tag, n as i64],
                )?;
            }
        }
    }
    // The change log and hand edits named fields by number; values were
    // hashtags.
    for (n, &group) in groups.iter().enumerate() {
        let old = format!("tag{}", n + 1);
        let new = field_name(group);
        let mut q = db.prepare("SELECT id, old, new FROM tag_changes WHERE field = ?1")?;
        let rows: Vec<(i64, String, String)> = q
            .query_map([&old], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<_, _>>()?;
        for (id, before, after) in rows {
            db.execute(
                "UPDATE tag_changes SET field = ?2, old = ?3, new = ?4 WHERE id = ?1",
                params![
                    id,
                    new,
                    hashtags(&before).join(", "),
                    hashtags(&after).join(", ")
                ],
            )?;
        }
        db.execute(
            "UPDATE OR IGNORE hand_edited SET field = ?2 WHERE field = ?1",
            params![old, new],
        )?;
    }
    let ids: Vec<String> = groups.iter().map(i64::to_string).collect();
    db.execute(
        "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
        params![UNNAMED, ids.join(",")],
    )?;
    Ok(())
}

/// Sorts "2 ONS" before "10 SONG": a leading number counts as a number.
fn natural(s: &str) -> (u64, String) {
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    (digits.parse().unwrap_or(u64::MAX), s.to_lowercase())
}

/// The name the change log and the Info tab know a group's field by.
pub fn field_name(group: i64) -> String {
    format!("tag:{group}")
}

/// The tag `name` in `group`, made if it isn't there.
fn tag_id(db: &Connection, group: i64, name: &str) -> Result<i64, Error> {
    let found: Option<i64> = db
        .prepare_cached("SELECT id FROM tags WHERE group_id = ?1 AND name = ?2")?
        .query_row(params![group, name], |r| r.get(0))
        .optional()?;
    if let Some(id) = found {
        return Ok(id);
    }
    db.prepare_cached("INSERT INTO tags (group_id, name) VALUES (?1, ?2)")?
        .execute(params![group, name])?;
    Ok(db.last_insert_rowid())
}

/// Every track's tags by row, in group order.
pub(crate) fn track_tags(db: &Connection) -> Result<HashMap<i64, Vec<TrackTag>>, Error> {
    let mut q = db.prepare(
        "SELECT tt.track_id, t.group_id, t.name FROM track_tags tt
         JOIN tags t ON t.id = tt.tag_id
         JOIN tag_groups g ON g.id = t.group_id
         ORDER BY tt.track_id, g.position, g.id, tt.position",
    )?;
    let rows = q.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            TrackTag {
                group: r.get(1)?,
                name: r.get(2)?,
            },
        ))
    })?;
    let mut out: HashMap<i64, Vec<TrackTag>> = HashMap::new();
    for row in rows {
        let (id, tag) = row?;
        out.entry(id).or_default().push(tag);
    }
    Ok(out)
}

/// Makes `tags` the track's tags in each group of `groups` (every group
/// when None). Tags of groups that are gone are left out.
pub(crate) fn write_track_tags(
    db: &Connection,
    row: i64,
    tags: &[TrackTag],
    groups: Option<&[i64]>,
) -> Result<(), Error> {
    let known: Vec<i64> = db
        .prepare_cached("SELECT id FROM tag_groups")?
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let groups: Vec<i64> = groups.map_or_else(|| known.clone(), <[i64]>::to_vec);
    for &group in &groups {
        db.prepare_cached(
            "DELETE FROM track_tags WHERE track_id = ?1
             AND tag_id IN (SELECT id FROM tags WHERE group_id = ?2)",
        )?
        .execute(params![row, group])?;
        if !known.contains(&group) {
            continue;
        }
        let mut n = 0;
        for t in tags.iter().filter(|t| t.group == group) {
            let tag = tag_id(db, group, &t.name)?;
            n += db
                .prepare_cached(
                    "INSERT OR IGNORE INTO track_tags (track_id, tag_id, position)
                     VALUES (?1, ?2, ?3)",
                )?
                .execute(params![row, tag, n as i64])?;
        }
    }
    Ok(())
}

/// The group another app's tags from `field` go into, made (named after
/// the field) the first time.
fn source_group(db: &Connection, source: &str, field: &str) -> Result<i64, Error> {
    let found: Option<i64> = db
        .prepare_cached("SELECT group_id FROM tag_sources WHERE source = ?1 AND field = ?2")?
        .query_row(params![source, field], |r| r.get(0))
        .optional()?;
    if let Some(id) = found {
        return Ok(id);
    }
    // A group of that name the DJ made already takes them.
    let named: Option<i64> = db
        .prepare_cached("SELECT id FROM tag_groups WHERE name = ?1 COLLATE NOCASE")?
        .query_row([field], |r| r.get(0))
        .optional()?;
    let id = match named {
        Some(id) => id,
        None => add_group(db, field)?,
    };
    db.execute(
        "INSERT INTO tag_sources (source, field, group_id) VALUES (?1, ?2, ?3)",
        params![source, field, id],
    )?;
    Ok(id)
}

fn add_group(db: &Connection, name: &str) -> Result<i64, Error> {
    db.execute(
        "INSERT INTO tag_groups (name, position)
         VALUES (?1, (SELECT COALESCE(MAX(position) + 1, 0) FROM tag_groups))",
        [name],
    )?;
    Ok(db.last_insert_rowid())
}

/// Puts the tags another app brought for a track (`info.imported_tags`)
/// in the DJ's groups. With `overwrite` off, a group the track has tags
/// in already keeps them.
pub(crate) fn import_track_tags(
    db: &Connection,
    source: &str,
    row: i64,
    info: &LibraryInfo,
    overwrite: bool,
) -> Result<(), Error> {
    if info.imported_tags.is_empty() {
        return Ok(());
    }
    let mut by_group: Vec<(i64, Vec<TrackTag>)> = Vec::new();
    for (field, name) in &info.imported_tags {
        let name = tag_name(name);
        if name.is_empty() {
            continue;
        }
        let group = source_group(db, source, field)?;
        let at = match by_group.iter().position(|(g, _)| *g == group) {
            Some(at) => at,
            None => {
                by_group.push((group, Vec::new()));
                by_group.len() - 1
            }
        };
        let tags = &mut by_group[at].1;
        if !tags.iter().any(|t| same_tag(&t.name, &name)) {
            tags.push(TrackTag { group, name });
        }
    }
    for (group, tags) in by_group {
        if !overwrite {
            let has: bool = db
                .prepare_cached(
                    "SELECT 1 FROM track_tags tt JOIN tags t ON t.id = tt.tag_id
                     WHERE tt.track_id = ?1 AND t.group_id = ?2",
                )?
                .exists(params![row, group])?;
            if has {
                continue;
            }
        }
        write_track_tags(db, row, &tags, Some(&[group]))?;
    }
    Ok(())
}

impl Library {
    /// The DJ's tag groups, in their order.
    pub fn tag_groups(&self) -> Result<Vec<TagGroup>, Error> {
        let mut q = self
            .db
            .prepare("SELECT id, name, single, color FROM tag_groups ORDER BY position, id")?;
        let rows = q.query_map([], |r| {
            Ok(TagGroup {
                id: r.get(0)?,
                name: r.get(1)?,
                single: r.get(2)?,
                color: r.get::<_, Option<i64>>(3)?.map(|c| c as u32),
            })
        })?;
        rows.collect()
    }

    /// A new group at the end, named `name`; its id.
    pub fn add_tag_group(&self, name: &str) -> Result<i64, Error> {
        add_group(&self.db, name.trim())
    }

    pub fn rename_tag_group(&self, id: i64, name: &str) -> Result<(), Error> {
        self.db.execute(
            "UPDATE tag_groups SET name = ?2 WHERE id = ?1",
            params![id, name.trim()],
        )?;
        Ok(())
    }

    /// Sets whether a track has at most one tag of the group. Turning it
    /// on leaves tracks with several as they are until they are edited.
    pub fn set_tag_group_single(&self, id: i64, single: bool) -> Result<(), Error> {
        self.db.execute(
            "UPDATE tag_groups SET single = ?2 WHERE id = ?1",
            params![id, single],
        )?;
        Ok(())
    }

    /// Moves a group to `position` (from 0) among the groups.
    pub fn move_tag_group(&mut self, id: i64, position: usize) -> Result<(), Error> {
        let mut ids: Vec<i64> = self.tag_groups()?.into_iter().map(|g| g.id).collect();
        let Some(from) = ids.iter().position(|&g| g == id) else {
            return Ok(());
        };
        ids.remove(from);
        ids.insert(position.min(ids.len()), id);
        let tx = self.db.transaction()?;
        for (n, g) in ids.iter().enumerate() {
            tx.execute(
                "UPDATE tag_groups SET position = ?2 WHERE id = ?1",
                params![g, n as i64],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Takes a group away, with its tags off every track.
    pub fn remove_tag_group(&self, id: i64) -> Result<(), Error> {
        self.db
            .execute("DELETE FROM tag_groups WHERE id = ?1", [id])?;
        Ok(())
    }

    /// How many tags a group has, and on how many tracks.
    pub fn tag_group_use(&self, id: i64) -> Result<GroupUse, Error> {
        let tags: i64 =
            self.db
                .query_row("SELECT COUNT(*) FROM tags WHERE group_id = ?1", [id], |r| {
                    r.get(0)
                })?;
        let tracks: i64 = self.db.query_row(
            "SELECT COUNT(DISTINCT tt.track_id) FROM track_tags tt
             JOIN tags t ON t.id = tt.tag_id WHERE t.group_id = ?1",
            [id],
            |r| r.get(0),
        )?;
        Ok(GroupUse {
            tags: tags as usize,
            tracks: tracks as usize,
        })
    }

    /// Names the groups made from the old numbered tag fields after what
    /// the DJ called those fields (`names[0]` for Tag 1); once only.
    pub fn name_migrated_tag_groups(&self, names: &[String]) -> Result<(), Error> {
        let Some(ids) = self.meta(UNNAMED) else {
            return Ok(());
        };
        for (id, name) in ids.split(',').zip(names) {
            let (Ok(id), name) = (id.parse::<i64>(), name.trim()) else {
                continue;
            };
            if !name.is_empty() {
                self.rename_tag_group(id, name)?;
            }
        }
        self.db
            .execute("DELETE FROM meta WHERE key = ?1", [UNNAMED])?;
        Ok(())
    }

    /// Copies each track's grouping into the tag group named `name` (made
    /// if there isn't one, set to several tags per track), as a tag added
    /// to those it has there. The grouping stays as it is. Returns the
    /// group and how many tracks got a tag.
    pub fn copy_grouping_to_group(&mut self, name: &str) -> Result<(i64, usize), Error> {
        let tx = self.db.transaction()?;
        let found: Option<i64> = tx
            .query_row(
                "SELECT id FROM tag_groups WHERE name = ?1 COLLATE NOCASE",
                [name.trim()],
                |r| r.get(0),
            )
            .optional()?;
        let group = match found {
            Some(id) => id,
            None => add_group(&tx, name.trim())?,
        };
        let rows: Vec<(i64, String)> = tx
            .prepare("SELECT id, grouping FROM tracks WHERE TRIM(grouping) != ''")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        let mut n = 0;
        for (row, grouping) in rows {
            let name = tag_name(&grouping);
            if name.is_empty() {
                continue;
            }
            let tag = tag_id(&tx, group, &name)?;
            n += tx.execute(
                "INSERT OR IGNORE INTO track_tags (track_id, tag_id, position)
                 VALUES (?1, ?2, (SELECT COUNT(*) FROM track_tags tt JOIN tags t ON t.id = tt.tag_id
                                  WHERE tt.track_id = ?1 AND t.group_id = ?3))",
                params![row, tag, group],
            )?;
        }
        tx.commit()?;
        Ok((group, n))
    }

    /// Every track's tags, by track id.
    pub fn all_track_tags(&self) -> Result<HashMap<usize, Vec<TrackTag>>, Error> {
        Ok(track_tags(&self.db)?
            .into_iter()
            .map(|(row, tags)| (crate::track_id(row), tags))
            .collect())
    }

    /// A group's tags with how many tracks have each, A to Z (a leading
    /// number counts as a number, so "2 ONS" comes before "10 SONG").
    pub fn tags_of_group(&self, group: i64) -> Result<Vec<TagInfo>, Error> {
        let mut q = self.db.prepare(
            "SELECT t.id, t.name, t.color, COUNT(tt.track_id) FROM tags t
             LEFT JOIN track_tags tt ON tt.tag_id = t.id
             WHERE t.group_id = ?1 GROUP BY t.id",
        )?;
        let rows = q.query_map([group], |r| {
            Ok(TagInfo {
                id: r.get(0)?,
                group,
                name: r.get(1)?,
                color: r.get::<_, Option<i64>>(2)?.map(|c| c as u32),
                tracks: r.get::<_, i64>(3)? as usize,
            })
        })?;
        let mut out: Vec<TagInfo> = rows.collect::<Result<_, _>>()?;
        out.sort_by_key(|t| natural(&t.name));
        Ok(out)
    }

    /// Renames a tag on every track. When the group has a tag of that name
    /// already, the two become one: its tracks get that tag. Returns the
    /// tag the tracks have now.
    pub fn rename_tag(&mut self, id: i64, name: &str) -> Result<i64, Error> {
        let name = tag_name(name);
        if name.is_empty() {
            return Ok(id);
        }
        let tx = self.db.transaction()?;
        let group: i64 = tx.query_row("SELECT group_id FROM tags WHERE id = ?1", [id], |r| {
            r.get(0)
        })?;
        let other: Option<i64> = tx
            .query_row(
                "SELECT id FROM tags WHERE group_id = ?1 AND name = ?2 AND id != ?3",
                params![group, name, id],
                |r| r.get(0),
            )
            .optional()?;
        let kept = match other {
            Some(other) => {
                tx.execute(
                    "INSERT OR IGNORE INTO track_tags (track_id, tag_id, position)
                     SELECT track_id, ?2, position FROM track_tags WHERE tag_id = ?1",
                    params![id, other],
                )?;
                // The merged tag keeps a color either had.
                tx.execute(
                    "UPDATE tags SET color = COALESCE(color, (SELECT color FROM tags WHERE id = ?1))
                     WHERE id = ?2",
                    params![id, other],
                )?;
                tx.execute("DELETE FROM tags WHERE id = ?1", [id])?;
                other
            }
            None => {
                tx.execute("UPDATE tags SET name = ?2 WHERE id = ?1", params![id, name])?;
                id
            }
        };
        tx.commit()?;
        Ok(kept)
    }

    /// Takes a tag off every track and out of its group.
    pub fn delete_tag(&self, id: i64) -> Result<(), Error> {
        self.db.execute("DELETE FROM tags WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn set_tag_color(&self, id: i64, color: Option<u32>) -> Result<(), Error> {
        self.db.execute(
            "UPDATE tags SET color = ?2 WHERE id = ?1",
            params![id, color.map(i64::from)],
        )?;
        Ok(())
    }

    /// Every tag with a color: group, name, color.
    pub fn tag_colors(&self) -> Result<Vec<(i64, String, u32)>, Error> {
        let mut q = self
            .db
            .prepare("SELECT group_id, name, color FROM tags WHERE color IS NOT NULL")?;
        let rows = q.query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? as u32))
        })?;
        rows.collect()
    }

    /// Saves a track's tags in every group.
    pub fn save_tags(&self, id: usize, info: &LibraryInfo) -> Result<(), Error> {
        let Some(row) = row_id(id) else { return Ok(()) };
        write_track_tags(&self.db, row, &info.tags, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::file_track;
    use std::path::Path;

    #[test]
    fn old_fields_become_groups_with_their_names() {
        let db = Connection::open_in_memory().unwrap();
        // A library from before tag groups, as far as these tables go.
        db.execute_batch(
            "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE tracks (id INTEGER PRIMARY KEY, user1 TEXT NOT NULL DEFAULT '',
                user2 TEXT NOT NULL DEFAULT '', user_more TEXT NOT NULL DEFAULT '');
             INSERT INTO tracks VALUES (1, '#uplifting #dreamy', '#synth', '\n#dusk');
             INSERT INTO tracks VALUES (2, '#Uplifting', '', '');",
        )
        .unwrap();
        db.execute_batch(crate::cleanup::CHANGES_SCHEMA).unwrap();
        db.execute(
            "INSERT INTO tag_changes (batch, track_id, field, old, new, at)
             VALUES (1, 1, 'tag2', '', '#synth', 0)",
            [],
        )
        .unwrap();
        setup(&db).unwrap();
        let groups: Vec<String> = db
            .prepare("SELECT name FROM tag_groups ORDER BY position")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(groups, ["Tag 1", "Tag 2", "Tag 3", "Tag 4"]);
        let tags = track_tags(&db).unwrap();
        let names: Vec<(i64, &str)> = tags[&1]
            .iter()
            .map(|t| (t.group, t.name.as_str()))
            .collect();
        assert_eq!(
            names,
            [(1, "uplifting"), (1, "dreamy"), (2, "synth"), (4, "dusk")]
        );
        // One tag, whatever its case.
        assert_eq!(tags[&2][0].name, "uplifting");
        let (field, new): (String, String) = db
            .query_row("SELECT field, new FROM tag_changes", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!((field.as_str(), new.as_str()), ("tag:2", "synth"));
    }

    #[test]
    fn migrated_groups_take_the_field_names_once() {
        let lib = Library::in_memory().unwrap();
        lib.name_migrated_tag_groups(&["Mood".into(), "Instruments".into()])
            .unwrap();
        let names: Vec<String> = lib
            .tag_groups()
            .unwrap()
            .into_iter()
            .map(|g| g.name)
            .collect();
        assert_eq!(names, ["Mood", "Instruments"]);
        lib.name_migrated_tag_groups(&["Other".into()]).unwrap();
        assert_eq!(lib.tag_groups().unwrap()[0].name, "Mood");
    }

    #[test]
    fn tags_are_saved_read_and_go_with_their_group() {
        let mut lib = Library::in_memory().unwrap();
        let t = lib.add_file(&file_track("/m/a.mp3", "A")).unwrap();
        let vibe = lib.add_tag_group("Vibe").unwrap();
        let mut info = t.info.clone();
        info.set_tags_in(
            vibe,
            ["Peak Time".into(), "#dark".into(), "peak time".into()],
        );
        lib.save_tags(t.id, &info).unwrap();
        let back = lib.all_tracks(|_: &Path| true).unwrap();
        assert_eq!(back[0].info.tag_text(vibe), "Peak Time, dark");
        assert_eq!(
            lib.tag_group_use(vibe).unwrap(),
            GroupUse { tags: 2, tracks: 1 }
        );
        lib.remove_tag_group(vibe).unwrap();
        assert!(lib.all_tracks(|_: &Path| true).unwrap()[0]
            .info
            .tags
            .is_empty());
    }

    #[test]
    fn grouping_copies_into_a_group() {
        let mut lib = Library::in_memory().unwrap();
        let mut a = file_track("/m/a.mp3", "A");
        a.info.grouping = "4 PEAK".into();
        let b = file_track("/m/b.mp3", "B");
        lib.sync(&[a, b]).unwrap();
        let (group, n) = lib.copy_grouping_to_group("Trip Stage").unwrap();
        assert_eq!(n, 1);
        let tracks = lib.all_tracks(|_: &Path| true).unwrap();
        assert_eq!(tracks[0].info.tag_text(group), "4 PEAK");
        assert_eq!(tracks[0].info.grouping, "4 PEAK", "grouping stays");
        // Again: the same group, nothing twice.
        assert_eq!(
            lib.copy_grouping_to_group("trip stage").unwrap(),
            (group, 0)
        );
        assert!(!lib.tag_groups().unwrap().last().unwrap().single);
    }

    #[test]
    fn tags_rename_merge_color_and_go() {
        let mut lib = Library::in_memory().unwrap();
        let tracks = [file_track("/m/a.mp3", "A"), file_track("/m/b.mp3", "B")];
        let tracks = lib.add_files(&tracks).unwrap();
        let mut a = tracks[0].info.clone();
        a.set_tags_in(1, ["dark".into(), "dusk".into()]);
        lib.save_tags(tracks[0].id, &a).unwrap();
        let mut b = tracks[1].info.clone();
        b.set_tags_in(1, ["Dark".into(), "10 SONG".into(), "2 ONS".into()]);
        lib.save_tags(tracks[1].id, &b).unwrap();
        let tags = lib.tags_of_group(1).unwrap();
        let list: Vec<(&str, usize)> = tags.iter().map(|t| (t.name.as_str(), t.tracks)).collect();
        assert_eq!(
            list,
            [("2 ONS", 1), ("10 SONG", 1), ("dark", 2), ("dusk", 1)]
        );
        let id = |name: &str| tags.iter().find(|t| t.name == name).unwrap().id;
        lib.set_tag_color(id("dusk"), Some(0xff9500)).unwrap();
        // Renamed to a name the group has: one tag, with the color.
        let kept = lib.rename_tag(id("dusk"), "DARK").unwrap();
        assert_eq!(kept, id("dark"));
        assert_eq!(
            lib.tag_colors().unwrap(),
            [(1, "dark".to_string(), 0xff9500)]
        );
        let back = lib.all_tracks(|_: &Path| true).unwrap();
        assert_eq!(back[0].info.tag_text(1), "dark");
        // A plain rename changes it on every track.
        lib.rename_tag(id("dark"), "Dark Room").unwrap();
        lib.delete_tag(id("2 ONS")).unwrap();
        let back = lib.all_tracks(|_: &Path| true).unwrap();
        assert_eq!(back[1].info.tag_text(1), "Dark Room, 10 SONG");
    }

    #[test]
    fn groups_move() {
        let mut lib = Library::in_memory().unwrap();
        let c = lib.add_tag_group("C").unwrap();
        lib.move_tag_group(c, 0).unwrap();
        let names: Vec<String> = lib
            .tag_groups()
            .unwrap()
            .into_iter()
            .map(|g| g.name)
            .collect();
        assert_eq!(names, ["C", "Tag 1", "Tag 2"]);
    }
}
