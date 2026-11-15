//! Crates: the DJ's long-lived lists for sorting music, a track in as many
//! as it fits (Yoga, Journeys, Dance), each track once and in no order of
//! their own, as against playlists, which keep an order. Also what Collection Health finds in playlists: crates
//! left empty, and entries for tracks taken out of the library.

use std::collections::HashMap;

use crate::{track_id, Error, Library, PlaylistKind};

/// What Collection Health finds in the DJ's lists.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListHealth {
    /// Crates with no tracks (playlists are left alone: one made for a gig
    /// starts empty).
    pub empty: Vec<i64>,
    /// Entries in lists for tracks removed from the library.
    pub dead: usize,
}

impl ListHealth {
    pub fn is_clean(&self) -> bool {
        self.empty.is_empty() && self.dead == 0
    }
}

/// The source of a playlist made at the top of Playlists rather than in a
/// folder, so it isn't taken for a crate.
pub const TOP_PLAYLIST: &str = "djsugar";

/// The source of a folder at the top of Crates rather than of Playlists.
pub const CRATE_FOLDER: &str = "djsugar-crates";

/// Whether a list is a crate.
pub fn is_crate(p: &crate::Playlist) -> bool {
    p.kind == PlaylistKind::Crate
}

/// Whether a list or folder came in with another app's import (its source
/// names the app), as against one the DJ made.
pub fn is_imported(p: &crate::Playlist) -> bool {
    p.source
        .as_deref()
        .is_some_and(|s| s != TOP_PLAYLIST && s != CRATE_FOLDER)
}

/// Moves the lists and folders the DJ made inside `source`'s imported
/// folders out to the top of Playlists (what's in them moves along), so
/// taking that import's lists away leaves them be. Returns how many moved.
pub(crate) fn rescue_own_lists(tx: &rusqlite::Transaction, source: &str) -> Result<usize, Error> {
    let own: Vec<i64> = tx
        .prepare(
            "SELECT c.id FROM playlists c JOIN playlists p ON c.parent = p.id
              WHERE p.source = ?1 AND (c.source IS NULL OR c.source = ?2 OR c.source = ?3)
              ORDER BY c.position, c.id",
        )?
        .query_map(rusqlite::params![source, TOP_PLAYLIST, CRATE_FOLDER], |r| {
            r.get(0)
        })?
        .collect::<Result<_, _>>()?;
    for &id in &own {
        let position: i64 = tx.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM playlists WHERE parent IS NULL",
            [],
            |r| r.get(0),
        )?;
        // Out to the top: a crate to Crates, anything else to Playlists.
        tx.execute(
            "UPDATE playlists SET parent = NULL, position = ?2, source = NULL WHERE id = ?1",
            rusqlite::params![id, position],
        )?;
    }
    Ok(own.len())
}

