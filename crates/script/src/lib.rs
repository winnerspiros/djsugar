//! DJSugarScript: short action lines in the style of VirtualDJ's VDJscript.
//!
//! ```text
//! rating 3                          rate the tracks picked in the browser
//! deck 1 play_pause                 a verb on one deck
//! sync & play                       a chain runs left to right
//! loop_exit ? loop_exit : loop 4    condition ? when true : when false
//! deck 2 (key_lock & quantize)      parentheses group, and pass the deck in
//! toggle 'slip_enabled'             any control by its name
//! ```
//!
//! A verb on its own targets the master deck. In a condition a verb is
//! asked rather than done: `play ? …` asks whether the deck is playing.
//! A chain runs every action even when one fails, and reports the first
//! error. Parsing checks every verb and its arguments, so an editor can
//! show a mistake as the DJ types; nothing runs until the whole line parses.
//!
//! The verbs live in [`VERBS`]. The ones backed by a control run here
//! ([`run_control`], [`query_control`]); the rest, such as `rating`, need
//! the app and go through its [`Host`].

mod controls;
mod parse;
mod verbs;

pub use controls::{level_control, query_control, run_control};
pub use parse::{parse, Arg, Deck, Expr, Script};
pub use verbs::{verb, ArgKind, ArgSpec, Kind, TakesDeck, Verb, FROM_CONTROLLER, VERBS};

use std::fmt;

/// A mistake in a line, or an action that couldn't be done.
#[derive(Clone, Debug, PartialEq)]
pub struct Error {
    pub message: String,
    /// Where in the line, in bytes, when it's a mistake in the text.
    pub at: Option<usize>,
}

impl Error {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            at: None,
        }
    }

    fn at(message: impl Into<String>, at: usize) -> Self {
        Self {
            message: message.into(),
            at: Some(at),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

/// One verb to do or ask, with its deck worked out (1 to 4) when the verb
/// takes one.
#[derive(Debug)]
pub struct Call<'a> {
    pub verb: &'static Verb,
    pub deck: Option<u8>,
    pub args: &'a [Arg],
    /// The controller's fader or knob, 0 to 1, when a controller runs the
    /// line: what a level verb with no level takes.
    pub input: Option<f64>,
    /// `input` is an encoder's clicks (1 a click right, -1 left) rather
    /// than where a fader or knob is: a level verb with no level written
    /// moves by [`STEP`] a click instead of jumping to the value.
    pub steps: bool,
}

/// How far a click of an encoder moves a level verb: 40 clicks from one
/// end to the other.
pub const STEP: f64 = 0.025;

/// What runs the verbs: the app, or a test.
pub trait Host {
    /// The deck a verb with no `deck` in front of it acts on, 1 to 4.
    fn master_deck(&self) -> u8;
    fn run(&mut self, call: &Call) -> Result<(), Error>;
    fn query(&mut self, call: &Call) -> Result<bool, Error>;
    /// How far up a verb is, 0 to 1, for `verb > level` and `verb < level`.
    fn level(&mut self, call: &Call) -> Result<f64, Error> {
        Ok(if self.query(call)? { 1.0 } else { 0.0 })
    }
    /// The controller's value for the line now running, 0 to 1.
    fn input(&self) -> Option<f64> {
        None
    }
    /// Whether [`Host::input`] is an encoder's clicks (see [`Call::steps`]).
    fn steps(&self) -> bool {
        false
    }
}

/// Parses and runs a line.
pub fn run(src: &str, host: &mut dyn Host) -> Result<(), Error> {
    parse(src)?.run(host)
}
