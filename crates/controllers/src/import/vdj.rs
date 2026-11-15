//! VirtualDJ mapper files: `<map value="PLAY_PAUSE" action="play_pause"/>`.
//! A mapper names the controller's buttons (`SHIFT_` for shifted, `LED_`
//! for lights) but not their MIDI; VirtualDJ keeps that in its own device
//! definitions, which are encrypted. DJ Sugar has its own tables for the
//! controllers it knows, built on the pictures of them in `layouts/`.
//!
//! The actions are VDJscript, which DJSugarScript is modelled on, so most
//! lines translate verb by verb. `holding ? a : b` becomes a long press
//! and a tap; `device_side` and `action_deck` are settled per side.

use super::Import;
use crate::format::{DjSugarMapping, Light, LightSource, Midi, Row, Variable};
use crate::layout::Layout;
use crate::mapping::When;

// ---------------------------------------------------------------- XML

/// `&apos;` and friends.
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest.find(';') else { break };
        let entity = &rest[1..end];
        let ch = match entity {
            "apos" => Some('\''),
            "quot" => Some('"'),
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            e if e.starts_with("#x") => u32::from_str_radix(&e[2..], 16)
                .ok()
                .and_then(char::from_u32),
            e if e.starts_with('#') => e[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn attribute(tag: &str, name: &str) -> Option<String> {
    let at = tag.find(&format!("{name}=\""))? + name.len() + 2;
    let end = tag[at..].find('"')?;
    Some(unescape(&tag[at..at + end]))
}

/// The device name and the `(value, action)` pairs.
fn read(text: &str) -> Result<(String, Vec<(String, String)>), String> {
    let start = text
        .find("<mapper")
        .ok_or("This isn't a VirtualDJ mapping")?;
    let head_end = text[start..].find('>').ok_or("Damaged VirtualDJ mapping")? + start;
    let device = attribute(&text[start..head_end], "device").unwrap_or_default();
    let mut maps = Vec::new();
    let mut rest = &text[head_end..];
    while let Some(i) = rest.find("<map ") {
        rest = &rest[i..];
        let end = rest
            .find("/>")
            .or_else(|| rest.find('>'))
            .unwrap_or(rest.len());
        let tag = &rest[..end];
        if let (Some(v), Some(a)) = (attribute(tag, "value"), attribute(tag, "action")) {
            maps.push((v, a));
        }
        rest = &rest[end.min(rest.len())..];
        if rest.len() <= 1 {
            break;
        }
        rest = &rest[1..];
    }
    Ok((device, maps))
}

// ---------------------------------------------------------- VDJscript

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Word(String),
    /// A number with what followed it: `%`, `ms`, `bt`, and whether it
    /// was written with a sign (`+1`).
    Num {
        v: f64,
        unit: &'static str,
        signed: bool,
    },
    Text(String),
    Question,
    Colon,
    And,
    Open,
    Close,
    Greater,
    Less,
}

fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let c: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < c.len() {
        let ch = c[i];
        match ch {
            ' ' | '\t' | '\n' | '\r' => i += 1,
            '?' => {
                out.push(Tok::Question);
                i += 1
            }
            ':' => {
                out.push(Tok::Colon);
                i += 1
            }
            '&' => {
                out.push(Tok::And);
                i += if c.get(i + 1) == Some(&'&') { 2 } else { 1 };
            }
            '(' => {
                out.push(Tok::Open);
                i += 1
            }
            ')' => {
                out.push(Tok::Close);
                i += 1
            }
            '>' => {
                out.push(Tok::Greater);
                i += 1
            }
            '<' => {
                out.push(Tok::Less);
                i += 1
            }
            '\'' | '"' => {
                let end = c[i + 1..]
                    .iter()
                    .position(|x| *x == ch)
                    .ok_or("unclosed quote")?;
                out.push(Tok::Text(c[i + 1..i + 1 + end].iter().collect()));
                i += end + 2;
            }
            _ if ch.is_ascii_digit()
                || ((ch == '+' || ch == '-' || ch == '.')
                    && c.get(i + 1)
                        .is_some_and(|n| n.is_ascii_digit() || *n == '.')) =>
            {
                let signed = ch == '+' || ch == '-';
                let s = i;
                i += 1;
                while i < c.len() && (c[i].is_ascii_digit() || c[i] == '.' || c[i] == '/') {
                    i += 1;
                }
                let text: String = c[s..i].iter().collect();
                let v = match text.split_once('/') {
                    Some((a, b)) => {
                        a.parse::<f64>().map_err(|_| "bad number")?
                            / b.parse::<f64>().map_err(|_| "bad number")?
                    }
                    None => text
                        .parse::<f64>()
                        .map_err(|_| format!("bad number {text}"))?,
                };
                let rest: String = c[i..].iter().take(2).collect();
                let unit = if rest.starts_with('%') {
                    i += 1;
                    "%"
                } else if rest == "ms" {
                    i += 2;
                    "ms"
                } else if rest == "bt" {
                    i += 2;
                    "bt"
                } else {
                    ""
                };
                out.push(Tok::Num { v, unit, signed });
            }
            _ if ch.is_alphanumeric() || ch == '_' || ch == '$' || ch == '@' => {
                let s = i;
                while i < c.len()
                    && (c[i].is_alphanumeric() || c[i] == '_' || c[i] == '$' || c[i] == '@')
                {
                    i += 1;
                }
                out.push(Tok::Word(
                    c[s..i].iter().collect::<String>().to_ascii_lowercase(),
                ));
            }
            _ => return Err(format!("'{ch}' isn't VDJscript DJ Sugar reads")),
        }
    }
    Ok(out)
}

#[derive(Clone, Debug, PartialEq)]
enum Sel {
    Number(u8),
    Left,
    Right,
    Default,
    All,
    Master,
}

#[derive(Clone, Debug, PartialEq)]
enum Arg {
    Num {
        v: f64,
        unit: &'static str,
        signed: bool,
    },
    Text(String),
    Word(String),
}

#[derive(Clone, Debug, PartialEq)]
enum V {
    Verb(String, Vec<Arg>),
    If(Box<V>, Box<V>, Option<Box<V>>),
    Chain(Vec<V>),
    Deck(Sel, Box<V>),
    Compare(Box<V>, bool, f64),
}

struct P {
    t: Vec<Tok>,
    i: usize,
}

impl P {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }
    fn seq(&mut self) -> Result<V, String> {
        let mut items = vec![self.item()?];
        while self.peek() == Some(&Tok::And) {
            self.i += 1;
            items.push(self.item()?);
        }
        Ok(if items.len() == 1 {
            items.pop().unwrap()
        } else {
            V::Chain(items)
        })
    }
    fn item(&mut self) -> Result<V, String> {
        let cond = self.prim()?;
        if self.peek() != Some(&Tok::Question) {
            return Ok(cond);
        }
        self.i += 1;
        let then = self.seq()?;
        let otherwise = if self.peek() == Some(&Tok::Colon) {
            self.i += 1;
            Some(Box::new(self.seq()?))
        } else {
            None
        };
        Ok(V::If(Box::new(cond), Box::new(then), otherwise))
    }
    fn prim(&mut self) -> Result<V, String> {
        match self.peek().cloned() {
            Some(Tok::Open) => {
                self.i += 1;
                let inner = self.seq()?;
                if self.peek() == Some(&Tok::Close) {
                    self.i += 1;
                }
                Ok(inner)
            }
            // `browsed_song rating 5`: the track picked in the browser,
            // which is where DJSugarScript's rating acts anyway.
            Some(Tok::Word(w)) if w == "browsed_song" || w == "browsed_file" => {
                self.i += 1;
                self.prim()
            }
            Some(Tok::Word(w)) if w == "deck" => {
                self.i += 1;
                let sel = match self.peek().cloned() {
                    Some(Tok::Num { v, .. }) => Sel::Number(v as u8),
                    Some(Tok::Word(w)) => match w.as_str() {
                        "left" => Sel::Left,
                        "right" => Sel::Right,
                        "default" => Sel::Default,
                        "all" => Sel::All,
                        "master" | "active" => Sel::Master,
                        _ => return Err(format!("deck {w}")),
                    },
                    _ => return Err("deck needs a number or a side".into()),
                };
                self.i += 1;
                Ok(V::Deck(sel, Box::new(self.prim()?)))
            }
            Some(Tok::Word(w)) => {
                self.i += 1;
                let mut args = Vec::new();
                loop {
                    match self.peek().cloned() {
                        Some(Tok::Num { v, unit, signed }) => {
                            args.push(Arg::Num { v, unit, signed })
                        }
                        Some(Tok::Text(t)) => args.push(Arg::Text(t)),
                        Some(Tok::Word(a))
                            if ["on", "off", "toggle", "in", "out", "left", "right"]
                                .contains(&a.as_str()) =>
                        {
                            args.push(Arg::Word(a))
                        }
                        _ => break,
                    }
                    self.i += 1;
                }
                let verb = V::Verb(w, args);
                match self.peek() {
                    Some(Tok::Greater) | Some(Tok::Less) => {
                        let above = self.peek() == Some(&Tok::Greater);
                        self.i += 1;
                        let Some(Tok::Num { v, unit, .. }) = self.peek().cloned() else {
                            return Err("a comparison needs a number".into());
                        };
                        self.i += 1;
                        let level = if unit == "%" { v / 100.0 } else { v };
                        Ok(V::Compare(Box::new(verb), above, level))
                    }
                    _ => Ok(verb),
                }
            }
            other => Err(format!("unexpected {other:?}")),
        }
    }
}

