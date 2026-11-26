//! The wide deck row: a platter on each outer edge ringed by the playing position, the
//! deck's readouts, pads and transport beside it, and a slim mixer between the decks.
//! Everything is sized from one unit that follows the row's height.

use sugar_layout::Rect;
use sugar_ui::{Painter, Style, Weight, rgb};

use crate::classic::{bold, fader, knob, mmss, overview, vu_meter};
use crate::state::{FX_PADS, LOOP_BEATS, Mode, State, Target, beats_label, pad_name};
use crate::sugar::{DECK, cue_colour, fade, hit};
use crate::theme::{BORDER, CONTROL, DIM, PANEL, PRESSED, RAISED, SEPARATOR, TEXT, WELL, centred};

const PINK: u32 = rgb(0xff4fa8);
const KEY: u32 = rgb(0x8e7cff);
const INK: u32 = rgb(0x151329);
const BLACK: u32 = rgb(0x000000);
const MIXER: f32 = 236.0;
/// The row's height when there is room for it.
pub const HEIGHT: f32 = 281.0;

pub fn row(p: &mut Painter, s: &mut State, r: Rect) {
    p.rect(r.x, r.y, r.w, r.h, 0.0, BLACK);
    let side = ((r.w - MIXER - 2.0) * 0.5).floor();
    deck(p, s, Rect::new(r.x, r.y, side, r.h), 0);
    mixer(p, s, Rect::new(r.x + side + 1.0, r.y, r.w - 2.0 * side - 2.0, r.h));
    deck(p, s, Rect::new(r.x + r.w - side, r.y, side, r.h), 1);
}

/// A pill: dark, or washed in its tint when on.
fn chip(p: &mut Painter, s: &mut State, b: Rect, label: &str, px: f32, (on, tint): (bool, u32), t: Option<Target>) {
    let down = t.is_some_and(|t| s.pressed(t));
    let (fill, edge) = match (down, on) {
        (true, _) => (PRESSED, tint),
        (false, true) => (fade(tint, 0x38), fade(tint, 0xb0)),
        (false, false) => (CONTROL, BORDER),
    };
    p.outlined(b.x, b.y, b.w, b.h, b.h * 0.5, fill, 1.0, edge);
    centred(p, b, bold(px, if on { tint } else { DIM }), label);
    if let Some(t) = t {
        hit(s, b, t);
    }
}

/// A round transport button: outlined in the deck's colour, filled with it when lit.
fn candy(p: &mut Painter, s: &mut State, b: Rect, u: f32, (lit, tint): (bool, u32), t: Target) -> u32 {
    let down = s.pressed(t);
    if lit {
        p.glow(b.x, b.y, b.w, b.h, b.h * 0.5, u * 1.2, fade(tint, 0x80));
    }
    let fill = if lit {
        tint
    } else if down {
        PRESSED
    } else {
        CONTROL
    };
    p.outlined(b.x, b.y, b.w, b.h, b.h * 0.5, fill, 1.5, if lit { tint } else { fade(tint, 0x73) });
    hit(s, b, t);
    if lit { INK } else { tint }
}

fn deck(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    let mirrored = d == 1;
    p.rect(r.x, r.y, r.w, r.h, 0.0, PANEL);
    let u = (r.h / 26.0).clamp(8.0, 15.0);
    let dia = (r.h - 2.0 * u).min(r.w * if r.w > 700.0 { 0.36 } else { 0.27 });
    let px = if mirrored { r.x + r.w - u - dia } else { r.x + u };
    platter(p, s, Rect::new(px, r.y + (r.h - dia) * 0.5, dia, dia), d);
    // The tempo fader runs down the edge by the mixer.
    let tw = (u * 3.4).max(34.0);
    let tx = if mirrored { r.x + u } else { r.x + r.w - u - tw };
    tempo(p, s, Rect::new(tx, r.y + u, tw, r.h - 2.0 * u), d, u);
    let x0 = if mirrored { tx + tw + u } else { px + dia + u * 1.2 };
    let x1 = if mirrored { px - u * 1.2 } else { tx - u };
    let pane = Rect::new(x0, r.y + u, x1 - x0, r.h - 2.0 * u);
    if pane.w < 120.0 {
        return;
    }
    let gap = (u * 0.7).round();
    let mut y = pane.y;
    let head = (u * 3.1).round();
    title(p, s, Rect::new(pane.x, y, pane.w, head), d, u);
    y += head + gap;
    let ov = Rect::new(pane.x, y, pane.w, (u * 2.6).round());
    if s.decks[d].loaded.is_some() {
        overview(p, s, ov, d);
    } else {
        p.rect(ov.x, ov.y, ov.w, ov.h, u * 0.5, WELL);
    }
    y += ov.h + gap;
    let th = (u * 2.9).round();
    readout(p, s, Rect::new(pane.x, y, pane.w, th), d, u);
    y += th + gap;
    let bh = (u * 3.2).round();
    let tools = (u * 2.0).max(20.0).round();
    tool_row(p, s, Rect::new(pane.x, y, pane.w, tools), d, u);
    y += tools + gap;
    let bottom = pane.y + pane.h;
    pads(p, s, Rect::new(pane.x, y, pane.w, bottom - bh - gap - y), d, u);
    transport(p, s, Rect::new(pane.x, bottom - bh, pane.w, bh), d, u);
}

