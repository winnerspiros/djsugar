//! Changing a row of the DJ's own mapping from the Controller Editor: what a
//! control does, when it acts and how its value is read. A change is a row
//! of the own mapping's file (see [`crate::format`]) put over the Mixxx
//! mapping it's based on; going back to the original takes the rows away.

use djsugar_control::{Behavior, ControlRegistry};

use crate::catalog::{describe, Function};
use crate::format::{Action, Midi, DjSugarMapping, Row};
use crate::mapping::{Options, When};
use crate::view;

/// What a row is set to: its function, when it acts and how the value is
/// read. Read from a row with [`Binding::of`] and written with [`set`].
#[derive(Clone, Debug, PartialEq)]
pub struct Binding {
    /// `[Channel1],play`; `None` when the control does nothing.
    pub control: Option<String>,
    pub when: When,
    pub buttons: Buttons,
    pub encoder: Encoder,
    pub invert: bool,
    /// Soft takeover: a knob or fader only takes over once it passes the
    /// value on screen.
    pub pickup: bool,
}

/// How a button's press is read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Buttons {
    /// As the function does it: hold for momentary ones, a press toggles
    /// on/off ones.
    #[default]
    Normal,
    /// Any press is on, let go is off (Mixxx's `button`).
    Hold,
    /// Every message is a press (Mixxx's `switch`): for controls that send
    /// one message per press.
    Trigger,
}

/// How a knob's value is read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Encoder {
    /// The value is where the knob is (0 to 127).
    #[default]
    Absolute,
    /// An endless encoder sending 64 ± steps.
    Relative,
    /// The same, turning the other way.
    RelativeReversed,
    /// An endless encoder sending steps as two's complement (1 up, 127 down).
    TwosComplement,
    /// An endless browse knob: each step is one move.
    Steps,
}

impl Buttons {
    pub const ALL: [Buttons; 3] = [Buttons::Normal, Buttons::Hold, Buttons::Trigger];

    pub fn label(self) -> &'static str {
        match self {
            Buttons::Normal => "As the function does it",
            Buttons::Hold => "On while held",
            Buttons::Trigger => "Every message is a press",
        }
    }

    fn option(self) -> Option<&'static str> {
        match self {
            Buttons::Normal => None,
            Buttons::Hold => Some("button"),
            Buttons::Trigger => Some("switch"),
        }
    }
}

impl Encoder {
    pub const ALL: [Encoder; 5] = [
        Encoder::Absolute,
        Encoder::Relative,
        Encoder::RelativeReversed,
        Encoder::TwosComplement,
        Encoder::Steps,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Encoder::Absolute => "Absolute (knob or fader)",
            Encoder::Relative => "Endless, 64 is still",
            Encoder::RelativeReversed => "Endless, 64 is still, reversed",
            Encoder::TwosComplement => "Endless, 1 up and 127 down",
            Encoder::Steps => "Endless, one step per click",
        }
    }

    fn option(self) -> Option<&'static str> {
        match self {
            Encoder::Absolute => None,
            Encoder::Relative => Some("rot64"),
            Encoder::RelativeReversed => Some("rot64inv"),
            Encoder::TwosComplement => Some("diff"),
            Encoder::Steps => Some("selectknob"),
        }
    }
}

/// When a row acts, as the editor lists them.
pub const WHENS: [When; 5] = [
    When::Press,
    When::Release,
    When::Long,
    When::Tap,
    When::Double,
];

pub fn when_label(w: When) -> &'static str {
    match w {
        When::Press => "Press (or move)",
        When::Release => "Release",
        When::Long => "Long press",
        When::Tap => "Tap (let go quickly)",
        When::Double => "Double press",
    }
}

impl Binding {
    /// What `row` is set to now.
    pub fn of(row: &view::Row) -> Self {
        let Some(s) = row.sources.first() else {
            return Binding::nothing(When::Press);
        };
        let o = s.options;
        Binding {
            control: (s.is_control() && !s.key.is_empty())
                .then(|| format!("{},{}", s.group, s.key)),
            when: s.when,
            buttons: if o.has(Options::BUTTON) {
                Buttons::Hold
            } else if o.has(Options::SWITCH) {
                Buttons::Trigger
            } else {
                Buttons::Normal
            },
            encoder: if o.has(Options::ROT64) {
                Encoder::Relative
            } else if o.has(Options::ROT64_INV) {
                Encoder::RelativeReversed
            } else if o.has(Options::DIFF) {
                Encoder::TwosComplement
            } else if o.has(Options::SELECT_KNOB) {
                Encoder::Steps
            } else {
                Encoder::Absolute
            },
            invert: o.has(Options::INVERT),
            pickup: o.has(Options::SOFT_TAKEOVER),
        }
    }