fn parse(src: &str) -> Result<V, String> {
    let t = lex(src)?;
    let mut p = P { t, i: 0 };
    let v = p.seq()?;
    if p.i < p.t.len() {
        // Stray closing brackets, and a last `: …` with no `?` for it, are
        // common in hand-written mappers; VirtualDJ ignores them.
        if p.t[p.i..].iter().all(|t| *t == Tok::Close) || p.t[p.i] == Tok::Colon {
            return Ok(v);
        }
        return Err(format!("couldn't read past {:?}", p.t[p.i]));
    }
    Ok(v)
}

// -------------------------------------------------------- translation

#[derive(Clone, Copy, Debug, PartialEq)]
enum Side {
    Left,
    Right,
    None,
}

/// Where a mapped control is, for translating its action.
#[derive(Clone, Copy, Debug)]
struct Ctx {
    deck: u8,
    side: Side,
    /// FLX4 pad pages: what `pad 3` means here.
    page: Page,
    /// The deck count VirtualDJ's left and right decks resolve against.
    light: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Page {
    None,
    HotCue,
    BeatLoop,
    BeatJump,
    Sampler,
    Other,
}

/// What a line becomes.
#[derive(Debug, PartialEq)]
enum Out {
    Bind(String, Vec<&'static str>),
    Script(String),
}

type R = Result<String, String>;

fn no(why: &str) -> R {
    Err(why.to_string())
}

fn num(args: &[Arg], i: usize) -> Option<f64> {
    match args.get(i) {
        Some(Arg::Num { v, .. }) => Some(*v),
        _ => None,
    }
}

fn text(args: &[Arg], i: usize) -> Option<&str> {
    match args.get(i) {
        Some(Arg::Text(t)) => Some(t),
        Some(Arg::Word(t)) => Some(t),
        _ => None,
    }
}

fn level(args: &[Arg]) -> Option<String> {
    match args.first() {
        Some(Arg::Num { v, unit: "%", .. }) => Some(format!("{}%", trim(*v))),
        _ => None,
    }
}

fn trim(v: f64) -> String {
    super::words::beats(v)
}

/// `'@$k2mode'` → `k2mode`.
fn var_name(t: &str) -> String {
    t.trim_start_matches('@')
        .trim_start_matches('$')
        .to_string()
}

fn deck_number(sel: &Sel, ctx: Ctx) -> Option<u8> {
    match sel {
        Sel::Number(n) => Some(*n),
        Sel::Left => Some(1),
        Sel::Right => Some(2),
        Sel::Default => Some(ctx.deck),
        Sel::All | Sel::Master => None,
    }
}

/// The Mixxx temporary bend for VirtualDJ's `pitch_bend +2%`: small up to
/// 3%, the 4% one beyond.
fn bend(percent: f64) -> &'static str {
    match (percent > 0.0, percent.abs() < 3.0) {
        (true, true) => "rate_temp_up_small",
        (true, false) => "rate_temp_up",
        (false, true) => "rate_temp_down_small",
        (false, false) => "rate_temp_down",
    }
}

fn ch(d: u8, item: &str) -> String {
    format!("[Channel{d}],{item}")
}

fn fx(d: u8, slot: u8, item: &str) -> String {
    format!("[EffectRack1_EffectUnit{d}_Effect{slot}],{item}")
}

/// A verb run as an action.
fn action(name: &str, args: &[Arg], ctx: Ctx) -> R {
    let d = ctx.deck;
    let press = |c: String| Ok(format!("press '{c}'"));
    let toggle = |c: String| Ok(format!("toggle '{c}'"));
    let slot = || num(args, 0).unwrap_or(1.0) as u8;
    let s = match name {
        "play_pause" | "pioneer_play" | "play_button" => "play_pause".into(),
        "cue_button" => return press(ch(d, "cue_default")),
        "stop_button" => "pause".into(),
        "loop_button" => "loop 4".into(),
        "rating" => match num(args, 0) {
            Some(n) => format!("rating {}", n as u8),
            None => return no("a rating without stars"),
        },
        "play" => "play".into(),
        "pause" | "stop" => "pause".into(),
        "cue_stop" | "pioneer_cue" | "cue" => return press(ch(d, "cue_default")),
        "goto_start" => "goto_start".into(),
        "play_stutter" => return press(ch(d, "cue_gotoandplay")),
        "sync" => "sync".into(),
        "masterdeck" | "master_deck" => "master_deck".into(),
        "pitch_lock" | "key_lock" | "keylock" => "key_lock".into(),
        "pitch_reset" | "pitch_zero" => "pitch_zero".into(),
        "slip" | "slip_mode" => "slip".into(),
        "quantize" => "quantize".into(),
        "loop" => match (num(args, 0), text(args, 0)) {
            (Some(n), _) => format!("loop {}", trim(n)),
            (_, Some("off")) => "loop_exit".into(),
            _ => "loop 4".into(),
        },
        "loop_exit" => "loop_exit".into(),
        "reloop_exit" | "reloop" => "reloop".into(),
        "loop_half" => "loop_half".into(),
        "loop_double" => "loop_double".into(),
        "loop_in" | "pioneer_loop_in" => "loop_in".into(),
        "loop_out" | "pioneer_loop_out" => "loop_out".into(),
        "hot_cue" | "hotcue" => match num(args, 0) {
            Some(n) => format!("hot_cue {}", n as u8),
            None => return no("a hot cue without a number"),
        },
        "delete_cue" => match num(args, 0) {
            Some(n) => return press(ch(d, &format!("hotcue_{}_clear", n as u8))),
            None => return no("deleting the selected cue isn't mappable yet"),
        },
        "load" => "load".into(),
        "beatjump" => match num(args, 0) {
            Some(n) if n < 0.0 => return press(ch(d, &format!("beatjump_{}_backward", trim(-n)))),
            Some(n) => return press(ch(d, &format!("beatjump_{}_forward", trim(n)))),
            None => return press(ch(d, "beatjump_forward")),
        },
        "eq_kill_high" => return toggle(super::words::eq_kill(d, 3)),
        "eq_kill_mid" => return toggle(super::words::eq_kill(d, 2)),
        "eq_kill_low" => return toggle(super::words::eq_kill(d, 1)),
        "effect_active" => return toggle(fx(d, slot(), "enabled")),
        "effect_select" => {
            let back = args
                .iter()
                .any(|a| matches!(a, Arg::Num { v, signed: true, .. } if *v < 0.0));
            let s = args
                .iter()
                .find_map(|a| match a {
                    Arg::Num {
                        v, signed: false, ..
                    } => Some(*v as u8),
                    _ => None,
                })
                .unwrap_or(1);
            return press(fx(d, s, if back { "prev_effect" } else { "next_effect" }));
        }
        "filter" => match level(args) {
            Some(l) => format!("filter {l}"),
            None => "filter".into(),
        },
        "set_gain" => return Ok(format!("set '{}' 1", ch(d, "pregain"))),
        "pitch" if level(args).as_deref() == Some("100%") => "pitch_zero".into(),
        "gain" | "eq_high" | "eq_mid" | "eq_low" | "pitch" | "level" | "volume" => {
            let verb = if name == "volume" { "level" } else { name };
            match (args.is_empty(), level(args)) {
                (true, _) => verb.into(),
                (_, Some(l)) => format!("{verb} {l}"),
                _ => return no(&format!("'{name}' with that value")),
            }
        }
        "pfl" => "pfl".into(),
        "effect_slider" => {
            let p = num(args, 1).unwrap_or(1.0) as u8;
            return Ok(format!(
                "adjust '{}'",
                fx(d, slot(), &format!("parameter{p}"))
            ));
        }
        "master_volume" | "headphone_volume" | "booth_volume" | "crossfader" => {
            let c = match name {
                "master_volume" => "[Master],gain",
                "headphone_volume" => "[Master],headGain",
                "booth_volume" => "[Master],booth_gain",
                _ => "[Master],crossfader",
            };
            return Ok(match level(args) {
                Some(l) => format!("adjust '{c}' {l}"),
                None => format!("adjust '{c}'"),
            });
        }
        "blink" if ctx.light => format!("blink {}", num(args, 0).unwrap_or(500.0) as u32),
        "var" | "get_var" => return no("asking a variable only works in a condition"),
        "toggle" => match text(args, 0) {
            Some(v) => format!("toggle_var '{}'", var_name(v)),
            None => return no("toggle needs a variable"),
        },
        "set" => match (text(args, 0), num(args, 1)) {
            (Some(v), Some(n)) => format!("set_var '{}' {}", var_name(v), trim(n)),
            (Some(v), None) => format!("set_var '{}' 1", var_name(v)),
            _ => return no("set needs a variable"),
        },
        "browser_enter" => return press("[Library],GoToItem".into()),
        "browser_folder" => match num(args, 0) {
            Some(n) if n < 0.0 => return press("[Playlist],SelectPrevPlaylist".into()),
            Some(_) => return press("[Playlist],SelectNextPlaylist".into()),
            None => return press("[Playlist],ToggleSelectedSidebarItem".into()),
        },
        "browser_window" if args.is_empty() => return press("[Library],MoveFocusForward".into()),
        "browser_zoom" => "expand_browser".into(),
        // Held: the bend lasts until the release row lets go (see
        // `translate`). DJ Sugar bends by Mixxx's 2% or 4% at once, without
        // VirtualDJ's easing in.
        "pitch_bend" => match num(args, 0) {
            Some(n) if n != 0.0 => return Ok(format!("set '{}' 1", ch(d, bend(n)))),
            _ => return no("a pitch bend without an amount"),
        },
        "loop_move" => format!("loop_move {}", trim(num(args, 0).unwrap_or(1.0))),
        "song_pos" => match level(args) {
            Some(l) => format!("song_pos {l}"),
            None => return no("song_pos without a position, on a button"),
        },
        "pad" | "padshift" => {
            return pad(name == "padshift", num(args, 0).unwrap_or(1.0) as u8, ctx)
        }
        "color" if ctx.light => match text(args, 0) {
            Some("black") => "off".into(),
            Some(c) => format!("color '{c}'"),
            None => "on".into(),
        },
        "on" | "off" if ctx.light => name.into(),
        "constant" if ctx.light => format!(
            "led {}",
            (num(args, 0).unwrap_or(0.0) * 127.0).round() as u8
        ),
        "nothing" => "set_var '_' 0".into(),
        "true" | "false" => return no("a mode switch for VirtualDJ's own definitions"),
        "touchwheel" | "touchwheel_touch" | "jog" => {
            return no("jog wheels are left to DJ Sugar's mapping for this controller")
        }
        "pad_page" | "padpage" => {
            return no("DJ Sugar's pad pages aren't switched from a mapping yet")
        }
        _ => return no(&format!("DJ Sugar has no '{name}' yet")),
    };
    Ok(s)
}

/// FLX4 pads: what VirtualDJ's default pad pages do.
fn pad(shift: bool, n: u8, ctx: Ctx) -> R {
    let d = ctx.deck;
    match (ctx.page, shift) {
        // Keys and controllers without pad pages: VirtualDJ's first page
        // is hot cues.
        (Page::HotCue | Page::None, false) => Ok(format!("hot_cue {n}")),
        (Page::HotCue | Page::None, true) => {
            Ok(format!("press '{}'", ch(d, &format!("hotcue_{n}_clear"))))
        }
        (Page::BeatLoop, false) => {
            let size = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0][(n.clamp(1, 8) - 1) as usize];
            Ok(format!("loop {}", trim(size)))
        }
        (Page::BeatJump, false) => {
            let size = 2f64.powi(((n.clamp(1, 8) - 1) / 2) as i32);
            let dir = if n % 2 == 1 { "backward" } else { "forward" };
            Ok(format!(
                "press '{}'",
                ch(d, &format!("beatjump_{}_{dir}", trim(size)))
            ))
        }
        (Page::Sampler, false) => {
            let s = n as u16 + if d == 2 { 8 } else { 0 };
            Ok(format!("press '[Sampler{s}],cue_gotoandplay'"))
        }
        _ => no("this VirtualDJ pad page has no DJ Sugar match yet"),
    }
}

