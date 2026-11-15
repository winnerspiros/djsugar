//! Keep going, Player mode's cruise control: while it is on and Automix
//! plays, Up Next never runs dry. When fewer than two tracks wait, the
//! next pick goes on the list; the next few show greyed after it. Picks
//! come from a pool (everything, a list, an artist) and mix with the track
//! before them (see `crate::keep_going`). Start Radio switches it on with
//! a seed. Off at first, and only in Player mode.

use std::collections::HashSet;

use slint::{ComponentHandle, SharedString};

use super::track_menu::main_artist;
use super::{is_browsed, Browser};
use crate::keep_going::{self as kg, Item};
use crate::KeepGoingUi;

/// How many picks show greyed after Up Next.
pub(super) const SHOWN: usize = 3;
/// Up Next is topped up while fewer than this many tracks are on it.
const WAITING: usize = 2;
/// Tracks played this recently are left out, in seconds.
const RECENT: i64 = 3 * 3600;

/// Where picks come from.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) enum Pool {
    /// The whole collection.
    #[default]
    All,
    /// One of the DJ's lists: its id and name.
    List(i64, String),
    /// An artist's tracks (by their main artist, lower case) and those
    /// sharing their genres.
    Artist(String, String),
}

impl Pool {
    fn name(&self) -> &str {
        match self {
            Pool::All => "Everything",
            Pool::List(_, name) => name,
            Pool::Artist(_, shown) => shown,
        }
    }
}

#[derive(Default)]
pub(super) struct KeepGoing {
    on: bool,
    /// The app is in Player mode.
    player: bool,
    pool: Pool,
    /// The next picks, as library indices.
    pub(super) picks: Vec<usize>,
    /// Track ids already picked this run, so none comes twice.
    used: HashSet<usize>,
    /// The track (library index) the picks follow, once worked out;
    /// None while they need working out again.
    after: Option<Option<usize>>,
    /// A track (library index) Start Radio puts next in Up Next.
    seed: Option<usize>,
    /// Automix should be switched on.
    automix: bool,
}

impl Browser {
    pub(super) fn install_keep_going(
        ui: &KeepGoingUi,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let w = with.clone();
        ui.on_set_on(move |on| w(&|b| b.set_keep_going(on)));
        let w = with.clone();
        ui.on_pick_pool(move |i| w(&|b| b.pick_pool(i)));
        let w = with.clone();
        ui.on_start_radio(move |row, artist| w(&|b| b.start_radio(row, artist)));
        let w = with.clone();
        ui.on_start_list_radio(move |node| w(&|b| b.start_list_radio(node)));
    }

    /// Whether the app is in Player mode.
    pub(super) fn keep_going_player(&self) -> bool {
        self.keep_going.player
    }

    /// Whether Keep going adds tracks now: on, and in Player mode.
    pub(super) fn keeps_going(&self) -> bool {
        self.keep_going.on && self.keep_going.player
    }

    fn set_keep_going(&mut self, on: bool) {
        let k = &mut self.keep_going;
        k.on = on;
        k.picks.clear();
        k.after = None;
        if on {
            k.used.clear();
            k.automix = true;
        } else {
            k.seed = None;
        }
        self.show_keep_going();
        self.refresh_side();
    }

    /// Picks from everything (0) or the list the browser shows (1).
    fn pick_pool(&mut self, i: i32) {
        let pool = match i {
            1 => match self.shown_list() {
                Some((id, name)) => Pool::List(id, name),
                None => return,
            },
            _ => Pool::All,
        };
        self.set_pool(pool);
    }

    fn set_pool(&mut self, pool: Pool) {
        let k = &mut self.keep_going;
        k.pool = pool;
        k.picks.clear();
        k.after = None;
        self.show_keep_going();
        self.refresh_side();
    }

    /// The list the browser shows, when it is one of the DJ's.
    fn shown_list(&self) -> Option<(i64, String)> {
        let n = self.nodes.get(self.current_node)?;
        n.playlist.map(|id| (id, n.name.clone()))
    }

    /// Start Radio on a row of the track list: the track goes next, then
    /// picks from everything, or from its artist and their genres.
    fn start_radio(&mut self, row: i32, artist: bool) {
        let Some(index) = self.rows.get(row.max(0) as usize).copied().flatten() else {
            return;
        };
        let pool = if artist {
            let shown = main_artist(&self.library[index].artist).to_string();
            if shown.is_empty() {
                return;
            }
            Pool::Artist(shown.to_lowercase(), shown)
        } else {
            Pool::All
        };
        self.set_keep_going(true);
        self.set_pool(pool);
        self.keep_going.seed = Some(index);
        self.keep_going.used.insert(self.library[index].id);
    }

    /// Start Radio on a crate or playlist in the tree.
    fn start_list_radio(&mut self, node: i32) {
        let Some(n) = self.nodes.get(node.max(0) as usize) else {
            return;
        };
        let Some(id) = n.playlist else { return };
        let pool = Pool::List(id, n.name.clone());
        self.set_keep_going(true);
        self.set_pool(pool);
    }

    /// Whether Keep going wants Automix switched on (once).
    pub fn take_keep_going_automix(&mut self) -> bool {
        std::mem::take(&mut self.keep_going.automix)
    }

