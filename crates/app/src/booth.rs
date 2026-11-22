//! DJ Sugar's Booth DJ view: two performance decks around the mixer, the
//! crossfader bar below. Sizes follow its `unit` rules so phones and DeX match.

use sugar_layout::Rect;
use sugar_ui::{Painter, Style, Weight, rgb};

use crate::state::{FX_PADS, LOOP_BEATS, Mode, State, Target, beats_label, pad_name};
use crate::sugar::{DECK, fade, hit};

const BG: u32 = rgb(0x1e1e20);
const PANEL: u32 = rgb(0x202020);
const LINE: u32 = rgb(0x626262);
const LINE2: u32 = rgb(0x676767);
const RING: u32 = rgb(0x3f3f3f);
const TEXT: u32 = rgb(0xb0b0b0);
const LABEL: u32 = rgb(0x939393);
const WHITE: u32 = rgb(0xd9d9d9);
const GREEN: u32 = rgb(0x85c85b);
const YELLOW: u32 = rgb(0xfca001);
const WARN: u32 = rgb(0x7d3b3b);
const ORANGE: u32 = rgb(0xff6a13);
const BLUE: u32 = rgb(0x01dcfc);

fn bold(px: f32, colour: u32) -> Style {
    Style::new(px.round().max(7.0), Weight::Bold, colour)
}

fn centred(p: &mut Painter, r: Rect, st: Style, text: &str) {
    let w = p.text_width(st, text).min(r.w - 2.0);
    p.text_in(r.x + (r.w - w) * 0.5, r.y + r.h * 0.5 + st.px * 0.36, r.w - 2.0, st, text);
}

/// The Booth `SmallButton`: grey frame, tinted and outlined in its colour when on.
fn small(p: &mut Painter, s: &mut State, r: Rect, unit: f32, (label, on, colour): (&str, bool, u32), t: Target) {
    let down = s.pressed(t);
    let fill = if down {
        LINE
    } else if on {
        fade(colour, 0x2e)
    } else {
        PANEL
    };
    p.outlined(r.x, r.y, r.w, r.h, 2.0, fill, 1.0, if on { colour } else { LINE2 });
    centred(p, r, bold(unit * 1.05, if on { colour } else { TEXT }), label);
    hit(s, r, t);
}

/// The Booth knob: a 270 degree ring, filled from the centre or the start, and a pointer.
pub fn knob(p: &mut Painter, s: &mut State, r: Rect, (v, bipolar, colour): (f32, bool, u32), label: &str, t: Target) {
    let lp = (r.w * 0.22).min(r.h * 0.28).round().clamp(7.0, 13.0);
    let lh = if label.is_empty() { 0.0 } else { lp + 3.0 };
    let size = r.w.min(r.h - lh).clamp(12.0, 64.0);
    let (cx, cy) = (r.x + r.w * 0.5, r.y + (r.h - lh - size) * 0.5 + size * 0.5);
    let lw = (size / 11.0).max(2.0);
    let rad = size * 0.5 - lw;
    p.arc(cx, cy, rad + lw * 0.5, lw, 0.625, 0.75, LINE);
    let at = 0.625 + v.clamp(0.0, 1.0) * 0.75;
    let from = if bipolar { 1.0 } else { 0.625 };
    if (at - from).abs() > 0.002 {
        p.arc(cx, cy, rad + lw * 0.5, lw, at.min(from), (at - from).abs(), colour);
    }
    let a = at * std::f32::consts::TAU;
    let (sn, cs) = a.sin_cos();
    p.line(
        (cx + sn * rad * 0.3, cy - cs * rad * 0.3),
        (cx + sn * rad * 0.8, cy - cs * rad * 0.8),
        (size / 18.0).max(1.5),
        WHITE,
    );
    if lh > 0.0 {
        let st = Style::new(lp, Weight::Regular, LABEL);
        let w = p.text_width(st, label);
        p.text(cx - w * 0.5, r.y + r.h - 2.0, st, label);
    }
    hit(s, r, t);
}

