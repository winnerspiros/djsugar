//! What to do when a plugin misbehaves.
//!
//! The engine already plays a bad block dry; this decides what happens to
//! the plugin afterwards: keep it, restart its helper, or switch it off until
//! the DJ turns it back on. A restarted plugin comes back bypassed, never on
//! by itself in the middle of a set.

/// A verdict for one plugin instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Carry on.
    Keep,
    /// Kill the helper if it still runs, start a new one, leave the slot off.
    Restart,
    /// Stop it and keep it off until the DJ enables it in Settings.
    Disable,
}

/// Limits, with the defaults from `notes/plugin-hosting-review.md`.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Crashes (or hangs) in one session before the plugin is disabled.
    pub max_crashes: u32,
    /// Blocks in a row a helper may miss before it counts as hung.
    pub max_missed: u32,
    /// Share of the block time the plugin may use before it counts as too
    /// slow (1.0 = all of it).
    pub cpu_budget: f32,
    /// Blocks in a row over budget before it is switched off.
    pub max_over_budget: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_crashes: 2,
            max_missed: 8,
            cpu_budget: 0.3,
            max_over_budget: 200,
        }
    }
}

/// Keeps count for one plugin across a session.
#[derive(Clone, Debug, Default)]
pub struct Policy {
    pub limits: Limits,
    crashes: u32,
    over_budget: u32,
}

impl Policy {
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            ..Self::default()
        }
    }

    /// Crashes and hangs so far.
    pub fn crashes(&self) -> u32 {
        self.crashes
    }

    /// The helper ended without being asked to.
    pub fn on_crash(&mut self) -> Action {
        self.crashes += 1;
        if self.crashes >= self.limits.max_crashes {
            Action::Disable
        } else {
            Action::Restart
        }
    }

    /// Called with [`crate::Connection::missed_in_row`]; too many is a hang,
    /// handled like a crash.
    pub fn on_missed(&mut self, missed_in_row: u32) -> Action {
        if missed_in_row >= self.limits.max_missed {
            self.on_crash()
        } else {
            Action::Keep
        }
    }

    /// Called with [`crate::Helper::load`] after each block.
    pub fn on_load(&mut self, load: f32) -> Action {
        if load > self.limits.cpu_budget {
            self.over_budget += 1;
            if self.over_budget >= self.limits.max_over_budget {
                self.over_budget = 0;
                return Action::Disable;
            }
        } else {
            self.over_budget = 0;
        }
        Action::Keep
    }

    /// The DJ turned it back on: start counting again.
    pub fn reset(&mut self) {
        self.crashes = 0;
        self.over_budget = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restarts_once_then_disables() {
        let mut p = Policy::default();
        assert_eq!(p.on_crash(), Action::Restart);
        assert_eq!(p.on_crash(), Action::Disable);
        p.reset();
        assert_eq!(p.on_crash(), Action::Restart);
    }

    #[test]
    fn a_hang_counts_as_a_crash() {
        let mut p = Policy::default();
        assert_eq!(p.on_missed(3), Action::Keep);
        assert_eq!(p.on_missed(8), Action::Restart);
        assert_eq!(p.crashes(), 1);
    }

    #[test]
    fn only_a_long_run_over_budget_disables() {
        let mut p = Policy::new(Limits {
            max_over_budget: 3,
            ..Limits::default()
        });
        assert_eq!(p.on_load(0.9), Action::Keep);
        assert_eq!(p.on_load(0.9), Action::Keep);
        assert_eq!(p.on_load(0.1), Action::Keep);
        assert_eq!(p.on_load(0.9), Action::Keep);
        assert_eq!(p.on_load(0.9), Action::Keep);
        assert_eq!(p.on_load(0.9), Action::Disable);
    }
}
