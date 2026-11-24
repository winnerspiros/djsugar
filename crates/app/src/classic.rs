//! DJ Sugar's Classic phone skin, the default on Android: two decks either
//! side of a fixed size mixer, each deck an info bar, overview, transport and pads.

use sugar_layout::Rect;
use sugar_ui::{Painter, Style, Weight, rgb};

use crate::state::{FX_PADS, LOOP_BEATS, Mode, State, Target, beats_label, pad_name};
use crate::sugar::{DECK, cue_colour, fade, hit};
use crate::theme::{BACKDROP, BORDER, CONTROL, DIM, FAINT, PANEL, PRESSED, RAISED, TEXT as WHITE, WELL, centred};

const ACCENT: u32 = rgb(0x5b8cff);
const PLAY: u32 = rgb(0x00e676);
const CUE: u32 = rgb(0xff9100);
const LOOP_ON: u32 = rgb(0x00e5ff);
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
    let gap = 4.0;
    if stacked || r.w < MIXER.0 * 3.2 {
        // Portrait: decks stacked full width, the mixer between them.
        let mh = MIXER.1.min(r.h * 0.34).round();
        let dh = ((r.h - mh - 2.0 * gap) * 0.5).floor();
        deck(p, s, Rect::new(r.x, r.y, r.w, dh), 0);
        mixer(p, s, Rect::new(r.x, r.y + dh + gap, r.w, mh));
        deck(p, s, Rect::new(r.x, r.y + r.h - dh, r.w, dh), 1);
    } else {
        let mw = MIXER.0;
        let dw = ((r.w - mw - 2.0 * gap) * 0.5).floor();
        deck(p, s, Rect::new(r.x, r.y, dw, r.h), 0);
        mixer(p, s, Rect::new(r.x + dw + gap, r.y, mw, r.h));
        deck(p, s, Rect::new(r.x + r.w - dw, r.y, dw, r.h), 1);
    }
}

/// The Classic deck height in a band of the given height: the mixer's, or less.
pub fn height(avail: f32) -> f32 {
    MIXER.1.min(avail)
}

fn deck(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    p.outlined(r.x, r.y, r.w, r.h, 10.0, PANEL, 1.0, BORDER);
    p.rect(r.x + 10.0, r.y, r.w - 20.0, 2.0, 1.0, fade(DECK[d], 0x90));
    let r = Rect::new(r.x + 5.0, r.y + 5.0, r.w - 10.0, r.h - 10.0);
    let info_h = (r.h * 0.2).clamp(28.0, 50.0).round();
    info(p, s, Rect::new(r.x, r.y, r.w, info_h), d);
    let mut top = r.y + info_h + 6.0;
    // The overview strip appears with a track, as in the reference.
    if s.decks[d].loaded.is_some() {
        let ov = (r.h * 0.14).clamp(14.0, 40.0).round();
        overview(p, s, Rect::new(r.x, top, r.w, ov), d);
        top += ov + 4.0;
    }
    let body = Rect::new(r.x, top, r.w, r.y + r.h - top);
    if body.h < 40.0 {
        return;
    }
    // Left to right as in the reference: transport, pads with their mode tabs,
    // beat jump, loop, the tempo platter, the sync column, the tempo fader.
    let u = body.w / 1080.0;
    let col = |w: f32| (w * u).round();
    let mut x = body.x;
    let mut take = |w: f32| {
        let at = x;
        x += w + 6.0;
        Rect::new(at, body.y, w, body.h)
    };
    let tr = take(col(75.0).max(40.0));
    transport(p, s, tr, d);
    let pr = take(col(350.0).max(150.0));
    pads(p, s, pr, d);
    let wide = body.w > 620.0;
    if wide {
        let bj = take(col(140.0));
        beatjump(p, s, bj, d);
        let lp = take(col(215.0));
        loops(p, s, lp, d);
    }
    let sp = body.h.min(col(165.0)).max(60.0);
    let sr = take(sp);
    spinny(p, s, Rect::new(sr.x, sr.y + (sr.h - sp) * 0.5, sp, sp), d);
    let rw = 22.0;
    let side = Rect::new(x, body.y, (body.x + body.w - rw - 6.0 - x).max(30.0), body.h);
    side_column(p, s, side, d, !wide);
    let rate = s.decks[d].rate;
    let rr = Rect::new(body.x + body.w - rw, body.y, rw, body.h);
    fader(p, s, rr, ((rate + 1.0) * 0.5, false, true), DECK[d], Target::Rate(d));
}

