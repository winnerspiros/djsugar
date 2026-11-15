//! The Albums and Mixed Sets sources in the rail: the albums folder (an
//! album a folder, or Artist/Album) and the mixed sets folder (recordings
//! of whole sets, DJ Sugar's own among them), each as a tree of its folders
//! over the collection's tracks. Mixed sets stay out of the collection;
//! albums are in it too.
//!
//! Play Album puts an album on the Automix list and plays it through
//! gaplessly.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use djsugar_engine::Track;
use slint::{ComponentHandle, ModelRc, VecModel};

use super::{
    file_path, is_browsed, track_menu::main_artist, Browser, Contents, Node, ALBUMS, KIND_ALBUM,
    KIND_ARTIST, KIND_FOLDER, KIND_MIX,
};
use crate::{settings, AlbumTile, AlbumsUi, LibraryEdit};

/// Longest side of a cover in the covers view: twice its width, for HiDPI
/// screens.
pub(super) const COVER_SIZE: u32 = 360;

/// Whether the Albums source shows covers rather than a list.
const ALBUMS_COVERS: &str = "albums_covers";

/// What most of `values` are, ignoring empty ones and case; `None` when
/// none is shared by more than half of them.
pub(super) fn most(values: &[&str]) -> Option<String> {
    let mut counts: HashMap<String, (usize, &str)> = HashMap::new();
    for v in values.iter().map(|v| v.trim()).filter(|v| !v.is_empty()) {
        counts.entry(v.to_lowercase()).or_insert((0, v)).0 += 1;
    }
    counts
        .into_values()
        .max_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.cmp(a.1)))
        .filter(|(n, _)| 2 * n > values.len())
        .map(|(_, v)| v.to_string())
}

/// The artist and album a folder's name or place says: "Air - Moon
/// Safari", or Moon Safari in an Air folder. The artist is `None` when
/// neither says.
fn folder_names(dir: &Path, root: &Path) -> (Option<String>, String) {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if let Some((artist, album)) = name.split_once(" - ") {
        let (artist, album) = (artist.trim(), album.trim());
        if !artist.is_empty() && !album.is_empty() {
            return (Some(artist.into()), album.into());
        }
    }
    let parent = dir
        .parent()
        .filter(|p| p.starts_with(root) && *p != root)
        .and_then(Path::file_name)
        .map(|n| n.to_string_lossy().into_owned());
    (parent, name)
}

/// An album's name and artist, from its tracks' album and artist tags:
/// the album most have (else the folder's name), and the main artist most
/// have (else the folder's artist, else "Various Artists").
pub(super) fn album_title(
    dir: &Path,
    root: &Path,
    albums: &[&str],
    artists: &[&str],
) -> (String, String) {
    let (folder_artist, folder_album) = folder_names(dir, root);
    let name = most(albums).unwrap_or(folder_album);
    let mains: Vec<&str> = artists.iter().map(|a| main_artist(a)).collect();
    let artist = most(&mains)
        .or(folder_artist)
        .unwrap_or_else(|| "Various Artists".into());
    (name, artist)
}

/// The year in a year or date tag ("2019", "2019-03-08", "03/2019").
fn year_of(tag: &str) -> Option<u32> {
    let digits: Vec<char> = tag.chars().collect();
    digits.windows(4).enumerate().find_map(|(i, w)| {
        let before = i.checked_sub(1).and_then(|b| digits.get(b));
        let after = digits.get(i + 4);
        let alone =
            !before.is_some_and(char::is_ascii_digit) && !after.is_some_and(char::is_ascii_digit);
        let year: u32 = w.iter().collect::<String>().parse().ok()?;
        (alone && (1000..3000).contains(&year)).then_some(year)
    })
}