/// The Booth fader: a thin track, an optional centre mark, a grey cap with a coloured line.
fn fader(
    p: &mut Painter,
    s: &mut State,
    r: Rect,
    (pos, horizontal, centre): (f32, bool, bool),
    colour: u32,
    t: Target,
) {
    let along = if horizontal { r.h } else { r.w };
    let handle = (along * 0.6).round().max(14.0);
    if horizontal {
        p.rect(r.x + handle * 0.25, r.y + r.h * 0.5 - 2.0, r.w - handle * 0.5, 4.0, 2.0, LINE);
        if centre {
            p.rect(r.x + r.w * 0.5 - 0.5, r.y + r.h * 0.2, 1.0, r.h * 0.6, 0.0, LINE);
        }
        let hw = handle * 0.55;
        let x = r.x + pos * (r.w - hw);
        p.outlined(x, r.y, hw, r.h, 2.0, LINE2, 1.0, if s.pressed(t) { WHITE } else { LABEL });
        p.rect(x + hw * 0.5 - 1.0, r.y + 3.0, 2.0, r.h - 6.0, 0.0, colour);
    } else {
        p.rect(r.x + r.w * 0.5 - 2.0, r.y + handle * 0.25, 4.0, r.h - handle * 0.5, 2.0, LINE);
        if centre {
            p.rect(r.x + r.w * 0.2, r.y + r.h * 0.5 - 0.5, r.w * 0.6, 1.0, 0.0, LINE);
        }
        let hh = handle * 0.55;
        let y = r.y + (1.0 - pos) * (r.h - hh);
        p.outlined(r.x, y, r.w, hh, 2.0, LINE2, 1.0, if s.pressed(t) { WHITE } else { LABEL });
        p.rect(r.x + 3.0, y + hh * 0.5 - 1.0, r.w - 6.0, 2.0, 0.0, colour);
    }
    hit(s, r, t);
}

/// Two decks, the mixer between them, the crossfader bar below; stacked on portrait screens.
pub fn dj(p: &mut Painter, s: &mut State, r: Rect) {
    p.rect(r.x, r.y, r.w, r.h, 0.0, BG);
    let gap = 3.0;
    let compact = r.h < 520.0;
    let bar = if compact { 30.0 } else { 40.0 };
    let top = Rect::new(r.x, r.y, r.w, r.h - bar - gap);
    if r.h > r.w * 1.1 {
        let mh = (top.h * 0.26).round();
        let dh = ((top.h - mh - 2.0 * gap) * 0.5).floor();
        deck(p, s, Rect::new(top.x, top.y, top.w, dh), 0, false);
        mixer(p, s, Rect::new(top.x, top.y + dh + gap, top.w, mh));
        deck(p, s, Rect::new(top.x, top.y + dh + mh + 2.0 * gap, top.w, dh), 1, true);
    } else {
        let mw = (top.w * 0.24).min(top.h * 0.9).round();
        let dw = ((top.w - mw - 2.0 * gap) * 0.5).floor();
        deck(p, s, Rect::new(top.x, top.y, dw, top.h), 0, false);
        mixer(p, s, Rect::new(top.x + dw + gap, top.y, mw, top.h));
        deck(p, s, Rect::new(top.x + top.w - dw, top.y, dw, top.h), 1, true);
    }
    bottom(p, s, Rect::new(r.x, r.y + r.h - bar, r.w, bar));
}

fn bottom(p: &mut Painter, s: &mut State, r: Rect) {
    p.outlined(r.x, r.y, r.w, r.h, 0.0, PANEL, 1.0, LINE);
    let bw = (r.h * 2.6).round();
    let auto = s.auto.is_some();
    small(p, s, Rect::new(r.x + 8.0, r.y + 4.0, bw, r.h - 8.0), r.h * 0.32, ("AUTO DJ", auto, BLUE), Target::AutoDj);
    let w = (r.w * 0.3).min(320.0).round();
    let pos = (s.crossfader + 1.0) * 0.5;
    fader(p, s, Rect::new(r.x + (r.w - w) * 0.5, r.y + 3.0, w, r.h - 6.0), (pos, true, true), BLUE, Target::Crossfader);
    let label = if s.recorder.is_some() { "REC ON" } else { "REC" };
    let bw = (r.h * 2.2).round();
    small(
        p,
        s,
        Rect::new(r.x + r.w - bw - 8.0, r.y + 4.0, bw, r.h - 8.0),
        r.h * 0.32,
        (label, s.recorder.is_some(), rgb(0xea2a4e)),
        Target::Record,
    );
}

