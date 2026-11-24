//! DJ Sugar's Classic phone skin, the default on Android: two decks either
//! side of a fixed size mixer, each deck an info bar, overview, transport and pads.

use sugar_layout::Rect;
use sugar_ui::{Painter, Style, Weight, rgb};

use crate::state::{FX_PADS, LOOP_BEATS, Mode, State, Target, beats_label, pad_name};
use crate::sugar::{DECK, cue_colour, fade, hit};
use crate::theme::{
    BACKDROP, BORDER, CONTROL, DIM, FAINT, PANEL, PRESSED, RAISED, SEPARATOR, TEXT as WHITE, WELL, centred,
};

const ACCENT: u32 = rgb(0xb69cff);
const KEY: u32 = rgb(0x8e7cff);
const CUE: u32 = rgb(0xff9f0a);
const LOOP_ON: u32 = rgb(0xb69cff);
const LINE: u32 = rgb(0x3a3a40);
const LABEL: u32 = rgb(0x8b8b93);

/// The mixer keeps this size; the decks share what is left.
const MIXER: (f32, f32) = (190.0, 260.0);

fn bold(px: f32, colour: u32) -> Style {
    Style::new(px.round().max(7.0), Weight::Bold, colour)
}

/// The deck button: a flat control, lit in its colour when on.
fn button(p: &mut Painter, s: &mut State, r: Rect, label: &str, (on, colour): (bool, u32), t: Target) {
    let down = s.pressed(t);
    crate::theme::labelled(p, r, label, 10.0, on, down, colour);
    hit(s, r, t);
}

pub fn dj(p: &mut Painter, s: &mut State, r: Rect, stacked: bool) {
    p.rect(r.x, r.y, r.w, r.h, 0.0, BACKDROP);
    if stacked || r.w < MIXER.0 * 3.2 {
        // Portrait: decks stacked full width, the mixer between them.
        let mh = MIXER.1.min(r.h * 0.34).round();
        let dh = ((r.h - mh - 2.0) * 0.5).floor();
        deck(p, s, Rect::new(r.x, r.y, r.w, dh), 0, false);
        mixer(p, s, Rect::new(r.x, r.y + dh + 1.0, r.w, mh));
        deck(p, s, Rect::new(r.x, r.y + r.h - dh, r.w, dh), 1, false);
    } else {
        let mw = MIXER.0;
        let dw = ((r.w - mw - 2.0) * 0.5).floor();
        deck(p, s, Rect::new(r.x, r.y, dw, r.h), 0, false);
        mixer(p, s, Rect::new(r.x + dw + 1.0, r.y, mw, r.h));
        deck(p, s, Rect::new(r.x + r.w - dw, r.y, dw, r.h), 1, true);
    }
}

/// The deck height in a band of the given height: the mixer's, or less.
pub fn height(avail: f32) -> f32 {
    MIXER.1.min(avail)
}

/// Lays out boxes along a row from the deck's outer edge: left to right for
/// deck 1, right to left for deck 2, so the decks mirror around the mixer.
struct Row {
    r: Rect,
    from: f32,
    mirrored: bool,
}

impl Row {
    fn new(r: Rect, mirrored: bool) -> Self {
        Row { r, from: 0.0, mirrored }
    }

    fn take(&mut self, w: f32, gap: f32) -> Rect {
        let x = if self.mirrored { self.r.x + self.r.w - self.from - w } else { self.r.x + self.from };
        self.from += w + gap;
        Rect::new(x, self.r.y, w, self.r.h)
    }

    fn left(&self) -> f32 {
        self.r.w - self.from
    }
}