/// An album's year: the one most of its tracks' tags give (the newest of
/// those tied), or `None` when none gives one.
fn album_year(tags: &[&str]) -> Option<u32> {
    let mut counts: HashMap<u32, usize> = HashMap::new();
    for y in tags.iter().filter_map(|t| year_of(t)) {
        *counts.entry(y).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by_key(|&(y, n)| (n, y))
        .map(|(y, _)| y)
}

/// Albums newest first, those without a year last, then by name.
fn newest_first(year: Option<u32>, name: &str) -> (std::cmp::Reverse<u32>, String) {
    (std::cmp::Reverse(year.unwrap_or(0)), name.to_lowercase())
}

/// One album folder in the Albums source.
struct Album {
    dir: PathBuf,
    name: String,
    artist: String,
    year: Option<u32>,
}

/// The Albums tree: All Albums, then each artist (by name, ignoring case)
/// with their albums under them, newest first.
fn artist_tree(root: &Path, albums: Vec<Album>) -> Vec<Node> {
    let mut by_artist: BTreeMap<String, (String, Vec<Album>)> = BTreeMap::new();
    for a in albums {
        by_artist
            .entry(a.artist.to_lowercase())
            .or_insert_with(|| (a.artist.clone(), Vec::new()))
            .1
            .push(a);
    }
    let mut nodes = vec![Node::new(
        "All Albums",
        0,
        0,
        Contents::Under(root.to_path_buf()),
    )];
    for (_, (artist, mut albums)) in by_artist {
        albums.sort_by_cached_key(|a| (newest_first(a.year, &a.name), a.dir.clone()));
        let dirs = albums.iter().map(|a| a.dir.clone()).collect();
        let mut n = Node::new(artist, 1, KIND_ARTIST, Contents::Albums(dirs));
        // Artists start closed, so the list stays short.
        n.expanded = false;
        nodes.push(n);
        for a in albums {
            nodes.push(Node::new(a.name, 2, KIND_ALBUM, Contents::Under(a.dir)));
        }
    }
    nodes
}

/// What finds a node again once the tree is built anew: its folder, or
/// its artist.
fn node_key(n: &Node) -> Option<String> {
    match &n.contents {
        Contents::Under(p) => Some(format!("dir:{}", p.display())),
        Contents::Albums(_) => Some(format!("artist:{}", n.name.to_lowercase())),
        _ => None,
    }
}

/// The tree of `root`'s folders that hold some of `files`, under a node
/// for all of them. Folders with tracks right in them are `leaf` kind
/// (an album, a folder of sets); those with only folders below are folders.
fn folder_tree<'a>(
    title: &str,
    root: &Path,
    files: impl Iterator<Item = &'a Path>,
    leaf: i32,
) -> Vec<Node> {
    let mut dirs: BTreeSet<PathBuf> = BTreeSet::new();
    let mut with_files: BTreeSet<PathBuf> = BTreeSet::new();
    for path in files {
        let Some(dir) = path.parent().filter(|d| d.starts_with(root) && *d != root) else {
            continue;
        };
        with_files.insert(dir.to_path_buf());
        let mut up = Some(dir);
        while let Some(d) = up.filter(|d| *d != root) {
            dirs.insert(d.to_path_buf());
            up = d.parent();
        }
    }
    let mut nodes = vec![Node::new(title, 0, 0, Contents::Under(root.to_path_buf()))];
    // In name order, ignoring case, each folder followed by what's in it.
    let mut sorted: Vec<PathBuf> = dirs.into_iter().collect();
    let key = |p: &PathBuf| {
        p.strip_prefix(root)
            .unwrap_or(p)
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
            .collect::<Vec<_>>()
    };
    sorted.sort_by_key(key);
    for dir in sorted {
        let depth = dir.strip_prefix(root).map_or(1, |r| r.components().count()) as i32;
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let kind = if with_files.contains(&dir) {
            leaf
        } else {
            KIND_FOLDER
        };
        let mut n = Node::new(name, depth, kind, Contents::Under(dir));
        // Artists start closed, so the albums list stays short.
        n.expanded = false;
        nodes.push(n);
    }
    nodes
}

impl Browser {
    pub(super) fn install_albums(
        edit: &LibraryEdit,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let w = with.clone();
        edit.on_play_album(move |row, add| {
            w(&|b| {
                let dir = b
                    .rows
                    .get(row as usize)
                    .copied()
                    .flatten()
                    .and_then(|i| file_path(&b.library[i])?.parent().map(Path::to_path_buf));
                if let Some(dir) = dir {
                    b.play_album(&dir, add);
                }
            });
        });
        let w = with.clone();
        edit.on_play_album_folder(move |index| {
            w(&|b| {
                let dir =
                    b.visible_nodes
                        .get(index as usize)
                        .and_then(|&n| match &b.nodes[n].contents {
                            Contents::Under(d) => Some(d.clone()),
                            _ => None,
                        });
                if let Some(dir) = dir {
                    b.play_album(&dir, false);
                }
            });
        });
    }