fn deck(p: &mut Painter, s: &mut State, r: Rect, d: usize, mirrored: bool) {
    let unit = (r.h / 14.0).min(r.w / 34.0).max(9.0);
    p.outlined(r.x, r.y, r.w, r.h, 0.0, PANEL, 1.0, LINE);
    let r = Rect::new(r.x + 4.0, r.y + 4.0, r.w - 8.0, r.h - 8.0);
    let gap = 4.0;
    // Pads, loop column, transport, jog, tempo, laid out from the outer edge in.
    let tempo_w = (unit * 2.6).round();
    let button = (unit * 4.2).round().min((r.h + 8.0 - 12.0) / 3.4);
    let loop_w = (unit * 5.4).round();
    let rest = r.w - tempo_w - button - loop_w - 4.0 * gap;
    let pads_w = (unit * 16.0).min(rest * 0.53).max(unit * 10.0).min(rest - unit * 8.0).max(0.0);
    let jog_w = rest - pads_w;
    let widths = [pads_w, loop_w, button, jog_w, tempo_w];
    let mut x = if mirrored { r.x + r.w } else { r.x };
    let mut cols = [Rect::default(); 5];
    for (c, w) in cols.iter_mut().zip(widths) {
        if mirrored {
            x -= w;
            *c = Rect::new(x, r.y, w, r.h);
            x -= gap;
        } else {
            *c = Rect::new(x, r.y, w, r.h);
            x += w + gap;
        }
    }
    pads(p, s, cols[0], d, unit);
    loops(p, s, cols[1], d, unit);
    transport(p, s, cols[2], d, unit);
    if jog_w >= unit * 6.0 {
        jog(p, s, cols[3], d, unit, mirrored);
    }
    let rate = (s.decks[d].rate + 1.0) * 0.5;
    fader(p, s, cols[4], (rate, false, true), DECK[d], Target::Rate(d));
}

fn pads(p: &mut Painter, s: &mut State, r: Rect, d: usize, unit: f32) {
    let mode_h = (unit * 1.8).round();
    let grid = Rect::new(r.x, r.y, r.w, r.h - mode_h - 3.0);
    let (pw, ph) = ((grid.w - 3.0) * 0.5, (grid.h - 9.0) / 4.0);
    let mode = s.decks[d].mode;
    for i in 0..8 {
        let b = Rect::new(grid.x + (i % 2) as f32 * (pw + 3.0), grid.y + (i / 2) as f32 * (ph + 3.0), pw, ph);
        let (label, lit, colour) = match mode {
            Mode::Cues => ((i + 1).to_string(), s.decks[d].cues[i].is_some(), crate::sugar::cue_colour(s, d, i)),
            Mode::Loops => (beats_label(LOOP_BEATS[i]), s.decks[d].looping == Some(i), DECK[0]),
            Mode::Samples => (pad_name(s, d, i), false, BLUE),
            Mode::Fx => (FX_PADS[i].to_string(), s.decks[d].echo == Some(i), YELLOW),
        };
        let down = s.pressed(Target::Pad(d, i));
        p.outlined(
            b.x,
            b.y,
            b.w,
            b.h,
            0.0,
            if down { LINE } else { PANEL },
            1.0,
            if down {
                WHITE
            } else if lit {
                colour
            } else {
                LINE
            },
        );
        if mode == Mode::Cues {
            let k = (b.h - 6.0).min(unit * 1.5);
            let lr = Rect::new(b.x + 3.0, b.y + (b.h - k) * 0.5, k, k);
            if lit {
                p.rect(lr.x, lr.y, lr.w, lr.h, 0.0, colour);
            }
            let letter = char::from(b'A' + i as u8).to_string();
            centred(p, lr, bold(k * 0.7, if lit { rgb(0x000000) } else { LINE }), &letter);
            if lit && b.w > k * 3.0 {
                let at = s.decks[d].cues[i].unwrap_or(0.0);
                let t = mmss(s.decks[d].seconds(at));
                centred(
                    p,
                    Rect::new(lr.x + k, b.y, b.w - k - 3.0, b.h),
                    Style::new((unit * 0.85).round(), Weight::Regular, TEXT),
                    &t,
                );
            }
        } else {
            centred(p, b, bold(unit * 0.95, if lit { WHITE } else { TEXT }), &label);
        }
        hit(s, b, Target::Pad(d, i));
    }
    let m = Rect::new(r.x, r.y + r.h - mode_h, r.w, mode_h);
    let next = match mode {
        Mode::Cues => Mode::Loops,
        Mode::Loops => Mode::Samples,
        Mode::Samples => Mode::Fx,
        Mode::Fx => Mode::Cues,
    };
    let t = Target::Mode(d, next);
    p.outlined(m.x, m.y, m.w, m.h, 0.0, if s.pressed(t) { LINE } else { PANEL }, 1.0, LINE);
    let name = ["HOT CUE", "BEAT LOOP", "SAMPLER", "FX"][mode as usize];
    centred(p, m, Style::new(unit.round(), Weight::Regular, TEXT), name);
    hit(s, m, t);
}

