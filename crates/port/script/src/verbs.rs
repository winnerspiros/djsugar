//! Every verb DJSugarScript knows, for the parser, the runner and editors.

/// What a verb does, which says how it runs and what it answers in a
/// condition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// Starts the deck; asks whether it plays.
    Play,
    /// Stops the deck; asks whether it's stopped.
    Pause,
    /// Flips an on/off control of the deck; asks whether it's on.
    Toggle(&'static str),
    /// Presses a button of the deck; can't be asked.
    Press(&'static str),
    /// Turns on a beat loop of the size given, or off when that size is on;
    /// asks whether any loop is on.
    Loop,
    /// Turns the loop off; asks whether one is on.
    LoopExit,
    /// Presses a hot cue; asks whether it's set.
    HotCue,
    /// Sets a 0..1 control of the deck to a level; can't be asked.
    Level(&'static str),
    /// `set 'control' value`; can't be asked.
    Set,
    /// `toggle 'control'`; asks whether the control is on.
    ToggleNamed,
    /// `press 'control'`; can't be asked.
    PressNamed,
    /// `get 'control'`: only asks, whether the control is above 0.
    Get,
    /// `adjust 'control' [level]`: sets any control's knob position from
    /// the controller's fader or knob (or the level); asks its position.
    Adjust,
    /// Sets a 0..1 control from the controller's fader or knob; also asked
    /// for its level in a comparison (`filter > 50%`).
    Value(&'static str),
    /// Only asked, for its level in a comparison: `vu_meter > 90%`.
    Reading(&'static str),
    /// `value`: only asked, the controller's fader or knob in a comparison
    /// (`value > 50%`).
    Input,
    /// Moves by the size given (1 if none) for each click of a controller's
    /// encoder, or once for a button or key: `jump 4` jumps 4 beats.
    Step(&'static str),
    /// `var 'name' [value]`: only asks, whether a variable is that value
    /// (or above 0). Variables belong to the host, see [`crate::Host`].
    Var,
    /// `set_var 'name' value`.
    SetVar,
    /// `toggle_var 'name'`: 0 becomes 1, anything else 0.
    ToggleVar,
    /// What an LED shows: `on`, `off`, `blink`, `color`, `led`. Only in a
    /// controller mapping's lights.
    Light,
    /// Done by the app, not a control: see [`crate::Host`].
    App,
}

/// Whether a verb acts on a deck.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TakesDeck {
    No,
    /// The deck in front of it, else the master deck.
    Yes,
    /// Only the deck in front of it; with none the verb acts elsewhere
    /// (`rating` rates the tracks picked in the browser).
    Optional,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArgKind {
    /// A plain number in this range.
    Number { min: f64, max: f64 },
    /// 0 to 1, or 0% to 100%.
    Level,
    /// A control's name: `'keylock'` on the deck, or `'[Master],gain'`.
    Control,
    /// A name in quotes: a variable, a colour.
    Text,
}

/// The default of a level the controller gives: a verb such as `volume`
/// on a fader with no level written takes the fader's.
pub const FROM_CONTROLLER: f64 = f64::NAN;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArgSpec {
    pub name: &'static str,
    pub kind: ArgKind,
    /// What a left-out argument counts as; `None` means it's needed.
    pub default: Option<f64>,
}

#[derive(Debug, PartialEq)]
pub struct Verb {
    pub name: &'static str,
    pub kind: Kind,
    pub deck: TakesDeck,
    pub args: &'static [ArgSpec],
    /// One line for editors.
    pub help: &'static str,
}

const fn def(name: &'static str, kind: Kind, deck: TakesDeck, args: &'static [ArgSpec], help: &'static str) -> Verb {
    Verb { name, kind, deck, args, help }
}

const CONTROL: ArgSpec = ArgSpec { name: "control", kind: ArgKind::Control, default: None };

const LEVEL: ArgSpec = ArgSpec { name: "level", kind: ArgKind::Level, default: Some(FROM_CONTROLLER) };

const NAME: ArgSpec = ArgSpec { name: "name", kind: ArgKind::Text, default: None };

const ANY_NUMBER: ArgKind = ArgKind::Number { min: f64::MIN, max: f64::MAX };

use Kind::*;
use TakesDeck::{Optional, Yes};

pub static VERBS: &[Verb] = &[
    def("play", Play, Yes, &[], "Start the deck"),
    def("pause", Pause, Yes, &[], "Stop the deck"),
    def(
        "play_pause",
        Toggle("play"),
        Yes,
        &[],
        "Start or stop the deck",
    ),
    def(
        "goto_start",
        Press("start"),
        Yes,
        &[],
        "Jump to the start of the track",
    ),
    def(
        "sync",
        Toggle("sync_enabled"),
        Yes,
        &[],
        "Turn sync on or off",
    ),
    def(
        "key_lock",
        Toggle("keylock"),
        Yes,
        &[],
        "Turn key lock on or off",
    ),
    def(
        "edit_grid",
        Toggle("beats_edit"),
        Yes,
        &[],
        "Start or finish editing the beat grid",
    ),
    def(
        "quantize",
        Toggle("quantize"),
        Yes,
        &[],
        "Turn quantize on or off",
    ),
    def(
        "slip",
        Toggle("slip_enabled"),
        Yes,
        &[],
        "Turn slip mode on or off",
    ),
    def(
        "loop",
        Loop,
        Yes,
        &[ArgSpec {
            name: "beats",
            kind: ArgKind::Number {
                min: 0.03125,
                max: 512.0,
            },
            default: Some(4.0),
        }],
        "Loop this many beats, or end that loop",
    ),
    def("loop_exit", LoopExit, Yes, &[], "End the loop"),
    def(
        "hot_cue",
        HotCue,
        Yes,
        &[ArgSpec {
            name: "number",
            kind: ArgKind::Number {
                min: 1.0,
                max: 36.0,
            },
            default: None,
        }],
        "Play from a hot cue, or set it",
    ),
    def("eject", Press("eject"), Yes, &[], "Unload the deck"),
    def(
        "volume",
        Level("volume"),
        Yes,
        &[ArgSpec {
            name: "level",
            kind: ArgKind::Level,
            default: None,
        }],
        "Set the channel fader",
    ),
    def(
        "level",
        Value("volume"),
        Yes,
        &[LEVEL],
        "The channel fader, from a controller fader or a level",
    ),
    def(
        "gain",
        Value("pregain"),
        Yes,
        &[LEVEL],
        "The deck's gain (trim)",
    ),
    def(
        "eq_high",
        Value("[EqualizerRack1_[Channel{n}]_Effect1],parameter3"),
        Yes,
        &[LEVEL],
        "The high EQ",
    ),
    def(
        "eq_mid",
        Value("[EqualizerRack1_[Channel{n}]_Effect1],parameter2"),
        Yes,
        &[LEVEL],
        "The mid EQ",
    ),
    def(
        "eq_low",
        Value("[EqualizerRack1_[Channel{n}]_Effect1],parameter1"),
        Yes,
        &[LEVEL],
        "The low EQ",
    ),
    def(
        "filter",
        Value("[QuickEffectRack1_[Channel{n}]],super1"),
        Yes,
        &[LEVEL],
        "The deck's filter knob",
    ),
    def("pitch", Value("rate"), Yes, &[LEVEL], "The tempo fader"),
    def(
        "song_pos",
        Value("playposition"),
        Yes,
        &[LEVEL],
        "Where the deck plays in the track; on an encoder each click moves it 0.5%",
    ),
    def(
        "value",
        Input,
        TakesDeck::No,
        &[],
        "In a comparison: the controller's fader or knob, value > 50%",
    ),
    def(
        "vu_meter",
        Reading("vu_meter"),
        Yes,
        &[],
        "In a comparison: the deck's level meter, vu_meter > 90%",
    ),
    def(
        "pfl",
        Toggle("pfl"),
        Yes,
        &[],
        "Turn headphone cue on or off",
    ),
    def(
        "master_deck",
        Toggle("sync_leader"),
        Yes,
        &[],
        "Make the deck the sync leader",
    ),
    def(
        "pitch_zero",
        Press("rate_set_zero"),
        Yes,
        &[],
        "Put the tempo back to the track's own",
    ),
    def("loop_half", Press("loop_halve"), Yes, &[], "Halve the loop"),
    def(
        "loop_double",
        Press("loop_double"),
        Yes,
        &[],
        "Double the loop",
    ),
    def(
        "loop_in",
        Press("loop_in"),
        Yes,
        &[],
        "Set where a loop starts",
    ),
    def(
        "loop_out",
        Press("loop_out"),
        Yes,
        &[],
        "Set where a loop ends",
    ),
    def(
        "reloop",
        Press("reloop_toggle"),
        Yes,
        &[],
        "Turn the last loop back on, or end it",
    ),
    def(
        "load",
        Press("LoadSelectedTrack"),
        Yes,
        &[],
        "Load the track picked in the browser",
    ),
    def(
        "crossfader",
        Toggle("[Mixer],crossfader_enabled"),
        TakesDeck::No,
        &[],
        "Turn the crossfader on or off; off, every deck plays as if set to thru",
    ),
    def(
        "rating",
        App,
        Optional,
        &[ArgSpec {
            name: "number of stars",
            kind: ArgKind::Number { min: 0.0, max: 5.0 },
            default: None,
        }],
        "Rate the tracks picked in the browser, or the deck's track; 0 clears",
    ),
    def(
        "show_info",
        App,
        TakesDeck::No,
        &[],
        "Open the side panel's Info tab on the track picked in the browser",
    ),
    def(
        "expand_browser",
        Toggle("[Skin],show_maximized_library"),
        TakesDeck::No,
        &[],
        "Expand the browser over the decks, or back",
    ),
    def(
        "jump",
        Step("beatjump"),
        Yes,
        &[ArgSpec {
            name: "beats",
            kind: ANY_NUMBER,
            default: Some(1.0),
        }],
        "Jump that many beats; on an encoder, each click (back when turned left)",
    ),
    def(
        "jog",
        Step("jog"),
        Yes,
        &[ArgSpec {
            name: "amount",
            kind: ANY_NUMBER,
            default: Some(25.0),
        }],
        "Turn the deck as its jog wheel does: bends it while it plays, moves through the track while paused; on an encoder, each click",
    ),
    def(
        "loop_move",
        Step("loop_move_loops"),
        Yes,
        &[ArgSpec {
            name: "loops",
            kind: ANY_NUMBER,
            default: Some(1.0),
        }],
        "Move the loop by its own length; on an encoder, each click",
    ),
    def(
        "set",
        Set,
        Yes,
        &[
            CONTROL,
            ArgSpec {
                name: "value",
                kind: ArgKind::Number {
                    min: f64::MIN,
                    max: f64::MAX,
                },
                default: None,
            },
        ],
        "Set any control to a value",
    ),
    def(
        "adjust",
        Adjust,
        Yes,
        &[CONTROL, LEVEL],
        "Move any control from a controller fader or knob, or to a level",
    ),
    def(
        "toggle",
        ToggleNamed,
        Yes,
        &[CONTROL],
        "Turn any on/off control on or off",
    ),
    def(
        "press",
        PressNamed,
        Yes,
        &[CONTROL],
        "Press any button control",
    ),
    def(
        "get",
        Get,
        Yes,
        &[CONTROL],
        "In a condition: whether a control is above 0",
    ),
    def(
        "var",
        Var,
        TakesDeck::No,
        &[
            NAME,
            ArgSpec {
                name: "value",
                kind: ANY_NUMBER,
                default: Some(FROM_CONTROLLER),
            },
        ],
        "In a condition: whether a variable is that value, or above 0",
    ),
    def(
        "set_var",
        SetVar,
        TakesDeck::No,
        &[
            NAME,
            ArgSpec {
                name: "value",
                kind: ANY_NUMBER,
                default: None,
            },
        ],
        "Set a variable of the mapping",
    ),
    def(
        "toggle_var",
        ToggleVar,
        TakesDeck::No,
        &[NAME],
        "Flip a variable between 0 and 1",
    ),
    def("on", Light, TakesDeck::No, &[], "In a light: on"),
    def("off", Light, TakesDeck::No, &[], "In a light: off"),
    def(
        "blink",
        Light,
        TakesDeck::No,
        &[
            ArgSpec {
                name: "milliseconds",
                kind: ArgKind::Number {
                    min: 50.0,
                    max: 10000.0,
                },
                default: Some(500.0),
            },
            ArgSpec {
                name: "colour",
                kind: ArgKind::Text,
                default: Some(f64::NAN),
            },
        ],
        "In a light: blink, 500 ms on and off unless given, in a color if named: blink 500 'green'",
    ),
    def(
        "color",
        Light,
        TakesDeck::No,
        &[NAME],
        "In a light: a color the controller knows, color 'red'",
    ),
    def(
        "led",
        Light,
        TakesDeck::No,
        &[ArgSpec {
            name: "value",
            kind: ArgKind::Number {
                min: 0.0,
                max: 127.0,
            },
            default: None,
        }],
        "In a light: send this value, 0 to 127",
    ),
];

/// The verb with this name.
pub fn verb(name: &str) -> Option<&'static Verb> {
    VERBS.iter().find(|v| v.name == name)
}