/// A verb asked in a condition.
fn query(name: &str, args: &[Arg], ctx: Ctx) -> R {
    let d = ctx.deck;
    let get = |c: String| Ok(format!("get '{c}'"));
    let s = match name {
        "play" | "is_playing" | "pioneer_play" => "play".into(),
        "loop" => "loop".into(),
        "hot_cue" | "hotcue" => match num(args, 0) {
            Some(n) => format!("hot_cue {}", n as u8),
            None => return no("a hot cue without a number"),
        },
        "loaded" | "load_pulse" => return get(ch(d, "track_loaded")),
        "is_sync" | "sync" => return get(ch(d, "sync_enabled")),
        "masterdeck" | "master_deck" => return get(ch(d, "sync_leader")),
        "pitch_lock" | "key_lock" => return get(ch(d, "keylock")),
        "pfl" => "pfl".into(),
        "effect_active" => return get(fx(d, num(args, 0).unwrap_or(1.0) as u8, "enabled")),
        "eq_kill_high" => return get(super::words::eq_kill(d, 3)),
        "eq_kill_mid" => return get(super::words::eq_kill(d, 2)),
        "eq_kill_low" => return get(super::words::eq_kill(d, 1)),
        "pioneer_loop_in" | "pioneer_loop_out" | "reloop_exit" | "loop_in" | "loop_out" => {
            return get(ch(d, "loop_enabled"))
        }
        "shift" => "var 'shift'".into(),
        "var" | "get_var" => match (text(args, 0), num(args, 1)) {
            (Some(v), Some(n)) => format!("var '{}' {}", var_name(v), trim(n)),
            (Some(v), None) => format!("var '{}'", var_name(v)),
            _ => return no("var needs a name"),
        },
        "filter" => match level(args) {
            // VirtualDJ asks whether the filter is at that level.
            Some(_) => {
                let v = num(args, 0).unwrap_or(50.0);
                format!(
                    "(filter > {}% & filter < {}%)",
                    trim(v - 1.0),
                    trim(v + 1.0)
                )
            }
            None => "filter > 50%".into(),
        },
        "true" => "var '_always' 0".into(),
        // Mixxx's focused_widget: 2 the folders, 3 the track list.
        "browser_window" => match text(args, 0) {
            Some("folders") => "get '[Library],focused_widget' < 2.5".into(),
            Some("songs") => "get '[Library],focused_widget' > 2.5".into(),
            _ => return no("asking about that browser window"),
        },
        _ => return no(&format!("DJ Sugar can't ask '{name}' yet")),
    };
    Ok(s)
}

/// A single verb a fader, knob or encoder drives: a direct binding.
fn fader(name: &str, args: &[Arg], d: u8, encoder: bool) -> Option<Out> {
    let b = |c: String| {
        Some(Out::Bind(
            c,
            if encoder { vec!["selectknob"] } else { vec![] },
        ))
    };
    if !args.is_empty() && !matches!(name, "effect_slider") {
        return None;
    }
    match name {
        "level" | "volume" => b(ch(d, "volume")),
        "gain" => b(ch(d, "pregain")),
        "eq_high" => b(super::words::eq(d, 3)),
        "eq_mid" => b(super::words::eq(d, 2)),
        "eq_low" => b(super::words::eq(d, 1)),
        "filter" => b(super::words::filter(d)),
        "crossfader" => b("[Master],crossfader".into()),
        "master_volume" => b("[Master],gain".into()),
        "headphone_volume" => b("[Master],headGain".into()),
        "headphone_mix" => b("[Master],headMix".into()),
        "booth_volume" => b("[Master],booth_gain".into()),
        // Each click: the loop moves by its length, the track by a beat
        // (the DJ can change the size in the line).
        "loop_move" if encoder => Some(Out::Script(format!("deck {d} loop_move 1"))),
        "song_pos" if encoder => Some(Out::Script(format!("deck {d} song_pos"))),
        "browser_folder" if encoder => b("[Playlist],SelectPlaylist".into()),
        "browser_window" if encoder => b("[Library],MoveFocus".into()),
        "browser_scroll" => Some(Out::Bind(
            "[Library],MoveVertical".into(),
            vec!["selectknob"],
        )),
        "effect_slider" => {
            let slot = num(args, 0).unwrap_or(1.0) as u8;
            let p = num(args, 1).unwrap_or(1.0) as u8;
            b(fx(d, slot, &format!("parameter{p}")))
        }
        _ => None,
    }
}

/// Settles what a line asks about the control itself: its side, the deck
/// it acts on. `None` when it isn't one of those.
fn fixed(v: &V, ctx: Ctx) -> Option<bool> {
    match v {
        V::Verb(n, a) if n == "device_side" => Some(match (text(a, 0), ctx.side) {
            (Some("left"), Side::Left) | (Some("right"), Side::Right) => true,
            (Some(_), Side::None) => ctx.deck % 2 == 1,
            _ => false,
        }),
        V::Verb(n, a) if n == "action_deck" => Some(num(a, 0).map(|x| x as u8) == Some(ctx.deck)),
        V::Verb(n, a) if n == "leftdeck" || n == "rightdeck" => {
            let _ = a;
            None
        }
        V::Verb(n, _) if n == "doubleclick" => Some(false),
        // VirtualDJ asks whether its effect chooser is open; DJ Sugar's isn't
        // opened from a controller.
        V::Verb(n, _) if n == "effect_select" => Some(false),
        V::Deck(_, inner) if matches!(inner.as_ref(), V::Verb(n, _) if n == "effect_select") => {
            Some(false)
        }
        V::Verb(n, a) if n == "get_decks" => Some(num(a, 0) == Some(2.0)),
        _ => None,
    }
}

