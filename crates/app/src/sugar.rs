use sugar_layout::Rect;
use sugar_ui::{Painter, Style, Weight, rgb};

use crate::state::{
    DETAIL_FRAMES, FX_PADS, GridOp, Hit, JUMP_BEATS, LOOP_BEATS, Mode, State, Target, View, beats_label,
};

const INK: u32 = rgb(0x0a0b10);
const SURFACE: u32 = rgb(0x14161c);
const DEEP: u32 = rgb(0x0d0e13);
const EDGE: u32 = rgb(0x333333);
const ACCENT: u32 = rgb(0xff4fa8);
const ACTIVE: u32 = rgb(0x7c5cff);
const TEXT: u32 = rgb(0xe8e8e8);
const GREY: u32 = rgb(0x888888);
const DARK: u32 = rgb(0x555555);
const GREEN: u32 = rgb(0x00e676);
const RED: u32 = rgb(0xff6b6b);
const AMBER: u32 = rgb(0xffd54f);
pub const CUES: [u32; 8] = [
    rgb(0xff4fa8),
    rgb(0xff3b30),
    rgb(0xff9500),
    rgb(0xffd60a),
    rgb(0x30d158),
    rgb(0x40c8e0),
    rgb(0x2f7bff),
    rgb(0xaf52de),
];
pub const DECK: [u32; 2] = [rgb(0x1f6fd1), rgb(0xe8650f)];

const LOGO: Style = Style::new(16.0, Weight::Bold, ACCENT);
const TAB: Style = Style::new(11.0, Weight::Bold, TEXT);
const TITLE: Style = Style::new(14.0, Weight::Semibold, TEXT);
const ARTIST: Style = Style::new(10.0, Weight::Regular, GREY);
const SMALL: Style = Style::new(9.0, Weight::Semibold, GREY);
const TIME: Style = Style::new(14.0, Weight::Bold, TEXT);
const PAD_TEXT: Style = Style::new(10.0, Weight::Bold, TEXT);

const TOOLBAR: f32 = 36.0;
const ROW: f32 = 44.0;
const MIXER_W: f32 = 190.0;

pub const fn fade(c: u32, a: u32) -> u32 {
    c & 0x00ff_ffff | a << 24
}

fn hit(s: &mut State, r: Rect, target: Target) {
    s.hits.push(Hit { rect: r, target });
}

fn button(p: &mut Painter, s: &mut State, r: Rect, label: &str, on: bool, colour: u32, target: Target) {
    let (fill, edge, ink) = if on { (fade(colour, 0x38), colour, TEXT) } else { (SURFACE, EDGE, GREY) };
    p.outlined(r.x, r.y, r.w, r.h, 5.0, fill, 1.0, edge);
    let style = Style::new(if r.h < 26.0 { 9.0 } else { 11.0 }, Weight::Bold, ink);
    let tw = p.text_width(style, label).min(r.w - 4.0);
    p.text_in(r.x + (r.w - tw) * 0.5, r.y + r.h * 0.5 + style.px * 0.36, r.w - 4.0, style, label);
    hit(s, r, target);
}

pub fn paint(p: &mut Painter, s: &mut State, w: f32, h: f32) {
    p.rect(0.0, 0.0, w, h, 0.0, rgb(0x000000));
    toolbar(p, s, Rect::new(0.0, 0.0, w, TOOLBAR));
    let mut y = TOOLBAR;
    if s.show_fx {
        effects(p, s, Rect::new(0.0, y, w, ROW));
        y += ROW;
    }
    if s.show_samplers {
        samplers(p, s, Rect::new(0.0, y, w, ROW));
        y += ROW;
    }
    let rest = h - y;
    match s.view {
        View::Dj => {
            let wh = (rest * 0.22).round().clamp(56.0, 96.0);
            waveforms(p, s, Rect::new(0.0, y, w, wh));
            let y = y + wh + 2.0;
            let lower = h - y;
            let dh = (lower * 0.64).clamp(150.0, 300.0).min(lower);
            let dw = ((w - MIXER_W) * 0.5).floor();
            deck(p, s, Rect::new(0.0, y, dw, dh), 0);
            mixer(p, s, Rect::new(dw, y, w - 2.0 * dw, dh));
            deck(p, s, Rect::new(w - dw, y, dw, dh), 1);
            if lower - dh > 60.0 {
                crate::screen::library(p, s, Rect::new(0.0, y + dh + 2.0, w, lower - dh - 2.0));
            }
        }
        View::Wave => {
            let bar = (rest * 0.38).round().clamp(86.0, 115.0);
            waveforms(p, s, Rect::new(0.0, y, w, rest - bar));
            for d in 0..2 {
                mini_deck(p, s, Rect::new(d as f32 * w * 0.5, h - bar, w * 0.5, bar), d);
            }
        }
        View::Lib => {
            for d in 0..2 {
                mini_deck(p, s, Rect::new(d as f32 * w * 0.5, y, w * 0.5, 70.0), d);
            }
            crate::screen::library(p, s, Rect::new(0.0, y + 72.0, w, rest - 72.0));
        }
        View::Ctl => {
            let wh = (rest * 0.42).round();
            waveforms(p, s, Rect::new(0.0, y, w, wh));
            crate::screen::library(p, s, Rect::new(0.0, y + wh + 2.0, w, rest - wh - 2.0));
        }
    }
    cue_menu(p, s, w, h);
    grid_menu(p, s, w, h);
}

