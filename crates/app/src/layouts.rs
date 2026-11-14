//! The DJ's layouts: which of openDJ's layouts the layout menu at the top
//! left offers, in their order, and the menu's chooser (its pencil) where
//! each is switched on or off and dragged into place. A controller's VIEW
//! (`[Skin],layout_next`) steps through the ones switched on.
//!
//! settings.json keeps the layouts switched on, in menu order, under
//! `layouts` (as before the chooser), and every layout's place under
//! `layouts_order`, so one switched off keeps its row.

use slint::{ComponentHandle, ModelRc, VecModel};

use crate::{settings, AppWindow, LayoutEntry, LayoutsUi};

/// A layout in the chooser: its number and whether it is switched on.
type Row = (i32, bool);

/// The layouts switched on, as layout numbers in menu order.
const KEY: &str = "layouts";
/// Every layout's place in the chooser, on or off.
const ORDER_KEY: &str = "layouts_order";

/// Every layout, in the chooser's starting order: its number (the value of
/// `layout`, which never changes so saved settings keep working), its name
/// and what it's for. Layout 0, the full decks, was "2-Deck Pro" and 4 was
/// "2-Deck" until 0.7.
const ALL: [(i32, &str, &str); 5] = [
    (
        1,
        "Player",
        "A music player that mixes for you, with one bar along the bottom.",
    ),
    (
        0,
        "2-Deck",
        "Two full decks with jogs, tempo faders and pads.",
    ),
    (2, "4-Deck", "Four deck strips around the mixer."),
    (
        3,
        "Prep",
        "Deck 1 alone, for getting tracks ready: grid, cues and a checklist.",
    ),
    (
        4,
        "2-Deck Basic",
        "Two light decks without jogs, with pads and loops in a bar under them.",
    ),
];

/// The layouts switched on at first, in order: Player, 2-Deck, 4-Deck and
/// Prep. 2-Deck Basic starts off.
const START: [i32; 4] = [1, 0, 2, 3];

/// Whether layout `l` exists and is offered (Prep and 2-Deck Basic only
/// while they are).
fn offered(l: i32) -> bool {
    ALL.iter().any(|&(n, _, _)| n == l)
        && (l != crate::PREP_LAYOUT || crate::PREP_OFFERED)
        && (l != crate::LITE_LAYOUT || crate::LITE_OFFERED)
}

/// Every offered layout once, each with its switch: the saved order first
/// (`order`, or failing that the saved `on` list), then the ones it
/// doesn't know in ALL's order. Nothing usable saved gives the start.
fn clean(
    on: Option<Vec<i32>>,
    order: Option<Vec<i32>>,
    offered: impl Fn(i32) -> bool,
) -> Vec<(i32, bool)> {
    let on: Vec<i32> = on
        .unwrap_or_default()
        .into_iter()
        .filter(|&l| offered(l))
        .collect();
    let on = if on.is_empty() {
        START.into_iter().filter(|&l| offered(l)).collect()
    } else {
        on
    };
    let mut list: Vec<(i32, bool)> = Vec::new();
    let saved_order = order.unwrap_or_else(|| on.clone());
    for l in saved_order
        .into_iter()
        .chain(ALL.iter().map(|&(n, _, _)| n))
    {
        if offered(l) && !list.iter().any(|&(x, _)| x == l) {
            list.push((l, on.contains(&l)));
        }
    }
    list
}

fn load() -> Vec<(i32, bool)> {
    let s = settings::load();
    let ints = |key: &str| {
        s[key].as_array().map(|a| {
            a.iter()
                .filter_map(|v| v.as_i64())
                .map(|v| v as i32)
                .collect()
        })
    };
    clean(ints(KEY), ints(ORDER_KEY), offered)
}

/// The layouts switched on, in `list`'s order.
fn on_of(list: &[(i32, bool)]) -> Vec<i32> {
    list.iter().filter(|t| t.1).map(|t| t.0).collect()
}

/// The DJ's layouts, in menu order.
pub fn chosen() -> Vec<i32> {
    on_of(&load())
}

fn save(list: &[(i32, bool)]) {
    settings::set(KEY, serde_json::json!(on_of(list)));
    let order: Vec<i32> = list.iter().map(|t| t.0).collect();
    settings::set(ORDER_KEY, serde_json::json!(order));
}