    /// Whether `t` is a mixed set: a file in the mixed sets folder.
    pub(super) fn is_mixed_set(&self, t: &Track) -> bool {
        self.mixed_sets
            .as_deref()
            .is_some_and(|m| file_path(t).is_some_and(|p| p.starts_with(m)))
    }

    /// Whether `t` is in the collection: neither a file only shown in
    /// Files nor a mixed set.
    pub(super) fn in_collection(&self, t: &Track) -> bool {
        !is_browsed(t) && !self.is_mixed_set(t)
    }

    /// The folder the Albums or Mixed Sets source shows.
    fn folder_of(&self, source: i32) -> Option<&Path> {
        if source == ALBUMS {
            self.folders.albums.as_deref()
        } else {
            self.mixed_sets.as_deref()
        }
    }

    /// Builds the Albums or Mixed Sets tree again from the library,
    /// keeping the node selected and those opened. Albums go under their
    /// artist; mixed sets keep their folders.
    pub(super) fn rebuild_folder_tree(&mut self, source: i32) {
        let selected = self.nodes.get(self.current_node).and_then(node_key);
        let open: Vec<String> = self
            .nodes
            .iter()
            .filter(|n| n.expanded && n.depth > 0)
            .filter_map(node_key)
            .collect();
        let Some(root) = self.folder_of(source).map(Path::to_path_buf) else {
            self.nodes = Vec::new();
            self.current_node = 0;
            return;
        };
        let mut nodes = if source == ALBUMS {
            artist_tree(&root, self.albums_in(&root))
        } else {
            let files = self
                .library
                .iter()
                .filter(|t| !is_browsed(t) && !t.info.missing)
                .filter_map(|t| file_path(t));
            folder_tree("All Recordings", &root, files, KIND_MIX)
        };
        for n in &mut nodes {
            if n.depth > 0 && node_key(n).is_some_and(|k| open.contains(&k)) {
                n.expanded = true;
            }
        }
        let at = selected
            .and_then(|k| nodes.iter().position(|n| node_key(n) == Some(k.clone())))
            .unwrap_or(0);
        // The folders above the selection are opened.
        let mut depth = nodes[at].depth;
        for i in (0..at).rev() {
            if nodes[i].depth < depth {
                nodes[i].expanded = true;
                depth = nodes[i].depth;
            }
        }
        self.nodes = nodes;
        self.current_node = at;
    }

    /// The album folders under `root` (each folder with tracks right in
    /// it), named by their tracks' tags.
    fn albums_in(&self, root: &Path) -> Vec<Album> {
        let mut tracks: HashMap<&Path, Vec<usize>> = HashMap::new();
        for (i, t) in self.library.iter().enumerate() {
            if is_browsed(t) || t.info.missing {
                continue;
            }
            let Some(dir) = file_path(t).and_then(Path::parent) else {
                continue;
            };
            if dir.starts_with(root) && dir != root {
                tracks.entry(dir).or_default().push(i);
            }
        }
        tracks
            .into_iter()
            .map(|(dir, ids)| {
                let (name, artist) = self.album_names(dir, root, &ids);
                Album {
                    dir: dir.to_path_buf(),
                    name,
                    artist,
                    year: self.album_year(&ids),
                }
            })
            .collect()
    }

    /// The name and artist of the album in `dir` holding tracks `ids`.
    fn album_names(&self, dir: &Path, root: &Path, ids: &[usize]) -> (String, String) {
        let albums: Vec<&str> = ids
            .iter()
            .map(|&i| self.library[i].album.as_str())
            .collect();
        let artists: Vec<&str> = ids
            .iter()
            .map(|&i| self.library[i].artist.as_str())
            .collect();
        album_title(dir, root, &albums, &artists)
    }

    /// The year of the album holding tracks `ids`.
    fn album_year(&self, ids: &[usize]) -> Option<u32> {
        let years: Vec<&str> = ids
            .iter()
            .map(|&i| self.library[i].info.year.as_str())
            .collect();
        album_year(&years)
    }