fn toolbar(p: &mut Painter, s: &mut State, r: Rect) {
    p.rect(r.x, r.y, r.w, r.h, 0.0, INK);
    let mut x = r.x + 10.0 + p.text(r.x + 10.0, r.y + 23.0, LOGO, "DJ Sugar") + 12.0;
    let views = [("DJ", View::Dj), ("WAVE", View::Wave), ("LIB", View::Lib), ("CTL", View::Ctl)];
    let compact = r.w < 760.0;
    let tw = if compact { 40.0 } else { 54.0 };
    p.outlined(x, r.y + 4.0, tw * 4.0, r.h - 8.0, 6.0, DEEP, 1.0, EDGE);
    for (n, (label, v)) in views.into_iter().enumerate() {
        let b = Rect::new(x + n as f32 * tw, r.y + 4.0, tw, r.h - 8.0);
        if s.view == v {
            p.rect(b.x + 2.0, b.y + 2.0, b.w - 4.0, b.h - 4.0, 4.0, ACCENT);
        }
        let ink = Style { colour: if s.view == v { rgb(0x000000) } else { GREY }, ..TAB };
        p.text(b.x + (b.w - p.text_width(ink, label)) * 0.5, b.y + b.h * 0.5 + 4.0, ink, label);
        hit(s, b, Target::View(v));
    }
    x += tw * 4.0 + 8.0;
    let h = r.h - 8.0;
    let items: [(&str, bool, Target); 5] = [
        ("LOAD 1", s.browser == Some(0), Target::Browse(0)),
        ("LOAD 2", s.browser == Some(1), Target::Browse(1)),
        ("FX", s.show_fx, Target::ToggleFx),
        ("SMPL", s.show_samplers, Target::ToggleSamplers),
        ("AUTO", s.auto.is_some(), Target::AutoDj),
    ];
    for (label, on, t) in items {
        let bw = if label.len() > 4 { 58.0 } else { 46.0 };
        if x + bw > r.w - 140.0 {
            break;
        }
        button(p, s, Rect::new(x, r.y + 4.0, bw, h), label, on, ACCENT, t);
        x += bw + 4.0;
    }
    let sx = (r.w - 220.0).max(x + 4.0);
    let sw = r.w - sx - 8.0;
    p.outlined(sx, r.y + 5.0, sw, h - 2.0, 6.0, DEEP, 1.0, if s.query.is_empty() { EDGE } else { ACCENT });
    if s.query.is_empty() {
        p.text_in(sx + 10.0, r.y + 22.0, sw - 16.0, Style { colour: DARK, ..ARTIST }, "Search");
    } else {
        let qw = p.text_in(sx + 10.0, r.y + 22.0, sw - 50.0, Style { px: 12.0, ..TITLE }, &s.query);
        p.rect(sx + 11.0 + qw, r.y + 11.0, 1.5, 15.0, 0.0, ACCENT);
        let n = s.shown.len().to_string();
        p.text(sx + sw - 8.0 - p.text_width(SMALL, &n), r.y + 21.0, SMALL, &n);
    }
}

fn effects(p: &mut Painter, s: &mut State, r: Rect) {
    p.rect(r.x, r.y, r.w, r.h, 0.0, INK);
    let half = r.w * 0.5;
    for d in 0..2 {
        let x0 = r.x + d as f32 * half + 8.0;
        p.text(x0, r.y + 26.0, Style { colour: DECK[d], ..TAB }, ["FX 1", "FX 2"][d]);
        let bw = ((half - 60.0) / FX_PADS.len() as f32 - 4.0).max(24.0);
        for (i, label) in FX_PADS.iter().enumerate() {
            let on = s.decks[d].echo == Some(i);
            let b = Rect::new(x0 + 40.0 + i as f32 * (bw + 4.0), r.y + 6.0, bw, r.h - 12.0);
            let prev = s.decks[d].mode;
            s.decks[d].mode = Mode::Fx;
            button(p, s, b, label, on, ACCENT, Target::Pad(d, i));
            s.decks[d].mode = prev;
        }
    }
    p.rect(half, r.y + 6.0, 1.0, r.h - 12.0, 0.0, EDGE);
}

fn samplers(p: &mut Painter, s: &mut State, r: Rect) {
    p.rect(r.x, r.y, r.w, r.h, 0.0, INK);
    let n = ((r.w - 16.0) / 110.0).floor().clamp(4.0, 16.0) as usize;
    let bw = (r.w - 16.0) / n as f32 - 4.0;
    for i in 0..n {
        let name = crate::state::sample_name(s, i);
        let b = Rect::new(r.x + 8.0 + i as f32 * (bw + 4.0), r.y + 6.0, bw, r.h - 12.0);
        p.outlined(b.x, b.y, b.w, b.h, 5.0, SURFACE, 1.0, EDGE);
        p.rect(b.x, b.y + 4.0, 3.0, b.h - 8.0, 1.5, ACCENT);
        p.text(b.x + 8.0, b.y + 12.0, SMALL, &(i + 1).to_string());
        p.text_in(b.x + 8.0, b.y + b.h - 7.0, b.w - 12.0, PAD_TEXT, &name);
        hit(s, b, Target::Sampler(i));
    }
}

fn waveforms(p: &mut Painter, s: &mut State, r: Rect) {
    let h = (r.h / 2.0).floor();
    for d in 0..2 {
        scrolling(p, s, Rect::new(r.x, r.y + d as f32 * h, r.w, h - 1.0), d);
    }
}

