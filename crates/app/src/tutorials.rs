//! The advanced tutorials in Help › Tutorials: hands-on lessons for looping,
//! effects, filtering the list, Automix and the DDJ-FLX4, on the guided
//! tour's engine ([`crate::tour`]). Each points at the real control, waits
//! until the DJ has used it (on screen or on a controller) and ends with a
//! link to its page in the user guide ([`finish`]). Finishing one marks it
//! done.

use serde_json::json;
use slint::{ComponentHandle, Model};

use crate::settings;
use crate::tour::{Action, Lesson, Step, Wait};
use crate::{AppWindow, FilterUi, StoreUi};

/// A tutorial, as Help › Tutorials lists it.
#[derive(Debug)]
pub struct Tutorial {
    /// Its id in settings.
    pub id: &'static str,
    pub name: &'static str,
    /// About how long it takes, shown beside it in Help › Tutorials.
    pub minutes: u32,
    /// A controller it needs plugged in, by a word of its port name.
    pub controller: Option<&'static str>,
    pub lesson: &'static Lesson,
}

pub const TUTORIALS: [Tutorial; 5] = [
    Tutorial {
        id: "looping",
        name: "Looping",
        minutes: 3,
        controller: None,
        lesson: &LOOPING,
    },
    Tutorial {
        id: "fx",
        name: "Using FX",
        minutes: 3,
        controller: None,
        lesson: &USING_FX,
    },
    Tutorial {
        id: "filtering",
        name: "Filtering tracks",
        minutes: 3,
        controller: None,
        lesson: &FILTERING,
    },
    Tutorial {
        id: "automix",
        name: "Automix",
        minutes: 3,
        controller: None,
        lesson: &AUTOMIX,
    },
    Tutorial {
        id: "ddj-flx4",
        name: "DDJ-FLX4",
        minutes: 5,
        controller: Some("DDJ-FLX4"),
        lesson: &DDJ_FLX4,
    },
];

pub fn find(id: &str) -> Option<&'static Tutorial> {
    TUTORIALS.iter().find(|t| t.id == id)
}

fn done_key(id: &str) -> String {
    format!("tutorial_done_{id}")
}

/// Whether the DJ has finished the tutorial.
pub fn done(id: &str) -> bool {
    settings::flag(&done_key(id), false)
}

pub fn mark_done(id: &str) {
    settings::set(&done_key(id), json!(true));
}

const EQ_A: &str = "[EqualizerRack1_[Channel1]_Effect1]";
const EQ_B: &str = "[EqualizerRack1_[Channel2]_Effect1]";
const CFX_A: &str = "[QuickEffectRack1_[Channel1]]";
const FX_A2: &str = "[EffectRack1_EffectUnit1_Effect2]";
const BEAT_FX: &str = "[EffectRack1_EffectUnit5_Effect1]";

/// Loads a track on deck A, or a demo track, then plays it: the start of
/// most tutorials.
const LOAD_A: Step = Step {
    target: "[Channel1],load",
    title: "Have a track on deck A",
    body: "Drag a track from the browser onto deck A, or load a demo track. If deck A \
           already has one, this step is done.",
    wait: Wait::HasTrack(0),
    action: Some(("Load demo track", Action::LoadDemo { deck: 0, demo: 0 })),
};

const PLAY_A: Step = Step {
    target: "[Channel1],play",
    title: "Press PLAY",
    body: "Deck A plays on the speakers.",
    wait: Wait::Above("[Channel1]", "play", 0.5),
    action: None,
};

/// The last tip: what was learnt, and the guide.
const fn finish(title: &'static str, body: &'static str, guide: &'static str) -> Step {
    Step {
        target: "",
        title,
        body,
        wait: Wait::Next,
        action: Some(("Open the guide", Action::Open(guide))),
    }
}

