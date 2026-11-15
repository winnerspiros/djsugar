//! The DJ's tag groups as chips in the Info tab: each tag of a group is a
//! chip, shown on all the tracks shown or on some ("Dreamy 5/12"). Tags
//! are added to and taken off every track at once, as one change Undo takes
//! back, and the field suggests tags already in the collection.

use std::collections::{BTreeMap, HashMap};

use djsugar_engine::{hashtags, same_tag as same, split_tags};
use djsugar_library::cleanup::Field;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use super::Browser;
use crate::{LibraryEdit, MenuTag, TagChip, TrackInfo};
use djsugar_engine::Track;

/// How many tags a field suggests while typing.
const SUGGESTIONS: usize = 6;
/// How many tags the track menu's Add Tag offers, and remembers as added
/// lately.
const MENU_TAGS: usize = 12;
const RECENT: usize = 8;

/// The tags typed in a group's field: parted by commas ("Peak Time,
/// Dark"), or hashtags ("#dark #synth") as VirtualDJ users type them.
pub(super) fn typed_tags(typed: &str) -> Vec<String> {
    if typed.contains('#') {
        hashtags(typed)
    } else {
        split_tags(typed)
    }
}

/// The tags of `group` on `tracks`, each with how many tracks have it:
/// in the order of the first track's, then the others' by how many have
/// them.
pub(super) fn chips_of(tracks: &[&Track], group: i64) -> Vec<(String, usize)> {
    let mut order: Vec<String> = Vec::new();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for t in tracks {
        for tag in t.info.tags_in(group) {
            let key = tag.to_lowercase();
            if !counts.contains_key(&key) {
                order.push(tag.to_string());
            }
            *counts.entry(key).or_default() += 1;
        }
    }
    let count = |t: &String| counts[&t.to_lowercase()];
    let first = tracks.first().map_or(0, |t| t.info.tags_in(group).count());
    let (mine, rest) = order.split_at(first.min(order.len()));
    let mut rest = rest.to_vec();
    rest.sort_by(|a, b| {
        count(b)
            .cmp(&count(a))
            .then_with(|| a.to_lowercase().cmp(&b.to_lowercase()))
    });
    mine.iter()
        .chain(rest.iter())
        .map(|t| (t.clone(), count(t)))
        .collect()
}

/// Tags of `group` in the collection that start with `typed` (a `#`
/// ignored), most used first, leaving out `taken`.
pub(super) fn suggest(
    library: &[&Track],
    group: i64,
    typed: &str,
    taken: &[String],
) -> Vec<String> {
    let bare = |s: &str| s.trim_start_matches('#').to_lowercase();
    let typed = bare(typed.trim());
    if typed.is_empty() {
        return Vec::new();
    }
    let mut counts: BTreeMap<String, (String, usize)> = BTreeMap::new();
    for t in library {
        for tag in t.info.tags_in(group) {
            if bare(tag).starts_with(&typed) && !taken.iter().any(|k| same(k, tag)) {
                counts
                    .entry(tag.to_lowercase())
                    .or_insert_with(|| (tag.to_string(), 0))
                    .1 += 1;
            }
        }
    }
    let mut found: Vec<(String, usize)> = counts.into_values().collect();
    found.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    found
        .into_iter()
        .take(SUGGESTIONS)
        .map(|(t, _)| t)
        .collect()
}