fn scrolling(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    p.rect(r.x, r.y, r.w, r.h, 0.0, rgb(0x000000));
    let deck = &s.decks[d];
    let pos = s.telemetry.decks[d].position;
    let mid = r.y + r.h * 0.5;
    let speed = 1.0 + deck.rate * 0.08;
    let per_px = DETAIL_FRAMES as f64 * 0.5 * f64::from(speed);
    let left = pos - per_px * f64::from(r.w) * 0.5;
    if let Some(l) = &deck.loaded {
        if let Some((a, b)) = deck.loop_range {
            let x0 = (((a - left) / per_px) as f32).clamp(0.0, r.w);
            let x1 = (((b - left) / per_px) as f32).clamp(0.0, r.w);
            p.rect(r.x + x0, r.y, x1 - x0, r.h, 0.0, fade(GREEN, 0x26));
            p.rect(r.x + x0, r.y, 1.5, r.h, 0.0, GREEN);
            p.rect(r.x + x1 - 1.5, r.y, 1.5, r.h, 0.0, GREEN);
        }
        if let Some((g, beat)) = deck.beat() {
            let mut n = ((left - g.first_beat) / beat).ceil();
            loop {
                let f = g.first_beat + n * beat;
                let x = ((f - left) / per_px) as f32;
                if x > r.w {
                    break;
                }
                if f >= 0.0 {
                    let bar = n.rem_euclid(4.0) == 0.0;
                    p.rect(r.x + x, r.y, 1.0, r.h, 0.0, if bar { rgb(0x3a3a3a) } else { rgb(0x222222) });
                }
                n += 1.0;
            }
        }
        let bars = (r.w / 2.0) as usize;
        let centre = (pos / DETAIL_FRAMES as f64) as isize;
        for b in 0..bars {
            let i = centre + ((b as f32 - bars as f32 * 0.5) * speed) as isize;
            let Some(&[v, lo, md, hi]) = usize::try_from(i).ok().and_then(|i| l.detail.get(i)) else { continue };
            let a = f32::from(v) / 255.0 * r.h * 0.47;
            let sum = (u32::from(lo) + u32::from(md) + u32::from(hi)).max(1);
            let ch = |x: u8| (u32::from(x) * 255 / sum * 2).min(255);
            let alpha = if (b as f32) < bars as f32 * 0.5 { 0x80 } else { 0xff };
            let x = r.x + b as f32 * 2.0;
            p.rect(x, mid - a, 1.5, a * 2.0 + 0.5, 0.0, ch(lo) | ch(md) << 8 | ch(hi) << 16 | alpha << 24);
        }
        for (n, cue) in deck.cues.iter().enumerate() {
            let Some(c) = cue else { continue };
            let x = ((c - left) / per_px) as f32;
            if (0.0..r.w).contains(&x) {
                let colour = CUES[usize::from(deck.cue_colours[n]) % 8];
                p.rect(r.x + x, r.y, 1.5, r.h, 0.0, colour);
                p.rect(r.x + x, r.y, 12.0, 11.0, 2.0, colour);
                p.text(r.x + x + 3.0, r.y + 9.0, Style { colour: rgb(0x000000), ..SMALL }, &(n + 1).to_string());
            }
        }
    } else {
        let msg = if deck.loading { "Loading" } else { "No track loaded" };
        p.text(r.x + 12.0, mid + 4.0, Style { colour: DARK, ..ARTIST }, msg);
    }
    p.rect(r.x, r.y + r.h, r.w, 1.0, 0.0, rgb(0x0a0b10));
    p.rect(r.x + r.w * 0.5 - 1.0, r.y, 2.0, r.h, 0.0, TEXT);
    p.rect(r.x + 4.0, r.y + 4.0, 3.0, r.h - 8.0, 1.5, DECK[d]);
}

fn cue_colour(s: &State, d: usize, i: usize) -> u32 {
    CUES[usize::from(s.decks[d].cue_colours[i]) % 8]
}

fn cue_menu(p: &mut Painter, s: &mut State, w: f32, h: f32) {
    let Some((d, i, x, y)) = s.cue_menu else { return };
    p.layer();
    hit(s, Rect::new(0.0, 0.0, w, h), Target::CueMenuClose);
    let (cell, gap) = (30.0, 4.0);
    let pw = 4.0 * cell + 3.0 * gap + 16.0;
    let ph = 2.0 * cell + gap + 70.0;
    let r = Rect::new(x.clamp(4.0, w - pw - 4.0), y.clamp(4.0, h - ph - 4.0), pw, ph);
    p.outlined(r.x, r.y, r.w, r.h, 8.0, SURFACE, 1.5, cue_colour(s, d, i));
    let head = format!("HOT CUE {}", i + 1);
    p.text(r.x + 8.0, r.y + 18.0, TAB, &head);
    for (n, &c) in CUES.iter().enumerate() {
        let b = Rect::new(
            r.x + 8.0 + (n % 4) as f32 * (cell + gap),
            r.y + 26.0 + (n / 4) as f32 * (cell + gap),
            cell,
            cell,
        );
        let on = usize::from(s.decks[d].cue_colours[i]) == n;
        p.outlined(b.x, b.y, b.w, b.h, 5.0, c, if on { 2.0 } else { 0.0 }, TEXT);
        hit(s, b, Target::CueColour(n as u8));
    }
    let by = r.y + r.h - 34.0;
    button(p, s, Rect::new(r.x + 8.0, by, r.w - 54.0, 26.0), "CLEAR", false, RED, Target::CueClear);
    button(p, s, Rect::new(r.x + r.w - 40.0, by, 32.0, 26.0), "X", false, ACCENT, Target::CueMenuClose);
}