/// Set on a pad: a saved loop is hot cue type 4.
const SAVED_LOOP: Wait = Wait::Any(&[
    Wait::Above("[Channel1]", "hotcue_1_type", 3.5),
    Wait::Above("[Channel1]", "hotcue_2_type", 3.5),
    Wait::Above("[Channel1]", "hotcue_3_type", 3.5),
    Wait::Above("[Channel1]", "hotcue_4_type", 3.5),
    Wait::Above("[Channel1]", "hotcue_5_type", 3.5),
    Wait::Above("[Channel1]", "hotcue_6_type", 3.5),
    Wait::Above("[Channel1]", "hotcue_7_type", 3.5),
    Wait::Above("[Channel1]", "hotcue_8_type", 3.5),
]);

/// Any loop roll held, by the sizes the LOOP ROLL pads have.
const ROLLING: Wait = Wait::Any(&[
    Wait::Above("[Channel1]", "beatlooproll_0.0625_activate", 0.5),
    Wait::Above("[Channel1]", "beatlooproll_0.125_activate", 0.5),
    Wait::Above("[Channel1]", "beatlooproll_0.25_activate", 0.5),
    Wait::Above("[Channel1]", "beatlooproll_0.5_activate", 0.5),
    Wait::Above("[Channel1]", "beatlooproll_1_activate", 0.5),
    Wait::Above("[Channel1]", "beatlooproll_2_activate", 0.5),
    Wait::Above("[Channel1]", "beatlooproll_4_activate", 0.5),
    Wait::Above("[Channel1]", "beatlooproll_8_activate", 0.5),
]);

pub const LOOPING: Lesson = Lesson {
    layout: Some(0),
    prepare: &[
        ("[Channel1]", "play", 0.0),
        ("[Channel2]", "play", 0.0),
        ("[Channel1]", "volume", 1.0),
        ("[Channel1]", "loop_enabled", 0.0),
        ("[Channel1]", "beatloop_size", 4.0),
    ],
    keep: &[
        ("[Channel1]", "pad_slot_1"),
        ("[Channel1]", "pad_slot_2"),
        ("[Channel1]", "pad_slot_3"),
        ("[Channel1]", "pad_slot_4"),
        ("[Channel1]", "pad_slot_5"),
        ("[Channel1]", "pad_slot_6"),
        ("[Channel1]", "pad_slot_7"),
        ("[Channel1]", "pad_slot_8"),
    ],
    steps: &[
        LOAD_A,
        PLAY_A,
        Step {
            target: "[Channel1],beatloop_activate",
            title: "Loop 4 beats",
            body: "Press the 4 in the LOOP box. Deck A repeats the 4 beats from where it is, \
                   in time, until you let go.",
            wait: Wait::Above("[Channel1]", "loop_enabled", 0.5),
            action: None,
        },
        Step {
            target: "[Channel1],loop_halve",
            title: "Halve it",
            body: "Press ‹ to halve the loop to 2 beats. Halving again and again makes a \
                   build-up.",
            wait: Wait::Below("[Channel1]", "beatloop_size", 3.0),
            action: None,
        },
        Step {
            target: "[Channel1],loop_double",
            title: "Double it back",
            body: "Press › to double it back to 4 beats.",
            wait: Wait::Above("[Channel1]", "beatloop_size", 3.0),
            action: None,
        },
        Step {
            target: "[Channel1],beatloop_activate",
            title: "Let go",
            body: "Press the loop length again to leave the loop. The track plays on from \
                   there.",
            wait: Wait::Below("[Channel1]", "loop_enabled", 0.5),
            action: None,
        },
        Step {
            target: "[Channel1],loop_in",
            title: "Make your own loop",
            body: "Press IN where the loop should start, then OUT where it should end. Any \
                   length works; with quantize on, both land on a beat.",
            wait: Wait::Above("[Channel1]", "loop_enabled", 0.5),
            action: None,
        },
        Step {
            target: "[Channel1],pads",
            title: "Save it on a pad",
            body: "While it loops, press an empty pad on the HOT CUES page. The pad keeps the \
                   loop and shows a loop sign instead of a play mark.",
            wait: SAVED_LOOP,
            action: Some(("Show hot cues", Action::Set("[Channel1]", "pad_panel", 0.0))),
        },
        Step {
            target: "[Channel1],beatloop_activate",
            title: "Leave the loop",
            body: "Press the loop length to let go of it again.",
            wait: Wait::Below("[Channel1]", "loop_enabled", 0.5),
            action: None,
        },
        Step {
            target: "[Channel1],pads",
            title: "Jump back into it",
            body: "Press the loop's pad. Deck A jumps to the loop and plays it; press the pad \
                   once more to leave it.",
            wait: Wait::Above("[Channel1]", "loop_enabled", 0.5),
            action: None,
        },
        Step {
            target: "[Channel1],pads",
            title: "Hold a loop roll",
            body: "On the LOOP ROLL page, hold a pad. It loops while you hold it; let go and \
                   the track carries on where it would have been, so you stay in time.",
            wait: ROLLING,
            action: Some((
                "Show loop rolls",
                Action::Set("[Channel1]", "pad_panel", 9.0),
            )),
        },
        finish(
            "That's looping",
            "Loops by length, halved and doubled, your own with IN and OUT, kept on a pad, \
             and rolls. The guide has the rest, such as loops on a controller.",
            "https://github.com/winnerspiros/djsugar/blob/rust/docs/guide/looping.md",
        ),
    ],
};