fn cond(v: &V, ctx: Ctx) -> R {
    match v {
        V::Verb(n, a) => query(n, a, ctx),
        V::Deck(sel, inner) => match deck_number(sel, ctx) {
            Some(d) => Ok(format!("deck {d} {}", cond(inner, Ctx { deck: d, ..ctx })?)),
            None if *sel == Sel::Master => Ok(format!("deck master {}", cond(inner, ctx)?)),
            None => no("asking all decks at once"),
        },
        V::Compare(inner, above, level) => {
            let (deck, verb) = match inner.as_ref() {
                V::Verb(n, _) => (ctx.deck, n.as_str()),
                _ => return no("only one verb can be compared"),
            };
            let what = match verb {
                "get_vu_meter" | "get_level" | "vu_meter" => "vu_meter",
                "filter" => "filter",
                "level" | "volume" => "level",
                "gain" => "gain",
                "pitch" => "pitch",
                v => return no(&format!("DJ Sugar can't compare '{v}' yet")),
            };
            Ok(format!(
                "deck {deck} {what} {} {}%",
                if *above { ">" } else { "<" },
                trim(level * 100.0)
            ))
        }
        V::Chain(items) => {
            let parts: Result<Vec<String>, String> = items.iter().map(|i| cond(i, ctx)).collect();
            Ok(format!("({})", parts?.join(" & ")))
        }
        V::If(..) => no("a condition inside a condition"),
    }
}

/// A line as an action (or, in a light, what it shows).
fn act(v: &V, ctx: Ctx) -> R {
    match v {
        V::Verb(n, a) if ctx.light && is_shown(n) => action(n, a, ctx),
        // A light that shows a level: an encoder's ring, a meter.
        V::Verb(n, a) if ctx.light && shows_level(n, a) => match n.as_str() {
            "get_vu_meter" | "get_level" => Ok("vu_meter".into()),
            _ => action(
                n,
                a,
                Ctx {
                    light: false,
                    ..ctx
                },
            ),
        },
        // A light that only asks: VirtualDJ lights it when the answer is yes.
        V::Verb(..) | V::Deck(..) | V::Compare(..) if ctx.light => {
            Ok(format!("({} ? on : off)", cond(v, ctx)?))
        }
        V::Verb(n, a) => action(n, a, ctx),
        V::Deck(sel, inner) => match deck_number(sel, ctx) {
            Some(d) => Ok(format!("deck {d} {}", act(inner, Ctx { deck: d, ..ctx })?)),
            None if *sel == Sel::All => {
                let a = act(inner, Ctx { deck: 1, ..ctx })?;
                let b = act(inner, Ctx { deck: 2, ..ctx })?;
                Ok(format!("deck 1 {a} & deck 2 {b}"))
            }
            None => Ok(format!("deck master {}", act(inner, ctx)?)),
        },
        V::Chain(items) => {
            let parts: Result<Vec<String>, String> = items.iter().map(|i| act(i, ctx)).collect();
            Ok(parts?.join(" & "))
        }
        V::Compare(..) => no("a comparison on its own does nothing"),
        V::If(c, then, otherwise) => {
            if let Some(yes) = fixed(c, ctx) {
                return match (yes, otherwise) {
                    (true, _) => act(then, ctx),
                    (false, Some(o)) => act(o, ctx),
                    (false, None) => Ok(if ctx.light {
                        "off".into()
                    } else {
                        "set_var '_' 0".into()
                    }),
                };
            }
            // `blink 500ms ? color 'blue' : off`: blinking.
            if let V::Verb(n, a) = c.as_ref() {
                if n == "blink" && ctx.light {
                    let ms = num(a, 0).unwrap_or(500.0);
                    return Ok(format!("blink {}", ms as u32));
                }
                if n == "holding" {
                    return no("holding inside a line");
                }
                if n == "param_add" || n == "param_bigger" || n == "param_greater" {
                    let t = act(then, ctx)?;
                    let o = match otherwise {
                        Some(o) => act(o, ctx)?,
                        None => "set_var '_' 0".into(),
                    };
                    return Ok(format!("(value < 50% ? {t} : {o})"));
                }
            }
            let c = cond(c, ctx)?;
            let t = act(then, ctx)?;
            let o = match otherwise {
                Some(o) => act(o, ctx)?,
                None if ctx.light => "off".into(),
                None => "set_var '_' 0".into(),
            };
            Ok(format!("({c} ? {t} : {o})"))
        }
    }
}

/// Verbs whose level a light can show.
fn shows_level(n: &str, a: &[Arg]) -> bool {
    match n {
        "effect_slider" | "get_vu_meter" | "get_level" | "master_volume" | "headphone_volume" => {
            true
        }
        "filter" | "gain" | "level" | "volume" | "eq_high" | "eq_mid" | "eq_low" => a.is_empty(),
        _ => false,
    }
}

fn is_shown(n: &str) -> bool {
    matches!(n, "color" | "on" | "off" | "constant" | "blink")
}

/// One VirtualDJ line on one control: what it does, or why it can't.
fn translate(src: &str, ctx: Ctx, encoder: bool) -> Vec<(When, Result<Out, String>)> {
    let v = match parse(src) {
        Ok(v) => v,
        Err(e) => {
            return vec![(
                When::Press,
                Err(format!("DJ Sugar couldn't read this: {e}")),
            )]
        }
    };
    // `holding ? a : b`: a long press and a tap.
    if let V::If(c, then, otherwise) = &v {
        if matches!(c.as_ref(), V::Verb(n, _) if n == "holding") {
            let mut out = vec![(When::Long, one(then, ctx, encoder))];
            if let Some(o) = otherwise {
                out.push((When::Tap, one(o, ctx, encoder)));
            }
            return out;
        }
    }
    let mut out = vec![(When::Press, one(&v, ctx, encoder))];
    if !encoder && !ctx.light && src.contains("pitch_bend") {
        let line = [
            "rate_temp_up",
            "rate_temp_down",
            "rate_temp_up_small",
            "rate_temp_down_small",
        ]
        .map(|k| format!("set '{}' 0", ch(ctx.deck, k)))
        .join(" & ");
        out.push((When::Release, Ok(Out::Script(line))));
    }
    out
}

fn one(v: &V, ctx: Ctx, encoder: bool) -> Result<Out, String> {
    if matches!(v, V::Verb(n, a) if n == "pitch" && a.is_empty()) {
        return Err("left to DJ Sugar's own mapping for this controller, which knows which way its tempo fader runs".into());
    }
    if !ctx.light {
        let (deck, inner) = match v {
            V::Deck(sel, inner) => match deck_number(sel, ctx) {
                Some(d) => (d, inner.as_ref()),
                None => (ctx.deck, v),
            },
            _ => (ctx.deck, v),
        };
        if let V::Verb(n, a) = inner {
            if let Some(b) = fader(n, a, deck, encoder) {
                return Ok(b);
            }
        }
    }
    let s = act(v, ctx)?;
    // What it wrote has to read back as DJSugarScript.
    djsugar_script::parse(&s)
        .map_err(|e| format!("DJ Sugar couldn't run its translation '{s}': {}", e.message))?;
    Ok(Out::Script(s))
}

// ------------------------------------------------------- controllers

/// Where one of VirtualDJ's names is on a controller: the message (and
/// the message the controller itself sends with SHIFT held, if any), and
/// the deck and side.
#[derive(Clone, Debug)]
struct Place {
    midi: Midi,
    shifted: Option<Midi>,
    deck: u8,
    side: Side,
    /// Variables the row needs, from the controller's modes.
    layer: &'static str,
    page: Page,
    encoder: bool,
    /// For the light: its message when it differs.
    light: Option<Midi>,
    /// For the light: colours when they differ from the controller's.
    colors: Option<&'static [(&'static str, u8)]>,
    /// The deck is the column's, so the mapping's deck order picks it.
    by_position: bool,
}

struct Device {
    name: &'static str,
    layout: Layout,
    colors: Vec<(String, u8)>,
    color_notes: bool,
    places: fn(&Device, &str) -> Vec<Place>,
    variables: &'static [(&'static str, bool)],
    /// A controller with two-deck and four-deck layouts: the use it starts
    /// in and its four-deck order.
    decks: Option<(u8, [u8; 4])>,
    note: &'static str,
}

impl Device {
    fn part(&self, id: &str, layer: &str) -> Option<Midi> {
        let part = self.layout.parts.iter().find(|p| p.id == id)?;
        let channel = self
            .layout
            .channel
            .map(|c| c.saturating_sub(1))
            .unwrap_or(0);
        let msgs: Vec<_> = part.midi.iter().filter(|m| m.layer == layer).collect();
        let status = |m: &crate::layout::PartMessage| {
            if m.any_channel {
                m.status | channel
            } else {
                m.status
            }
        };
        let first = msgs.first()?;
        let mut midi = Midi::new(status(first), first.number);
        // Two plain CCs: a 14-bit fader.
        if let Some(second) = msgs.get(1) {
            if first.status & 0xF0 == 0xB0 && second.status == first.status {
                midi.lsb = Some((status(second), second.number));
            }
        }
        Some(midi)
    }

    fn place(&self, id: &str, deck: u8, side: Side) -> Option<Place> {
        Some(Place {
            midi: self.part(id, "")?,
            shifted: self.part(id, "SHIFT"),
            deck,
            side,
            layer: "",
            page: Page::None,
            encoder: self
                .layout
                .parts
                .iter()
                .any(|p| p.id == id && p.kind == "encoder"),
            light: None,
            colors: None,
            by_position: false,
        })
    }
}