/// The deck's letter on its colour, then the title over the artist.
fn title(p: &mut Painter, s: &mut State, r: Rect, d: usize, u: f32) {
    let mirrored = d == 1;
    let b = u * 2.0;
    let bx = if mirrored { r.x + r.w - b } else { r.x };
    let badge = Rect::new(bx, r.y + (r.h - b) * 0.5, b, b);
    p.rect(badge.x, badge.y, badge.w, badge.h, u * 0.6, DECK[d]);
    centred(p, badge, bold(u * 1.1, INK), ["A", "B"][d]);
    let deck = &s.decks[d];
    let (name, artist, ink) = match &deck.loaded {
        Some(l) => (l.entry.title.to_string(), l.entry.artist.to_string(), TEXT),
        None if deck.loading => ("Loading".to_string(), String::new(), DIM),
        None => ("Drop a track here".to_string(), String::new(), DIM),
    };
    if deck.loaded.is_some() {
        // Tapping the letter ejects the track.
        hit(s, badge, Target::Eject(d));
    }
    let big = bold(u * 1.35, ink);
    let small = Style::new((u * 1.05).round().max(8.0), Weight::Regular, DIM);
    let w = r.w - b - u * 0.6;
    let top = if artist.is_empty() { r.y + (r.h - big.px) * 0.5 - u * 0.15 } else { r.y };
    for (st, text, base) in [(big, &name, top + big.px), (small, &artist, top + big.px + small.px + u * 0.4)] {
        let tw = p.text_width(st, text).min(w);
        let x = if mirrored { r.x + w - tw } else { r.x + b + u * 0.6 };
        p.text_in(x, base, w, st, text);
    }
}