fn deck(p: &mut Painter, s: &mut State, r: Rect, d: usize, mirrored: bool) {
    p.rect(r.x, r.y, r.w, r.h, 0.0, PANEL);
    let hh = (r.h * 0.36).clamp(52.0, 96.0).round();
    header(p, s, Rect::new(r.x, r.y, r.w, hh), d, mirrored);
    p.rect(r.x, r.y + hh, r.w, 1.0, 0.0, SEPARATOR);
    let body = Rect::new(r.x + 8.0, r.y + hh + 8.0, r.w - 16.0, r.h - hh - 16.0);
    if body.h < 60.0 {
        return;
    }
    let mut row = Row::new(body, mirrored);
    let sc = row.take(56.0_f32.min(body.w * 0.12).max(44.0), 8.0);
    tempo(p, s, sc, d);
    // The jog takes the room its height allows, after the pads and the loop box.
    let jog = (body.h - 36.0).min(body.w * 0.3).max(60.0);
    let loop_w = if row.left() - jog > 330.0 { 108.0 } else { 0.0 };
    let pads_w = (row.left() - jog - loop_w - 16.0).max(120.0);
    let pr = row.take(pads_w, 8.0);
    pads(p, s, pr, d);
    if loop_w > 0.0 {
        let lr = row.take(loop_w, 8.0);
        loop_box(p, s, lr, d);
    }
    let jr = row.take(row.left().max(jog), 0.0);
    jog_and_transport(p, s, jr, d);
}

/// The header: a cover tile, title and artist over the overview, the time
/// left and the key.
fn header(p: &mut Painter, s: &mut State, r: Rect, d: usize, mirrored: bool) {
    let mut row = Row::new(Rect::new(r.x + 6.0, r.y + 6.0, r.w - 12.0, r.h - 12.0), mirrored);
    let art = row.take(r.h - 12.0, 10.0);
    let loaded = s.decks[d].loaded.is_some();
    cover(p, art, d, loaded);
    if loaded {
        // Tapping the cover ejects the track.
        hit(s, art, Target::Eject(d));
    }
    let key_w = 64.0_f32.min(r.w * 0.14);
    let time_w = 82.0_f32.min(r.w * 0.18);
    let text_w = (row.left() - key_w - time_w - 20.0).max(40.0);
    let text = row.take(text_w, 10.0);
    let time = row.take(time_w, 10.0);
    let key = row.take(key_w, 0.0);
    let deck = &s.decks[d];
    let ov_h = (text.h * 0.42).round();
    let line = text.h - ov_h - 6.0;
    let tp = (line * 0.42).clamp(10.0, 17.0);
    let (title, sub) = match &deck.loaded {
        Some(l) => {
            let bpm = l.grid.map(|g| g.bpm * (1.0 + f64::from(deck.rate) * s.settings.rate_range()));
            let bpm = bpm.map_or_else(String::new, |b| format!("  |  {b:.2}"));
            (l.entry.title.to_string(), format!("{}{bpm}", l.entry.artist))
        }
        None if deck.loading => ("Loading".to_string(), String::new()),
        None => ("No track".to_string(), "Hold a track in the library to load it".to_string()),
    };
    let ink = if deck.loaded.is_some() { WHITE } else { DIM };
    p.text_in(text.x, text.y + tp, text.w, Style::new(tp, Weight::Semibold, ink), &title);
    let sp = (tp * 0.78).round().max(9.0);
    p.text_in(text.x, text.y + tp + sp + 4.0, text.w, Style::new(sp, Weight::Regular, DIM), &sub);
    // The overview runs under the title and on under the time.
    let ov = Rect::new(text.x.min(time.x), text.y + text.h - ov_h, text.w + time.w + 10.0, ov_h);
    let loaded = deck.loaded.is_some();
    if loaded {
        overview(p, s, ov, d);
    } else {
        p.rect(ov.x, ov.y, ov.w, ov.h, 4.0, WELL);
    }
    let deck = &s.decks[d];
    // Time left, large, and the length under it.
    if loaded {
        let left = deck.seconds((deck.length() - s.telemetry.decks[d].position).max(0.0));
        let big = bold((line * 0.5).clamp(12.0, 24.0), if left < 30.0 { CUE } else { WHITE });
        let t = format!("-{}", mmss(left));
        let w = p.text_width(big, &t);
        p.text(time.x + time.w - w, time.y + big.px, big, &t);
        let small = Style::new((big.px * 0.45).max(8.0).round(), Weight::Regular, DIM);
        let total = mmss(deck.seconds(deck.length()));
        let w = p.text_width(small, &total);
        p.text(time.x + time.w - w, time.y + big.px + small.px + 4.0, small, &total);
    }
    key_box(p, s, Rect::new(key.x, key.y, key.w, line), d);
}