fn grid_menu(p: &mut Painter, s: &mut State, w: f32, h: f32) {
    let Some(d) = s.grid_menu else { return };
    p.layer();
    p.rect(0.0, 0.0, w, h, 0.0, fade(rgb(0x000000), 0x90));
    hit(s, Rect::new(0.0, 0.0, w, h), Target::GridClose);
    let pw = 340.0f32.min(w - 16.0);
    let row = 34.0;
    let ph = 54.0 + 5.0 * (row + 6.0) + 6.0;
    let r = Rect::new((w - pw) * 0.5, ((h - ph) * 0.5).max(4.0), pw, ph);
    p.outlined(r.x, r.y, r.w, r.h, 10.0, SURFACE, 1.5, DECK[d]);
    hit(s, r, Target::Grid(d));
    p.text(r.x + 12.0, r.y + 22.0, Style { colour: DECK[d], ..TAB }, &format!("BEATGRID  DECK {}", d + 1));
    let locked = s.decks[d].grid_locked;
    if let Some((g, _)) = s.decks[d].beat() {
        let bpm = format!("{:.2} BPM", g.bpm);
        p.text(r.x + r.w - 44.0 - p.text_width(TAB, &bpm), r.y + 22.0, Style { colour: AMBER, ..TAB }, &bpm);
    }
    button(p, s, Rect::new(r.x + r.w - 38.0, r.y + 6.0, 30.0, 24.0), "X", false, ACCENT, Target::GridClose);
    let hint = if locked { "Grid locked: unlock to edit" } else { "Move the grid until beat lines sit on the kicks" };
    p.text_in(r.x + 12.0, r.y + 44.0, r.w - 24.0, ARTIST, hint);
    let edits = !locked;
    let undo = edits && !s.decks[d].grid_undo.is_empty();
    let ops = [
        ("1/2 BEAT", Target::GridOp(GridOp::Half), edits),
        ("DOWNBEAT HERE", Target::GridOp(GridOp::Here), edits),
        ("EARLIER", Target::GridOp(GridOp::Earlier), edits),
        ("LATER", Target::GridOp(GridOp::Later), edits),
        ("BPM / 2", Target::GridOp(GridOp::Halve), edits),
        ("BPM x 2", Target::GridOp(GridOp::Double), edits),
        ("MATCH OTHER DECK", Target::GridOp(GridOp::Match), edits),
        ("UNDO", Target::GridOp(GridOp::Undo), undo),
        (if locked { "UNLOCK GRID" } else { "LOCK GRID" }, Target::GridOp(GridOp::Lock), true),
        ("CLOSE", Target::GridClose, true),
    ];
    let bw = (r.w - 30.0) * 0.5;
    for (n, (label, t, enabled)) in ops.into_iter().enumerate() {
        let b = Rect::new(r.x + 12.0 + (n % 2) as f32 * (bw + 6.0), r.y + 54.0 + (n / 2) as f32 * (row + 6.0), bw, row);
        let on = n == 8 && locked;
        button(p, s, b, label, on, if enabled { if n == 8 { AMBER } else { ACCENT } } else { DARK }, t);
    }
}

fn overview(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    p.rect(r.x, r.y, r.w, r.h, 3.0, DEEP);
    let deck = &s.decks[d];
    let Some(l) = &deck.loaded else { return };
    let len = deck.length().max(1.0);
    let played = (s.telemetry.decks[d].position / len) as f32;
    let n = l.overview.len().max(1);
    let cols = r.w.max(1.0) as usize;
    for c in 0..cols {
        let i = c * n / cols;
        let a = f32::from(l.overview[i]) / 255.0 * r.h * 0.48;
        let ink = if (c as f32) < played * cols as f32 { DARK } else { DECK[d] };
        p.rect(r.x + c as f32, r.y + r.h * 0.5 - a, 1.0, a * 2.0 + 0.5, 0.0, ink);
    }
    for (cue, &c) in deck.cues.iter().zip(&deck.cue_colours) {
        if let Some(cue) = cue {
            p.rect(r.x + (cue / len) as f32 * r.w, r.y, 1.0, r.h, 0.0, CUES[usize::from(c) % 8]);
        }
    }
    p.rect(r.x + r.w * played - 1.0, r.y, 2.0, r.h, 1.0, TEXT);
    hit(s, r, Target::Overview(d));
}

fn clock(secs: f64) -> String {
    let t = secs.max(0.0);
    format!("{:02}:{:04.1}", (t / 60.0) as u32, t % 60.0)
}

fn info(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    let deck = &s.decks[d];
    let t = s.telemetry.decks[d];
    let art = r.h;
    p.rect(r.x, r.y, art, art, 4.0, fade(DECK[d], 0x40));
    let num = (d + 1).to_string();
    let big = Style::new(art * 0.42, Weight::Bold, DECK[d]);
    p.text(r.x + (art - p.text_width(big, &num)) * 0.5, r.y + art * 0.66, big, &num);
    let right = 150.0f32.min(r.w * 0.4);
    let tx = r.x + art + 8.0;
    let tw = r.w - art - right - 16.0;
    match &deck.loaded {
        Some(l) => {
            p.text_in(tx, r.y + r.h * 0.42, tw, TITLE, &l.entry.title);
            p.text_in(tx, r.y + r.h * 0.42 + 14.0, tw, ARTIST, &l.entry.folder);
            let remain = deck.seconds(deck.length() - t.position);
            let rx = r.x + r.w - right;
            let time = format!("-{}", clock(remain));
            p.text(r.x + r.w - 4.0 - p.text_width(TIME, &time), r.y + r.h * 0.42, TIME, &time);
            let mut parts = Vec::with_capacity(3);
            if let Some(g) = l.grid {
                parts.push(format!("{:.2} BPM", g.bpm * (1.0 + f64::from(deck.rate) * 0.08)));
            }
            if let Some(k) = l.key {
                parts.push(k.camelot());
            }
            parts.push(format!("{:+.1}%", deck.rate * 8.0));
            let line = parts.join("  ");
            let lw = p.text_width(SMALL, &line);
            p.text_in(
                (r.x + r.w - 4.0 - lw).max(rx),
                r.y + r.h * 0.42 + 14.0,
                right,
                Style { colour: AMBER, ..SMALL },
                &line,
            );
        }
        None => {
            let msg = if deck.loading { "Loading" } else { "Tap LOAD to pick a track" };
            p.text_in(tx, r.y + r.h * 0.5 + 4.0, tw, Style { colour: DARK, ..ARTIST }, msg);
        }
    }
    hit(s, Rect::new(r.x, r.y, r.w - right, r.h), Target::Browse(d));
    hit(s, Rect::new(r.x + r.w - right, r.y, right, r.h), Target::Grid(d));
}