fn sides(dev: &Device, left: &str, right: &str) -> Vec<Place> {
    [(left, 1, Side::Left), (right, 2, Side::Right)]
        .into_iter()
        .filter_map(|(id, d, s)| dev.place(id, d, s))
        .collect()
}

fn flx4_places(dev: &Device, name: &str) -> Vec<Place> {
    let both = |id: &str| sides(dev, &format!("l-{id}"), &format!("r-{id}"));
    let one = |id: &str, deck: u8| {
        dev.place(id, deck, Side::None)
            .into_iter()
            .collect::<Vec<_>>()
    };
    match name {
        "PLAY_PAUSE" => both("play"),
        "CUE" => both("cue"),
        "SYNC" => both("sync"),
        "SYNC_LONG" => sides(dev, "l-sync", "r-sync")
            .into_iter()
            .filter_map(|mut p| {
                let id = if p.deck == 1 { "l-sync" } else { "r-sync" };
                p.midi = dev.part(id, "Long press")?;
                p.shifted = None;
                Some(p)
            })
            .collect(),
        "LOOP_IN" => both("in"),
        "LOOP_OUT" => both("out"),
        "4BEAT_LOOP" | "RELOOP" => both("exit"),
        "CUE_CALL-" => both("call-l"),
        "CUE_CALL+" => both("call-r"),
        "SHIFT" => both("shift"),
        "PITCH" => both("tempo"),
        "GAIN" => both("trim"),
        "EQ_HIGH" => both("hi"),
        "EQ_MID" => both("mid"),
        "EQ_LOW" => both("low"),
        "VOLUME" => both("fader"),
        "FILTER" => both("cfx"),
        "PFL" => both("pfl"),
        "LOAD" => sides(dev, "load-l", "load-r"),
        "JOG" | "JOG_TOUCH" | "JOG_OUT" => both("jog"),
        "CROSSFADER" => one("xfader", 1),
        "MASTER_VOL" => one("master", 1),
        "HEADPHONES_VOL" => one("phones", 1),
        "HEADPHONES_MIX" => one("mix", 1),
        "BROWSE_ENC" => one("browse", 1),
        "BROWSE_ENC_PUSH" => dev
            .part("browse", "Press")
            .map(|m| Place {
                midi: m,
                shifted: dev.part("browse", "SHIFT press"),
                ..dev.place("browse", 1, Side::None).unwrap()
            })
            .into_iter()
            .collect(),
        "FX_SELECT" => one("fx-select", 1),
        "FX_BEATS-" => one("beat-l", 1),
        "FX_BEATS+" => one("beat-r", 1),
        "FX_LEVEL" => one("fx-level", 1),
        "FX_ON" => one("fx-on", 1),
        "SMART_COLOR" => one("smart-cfx", 1),
        "PAD_MODE_HOTCUE" => both("m-hot"),
        "PAD_MODE_PADFX1" => both("m-fx"),
        "PAD_MODE_BEATJUMP" => both("m-jump"),
        "PAD_MODE_SAMPLER" => both("m-samp"),
        _ => {
            // HOTCUE_PAD3, BEATLOOP_PAD8: a pad on one of the pad pages.
            let Some((page, n)) = name.rsplit_once("_PAD") else {
                return Vec::new();
            };
            let Ok(n) = n.parse::<u8>() else {
                return Vec::new();
            };
            let (label, kind) = match page {
                "HOTCUE" => ("Hot cue", Page::HotCue),
                "PADFX1" => ("Pad FX 1", Page::Other),
                "PADFX2" => ("Pad FX 2", Page::Other),
                "BEATJUMP" => ("Beat jump", Page::BeatJump),
                "SAMPLER" => ("Sampler", Page::Sampler),
                "KEYBOARD" => ("Keyboard", Page::Other),
                "BEATLOOP" => ("Beat loop", Page::BeatLoop),
                "KEYSHIFT" => ("Key shift", Page::Other),
                _ => return Vec::new(),
            };
            [("l", 1, Side::Left), ("r", 2, Side::Right)]
                .into_iter()
                .filter_map(|(s, d, side)| {
                    let id = format!("{s}-pad{n}");
                    Some(Place {
                        midi: dev.part(&id, label)?,
                        shifted: dev.part(&id, &format!("{label} + SHIFT")),
                        page: kind,
                        ..dev.place(&id, d, side).unwrap_or(Place {
                            midi: Midi::default(),
                            shifted: None,
                            deck: d,
                            side,
                            layer: "",
                            page: kind,
                            encoder: false,
                            light: None,
                            colors: None,
                            by_position: false,
                        })
                    })
                })
                .collect()
        }
    }
}

/// Controls an encoder's clicks drive as they are, by Mixxx's selectknob:
/// the library's moves, and other controls that take steps.
fn is_relative_control(control: &str) -> bool {
    control.starts_with("[Library]")
        || control.starts_with("[Playlist]")
        || control.ends_with(",chain_selector")
        || control.ends_with(",effect_selector")
        || control.ends_with(",jog")
}

/// Xone K2: VirtualDJ names its columns per side. With two decks
/// (`K22D_`) a side has an outer and an inner column; with four (`K2_`)
/// each column is a deck: 3, 1, 2, 4 from the left.
fn k2_places(dev: &Device, name: &str) -> Vec<Place> {
    let (family, rest) = if let Some(r) = name.strip_prefix("K22D_") {
        ("2", r)
    } else if let Some(r) = name.strip_prefix("K2_") {
        ("4", r)
    } else if let Some(r) = name.strip_prefix("2XK2_") {
        ("x", r)
    } else {
        // LED_BUTTON_ABCD_OUTER: the pad lights, named without a family.
        let two = name.ends_with("_INNER") || name.ends_with("_OUTER");
        (if two { "2" } else { "4" }, name)
    };
    // Columns 1 to 4 with their deck and side, for this name.
    let columns: Vec<(u8, u8, Side)> = match family {
        "2" if rest.ends_with("_OUTER") => vec![(1, 1, Side::Left), (4, 2, Side::Right)],
        "2" if rest.ends_with("_INNER") => vec![(2, 1, Side::Left), (3, 2, Side::Right)],
        // Columns 1 to 4; the mapping's deck order (3 1 2 4 to start, as
        // VirtualDJ and DJ Sugar's 4-Deck screen) says which deck each is.
        "4" => vec![
            (1, 1, Side::Left),
            (2, 2, Side::Left),
            (3, 3, Side::Right),
            (4, 4, Side::Right),
        ],
        _ => vec![(1, 1, Side::Left)],
    };
    let layer = match family {
        "2" => "decks=2",
        "4" => "decks=4",
        _ => "2xk2",
    };
    let part = rest.trim_end_matches("_OUTER").trim_end_matches("_INNER");
    let at = |id: String, label: &str, deck: u8, side: Side| -> Option<Place> {
        let midi = dev.part(&id, label)?;
        Some(Place {
            midi,
            shifted: None,
            deck,
            side,
            layer,
            page: Page::None,
            encoder: label.is_empty() && id.starts_with("enc"),
            light: None,
            colors: None,
            by_position: false,
        })
    };
    let each = |f: &dyn Fn(u8) -> (String, &'static str)| -> Vec<Place> {
        columns
            .iter()
            .filter_map(|(c, d, s)| {
                let (id, label) = f(*c);
                let mut p = at(id, label, *d, *s)?;
                p.by_position = family == "4";
                Some(p)
            })
            .collect()
    };
    match part {
        "ENCODER" => each(&|c| (format!("enc{c}"), "")),
        "ENCODER_PUSH" => each(&|c| (format!("enc{c}"), "Press")),
        "FADER" => each(&|c| (format!("fader{c}"), "")),
        // The layer and exit buttons light red, amber and green on notes
        // 4 apart.
        "MODE" | "SHIFT" => at(
            if part == "MODE" { "layer" } else { "exit" }.into(),
            "",
            1,
            Side::None,
        )
        .map(|mut p| {
            p.colors = Some(&[
                ("red", 0),
                ("orange", 4),
                ("amber", 4),
                ("yellow", 4),
                ("green", 8),
            ]);
            if part == "SHIFT" {
                p.layer = "";
            }
            p
        })
        .into_iter()
        .collect(),
        "ENC_BTM_L" => at("enc-bl".into(), "", 1, Side::Left).into_iter().collect(),
        "ENC_BTM_R" => at("enc-br".into(), "", 2, Side::Right)
            .into_iter()
            .collect(),
        "ENC_BTM_L_PUSH" => at("enc-bl".into(), "Press", 1, Side::Left)
            .into_iter()
            .collect(),
        "ENC_BTM_R_PUSH" => at("enc-br".into(), "Press", 2, Side::Right)
            .into_iter()
            .collect(),
        _ => {
            if let Some(r) = part.strip_prefix("KNOB_") {
                let Ok(row) = r.parse::<u8>() else {
                    return Vec::new();
                };
                return each(&|c| (format!("knob{row}-{c}"), ""))
                    .into_iter()
                    .map(|mut p| {
                        // The knob's light is the button under it.
                        let col = columns
                            .iter()
                            .find(|(_, d, s)| *d == p.deck && *s == p.side)
                            .map(|x| x.0);
                        p.light = col.and_then(|c| dev.part(&format!("btn{row}-{c}"), ""));
                        p
                    })
                    .collect();
            }
            if let Some(r) = part.strip_prefix("BUTTON_") {
                if let Ok(row) = r.parse::<u8>() {
                    return each(&|c| (format!("btn{row}-{c}"), ""));
                }
                // A grid row (ABCD) at each column, or one grid button (A).
                const ROWS: [&str; 4] = ["ABCD", "EFGH", "IJKL", "MNOP"];
                if let Some(i) = ROWS.iter().position(|x| *x == r) {
                    return each(&|c| {
                        (format!("pad-{}", &ROWS[i][c as usize - 1..c as usize]), "")
                    });
                }
                if r.len() == 1 {
                    let ch = r.chars().next().unwrap_or('A');
                    let idx = (ch as u8).wrapping_sub(b'A');
                    if idx < 16 {
                        let col = idx % 4 + 1;
                        let (deck, side) = if col <= 2 {
                            (1, Side::Left)
                        } else {
                            (2, Side::Right)
                        };
                        return at(format!("pad-{r}"), "", deck, side).into_iter().collect();
                    }
                }
            }
            Vec::new()
        }
    }
}

