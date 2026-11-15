//! Crates in the browser (see [`djsugar_library::sets`]): the crates each
//! track is in, for the Crates column and `crate:`, and tidying the lists
//! from Collection Health.

use djsugar_library::sets::ListHealth;

use super::filters::CrateLook;
use super::Browser;

impl Browser {
    /// Reads again which crates each track is in, and what Collection Health
    /// finds in the lists.
    pub(super) fn refresh_crates(&mut self) {
        let Some(db) = &self.db else { return };
        self.crates = db.crates_by_track().unwrap_or_default();
        self.list_health = db.list_health().unwrap_or_default();
        // How each crate looks, for the Crates chip's list.
        self.filter.crate_looks = db
            .playlists()
            .unwrap_or_default()
            .into_iter()
            .filter(djsugar_library::sets::is_crate)
            .map(|p| {
                let icon = p
                    .icon
                    .as_deref()
                    .and_then(|name| djsugar_library::LIST_ICONS.iter().position(|&i| i == name))
                    .unwrap_or(0);
                let look = CrateLook {
                    icon,
                    color: p.color,
                };
                (p.name.to_lowercase(), look)
            })
            .collect();
    }

    /// What Collection Health says about the lists; empty when they're fine.
    pub(super) fn list_health_text(&self) -> String {
        let ListHealth { empty, dead } = &self.list_health;
        let mut parts = Vec::new();
        if !empty.is_empty() {
            parts.push(format!(
                "{} empty crate{}",
                empty.len(),
                if empty.len() == 1 { "" } else { "s" }
            ));
        }
        if *dead > 0 {
            parts.push(format!(
                "{dead} entr{} for tracks removed from your collection",
                if *dead == 1 { "y" } else { "ies" }
            ));
        }
        parts.join(", ")
    }

    /// Deletes empty crates and entries for removed tracks.
    pub(super) fn tidy_lists(&mut self) {
        let Some(db) = &mut self.db else { return };
        self.health_status = match db.tidy_lists() {
            Ok((lists, entries)) => {
                let mut s = Vec::new();
                if lists > 0 {
                    s.push(format!(
                        "Deleted {lists} empty crate{}",
                        if lists == 1 { "" } else { "s" }
                    ));
                }
                if entries > 0 {
                    s.push(format!(
                        "took {entries} removed track{} out of lists",
                        if entries == 1 { "" } else { "s" }
                    ));
                }
                if s.is_empty() {
                    "The lists were already tidy.".into()
                } else {
                    let s = s.join(" and ");
                    let mut c = s.chars();
                    c.next()
                        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                        .unwrap_or_default()
                        + "."
                }
            }
            Err(e) => format!("Couldn't tidy the lists: {e}"),
        };
        self.reload_tree(None);
        self.refresh_health_ui();
    }
}