fn deck(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    p.rect(r.x, r.y, r.w, r.h, 0.0, rgb(0x000000));
    let r = Rect::new(r.x + 6.0, r.y + 4.0, r.w - 12.0, r.h - 8.0);
    let info_h = 44.0f32.min(r.h * 0.22);
    info(p, s, Rect::new(r.x, r.y, r.w, info_h), d);
    let y = r.y + info_h + 4.0;
    let tempo_w = if r.w >= 220.0 { if r.w < 320.0 { 52.0 } else { 64.0 } } else { 0.0 };
    let body = Rect::new(r.x, y, r.w - tempo_w - if tempo_w > 0.0 { 4.0 } else { 0.0 }, r.y + r.h - y);
    if tempo_w > 0.0 {
        tempo(p, s, Rect::new(r.x + r.w - tempo_w, y, tempo_w, body.h), d);
    }
    let ov = 22.0f32.min(body.h * 0.16);
    let tw = (body.w * 0.48).min(180.0);
    overview(p, s, Rect::new(body.x, body.y, body.w - tw - 2.0, ov), d);
    tools(p, s, Rect::new(body.x + body.w - tw, body.y, tw, ov), d);
    let y = body.y + ov + 4.0;
    let h = body.y + body.h - y;
    let pc = (h * 0.5 - 2.0).clamp(26.0, 60.0);
    let pw = pc.max(44.0);
    transport(p, s, Rect::new(body.x, y, pw, h), d);
    let spinny = body.w >= 800.0;
    let sw = if spinny { h.min(body.w * 0.22) } else { 0.0 };
    let cx = body.x + pw + 4.0;
    let cw = body.w - pw - 4.0 - if spinny { sw + 4.0 } else { 0.0 };
    controls(p, s, Rect::new(cx, y, cw, h), d);
    if spinny {
        platter(p, s, Rect::new(body.x + body.w - sw, y, sw, h), d);
    }
}

fn transport(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    let playing = s.telemetry.decks[d].playing;
    let bh = (r.h - 4.0) * 0.5;
    let play = Rect::new(r.x, r.y, r.w, bh);
    p.outlined(
        play.x,
        play.y,
        play.w,
        play.h,
        6.0,
        if playing { fade(GREEN, 0x30) } else { SURFACE },
        1.5,
        if playing { GREEN } else { EDGE },
    );
    let (cx, cy, k) = (play.x + play.w * 0.5, play.y + play.h * 0.5, (bh * 0.22).min(12.0));
    if playing {
        p.rect(cx - k * 0.8, cy - k, k * 0.55, k * 2.0, 1.0, GREEN);
        p.rect(cx + k * 0.25, cy - k, k * 0.55, k * 2.0, 1.0, GREEN);
    } else {
        for i in 0..(k * 2.0) as i32 {
            let fy = i as f32;
            let len = (k * 1.7) * (1.0 - (fy - k).abs() / k);
            p.rect(cx - k * 0.7, cy - k + fy, len, 1.0, 0.0, TEXT);
        }
    }
    hit(s, play, Target::Play(d));
    let cue = Rect::new(r.x, r.y + bh + 4.0, r.w, bh);
    p.outlined(cue.x, cue.y, cue.w, cue.h, 6.0, SURFACE, 1.5, AMBER);
    let cs = Style { colour: AMBER, ..TAB };
    p.text(cue.x + (cue.w - p.text_width(cs, "CUE")) * 0.5, cue.y + cue.h * 0.5 + 4.0, cs, "CUE");
    hit(s, cue, Target::Cue(d));
}

fn tools(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    let tools: [(&str, bool, u32, Target); 4] = [
        ("REV", s.decks[d].reverse, ACTIVE, Target::Reverse(d)),
        ("SLIP", s.decks[d].slip, ACTIVE, Target::Slip(d)),
        ("Q", s.decks[d].quantize, ACTIVE, Target::Quantize(d)),
        ("EJECT", false, RED, Target::Eject(d)),
    ];
    let weights = [1.0, 1.1, 0.7, 1.3];
    let unit = (r.w - 8.0) / weights.iter().sum::<f32>();
    let mut x = r.x + 2.0;
    for ((label, on, colour, t), w) in tools.into_iter().zip(weights) {
        button(p, s, Rect::new(x, r.y, unit * w, r.h), label, on, colour, t);
        x += unit * w + 2.0;
    }
}

fn controls(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    let side = if r.w >= 200.0 { (r.w * 0.34).clamp(76.0, 104.0) } else { 0.0 };
    let pw = r.w - if side > 0.0 { side + 4.0 } else { 0.0 };
    let bar = 22.0;
    let modes = [("HOT", Mode::Cues), ("LOOP", Mode::Loops), ("SMPL", Mode::Samples), ("FX", Mode::Fx)];
    let mw = pw / 4.0 - 2.0;
    for (n, (label, m)) in modes.into_iter().enumerate() {
        let b = Rect::new(r.x + n as f32 * (mw + 2.0), r.y, mw, bar);
        button(p, s, b, label, s.decks[d].mode == m, ACCENT, Target::Mode(d, m));
    }
    pads(p, s, Rect::new(r.x, r.y + bar + 3.0, pw - 2.0, r.h - bar - 3.0), d);
    if side > 0.0 {
        loops(p, s, Rect::new(r.x + r.w - side, r.y, side, r.h), d);
    }
}
fn pads(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    let (cols, rows) = if r.h < 50.0 { (8, 1) } else { (4, 2) };
    let pw = (r.w - (cols - 1) as f32 * 3.0) / cols as f32;
    let ph = (r.h - (rows - 1) as f32 * 3.0) / rows as f32;
    let mode = s.decks[d].mode;
    for i in 0..cols * rows {
        let b = Rect::new(r.x + (i % cols) as f32 * (pw + 3.0), r.y + (i / cols) as f32 * (ph + 3.0), pw, ph);
        let (label, on, colour) = match mode {
            Mode::Cues => ((i + 1).to_string(), s.decks[d].cues[i].is_some(), cue_colour(s, d, i)),
            Mode::Loops => (beats_label(LOOP_BEATS[i]), s.decks[d].looping == Some(i), GREEN),
            Mode::Samples => (crate::state::pad_name(s, d, i), false, ACTIVE),
            Mode::Fx => (FX_PADS[i].to_string(), s.decks[d].echo == Some(i), AMBER),
        };
        let fill = if on { fade(colour, 0xc0) } else { SURFACE };
        p.outlined(b.x, b.y, b.w, b.h, 5.0, fill, 1.0, if on { colour } else { fade(colour, 0x60) });
        let ink = Style { colour: if on { rgb(0x000000) } else { TEXT }, ..PAD_TEXT };
        let tw = p.text_width(ink, &label).min(b.w - 6.0);
        p.text_in(b.x + (b.w - tw) * 0.5, b.y + b.h * 0.5 + 4.0, b.w - 6.0, ink, &label);
        hit(s, b, Target::Pad(d, i));
    }
}