    /// The tracks right in album folder `dir`, as the album plays.
    pub(super) fn album_tracks(&self, dir: &Path) -> Vec<usize> {
        self.tracks_under(dir)
            .into_iter()
            .filter(|&t| file_path(&self.library[t]).and_then(Path::parent) == Some(dir))
            .collect()
    }

    /// The collection's tracks in `dir` and below, as an album plays: by
    /// folder, then track number, then file name.
    pub(super) fn tracks_under(&self, dir: &Path) -> Vec<usize> {
        let mut found: Vec<(usize, &Path)> = (0..self.library.len())
            .filter(|&i| !is_browsed(&self.library[i]) && !self.library[i].info.missing)
            .filter_map(|i| Some((i, file_path(&self.library[i])?)))
            .filter(|(_, p)| p.starts_with(dir))
            .collect();
        found.sort_by_cached_key(|&(i, p)| {
            let number = match self.library[i].info.track_number {
                0 => u32::MAX,
                n => n,
            };
            let lower = |p: Option<&Path>| p.map(|p| p.to_string_lossy().to_lowercase());
            (
                lower(p.parent()),
                number,
                lower(p.file_name().map(Path::new)),
            )
        });
        found.into_iter().map(|(i, _)| i).collect()
    }

    /// Puts the album in `dir` on the Automix list (after what is there
    /// with `add`, else instead of it) and, unless adding, has Automix play
    /// it gaplessly.
    fn play_album(&mut self, dir: &Path, add: bool) {
        // Only the album's own tracks, not those in folders below it.
        let album = self.album_tracks(dir);
        if album.is_empty() {
            return;
        }
        self.adopt(&album);
        if add {
            self.side[0].extend(album);
        } else {
            self.side[0] = album;
            self.album_request = true;
        }
        self.refresh_side();
    }

    pub(super) fn install_album_covers(
        ui: &AlbumsUi,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        ui.set_covers(settings::flag(ALBUMS_COVERS, true));
        let w = with.clone();
        ui.on_set_covers(move |on| {
            w(&|b| {
                settings::set(ALBUMS_COVERS, serde_json::json!(on));
                b.album_back = None;
                b.refresh_album_ui();
            })
        });
        let w = with.clone();
        ui.on_open(move |i| w(&|b| b.open_album(i as usize)));
        let w = with.clone();
        ui.on_back(move || {
            w(&|b| {
                let back = b.album_back.take().or_else(|| {
                    // The folder above the album: its artist, or All Albums.
                    let depth = b.nodes.get(b.current_node)?.depth;
                    (0..b.current_node)
                        .rev()
                        .find(|&n| b.nodes[n].depth < depth)
                });
                if let Some(n) = back {
                    b.show_node(n);
                }
            })
        });
        let w = with.clone();
        ui.on_play(move |i, add| {
            w(&|b| {
                if let Some(dir) = b.album_tiles.get(i as usize).cloned() {
                    b.play_album(&dir, add);
                }
            })
        });
    }

    /// Whether the Albums source shows covers (when the folder selected
    /// isn't an album itself).
    fn showing_covers(&self) -> bool {
        self.tree == ALBUMS && settings::flag(ALBUMS_COVERS, true)
    }

    /// Whether the node selected is an album.
    fn on_album(&self) -> bool {
        self.nodes
            .get(self.current_node)
            .is_some_and(|n| n.kind == KIND_ALBUM)
    }