/// The tags the track menu offers: `recent` ones (group, tag) first, then
/// the most used in every one of `groups` in `library`; those containing
/// `find` (a `#` ignored) when it isn't empty.
pub(super) fn menu_tags(
    library: &[&Track],
    groups: &[i64],
    recent: &[(i64, String)],
    find: &str,
) -> Vec<(i64, String)> {
    let bare = |s: &str| s.trim_start_matches('#').to_lowercase();
    let find = bare(find.trim());
    let mut counts: BTreeMap<(i64, String), (String, usize)> = BTreeMap::new();
    for t in library {
        for tag in t.info.tags.iter().filter(|t| groups.contains(&t.group)) {
            counts
                .entry((tag.group, tag.name.to_lowercase()))
                .or_insert_with(|| (tag.name.clone(), 0))
                .1 += 1;
        }
    }
    let mut used: Vec<(i64, String, usize)> = counts
        .into_iter()
        .map(|((g, _), (tag, c))| (g, tag, c))
        .collect();
    used.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.1.cmp(&b.1)));
    let mut out: Vec<(i64, String)> = Vec::new();
    let candidates = recent
        .iter()
        .filter(|(g, _)| groups.contains(g))
        .cloned()
        .chain(used.into_iter().map(|(g, t, _)| (g, t)));
    for (g, tag) in candidates {
        if !find.is_empty() && !bare(&tag).contains(&find) {
            continue;
        }
        if !out.iter().any(|(m, t)| *m == g && same(t, &tag)) {
            out.push((g, tag));
        }
        if out.len() == MENU_TAGS {
            break;
        }
    }
    out
}

/// The track's tags in `group` with `tags` added (one per track in a
/// group set to one, the last of them) or taken off, as one line.
fn changed(t: &Track, group: i64, tags: &[String], add: bool) -> String {
    let single = super::query::field_names()
        .group(group)
        .is_some_and(|g| g.single);
    let mut info = t.info.clone();
    for tag in tags {
        if add {
            info.add_tag(group, tag, single);
        } else {
            info.remove_tag(group, tag);
        }
    }
    info.tag_text(group)
}

/// The chips the Info tab draws for `group`.
pub(super) fn chip_model(tracks: &[&Track], group: i64) -> ModelRc<TagChip> {
    let names = super::query::field_names();
    let chips: Vec<TagChip> = chips_of(tracks, group)
        .into_iter()
        .map(|(tag, count)| {
            let color = names.tag_color(group, &tag);
            TagChip {
                count: count as i32,
                tint: slint::Color::from_argb_encoded(0xff00_0000 | color.unwrap_or(0)),
                has_color: color.is_some(),
                tag: tag.into(),
            }
        })
        .collect();
    ModelRc::new(VecModel::from(chips))
}

impl Browser {
    pub(super) fn install_tag_chips(
        ui: &TrackInfo,
        with: impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static,
    ) {
        let w = with.clone();
        ui.on_tag_add(move |id, typed| w(&|b| b.change_tag(&id, &typed, true)));
        let w = with.clone();
        ui.on_tag_remove(move |id, tag| w(&|b| b.change_tag(&id, &tag, false)));
        let w = with;
        ui.on_tag_suggest(move |id, typed| w(&|b| b.suggest_tags(&id, &typed)));
    }

    /// The tracks the Info tab shows: the picked ones, or the one.
    fn info_tracks(&self) -> Vec<(usize, std::sync::Arc<Track>)> {
        if !self.info.many.is_empty() {
            self.info.many.clone()
        } else {
            self.info.shown.clone().into_iter().collect()
        }
    }

    /// Adds the tags typed (`add`), or takes one off, in the tag group
    /// field `id` of every track shown.
    fn change_tag(&mut self, id: &str, typed: &str, add: bool) {
        let Some(Field::Tag(group)) = Field::from_name(id) else {
            return;
        };
        let tags: Vec<String> = if add {
            typed_tags(typed)
        } else {
            vec![typed.trim().to_string()]
        };
        if tags.iter().all(|t| t.is_empty()) {
            return;
        }
        let shown = self.info_tracks();
        if let [(_, t)] = shown.as_slice() {
            // One track: saved as any field typed in the tab.
            let text = changed(t, group, &tags, add);
            self.set_suggestions(Vec::new());
            if text != t.info.tag_text(group) {
                self.info_edit(id, &text);
            }
            return;
        }
        self.set_suggestions(Vec::new());
        let indices: Vec<usize> = shown.iter().map(|(i, _)| *i).collect();
        if let Some(status) = self.tag_tracks(&indices, group, &tags, add) {
            if let Some(app) = self.app.upgrade() {
                let ui = app.global::<TrackInfo>();
                ui.set_status(status.into());
                ui.set_status_ok(true);
            }
        }
    }

