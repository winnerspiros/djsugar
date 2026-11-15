//! The order of the sources in the library rail, which the DJ drags in the
//! rail's "•••" menu (library.slint, `RailOrder`).
//!
//! Collection stays first, with Collection Health under it. The others move
//! within their section: DJ Sugar's own (Built-in), then the services,
//! servers and add-ons (External). Kept in settings.json under
//! `rail_order`, by name, so add-ons can come and go.

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, VecModel};

use crate::{AppWindow, RailOrder};

/// DJ Sugar's own sources after Collection, in their first order.
const BUILT_IN: [&str; 4] = ["Files", "Albums", "Recordings", "Jam"];

/// The services in their first order; add-ons' sources follow.
const EXTERNAL: [&str; 5] = ["Spotify", "TIDAL", "Apple Music", "SoundCloud", "Mixcloud"];

thread_local! {
    /// Each section's sources, by name, in the DJ's order.
    static ORDER: RefCell<(Vec<String>, Vec<String>)> = RefCell::default();
}

/// The two sections in the saved order; sources the save doesn't name (new,
/// or an add-on just installed) go at the end of their section, and ones
/// no longer there drop out.
fn sections(saved: &[String], addons: &[String]) -> (Vec<String>, Vec<String>) {
    let pick = |all: Vec<String>| {
        let mut out: Vec<String> = saved.iter().filter(|n| all.contains(n)).cloned().collect();
        out.dedup();
        for n in all {
            if !out.contains(&n) {
                out.push(n);
            }
        }
        out
    };
    let built_in = pick(BUILT_IN.iter().map(|n| n.to_string()).collect());
    let external = pick(
        EXTERNAL
            .iter()
            .map(|n| n.to_string())
            .chain(addons.iter().cloned())
            .collect(),
    );
    (built_in, external)
}

/// Moves source `name` `steps` places along its section.
fn moved(order: &mut (Vec<String>, Vec<String>), name: &str, steps: i32) {
    for list in [&mut order.0, &mut order.1] {
        if let Some(from) = list.iter().position(|n| n == name) {
            let to = (from as i32 + steps).clamp(0, list.len() as i32 - 1) as usize;
            let item = list.remove(from);
            list.insert(to, item);
            return;
        }
    }
}

fn publish(app: &AppWindow) {
    let names = super::all_source_names();
    let index = |list: &[String]| -> Vec<i32> {
        list.iter()
            .filter_map(|n| names.iter().position(|m| m == n).map(|i| i as i32))
            .collect()
    };
    let (built_in, external) = ORDER.with(|o| {
        let o = o.borrow();
        (index(&o.0), index(&o.1))
    });
    let ui = app.global::<RailOrder>();
    ui.set_built_in(Rc::new(VecModel::from(built_in)).into());
    ui.set_external(Rc::new(VecModel::from(external)).into());
}

/// Loads the saved order and answers the menu's drags.
pub(super) fn install(app: &AppWindow) {
    let addons: Vec<String> = super::all_source_names()
        .into_iter()
        .skip(super::SOURCE_NAMES.len())
        .collect();
    let saved = crate::settings::rail_order().unwrap_or_default();
    ORDER.with(|o| *o.borrow_mut() = sections(&saved, &addons));
    publish(app);
    let weak = app.as_weak();
    app.global::<RailOrder>().on_moved(move |source, steps| {
        let Some(app) = weak.upgrade() else { return };
        let Some(name) = super::all_source_names()
            .get(source.max(0) as usize)
            .cloned()
        else {
            return;
        };
        let all = ORDER.with(|o| {
            let mut o = o.borrow_mut();
            moved(&mut o, &name, steps);
            o.0.iter().chain(o.1.iter()).cloned().collect::<Vec<_>>()
        });
        crate::settings::set_rail_order(&all);
        publish(&app);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn first_order_without_a_save() {
        let (b, e) = sections(&[], &s(&["Plex"]));
        assert_eq!(b, s(&BUILT_IN));
        assert_eq!(e.last().map(String::as_str), Some("Plex"));
    }

    #[test]
    fn saved_order_kept_and_new_sources_added_at_the_end() {
        let saved = s(&["Jam", "Plex", "Files", "TIDAL", "Gone"]);
        let (b, e) = sections(&saved, &s(&["Plex", "Soulseek"]));
        assert_eq!(b, s(&["Jam", "Files", "Albums", "Recordings"]));
        assert_eq!(&e[..2], &s(&["Plex", "TIDAL"])[..]);
        assert_eq!(e.last().map(String::as_str), Some("Soulseek"));
        assert!(!e.contains(&"Gone".to_string()));
    }

    #[test]
    fn moves_stay_in_their_section() {
        let mut o = sections(&[], &[]);
        moved(&mut o, "Jam", -10);
        assert_eq!(o.0[0], "Jam");
        moved(&mut o, "Spotify", 2);
        assert_eq!(
            o.1,
            s(&["TIDAL", "Apple Music", "Spotify", "SoundCloud", "Mixcloud"])
        );
        assert_eq!(o.0.len(), 4);
    }
}