/// The time left, large, then the tempo and key chips and the key shift.
fn readout(p: &mut Painter, s: &mut State, r: Rect, d: usize, u: f32) {
    let mirrored = d == 1;
    let deck = &s.decks[d];
    let at = s.telemetry.decks[d].position;
    let rate = f64::from(deck.rate) * s.settings.rate_range();
    let (time, low) = match &deck.loaded {
        Some(_) => {
            let left = deck.seconds((deck.length() - at).max(0.0));
            (format!("-{}", mmss(left)), left < 30.0)
        }
        None => ("--:--".to_string(), false),
    };
    let bpm = deck
        .loaded
        .as_ref()
        .and_then(|l| l.grid)
        .map_or_else(|| "BPM".to_string(), |g| format!("{:.2}", g.bpm * (1.0 + rate)));
    let key = deck.loaded.as_ref().and_then(|l| l.key).map_or_else(|| "KEY".to_string(), |k| s.settings.key(k));
    let st = deck.semitones;
    let loaded = deck.loaded.is_some();
    let key = if st == 0 { key } else { format!("{key} {st:+}") };
    let big = bold(u * 2.2, if low { DECK[d] } else { TEXT });
    let cp = u * 1.05;
    let ch = (u * 2.0).max(18.0);
    let cy = r.y + (r.h - ch) * 0.5;
    let label = bold(cp, TEXT);
    let widths =
        [p.text_width(big, &time) + u * 0.9, p.text_width(label, &bpm) + u * 1.6, p.text_width(label, &key) + u * 1.6];
    let step = ch.max(20.0);
    let total: f32 = widths.iter().sum::<f32>() + u + 2.0 * step + u * 0.5;
    let mut x = if mirrored { r.x + r.w - total } else { r.x };
    p.text(x, r.y + r.h * 0.5 + big.px * 0.36, big, &time);
    x += widths[0];
    let b = Rect::new(x, cy, widths[1], ch);
    p.outlined(b.x, b.y, b.w, b.h, ch * 0.5, CONTROL, 1.0, BORDER);
    centred(p, b, bold(cp, if loaded { TEXT } else { DIM }), &bpm);
    x += widths[1] + u * 0.5;
    let b = Rect::new(x, cy, widths[2], ch);
    p.outlined(b.x, b.y, b.w, b.h, ch * 0.5, CONTROL, 1.0, BORDER);
    centred(p, b, bold(cp, if loaded { KEY } else { DIM }), &key);
    x += widths[2] + u * 0.5;
    chip(p, s, Rect::new(x, cy, step, ch), "-", cp, (false, KEY), Some(Target::Pitch(d, -1)));
    chip(p, s, Rect::new(x + step + 2.0, cy, step, ch), "+", cp, (false, KEY), Some(Target::Pitch(d, 1)));
    // The deck switches fill what is left, on the far side.
    let deck = &s.decks[d];
    let switches = [
        ("Q", deck.quantize, Target::Quantize(d)),
        ("SLIP", deck.slip, Target::Slip(d)),
        ("REV", deck.reverse, Target::Reverse(d)),
        ("GRID", deck.grid_locked, Target::Grid(d)),
    ];
    let sw = (u * 3.4).max(34.0);
    let room = r.w - total - u;
    let n = ((room / (sw + 3.0)).floor() as usize).min(switches.len());
    let width = n as f32 * (sw + 3.0) - 3.0;
    let mut x = if mirrored { r.x } else { r.x + r.w - width };
    for (label, on, t) in switches.into_iter().take(n) {
        chip(p, s, Rect::new(x, cy, sw, ch), label, cp * 0.85, (on, PINK), Some(t));
        x += sw + 3.0;
    }
}

const MODES: [(Mode, &str); 4] =
    [(Mode::Cues, "CUES"), (Mode::Loops, "LOOPS"), (Mode::Samples, "SMPL"), (Mode::Fx, "FX")];

/// What the pads play, as one segmented pill, then the loop: its size between halve and
/// double, IN and OUT.
fn tool_row(p: &mut Painter, s: &mut State, r: Rect, d: usize, u: f32) {
    let mirrored = d == 1;
    let px = (u * 0.9).clamp(8.0, 11.0);
    // The loop's cells, as many as fit beside the modes: all five, the size and its steps, or none.
    let n = if r.w >= 370.0 {
        5
    } else if r.w >= 262.0 {
        3
    } else {
        0
    };
    let looped = n > 0;
    let cw = if looped { ((r.w * 0.46 - 3.0 * (n as f32 - 1.0)) / n as f32).clamp(30.0, 38.0).floor() } else { 0.0 };
    let lw = if looped { n as f32 * cw + 3.0 * (n as f32 - 1.0) } else { 0.0 };
    let mw = (r.w - lw - if looped { u * 0.6 } else { 0.0 }).min(260.0);
    let mx = if mirrored { r.x + r.w - mw } else { r.x };
    p.outlined(mx, r.y, mw, r.h, r.h * 0.5, WELL, 1.0, BORDER);
    let seg = mw / MODES.len() as f32;
    let mode = s.decks[d].mode;
    for (i, (m, label)) in MODES.into_iter().enumerate() {
        let b = Rect::new(mx + i as f32 * seg, r.y, seg, r.h);
        let t = Target::Mode(d, m);
        let on = mode == m;
        if on || s.pressed(t) {
            p.outlined(b.x + 2.0, b.y + 2.0, b.w - 4.0, b.h - 4.0, (b.h - 4.0) * 0.5, RAISED, 1.0, BORDER);
        }
        centred(p, b, bold(px, if on { TEXT } else { DIM }), label);
        hit(s, b, t);
    }
    if !looped {
        return;
    }
    let lx = if mirrored { r.x } else { r.x + r.w - lw };
    let looping = s.decks[d].loop_range.is_some();
    let size = beats_label(s.decks[d].loop_size as f64);
    let cells: [(&str, bool, Target); 5] = [
        ("<", false, Target::LoopSize(d, -1)),
        (&size, looping, Target::LoopToggle(d)),
        (">", false, Target::LoopSize(d, 1)),
        ("IN", s.decks[d].loop_in.is_some(), Target::LoopIn(d)),
        ("OUT", false, Target::LoopOut(d)),
    ];
    for (i, (label, on, t)) in cells.into_iter().take(n).enumerate() {
        chip(p, s, Rect::new(lx + i as f32 * (cw + 3.0), r.y, cw, r.h), label, px, (on, PINK), Some(t));
    }
}