    fn nothing(when: When) -> Self {
        Binding {
            control: None,
            when,
            buttons: Buttons::Normal,
            encoder: Encoder::Absolute,
            invert: false,
            pickup: false,
        }
    }

    fn options(&self) -> Vec<String> {
        let mut out: Vec<String> = [self.buttons.option(), self.encoder.option()]
            .into_iter()
            .flatten()
            .map(String::from)
            .collect();
        if self.invert {
            out.push("invert".into());
        }
        out
    }
}

/// `[("shift", None), ("m1", Some(2))]` → `"shift & m1=2"`.
pub fn layer_text(layer: &[(String, Option<f64>)]) -> String {
    layer
        .iter()
        .map(|(n, v)| match v {
            Some(v) => format!("{n}={v}"),
            None => n.clone(),
        })
        .collect::<Vec<_>>()
        .join(" & ")
}

/// The message a row's first binding comes in on, with the other half of a
/// 14-bit pair.
fn midi_of(row: &view::Row) -> Option<Midi> {
    let first = row.sources.first()?;
    let mut midi = Midi::new(first.status, first.number);
    if first.options.has(Options::FOURTEEN_BIT_MSB) {
        midi.lsb = row
            .sources
            .iter()
            .find(|s| s.options.has(Options::FOURTEEN_BIT_LSB))
            .map(|s| (s.status, s.number));
    }
    Some(midi)
}

/// The own mapping's rows for the message and layer of `row`.
fn own_rows<'a>(
    own: &'a DjSugarMapping,
    row: &'a view::Row,
) -> impl Iterator<Item = (usize, &'a Row)> + 'a {
    own.rows.iter().enumerate().filter(move |(_, r)| {
        !r.off
            && row.sources.iter().any(|s| {
                (r.midi.status, r.midi.number) == (s.status, s.number)
                    && r.layer_conditions() == s.layer
            })
    })
}

/// Whether the own mapping changes `row`.
pub fn is_edited(own: &DjSugarMapping, row: &view::Row) -> bool {
    own_rows(own, row).next().is_some()
}

/// Sets what `row` does in the own mapping. The row's message then does only
/// this on its layer, whatever the base mapping or its script did there
/// (SHIFT functions a script gave the same button go too).
pub fn set(own: &mut DjSugarMapping, row: &view::Row, b: &Binding) {
    let (Some(midi), Some(first)) = (midi_of(row), row.sources.first()) else {
        return;
    };
    let layer = layer_text(&first.layer);
    let same = |r: &Row, when: When| {
        (r.midi.status, r.midi.number) == (midi.status, midi.number)
            && r.layer == layer
            && r.when == when
            && !r.off
    };
    // This file's row for the binding shown goes; its stand-in for the
    // base's binding at another time stays.
    own.rows
        .retain(|r| !(same(r, first.when) && !r.remove) && !(same(r, b.when) && r.remove));
    if b.when != first.when && !own.rows.iter().any(|r| same(r, first.when)) {
        own.rows.push(Row {
            layer: layer.clone(),
            when: first.when,
            remove: true,
            ..Row::new(midi, Action::Nothing)
        });
    }
    let mut new = match &b.control {
        Some(c) => Row::new(
            midi,
            Action::Bind {
                control: c.clone(),
                options: b.options(),
            },
        ),
        None => Row {
            remove: true,
            ..Row::new(midi, Action::Nothing)
        },
    };
    new.layer = layer;
    new.when = b.when;
    new.name = row.control.clone();
    if b.pickup && b.control.is_some() {
        new.takeover = Some("pickup".into());
    }
    own.rows.push(new);
}

/// What a control can do besides a function: an DJSugarScript line, or be a
/// modifier other rows look at through their `layer` ("Only when").
#[derive(Clone, Debug, PartialEq)]
pub enum Does {
    /// A function from the picker (see [`Binding`]).
    Function,
    /// An DJSugarScript line: `deck 1 play ? pause : play`.
    Script(String),
    /// Sets the variable to 1 while held and 0 when let go: a SHIFT.
    Hold(String),
    /// Flips the variable between 1 and 0 on each press: a mode button.
    Toggle(String),
}

impl Does {
    /// What the row shown does, read from its own mapping rows.
    pub fn of(own: Option<&DjSugarMapping>, row: &view::Row) -> Self {
        let Some(line) = row.sources.first().and_then(|s| s.action.clone()) else {
            return Does::Function;
        };
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.as_slice() {
            ["toggle_var", name] => Does::Toggle(unquote(name)),
            ["set_var", name, "1"] => {
                // A hold has its let-go row too.
                let name = unquote(name);
                let release = format!("set_var '{name}' 0");
                let has_release = own.is_some_and(|o| {
                    own_rows_all_whens(o, row).any(|r| {
                        r.when == When::Release
                            && matches!(&r.action, Action::Script(l) if *l == release)
                    })
                });
                if has_release {
                    Does::Hold(name)
                } else {
                    Does::Script(line)
                }
            }
            _ => Does::Script(line),
        }
    }
}

fn unquote(s: &str) -> String {
    s.trim_matches('\'').to_string()
}

/// The own mapping's rows on the message and layer of `row`, at any time.
fn own_rows_all_whens<'a>(
    own: &'a DjSugarMapping,
    row: &'a view::Row,
) -> impl Iterator<Item = &'a Row> + 'a {
    let layer = row
        .sources
        .first()
        .map(|s| s.layer.clone())
        .unwrap_or_default();
    let keys: Vec<(u8, u8)> = row.sources.iter().map(|s| (s.status, s.number)).collect();
    own.rows.iter().filter(move |r| {
        !r.off && keys.contains(&(r.midi.status, r.midi.number)) && r.layer_conditions() == layer
    })
}