/// A square standing in for the cover: the deck's colour fading to black, its number.
fn cover(p: &mut Painter, r: Rect, d: usize, loaded: bool) {
    let top = if loaded { fade(DECK[d], 0xb0) } else { RAISED };
    p.gradient(r.x, r.y, r.w, r.h, 4.0, top, rgb(0x101012), 1.0, BORDER);
    centred(p, r, bold(r.h * 0.42, fade(WHITE, if loaded { 0xe0 } else { 0x40 })), if d == 0 { "1" } else { "2" });
}

/// The key in its own column, with the semitone shift and the sync leader's badge.
fn key_box(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    p.rect(r.x - 6.0, r.y, 1.0, r.h, 0.0, SEPARATOR);
    let deck = &s.decks[d];
    let st = deck.semitones;
    let key = deck.loaded.as_ref().and_then(|l| l.key).map_or_else(|| "--".to_string(), |k| s.settings.key(k));
    let kp = (r.h * 0.34).clamp(11.0, 17.0);
    let line = Rect::new(r.x, r.y, r.w, kp + 6.0);
    centred(p, line, bold(kp, KEY), &key);
    let t = s.telemetry.decks[d];
    let by = line.y + line.h + 2.0;
    let bh = (r.h - line.h - 2.0).clamp(12.0, 20.0);
    if t.sync_leader > 0 {
        let b = Rect::new(r.x + 4.0, by, r.w - 8.0, bh.min(16.0));
        p.rect(b.x, b.y, b.w, b.h, b.h * 0.5, fade(KEY, 0x40));
        centred(p, b, bold(8.0, KEY), "MASTER");
    } else {
        // The key shift: down, the semitones, up.
        let w = ((r.w - 4.0) / 3.0).floor();
        button(p, s, Rect::new(r.x, by, w, bh), "-", (false, KEY), Target::Pitch(d, -1));
        let n = if st == 0 { "0".to_string() } else { format!("{st:+}") };
        centred(p, Rect::new(r.x + w + 2.0, by, w, bh), bold(9.0, if st == 0 { FAINT } else { KEY }), &n);
        button(p, s, Rect::new(r.x + 2.0 * (w + 2.0), by, w, bh), "+", (false, KEY), Target::Pitch(d, 1));
    }
}

/// SYNC over the tempo fader, key lock beside it, the rate under it.
fn tempo(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    let t = s.telemetry.decks[d];
    let (label, on, colour) = match (t.sync_leader, t.sync_lock) {
        (1.., _) => ("MASTER", true, KEY),
        (0, true) => ("SYNC", true, DECK[d]),
        (0, false) => ("SYNC", false, DECK[d]),
    };
    let sh = 28.0_f32.min(r.h * 0.2);
    button(p, s, Rect::new(r.x, r.y, r.w, sh), label, (on, colour), Target::Sync(d));
    let foot = 16.0;
    let kl = 20.0;
    let f = Rect::new(r.x + r.w * 0.5 - 11.0, r.y + sh + 8.0, 22.0, r.h - sh - foot - kl - 16.0);
    let rate = s.decks[d].rate;
    let up = s.rate_from(rate);
    fader(p, s, f, ((up + 1.0) * 0.5, false, true), DECK[d], Target::Rate(d));
    let lock = Rect::new(r.x, f.y + f.h + 4.0, r.w, kl);
    button(p, s, lock, "KEY", (s.decks[d].key_lock, DECK[d]), Target::KeyLock(d));
    let pct = format!("{:+.1}%", f64::from(rate) * s.settings.rate_range() * 100.0);
    centred(p, Rect::new(r.x - 4.0, r.y + r.h - foot, r.w + 8.0, foot), Style::new(10.0, Weight::Semibold, DIM), &pct);
}