    /// Runs Keep going for one frame. `player`: the app is in Player mode;
    /// `playing`: the id of the track the Player bar shows.
    pub fn keep_going_tick(
        &mut self,
        player: bool,
        automix: &crate::automix::AutoMix,
        playing: Option<usize>,
    ) {
        if player != self.keep_going.player {
            self.keep_going.player = player;
            self.refresh_side();
            // Player opens on Home.
            if player && !self.pages_open() {
                self.go_home();
            }
        }
        self.show_keep_going();
        if !self.keeps_going() {
            return;
        }
        if let Some(seed) = self.keep_going.seed.take() {
            let id = self.library[seed].id;
            let at = if automix.is_on() && playing.is_some() {
                let top: Vec<usize> = (0..3)
                    .filter_map(|i| self.automix_at(i))
                    .map(|t| t.id)
                    .collect();
                automix.fixed(&top)
            } else {
                0
            };
            self.add_to_automix_at(id, Some(at));
            self.keep_going.after = None;
        }
        // Also when Keep going has just asked for Automix: it starts
        // this frame, with something to play.
        let automix_on = automix.is_on() || self.keep_going.automix;
        if automix_on && self.automix_len() < WAITING {
            self.work_out_picks(playing);
            let mut added = false;
            while self.automix_len() < WAITING && !self.keep_going.picks.is_empty() {
                let i = self.keep_going.picks.remove(0);
                let id = self.library[i].id;
                self.keep_going.used.insert(id);
                self.side[0].push(i);
                added = true;
            }
            if added {
                self.keep_going.after = None;
            }
        }
        if self.work_out_picks(playing) {
            self.refresh_side();
        }
    }

    /// Works the picks out again when the end of Up Next changed. Returns
    /// whether it did.
    fn work_out_picks(&mut self, playing: Option<usize>) -> bool {
        let end = self.side[0]
            .last()
            .copied()
            .or_else(|| playing.and_then(|id| self.index_of.get(&id).copied()));
        if self.keep_going.after == Some(end) {
            return false;
        }
        self.keep_going.after = Some(end);
        let members = self.pool_members();
        let now = chrono::Utc::now().timestamp();
        let queued: HashSet<usize> = self.side[0].iter().copied().collect();
        let k = &self.keep_going;
        let members: Vec<usize> = members
            .into_iter()
            .filter(|&i| {
                let t = &self.library[i];
                !queued.contains(&i)
                    && !k.used.contains(&t.id)
                    && !self.loaded.contains(&Some(t.id))
                    && t.info.last_played.is_none_or(|p| now - p > RECENT)
            })
            .collect();
        let items: Vec<Item> = members.iter().map(|&i| self.pick_item(i)).collect();
        let after = end.map(|i| self.pick_item(i));
        let mut random = kg::random();
        let picked = kg::picks(&items, after.as_ref(), SHOWN, self.key_rules.0, &mut random);
        self.keep_going.picks = picked.into_iter().map(|p| members[p]).collect();
        true
    }

    fn pick_item(&self, i: usize) -> Item {
        let t = &self.library[i];
        Item {
            bpm: t.bpm,
            key: djsugar_engine::key::Key::parse(&t.key),
            artist: main_artist(&t.artist).to_lowercase(),
        }
    }

    /// The pool's tracks, as library indices.
    fn pool_members(&self) -> Vec<usize> {
        match &self.keep_going.pool {
            Pool::All => (0..self.library.len())
                .filter(|&i| !is_browsed(&self.library[i]))
                .collect(),
            Pool::List(id, _) => self
                .db
                .as_ref()
                .and_then(|db| db.playlist_tracks(*id).ok())
                .unwrap_or_default()
                .into_iter()
                .filter_map(|id| self.index_of.get(&id).copied())
                .collect(),
            Pool::Artist(artist, _) => {
                let by =
                    |i: &usize| main_artist(&self.library[*i].artist).to_lowercase() == *artist;
                let genres: HashSet<String> = (0..self.library.len())
                    .filter(by)
                    .map(|i| self.library[i].genre.trim().to_lowercase())
                    .filter(|g| !g.is_empty())
                    .collect();
                (0..self.library.len())
                    .filter(|i| {
                        let t = &self.library[*i];
                        !is_browsed(t) && (by(i) || genres.contains(&t.genre.trim().to_lowercase()))
                    })
                    .collect()
            }
        }
    }

    /// What the heading over the greyed picks says.
    pub(super) fn keep_going_heading(&self) -> String {
        format!(
            "KEEP GOING · {}",
            self.keep_going.pool.name().to_uppercase()
        )
    }

    /// Keeps the switch, the pool and the offered list in step.
    fn show_keep_going(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let ui = app.global::<KeepGoingUi>();
        let k = &self.keep_going;
        if ui.get_on() != k.on {
            ui.set_on(k.on);
        }
        if ui.get_player() != k.player {
            ui.set_player(k.player);
        }
        let pool = k.pool.name();
        if ui.get_pool() != pool {
            ui.set_pool(pool.into());
        }
        let list = self.shown_list().map(|(_, n)| n).unwrap_or_default();
        if ui.get_list_name() != list.as_str() {
            ui.set_list_name(SharedString::from(list));
        }
    }
}