/// Midi Fighter Twister: `MD1_ENC_2_3` is bank 1, row 2, column 3;
/// `MD1_LEFT_BUT_1` a side button. Left half is the left deck.
fn twister_places(dev: &Device, name: &str) -> Vec<Place> {
    let Some(rest) = name.strip_prefix("MD") else {
        return Vec::new();
    };
    let Some((bank, rest)) = rest.split_once('_') else {
        return Vec::new();
    };
    let Ok(bank) = bank.parse::<u8>() else {
        return Vec::new();
    };
    let label = |press: bool| {
        if press {
            format!("Bank {bank} press")
        } else {
            format!("Bank {bank}")
        }
    };
    // The picture shows banks 1 to 3; bank 4 follows the same pattern.
    let computed = |id: &str, layer: &str| -> Option<Midi> {
        let press = layer.ends_with("press");
        if let Some(n) = id.strip_prefix("enc") {
            let n: u8 = n.parse().ok()?;
            return Some(Midi::new(
                if press { 0xB1 } else { 0xB0 },
                (bank - 1) * 16 + n - 1,
            ));
        }
        let (side, n) = id.split_at(1);
        let n: u8 = n.parse().ok()?;
        let base = if side == "l" { 0x08 } else { 0x0B };
        Some(Midi::new(0xB3, base + (bank - 1) * 6 + n - 1))
    };
    let mk = |id: String, layer: String, deck: u8, side: Side, encoder: bool| -> Option<Place> {
        Some(Place {
            midi: dev.part(&id, &layer).or_else(|| computed(&id, &layer))?,
            shifted: None,
            deck,
            side,
            layer: "",
            page: Page::None,
            encoder,
            light: None,
            colors: None,
            by_position: false,
        })
    };
    let grid = |press: bool, r: &str| -> Option<Place> {
        let (row, col) = r.split_once('_')?;
        let (row, col): (u8, u8) = (row.parse().ok()?, col.parse().ok()?);
        let n = (row - 1) * 4 + col;
        let (deck, side) = if col <= 2 {
            (1, Side::Left)
        } else {
            (2, Side::Right)
        };
        mk(format!("enc{n}"), label(press), deck, side, false).map(|mut p| {
            // The ring shows on the turn's message; the push's light is
            // the RGB colour, on the press's.
            p.light = Some(p.midi);
            p
        })
    };
    let one = if let Some(r) = rest.strip_prefix("ENC_") {
        grid(false, r)
    } else if let Some(r) = rest.strip_prefix("PUSH_") {
        grid(true, r)
    } else if let Some(n) = rest.strip_prefix("LEFT_BUT_") {
        mk(format!("l{n}"), label(false), 1, Side::Left, false)
    } else if let Some(n) = rest.strip_prefix("RIGHT_BUT_") {
        mk(format!("r{n}"), label(false), 2, Side::Right, false)
    } else {
        None
    };
    one.into_iter().collect()
}

fn device(name: &str) -> Option<Device> {
    let colors = |c: &[(&str, u8)]| c.iter().map(|(n, v)| (n.to_string(), *v)).collect();
    match name.to_ascii_uppercase().as_str() {
        "PIFLX4" | "DDJ-FLX4" | "DDJFLX4" => Some(Device {
            name: "Pioneer DDJ-FLX4",
            layout: Layout::for_file("Pioneer-DDJ-FLX4.midi.xml")?,
            colors: Vec::new(),
            color_notes: false,
            places: flx4_places,
            variables: &[("shift", false)],
            decks: None,
            note: "",
        }),
        "XONEK2" | "XONE:K2" | "K2" => Some(Device {
            name: "Allen & Heath Xone:K2",
            layout: Layout::for_file("Allen and Heath Xone K2.midi.xml")?,
            colors: colors(&[("red", 0), ("orange", 36), ("amber", 36), ("yellow", 36), ("green", 72)]),
            color_notes: true,
            places: k2_places,
            variables: &[("shift", false), ("2xk2", true)],
            decks: Some((2, [3, 1, 2, 4])),
            note: "VirtualDJ's K2 has a two-deck and a four-deck layout: Settings › Controllers picks which one you use, and the deck order for four decks (3 1 2 4 to start). Its rows for two K2s (2XK2) wait on the 2xk2 variable.",
        }),
        "FIGHTERTW" | "MIDIFIGHTERTWISTER" => Some(Device {
            name: "DJ TechTools Midi Fighter Twister",
            layout: Layout::for_file("DJ TechTools MIDI Fighter Twister.midi.xml")?,
            colors: colors(&[
                ("blue", 1),
                ("cyan", 30),
                ("green", 50),
                ("yellow", 66),
                ("orange", 74),
                ("red", 85),
                ("purple", 108),
                ("magenta", 115),
                ("pink", 120),
            ]),
            color_notes: false,
            places: twister_places,
            variables: &[("shift", false)],
            decks: None,
            note: "",
        }),
        _ => None,
    }
}

/// VirtualDJ's names for its own mode switches; nothing to map.
const MODE_KEYS: [&str; 4] = ["2DECKS_MODE", "4DECKS_MODE", "ONINIT", "ONEXIT"];