/// LOOP: its size between halve and double, IN and OUT, then beat jump and the deck switches.
fn loop_box(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    let gap = 4.0;
    let lh = 14.0;
    let rh = ((r.h - lh - 4.0 * gap) / 4.0).floor().min(28.0);
    let mut y = r.y;
    let caption = |p: &mut Painter, y: f32, t: &str| centred(p, Rect::new(r.x, y, r.w, lh), bold(9.0, FAINT), t);
    caption(p, y, "LOOP");
    y += lh + 2.0;
    let third = ((r.w - 2.0 * gap) / 3.0).floor();
    let looping = s.decks[d].loop_range.is_some();
    button(p, s, Rect::new(r.x, y, third, rh), "<", (false, LOOP_ON), Target::LoopSize(d, -1));
    let size = Rect::new(r.x + third + gap, y, third, rh);
    button(p, s, size, "", (looping, LOOP_ON), Target::LoopToggle(d));
    centred(p, size, bold(12.0, if looping { LOOP_ON } else { WHITE }), &beats_label(s.decks[d].loop_size as f64));
    button(p, s, Rect::new(r.x + 2.0 * (third + gap), y, third, rh), ">", (false, LOOP_ON), Target::LoopSize(d, 1));
    y += rh + gap;
    let half = ((r.w - gap) * 0.5).floor();
    button(p, s, Rect::new(r.x, y, half, rh), "IN", (s.decks[d].loop_in.is_some(), LOOP_ON), Target::LoopIn(d));
    button(p, s, Rect::new(r.x + half + gap, y, half, rh), "OUT", (false, LOOP_ON), Target::LoopOut(d));
    y += rh + gap;
    button(p, s, Rect::new(r.x, y, third, rh), "<<", (false, ACCENT), Target::Jump(d, -1));
    centred(p, Rect::new(r.x + third + gap, y, third, rh), bold(10.0, DIM), &beats_label(s.decks[d].jump as f64));
    button(p, s, Rect::new(r.x + 2.0 * (third + gap), y, third, rh), ">>", (false, ACCENT), Target::Jump(d, 1));
    y += rh + gap;
    let deck = &s.decks[d];
    let quarter = ((r.w - 3.0) / 4.0).floor();
    let chips = [
        ("Q", deck.quantize, Target::Quantize(d)),
        ("SLIP", deck.slip, Target::Slip(d)),
        ("REV", deck.reverse, Target::Reverse(d)),
        ("GRID", deck.grid_locked, Target::Grid(d)),
    ];
    for (i, (label, on, t)) in chips.into_iter().enumerate() {
        let b = Rect::new(r.x + i as f32 * (quarter + 1.0), y, quarter, rh);
        let down = s.pressed(t);
        crate::theme::labelled(p, b, label, 7.0, on, down, ACCENT);
        hit(s, b, t);
    }
}

/// The jog: a ring that fills as the track plays round the tempo, with PLAY and CUE under it.
fn jog_and_transport(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    let bh = 28.0_f32.min(r.h * 0.18);
    let side = (r.h - bh - 8.0).min(r.w).max(40.0);
    let jr = Rect::new(r.x + (r.w - side) * 0.5, r.y, side, side);
    spinny(p, s, jr, d);
    let bw = ((side.max(120.0).min(r.w) - 8.0) * 0.5).floor();
    let bx = r.x + (r.w - 2.0 * bw - 8.0) * 0.5;
    let by = r.y + r.h - bh;
    let playing = s.telemetry.decks[d].playing;
    let play = Rect::new(bx, by, bw, bh);
    let down = s.pressed(Target::Play(d));
    p.outlined(
        play.x,
        play.y,
        play.w,
        play.h,
        6.0,
        if down { PRESSED } else { CONTROL },
        if playing { 2.0 } else { 1.0 },
        if playing { DECK[d] } else { BORDER },
    );
    let k = (bh * 0.42).round();
    let (cx, cy) = (play.x + play.w * 0.5, play.y + play.h * 0.5);
    let ink = if playing { DECK[d] } else { WHITE };
    for i in 0..(k as usize) {
        let f = i as f32;
        let half = k * 0.5 * (1.0 - f / k);
        p.rect(cx - k * 0.4 + f, cy - half, 1.0, half * 2.0, 0.0, ink);
    }
    hit(s, play, Target::Play(d));
    let cue = Rect::new(bx + bw + 8.0, by, bw, bh);
    let down = s.pressed(Target::Cue(d));
    p.outlined(
        cue.x,
        cue.y,
        cue.w,
        cue.h,
        6.0,
        if down { fade(CUE, 0x60) } else { CONTROL },
        1.0,
        if down { CUE } else { BORDER },
    );
    centred(p, cue, bold(11.0, if down { WHITE } else { CUE }), "CUE");
    hit(s, cue, Target::Cue(d));
}