fn stepper(
    p: &mut Painter,
    s: &mut State,
    r: Rect,
    value: &str,
    less: (&str, Target),
    more: (&str, Target),
    tap: Option<Target>,
) {
    let bw = (r.w * 0.3).min(r.h + 4.0);
    button(p, s, Rect::new(r.x, r.y, bw, r.h), less.0, false, ACCENT, less.1);
    button(p, s, Rect::new(r.x + r.w - bw, r.y, bw, r.h), more.0, false, ACCENT, more.1);
    let mid = Rect::new(r.x + bw + 2.0, r.y, r.w - 2.0 * bw - 4.0, r.h);
    p.rect(mid.x, mid.y, mid.w, mid.h, 4.0, DEEP);
    let tw = p.text_width(PAD_TEXT, value);
    p.text(mid.x + (mid.w - tw) * 0.5, mid.y + mid.h * 0.5 + 4.0, PAD_TEXT, value);
    if let Some(t) = tap {
        hit(s, mid, t);
    }
}

fn loops(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    let row = ((r.h - 9.0) / 4.0).min(34.0);
    let gap = 3.0;
    let hw = (r.w - gap) * 0.5;
    let armed = s.decks[d].loop_in.is_some();
    button(p, s, Rect::new(r.x, r.y, hw, row), "IN", armed, GREEN, Target::LoopIn(d));
    button(p, s, Rect::new(r.x + hw + gap, r.y, hw, row), "OUT", false, GREEN, Target::LoopOut(d));
    let y = r.y + row + gap;
    let size = beats_label(LOOP_BEATS[s.decks[d].loop_size]);
    stepper(
        p,
        s,
        Rect::new(r.x, y, r.w, row),
        &size,
        ("-", Target::LoopSize(d, -1)),
        ("+", Target::LoopSize(d, 1)),
        None,
    );
    let on = s.decks[d].loop_range.is_some();
    let y = y + row + gap;
    button(p, s, Rect::new(r.x, y, r.w, row), if on { "EXIT LOOP" } else { "LOOP" }, on, GREEN, Target::LoopToggle(d));
    let y = y + row + gap;
    let size = beats_label(JUMP_BEATS[s.decks[d].jump]);
    stepper(
        p,
        s,
        Rect::new(r.x, y, r.w, row),
        &size,
        ("<", Target::Jump(d, -1)),
        (">", Target::Jump(d, 1)),
        Some(Target::JumpSize(d, 0)),
    );
}

fn tempo(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    p.rect(r.x, r.y, r.w, r.h, 5.0, SURFACE);
    let bh = 22.0;
    button(p, s, Rect::new(r.x + 3.0, r.y + 3.0, r.w - 6.0, bh), "SYNC", false, ACCENT, Target::Sync(d));
    let kl = s.decks[d].key_lock;
    let bottom = r.y + r.h - 3.0;
    let st = s.decks[d].semitones;
    let hw = (r.w - 8.0) * 0.5;
    button(p, s, Rect::new(r.x + 3.0, bottom - bh, hw, bh), "-", false, ACCENT, Target::Pitch(d, -1));
    button(p, s, Rect::new(r.x + 5.0 + hw, bottom - bh, hw, bh), "+", false, ACCENT, Target::Pitch(d, 1));
    let key = if st == 0 { "KEY".to_string() } else { format!("{st:+}") };
    button(
        p,
        s,
        Rect::new(r.x + 3.0, bottom - 2.0 * bh - 3.0, r.w - 6.0, bh),
        &key,
        kl || st != 0,
        ACTIVE,
        Target::KeyLock(d),
    );
    let ft = r.y + bh + 10.0;
    let fh = bottom - 2.0 * bh - 10.0 - ft;
    if fh < 30.0 {
        return;
    }
    let cx = r.x + r.w * 0.5;
    p.rect(cx - 1.5, ft, 3.0, fh, 1.5, DEEP);
    p.rect(cx - 8.0, ft + fh * 0.5, 16.0, 1.0, 0.0, DARK);
    let v = s.decks[d].rate;
    let y = ft + (1.0 - (v + 1.0) * 0.5) * (fh - 10.0);
    p.rect(r.x + 6.0, y, r.w - 12.0, 10.0, 3.0, if v == 0.0 { TEXT } else { ACCENT });
    hit(s, Rect::new(r.x, ft, r.w, fh), Target::Rate(d));
}