/// A titled dark panel, the frame of the beat jump and loop blocks.
fn panel(p: &mut Painter, r: Rect, title: &str) -> Rect {
    p.outlined(r.x, r.y, r.w, r.h, 8.0, WELL, 1.0, BORDER);
    let th = (r.h * 0.26).clamp(14.0, 30.0);
    centred(
        p,
        Rect::new(r.x, r.y, r.w, th),
        Style::new((th * 0.46).round().max(8.0), Weight::Semibold, FAINT),
        &title.to_uppercase(),
    );
    Rect::new(r.x + 4.0, r.y + th + 2.0, r.w - 8.0, r.h - th - 6.0)
}

fn beatjump(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    let inner = panel(p, r, "Beatjump");
    let h = ((inner.h - 4.0) * 0.5).floor();
    let w = ((inner.w - 4.0) * 0.5).floor();
    button(p, s, Rect::new(inner.x, inner.y, w, h), "<<", (false, ACCENT), Target::Jump(d, -1));
    button(p, s, Rect::new(inner.x + w + 4.0, inner.y, w, h), ">>", (false, ACCENT), Target::Jump(d, 1));
    let n = beats_label(s.decks[d].jump as f64);
    centred(p, Rect::new(inner.x, inner.y + h + 4.0, inner.w, h), bold((h * 0.4).min(18.0), WHITE), &n);
}

fn loops(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    let inner = panel(p, r, "Loop");
    let h = ((inner.h - 4.0) * 0.5).floor();
    let w = ((inner.w - 8.0) / 3.0).floor();
    let cell = |i: f32, y: f32| Rect::new(inner.x + i * (w + 4.0), y, w, h);
    let on = s.decks[d].loop_range.is_some();
    let armed = s.decks[d].loop_in.is_some();
    button(p, s, cell(0.0, inner.y), "IN", (armed, ACCENT), Target::LoopIn(d));
    button(p, s, cell(1.0, inner.y), "OUT", (false, ACCENT), Target::LoopOut(d));
    button(p, s, cell(2.0, inner.y), "RECALL", (on, LOOP_ON), Target::LoopToggle(d));
    let y = inner.y + h + 4.0;
    button(p, s, cell(0.0, y), "/2", (false, ACCENT), Target::LoopSize(d, -1));
    let n = beats_label(s.decks[d].loop_size as f64);
    centred(p, cell(1.0, y), bold((h * 0.4).min(18.0), WHITE), &n);
    button(p, s, cell(2.0, y), "x2", (false, ACCENT), Target::LoopSize(d, 1));
}

/// Beat grid, sync and the deck switches stacked beside the platter.
fn side_column(p: &mut Painter, s: &mut State, r: Rect, d: usize, narrow: bool) {
    let deck = &s.decks[d];
    let (kl, q, slip, rev, grid) = (deck.key_lock, deck.quantize, deck.slip, deck.reverse, deck.grid_locked);
    let t = s.telemetry.decks[d];
    let sync = match (t.sync_leader, t.sync_lock) {
        (1.., _) => ("MSTR", true, rgb(0xff9100)),
        (0, true) => ("LOCK", true, rgb(0x00e676)),
        (0, false) => ("SYNC", false, rgb(0x00e676)),
    };
    let mut items: Vec<(&str, bool, u32, Target)> = vec![
        ("GRID", grid, ACCENT, Target::Grid(d)),
        (sync.0, sync.1, sync.2, Target::Sync(d)),
        ("Q", q, ACCENT, Target::Quantize(d)),
        ("KEY", kl, ACCENT, Target::KeyLock(d)),
        ("SLIP", slip, ACCENT, Target::Slip(d)),
        ("REV", rev, rgb(0xbf360c), Target::Reverse(d)),
    ];
    if narrow {
        items.extend([
            ("PITCH -", false, ACCENT, Target::Pitch(d, -1)),
            ("PITCH +", false, ACCENT, Target::Pitch(d, 1)),
        ]);
    }
    let cols = if r.w > 150.0 { 2 } else { 1 };
    let rows = items.len().div_ceil(cols);
    let h = ((r.h - (rows - 1) as f32 * 3.0) / rows as f32).floor();
    let w = ((r.w - (cols - 1) as f32 * 3.0) / cols as f32).floor();
    for (i, (label, on, colour, t)) in items.into_iter().enumerate() {
        let b = Rect::new(r.x + (i % cols) as f32 * (w + 3.0), r.y + (i / cols) as f32 * (h + 3.0), w, h);
        button(p, s, b, label, (on, colour), t);
    }
}