/// Checks an "Only when" text: variable names, each alone (above 0) or
/// with `=` and a number, joined with `&`: `shift`, `mode=2 & shift`.
/// Returns it written the usual way, or what's wrong.
pub fn check_layer(text: &str) -> Result<String, String> {
    let mut parts = Vec::new();
    for p in text.split('&').map(str::trim).filter(|p| !p.is_empty()) {
        let (name, value) = match p.split_once('=') {
            Some((n, v)) => (n.trim(), Some(v.trim())),
            None => (p, None),
        };
        let ok_name = !name.is_empty()
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !name.starts_with(|c: char| c.is_ascii_digit());
        if !ok_name {
            return Err(format!(
                "“{name}” isn't a variable name: letters, digits and _."
            ));
        }
        match value {
            Some(v) => match v.parse::<f64>() {
                Ok(_) => parts.push(format!("{name}={v}")),
                Err(_) => return Err(format!("“{v}” isn't a number.")),
            },
            None => parts.push(name.to_string()),
        }
    }
    Ok(parts.join(" & "))
}

/// Makes `row` an DJSugarScript line or a modifier ([`Does`]); for
/// [`Does::Function`] it does nothing (use [`set`]). The message's other
/// rows on this layer go, so a modifier does only that. A modifier's
/// variable is added to the mapping's variables if it's new.
pub fn set_does(own: &mut DjSugarMapping, row: &view::Row, does: &Does) {
    let when = row.sources.first().map_or(When::Press, |s| s.when);
    set_does_at(own, row, does, when);
}