const MODES: [(Mode, &str); 4] =
    [(Mode::Cues, "HOT CUES"), (Mode::Loops, "LOOPS"), (Mode::Samples, "SAMPLER"), (Mode::Fx, "PAD FX")];

/// Pads under their mode tabs: a dark grid whose labels light in the pad's colour.
fn pads(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    p.outlined(r.x, r.y, r.w, r.h, 6.0, WELL, 1.0, BORDER);
    let th = 22.0_f32.min(r.h * 0.18).round();
    let mode = s.decks[d].mode;
    let tw = r.w / MODES.len() as f32;
    for (i, (m, label)) in MODES.into_iter().enumerate() {
        let b = Rect::new(r.x + i as f32 * tw, r.y, tw, th);
        let t = Target::Mode(d, m);
        let on = mode == m;
        if on || s.pressed(t) {
            p.rect(b.x + 1.0, b.y + 1.0, b.w - 2.0, b.h - 1.0, 5.0, RAISED);
        }
        centred(p, b, bold((th * 0.36).clamp(7.0, 9.0), if on { WHITE } else { FAINT }), label);
        hit(s, b, t);
    }
    p.rect(r.x, r.y + th, r.w, 1.0, 0.0, SEPARATOR);
    let g = Rect::new(r.x + 1.0, r.y + th + 1.0, r.w - 2.0, r.h - th - 2.0);
    let (pw, ph) = ((g.w - 3.0) / 4.0, (g.h - 1.0) * 0.5);
    for i in 0..8 {
        let b = Rect::new(g.x + (i % 4) as f32 * (pw + 1.0), g.y + (i / 4) as f32 * (ph + 1.0), pw, ph);
        let t = Target::Pad(d, i);
        let down = s.pressed(t);
        let (label, lit) = match mode {
            Mode::Cues => match s.decks[d].cues[i] {
                Some(at) => (mmss(s.decks[d].seconds(at)), Some(cue_colour(s, d, i))),
                None => ("+".to_string(), None),
            },
            Mode::Loops => (beats_label(LOOP_BEATS[i]), (s.decks[d].looping == Some(i)).then_some(LOOP_ON)),
            Mode::Samples => (pad_name(s, d, i), None),
            Mode::Fx => (FX_PADS[i].to_string(), (s.decks[d].echo == Some(i)).then_some(rgb(0xfca001))),
        };
        let fill = match (down, lit) {
            (true, Some(c)) => fade(c, 0x50),
            (true, None) => PRESSED,
            (false, Some(c)) => fade(c, 0x14),
            (false, None) => CONTROL,
        };
        p.rect(b.x, b.y, b.w, b.h, 4.0, fill);
        let ink = lit.unwrap_or(if mode == Mode::Cues { FAINT } else { DIM });
        let st = bold((b.h * 0.24).clamp(8.0, 12.0), ink);
        if mode == Mode::Cues && lit.is_some() {
            // A play mark ahead of the cue's time, as the pads show it.
            let tw = p.text_width(st, &label);
            let k = st.px * 0.7;
            let x0 = b.x + (b.w - tw - k - 4.0) * 0.5;
            let cy = b.y + b.h * 0.5;
            for j in 0..(k as usize) {
                let f = j as f32;
                let half = k * 0.5 * (1.0 - f / k);
                p.rect(x0 + f, cy - half, 1.0, half * 2.0, 0.0, ink);
            }
            p.text(x0 + k + 4.0, cy + st.px * 0.36, st, &label);
        } else {
            centred(p, b, st, &label);
        }
        if let Some(c) = lit {
            p.rect(b.x + b.w * 0.3, b.y + b.h - 4.0, b.w * 0.4, 2.0, 1.0, c);
        }
        hit(s, b, t);
    }
}