/// The layout after `current` in `list`, back to the first after the last
/// (or the first when `current` isn't in it).
fn next_in(list: &[i32], current: i32) -> i32 {
    match list.iter().position(|&l| l == current) {
        Some(i) => list[(i + 1) % list.len()],
        None => list.first().copied().unwrap_or(current),
    }
}

/// The layout VIEW goes to from `current`.
pub fn next(current: i32) -> i32 {
    next_in(&chosen(), current)
}

/// Whether `l`'s switch can't be turned off: it is the one in use, or the
/// last one on.
fn locked(list: &[(i32, bool)], l: i32, current: i32) -> bool {
    let on = list.iter().filter(|t| t.1).count();
    list.iter()
        .any(|&(x, o)| x == l && o && (x == current || on <= 1))
}

/// `list` with `l` switched off, unless it is locked on.
fn switched_off(list: &[(i32, bool)], l: i32, current: i32) -> Vec<(i32, bool)> {
    if locked(list, l, current) {
        return list.to_vec();
    }
    list.iter().map(|&(x, o)| (x, o && x != l)).collect()
}

/// `list` with `l` switched on, where it is.
fn switched_on(list: &[(i32, bool)], l: i32) -> Vec<(i32, bool)> {
    list.iter().map(|&(x, o)| (x, o || x == l)).collect()
}

/// `list` with `l` moved `by` rows, kept in bounds.
fn moved(list: &[(i32, bool)], l: i32, by: i32) -> Vec<(i32, bool)> {
    let mut list = list.to_vec();
    if let Some(i) = list.iter().position(|&(x, _)| x == l) {
        let j = (i as i32 + by).clamp(0, list.len() as i32 - 1) as usize;
        let x = list.remove(i);
        list.insert(j, x);
    }
    list
}

/// The starting list, for Reset to default; the layout in use stays on.
fn reset(current: i32) -> Vec<(i32, bool)> {
    let list = clean(None, None, offered);
    switched_on(&list, current)
}

fn entry(l: i32, on: bool, locked: bool) -> LayoutEntry {
    let (_, name, detail) = ALL
        .iter()
        .find(|&&(n, _, _)| n == l)
        .copied()
        .unwrap_or(ALL[1]);
    LayoutEntry {
        layout: l,
        name: name.into(),
        detail: detail.into(),
        on,
        locked,
    }
}

fn model(list: impl IntoIterator<Item = LayoutEntry>) -> ModelRc<LayoutEntry> {
    ModelRc::new(VecModel::from(list.into_iter().collect::<Vec<_>>()))
}

/// Fills the layout menu and its chooser from the saved
/// list and the layout showing. Call it whenever either changes.
pub fn refresh(app: &AppWindow) {
    let current = app.get_layout();
    let list = load();
    let ui = app.global::<LayoutsUi>();
    ui.set_current(current);
    // The menu also shows the layout in use when it isn't one of the DJ's
    // (the tour, or a mapping that sets a layout by number), at the end.
    let mut shown = on_of(&list);
    if !shown.contains(&current) && offered(current) {
        shown.push(current);
    }
    ui.set_menu(model(shown.into_iter().map(|l| entry(l, true, false))));
    ui.set_all(model(
        list.iter()
            .map(|&(l, on)| entry(l, on, locked(&list, l, current))),
    ));
}