pub(super) fn import(text: &str, file: &str) -> Result<Import, String> {
    let (device_name, maps) = read(text)?;
    if device_name.eq_ignore_ascii_case("KEYBOARD") {
        return Ok(keyboard(&maps));
    }
    let dev = device(&device_name).ok_or_else(|| {
        format!(
            "DJ Sugar doesn't know where the buttons of VirtualDJ's '{device_name}' are yet. It knows the DDJ-FLX4, Xone:K2 and Midi Fighter Twister."
        )
    })?;
    let title = file
        .strip_suffix(".xml")
        .unwrap_or(file)
        .rsplit(" - ")
        .next()
        .unwrap_or(file)
        .trim()
        .to_string();
    let mut m = DjSugarMapping::new(&format!("{title} (VirtualDJ)"));
    m.controller.name = dev.name.to_string();
    m.controller.layout = Some(dev.layout.file.clone());
    m.based_on = Some(dev.layout.file.clone());
    let mut import = Import::default();
    let mut unknown = Vec::new();
    let mut taken: Vec<(Midi, String, When)> = Vec::new();
    for (value, src) in &maps {
        if MODE_KEYS.contains(&value.as_str()) {
            continue;
        }
        let mut name = value.as_str();
        let mut shift = false;
        let mut led = false;
        loop {
            if let Some(r) = name.strip_prefix("SHIFT_") {
                shift = true;
                name = r;
            } else if let Some(r) = name.strip_prefix("LED_") {
                led = true;
                name = r;
            } else {
                break;
            }
        }
        let mut long = false;
        let mut places = (dev.places)(&dev, name);
        if places.is_empty() {
            if let Some(r) = name.strip_suffix("_LONG") {
                places = (dev.places)(&dev, r);
                long = true;
            }
        }
        if places.is_empty() {
            unknown.push(value.clone());
            continue;
        }
        for p in places {
            if p.midi == Midi::default() {
                continue;
            }
            let ctx = Ctx {
                deck: p.deck,
                side: p.side,
                page: p.page,
                light: led,
            };
            // The controller's own SHIFT message, else DJ Sugar's shift layer.
            let (msg, mut layer) = match (shift, p.shifted) {
                (true, Some(s)) => (s, String::new()),
                (true, None) => (p.midi, "shift".to_string()),
                _ => (p.midi, String::new()),
            };
            if !p.layer.is_empty() {
                layer = if layer.is_empty() {
                    p.layer.to_string()
                } else {
                    format!("{} & {layer}", p.layer)
                };
            }
            if led {
                let msg = if shift { msg } else { p.light.unwrap_or(msg) };
                let mut light = match translate(src, ctx, false).pop() {
                    Some((_, Ok(Out::Script(s)))) => {
                        // A light on a layer shows only while the layer holds.
                        let mut l = Light::new(msg, LightSource::Script(s));
                        l.layer = layer.clone();
                        l
                    }
                    Some((_, Err(why))) => {
                        let mut l = Light::new(msg, LightSource::Nothing);
                        l.off = true;
                        l.note = why;
                        l
                    }
                    _ => continue,
                };
                if light.off {
                    light.original = src.trim().to_string();
                }
                light.name = value.clone();
                light.colors = match p.colors {
                    Some(c) => c.iter().map(|(n, v)| (n.to_string(), *v)).collect(),
                    None => dev.colors.clone(),
                };
                light.color_notes = dev.color_notes;
                light.by_position = p.by_position;
                m.lights.push(light);
                continue;
            }
            if name == "SHIFT" {
                let mut down = Row::script(msg, "set_var 'shift' 1");
                let mut up = Row::script(msg, "set_var 'shift' 0");
                up.when = When::Release;
                for r in [&mut down, &mut up] {
                    r.name = value.clone();
                    r.layer = layer.clone();
                }
                m.rows.push(down);
                m.rows.push(up);
                continue;
            }
            for (when, out) in translate(src, ctx, p.encoder) {
                let when = if long && when == When::Press {
                    When::Long
                } else {
                    when
                };
                let mut row = match out {
                    // An encoder turning a knob control (a filter, a gain)
                    // moves it a step a click; Mixxx's selectknob would set
                    // the control to the click count itself.
                    Ok(Out::Bind(c, _)) if p.encoder && !is_relative_control(&c) => {
                        let mut r = Row::script(msg, &format!("adjust '{c}'"));
                        r.steps = true;
                        r
                    }
                    Ok(Out::Bind(c, o)) => Row::bind(msg, &c, &o),
                    Ok(Out::Script(s)) => {
                        let mut r = Row::script(msg, &s);
                        // On an encoder the line gets the clicks: jumps and
                        // loop moves count them, levels move a step a click.
                        r.steps = p.encoder;
                        r
                    }
                    Err(why) => {
                        let mut r = Row::unsupported(msg, value, src.trim(), &why);
                        r.when = when;
                        r
                    }
                };
                row.name = value.clone();
                row.layer = layer.clone();
                row.by_position = p.by_position;
                row.when = when;
                // Two names on one button: VirtualDJ picks by its own rules;
                // DJ Sugar keeps the first.
                let key = (msg, layer.clone(), when);
                if !row.off && taken.contains(&key) {
                    row.off = true;
                    row.original = src.trim().to_string();
                    row.note = "Another VirtualDJ name already uses this button here".into();
                    row.action = crate::format::Action::Nothing;
                }
                if !row.off {
                    taken.push(key);
                }
                m.rows.push(row);
            }
        }
    }
    for (name, saved) in dev.variables {
        m.variables.push(Variable {
            name: name.to_string(),
            default: 0.0,
            saved: *saved,
        });
    }
    // Variables the lines use.
    let mut seen: Vec<String> = m.variables.iter().map(|v| v.name.clone()).collect();
    let lines: Vec<String> = m
        .rows
        .iter()
        .filter_map(|r| match &r.action {
            crate::format::Action::Script(s) => Some(s.clone()),
            _ => None,
        })
        .chain(m.lights.iter().filter_map(|l| match &l.source {
            LightSource::Script(s) => Some(s.clone()),
            _ => None,
        }))
        .collect();
    for line in lines {
        for part in line.split("var '").skip(1) {
            if let Some((name, _)) = part.split_once('\'') {
                if !seen.iter().any(|s| s == name) && name != "_" && name != "_always" {
                    seen.push(name.to_string());
                    m.variables.push(Variable {
                        name: name.to_string(),
                        default: 0.0,
                        saved: false,
                    });
                }
            }
        }
    }
    import.notes.push(format!(
        "VirtualDJ keeps its controller definitions encrypted, so DJ Sugar placed these names with its own table for the {}. Anything left greyed keeps working as DJ Sugar's own mapping for it does.",
        dev.name
    ));
    if !dev.note.is_empty() {
        import.notes.push(dev.note.to_string());
    }
    if let Some((decks, order)) = dev.decks {
        m.decks = Some(decks);
        m.deck_order = Some(order);
    }
    if !unknown.is_empty() {
        import.notes.push(format!(
            "DJ Sugar doesn't know where these are on the {}: {}.",
            dev.name,
            unknown.join(", ")
        ));
    }
    import.mappings.push(m);
    Ok(import)
}

// ---------------------------------------------------------- keyboard

/// `CMD+SHIFT+D` → `cmd+shift+d`.
fn key(vdj: &str) -> Option<String> {
    let mut mods = Vec::new();
    let mut key = None;
    // `ALT++` and `CMD+,` keep their last part.
    let parts: Vec<&str> = if let Some(r) = vdj.strip_suffix("++") {
        r.split('+').chain(std::iter::once("+")).collect()
    } else {
        vdj.split('+').collect()
    };
    for p in parts {
        let p = p.trim();
        match p.to_ascii_uppercase().as_str() {
            "CMD" | "CTRL" | "COMMAND" => mods.push("cmd"),
            "ALT" | "OPTION" => mods.push("alt"),
            "SHIFT" => mods.push("shift"),
            "" => {}
            k => {
                key = Some(match k {
                    "SPACE" => "space".to_string(),
                    "RETURN" | "ENTER" => "return".to_string(),
                    "BACK" | "BACKSPACE" => "backspace".to_string(),
                    "ESCAPE" | "ESC" => "escape".to_string(),
                    "TAB" => "tab".to_string(),
                    "LEFT" | "RIGHT" | "UP" | "DOWN" => k.to_ascii_lowercase(),
                    "DELETE" | "DEL" => "delete".to_string(),
                    _ => p.to_lowercase(),
                })
            }
        }
    }
    let key = key?;
    let mut out: Vec<&str> = ["cmd", "alt", "shift"]
        .into_iter()
        .filter(|m| mods.contains(m))
        .collect();
    out.push(&key);
    Some(out.join("+"))
}

fn keyboard(maps: &[(String, String)]) -> Import {
    let mut import = Import::default();
    for (value, src) in maps {
        let Some(k) = key(value) else {
            continue;
        };
        if value.eq_ignore_ascii_case("UNMAPPED") || k.contains(' ') {
            continue;
        }
        let ctx = Ctx {
            deck: 1,
            side: Side::None,
            page: Page::None,
            light: false,
        };
        // Keys act on the master deck unless the line names one.
        let v = parse(src);
        let line = v.and_then(|v| {
            let s = act(&strip_default_deck(v), ctx)?;
            djsugar_script::parse(&s).map_err(|e| e.message)?;
            Ok(s)
        });
        match line {
            Ok(s) => import.shortcuts.push((k, s)),
            Err(_) => import.skipped_shortcuts.push((k, src.trim().to_string())),
        }
    }
    import.notes.push(
        "Keyboard shortcuts that DJ Sugar can't do yet are listed in the report and left as they were."
            .into(),
    );
    import
}