fn fx_bar_shown(app: &AppWindow) -> bool {
    app.get_fx_bar_visible()
}

pub const USING_FX: Lesson = Lesson {
    layout: Some(0),
    prepare: &[
        ("[Channel1]", "play", 0.0),
        ("[Channel2]", "play", 0.0),
        ("[Channel1]", "volume", 1.0),
        ("[EffectRack1_EffectUnit1_Effect1]", "enabled", 0.0),
        (FX_A2, "enabled", 0.0),
        ("[EffectRack1_EffectUnit1_Effect3]", "enabled", 0.0),
        (FX_A2, "meta", 0.5),
        (BEAT_FX, "enabled", 0.0),
        (CFX_A, "loaded_chain_preset", 1.0),
        (CFX_A, "super1", 0.5),
    ],
    keep: &[],
    steps: &[
        LOAD_A,
        PLAY_A,
        Step {
            target: "ui,fx_button",
            title: "Show the FX bar",
            body: "The FX button in the top bar shows the effects under the decks. Turn it on \
                   if the bar is hidden.",
            wait: Wait::App(fx_bar_shown),
            action: None,
        },
        Step {
            target: "[EffectRack1_EffectUnit1_Effect2],enabled",
            title: "Turn on an effect",
            body: "Deck A's three effects sit on its side of the FX bar. Press ON on the \
                   second one.",
            wait: Wait::Above(FX_A2, "enabled", 0.5),
            action: None,
        },
        Step {
            target: "[EffectRack1_EffectUnit1_Effect2],meta",
            title: "Turn up D/W",
            body: "D/W is how much of the effect you hear, from none (D, dry) to all of it \
                   (W, wet). Turn it right.",
            wait: Wait::Above(FX_A2, "meta", 0.75),
            action: None,
        },
        Step {
            target: "[EffectRack1_EffectUnit1_Effect2],beats",
            title: "Change its length",
            body: "Effects that follow the beat, such as echo, have a length in beats: try \
                   another with the arrows. Effects without one show a slider for their main \
                   setting instead.",
            wait: Wait::Changed(FX_A2, "beats"),
            action: None,
        },
        Step {
            target: "[EffectRack1_EffectUnit1_Effect2]",
            title: "Try another effect",
            body: "Click the effect's name for the list, or step through them with the arrows \
                   beside it.",
            wait: Wait::Changed(FX_A2, "loaded_effect"),
            action: None,
        },
        Step {
            target: "[EffectRack1_EffectUnit1_Effect2],enabled",
            title: "Turn it off",
            body: "Press ON again. The effect's tail rings out, as on a mixer.",
            wait: Wait::Below(FX_A2, "enabled", 0.5),
            action: None,
        },
        Step {
            target: "[EffectRack1_EffectUnit5]",
            title: "Beat FX",
            body: "The Beat FX in the middle plays on deck 1, deck 2 or both, as on a Pioneer \
                   mixer. Pick 1, then press its ON.",
            wait: Wait::Above(BEAT_FX, "enabled", 0.5),
            action: None,
        },
        Step {
            target: "[EffectRack1_EffectUnit5_Effect1],enabled",
            title: "Turn the Beat FX off",
            body: "Press its ON again.",
            wait: Wait::Below(BEAT_FX, "enabled", 0.5),
            action: None,
        },
        Step {
            target: "[QuickEffectRack1_[Channel1]],loaded_chain_preset",
            title: "Pick a Color FX",
            body: "The small ▾ beside deck A's filter knob picks what the knob plays. Pick \
                   Space.",
            wait: Wait::Above(CFX_A, "loaded_chain_preset", 1.5),
            action: None,
        },
        Step {
            target: "[QuickEffectRack1_[Channel1]],super1",
            title: "Turn the knob left",
            body: "The knob's name now says which Color FX it plays. Turn it left: the effect \
                   grows the further you go.",
            wait: Wait::Below(CFX_A, "super1", 0.25),
            action: None,
        },
        Step {
            target: "[QuickEffectRack1_[Channel1]],super1",
            title: "Now right",
            body: "Turn it right past the middle. Each side plays the effect its own way.",
            wait: Wait::Above(CFX_A, "super1", 0.75),
            action: None,
        },
        Step {
            target: "[QuickEffectRack1_[Channel1]],super1",
            title: "Back to the middle",
            body: "In the middle the knob does nothing. Double-click it to snap it there.",
            wait: Wait::Both(
                &Wait::Above(CFX_A, "super1", 0.45),
                &Wait::Below(CFX_A, "super1", 0.55),
            ),
            action: None,
        },
        finish(
            "That's FX",
            "Deck effects, the Beat FX and Color FX. The guide lists every effect and what \
             its settings do.",
            "https://github.com/winnerspiros/djsugar/blob/rust/docs/guide/fx.md",
        ),
    ],
};

