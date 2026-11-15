//! Prep's metronome: it turns on by itself when the Prep deck has a track
//! with an unlocked grid, so the grid can be checked by ear while it is
//! fixed, and back off once the grid is locked or Prep is left, if it was
//! the one that turned it on. Turned off by hand, it stays off for that
//! track.

use djsugar_control::{ControlHandle, ControlRegistry};

pub struct PrepClick {
    metronome: ControlHandle,
    bpmlock: ControlHandle,
    loaded: ControlHandle,
    /// Prep showed an unlocked grid on the last tick.
    open: bool,
    /// The click is on because of Prep.
    ours: bool,
}

impl PrepClick {
    /// `group`: the Prep deck's, `[Channel1]`.
    pub fn new(reg: &ControlRegistry, group: &str) -> Self {
        let c = |k: &str| {
            reg.get(group, k)
                .unwrap_or_else(|| panic!("missing control {group},{k}"))
        };
        Self {
            metronome: c("metronome"),
            bpmlock: c("bpmlock"),
            loaded: c("track_loaded"),
            open: false,
            ours: false,
        }
    }

    /// `prep`: the Prep layout is showing.
    pub fn tick(&mut self, prep: bool) {
        let open = prep && self.loaded.get_bool() && !self.bpmlock.get_bool();
        match (self.open, open) {
            (false, true) if !self.metronome.get_bool() => {
                self.metronome.set_bool(true);
                self.ours = true;
            }
            (true, false) => {
                if self.ours {
                    self.metronome.set_bool(false);
                }
                self.ours = false;
            }
            _ => {}
        }
        // Turned off by hand: no longer Prep's to turn off.
        if self.ours && !self.metronome.get_bool() {
            self.ours = false;
        }
        self.open = open;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (std::sync::Arc<ControlRegistry>, PrepClick) {
        let reg = ControlRegistry::new();
        for k in ["metronome", "bpmlock", "track_loaded"] {
            reg.toggle("[Channel1]", k);
        }
        let click = PrepClick::new(&reg, "[Channel1]");
        (reg, click)
    }

    fn on(reg: &ControlRegistry, k: &str) -> bool {
        reg.get("[Channel1]", k).unwrap().get_bool()
    }

    fn set(reg: &ControlRegistry, k: &str, v: bool) {
        reg.get("[Channel1]", k).unwrap().set_bool(v);
    }

    #[test]
    fn clicks_while_the_grid_is_open_in_prep() {
        let (reg, mut click) = setup();
        set(&reg, "track_loaded", true);
        click.tick(false);
        assert!(!on(&reg, "metronome"), "not outside Prep");
        click.tick(true);
        assert!(on(&reg, "metronome"));
        set(&reg, "bpmlock", true);
        click.tick(true);
        assert!(!on(&reg, "metronome"), "off once locked");
    }

    #[test]
    fn leaving_prep_turns_only_its_own_click_off() {
        let (reg, mut click) = setup();
        set(&reg, "track_loaded", true);
        set(&reg, "metronome", true);
        click.tick(true);
        click.tick(false);
        assert!(on(&reg, "metronome"), "the DJ's own click stays");
    }

    #[test]
    fn turned_off_by_hand_stays_off() {
        let (reg, mut click) = setup();
        set(&reg, "track_loaded", true);
        click.tick(true);
        set(&reg, "metronome", false);
        click.tick(true);
        click.tick(true);
        assert!(!on(&reg, "metronome"));
    }
}