/// Wires the menu's chooser and its Dark / Light / Auto row. `switch`
/// changes the layout as the menu does.
pub fn install(app: &AppWindow, switch: impl Fn(i32) + 'static) {
    let ui = app.global::<LayoutsUi>();
    let weak = app.as_weak();
    ui.on_pick(move |l| {
        switch(l);
        if let Some(app) = weak.upgrade() {
            refresh(&app);
        }
    });
    let weak = app.as_weak();
    let change = move |f: &dyn Fn(&[Row], i32) -> Vec<Row>| {
        if let Some(app) = weak.upgrade() {
            save(&f(&load(), app.get_layout()));
            refresh(&app);
        }
    };
    let change = std::rc::Rc::new(change);
    let c = change.clone();
    ui.on_toggled(move |l| {
        c(&|list, current| {
            if list.iter().any(|&(x, on)| x == l && on) {
                switched_off(list, l, current)
            } else {
                switched_on(list, l)
            }
        })
    });
    let c = change.clone();
    ui.on_moved(move |l, by| c(&|list, _| moved(list, l, by)));
    let c = change.clone();
    ui.on_reset(move || c(&|_, current| reset(current)));
    let weak = app.as_weak();
    ui.on_appearance_picked(move |m| {
        if let Some(app) = weak.upgrade() {
            crate::appearance::choose(&app, m.max(0) as usize);
        }
    });
    refresh(app);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(_: i32) -> bool {
        true
    }

    fn ons(list: &[(i32, bool)]) -> Vec<i32> {
        on_of(list)
    }

    #[test]
    fn starts_with_player_2deck_4deck_prep() {
        let list = clean(None, None, all);
        assert_eq!(ons(&list), vec![1, 0, 2, 3]);
        assert_eq!(list.last(), Some(&(4, false)));
        assert_eq!(ons(&clean(Some(vec![]), None, all)), vec![1, 0, 2, 3]);
    }

    #[test]
    fn hides_2deck_basic_while_not_offered() {
        // A DJ who had 2-Deck Basic switched on gets the rest, without it.
        let list = clean(Some(vec![1, 4, 0]), Some(vec![1, 4, 0, 2, 3]), offered);
        assert_eq!(offered(4), crate::LITE_OFFERED);
        if !crate::LITE_OFFERED {
            assert_eq!(ons(&list), vec![1, 0]);
            assert!(!list.iter().any(|t| t.0 == 4));
        }
    }

    #[test]
    fn keeps_a_list_saved_before_the_chooser() {
        // 0.6 saved only the layouts in the menu.
        let list = clean(Some(vec![1, 4, 0, 2]), None, all);
        assert_eq!(
            list,
            vec![(1, true), (4, true), (0, true), (2, true), (3, false)]
        );
    }

    #[test]
    fn keeps_the_places_of_ones_switched_off() {
        let list = clean(Some(vec![2, 1]), Some(vec![4, 2, 0, 1, 3]), all);
        assert_eq!(
            list,
            vec![(4, false), (2, true), (0, false), (1, true), (3, false)]
        );
        assert_eq!(ons(&list), vec![2, 1]);
    }

    #[test]
    fn drops_unknown_hidden_and_repeated() {
        let offered = |l: i32| (0..=4).contains(&l) && l != 3;
        let list = clean(Some(vec![2, 9, 2, 3, 1]), None, offered);
        assert_eq!(ons(&list), vec![2, 1]);
        assert!(!list.iter().any(|t| t.0 == 3 || t.0 == 9));
        assert_eq!(list.len(), 4);
    }

    #[test]
    fn view_steps_round() {
        assert_eq!(next_in(&[1, 4], 4), 1);
        assert_eq!(next_in(&[1, 4], 1), 4);
        assert_eq!(next_in(&[1, 4, 2], 0), 1);
        assert_eq!(next_in(&[4], 4), 4);
    }

    #[test]
    fn the_layout_in_use_and_the_last_one_stay_on() {
        let list = vec![(1, true), (4, true), (0, false)];
        assert_eq!(ons(&switched_off(&list, 4, 4)), vec![1, 4]);
        assert_eq!(ons(&switched_off(&list, 1, 4)), vec![4]);
        let one = vec![(4, true), (1, false)];
        assert_eq!(ons(&switched_off(&one, 4, 1)), vec![4]);
        assert!(locked(&one, 4, 1));
        assert!(!locked(&one, 1, 1));
    }

    #[test]
    fn switching_on_keeps_the_row() {
        let list = vec![(1, true), (4, false), (0, true)];
        assert_eq!(switched_on(&list, 4), vec![(1, true), (4, true), (0, true)]);
    }

    #[test]
    fn moves_within_bounds() {
        let list = vec![(1, true), (4, false), (0, true)];
        assert_eq!(moved(&list, 0, -1), vec![(1, true), (0, true), (4, false)]);
        assert_eq!(moved(&list, 1, -1), list);
        assert_eq!(moved(&list, 1, 5), vec![(4, false), (0, true), (1, true)]);
    }

    #[test]
    fn reset_keeps_the_layout_in_use() {
        if crate::LITE_OFFERED {
            assert!(reset(4).contains(&(4, true)));
            assert!(reset(1).contains(&(4, false)));
        } else {
            // 2-Deck Basic, hidden, isn't in the chooser at all.
            assert!(reset(0).contains(&(0, true)));
            assert!(!reset(1).iter().any(|t| t.0 == 4));
        }
    }
}