    /// Adds `tags` to tag group `group` of the library tracks `indices`,
    /// or takes them off, as one change Undo takes back. Says what it did,
    /// or `None` when nothing changed.
    pub(super) fn tag_tracks(
        &mut self,
        indices: &[usize],
        group: i64,
        tags: &[String],
        add: bool,
    ) -> Option<String> {
        let mut changes = Vec::new();
        for &index in indices {
            let Some(t) = self.library.get(index) else {
                continue;
            };
            let text = changed(t, group, tags, add);
            if text != t.info.tag_text(group) {
                changes.push((index, Field::Tag(group), text));
            }
        }
        if add {
            for tag in tags {
                self.info
                    .recent_tags
                    .retain(|(g, t)| !(*g == group && same(t, tag)));
                self.info.recent_tags.insert(0, (group, tag.to_string()));
            }
            self.info.recent_tags.truncate(RECENT);
        }
        if changes.is_empty() {
            return None;
        }
        let status = self.make_changes(changes, if add { "add tag" } else { "remove tag" });
        self.refresh_tree();
        self.refresh_rows();
        self.refresh_side();
        Some(status)
    }

    /// Fills the track menu's Add Tag panel: tags added lately, then the
    /// collection's most used, those matching `find` when it isn't empty.
    pub(super) fn find_menu_tags(&mut self, find: &str) {
        let names = super::query::field_names();
        let library: Vec<&Track> = self.library.iter().map(|t| &**t).collect();
        let groups: Vec<i64> = names.tags.iter().map(|g| g.id).collect();
        let picks = menu_tags(&library, &groups, &self.info.recent_tags, find);
        let rows: Vec<MenuTag> = picks
            .into_iter()
            .map(|(g, tag)| MenuTag {
                tag: tag.into(),
                id: Field::Tag(g).name().into_owned().into(),
                field: names
                    .group(g)
                    .map(|g| g.name.clone())
                    .unwrap_or_default()
                    .into(),
            })
            .collect();
        if let Some(app) = self.app.upgrade() {
            app.global::<LibraryEdit>()
                .set_tag_picks(ModelRc::new(VecModel::from(rows)));
        }
    }

    /// The track menu's Add Tag: tag `tag` of group field `id` goes on the
    /// rows `first` to `last` (or all the picked rows).
    pub(super) fn add_tag_to_rows(&mut self, first: usize, last: usize, id: &str, tag: &str) {
        let Some(Field::Tag(group)) = Field::from_name(id) else {
            return;
        };
        let tags = split_tags(tag);
        if tags.is_empty() {
            return;
        }
        let indices = self.chosen_tracks(first, last);
        self.adopt(&indices);
        self.tag_tracks(&indices, group, &tags, true);
    }

    /// Offers the collection's tags that start with what is typed in the
    /// tag group field `id`.
    fn suggest_tags(&mut self, id: &str, typed: &str) {
        let Some(Field::Tag(group)) = Field::from_name(id) else {
            return;
        };
        let shown = self.info_tracks();
        let tracks: Vec<&Track> = shown.iter().map(|(_, t)| &**t).collect();
        // Tags every track has already aren't offered again.
        let taken: Vec<String> = chips_of(&tracks, group)
            .into_iter()
            .filter(|(_, c)| *c == tracks.len())
            .map(|(t, _)| t)
            .collect();
        let library: Vec<&Track> = self.library.iter().map(|t| &**t).collect();
        let found = suggest(&library, group, typed, &taken);
        if let Some(app) = self.app.upgrade() {
            app.global::<TrackInfo>().set_suggest_for(id.into());
        }
        self.set_suggestions(found);
    }

