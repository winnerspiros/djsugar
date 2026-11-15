//! Where each list was left: how far it was scrolled and which track was
//! picked, so going back to a list after another shows it as it was. Kept
//! only while DJ Sugar runs.

use slint::Model;

use super::{Browser, Contents};
use crate::TrackRow;

/// A list as it was left.
#[derive(Clone)]
pub(super) struct View {
    /// How far down the list was scrolled.
    y: f32,
    /// The current track: a collection track by its id, any other by its
    /// title, artist and album; and how far below the top of the view its
    /// row was.
    track: Option<(Track, f32)>,
}

#[derive(Clone, PartialEq)]
enum Track {
    Local(usize),
    Named(String, String, String),
}

/// A list's place in the browser: the source and the list in its tree.
pub(super) type ViewKey = (i32, String);

/// The height of a track list row, as `TrackList` draws it.
const ROW_HEIGHT: f32 = 32.0;

fn names(r: &TrackRow) -> Track {
    Track::Named(
        r.title.to_string(),
        r.artist.to_string(),
        r.album.to_string(),
    )
}

impl Browser {
    /// The list shown now, by what it holds: a folder by its path, a
    /// streaming playlist by its key, any other list by its id or name.
    fn view_key(&self) -> Option<ViewKey> {
        let n = self.nodes.get(self.current_node)?;
        let name = match &n.contents {
            Contents::Dir(p) | Contents::Under(p) => format!("dir:{}", p.display()),
            Contents::Stream(k) => format!("stream:{k}"),
            _ => Self::node_key(n),
        };
        let part = match (self.health_view, n.history) {
            (true, _) => "health",
            (_, true) => "history",
            _ => "list",
        };
        Some((self.tree, format!("{part}/{name}")))
    }

    /// Notes where the list shown now was left, before another is shown.
    pub(super) fn park_view(&mut self) {
        let (Some(app), Some(key)) = (self.app.upgrade(), self.shown_view.clone()) else {
            return;
        };
        // Left before its tracks came in: still where it was before.
        if let Some(view) = self.waiting_view.take() {
            self.views.insert(key, view);
            return;
        }
        let y = app.get_track_scroll_y();
        let track = usize::try_from(app.get_current_track()).ok().and_then(|i| {
            let row = self.list.row_data(i)?;
            let track = match self.rows.get(i) {
                Some(Some(id)) => Track::Local(*id),
                _ => names(&row),
            };
            Some((track, i as f32 * ROW_HEIGHT - y))
        });
        self.views.insert(key, View { y, track });
    }

    /// After the rows are filled: a list other than the one shown before
    /// comes back where it was left, or at its top. A list whose tracks
    /// are still on their way waits for them.
    pub(super) fn follow_view(&mut self) {
        let key = self.view_key();
        if key != self.shown_view {
            self.waiting_view = key.as_ref().and_then(|k| self.views.get(k).cloned());
            if self.waiting_view.is_none() {
                self.show_view_at(-1, 0.0, 0.0);
            }
            self.shown_view = key;
        }
        if self.list.row_count() == 0 {
            return;
        }
        let Some(view) = self.waiting_view.take() else {
            return;
        };
        let found = view.track.and_then(|(track, offset)| {
            let at = match &track {
                Track::Local(id) => self.rows.iter().position(|r| *r == Some(*id)),
                Track::Named(..) => (0..self.list.row_count()).find(|&i| {
                    self.rows.get(i).is_none_or(|r| r.is_none())
                        && self.list.row_data(i).is_some_and(|r| names(&r) == track)
                }),
            };
            Some((at?, offset))
        });
        match found {
            Some((row, offset)) => self.show_view_at(row as i32, offset, view.y),
            None => self.show_view_at(-1, 0.0, view.y),
        }
    }

    fn show_view_at(&mut self, row: i32, offset: f32, y: f32) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        self.pick_only(row);
        app.invoke_show_tracks_at(row, offset, y);
    }
}

#[cfg(test)]
mod tests {
    use super::{names, Track};
    use crate::TrackRow;

    #[test]
    fn rows_named_alike_match() {
        let row = |t: &str| TrackRow {
            title: t.into(),
            artist: "A".into(),
            album: "B".into(),
            ..Default::default()
        };
        assert!(names(&row("One")) == names(&row("One")));
        assert!(names(&row("One")) != names(&row("Two")));
        assert!(Track::Local(3) != names(&row("One")));
    }
}