    /// Shows the covers view (or not) for what the list now holds: one
    /// tile per album folder among the listed tracks, in their order.
    pub(super) fn refresh_album_ui(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let ui = app.global::<AlbumsUi>();
        let covers = self.showing_covers();
        let grid = covers && !self.on_album();
        ui.set_offered(self.tree == ALBUMS);
        ui.set_covers(covers || (self.tree != ALBUMS && settings::flag(ALBUMS_COVERS, true)));
        ui.set_grid(grid);
        ui.set_opened(covers && self.on_album());
        // "‹ Artist" back from an album: to where it was opened from, else
        // the artist above it.
        let back = self.album_back.or_else(|| {
            let depth = self.nodes.get(self.current_node)?.depth;
            (0..self.current_node)
                .rev()
                .find(|&n| self.nodes[n].depth < depth)
        });
        let label = match back.and_then(|n| self.nodes.get(n)) {
            Some(n) if n.depth > 0 => n.name.clone(),
            _ => "Albums".into(),
        };
        ui.set_back_to(label.into());
        if !grid {
            if !self.album_tiles.is_empty() {
                self.album_tiles.clear();
                ui.set_tiles(ModelRc::default());
            }
            return;
        }
        let mut order: Vec<PathBuf> = Vec::new();
        let mut tracks: HashMap<PathBuf, Vec<usize>> = HashMap::new();
        for &i in self.rows.iter().flatten() {
            let Some(dir) = file_path(&self.library[i]).and_then(Path::parent) else {
                continue;
            };
            let list = tracks.entry(dir.to_path_buf()).or_default();
            if list.is_empty() {
                order.push(dir.to_path_buf());
            }
            list.push(i);
        }
        let root = self.folders.albums.clone().unwrap_or_default();
        // Under an artist the year says enough; elsewhere the artist too.
        let on_artist = self
            .nodes
            .get(self.current_node)
            .is_some_and(|n| n.kind == KIND_ARTIST);
        let mut shown: Vec<(PathBuf, Option<u32>, AlbumTile)> = order
            .into_iter()
            .map(|dir| {
                let ids = &tracks[&dir];
                let (name, artist) = self.album_names(&dir, &root, ids);
                let year = self.album_year(ids);
                let detail = match year {
                    Some(y) if on_artist => y.to_string(),
                    Some(y) => format!("{y} · {artist}"),
                    None => artist.clone(),
                };
                let cover = self.album_art.get(&self.library[ids[0]]);
                let tile = AlbumTile {
                    name: name.into(),
                    artist: artist.into(),
                    detail: detail.into(),
                    has_cover: cover.is_some(),
                    cover: cover.unwrap_or_default(),
                };
                (dir, year, tile)
            })
            .collect();
        shown.sort_by_cached_key(|(_, year, t)| newest_first(*year, &t.name));
        let (order, tiles): (Vec<PathBuf>, Vec<AlbumTile>) =
            shown.into_iter().map(|(d, _, t)| (d, t)).unzip();
        self.album_tiles = order;
        ui.set_tiles(ModelRc::new(VecModel::from(tiles)));
    }

    /// Selects tree node `n` and lists it, as clicking it does.
    fn show_node(&mut self, n: usize) {
        self.current_node = n;
        // The folders above it open, so it shows in the tree.
        let mut depth = self.nodes[n].depth;
        for i in (0..n).rev() {
            if self.nodes[i].depth < depth {
                self.nodes[i].expanded = true;
                depth = self.nodes[i].depth;
            }
        }
        if let Some(app) = self.app.upgrade() {
            app.set_current_track(-1);
        }
        self.refresh_tree();
        self.refresh_rows();
    }

    /// Opens the album at `index` of the covers: its tracks in the list,
    /// with "‹ Albums" back to the covers.
    fn open_album(&mut self, index: usize) {
        let Some(dir) = self.album_tiles.get(index) else {
            return;
        };
        let Some(n) = self
            .nodes
            .iter()
            .position(|n| matches!(&n.contents, Contents::Under(d) if d == dir))
        else {
            return;
        };
        self.album_back = Some(self.current_node);
        self.show_node(n);
    }