/// Eight pads in two rows: dark until set, then washed in their colour.
fn pads(p: &mut Painter, s: &mut State, r: Rect, d: usize, u: f32) {
    let gap = (u * 0.45).round();
    let (pw, ph) = ((r.w - 3.0 * gap) / 4.0, (r.h - gap) * 0.5);
    let mode = s.decks[d].mode;
    for i in 0..8 {
        let b = Rect::new(r.x + (i % 4) as f32 * (pw + gap), r.y + (i / 4) as f32 * (ph + gap), pw, ph);
        let t = Target::Pad(d, i);
        let down = s.pressed(t);
        let number = (i + 1).to_string();
        let (label, lit) = match mode {
            Mode::Cues => match s.decks[d].cues[i] {
                Some(at) => (mmss(s.decks[d].seconds(at)), Some(cue_colour(s, d, i))),
                None => (number, None),
            },
            Mode::Loops => (beats_label(LOOP_BEATS[i]), (s.decks[d].looping == Some(i)).then_some(PINK)),
            Mode::Samples => (pad_name(s, d, i), None),
            Mode::Fx => (FX_PADS[i].to_string(), (s.decks[d].echo == Some(i)).then_some(rgb(0xff9f0a))),
        };
        let (fill, edge, ink) = match (lit, down) {
            (Some(c), true) => (c, c, INK),
            (Some(c), false) => (fade(c, 0x38), fade(c, 0xb3), TEXT),
            (None, true) => (PRESSED, BORDER, TEXT),
            (None, false) => (CONTROL, BORDER, DIM),
        };
        if lit.is_some() && down {
            p.glow(b.x, b.y, b.w, b.h, u * 0.7, u, fade(fill, 0x99));
        }
        p.outlined(b.x, b.y, b.w, b.h, u * 0.7, fill, 1.0, edge);
        centred(p, b, bold(u * 1.05, ink), &label);
        hit(s, b, t);
    }
}

/// CUE, play and SYNC along the bottom.
fn transport(p: &mut Painter, s: &mut State, r: Rect, d: usize, u: f32) {
    let gap = (u * 0.6).round();
    let unit = (r.w - 2.0 * gap) / 3.4;
    let tint = DECK[d];
    let t = s.telemetry.decks[d];
    let cue = Rect::new(r.x, r.y, unit, r.h);
    let lit = s.pressed(Target::Cue(d)) || s.decks[d].previewing;
    let ink = candy(p, s, cue, u, (lit, tint), Target::Cue(d));
    centred(p, cue, bold(u * 1.05, ink), "CUE");
    let play = Rect::new(cue.x + unit + gap, r.y, unit * 1.4, r.h);
    let ink = candy(p, s, play, u, (t.playing, tint), Target::Play(d));
    let (cx, cy) = (play.x + play.w * 0.5, play.y + play.h * 0.5);
    let k = (u * 1.3).round();
    if t.playing {
        p.rect(cx - k * 0.5, cy - k * 0.5, k * 0.34, k, 1.0, ink);
        p.rect(cx + k * 0.16, cy - k * 0.5, k * 0.34, k, 1.0, ink);
    } else {
        for i in 0..(k as usize) {
            let f = i as f32;
            let half = k * 0.5 * (1.0 - f / k);
            p.rect(cx - k * 0.4 + f, cy - half, 1.0, half * 2.0, 0.0, ink);
        }
    }
    let sync = Rect::new(play.x + play.w + gap, r.y, r.x + r.w - play.x - play.w - gap, r.h);
    let (label, on) = if t.sync_leader > 0 { ("MASTER", true) } else { ("SYNC", t.sync_lock) };
    let ink = candy(p, s, sync, u, (on, tint), Target::Sync(d));
    centred(p, sync, bold(u * 1.05, ink), label);
}