/// As [`set_does`], with a script line acting at `when` (Acts on).
pub fn set_does_at(own: &mut DjSugarMapping, row: &view::Row, does: &Does, when: When) {
    let (Some(midi), Some(first)) = (midi_of(row), row.sources.first()) else {
        return;
    };
    let layer = layer_text(&first.layer);
    let lines: Vec<(When, String)> = match does {
        Does::Function => return,
        Does::Script(line) => vec![(when, line.clone())],
        Does::Hold(v) => vec![
            (When::Press, format!("set_var '{v}' 1")),
            (When::Release, format!("set_var '{v}' 0")),
        ],
        Does::Toggle(v) => vec![(When::Press, format!("toggle_var '{v}'"))],
    };
    let on_layer = |r: &Row| {
        (r.midi.status, r.midi.number) == (midi.status, midi.number) && r.layer == layer && !r.off
    };
    let modifier = matches!(does, Does::Hold(_) | Does::Toggle(_));
    if modifier {
        // Everything this button did on this layer goes, the base's too.
        own.rows.retain(|r| !on_layer(r));
        for when in crate::edit::WHENS {
            own.rows.push(Row {
                layer: layer.clone(),
                when,
                remove: true,
                ..Row::new(midi, Action::Nothing)
            });
        }
        // The modifier's own rows replace the stand-ins at their times.
        own.rows
            .retain(|r| !(on_layer(r) && r.remove && lines.iter().any(|(w, _)| *w == r.when)));
    } else {
        own.rows.retain(|r| {
            !(on_layer(r) && r.when == first.when && !r.remove)
                && !(on_layer(r) && r.when == when && r.remove)
        });
        if when != first.when && !own.rows.iter().any(|r| on_layer(r) && r.when == first.when) {
            // The base's binding at the old time goes, as in `set`.
            own.rows.push(Row {
                layer: layer.clone(),
                when: first.when,
                remove: true,
                ..Row::new(midi, Action::Nothing)
            });
        }
    }
    for (when, line) in lines {
        let mut new = Row::script(midi, &line);
        new.layer = layer.clone();
        new.when = when;
        new.name = row.control.clone();
        own.rows.push(new);
    }
    if let Does::Hold(v) | Does::Toggle(v) = does {
        if !own.variables.iter().any(|x| &x.name == v) {
            own.variables.push(crate::format::Variable {
                name: v.clone(),
                default: 0.0,
                saved: false,
            });
        }
    }
}

/// Puts what `row` does on the layer `layer` ("Only when", already
/// checked with [`check_layer`]) too, or instead when `moving`: there the
/// message stops doing it on its own layer.
pub fn set_layer(own: &mut DjSugarMapping, row: &view::Row, layer: &str, moving: bool) {
    let (Some(midi), Some(first)) = (midi_of(row), row.sources.first()) else {
        return;
    };
    let old = layer_text(&first.layer);
    if old == layer {
        return;
    }
    let copies: Vec<Row> = row
        .sources
        .iter()
        .filter(|s| (s.status, s.number) == (midi.status, midi.number))
        .map(|s| {
            let action = match &s.action {
                Some(line) => Action::Script(line.clone()),
                None if s.options.has(Options::SCRIPT) => Action::Js {
                    function: s.key.clone(),
                    group: s.group.clone(),
                },
                None => Action::Bind {
                    control: format!("{},{}", s.group, s.key),
                    options: s.options.names().into_iter().map(String::from).collect(),
                },
            };
            let mut r = Row::new(midi, action);
            r.layer = layer.to_string();
            r.when = s.when;
            r.name = row.control.clone();
            r
        })
        .collect();
    if moving {
        let whens: Vec<When> = row.sources.iter().map(|s| s.when).collect();
        own.rows.retain(|r| {
            !((r.midi.status, r.midi.number) == (midi.status, midi.number)
                && r.layer == old
                && whens.contains(&r.when)
                && !r.off)
        });
        for when in whens {
            if !own
                .rows
                .iter()
                .any(|r| r.remove && r.midi == midi && r.layer == old && r.when == when)
            {
                own.rows.push(Row {
                    layer: old.clone(),
                    when,
                    remove: true,
                    ..Row::new(midi, Action::Nothing)
                });
            }
        }
    }
    own.rows.extend(copies);
}

/// Puts `row` back as the base mapping has it.
pub fn reset(own: &mut DjSugarMapping, row: &view::Row) {
    let gone: Vec<usize> = own_rows(own, row).map(|(i, _)| i).collect();
    let mut i = 0;
    own.rows.retain(|_| {
        let keep = !gone.contains(&i);
        i += 1;
        keep
    });
}

/// A function a control can be given, on each target that has it.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub category: &'static str,
    pub name: String,
    /// The targets ("Deck 1", "FX 2") with the control for each.
    pub targets: Vec<(String, String)>,
    /// A knob or fader function (a level, a position) rather than a button.
    pub continuous: bool,
}