impl Library {
    /// Moves the DJ's own lists out of every import's folders (see
    /// [`rescue_own_lists`]); made before imports were read-only.
    pub fn rescue_own_lists(&mut self) -> Result<usize, Error> {
        let tx = self.db.transaction()?;
        let sources: Vec<String> = tx
            .prepare(
                "SELECT DISTINCT source FROM playlists
                  WHERE source IS NOT NULL AND source NOT IN (?1, ?2)",
            )?
            .query_map([TOP_PLAYLIST, CRATE_FOLDER], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        let mut moved = 0;
        for s in &sources {
            moved += rescue_own_lists(&tx, s)?;
        }
        tx.commit()?;
        Ok(moved)
    }

    /// Makes another app's imported lists and folders the DJ's own: they
    /// stay where they are and can be edited, and that app's next import
    /// no longer replaces them. Returns how many changed hands.
    pub fn adopt_import(&self, source: &str) -> Result<usize, Error> {
        self.db.execute(
            "UPDATE playlists SET source = NULL WHERE source = ?1",
            [source],
        )
    }

    /// Makes an empty crate in `folder`, or at the top of Crates.
    pub fn create_crate(&self, name: &str, folder: Option<i64>) -> Result<i64, Error> {
        self.create_playlist(name, folder, PlaylistKind::Crate)
    }

    /// Makes a folder at the top of Crates.
    pub fn create_crate_folder(&self, name: &str) -> Result<i64, Error> {
        let id = self.create_playlist(name, None, PlaylistKind::Folder)?;
        self.db.execute(
            "UPDATE playlists SET source = ?2 WHERE id = ?1",
            rusqlite::params![id, CRATE_FOLDER],
        )?;
        Ok(id)
    }

    /// Turns a playlist into a crate (each track once, the first kept) or
    /// a crate into a playlist (in the order its tracks went in).
    pub fn set_crate(&mut self, id: i64, krate: bool) -> Result<(), Error> {
        let tx = self.db.transaction()?;
        let (from, to) = if krate {
            (PlaylistKind::List, PlaylistKind::Crate)
        } else {
            (PlaylistKind::Crate, PlaylistKind::List)
        };
        let changed = tx.execute(
            "UPDATE playlists SET kind = ?3 WHERE id = ?1 AND kind = ?2",
            rusqlite::params![id, from.to_i64(), to.to_i64()],
        )?;
        if changed > 0 && krate {
            tx.execute(
                "DELETE FROM playlist_tracks
                  WHERE playlist_id = ?1
                    AND EXISTS (SELECT 1 FROM playlist_tracks p
                                 WHERE p.playlist_id = ?1
                                   AND p.track_id = playlist_tracks.track_id
                                   AND p.position < playlist_tracks.position)",
                [id],
            )?;
        }
        tx.commit()
    }

    /// Moves a list or folder to the end of `folder`, or to the top of
    /// Crates (`crates`) or Playlists. What's inside a folder moves along.
    pub fn move_playlist(&self, id: i64, folder: Option<i64>, crates: bool) -> Result<(), Error> {
        let position: i64 = self.db.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM playlists WHERE parent IS ?1",
            [folder],
            |r| r.get(0),
        )?;
        // Only a folder at the top needs marking to sit under Crates.
        let source = match folder {
            None if crates => Some(CRATE_FOLDER),
            _ => None,
        };
        self.db.execute(
            "UPDATE playlists SET parent = ?2, position = ?3,
                    source = CASE WHEN kind = ?5 THEN ?4 ELSE NULL END
              WHERE id = ?1",
            rusqlite::params![id, folder, position, source, PlaylistKind::Folder.to_i64()],
        )?;
        Ok(())
    }

    /// Makes an empty playlist at the top of Playlists, out of any folder.
    pub fn create_top_playlist(&self, name: &str) -> Result<i64, Error> {
        let id = self.create_playlist(name, None, PlaylistKind::List)?;
        self.db.execute(
            "UPDATE playlists SET source = ?2 WHERE id = ?1",
            rusqlite::params![id, TOP_PLAYLIST],
        )?;
        Ok(id)
    }

    /// The crates: the DJ's own lists at the top of the tree, all but the
    /// Inbox list downloads are noted in.
    fn crate_ids(&self) -> Result<HashMap<i64, String>, Error> {
        let inbox = self.inbox_list_id();
        Ok(self
            .playlists()?
            .into_iter()
            .filter(|p| is_crate(p) && Some(p.id) != inbox)
            .map(|p| (p.id, p.name))
            .collect())
    }