/// The tempo: the rate over its fader, key lock under it.
fn tempo(p: &mut Painter, s: &mut State, r: Rect, d: usize, u: f32) {
    let foot = (u * 3.2).round();
    let head = (u * 1.4).round();
    let rate = s.decks[d].rate;
    let pct = format!("{:+.1}%", f64::from(rate) * s.settings.rate_range() * 100.0);
    centred(
        p,
        Rect::new(r.x - 4.0, r.y, r.w + 8.0, head),
        Style::new((u * 0.9).clamp(8.0, 11.0), Weight::Semibold, DIM),
        &pct,
    );
    let f = Rect::new(r.x + r.w * 0.5 - 13.0, r.y + head + 4.0, 26.0, r.h - head - foot - 4.0 - (u * 0.7).round());
    // Ticks behind the fader, the middle one longer.
    for i in 0..=8 {
        let y = f.y + 7.0 + (f.h - 14.0) * i as f32 / 8.0;
        let w = if i == 4 { r.w } else { r.w * 0.6 };
        p.rect(r.x + (r.w - w) * 0.5, y, w, 1.0, 0.0, if i == 4 { rgb(0x55555a) } else { rgb(0x3e3e42) });
    }
    let up = s.rate_from(rate);
    fader(p, s, f, ((up + 1.0) * 0.5, false, false), DECK[d], Target::Rate(d));
    let lock = Rect::new(r.x, r.y + r.h - foot, r.w, foot);
    chip(p, s, lock, "KEY", (u * 0.85).clamp(8.0, 10.0), (s.decks[d].key_lock, DECK[d]), Some(Target::KeyLock(d)));
}

/// The platter: grooves, the played part as a ring round the rim, a turning marker, and
/// the cover or the mark in the middle.
fn platter(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    let rad = r.w * 0.5;
    let (cx, cy) = (r.x + rad, r.y + rad);
    let tint = DECK[d];
    let deck = &s.decks[d];
    let t = s.telemetry.decks[d];
    if t.playing {
        p.glow(r.x, r.y, r.w, r.h, rad, (r.w / 9.0).min(r.x.min(r.y) - 1.0).max(2.0), fade(tint, 0x8c));
    }
    p.disc(cx, cy, rad, RAISED, WELL);
    p.outlined(r.x, r.y, r.w, r.h, rad, 0, 1.0, BORDER);
    for g in [0.84, 0.74, 0.64] {
        let gr = rad * g;
        p.outlined(cx - gr, cy - gr, gr * 2.0, gr * 2.0, gr, 0, 1.0, SEPARATOR);
    }
    let ring = (r.w / 26.0).max(3.0);
    let mid = rad * 0.92;
    p.arc(cx, cy, mid, ring, 0.0, 1.0, SEPARATOR);
    let loaded = deck.loaded.is_some();
    if loaded {
        let played = (t.position / deck.length().max(1.0)).clamp(0.0, 1.0) as f32;
        if played > 0.002 {
            p.arc(cx, cy, mid, ring, 0.0, played, tint);
        }
        // The marker turns at 33 1/3 a minute.
        let turn = (deck.seconds(t.position) * (100.0 / 180.0)).fract() as f32 * std::f32::consts::TAU;
        let (sn, cs) = turn.sin_cos();
        p.line(
            (cx + sn * rad * 0.6, cy - cs * rad * 0.6),
            (cx + sn * rad * 0.8, cy - cs * rad * 0.8),
            (r.w / 50.0).max(2.0),
            TEXT,
        );
    }
    let lr = rad * 0.5;
    match deck.loaded.as_ref().and_then(|l| l.art.as_ref()) {
        Some(art) => {
            // The cover, cut to the label's circle.
            p.disc(cx, cy, lr, BLACK, BLACK);
            art.draw(p, Rect::new(cx - lr + 1.0, cy - lr + 1.0, lr * 2.0 - 2.0, lr * 2.0 - 2.0), lr);
        }
        None if loaded => {
            p.disc(cx, cy, lr, tint, tint);
            let initials: String = deck
                .loaded
                .iter()
                .flat_map(|l| l.entry.title.split_whitespace().take(2).filter_map(|w| w.chars().next()))
                .flat_map(char::to_uppercase)
                .collect();
            centred(p, Rect::new(cx - lr, cy - lr, lr * 2.0, lr * 2.0), bold(r.w / 7.0, INK), &initials);
        }
        None => {
            p.disc(cx, cy, lr, RAISED, RAISED);
            // The mark, faint: a ring and a dot.
            p.arc(cx, cy, lr * 0.42, lr * 0.16, 0.0, 1.0, fade(PINK, 0x59));
            p.disc(cx, cy, lr * 0.12, WELL, WELL);
        }
    }
    hit(s, r, Target::Jog(d));
}