fn loops(p: &mut Painter, s: &mut State, r: Rect, d: usize, unit: f32) {
    let looping = s.decks[d].loop_range.is_some();
    let mut y = r.y;
    let st = bold(unit * 0.95, if looping { WARN } else { LABEL });
    p.text(r.x + 1.0, y + st.px, st, "LOOP");
    y += st.px + 5.0;
    let big = Rect::new(r.x, y, r.w, (unit * 3.8).round());
    let t = Target::LoopToggle(d);
    let fill = if s.pressed(t) {
        rgb(0x5a2a08)
    } else if looping {
        WARN
    } else {
        PANEL
    };
    p.outlined(
        big.x,
        big.y,
        big.w,
        big.h,
        3.0,
        fill,
        if looping { 2.0 } else { 1.0 },
        if looping { ORANGE } else { LINE },
    );
    let size = beats_label(LOOP_BEATS[s.decks[d].loop_size]);
    let ink = if looping { BG } else { DECK[1] };
    centred(p, Rect::new(big.x, big.y, big.w, big.h * 0.68), bold(unit * 1.8, ink), &size);
    let sub = if looping { "EXIT" } else { "BEATS" };
    centred(
        p,
        Rect::new(big.x, big.y + big.h * 0.58, big.w, big.h * 0.36),
        bold(unit * 0.8, if looping { BG } else { LABEL }),
        sub,
    );
    hit(s, big, t);
    y += big.h + 3.0;
    let row = (unit * 2.4).round();
    let hw = (r.w - 3.0) * 0.5;
    small(p, s, Rect::new(r.x, y, hw, row), unit, ("1/2", false, WARN), Target::LoopSize(d, -1));
    small(p, s, Rect::new(r.x + hw + 3.0, y, hw, row), unit, ("x2", false, WARN), Target::LoopSize(d, 1));
    y += row + 3.0;
    let armed = s.decks[d].loop_in.is_some();
    small(p, s, Rect::new(r.x, y, hw, row), unit, ("IN", armed, ORANGE), Target::LoopIn(d));
    small(p, s, Rect::new(r.x + hw + 3.0, y, hw, row), unit, ("OUT", false, ORANGE), Target::LoopOut(d));
    let bh = (unit * 2.2).round();
    let y = r.y + r.h - 2.0 * bh - 3.0;
    small(p, s, Rect::new(r.x, y, r.w, bh), unit, ("GRID", false, WARN), Target::Grid(d));
    small(p, s, Rect::new(r.x, y + bh + 3.0, r.w, bh), unit, ("SLIP", s.decks[d].slip, ORANGE), Target::Slip(d));
}