    /// Whether Play Album was picked since this was last asked: Automix
    /// then plays the Automix list gaplessly.
    pub fn take_album_request(&mut self) -> bool {
        std::mem::take(&mut self.album_request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_album_is_named_by_most_of_its_tags() {
        let root = Path::new("/m/Albums");
        let dir = Path::new("/m/Albums/Air/Moon Safari");
        assert_eq!(
            album_title(
                dir,
                root,
                &["Moon Safari", "moon safari", ""],
                &["Air", "Air feat. Beth Hirsch", "Air"]
            ),
            ("Moon Safari".into(), "Air".into())
        );
        // No album tags: the folder's name; artists all differ: the
        // folder's artist.
        assert_eq!(
            album_title(dir, root, &["", ""], &["Air", "Beck"]),
            ("Moon Safari".into(), "Air".into())
        );
        // "Artist - Album" folders say both.
        let dir = Path::new("/m/Albums/Jon Hopkins - RITUAL");
        assert_eq!(
            album_title(dir, root, &["", ""], &["Jon Hopkins & 7RAYS", "7RAYS"]),
            ("RITUAL".into(), "Jon Hopkins".into())
        );
        // Nothing says who: various.
        let dir = Path::new("/m/Albums/Cafe del Mar");
        assert_eq!(
            album_title(dir, root, &[""], &["A", "B"]),
            ("Cafe del Mar".into(), "Various Artists".into())
        );
    }

    #[test]
    fn albums_go_under_their_artist() {
        let album = |dir: &str, name: &str, artist: &str| Album {
            dir: PathBuf::from(dir),
            name: name.into(),
            artist: artist.into(),
            year: None,
        };
        let nodes = artist_tree(
            Path::new("/m/Albums"),
            vec![
                album("/m/Albums/Jon Hopkins - RITUAL", "RITUAL", "Jon Hopkins"),
                album("/m/Albums/East Forest - Spores", "Spores", "East Forest"),
                album(
                    "/m/Albums/jon hopkins - Music",
                    "Music for Psychedelic Therapy",
                    "jon hopkins",
                ),
                album(
                    "/m/Albums/East Forest - Music",
                    "Music for Mushrooms",
                    "East Forest",
                ),
            ],
        );
        let shown: Vec<(&str, i32, i32)> = nodes
            .iter()
            .map(|n| (n.name.as_str(), n.depth, n.kind))
            .collect();
        assert_eq!(
            shown,
            [
                ("All Albums", 0, 0),
                ("East Forest", 1, KIND_ARTIST),
                ("Music for Mushrooms", 2, KIND_ALBUM),
                ("Spores", 2, KIND_ALBUM),
                ("Jon Hopkins", 1, KIND_ARTIST),
                ("Music for Psychedelic Therapy", 2, KIND_ALBUM),
                ("RITUAL", 2, KIND_ALBUM),
            ]
        );
        // An artist lists their albums in the tree's order.
        let Contents::Albums(dirs) = &nodes[1].contents else {
            panic!("an artist holds their albums");
        };
        assert_eq!(
            dirs,
            &[
                PathBuf::from("/m/Albums/East Forest - Music"),
                PathBuf::from("/m/Albums/East Forest - Spores"),
            ]
        );
    }

    #[test]
    fn albums_go_newest_first() {
        assert_eq!(year_of("2019-03-08"), Some(2019));
        assert_eq!(year_of("03/2019"), Some(2019));
        assert_eq!(year_of("12345"), None);
        assert_eq!(year_of(""), None);
        assert_eq!(album_year(&["2018", "2018-05-01", "2021", ""]), Some(2018));
        assert_eq!(album_year(&["2018", "2021"]), Some(2021));
        let album = |name: &str, year: Option<u32>| Album {
            dir: PathBuf::from(format!("/m/Albums/{name}")),
            name: name.into(),
            artist: "Jon Hopkins".into(),
            year,
        };
        let nodes = artist_tree(
            Path::new("/m/Albums"),
            vec![
                album("Immunity", Some(2013)),
                album("Demos", None),
                album("RITUAL", Some(2024)),
                album("Singularity", Some(2018)),
            ],
        );
        let names: Vec<&str> = nodes.iter().skip(2).map(|n| n.name.as_str()).collect();
        assert_eq!(names, ["RITUAL", "Singularity", "Immunity", "Demos"]);
    }

    #[test]
    fn folders_nest_in_the_folder_tree() {
        let files = [
            "/m/Albums/Air/Moon Safari/01.flac",
            "/m/Albums/Air/Talkie Walkie/01.flac",
            "/m/Albums/bonobo - migration/01.mp3",
            "/m/Albums/loose.mp3",
            "/m/Other/x.mp3",
        ];
        let nodes = folder_tree(
            "All Albums",
            Path::new("/m/Albums"),
            files.iter().map(Path::new),
            KIND_ALBUM,
        );
        let shown: Vec<(&str, i32, i32)> = nodes
            .iter()
            .map(|n| (n.name.as_str(), n.depth, n.kind))
            .collect();
        assert_eq!(
            shown,
            [
                ("All Albums", 0, 0),
                ("Air", 1, KIND_FOLDER),
                ("Moon Safari", 2, KIND_ALBUM),
                ("Talkie Walkie", 2, KIND_ALBUM),
                ("bonobo - migration", 1, KIND_ALBUM),
            ]
        );
    }
}