fn mixer(p: &mut Painter, s: &mut State, r: Rect) {
    p.rect(r.x, r.y, r.w, r.h, 0.0, PANEL);
    let size = (r.h / 14.0).clamp(14.0, 22.0) * 2.0;
    let xf = 24.0;
    let body = Rect::new(r.x + 8.0, r.y + 8.0, r.w - 16.0, r.h - 16.0 - xf - 6.0);
    // EQ and filter knobs down the outsides.
    let kh = body.h / 4.0;
    for d in 0..2 {
        let x = if d == 0 { body.x } else { body.x + body.w - 44.0 };
        for (n, label) in ["HIGH", "MID", "LOW", "CFX"].into_iter().enumerate() {
            let b = Rect::new(x, body.y + n as f32 * kh, 44.0, kh);
            let v = s.decks[d].knobs[n];
            let colour = if n == 3 { PINK } else { rgb(0x0a84ff) };
            knob(
                p,
                s,
                Rect::new(b.x, b.y + (kh - size - 12.0).max(0.0) * 0.5, b.w, (size + 12.0).min(kh)),
                (v, true, colour),
                label,
                Target::Knob(d, n),
            );
        }
    }
    // Gain over the fader for each channel, headphone cue and the meters between.
    let inner = Rect::new(body.x + 44.0, body.y, body.w - 88.0, body.h);
    let cw = 34.0;
    let mid = inner.w - 2.0 * cw - 8.0;
    for d in 0..2 {
        let x = if d == 0 { inner.x } else { inner.x + inner.w - cw };
        let v = s.decks[d].gain;
        let top = (size * 0.72).round() + 11.0;
        knob(p, s, Rect::new(x, inner.y, cw, top), (v, true, DECK[d]), "GAIN", Target::Gain(d));
        let f = Rect::new(x + 2.0, inner.y + top + 4.0, cw - 4.0, inner.h - top - 4.0);
        for i in 0..7 {
            let y = f.y + 7.0 + (f.h - 14.0) * i as f32 / 6.0;
            p.rect(f.x, y, f.w, 1.0, 0.0, rgb(0x3e3e42));
        }
        let vol = s.decks[d].volume;
        fader(p, s, f, (vol, false, false), DECK[d], Target::Volume(d));
    }
    let mx = inner.x + cw + 4.0;
    let ph = 22.0;
    let pw = ((mid - 4.0) * 0.5).floor().min(34.0);
    let px = mx + (mid - 2.0 * pw - 4.0) * 0.5;
    for d in 0..2 {
        let b = Rect::new(px + d as f32 * (pw + 4.0), inner.y, pw, ph);
        let on = s.decks[d].pfl;
        let t = Target::Pfl(d);
        let down = s.pressed(t);
        p.outlined(
            b.x,
            b.y,
            b.w,
            b.h,
            6.0,
            if on {
                fade(DECK[d], 0x38)
            } else if down {
                PRESSED
            } else {
                CONTROL
            },
            1.0,
            if on { DECK[d] } else { BORDER },
        );
        // Headphones: a band over two cups.
        let ink = if on { DECK[d] } else { DIM };
        let (cx, cy) = (b.x + b.w * 0.5, b.y + b.h * 0.5 + 1.0);
        p.arc(cx, cy, 5.5, 1.5, 0.75, 0.5, ink);
        p.rect(cx - 7.0, cy - 0.5, 3.0, 6.0, 1.0, ink);
        p.rect(cx + 4.0, cy - 0.5, 3.0, 6.0, 1.0, ink);
        hit(s, b, t);
    }
    let my = inner.y + ph + 10.0;
    let mh = inner.y + inner.h - my;
    for (deck, dx) in s.telemetry.decks.iter().zip([-13.0, 3.0]) {
        let peak = deck.peak;
        let x = mx + mid * 0.5 + dx;
        vu_meter(p, Rect::new(x, my, 4.0, mh), peak);
        vu_meter(p, Rect::new(x + 6.0, my, 4.0, mh), peak);
    }
    let x = Rect::new(r.x + 18.0, r.y + r.h - 8.0 - xf, r.w - 36.0, xf);
    let pos = (s.crossfader + 1.0) * 0.5;
    let tint = if s.auto.is_some() { crate::theme::AUTOMIX } else { PINK };
    fader(p, s, x, (pos, true, true), tint, Target::Crossfader);
}