fn searching(app: &AppWindow) -> bool {
    !app.global::<StoreUi>().get_search_text().is_empty()
}

fn not_searching(app: &AppWindow) -> bool {
    !searching(app)
}

/// Whether a chip of `kind` (see FilterChip in filters.slint) is on.
fn chip_on(app: &AppWindow, kind: fn(i32) -> bool) -> bool {
    app.global::<FilterUi>()
        .get_chips()
        .iter()
        .any(|c| c.active && kind(c.kind))
}

/// A mix under way: the status reads "Mixing into <title>", not the
/// "Mixing into <title> in 0:12" countdown before one. (The crossfader
/// moves as soon as Automix starts, so it can't tell.)
fn mixing(app: &AppWindow) -> bool {
    is_mixing(&app.get_automix_status())
}

fn is_mixing(status: &str) -> bool {
    let countdown = status.rsplit_once(" in ").is_some_and(|(_, t)| {
        t.split_once(':').is_some_and(|(m, s)| {
            !m.is_empty() && s.len() == 2 && (m.to_owned() + s).bytes().all(|b| b.is_ascii_digit())
        })
    });
    status.starts_with("Mixing into ") && !countdown
}

fn filter_bar_shown(app: &AppWindow) -> bool {
    app.global::<FilterUi>().get_shown()
}

fn bpm_match(app: &AppWindow) -> bool {
    chip_on(app, |k| k == 2)
}

fn key_match(app: &AppWindow) -> bool {
    chip_on(app, |k| k == 3)
}

