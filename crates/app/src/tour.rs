//! A first mix, one step at a time: each step names what to do, rings the control for it and moves on once it is done.

use crate::state::{State, Target};

pub struct Step {
    pub title: &'static str,
    /// Two lines under the title.
    pub body: [&'static str; 2],
    /// The control to ring, where the window shows it.
    pub mark: Option<Target>,
    done: fn(&State) -> bool,
}

/// A fader this low is down, a knob this low is off.
const DOWN: f32 = 0.08;

pub const STEPS: [Step; 12] = [
    Step {
        title: "Put a track on deck A",
        body: [
            "Pick a track in the list, then press DECK A under the list.",
            "A double tap on the track does the same.",
        ],
        mark: Some(Target::LoadSelected(0)),
        done: |s| s.decks[0].loaded.is_some(),
    },
    Step {
        title: "Play it",
        body: ["Press play on deck A.", "Its fader is up, so this is what the room hears."],
        mark: Some(Target::Play(0)),
        done: |s| s.telemetry.decks[0].playing,
    },
    Step {
        title: "Put the next track on deck B",
        body: ["Pick another track and press DECK B.", "Nothing is heard of it until it plays."],
        mark: Some(Target::LoadSelected(1)),
        done: |s| s.decks[1].loaded.is_some(),
    },
    Step {
        title: "Keep deck B out of the room",
        body: ["Pull deck B's fader all the way down.", "It can play now without anyone hearing it."],
        mark: Some(Target::Volume(1)),
        done: |s| s.decks[1].volume < DOWN,
    },
    Step {
        title: "Hear deck B yourself",
        body: ["Press the headphones button under deck B's fader.", "Only your headphones get what it sends."],
        mark: Some(Target::Pfl(1)),
        done: |s| s.decks[1].pfl,
    },
    Step {
        title: "Match the tempo",
        body: ["Press SYNC on deck B.", "It takes deck A's tempo and lines its beats up with it."],
        mark: Some(Target::Sync(1)),
        done: |s| s.telemetry.decks[1].sync_lock,
    },
    Step {
        title: "Take deck B's bass away",
        body: ["Turn deck B's LOW knob all the way left.", "Two bass lines at once sound muddy."],
        mark: Some(Target::Knob(1, 2)),
        done: |s| s.decks[1].knobs[2] < DOWN,
    },
    Step {
        title: "Start deck B",
        body: ["Press play on deck B.", "Listen in the headphones: the two should run together."],
        mark: Some(Target::Play(1)),
        done: |s| s.telemetry.decks[1].playing,
    },
    Step {
        title: "Bring deck B in",
        body: ["Push deck B's fader up, slowly.", "The room hears both tracks now, deck B without its bass."],
        mark: Some(Target::Volume(1)),
        done: |s| s.decks[1].volume > 0.7,
    },
    Step {
        title: "Swap the bass",
        body: ["Turn deck A's LOW all the way left,", "then deck B's LOW back up to the middle."],
        mark: Some(Target::Knob(0, 2)),
        done: |s| s.decks[0].knobs[2] < DOWN && s.decks[1].knobs[2] > 0.4,
    },
    Step {
        title: "Take deck A out",
        body: ["Pull deck A's fader down,", "or slide the crossfader all the way to deck B."],
        mark: Some(Target::Volume(0)),
        done: |s| s.decks[0].volume < DOWN || s.crossfader > 0.95,
    },
    Step {
        title: "That was a mix",
        body: ["One track gave way to the next, in time.", "Deck A is free for the track after this one."],
        mark: None,
        done: |_| false,
    },
];

/// The step after `at` that is still to do; the last step is never done, so the tour ends on it.
pub fn advance(s: &State, at: usize) -> usize {
    let at = at.min(STEPS.len() - 1);
    if (STEPS[at].done)(s) { at + 1 } else { at }
}