fn info(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    p.outlined(r.x, r.y, r.w, r.h, 8.0, RAISED, 1.0, BORDER);
    let badge = Rect::new(r.x + 5.0, r.y + 5.0, r.h - 10.0, r.h - 10.0);
    p.outlined(badge.x, badge.y, badge.w, badge.h, 6.0, fade(DECK[d], 0x26), 1.0, fade(DECK[d], 0xb0));
    centred(p, badge, bold(badge.h * 0.52, DECK[d]), if d == 0 { "1" } else { "2" });
    let deck = &s.decks[d];
    let bpm = deck.loaded.as_ref().and_then(|l| l.grid).map(|g| g.bpm * (1.0 + f64::from(deck.rate) * 0.08));
    // LOAD's place on the right ejects a loaded track.
    let ej_w = (r.h * 1.7).min(r.w * 0.16).round();
    let ej = Rect::new(r.x + r.w - ej_w - 4.0, r.y + 4.0, ej_w, r.h - 8.0);
    button(p, s, ej, "EJECT", (false, DECK[d]), Target::Eject(d));
    let deck = &s.decks[d];
    let pill_w = (r.h * 1.6).min(r.w * 0.2).round();
    let pill = Rect::new(ej.x - pill_w - 6.0, r.y + 4.0, pill_w, r.h - 8.0);
    p.outlined(pill.x, pill.y, pill.w, pill.h, pill.h * 0.5, WELL, 1.0, BORDER);
    let known = bpm.is_some();
    let bpm = bpm.map_or_else(|| "--.-".to_string(), |b| format!("{b:.1}"));
    centred(p, pill, bold((pill.h * 0.44).min(14.0), if known { WHITE } else { FAINT }), &bpm);
    let tx = badge.x + badge.w + 6.0;
    let tw = pill.x - tx - 6.0;
    let (title, artist) = deck.loaded.as_ref().map_or(("No track".to_string(), "Press LOAD".to_string()), |l| {
        (l.entry.title.to_string(), l.entry.artist.to_string())
    });
    let tp = (r.h * 0.3).clamp(9.0, 14.0);
    let ink = if deck.loaded.is_some() { WHITE } else { DIM };
    p.text_in(tx, r.y + r.h * 0.45, tw, bold(tp, ink), &title);
    if !artist.is_empty() {
        p.text_in(tx, r.y + r.h * 0.45 + tp + 2.0, tw, Style::new(tp * 0.85, Weight::Regular, DIM), &artist);
    }
    let left = deck.length() - s.telemetry.decks[d].position;
    if deck.loaded.is_some() && r.w > 260.0 {
        let t = format!("-{}", mmss(deck.seconds(left.max(0.0))));
        let st = bold(tp * 0.9, if deck.seconds(left) < 30.0 { CUE } else { WHITE });
        let w = p.text_width(st, &t);
        p.text(pill.x - w - 8.0, r.y + r.h * 0.5 + st.px * 0.36, st, &t);
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

fn transport(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    let h = ((r.h - 4.0) * 0.5).floor();
    let playing = s.telemetry.decks[d].playing;
    let play = Rect::new(r.x, r.y, r.w, h);
    let down = s.pressed(Target::Play(d));
    let radius = (play.w * 0.18).clamp(6.0, 12.0);
    p.outlined(play.x, play.y, play.w, play.h, radius, if down { PRESSED } else { CONTROL }, 1.0, BORDER);
    if playing {
        p.glow(play.x, play.y, play.w, play.h, radius, 8.0, fade(PLAY, 0x30));
        p.outlined(play.x, play.y, play.w, play.h, radius, fade(PLAY, 0x18), 2.0, PLAY);
    }
    // The play triangle.
    let k = (play.w.min(play.h) * 0.32).round();
    let (cx, cy) = (play.x + play.w * 0.5, play.y + play.h * 0.5);
    let ink = if playing { PLAY } else { WHITE };
    for i in 0..(k as usize) {
        let f = i as f32;
        let half = k * 0.5 * (1.0 - f / k);
        p.rect(cx - k * 0.4 + f, cy - half, 1.0, half * 2.0, 0.0, ink);
    }
    hit(s, play, Target::Play(d));
    let cue = Rect::new(r.x, r.y + h + 4.0, r.w, h);
    let down = s.pressed(Target::Cue(d));
    let fill = if down { fade(CUE, 0x70) } else { fade(CUE, 0x14) };
    p.outlined(cue.x, cue.y, cue.w, cue.h, radius, fill, 1.5, fade(CUE, if down { 0xff } else { 0x90 }));
    centred(p, cue, bold((cue.w * 0.22).min(15.0), if down { WHITE } else { CUE }), "CUE");
    hit(s, cue, Target::Cue(d));
}

const MODES: [(Mode, &str); 4] =
    [(Mode::Cues, "CUE"), (Mode::Loops, "LOOP"), (Mode::Samples, "SMPL"), (Mode::Fx, "FX")];

fn pads(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    let tab_w = (r.w * 0.13).clamp(38.0, 46.0).round();
    let th = ((r.h - 3.0 * 3.0) / 4.0).floor();
    let mode = s.decks[d].mode;
    for (i, (m, label)) in MODES.into_iter().enumerate() {
        let b = Rect::new(r.x + r.w - tab_w, r.y + i as f32 * (th + 3.0), tab_w, th);
        button(p, s, b, label, (mode == m, ACCENT), Target::Mode(d, m));
    }
    let g = Rect::new(r.x, r.y, r.w - tab_w - 4.0, r.h);
    let (pw, ph) = (((g.w - 9.0) / 4.0).floor(), ((g.h - 3.0) * 0.5).floor());
    for i in 0..8 {
        let b = Rect::new(g.x + (i % 4) as f32 * (pw + 3.0), g.y + (i / 4) as f32 * (ph + 3.0), pw, ph);
        let t = Target::Pad(d, i);
        let down = s.pressed(t);
        // What the pad shows, and its colour when it is set or running.
        let (label, sub, lit) = match mode {
            Mode::Cues => ((i + 1).to_string(), "", s.decks[d].cues[i].is_some().then(|| cue_colour(s, d, i))),
            Mode::Loops => (beats_label(LOOP_BEATS[i]), "BEATS", (s.decks[d].looping == Some(i)).then_some(LOOP_ON)),
            Mode::Samples => (pad_name(s, d, i), "", None),
            Mode::Fx => (FX_PADS[i].to_string(), "", (s.decks[d].echo == Some(i)).then_some(rgb(0xfca001))),
        };
        let radius = 8.0_f32.min(b.h * 0.25);
        match lit {
            Some(c) => {
                p.outlined(b.x, b.y, b.w, b.h, radius, fade(c, if down { 0x90 } else { 0x38 }), 1.5, c);
                p.rect(b.x + 6.0, b.y + 4.0, b.w - 12.0, 3.0, 1.5, c);
            }
            None => crate::theme::control(p, b, radius, false, down, ACCENT),
        }
        let ink = match lit {
            Some(_) => WHITE,
            None if mode == Mode::Cues => FAINT,
            None => DIM,
        };
        let big = (b.h * 0.3).clamp(9.0, 15.0);
        if sub.is_empty() || b.h < 34.0 {
            centred(p, b, bold(big, ink), &label);
        } else {
            centred(p, Rect::new(b.x, b.y, b.w, b.h * 0.7), bold(big, ink), &label);
            centred(p, Rect::new(b.x, b.y + b.h * 0.55, b.w, b.h * 0.4), bold(8.0, ink), sub);
        }
        hit(s, b, t);
    }
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
    let bpm = deck.loaded.as_ref().and_then(|l| l.grid).map(|g| g.bpm * (1.0 + f64::from(deck.rate) * 0.08));
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