fn mmss(secs: f64) -> String {
    let t = secs.max(0.0) as u32;
    format!("{}:{:02}", t / 60, t % 60)
}

fn overview(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    p.outlined(r.x, r.y, r.w, r.h, 6.0, WELL, 1.0, BORDER);
    let deck = &s.decks[d];
    if let Some(l) = &deck.loaded {
        let len = deck.length().max(1.0);
        let played = (s.telemetry.decks[d].position / len) as f32;
        let n = l.overview.len().max(1);
        let cols = r.w.max(1.0) as usize;
        for c in 0..cols {
            let a = f32::from(l.overview[c * n / cols]) / 255.0 * r.h * 0.46;
            let ink = if (c as f32) < played * cols as f32 { fade(WHITE, 0x50) } else { DECK[d] };
            p.rect(r.x + c as f32, r.y + r.h * 0.5 - a, 1.0, a * 2.0 + 0.5, 0.0, ink);
        }
        for (i, cue) in deck.cues.iter().enumerate() {
            if let Some(cue) = cue {
                p.rect(r.x + (cue / len) as f32 * r.w, r.y, 1.5, r.h, 0.0, cue_colour(s, d, i));
            }
        }
        p.rect(r.x + r.w * played - 1.0, r.y, 2.0, r.h, 1.0, WHITE);
    }
    hit(s, r, Target::Overview(d));
}

fn mixer(p: &mut Painter, s: &mut State, r: Rect) {
    p.outlined(r.x, r.y, r.w, r.h, 10.0, PANEL, 1.0, BORDER);
    let xf_h = (r.h * 0.12).clamp(22.0, 34.0).round();
    let tw = (r.w - 8.0).min(MIXER.0 * 1.6);
    let top = Rect::new(r.x + (r.w - tw) * 0.5, r.y + 4.0, tw, r.h - xf_h - 12.0);
    let cw = ((top.w - 4.0) / 4.0).floor();
    let col = |i: f32| Rect::new(top.x + i * (cw + 1.333), top.y, cw, top.h);
    // Columns from the outside in: deck 1 EQ, deck 1 channel, deck 2 channel, deck 2 EQ.
    for (d, eq, ch) in [(0, col(0.0), col(1.0)), (1, col(3.0), col(2.0))] {
        eq_column(p, s, eq, d);
        channel(p, s, ch, d);
    }
    let xf = Rect::new(top.x + 4.0, r.y + r.h - xf_h - 4.0, top.w - 8.0, xf_h);
    let pos = (s.crossfader + 1.0) * 0.5;
    fader(p, s, xf, (pos, true, true), ACCENT, Target::Crossfader);
}

fn eq_column(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    let kh = ((r.h - 9.0) / 4.0).floor();
    for (n, label) in ["HI", "MID", "LOW"].into_iter().enumerate() {
        let b = Rect::new(r.x, r.y + n as f32 * (kh + 3.0), r.w, kh);
        let v = s.decks[d].knobs[n];
        knob(p, s, b, (v, true, WHITE), label, Target::Knob(d, n));
    }
    let b = Rect::new(r.x, r.y + 3.0 * (kh + 3.0), r.w, kh);
    p.outlined(b.x, b.y, b.w, b.h, 6.0, WELL, 1.0, BORDER);
    let v = s.decks[d].knobs[3];
    knob(p, s, b, (v, true, rgb(0xea2a4e)), "FX", Target::Knob(d, 3));
}

fn channel(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    let gk = r.w.min(r.h * 0.22).round();
    let g = Rect::new(r.x, r.y, r.w, gk);
    p.outlined(g.x, g.y, g.w, g.h, 6.0, WELL, 1.0, BORDER);
    let v = s.decks[d].gain;
    knob(p, s, g, (v, true, DECK[d]), "", Target::Gain(d));
    let pfl_h = 26.0_f32.min(r.h * 0.14).round();
    let f = Rect::new(r.x, r.y + gk + 4.0, r.w, r.h - gk - pfl_h - 8.0);
    let vol = s.decks[d].volume;
    let vu = 4.0;
    vu_meter(p, Rect::new(f.x, f.y, vu, f.h), s.telemetry.decks[d].peak);
    vu_meter(p, Rect::new(f.x + f.w - vu, f.y, vu, f.h), s.telemetry.decks[d].peak);
    fader(
        p,
        s,
        Rect::new(f.x + vu + 3.0, f.y, f.w - 2.0 * (vu + 3.0), f.h),
        (vol, false, false),
        DECK[d],
        Target::Volume(d),
    );
    let on = s.decks[d].pfl;
    button(p, s, Rect::new(r.x, r.y + r.h - pfl_h, r.w, pfl_h), "PFL", (on, DECK[d]), Target::Pfl(d));
}