fn platter(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    let k = r.w.min(r.h);
    let (x, y) = (r.x + (r.w - k) * 0.5, r.y + (r.h - k) * 0.5);
    p.outlined(x, y, k, k, k * 0.5, DEEP, 2.0, EDGE);
    let pos = s.telemetry.decks[d].position;
    let secs = s.decks[d].seconds(pos);
    let a = (secs * std::f64::consts::TAU / 1.8) as f32;
    let (sn, cs) = a.sin_cos();
    let rr = k * 0.42;
    let (cx, cy) = (x + k * 0.5, y + k * 0.5);
    p.rect(cx + sn * rr - 3.0, cy - cs * rr - 3.0, 6.0, 6.0, 3.0, DECK[d]);
    p.outlined(cx - k * 0.12, cy - k * 0.12, k * 0.24, k * 0.24, k * 0.12, SURFACE, 1.0, EDGE);
    hit(s, Rect::new(x, y, k, k), Target::Jog(d));
}

fn mini_deck(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    p.rect(r.x, r.y, r.w, r.h, 0.0, INK);
    let r = Rect::new(r.x + 4.0, r.y + 4.0, r.w - 8.0, r.h - 8.0);
    let bw = 48.0f32.min(r.h);
    transport(p, s, Rect::new(r.x, r.y, bw, r.h), d);
    let rest = Rect::new(r.x + bw + 6.0, r.y, r.w - bw - 6.0, r.h);
    let ih = (rest.h * 0.55).min(40.0);
    info(p, s, Rect::new(rest.x, rest.y, rest.w, ih), d);
    if rest.h - ih > 14.0 {
        overview(p, s, Rect::new(rest.x, rest.y + ih + 4.0, rest.w, rest.h - ih - 4.0), d);
    }
}

fn knob(p: &mut Painter, s: &mut State, r: Rect, v: f32, colour: u32, label: &str, target: Target) {
    let k = r.w.min(r.h - 10.0);
    let (x, y) = (r.x + (r.w - k) * 0.5, r.y);
    p.outlined(x, y, k, k, k * 0.5, DEEP, 1.5, EDGE);
    let steps = 24;
    let on = (v - 0.5).abs() >= 0.01;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let lit = on && ((t >= 0.5 && t <= v) || (t <= 0.5 && t >= v));
        let a = (t - 0.5) * 1.5 * std::f32::consts::PI;
        let (sn, cs) = a.sin_cos();
        let rr = k * 0.5 - 2.0;
        p.rect(
            x + k * 0.5 + sn * rr - 1.0,
            y + k * 0.5 - cs * rr - 1.0,
            2.0,
            2.0,
            1.0,
            if lit { colour } else { rgb(0x222222) },
        );
    }
    let a = (v - 0.5) * 1.5 * std::f32::consts::PI;
    let (sn, cs) = a.sin_cos();
    let rr = k * 0.28;
    p.rect(x + k * 0.5 + sn * rr - 2.0, y + k * 0.5 - cs * rr - 2.0, 4.0, 4.0, 2.0, TEXT);
    let lw = p.text_width(SMALL, label);
    p.text(r.x + (r.w - lw) * 0.5, y + k + 9.0, SMALL, label);
    hit(s, r, target);
}

fn mixer(p: &mut Painter, s: &mut State, r: Rect) {
    p.rect(r.x, r.y, r.w, r.h, 0.0, INK);
    let xf = 34.0;
    let body = Rect::new(r.x + 4.0, r.y + 4.0, r.w - 8.0, r.h - xf - 8.0);
    let cw = (body.w - 9.0) / 4.0;
    let labels = ["HI", "MID", "LOW", "FILT"];
    for d in 0..2 {
        let eq = Rect::new(if d == 0 { body.x } else { body.x + body.w - cw }, body.y, cw, body.h);
        let kh = (eq.h / 4.0).min(cw + 10.0);
        for (n, label) in labels.iter().enumerate() {
            let v = s.decks[d].knobs[n];
            let colour = if n == 3 { ACCENT } else { TEXT };
            knob(p, s, Rect::new(eq.x, eq.y + n as f32 * kh, eq.w, kh), v, colour, label, Target::Knob(d, n));
        }
        let ch = Rect::new(body.x + (cw + 3.0) * (1 + d) as f32, body.y, cw, body.h);
        let kh = (cw + 10.0).min(ch.h * 0.3);
        let g = s.decks[d].gain;
        knob(p, s, Rect::new(ch.x, ch.y, ch.w, kh), g, ACCENT, "GAIN", Target::Gain(d));
        let pfl = 20.0;
        let ft = ch.y + kh + 4.0;
        let fh = ch.y + ch.h - pfl - 4.0 - ft;
        let peak = s.telemetry.decks[d].peak.min(1.0);
        let mx = ch.x + ch.w - 6.0;
        p.rect(mx, ft, 4.0, fh, 2.0, DEEP);
        for seg in 0..((fh / 4.0) as usize) {
            let t = seg as f32 * 4.0 / fh;
            if t > peak {
                break;
            }
            let c = if t > 0.92 {
                RED
            } else if t > 0.75 {
                AMBER
            } else {
                GREEN
            };
            p.rect(mx, ft + fh - (seg as f32 + 1.0) * 4.0 + 1.0, 4.0, 3.0, 0.0, c);
        }
        let fx = ch.x + (ch.w - 6.0) * 0.5;
        p.rect(fx - 1.5, ft, 3.0, fh, 1.5, DEEP);
        let v = s.decks[d].volume;
        p.rect(fx - 10.0, ft + (1.0 - v) * (fh - 10.0), 20.0, 10.0, 3.0, TEXT);
        hit(s, Rect::new(ch.x, ft, ch.w, fh), Target::Volume(d));
        let on = s.decks[d].pfl;
        button(p, s, Rect::new(ch.x, ch.y + ch.h - pfl, ch.w, pfl), "PFL", on, ACCENT, Target::Pfl(d));
    }
    let y = r.y + r.h - xf;
    let w = r.w - 24.0;
    p.rect(r.x + 12.0, y + xf * 0.5 - 2.0, w, 4.0, 2.0, DEEP);
    let t = r.x + 12.0 + (s.crossfader + 1.0) * 0.5 * (w - 14.0);
    p.rect(t, y + 6.0, 14.0, xf - 12.0, 3.0, ACCENT);
    hit(s, Rect::new(r.x + 6.0, y, r.w - 12.0, xf), Target::Crossfader);
}

