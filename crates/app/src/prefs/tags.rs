//! Settings › Collection › Tags: tag groups and their tags.

use super::*;

impl Prefs {
    /// Settings › Collection › Tags: the DJ's tag groups, to add, rename,
    /// set to one tag per track or remove, and the grouping's name.
    pub(super) fn tag_items(&self) -> Vec<PrefItem> {
        let mut items = vec![item(
            "",
            NOTE,
            "",
            "Tags sit in groups you make, such as Mood, Instruments or Vibe. Each group \
             is a search word (mood:uplifting) and a filter on the bar above the track list.",
        )];
        let Some(db) = open_library() else {
            return items;
        };
        let groups = db.tag_groups().unwrap_or_default();
        for (n, g) in groups.iter().enumerate() {
            let used = db.tag_group_use(g.id).unwrap_or_default();
            let usage = match (used.tags, used.tracks) {
                (0, _) => "No tags yet".to_string(),
                (t, n) => format!("{} on {}", count(t, "tag"), count(n, "track")),
            };
            let open = self.open_group == Some(g.id);
            items.push(PrefItem {
                value: g.name.clone().into(),
                choices: strings(&["Several", "One"]),
                chosen: g.single as i32,
                on: open,
                action: if n == 0 { "first".into() } else { "".into() },
                ..item(
                    &format!("tag_group:{}", g.id),
                    TAG_GROUP,
                    &g.name,
                    &format!("{usage} · {}:", query::keyword(&g.name)),
                )
            });
            if self.removing_group == Some(g.id) {
                items.push(PrefItem {
                    action: "Remove".into(),
                    ..item(
                        &format!("tag_group_remove:{}", g.id),
                        BUTTON,
                        &format!("Remove {}?", g.name),
                        &format!("Its tags come off {}.", count(used.tracks, "track")),
                    )
                });
            }
            if open {
                items.extend(self.tag_rows(&db, g.id));
            }
        }
        items.push(PrefItem {
            action: "Add".into(),
            ..item(
                "add_tag_group",
                BUTTON,
                "Add a group",
                "Another set of tags, such as Vibe or Energy.",
            )
        });
        items.push(item("", HEADING, "GROUPING", ""));
        items.push(PrefItem {
            value: settings::field_names().grouping.into(),
            ..item(
                "grouping_name",
                TEXT,
                "Grouping",
                "What you use the Grouping tag for, such as Trip Stage.",
            )
        });
        let grouping = settings::field_names().grouping;
        items.push(PrefItem {
            action: "Copy".into(),
            value: self.grouping_note.clone().into(),
            ..item(
                "copy_grouping",
                BUTTON,
                "Copy into a tag group",
                &if self.grouping_note.is_empty() {
                    format!(
                        "Makes a {grouping} group with each track's {grouping} as a tag, so a \
                         track can have more than one. Grouping stays as it is."
                    )
                } else {
                    self.grouping_note.clone()
                },
            )
        });
        items
    }

    /// An open group's tags: color, name (renamed on Return; a name the
    /// group has merges the two) and ×.
    pub(super) fn tag_rows(&self, db: &djsugar_library::Library, group: i64) -> Vec<PrefItem> {
        let tags = db.tags_of_group(group).unwrap_or_default();
        if tags.is_empty() {
            return vec![item(
                "",
                NOTE,
                "",
                "No tags in this group yet: add them in the Info tab.",
            )];
        }
        let mut colors = vec!["No color".to_string()];
        colors.extend(query::COLOR_NAMES.iter().map(|(n, _)| capitalized(n)));
        let mut rows = vec![item(
            "",
            NOTE,
            "",
            "Rename a tag and press Return: it changes on every track. A name the group has \
             already merges the two.",
        )];
        for t in tags {
            let chosen = t
                .color
                .and_then(|c| query::COLOR_NAMES.iter().position(|(_, x)| *x == c))
                .map_or(0, |i| i as i32 + 1);
            rows.push(PrefItem {
                value: t.name.clone().into(),
                choices: strings(&colors),
                chosen,
                ..item(
                    &format!("tag:{}", t.id),
                    TAG_ROW,
                    &t.name,
                    &count(t.tracks, "track"),
                )
            });
            if self.removing_tag == Some(t.id) {
                rows.push(PrefItem {
                    action: "Delete".into(),
                    ..item(
                        &format!("tag_delete:{}", t.id),
                        BUTTON,
                        &format!("Delete {}?", t.name),
                        &format!("It comes off {}.", count(t.tracks, "track")),
                    )
                });
            }
        }
        rows
    }

    /// A tag's ×, Delete, a group's Tags or ↑. Whether `id` was one.
    pub(super) fn tag_pressed(&mut self, id: &str) -> bool {
        let number = |prefix: &str, suffix: &str| {
            id.strip_prefix(prefix)
                .and_then(|x| x.strip_suffix(suffix))
                .and_then(|x| x.parse::<i64>().ok())
        };
        if let Some(t) = id.strip_prefix("tag_delete:").and_then(|x| x.parse().ok()) {
            self.removing_tag = None;
            with_library(|db| db.delete_tag(t));
            settings::tags_changed();
        } else if let Some(t) = number("tag:", "#remove") {
            let unused = open_library()
                .and_then(|db| db.tags_of_group(self.open_group?).ok())
                .is_some_and(|tags| tags.iter().any(|x| x.id == t && x.tracks == 0));
            if unused {
                self.removing_tag = None;
                with_library(|db| db.delete_tag(t));
                settings::tags_changed();
            } else {
                self.removing_tag = (self.removing_tag != Some(t)).then_some(t);
            }
        } else if let Some(g) = number("tag_group:", "#open") {
            self.open_group = (self.open_group != Some(g)).then_some(g);
            self.removing_tag = None;
        } else if let Some(g) = number("tag_group:", "#up") {
            with_library(|db| {
                let at = db.tag_groups()?.iter().position(|x| x.id == g);
                match at {
                    Some(at) if at > 0 => db.move_tag_group(g, at - 1),
                    _ => Ok(()),
                }
            });
        } else {
            return false;
        }
        true
    }

    /// Remove on a tag group: at once for one no track uses, otherwise
    /// after a second press on the row that asks. Whether `id` was one.
    pub(super) fn tag_group_pressed(&mut self, id: &str) -> bool {
        let group = |prefix: &str| {
            id.strip_prefix(prefix)
                .and_then(|g| g.trim_end_matches("#remove").parse::<i64>().ok())
        };
        if let Some(g) = group("tag_group_remove:") {
            self.removing_group = None;
            with_library(|db| db.remove_tag_group(g));
            return true;
        }
        let Some(g) = group("tag_group:").filter(|_| id.ends_with("#remove")) else {
            return false;
        };
        let unused = open_library()
            .and_then(|db| db.tag_group_use(g).ok())
            .is_some_and(|u| u.tracks == 0);
        if unused {
            self.removing_group = None;
            with_library(|db| db.remove_tag_group(g));
        } else {
            self.removing_group = (self.removing_group != Some(g)).then_some(g);
        }
        true
    }
}