/// Every function there is to choose from, by category in the editor's
/// order: the engine's controls in `registry`, by their plain names.
pub fn choices(registry: &ControlRegistry) -> Vec<Choice> {
    let mut out: Vec<Choice> = Vec::new();
    for k in registry.keys() {
        let Some(c) = registry.get(&k.group, &k.item) else {
            continue;
        };
        let behavior = c.behavior();
        if matches!(behavior, Behavior::Indicator | Behavior::Free)
            || k.item.ends_with("_indicator")
            || k.item.contains("VuMeter")
            || k.group.starts_with("[PreviewDeck")
        {
            continue;
        }
        let f: Function = describe(&k.group, &k.item);
        let control = format!("{},{}", k.group, k.item);
        match out
            .iter_mut()
            .find(|c| c.category == f.category && c.name == f.name)
        {
            Some(c) => {
                if !c.targets.iter().any(|(t, _)| *t == f.target) {
                    c.targets.push((f.target, control));
                }
            }
            None => out.push(Choice {
                category: f.category,
                continuous: matches!(behavior, Behavior::Linear | Behavior::CenteredOnDefault),
                targets: vec![(f.target.clone(), control)],
                name: f.name.clone(),
            }),
        }
    }
    let order = |c: &Choice| Function::new(c.category, "", "").order();
    out.sort_by(|a, b| {
        order(a)
            .cmp(&order(b))
            .then_with(|| natural(&a.name, &b.name))
    });
    for c in &mut out {
        c.targets.sort_by(|a, b| natural(&a.0, &b.0));
    }
    out
}

/// "Hot cue 2" before "Hot cue 10".
fn natural(a: &str, b: &str) -> std::cmp::Ordering {
    let key = |s: &str| {
        let digits: String = s
            .chars()
            .skip_while(|c| !c.is_ascii_digit())
            .take_while(|c| c.is_ascii_digit())
            .collect();
        let head: String = s.chars().take_while(|c| !c.is_ascii_digit()).collect();
        (
            head.to_lowercase(),
            digits.parse::<u64>().unwrap_or(0),
            s.to_lowercase(),
        )
    };
    key(a).cmp(&key(b))
}