    fn set_suggestions(&self, found: Vec<String>) {
        if let Some(app) = self.app.upgrade() {
            let found: Vec<SharedString> = found.into_iter().map(Into::into).collect();
            app.global::<TrackInfo>()
                .set_suggestions(ModelRc::new(VecModel::from(found)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use djsugar_engine::Source;

    fn track(tags: &str) -> Track {
        let mut t = Track {
            id: 1,
            artist: String::new(),
            title: String::new(),
            album: String::new(),
            genre: String::new(),
            rating: 0,
            bpm: 0.0,
            key: String::new(),
            duration: 0.0,
            first_beat: 0.0,
            source: Source::Synth { seed: 1 },
            waveform: Default::default(),
            audio: None,
            cover: None,
            grid: Default::default(),
            info: Default::default(),
        };
        t.info.set_tags_in(1, split_tags(tags));
        t
    }

    #[test]
    fn reads_typed_tags() {
        assert_eq!(typed_tags("Peak Time, dark"), ["Peak Time", "dark"]);
        assert_eq!(typed_tags("#dark #synth"), ["dark", "synth"]);
        assert!(typed_tags(" , ").is_empty());
    }

    #[test]
    fn adds_and_takes_off_tags() {
        let t = track("dark, synth");
        assert_eq!(
            changed(&t, 1, &["Sunset".into()], true),
            "dark, synth, Sunset"
        );
        assert_eq!(
            changed(&t, 1, &["DARK".into()], true),
            "dark, synth",
            "once, whatever the case"
        );
        assert_eq!(changed(&t, 1, &["Dark".into()], false), "synth");
        assert_eq!(changed(&t, 2, &["a".into()], true), "a");
    }

    #[test]
    fn counts_tags_on_several_tracks() {
        let a = track("uplifting, dreamy");
        let b = track("uplifting, synth, synth");
        let c = track("Uplifting, synth");
        let chips = chips_of(&[&a, &b, &c], 1);
        assert_eq!(
            chips,
            [
                ("uplifting".to_string(), 3),
                ("dreamy".to_string(), 1),
                ("synth".to_string(), 2),
            ],
            "the first track's order, then the rest by count"
        );
        assert!(chips_of(&[&a], 2).is_empty());
    }

    #[test]
    fn suggests_tags_in_use() {
        let a = track("sunset, synth");
        let b = track("sunrise, synth");
        let c = track("synth, sunset");
        let lib = [&a, &b, &c];
        assert_eq!(suggest(&lib, 1, "su", &[]), ["sunset", "sunrise"]);
        assert_eq!(suggest(&lib, 1, "#sy", &[]), ["synth"]);
        assert_eq!(
            suggest(&lib, 1, "s", &["synth".to_string()]),
            ["sunset", "sunrise"],
            "tags already on every track aren't offered"
        );
        assert!(suggest(&lib, 1, " ", &[]).is_empty());
    }

    #[test]
    fn offers_recent_then_most_used_tags() {
        let a = track("sunset, synth");
        let b = track("synth");
        let mut c = track("dark");
        c.info.set_tags_in(2, ["vox".into()]);
        let lib = [&a, &b, &c];
        let recent = [(1, "dark".to_string())];
        assert_eq!(
            menu_tags(&lib, &[1, 2], &recent, ""),
            [
                (1, "dark".to_string()),
                (1, "synth".to_string()),
                (1, "sunset".to_string()),
                (2, "vox".to_string()),
            ]
        );
        assert_eq!(
            menu_tags(&lib, &[1, 2], &recent, "#S"),
            [(1, "synth".to_string()), (1, "sunset".to_string())]
        );
        assert_eq!(menu_tags(&lib, &[1], &[], "vox"), []);
    }
}