fn field_chip(app: &AppWindow) -> bool {
    chip_on(app, |k| k == 0 || k == 1)
}

fn smart_list_open(app: &AppWindow) -> bool {
    let folders = app.get_folders();
    usize::try_from(app.get_current_folder())
        .ok()
        .and_then(|i| folders.row_data(i))
        .is_some_and(|f| f.kind == 3)
}

fn nothing_narrows(app: &AppWindow) -> bool {
    !searching(app) && !app.global::<FilterUi>().get_any_active()
}

pub const FILTERING: Lesson = Lesson {
    layout: Some(0),
    prepare: &[("[Channel1]", "volume", 1.0)],
    keep: &[],
    steps: &[
        LOAD_A,
        PLAY_A,
        Step {
            target: "ui,search",
            title: "Search",
            body: "Type part of an artist or a title. The list narrows as you type. ⌘F (Ctrl+F) \
                   jumps here from anywhere.",
            wait: Wait::App(searching),
            action: None,
        },
        Step {
            target: "ui,search",
            title: "Clear it",
            body: "Press Esc, or delete what you typed.",
            wait: Wait::App(not_searching),
            action: None,
        },
        Step {
            target: "ui,filter_button",
            title: "Show the filter bar",
            body: "This button shows the filter bar over the list. It lights up while a search \
                   or a filter narrows the list.",
            wait: Wait::App(filter_bar_shown),
            action: None,
        },
        Step {
            target: "ui,chip_bpm",
            title: "Match the tempo",
            body: "Click the BPM chip on the filter bar. The list keeps the tracks near deck \
                   A's tempo; its ▾ sets how near.",
            wait: Wait::App(bpm_match),
            action: None,
        },
        Step {
            target: "ui,chip_key",
            title: "Match the key",
            body: "Click the Key chip. Now only tracks in a key that mixes with deck A are left, \
                   ready to go next.",
            wait: Wait::App(key_match),
            action: None,
        },
        Step {
            target: "ui,filter_add",
            title: "Filter by a field",
            body: "Click a chip such as Genre and tick what to keep. + puts another field on \
                   the bar.",
            wait: Wait::App(field_chip),
            action: None,
        },
        Step {
            target: "ui,save_search",
            title: "Keep it as a smart playlist",
            body: "Press Save and give it a name. A smart playlist fills itself: new tracks \
                   that match join it.",
            wait: Wait::App(smart_list_open),
            action: None,
        },
        Step {
            target: "ui,filter_clear",
            title: "Clear the filters",
            body: "Back in your collection, Clear on the filter bar lets every track back in.",
            wait: Wait::App(nothing_narrows),
            action: None,
        },
        finish(
            "That's filtering",
            "Search, tempo and key match, filter chips and smart playlists. The guide has \
             the search words, such as bpm:120-126.",
            "https://github.com/winnerspiros/djsugar/blob/rust/docs/guide/filtering.md",
        ),
    ],
};

fn automix_open(app: &AppWindow) -> bool {
    app.get_side_panel_visible() && app.get_side_tab() == 0
}

fn automix_queued(app: &AppWindow) -> bool {
    automix_open(app) && app.get_side_rows().row_count() >= 3
}

fn bars_16(app: &AppWindow) -> bool {
    app.get_automix_length() == 4
}

fn bass_swap(app: &AppWindow) -> bool {
    app.get_automix_style() == 1
}