/// The Classic meter: a black capsule filled from the bottom, green, amber, then red at the top.
fn vu_meter(p: &mut Painter, r: Rect, peak: f32) {
    p.outlined(r.x - 0.5, r.y, r.w + 1.0, r.h, r.w * 0.5, WELL, 1.0, BORDER);
    let fill = peak.clamp(0.0, 1.0) * (r.h - 2.0);
    for (lo, hi, colour) in [(0.0, 0.7, rgb(0x85c85b)), (0.7, 0.85, rgb(0xfca001)), (0.85, 1.0, rgb(0xea2a4e))] {
        let (a, b) = (lo * (r.h - 2.0), (hi * (r.h - 2.0)).min(fill));
        if b > a {
            p.rect(r.x, r.y + r.h - 1.0 - b, r.w, b - a, r.w * 0.5, colour);
        }
    }
}

/// The Booth knob: a 270 degree ring, filled from the centre or the start, and a pointer.
fn knob(p: &mut Painter, s: &mut State, r: Rect, (v, bipolar, colour): (f32, bool, u32), label: &str, t: Target) {
    let lp = (r.w * 0.22).min(r.h * 0.28).round().clamp(7.0, 13.0);
    let lh = if label.is_empty() { 0.0 } else { lp + 3.0 };
    let size = r.w.min(r.h - lh).clamp(12.0, 64.0);
    let (cx, cy) = (r.x + r.w * 0.5, r.y + (r.h - lh - size) * 0.5 + size * 0.5);
    let lw = (size / 11.0).max(2.0);
    let rad = size * 0.5 - lw;
    p.disc(cx, cy, rad * 0.78, rgb(0x2a2a30), rgb(0x141417));
    p.outlined(cx - rad * 0.78, cy - rad * 0.78, rad * 1.56, rad * 1.56, rad * 0.78, 0, 1.0, BORDER);
    p.arc(cx, cy, rad + lw * 0.5, lw, 0.625, 0.75, LINE);
    let at = 0.625 + v.clamp(0.0, 1.0) * 0.75;
    let from = if bipolar { 1.0 } else { 0.625 };
    if (at - from).abs() > 0.002 {
        p.arc(cx, cy, rad + lw * 0.5, lw, at.min(from), (at - from).abs(), colour);
    }
    let a = at * std::f32::consts::TAU;
    let (sn, cs) = a.sin_cos();
    p.line(
        (cx + sn * rad * 0.25, cy - cs * rad * 0.25),
        (cx + sn * rad * 0.7, cy - cs * rad * 0.7),
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
        p.rect(r.x + handle * 0.25, r.y + r.h * 0.5 - 2.0, r.w - handle * 0.5, 4.0, 2.0, WELL);
        if centre {
            p.rect(r.x + r.w * 0.5 - 0.5, r.y + r.h * 0.2, 1.0, r.h * 0.6, 0.0, LINE);
        }
        let hw = handle * 0.55;
        let x = r.x + pos * (r.w - hw);
        let edge = if s.pressed(t) { WHITE } else { BORDER };
        p.glow(x, r.y + 2.0, hw, r.h, 4.0, 6.0, fade(rgb(0x000000), 0xa0));
        p.gradient(x, r.y, hw, r.h, 4.0, rgb(0x46464d), rgb(0x26262b), 1.0, edge);
        p.rect(x + hw * 0.5 - 1.0, r.y + 3.0, 2.0, r.h - 6.0, 1.0, colour);
    } else {
        p.rect(r.x + r.w * 0.5 - 2.0, r.y + handle * 0.25, 4.0, r.h - handle * 0.5, 2.0, WELL);
        if centre {
            p.rect(r.x + r.w * 0.2, r.y + r.h * 0.5 - 0.5, r.w * 0.6, 1.0, 0.0, LINE);
        }
        let hh = handle * 0.55;
        let y = r.y + (1.0 - pos) * (r.h - hh);
        let edge = if s.pressed(t) { WHITE } else { BORDER };
        p.glow(r.x, y + 2.0, r.w, hh, 4.0, 6.0, fade(rgb(0x000000), 0xa0));
        p.gradient(r.x, y, r.w, hh, 4.0, rgb(0x46464d), rgb(0x26262b), 1.0, edge);
        p.rect(r.x + 3.0, y + hh * 0.5 - 1.0, r.w - 6.0, 2.0, 1.0, colour);
    }
    hit(s, r, t);
}