fn transport(p: &mut Painter, s: &mut State, r: Rect, d: usize, unit: f32) {
    let sh = (unit * 2.2).round();
    small(p, s, Rect::new(r.x, r.y, r.w, sh), unit, ("SYNC", false, DECK[0]), Target::Sync(d));
    let k = r.w;
    let play_y = r.y + r.h - k;
    let cue_y = play_y - 4.0 - k;
    let cue = Target::Cue(d);
    let lit = s.pressed(cue);
    p.outlined(
        r.x,
        cue_y,
        k,
        k,
        k * 0.5,
        if lit { rgb(0x3a2408) } else { PANEL },
        2.0,
        if lit { rgb(0xff9a1f) } else { rgb(0x8a5a20) },
    );
    centred(p, Rect::new(r.x, cue_y, k, k), bold(k * 0.28, if lit { rgb(0xff9a1f) } else { rgb(0xc08040) }), "CUE");
    hit(s, Rect::new(r.x, cue_y, k, k), cue);
    let play = Target::Play(d);
    let on = s.telemetry.decks[d].playing;
    let fill = if s.pressed(play) { rgb(0x0c2a0c) } else { PANEL };
    p.outlined(r.x, play_y, k, k, k * 0.5, fill, 2.0, if on { GREEN } else { rgb(0x2a6a2a) });
    let (cx, cy) = (r.x + k * 0.5, play_y + k * 0.5);
    if on {
        let bh = (k * 0.34).round();
        let bw = (bh * 0.3).round();
        let sp = (bh * 0.3).round();
        p.rect(cx - sp * 0.5 - bw, cy - bh * 0.5, bw, bh, 0.0, GREEN);
        p.rect(cx + sp * 0.5, cy - bh * 0.5, bw, bh, 0.0, GREEN);
    } else {
        let h = (k * 0.36).round();
        let w = (h * 0.9).round();
        let x0 = cx - w * 0.5 + (w * 0.1).round();
        for i in 0..h as i32 {
            let f = i as f32 / h;
            let len = w * (1.0 - (f * 2.0 - 1.0).abs());
            p.rect(x0, cy - h * 0.5 + i as f32, len, 1.0, 0.0, GREEN);
        }
    }
    hit(s, Rect::new(r.x, play_y, k, k), play);
}

fn jog(p: &mut Painter, s: &mut State, r: Rect, d: usize, unit: f32, mirrored: bool) {
    let size = r.w.min(r.h);
    let (cx, cy) = (r.x + r.w * 0.5, r.y + r.h * 0.5);
    let rad = size * 0.5;
    let bw = (size * 0.04).round().max(2.0);
    p.outlined(cx - rad, cy - rad, size, size, rad, BG, bw, LINE2);
    let inner = rad - (size * 0.1).round();
    p.outlined(cx - inner, cy - inner, inner * 2.0, inner * 2.0, inner, 0, 1.0, RING);
    let deck = &s.decks[d];
    if deck.loaded.is_some() {
        let turn = (deck.seconds(s.telemetry.decks[d].position) / 1.8).fract() as f32 * std::f32::consts::TAU;
        let (sn, cs) = turn.sin_cos();
        let len = (size * 0.12).round();
        let w = (size * 0.035).round().max(3.0);
        let (a, b) = (rad - 2.0, rad - 2.0 - len);
        p.line((cx + sn * a, cy - cs * a), (cx + sn * b, cy - cs * b), w, DECK[d]);
    }
    let bpm = deck.loaded.as_ref().and_then(|l| l.grid).map(|g| g.bpm * (1.0 + f64::from(deck.rate) * 0.08));
    let (whole, frac) = bpm.map_or(("--".to_string(), ".--".to_string()), |b| {
        (format!("{}", b.floor()), format!(".{:02}", ((b.fract()) * 100.0).floor() as u32))
    });
    let big = Style::new((size * 0.17).round(), Weight::Regular, WHITE);
    let sm = Style::new((size * 0.09).round(), Weight::Regular, WHITE);
    let (ww, fw) = (p.text_width(big, &whole), p.text_width(sm, &frac));
    let base = cy + big.px * 0.2;
    p.text(cx - (ww + fw) * 0.5, base, big, &whole);
    p.text(cx - (ww + fw) * 0.5 + ww, base, sm, &frac);
    let pct = f64::from(deck.rate) * 8.0;
    let line = format!("{}{pct:.1}%  +-8", if pct >= 0.005 { "+" } else { "" });
    let st = Style::new((size * 0.075).round().max(7.0), Weight::Regular, LABEL);
    let w = p.text_width(st, &line);
    p.text(cx - w * 0.5, base + st.px * 1.6, st, &line);
    hit(s, Rect::new(cx - rad, cy - rad, size, size), Target::Jog(d));
    let (bw, bh) = ((unit * 2.8).round(), (unit * 2.0).round());
    let x = if mirrored { r.x } else { r.x + r.w - bw };
    let ox = if mirrored { r.x + r.w - bw } else { r.x };
    let rev = s.decks[d].reverse;
    small(p, s, Rect::new(x, r.y, bw, bh), unit, ("REV", rev, WARN), Target::Reverse(d));
    small(p, s, Rect::new(x, r.y + bh + 3.0, bw, bh), unit, ("EJ", false, WARN), Target::Eject(d));
    let st = s.decks[d].semitones;
    small(p, s, Rect::new(ox, r.y, bw, bh), unit, ("K+", st > 0, WARN), Target::Pitch(d, 1));
    small(p, s, Rect::new(ox, r.y + bh + 3.0, bw, bh), unit, ("K-", st < 0, WARN), Target::Pitch(d, -1));
    let q = s.decks[d].quantize;
    let mt = s.decks[d].key_lock;
    small(p, s, Rect::new(x, r.y + r.h - 2.0 * bh - 3.0, bw, bh), unit, ("Q", q, WARN), Target::Quantize(d));
    small(p, s, Rect::new(x, r.y + r.h - bh, bw, bh), unit, ("MT", mt, WARN), Target::KeyLock(d));
}