    /// The crates each track is in, by track id, each crate once, in name
    /// order.
    pub fn crates_by_track(&self) -> Result<HashMap<usize, Vec<String>>, Error> {
        let crates = self.crate_ids()?;
        let mut q = self
            .db
            .prepare("SELECT DISTINCT playlist_id, track_id FROM playlist_tracks")?;
        let mut out: HashMap<usize, Vec<String>> = HashMap::new();
        for row in q.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
            let (list, track) = row?;
            if let Some(name) = crates.get(&list) {
                out.entry(track_id(track)).or_default().push(name.clone());
            }
        }
        for names in out.values_mut() {
            names.sort_by_key(|n| n.to_lowercase());
            names.dedup();
        }
        Ok(out)
    }

    /// Empty crates and entries for removed tracks in the DJ's lists.
    pub fn list_health(&self) -> Result<ListHealth, Error> {
        let crates = self.crate_ids()?;
        let mut q = self.db.prepare(
            "SELECT p.id FROM playlists p WHERE p.kind = 5
               AND NOT EXISTS (SELECT 1 FROM playlist_tracks t WHERE t.playlist_id = p.id)",
        )?;
        let empty = q
            .query_map([], |r| r.get::<_, i64>(0))?
            .filter_map(|r| r.ok())
            .filter(|id| crates.contains_key(id))
            .collect();
        let dead: i64 = self.db.query_row(
            "SELECT COUNT(*) FROM playlist_tracks
              WHERE track_id IN (SELECT track_id FROM removed)
                AND playlist_id IN (SELECT id FROM playlists WHERE kind IN (0, 5))",
            [],
            |r| r.get(0),
        )?;
        Ok(ListHealth {
            empty,
            dead: dead as usize,
        })
    }

    /// Deletes the empty crates and the entries for removed tracks that
    /// [`Library::list_health`] found; returns how many of each went.
    pub fn tidy_lists(&mut self) -> Result<(usize, usize), Error> {
        let health = self.list_health()?;
        let tx = self.db.transaction()?;
        for id in &health.empty {
            tx.execute("DELETE FROM playlists WHERE id = ?1", [id])?;
        }
        let dead = tx.execute(
            "DELETE FROM playlist_tracks
              WHERE track_id IN (SELECT track_id FROM removed)
                AND playlist_id IN (SELECT id FROM playlists WHERE kind IN (0, 5))",
            [],
        )?;
        tx.commit()?;
        Ok((health.empty.len(), dead))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::file_track;

    fn library() -> (Library, Vec<usize>) {
        let mut lib = Library::in_memory().unwrap();
        lib.sync(&[
            file_track("/m/a.mp3", "A"),
            file_track("/m/b.mp3", "B"),
            file_track("/m/c.mp3", "C"),
        ])
        .unwrap();
        let ids = lib.tracks(|_| true).unwrap().iter().map(|t| t.id).collect();
        (lib, ids)
    }

    #[test]
    fn a_playlist_at_the_top_is_not_a_crate() {
        let (lib, _) = library();
        let list = lib.create_top_playlist("Gig").unwrap();
        let krate = lib.create_crate("Yoga", None).unwrap();
        let lists = lib.playlists().unwrap();
        let get = |id| lists.iter().find(|p| p.id == id).unwrap();
        assert!(!is_crate(get(list)));
        assert!(is_crate(get(krate)));
        assert!(!lib.crate_ids().unwrap().contains_key(&list));
    }

    #[test]
    fn crates_are_their_own_kind() {
        let (mut lib, ids) = library();
        let yoga = lib.create_crate("Yoga", None).unwrap();
        let folder = lib
            .create_playlist("Gigs", None, PlaylistKind::Folder)
            .unwrap();
        let friday = lib
            .create_playlist("Friday", Some(folder), PlaylistKind::List)
            .unwrap();
        let inbox = lib.inbox_list().unwrap();
        lib.add_to_playlist(yoga, &[ids[0], ids[1]]).unwrap();
        lib.add_to_playlist(friday, &[ids[0], ids[2]]).unwrap();
        lib.add_to_playlist(inbox, &[ids[2]]).unwrap();
        let crates = lib.crates_by_track().unwrap();
        assert_eq!(crates[&ids[0]], ["Yoga"]);
        assert_eq!(crates[&ids[1]], ["Yoga"]);
        assert!(!crates.contains_key(&ids[2]));
    }

    #[test]
    fn a_crate_holds_each_track_once_and_can_live_in_a_folder() {
        let (mut lib, ids) = library();
        let folder = lib.create_crate_folder("Moods").unwrap();
        let calm = lib.create_crate("Calm", Some(folder)).unwrap();
        lib.add_to_playlist(calm, &[ids[0], ids[0], ids[1]])
            .unwrap();
        lib.add_to_playlist(calm, &[ids[1], ids[2]]).unwrap();
        assert_eq!(lib.playlist_tracks(calm).unwrap(), [ids[0], ids[1], ids[2]]);
        assert_eq!(lib.crates_by_track().unwrap()[&ids[2]], ["Calm"]);
        let moods = lib.playlists().unwrap();
        let moods = moods.iter().find(|p| p.id == folder).unwrap();
        assert!(!is_imported(moods));
    }

    #[test]
    fn crates_stay_in_crate_folders() {
        let (mut lib, _) = library();
        let moods = lib.create_crate_folder("Moods").unwrap();
        let calm = lib.create_crate("Calm", Some(moods)).unwrap();
        assert_eq!(lib.rescue_own_lists().unwrap(), 0);
        let lists = lib.playlists().unwrap();
        assert_eq!(
            lists.iter().find(|p| p.id == calm).unwrap().parent,
            Some(moods)
        );
    }

    #[test]
    fn folders_move_between_crates_and_playlists() {
        let (lib, _) = library();
        let gigs = lib
            .create_playlist("Gigs", None, PlaylistKind::Folder)
            .unwrap();
        let friday = lib
            .create_playlist("Friday", Some(gigs), PlaylistKind::List)
            .unwrap();
        lib.move_playlist(gigs, None, true).unwrap();
        let get = |lib: &Library, id| {
            lib.playlists()
                .unwrap()
                .into_iter()
                .find(|p| p.id == id)
                .unwrap()
        };
        assert_eq!(get(&lib, gigs).source.as_deref(), Some(CRATE_FOLDER));
        assert_eq!(get(&lib, friday).parent, Some(gigs));
        let moods = lib.create_crate_folder("Moods").unwrap();
        lib.move_playlist(gigs, Some(moods), false).unwrap();
        let g = get(&lib, gigs);
        assert_eq!((g.parent, g.source), (Some(moods), None));
        lib.move_playlist(friday, None, false).unwrap();
        assert_eq!(get(&lib, friday).parent, None);
    }

    #[test]
    fn lists_change_kind() {
        let (mut lib, ids) = library();
        let gig = lib.create_top_playlist("Gig").unwrap();
        lib.add_to_playlist(gig, &[ids[1], ids[0], ids[1]]).unwrap();
        lib.set_crate(gig, true).unwrap();
        assert_eq!(lib.playlist_tracks(gig).unwrap(), [ids[1], ids[0]]);
        assert!(lib.crate_ids().unwrap().contains_key(&gig));
        lib.set_crate(gig, false).unwrap();
        assert!(!lib.crate_ids().unwrap().contains_key(&gig));
    }

    #[test]
    fn tidies_empty_crates_and_removed_tracks() {
        let (mut lib, ids) = library();
        let empty = lib.create_crate("Old", None).unwrap();
        let yoga = lib.create_crate("Yoga", None).unwrap();
        let gig = lib
            .create_playlist("Gig", None, PlaylistKind::Folder)
            .unwrap();
        lib.create_playlist("Set", Some(gig), PlaylistKind::List)
            .unwrap();
        lib.add_to_playlist(yoga, &[ids[0], ids[1]]).unwrap();
        lib.remove_tracks(&[ids[1]]).unwrap();
        let health = lib.list_health().unwrap();
        // The gig's playlist is empty too, but a new one is meant to be.
        assert_eq!(
            health,
            ListHealth {
                empty: vec![empty],
                dead: 1
            }
        );
        assert_eq!(lib.tidy_lists().unwrap(), (1, 1));
        assert!(lib.list_health().unwrap().is_clean());
        assert_eq!(lib.playlist_tracks(yoga).unwrap(), [ids[0]]);
    }
}