pub const AUTOMIX: Lesson = Lesson {
    layout: Some(0),
    prepare: &[("[AutoDJ]", "enabled", 0.0)],
    keep: &[],
    steps: &[
        Step {
            target: "ui,side_automix",
            title: "Open Automix",
            body: "The panel on the right holds the Automix list. Click its Automix tab.",
            wait: Wait::App(automix_open),
            action: None,
        },
        Step {
            target: "ui,side_automix",
            title: "Queue three tracks",
            body: "Drag tracks from the list into Automix, or right-click a track and choose \
                   Add to Automix.",
            wait: Wait::App(automix_queued),
            action: None,
        },
        Step {
            target: "ui,automix_length",
            title: "Pick a length",
            body: "How long each mix lasts, in seconds or bars. Pick 16 bars.",
            wait: Wait::App(bars_16),
            action: None,
        },
        Step {
            target: "ui,automix_style",
            title: "Pick a style",
            body: "How the tracks blend. Pick Bass swap: the next track comes in without its \
                   bass, then the two swap.",
            wait: Wait::App(bass_swap),
            action: None,
        },
        Step {
            target: "ui,automix_button",
            title: "Press AUTOMIX",
            body: "Automix plays the list, beatmatched, mixing each track into the next.",
            wait: Wait::Above("[AutoDJ]", "enabled", 0.5),
            action: None,
        },
        Step {
            target: "ui,automix_mix_now",
            title: "Mix now",
            body: "Automix mixes near the end of each track. Press Mix now to start the next \
                   mix straight away.",
            wait: Wait::App(mixing),
            action: None,
        },
        Step {
            target: "ui,automix_button",
            title: "Take over",
            body: "Press AUTOMIX again and the decks are yours. What's playing keeps playing.",
            wait: Wait::Below("[AutoDJ]", "enabled", 0.5),
            action: None,
        },
        finish(
            "That's Automix",
            "A queue, its length and style, Mix now and taking over. The guide covers the \
             rest of the list, such as ordering it by key.",
            "https://github.com/winnerspiros/djsugar/blob/rust/docs/guide/automix.md",
        ),
    ],
};

/// A hot cue set on deck B since the step began.
const NEW_CUE_B: Wait = Wait::Any(&[
    Wait::Changed("[Channel2]", "hotcue_1_enabled"),
    Wait::Changed("[Channel2]", "hotcue_2_enabled"),
    Wait::Changed("[Channel2]", "hotcue_3_enabled"),
    Wait::Changed("[Channel2]", "hotcue_4_enabled"),
    Wait::Changed("[Channel2]", "hotcue_5_enabled"),
    Wait::Changed("[Channel2]", "hotcue_6_enabled"),
    Wait::Changed("[Channel2]", "hotcue_7_enabled"),
    Wait::Changed("[Channel2]", "hotcue_8_enabled"),
]);

/// A sampler pad playing: deck B's pads play the second bank's sounds or
/// the first's.
const SAMPLE_PLAYING: Wait = Wait::Any(&[
    Wait::Above("[Sampler1]", "play", 0.5),
    Wait::Above("[Sampler2]", "play", 0.5),
    Wait::Above("[Sampler3]", "play", 0.5),
    Wait::Above("[Sampler4]", "play", 0.5),
    Wait::Above("[Sampler5]", "play", 0.5),
    Wait::Above("[Sampler6]", "play", 0.5),
    Wait::Above("[Sampler7]", "play", 0.5),
    Wait::Above("[Sampler8]", "play", 0.5),
    Wait::Above("[Sampler9]", "play", 0.5),
    Wait::Above("[Sampler10]", "play", 0.5),
    Wait::Above("[Sampler11]", "play", 0.5),
    Wait::Above("[Sampler12]", "play", 0.5),
    Wait::Above("[Sampler13]", "play", 0.5),
    Wait::Above("[Sampler14]", "play", 0.5),
    Wait::Above("[Sampler15]", "play", 0.5),
    Wait::Above("[Sampler16]", "play", 0.5),
]);