/// The platter: a record whose rim shows how much of the track has played, a
/// marker turning with the music and the tempo on its label.
fn spinny(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    let rad = r.w * 0.5;
    let (cx, cy) = (r.x + rad, r.y + rad);
    p.glow(r.x, r.y + 4.0, r.w, r.h, rad, 12.0, fade(rgb(0x000000), 0xb0));
    p.disc(cx, cy, rad, rgb(0x141416), rgb(0x060607));
    p.outlined(cx - rad, cy - rad, rad * 2.0, rad * 2.0, rad, 0, 1.0, BORDER);
    for f in [0.86, 0.79, 0.72] {
        let g = rad * f;
        p.outlined(cx - g, cy - g, g * 2.0, g * 2.0, g, 0, 1.0, fade(rgb(0xffffff), 0x08));
    }
    let ring = (rad * 0.06).clamp(2.0, 4.0);
    p.arc(cx, cy, rad - ring * 0.5 - 2.0, ring, 0.0, 1.0, RAISED);
    let deck = &s.decks[d];
    if deck.loaded.is_some() {
        let played = (s.telemetry.decks[d].position / deck.length().max(1.0)).clamp(0.0, 1.0) as f32;
        p.arc(cx, cy, rad - ring * 0.5 - 2.0, ring, 0.0, played, DECK[d]);
        let turn = (deck.seconds(s.telemetry.decks[d].position) / 1.8).fract() as f32 * std::f32::consts::TAU;
        let (sn, cs) = turn.sin_cos();
        p.line(
            (cx + sn * rad * 0.66, cy - cs * rad * 0.66),
            (cx + sn * (rad - ring - 5.0), cy - cs * (rad - ring - 5.0)),
            (rad * 0.05).clamp(2.0, 4.0),
            WHITE,
        );
    }
    let label = rad * 0.6;
    p.disc(cx, cy, label, rgb(0x26262b), rgb(0x18181c));
    p.outlined(cx - label, cy - label, label * 2.0, label * 2.0, label, 0, 1.0, BORDER);
    // The tempo readout in the middle: BPM, then the rate offset in the deck colour.
    let bpm = deck
        .loaded
        .as_ref()
        .and_then(|l| l.grid)
        .map(|g| g.bpm * (1.0 + f64::from(deck.rate) * s.settings.rate_range()));
    let known = bpm.is_some();
    let bpm = bpm.map_or_else(|| "--.--".to_string(), |b| format!("{b:.2}"));
    let pct = format!("{:+.2}%", f64::from(deck.rate) * 8.0);
    let big = bold((rad * 0.24).clamp(9.0, 22.0), if known { WHITE } else { FAINT });
    centred(p, Rect::new(cx - label, cy - big.px * 1.05, label * 2.0, big.px * 1.2), big, &bpm);
    let sm = Style::new((rad * 0.11).clamp(7.0, 11.0).round(), Weight::Semibold, DECK[d]);
    centred(p, Rect::new(cx - label, cy + 1.0, label * 2.0, sm.px * 1.3), sm, &pct);
    let unit = Style::new((rad * 0.09).clamp(7.0, 10.0).round(), Weight::Semibold, FAINT);
    centred(p, Rect::new(cx - label, cy + sm.px * 1.4, label * 2.0, unit.px * 1.3), unit, "BPM");
    hit(s, r, Target::Jog(d));
}