fn mixer(p: &mut Painter, s: &mut State, r: Rect) {
    p.outlined(r.x, r.y, r.w, r.h, 0.0, PANEL, 1.0, LINE);
    let unit = (r.h / 14.0).min(r.w / 16.0).max(9.0);
    let inner = Rect::new(r.x + 4.0, r.y + 4.0, r.w - 8.0, r.h - 8.0);
    let cw = (inner.w - 6.0) * 0.5;
    for d in 0..2 {
        channel(p, s, Rect::new(inner.x + d as f32 * (cw + 6.0), inner.y, cw, inner.h), d, unit);
    }
}

fn channel(p: &mut Painter, s: &mut State, r: Rect, d: usize, unit: f32) {
    let fw = (unit * 2.8).round();
    let kw = r.w - fw - 3.0;
    let wide = kw > r.h * 1.4;
    let (cols, rows) = if wide {
        (5.0, 1.0)
    } else if kw > r.h * 0.5 {
        (2.0, 3.0)
    } else {
        (1.0, 5.0)
    };
    let (cw, chh) = (kw / cols, r.h / rows);
    let cell = |n: usize| {
        let (c, row) = ((n % cols as usize) as f32, (n / cols as usize) as f32);
        Rect::new(r.x + c * cw + 1.0, r.y + row * chh + 1.0, cw - 2.0, chh - 2.0)
    };
    let g = s.decks[d].gain;
    knob(p, s, cell(0), (g, true, DECK[0]), "TRIM", Target::Gain(d));
    for (n, label) in ["HIGH", "MID", "LOW"].into_iter().enumerate() {
        let v = s.decks[d].knobs[n];
        knob(p, s, cell(n + 1), (v, true, DECK[0]), label, Target::Knob(d, n));
    }
    let v = s.decks[d].knobs[3];
    knob(p, s, cell(4), (v, true, GREEN), "FILTER", Target::Knob(d, 3));
    let c = Rect::new(r.x + kw + 3.0, r.y, fw, r.h);
    let ch = (unit * 1.8).round();
    let pfl = s.decks[d].pfl;
    let t = Target::Pfl(d);
    p.outlined(c.x, c.y, c.w, ch, 2.0, if pfl { rgb(0x3a1a08) } else { PANEL }, 1.0, if pfl { WARN } else { LINE2 });
    centred(p, Rect::new(c.x, c.y, c.w, ch), bold((unit * 0.95).min(ch * 0.55), if pfl { WARN } else { TEXT }), "CUE");
    hit(s, Rect::new(c.x, c.y, c.w, ch), t);
    let y = c.y + ch + 3.0;
    let h = c.y + c.h - y;
    let mw = (unit * 0.45).round().max(3.0);
    p.rect(c.x, y, mw, h, 0.0, PANEL);
    let peak = s.telemetry.decks[d].peak;
    let vu = peak.clamp(0.0, 1.0);
    let colour = if peak >= 1.0 {
        WARN
    } else if vu > 0.8 {
        YELLOW
    } else {
        GREEN
    };
    p.rect(c.x, y + h * (1.0 - vu), mw, h * vu, 0.0, colour);
    let vol = s.decks[d].volume;
    fader(p, s, Rect::new(c.x + mw + 2.0, y, c.w - mw - 2.0, h), (vol, false, false), DECK[d], Target::Volume(d));
}

fn mmss(secs: f64) -> String {
    let t = secs.max(0.0);
    format!("{}:{:04.1}", (t / 60.0) as u32, t % 60.0)
}