/// A mix on the DDJ-FLX4, each step on its own controls. The tips point at
/// the same controls on screen, which move as the controller does.
pub const DDJ_FLX4: Lesson = Lesson {
    layout: Some(0),
    prepare: &[
        ("[Channel1]", "play", 0.0),
        ("[Channel2]", "play", 0.0),
        ("[Channel1]", "volume", 1.0),
        ("[Channel2]", "volume", 0.0),
        ("[Channel2]", "pfl", 0.0),
        ("[Channel2]", "sync_enabled", 0.0),
        ("[Channel2]", "loop_enabled", 0.0),
        ("[Master]", "crossfader", 0.0),
        (EQ_A, "parameter1", 1.0),
        (EQ_B, "parameter1", 1.0),
        (CFX_A, "super1", 0.5),
        (BEAT_FX, "enabled", 0.0),
        ("[Mixer]", "knob_mode", 0.0),
    ],
    keep: &[],
    steps: &[
        Step {
            target: "[Channel1],load",
            title: "Load deck 1",
            body: "Turn the browse knob in the middle to pick a track, then press LOAD on \
                   the left.",
            wait: Wait::Loaded(0),
            action: Some(("Load demo track", Action::LoadDemo { deck: 0, demo: 0 })),
        },
        Step {
            target: "[Channel1],play",
            title: "PLAY/PAUSE",
            body: "Press PLAY/PAUSE on the left deck. Its light blinks while paused.",
            wait: Wait::Above("[Channel1]", "play", 0.5),
            action: None,
        },
        Step {
            target: "[Channel1],cue_default",
            title: "CUE",
            body: "Press CUE: the deck jumps back to the cue point and stops. Then press \
                   PLAY/PAUSE again.",
            wait: Wait::Below("[Channel1]", "play", 0.5),
            action: None,
        },
        Step {
            target: "[Channel1],play",
            title: "Play again",
            body: "Press PLAY/PAUSE and let deck 1 play.",
            wait: Wait::Above("[Channel1]", "play", 0.5),
            action: None,
        },
        Step {
            target: "[Channel1],jog",
            title: "Scratch",
            body: "Touch the top of the left jog wheel and move it back and forth. Turning \
                   the outer ring instead nudges the tempo, to fix a drift.",
            wait: Wait::Above("[Channel1]", "scratch2_enable", 0.5),
            action: None,
        },
        Step {
            target: "[Channel2],load",
            title: "Load deck 2",
            body: "Pick the next track with the browse knob and press LOAD on the right. Its \
                   channel fader is down, so nobody hears it yet.",
            wait: Wait::Loaded(1),
            action: Some(("Load demo track", Action::LoadDemo { deck: 1, demo: 1 })),
        },
        Step {
            target: "[Channel2],pfl",
            title: "Headphones CUE",
            body: "Press CUE above deck 2's channel fader to hear it in your headphones. The \
                   HEADPHONES MIXING knob blends it with the main mix.",
            wait: Wait::Above("[Channel2]", "pfl", 0.5),
            action: None,
        },
        Step {
            target: "[Channel2],sync_enabled",
            title: "BEAT SYNC",
            body: "Press BEAT SYNC on the right deck. Deck 2 takes deck 1's tempo and \
                   stays in time with it.",
            wait: Wait::Above("[Channel2]", "sync_enabled", 0.5),
            action: None,
        },
        Step {
            target: "[EqualizerRack1_[Channel2]_Effect1],parameter1",
            title: "Deck 2's LOW down",
            body: "Turn LOW on channel 2 all the way left, so its bass waits.",
            wait: Wait::Below(EQ_B, "parameter1", 0.25),
            action: None,
        },
        Step {
            target: "[Channel2],play",
            title: "Start deck 2",
            body: "Press PLAY/PAUSE on the right deck. It plays in time, in your headphones.",
            wait: Wait::Above("[Channel2]", "play", 0.5),
            action: None,
        },
        Step {
            target: "[Channel2],volume",
            title: "Channel fader up",
            body: "Push channel 2's fader slowly up. Both decks play now.",
            wait: Wait::Above("[Channel2]", "volume", 0.85),
            action: None,
        },
        Step {
            target: "[EqualizerRack1_[Channel1]_Effect1],parameter1",
            title: "Swap the bass",
            body: "Turn channel 1's LOW all the way down, then channel 2's back to the middle.",
            wait: Wait::Both(
                &Wait::Below(EQ_A, "parameter1", 0.25),
                &Wait::Above(EQ_B, "parameter1", 0.75),
            ),
            action: None,
        },
        Step {
            target: "[QuickEffectRack1_[Channel1]],super1",
            title: "Filter deck 1 away",
            body: "Turn channel 1's CFX knob: left takes the highs away, right the lows. SHIFT \
                   + SMART CFX picks another Color FX for it.",
            wait: Wait::Either(
                &Wait::Below(CFX_A, "super1", 0.2),
                &Wait::Above(CFX_A, "super1", 0.8),
            ),
            action: None,
        },
        Step {
            target: "[Channel1],volume",
            title: "Deck 1 out",
            body: "Pull channel 1's fader down, or slide the crossfader to the right.",
            wait: Wait::Either(
                &Wait::Below("[Channel1]", "volume", 0.1),
                &Wait::Above("[Master]", "crossfader", 0.9),
            ),
            action: None,
        },
        Step {
            target: "[Channel2],pads",
            title: "HOT CUE",
            body: "Press HOT CUE above deck 2's pads, then an empty pad to set a hot cue. \
                   Press it again to jump back to it.",
            wait: NEW_CUE_B,
            action: None,
        },
        Step {
            target: "[Channel2],pads",
            title: "BEAT LOOP",
            body: "Hold SHIFT and press BEAT JUMP for BEAT LOOP, then press a pad: each loops \
                   a length, 1/4 beat on the first up to 32 on the last. Press it again to \
                   leave.",
            wait: Wait::Above("[Channel2]", "loop_enabled", 0.5),
            action: None,
        },
        Step {
            target: "[Channel2],pads",
            title: "SAMPLER",
            body: "Press SAMPLER, then a pad to play a sound over the mix.",
            wait: SAMPLE_PLAYING,
            action: None,
        },
        Step {
            target: "[EffectRack1_EffectUnit5]",
            title: "Beat FX",
            body: "BEAT FX SELECT picks the effect, CH SELECT the channel and the beat \
                   buttons its length. Press ON/OFF and turn LEVEL/DEPTH.",
            wait: Wait::Above(BEAT_FX, "enabled", 0.5),
            action: None,
        },
        Step {
            target: "[EffectRack1_EffectUnit5_Effect1],enabled",
            title: "Beat FX off",
            body: "Press ON/OFF again.",
            wait: Wait::Below(BEAT_FX, "enabled", 0.5),
            action: None,
        },
        finish(
            "That's the DDJ-FLX4",
            "A whole mix on the controller, with its pads and effects. The guide shows \
             every button, with SHIFT.",
            "https://github.com/winnerspiros/djsugar/blob/rust/docs/guide/controllers/ddj-flx4.md",
        ),
    ],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mix_now_waits_for_the_mix_not_the_countdown() {
        assert!(is_mixing("Mixing into Sunrise"));
        assert!(is_mixing("Mixing into Lost in Space"));
        assert!(!is_mixing("Mixing into Sunrise in 0:12"));
        assert!(!is_mixing("Starting…"));
    }

    #[test]
    fn ids_are_unique_and_lessons_end_with_a_guide() {
        for (i, t) in TUTORIALS.iter().enumerate() {
            assert!(TUTORIALS[..i].iter().all(|u| u.id != t.id), "{}", t.id);
            assert_eq!(find(t.id).map(|f| f.name), Some(t.name));
            let last = t.lesson.steps.last().unwrap();
            assert!(matches!(last.wait, Wait::Next), "{}", t.id);
            assert!(
                matches!(last.action, Some((_, Action::Open(url))) if url.starts_with("https://github.com/winnerspiros/djsugar/blob/rust/docs/guide/")),
                "{}",
                t.id
            );
        }
    }
}