/// A keyboard line has no deck of its own: verbs without `deck` act on the
/// master deck, which DJSugarScript does when no deck is written. Lines
/// translated with deck 1 would name it, so `deck default` stays unnamed.
fn strip_default_deck(v: V) -> V {
    match v {
        V::Deck(Sel::Default, inner) => strip_default_deck(*inner),
        V::Deck(s, inner) => V::Deck(s, Box::new(strip_default_deck(*inner))),
        V::Chain(items) => V::Chain(items.into_iter().map(strip_default_deck).collect()),
        V::If(c, t, o) => V::If(
            Box::new(strip_default_deck(*c)),
            Box::new(strip_default_deck(*t)),
            o.map(|o| Box::new(strip_default_deck(*o))),
        ),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::Action;

    fn ctx(deck: u8, side: Side) -> Ctx {
        Ctx {
            deck,
            side,
            page: Page::None,
            light: false,
        }
    }

    fn script(src: &str, c: Ctx) -> String {
        match translate(src, c, false).pop().unwrap().1 {
            Ok(Out::Script(s)) => s,
            other => panic!("{src}: {other:?}"),
        }
    }

    #[test]
    fn reads_vdjscript() {
        let v = parse("var '@$k2mode' ? hot_cue 1 : pitch_bend +2% 500ms").unwrap();
        assert!(matches!(v, V::If(..)));
        let v =
            parse("deck left effect_select ? deck left effect_select : deck left effect_slider 1")
                .unwrap();
        assert!(matches!(v, V::If(..)));
        assert!(parse("toggle '@$k2mode' & toggle '@$PERF'").is_ok());
        assert!(parse("get_vu_meter > 95% ? off : color 'red'").is_ok());
    }

    #[test]
    fn translates_lines() {
        let c = ctx(2, Side::Right);
        assert_eq!(script("play_pause", c), "play_pause");
        assert_eq!(
            script("var '@$k2mode' ? hot_cue 1 : sync", c),
            "(var 'k2mode' ? hot_cue 1 : sync)"
        );
        assert_eq!(
            script("device_side 'left' ? hot_cue 1 : hot_cue 2", c),
            "hot_cue 2"
        );
        assert_eq!(
            script("deck 1 delete_cue 3", c),
            "deck 1 press '[Channel1],hotcue_3_clear'"
        );
        assert_eq!(
            script("beatjump -32", c),
            "press '[Channel2],beatjump_32_backward'"
        );
        assert_eq!(
            translate("eq_high", c, false).pop().unwrap().1,
            Ok(Out::Bind(
                "[EqualizerRack1_[Channel2]_Effect1],parameter3".into(),
                vec![]
            ))
        );
        let held = translate("holding ? browser_folder : deck right load", c, false);
        assert_eq!(held[0].0, When::Long);
        assert_eq!(
            held[0].1,
            Ok(Out::Script(
                "press '[Playlist],ToggleSelectedSidebarItem'".into()
            ))
        );
        // The bend lasts while held: a release row lets go.
        let bend = translate(
            "var '@$k2mode' ? hot_cue 1 : pitch_bend +2% 500ms",
            c,
            false,
        );
        assert_eq!(
            bend[0].1,
            Ok(Out::Script(
                "(var 'k2mode' ? hot_cue 1 : set '[Channel2],rate_temp_up_small' 1)".into()
            ))
        );
        assert_eq!(bend[1].0, When::Release);
        assert_eq!(
            translate("loop_move", c, true)[0].1,
            Ok(Out::Script("deck 2 loop_move 1".into()))
        );
        assert_eq!(
            translate(
                "browser_window 'folders' ? browser_enter : deck right load",
                c,
                false
            )[0]
            .1,
            Ok(Out::Script(
                "(get '[Library],focused_widget' < 2.5 ? press '[Library],GoToItem' : deck 2 load)"
                    .into()
            ))
        );
        assert_eq!(held[1], (When::Tap, Ok(Out::Script("deck 2 load".into()))));
    }

    #[test]
    fn the_k2_vu_meter_lights() {
        let c = Ctx {
            light: true,
            ..ctx(1, Side::Left)
        };
        assert_eq!(
            script("get_vu_meter > 75% ? off : color 'orange'", c),
            "(deck 1 vu_meter > 75% ? off : color 'orange')"
        );
        assert_eq!(
            script("is_sync", c),
            "(get '[Channel1],sync_enabled' ? on : off)"
        );
        assert_eq!(
            script("deck left effect_active ? blink 500ms ? color 'blue' : color 'black' : color 'blue'", c),
            "(deck 1 get '[EffectRack1_EffectUnit1_Effect1],enabled' ? blink 500 : color 'blue')"
        );
    }

    const K2: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<mapper device="XONEK2" author="Atomix Productions" version="850">
	<map value="SHIFT" action="shift" />
	<map value="K22D_FADER_INNER" action="level" />
	<map value="K22D_BUTTON_1_INNER" action="eq_kill_high" />
	<map value="SHIFT_K22D_BUTTON_EFGH_OUTER" action="device_side &apos;left&apos; ?  delete_cue 3 : delete_cue 4" />
	<map value="LED_K22D_KNOB_1_INNER" action="get_vu_meter &gt; 75% ? off : color &apos;orange&apos;" />
	<map value="K2_KNOB_1" action="eq_high" />
	<map value="K22D_ENC_BTM_L_PUSH" action="holding ? browser_folder : browser_window &apos;folders&apos; ? browser_enter : deck left load" />
	<map value="MYSTERY_KNOB" action="eq_high" />
	<map value="2XK2_BUTTON_N" action="deck 1 play_pause" />
</mapper>"#;

    #[test]
    fn reads_a_k2_mapper() {
        let i = import(K2, "Allen & Heath XONE K2 - DJ Nahuel.xml").unwrap();
        let m = &i.mappings[0];
        assert_eq!(m.name, "DJ Nahuel (VirtualDJ)");
        assert_eq!(
            m.based_on.as_deref(),
            Some("Allen and Heath Xone K2.midi.xml")
        );
        // Inner fader: column 2 for deck 1, column 3 for deck 2.
        let faders: Vec<_> = m
            .rows
            .iter()
            .filter(|r| r.name == "K22D_FADER_INNER")
            .collect();
        assert_eq!(faders.len(), 2);
        assert_eq!(faders[0].midi, Midi::new(0xBE, 0x11));
        assert_eq!(faders[0].layer, "decks=2");
        assert!(
            matches!(&faders[1].action, Action::Bind { control, .. } if control == "[Channel2],volume")
        );
        let del: Vec<_> = m
            .rows
            .iter()
            .filter(|r| r.name.starts_with("SHIFT_K22D_BUTTON_EFGH"))
            .collect();
        assert_eq!(del[0].layer, "decks=2 & shift");
        assert_eq!(
            del[0].action,
            Action::Script("press '[Channel1],hotcue_3_clear'".into())
        );
        assert_eq!(
            del[1].action,
            Action::Script("press '[Channel2],hotcue_4_clear'".into())
        );
        // The VU meter shows on the button under knob 1.
        let vu = m
            .lights
            .iter()
            .find(|l| l.name == "LED_K22D_KNOB_1_INNER")
            .unwrap();
        assert_eq!(vu.midi, Midi::new(0x9E, 0x31));
        assert!(vu.color_notes);
        assert!(matches!(&vu.source, LightSource::Script(s) if s.contains("vu_meter > 75%")));
        let k4: Vec<_> = m.rows.iter().filter(|r| r.name == "K2_KNOB_1").collect();
        assert_eq!(k4.len(), 4);
        assert_eq!(k4[0].layer, "decks=4");
        // Columns are positions; the deck order says which deck each is.
        assert!(k4.iter().all(|r| r.by_position));
        assert_eq!((m.decks, m.deck_order), (Some(2), Some([3, 1, 2, 4])));
        let load: Vec<_> = m
            .rows
            .iter()
            .filter(|r| r.name == "K22D_ENC_BTM_L_PUSH")
            .collect();
        assert_eq!(
            load[1].action,
            Action::Script(
                "(get '[Library],focused_widget' < 2.5 ? press '[Library],GoToItem' : deck 1 load)"
                    .into()
            )
        );
        assert!(i.notes.iter().any(|n| n.contains("MYSTERY_KNOB")));
        for r in &m.rows {
            if let Action::Script(s) = &r.action {
                djsugar_script::parse(s).unwrap();
            }
        }
    }

    #[test]
    fn flx4_shift_uses_the_controllers_own_message() {
        let src = r#"<mapper device="PIFLX4"><map value="SHIFT_CUE" action="goto_start" /><map value="HOTCUE_PAD3" action="pad 3" /><map value="SHIFT_HOTCUE_PAD3" action="padshift 3" /></mapper>"#;
        let m = &import(src, "x.xml").unwrap().mappings[0];
        let cue = &m.rows[0];
        assert_eq!(cue.midi, Midi::new(0x90, 0x48));
        assert_eq!(cue.layer, "");
        let pad = m.rows.iter().find(|r| r.name == "HOTCUE_PAD3").unwrap();
        assert_eq!(pad.midi, Midi::new(0x97, 0x02));
        assert_eq!(pad.action, Action::Script("hot_cue 3".into()));
        let del = m
            .rows
            .iter()
            .find(|r| r.name == "SHIFT_HOTCUE_PAD3")
            .unwrap();
        assert_eq!(del.midi, Midi::new(0x98, 0x02));
    }

    #[test]
    fn keyboard_files_become_shortcuts() {
        let src = r#"<mapper device="KEYBOARD"><map value="SPACE" action="play_pause" /><map value="CMD+SHIFT+D" action="deck 2 sync" /><map value="ALT+X" action="video_fx" /></mapper>"#;
        let i = import(src, "keys.xml").unwrap();
        assert_eq!(
            i.shortcuts,
            vec![
                ("space".into(), "play_pause".into()),
                ("cmd+shift+d".into(), "deck 2 sync".into())
            ]
        );
        assert_eq!(i.skipped_shortcuts[0].0, "alt+x");
    }

    #[test]
    fn a_k2_import_runs_over_djsugars_k2_mapping() {
        let dir = crate::default_mappings_dir().expect("bundled mappings");
        let base = crate::Mapping::load(&dir.join("Allen and Heath Xone K2.midi.xml")).unwrap();
        let m = &import(K2, "k2.xml").unwrap().mappings[0];
        let built = m.build(Some(&base), &dir);
        // The inner fader of deck 1 now runs the imported row.
        // The inner fader of deck 1 runs the imported row alone: used with
        // two decks, the four-deck rows and the base's binding are gone.
        let fader = &built.inputs[&(0xBE, 0x11)];
        assert_eq!(fader.len(), 1, "{fader:?}");
        assert!(
            fader[0].group == "[Channel1]" && fader[0].key == "volume" && fader[0].layer.is_empty()
        );
        // Used with four decks, column 1 is deck 3 and column 2 deck 1.
        let mut four = m.clone();
        four.decks = Some(4);
        let (rows, _) = four.for_decks();
        let eq: Vec<_> = rows.iter().filter(|r| r.name == "K2_KNOB_1").collect();
        assert!(
            matches!(&eq[0].action, Action::Bind { control, .. } if control.contains("[Channel3]"))
        );
        assert!(
            matches!(&eq[1].action, Action::Bind { control, .. } if control.contains("[Channel1]"))
        );
        assert!(rows.iter().all(|r| r.name != "K22D_FADER_INNER"));
        // The VU meter light is a script light.
        assert!(built.outputs.iter().any(|o| o
            .script
            .as_deref()
            .is_some_and(|s| s.contains("vu_meter"))
            && o.color_notes));
    }
}