/// A name for the DJ's first own copy of `name`: "My Pioneer DDJ-FLX4", or
/// with a number when that's taken.
pub fn my_name(name: &str, taken: &[String]) -> String {
    let base = format!("My {name}");
    (1..)
        .map(|n| {
            if n == 1 {
                base.clone()
            } else {
                format!("{base} {n}")
            }
        })
        .find(|n| !taken.contains(n))
        .unwrap_or(base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping::Mapping;

    fn flx4() -> (Mapping, std::path::PathBuf) {
        let dir = crate::default_mappings_dir().expect("bundled mappings");
        let m = Mapping::load(&dir.join("Pioneer-DDJ-FLX4.midi.xml")).unwrap();
        (m, dir)
    }

    fn view_row(m: &Mapping, key: (u8, u8)) -> view::Row {
        let input = &m.inputs[&key][0];
        view::Row {
            function: describe(&input.group, &input.key),
            control: "PLAY".into(),
            part: None,
            layer: String::new(),
            kind: "button".into(),
            keys: vec![crate::probe::message_key(key.0, key.1)],
            midi: String::new(),
            script: None,
            description: String::new(),
            options: String::new(),
            sources: vec![view::Source {
                status: key.0,
                number: key.1,
                group: input.group.clone(),
                key: input.key.clone(),
                options: input.options,
                action: None,
                layer: Vec::new(),
                when: When::Press,
            }],
            edited: false,
            off: false,
            note: String::new(),
        }
    }

    #[test]
    fn a_change_goes_over_the_base_and_resets() {
        let (base, dir) = flx4();
        let key = (0x90, 0x0B);
        let row = view_row(&base, key);
        let mut own = DjSugarMapping::new("Mine");
        own.based_on = Some("Pioneer-DDJ-FLX4.midi.xml".into());
        let mut b = Binding::of(&row);
        b.control = Some("[Channel1],sync_enabled".into());
        b.buttons = Buttons::Hold;
        set(&mut own, &row, &b);
        assert!(is_edited(&own, &row));
        let built = own.build(Some(&base), &dir);
        let inputs = &built.inputs[&key];
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].key, "sync_enabled");
        assert!(inputs[0].options.has(Options::BUTTON));
        // Setting it again replaces the change rather than adding one.
        b.control = Some("[Channel1],cue_default".into());
        set(&mut own, &row, &b);
        assert_eq!(own.rows.len(), 1);
        reset(&mut own, &row);
        assert!(own.rows.is_empty());
        assert!(!is_edited(&own, &row));
    }

    #[test]
    fn acting_at_another_time_takes_the_press_away() {
        let (base, dir) = flx4();
        let key = (0x90, 0x0B);
        let row = view_row(&base, key);
        let mut own = DjSugarMapping::new("Mine");
        let mut b = Binding::of(&row);
        b.when = When::Long;
        set(&mut own, &row, &b);
        let built = own.build(Some(&base), &dir);
        let inputs = &built.inputs[&key];
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].when, When::Long);
        // Doing nothing is a row that only takes the base's away.
        let mut none = DjSugarMapping::new("Mine");
        set(&mut none, &row, &Binding::nothing(When::Press));
        assert!(!none.build(Some(&base), &dir).inputs.contains_key(&key));
    }

    #[test]
    fn functions_to_choose_from() {
        let registry = ControlRegistry::new();
        registry.toggle("[Channel1]", "play");
        registry.toggle("[Channel2]", "play");
        registry.create("[Channel1]", "volume", 1.0, 0.0, 1.0, Behavior::Linear);
        registry.indicator("[Channel1]", "play_indicator", 1.0);
        let c = choices(&registry);
        let play = c
            .iter()
            .find(|c| c.targets[0].1 == "[Channel1],play")
            .unwrap();
        assert_eq!(play.targets.len(), 2);
        assert!(!play.continuous);
        assert!(c.iter().any(|c| c.continuous));
        assert!(!c
            .iter()
            .any(|c| c.targets.iter().any(|t| t.1.contains("indicator"))));
        assert_eq!(my_name("FLX4", &["My FLX4".into()]), "My FLX4 2");
        assert_eq!(
            layer_text(&[("shift".into(), None), ("m1".into(), Some(2.0))]),
            "shift & m1=2"
        );
    }

    #[test]
    fn only_when_is_checked() {
        assert_eq!(check_layer(" shift ").unwrap(), "shift");
        assert_eq!(check_layer("mode = 2 & shift").unwrap(), "mode=2 & shift");
        assert_eq!(check_layer("").unwrap(), "");
        assert!(check_layer("2fast").is_err());
        assert!(check_layer("mode=two").is_err());
    }

    #[test]
    fn a_button_becomes_a_shift_and_another_row_uses_it() {
        let (base, dir) = flx4();
        let key = (0x90, 0x0B);
        let row = view_row(&base, key);
        let mut own = DjSugarMapping::new("Mine");
        set_does(&mut own, &row, &Does::Hold("shift".into()));
        assert_eq!(own.variables.len(), 1);
        let built = own.build(Some(&base), &dir);
        let inputs = &built.inputs[&key];
        let lines: Vec<_> = inputs.iter().filter_map(|i| i.action.clone()).collect();
        assert_eq!(lines, ["set_var 'shift' 1", "set_var 'shift' 0"]);
        assert!(inputs.iter().all(|i| i.action.is_some()));
        let mut held = row.clone();
        held.sources[0].action = Some("set_var 'shift' 1".into());
        assert_eq!(Does::of(Some(&own), &held), Does::Hold("shift".into()));

        // CUE on deck 1 does sync while shift is held, and still cues.
        let cue = view_row(&base, (0x90, 0x0C));
        let mut copy = DjSugarMapping::new("Mine");
        set_layer(&mut copy, &cue, "shift", false);
        let built = copy.build(Some(&base), &dir);
        let inputs = &built.inputs[&(0x90, 0x0C)];
        assert!(inputs
            .iter()
            .any(|i| i.layer == [("shift".to_string(), None)]));
        assert!(inputs.iter().any(|i| i.layer.is_empty()));
        // Moved, it only works with shift.
        let mut moved = DjSugarMapping::new("Mine");
        set_layer(&mut moved, &cue, "shift", true);
        let built = moved.build(Some(&base), &dir);
        assert!(built.inputs[&(0x90, 0x0C)]
            .iter()
            .all(|i| !i.layer.is_empty()));
    }

    #[test]
    fn a_script_line_replaces_the_function() {
        let (base, dir) = flx4();
        let key = (0x90, 0x0B);
        let row = view_row(&base, key);
        let mut own = DjSugarMapping::new("Mine");
        set_does(
            &mut own,
            &row,
            &Does::Script("var 'shift' ? sync : play_pause".into()),
        );
        let built = own.build(Some(&base), &dir);
        let inputs = &built.inputs[&key];
        assert_eq!(inputs.len(), 1);
        assert_eq!(
            inputs[0].action.as_deref(),
            Some("var 'shift' ? sync : play_pause")
        );
    }
}