fn mmss(secs: f32) -> String {
    let t = secs.max(0.0) as u32;
    format!("{}:{:02}", t / 60, t % 60)
}

pub fn library(p: &mut Painter, s: &mut State, r: Rect) {
    use crate::library::Column as C;
    p.rect(r.x, r.y, r.w, r.h, 0.0, rgb(0x000000));
    let bar = if s.selected.is_some() { 40.0 } else { 0.0 };
    let list = Rect::new(r.x, r.y, r.w, r.h - bar);
    s.library = list;
    s.row_h = 22.0;
    let wide = r.w >= 700.0;
    let cols: &[(C, &str, f32)] = if wide {
        &[
            (C::Index, "#", 0.05),
            (C::Title, "TITLE", 0.37),
            (C::Artist, "ARTIST", 0.27),
            (C::Bpm, "BPM", 0.1),
            (C::Key, "KEY", 0.08),
            (C::Time, "TIME", 0.13),
        ]
    } else {
        &[
            (C::Index, "#", 0.07),
            (C::Title, "TITLE", 0.5),
            (C::Bpm, "BPM", 0.15),
            (C::Key, "KEY", 0.12),
            (C::Time, "TIME", 0.16),
        ]
    };
    let inner = r.w - 24.0;
    let mut x = r.x + 12.0;
    let xs: Vec<f32> = cols
        .iter()
        .map(|&(_, _, f)| {
            let at = x;
            x += inner * f;
            at
        })
        .collect();
    for (&(col, name, f), &cx) in cols.iter().zip(&xs) {
        let on = s.sort.0 == col;
        let style = Style { colour: if on { ACCENT } else { GREY }, ..SMALL };
        let w = p.text(cx, r.y + 19.0, style, name);
        if on {
            p.text(cx + w + 4.0, r.y + 19.0, style, if s.sort.1 { "^" } else { "v" });
        }
        hit(s, Rect::new(cx - 4.0, r.y, inner * f, 28.0), Target::Sort(col));
    }
    p.rect(r.x + 8.0, r.y + 28.0, r.w - 16.0, 1.0, 0.0, EDGE);
    let row = s.row_h;
    let first = (s.scroll / row) as usize;
    let rows = ((list.h - 30.0) / row) as usize + 1;
    hit(s, Rect::new(list.x, list.y + 29.0, list.w, list.h - 29.0), Target::Library);
    let cell = Style::new(12.0, Weight::Regular, TEXT);
    for (n, &i) in s.shown.iter().skip(first).take(rows).enumerate() {
        let y = r.y + 30.0 + n as f32 * row - (s.scroll % row);
        if y < r.y + 29.0 || y + row > list.y + list.h {
            continue;
        }
        let e = &s.entries[i as usize];
        let deck = s.decks.iter().position(|d| d.loaded.as_ref().is_some_and(|l| l.entry.source == e.source));
        if s.selected == Some(i) {
            p.outlined(r.x + 6.0, y, r.w - 12.0, row, 4.0, fade(ACCENT, 0x30), 1.0, ACCENT);
        } else if let Some(d) = deck {
            p.rect(r.x + 6.0, y, r.w - 12.0, row, 4.0, fade(DECK[d.min(1)], 0x50));
        } else if n % 2 == 1 {
            p.rect(r.x + 6.0, y, r.w - 12.0, row, 0.0, rgb(0x08090c));
        }
        let m = s.meta.get(i as usize).copied().flatten();
        let base = y + row - 6.0;
        for (&(col, _, f), &cx) in cols.iter().zip(&xs) {
            let w = inner * f - 8.0;
            match col {
                C::Index => p.text(cx, base, SMALL, &(first + n + 1).to_string()),
                C::Title => p.text_in(cx, base, w, cell, &e.title),
                C::Artist => p.text_in(
                    cx,
                    base,
                    w,
                    Style { colour: GREY, ..cell },
                    if e.artist.is_empty() { &e.folder } else { &e.artist },
                ),
                C::Bpm => p.text(
                    cx,
                    base,
                    Style { colour: AMBER, ..cell },
                    &m.filter(|m| m.bpm > 0.0).map_or(String::new(), |m| format!("{:.1}", m.bpm)),
                ),
                C::Key => p.text(
                    cx,
                    base,
                    Style { colour: GREEN, ..cell },
                    &m.and_then(|m| m.key).map_or(String::new(), |k| k.camelot()),
                ),
                C::Time => p.text(cx, base, Style { colour: GREY, ..cell }, &m.map_or(String::new(), |m| mmss(m.secs))),
            };
        }
    }
    if s.shown.is_empty() {
        p.text(r.x + 12.0, r.y + 52.0, ARTIST, "No matches");
    }
    let Some(i) = s.selected else { return };
    let y = r.y + r.h - bar;
    p.rect(r.x, y, r.w, bar, 0.0, INK);
    let name = &s.entries[i as usize].title;
    let bw = 96.0f32.min(r.w * 0.2);
    p.text_in(r.x + 12.0, y + 25.0, r.w - 2.0 * bw - 36.0, TITLE, name);
    for d in 0..2 {
        let b = Rect::new(r.x + r.w - (2 - d) as f32 * (bw + 6.0), y + 6.0, bw, bar - 12.0);
        p.rect(b.x, b.y, b.w, b.h, 5.0, DECK[d]);
        let label = ["DECK 1", "DECK 2"][d];
        p.text(b.x + (b.w - p.text_width(TAB, label)) * 0.5, b.y + b.h * 0.5 + 4.0, TAB, label);
        hit(s, b, Target::LoadSelected(d));
    }
}
