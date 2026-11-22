use sugar_layout::{Axis, Id, Node, Rect, Size};
use sugar_ui::{Painter, Style, Weight, rgb};

use crate::state::{DETAIL_FRAMES, DeckUi, FX_PADS, Hit, LOOP_BEATS, Mode, State, Target};

pub const BACKGROUND: u32 = rgb(0x000000);
const PANEL: u32 = rgb(0x000000);
const RAISED: u32 = rgb(0x111114);
const LINE: u32 = rgb(0x222228);
const DECK: [u32; 2] = [rgb(0xff2e93), rgb(0x3fe0ff)];
const ACCENT: u32 = rgb(0xffe45c);
const TEXT: u32 = rgb(0xf2f2f5);
const DIM: u32 = rgb(0x8a8a96);
const PLAY: u32 = rgb(0x39ff88);

const MENU_ITEM: Style = Style::new(12.5, Weight::Regular, TEXT);
const BRAND: Style = Style::new(14.0, Weight::Bold, DECK[0]);
const HINT: Style = Style::new(12.5, Weight::Regular, DIM);
const HEADER: Style = Style::new(10.5, Weight::Semibold, DIM);
const TITLE: Style = Style::new(15.0, Weight::Semibold, TEXT);
const SUB: Style = Style::new(12.0, Weight::Regular, DIM);
const TIME: Style = Style::new(13.0, Weight::Semibold, ACCENT);
const LABEL: Style = Style::new(10.5, Weight::Semibold, DIM);
const BUTTON: Style = Style::new(11.5, Weight::Bold, TEXT);
const CELL: Style = Style::new(13.0, Weight::Regular, TEXT);
const LYRIC: Style = Style::new(13.0, Weight::Regular, DIM);

const fn fade(c: u32, a: u32) -> u32 {
    c & 0x00ff_ffff | a << 24
}

const MENU: [&str; 9] = ["File", "Edit", "View", "Deck", "Library", "Mix", "Tools", "Window", "Help"];
const KNOBS: [&str; 4] = ["HI", "MID", "LOW", "FX"];
const MENU_H: f32 = 24.0;
const PAD: f32 = 6.0;
const GAP: f32 = 6.0;
pub const ROW_H: f32 = 22.0;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum M {
    Top,
    Waves,
    DeckA,
    Mixer,
    DeckB,
    Library,
    Side,
    Ring,
}

const ALL: [M; 8] = [M::Top, M::Waves, M::DeckA, M::Mixer, M::DeckB, M::Library, M::Side, M::Ring];

fn module(id: M, w: f32, h: f32, grow: f32, max: f32, priority: u8) -> Node {
    Node::Module { id: id as Id, min: Size { w, h }, grow, max, priority }
}

pub fn tree() -> Node {
    let open = f32::INFINITY;
    Node::group(
        Axis::Column,
        GAP,
        u8::MAX,
        vec![
            module(M::Top, 280.0, 34.0, 0.0, open, 250),
            module(M::Waves, 280.0, 76.0, 0.4, open, 235),
            Node::group(
                Axis::Row,
                GAP,
                240,
                vec![
                    module(M::DeckA, 170.0, 180.0, 1.0, open, 240),
                    module(M::Mixer, 170.0, 180.0, 0.5, 300.0, 230),
                    module(M::DeckB, 170.0, 180.0, 1.0, open, 240),
                ],
            ),
            Node::group(
                Axis::Row,
                GAP,
                100,
                vec![module(M::Library, 240.0, 110.0, 1.0, open, 100), module(M::Side, 200.0, 110.0, 0.0, 280.0, 50)],
            ),
        ],
    )
}

pub fn portrait() -> Node {
    let open = f32::INFINITY;
    Node::group(
        Axis::Column,
        GAP,
        u8::MAX,
        vec![
            module(M::Top, 240.0, 34.0, 0.0, open, 250),
            module(M::Ring, 240.0, 220.0, 1.0, 420.0, 248),
            Node::group(
                Axis::Row,
                GAP,
                240,
                vec![module(M::DeckA, 150.0, 250.0, 1.0, open, 240), module(M::DeckB, 150.0, 250.0, 1.0, open, 240)],
            ),
            module(M::Mixer, 240.0, 150.0, 0.0, 180.0, 150),
            module(M::Library, 240.0, 120.0, 0.6, open, 100),
        ],
    )
}

pub fn inset(window: Rect) -> Rect {
    Rect::new(PAD, MENU_H + PAD, (window.w - 2.0 * PAD).max(0.0), (window.h - MENU_H - 2.0 * PAD).max(0.0))
}

fn hit(s: &mut State, x: f32, y: f32, w: f32, h: f32, target: Target) {
    s.hits.push(Hit { rect: Rect::new(x, y, w, h), target });
}

pub fn paint(p: &mut Painter, s: &mut State, window: Rect, placed: &[(Id, Rect)]) {
    s.hits.clear();
    p.rect(0.0, 0.0, window.w, MENU_H, 0.0, PANEL);
    p.rect(0.0, MENU_H - 1.0, window.w, 1.0, 0.0, LINE);
    let mut x = 10.0;
    for item in MENU {
        x += p.text(x, 16.5, MENU_ITEM, item) + 18.0;
    }
    for &(id, r) in placed {
        p.outlined(r.x, r.y, r.w, r.h, 8.0, PANEL, 1.0, LINE);
        match ALL[usize::from(id)] {
            M::Top => top(p, s, r),
            M::Waves => waves(p, s, r),
            M::DeckA => deck(p, s, r, 0),
            M::DeckB => deck(p, s, r, 1),
            M::Mixer => mixer(p, s, r),
            M::Library => library(p, s, r),
            M::Side => lyrics(p, r),
            M::Ring => ring(p, s, r),
        }
    }
    if let Some(d) = s.browser {
        p.layer();
        let r = screen_body(window);
        p.outlined(r.x, r.y, r.w, r.h, 10.0, PANEL, 1.5, DECK[d]);
        let head = Style::new(14.0, Weight::Bold, DECK[d]);
        let x = r.x + 30.0 + p.text(r.x + 14.0, r.y + 24.0, head, ["Load to deck A", "Load to deck B"][d]);
        let q = if s.query.is_empty() { "Type to search" } else { &s.query };
        let tw = p.text_in(x, r.y + 24.0, r.w * 0.5, if s.query.is_empty() { HINT } else { TITLE }, q);
        if !s.query.is_empty() {
            p.rect(x + tw + 2.0, r.y + 11.0, 2.0, 17.0, 1.0, ACCENT);
        }
        let close = "Close";
        let cw = p.text_width(BUTTON, close) + 24.0;
        p.outlined(r.x + r.w - cw - 10.0, r.y + 8.0, cw, 24.0, 6.0, RAISED, 1.0, LINE);
        p.text(r.x + r.w - cw + 2.0, r.y + 24.0, BUTTON, close);
        let list = Rect::new(r.x, r.y + 34.0, r.w, r.h - 34.0);
        s.hits.push(Hit { rect: r, target: Target::CloseBrowser });
        library(p, s, list);
        hit(s, r.x + r.w - cw - 10.0, r.y + 4.0, cw, 32.0, Target::CloseBrowser);
    }
}

fn screen_body(window: Rect) -> Rect {
    Rect::new(PAD, MENU_H + PAD, window.w - 2.0 * PAD, window.h - MENU_H - 2.0 * PAD)
}

fn top(p: &mut Painter, s: &State, r: Rect) {
    let x = r.x + 28.0 + p.text(r.x + 12.0, r.y + 22.0, BRAND, "DJ Sugar");
    let w = r.x + r.w - x - 12.0;
    if s.query.is_empty() {
        let hint = s.status.as_deref().unwrap_or("Type to search the library");
        p.text_in(x, r.y + 22.0, w, HINT, hint);
    } else {
        let tw = p.text_in(x, r.y + 22.0, w - 80.0, TITLE, &s.query);
        p.rect(x + tw + 2.0, r.y + 9.0, 2.0, 17.0, 1.0, ACCENT);
        let n = format!("{} found", s.shown.len());
        p.text(r.x + r.w - 12.0 - p.text_width(SUB, &n), r.y + 22.0, SUB, &n);
    }
}

fn clock(secs: f64) -> String {
    let s = secs.max(0.0);
    format!("{}:{:04.1}", (s / 60.0) as u32, s % 60.0)
}

fn waves(p: &mut Painter, s: &mut State, r: Rect) {
    let h = (r.h - 3.0 * PAD) * 0.5;
    for d in 0..2 {
        let y = r.y + PAD + d as f32 * (h + PAD);
        let ov = (h * 0.22).clamp(8.0, 18.0);
        let main = Rect::new(r.x + PAD, y, r.w - 2.0 * PAD, h - ov - 3.0);
        let over = Rect::new(r.x + PAD, y + h - ov, r.w - 2.0 * PAD, ov);
        p.rect(main.x, main.y, main.w, main.h, 4.0, RAISED);
        p.rect(over.x, over.y, over.w, over.h, 3.0, RAISED);
        let deck = &s.decks[d];
        let pos = s.telemetry.decks[d].position;
        if let Some(l) = &deck.loaded {
            let mid = main.y + main.h * 0.5;
            let bars = (main.w / 2.0) as usize;
            let centre = (pos / DETAIL_FRAMES as f64) as isize;
            let scale = 1.0 + deck.rate * 0.08;
            for b in 0..bars {
                let i = centre + ((b as f32 - bars as f32 * 0.5) * scale) as isize;
                let Some(&[v, lo, md, hi]) = usize::try_from(i).ok().and_then(|i| l.detail.get(i)) else { continue };
                let a = f32::from(v) / 255.0 * main.h * 0.48;
                let sum = (u32::from(lo) + u32::from(md) + u32::from(hi)).max(1);
                let ch = |x: u8| (u32::from(x) * 255 / sum * 2).min(255);
                let alpha = if (b as f32) < bars as f32 * 0.5 { 0x70 } else { 0xff };
                let c = ch(lo) | (ch(md) << 8) | (ch(hi) << 16) | (alpha << 24);
                let x = main.x + b as f32 * 2.0;
                p.rect(x, mid - a, 1.4, a * 2.0 + 0.5, 0.0, c);
                let ah = f32::from(hi) / 255.0 * main.h * 0.48;
                p.rect(x, mid - ah, 1.4, ah * 2.0 + 0.5, 0.0, fade(TEXT, alpha / 3))
            }
            let n = l.overview.len().max(1);
            let step = over.w / n as f32;
            let played = (pos / deck.length().max(1.0)) as f32;
            for (i, &v) in l.overview.iter().enumerate().step_by((n / over.w.max(1.0) as usize).max(1)) {
                let a = f32::from(v) / 255.0 * over.h * 0.5;
                let x = over.x + i as f32 * step;
                let c = if (i as f32) < played * n as f32 { DIM } else { DECK[d] };
                p.rect(x, over.y + over.h * 0.5 - a, step.max(1.0), a * 2.0 + 0.5, 0.0, c);
            }
            p.rect(over.x + over.w * played - 1.0, over.y, 2.0, over.h, 1.0, TEXT);
        } else {
            let msg = if deck.loading { "Loading..." } else { "Empty deck: tap a track in the library" };
            p.text(main.x + 10.0, main.y + main.h * 0.5 + 4.0, SUB, msg);
        }
        if let Some(l) = &deck.loaded
            && let Some(g) = l.grid
        {
            let beat = g.beat_frames(l.track.rate);
            let per_px = DETAIL_FRAMES as f64 * 0.5 * f64::from(1.0 + deck.rate * 0.08);
            let left = pos - per_px * f64::from(main.w) * 0.5;
            if let Some((a, b)) = deck.loop_range {
                let x0 = (((a - left) / per_px) as f32).clamp(0.0, main.w);
                let x1 = (((b - left) / per_px) as f32).clamp(0.0, main.w);
                p.rect(main.x + x0, main.y, x1 - x0, main.h, 0.0, fade(ACCENT, 0x30));
            }
            let mut n = ((left - g.first_beat) / beat).ceil();
            loop {
                let f = g.first_beat + n * beat;
                let x = ((f - left) / per_px) as f32;
                if x > main.w {
                    break;
                }
                n += 1.0;
                if f < 0.0 {
                    continue;
                }
                let bar = (n - 1.0).rem_euclid(4.0) == 0.0;
                p.rect(main.x + x, main.y, 1.0, main.h, 0.0, if bar { fade(TEXT, 0x50) } else { fade(TEXT, 0x20) });
            }
        }
        p.rect(main.x + main.w * 0.5 - 1.0, main.y, 2.0, main.h, 1.0, TEXT);
        if s.decks[d].loaded.is_some() {
            hit(s, over.x, over.y, over.w, over.h, Target::Overview(d));
        }
    }
}

fn deck(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    let colour = DECK[d];
    let t = s.telemetry.decks[d];
    let ui: &DeckUi = &s.decks[d];
    let right = r.x + r.w - 12.0;
    let remain = ui.seconds(ui.length() - t.position);
    let info = if ui.loaded.is_some() { format!("-{}", clock(remain)) } else { String::new() };
    let iw = p.text_width(TIME, &info);
    p.text(right - iw, r.y + 24.0, TIME, &info);
    let x = r.x + 22.0 + p.text(r.x + 12.0, r.y + 24.0, Style::new(18.0, Weight::Bold, colour), ["A", "B"][d]);
    let (title, sub) = match &ui.loaded {
        Some(l) => (l.entry.title.to_string(), l.entry.folder.to_string()),
        None => ("No track".into(), String::new()),
    };
    p.text_in(x, r.y + 24.0, right - iw - 12.0 - x, TITLE, &title);
    let rate = f64::from(ui.rate) * 8.0;
    let bpm = ui
        .loaded
        .as_ref()
        .and_then(|l| l.grid)
        .map_or(String::new(), |g| format!("{:.1} BPM  ", g.bpm * (1.0 + rate / 100.0)));
    let sub = format!("{bpm}{rate:+.1}%   {sub}");
    p.text_in(x, r.y + 42.0, right - x, SUB, &sub);

    hit(s, r.x, r.y, r.w, 50.0, Target::Browse(d));
    let ui = &s.decks[d];
    let fader_w = 22.0;
    let fx = if d == 0 { r.x + PAD } else { r.x + r.w - PAD - fader_w };
    let body = Rect::new(r.x + PAD, r.y + 54.0, r.w - 2.0 * PAD, r.h - 54.0 - PAD);
    let fy = body.y;
    let fh = body.h;
    p.rect(fx + fader_w * 0.5 - 2.0, fy + 4.0, 4.0, fh - 8.0, 2.0, RAISED);
    p.rect(fx, fy + fh * 0.5 - 0.5, fader_w, 1.0, 0.0, LINE);
    let cap = fy + (1.0 - (ui.rate + 1.0) * 0.5) * (fh - 14.0);
    p.rect(fx, cap, fader_w, 14.0, 3.0, if ui.rate == 0.0 { DIM } else { ACCENT });
    hit(s, fx - 4.0, fy, fader_w + 8.0, fh, Target::Rate(d));

    let inner_x = if d == 0 { fx + fader_w + PAD } else { body.x };
    let inner_w = body.w - fader_w - PAD;
    let narrow = inner_w < 260.0;
    let btn_h = 34.0f32.min(body.h * 0.22);
    let jog = if narrow {
        (inner_w * 0.58).min(body.h * 0.45).max(40.0)
    } else {
        (body.h - btn_h - PAD).min(inner_w * 0.48).max(40.0)
    };
    let jx = inner_x;
    let jy = body.y;
    p.outlined(jx, jy, jog, jog, jog * 0.5, RAISED, 2.0, if t.playing { colour } else { LINE });
    p.outlined(jx + jog * 0.3, jy + jog * 0.3, jog * 0.4, jog * 0.4, jog * 0.2, PANEL, 1.0, LINE);
    let secs = s.decks[d].seconds(t.position);
    let angle = (secs / 1.8 * std::f64::consts::TAU) as f32;
    let (sn, cs) = angle.sin_cos();
    let (cx, cy, rr) = (jx + jog * 0.5, jy + jog * 0.5, jog * 0.42);
    p.rect(cx + sn * rr - 3.0, cy - cs * rr - 3.0, 6.0, 6.0, 3.0, colour);
    hit(s, jx, jy, jog, jog, Target::Jog(d));

    let (play, cue) = if narrow {
        let bx = jx + jog + PAD;
        let bw = inner_x + inner_w - bx;
        let bh = ((jog - PAD) * 0.5).min(44.0);
        (Rect::new(bx, jy, bw, bh), Rect::new(bx, jy + bh + PAD, bw, bh))
    } else {
        let by = body.y + body.h - btn_h;
        let bw = (jog - PAD) * 0.5;
        (Rect::new(jx, by, bw, btn_h), Rect::new(jx + bw + PAD, by, bw, btn_h))
    };
    let playing = t.playing;
    p.outlined(
        play.x,
        play.y,
        play.w,
        play.h,
        6.0,
        if playing { fade(PLAY, 0x20) } else { RAISED },
        1.5,
        if playing { PLAY } else { LINE },
    );
    p.text(play.x + play.w * 0.5 - p.text_width(BUTTON, "PLAY") * 0.5, play.y + play.h * 0.5 + 4.0, BUTTON, "PLAY");
    hit(s, play.x, play.y, play.w, play.h, Target::Play(d));
    p.outlined(cue.x, cue.y, cue.w, cue.h, 6.0, RAISED, 1.5, ACCENT);
    p.text(cue.x + cue.w * 0.5 - p.text_width(BUTTON, "CUE") * 0.5, cue.y + cue.h * 0.5 + 4.0, BUTTON, "CUE");
    hit(s, cue.x, cue.y, cue.w, cue.h, Target::Cue(d));

    let (px, py) = if narrow { (inner_x, jy + jog + PAD) } else { (jx + jog + PAD, body.y) };
    let pw = inner_x + inner_w - px;
    if pw < 60.0 {
        return;
    }
    let key_h = 24.0;
    let kl = s.decks[d].key_lock;
    let tabs = [
        ("SYNC", None),
        ("KEY", None),
        ("CUE", Some(Mode::Cues)),
        ("LOOP", Some(Mode::Loops)),
        ("SMP", Some(Mode::Samples)),
        ("FX", Some(Mode::Fx)),
    ];
    let short = pw < 280.0;
    let tw = (pw - 5.0 * 4.0) / 6.0;
    for (n, (label, mode)) in tabs.into_iter().enumerate() {
        let x = px + n as f32 * (tw + 4.0);
        let on = mode.map_or(n == 1 && kl, |m| s.decks[d].mode == m);
        let edge = if on { if mode.is_none() { ACCENT } else { colour } } else { LINE };
        let label = if short { ["SYN", "KEY", "CUE", "LP", "SMP", "FX"][n] } else { label };
        p.outlined(x, py, tw, key_h, 5.0, if on { fade(edge, 0x30) } else { RAISED }, 1.0, edge);
        p.text(x + tw * 0.5 - p.text_width(LABEL, label) * 0.5, py + 16.0, LABEL, label);
        let t = match (n, mode) {
            (0, _) => Target::Sync(d),
            (_, None) => Target::KeyLock(d),
            (_, Some(m)) => Target::Mode(d, m),
        };
        hit(s, x, py, tw, key_h, t);
    }
    let gy = py + key_h + PAD;
    let gh = body.y + body.h - gy;
    let cw = (pw - 3.0 * PAD) * 0.25;
    let ch = (gh - PAD) * 0.5;
    let rate = s.decks[d].loaded.as_ref().map_or(48_000, |l| l.track.rate);
    for i in 0..8 {
        let x = px + (i % 4) as f32 * (cw + PAD);
        let y = gy + (i / 4) as f32 * (ch + PAD);
        let (label, lit) = match s.decks[d].mode {
            Mode::Cues => match s.decks[d].cues[i] {
                Some(at) => (clock(at / f64::from(rate)), true),
                None => (format!("{}", i + 1), false),
            },
            Mode::Loops => {
                let b = LOOP_BEATS[i];
                (
                    if b < 1.0 { format!("1/{}", (1.0 / b) as u32) } else { format!("{b}") },
                    s.decks[d].looping == Some(i),
                )
            }
            Mode::Fx => (FX_PADS[i].to_string(), s.decks[d].echo == Some(i)),
            Mode::Samples => {
                let name = &*s.entries[s.pads[d][i]].title;
                let short = name
                    .split_once(' ')
                    .map_or(name, |(n, rest)| if n.chars().all(|c| c.is_ascii_digit()) { rest } else { name });
                (short.to_string(), false)
            }
        };
        let fill = if lit { fade(colour, 0x60) } else { RAISED };
        p.outlined(x, y, cw, ch, 6.0, fill, 1.0, fade(colour, if lit { 0xff } else { 0x70 }));
        p.text_in(x + 5.0, y + ch - 6.0, cw - 8.0, if lit { BUTTON } else { LABEL }, &label);
        if s.decks[d].mode == Mode::Fx && i < 6 {
            p.text(x + 5.0, y + 14.0, LABEL, "ECHO");
        }
        hit(s, x, y, cw, ch, Target::Pad(d, i));
    }
}

fn knob(p: &mut Painter, x: f32, y: f32, k: f32, v: f32, colour: u32) {
    p.outlined(x, y, k, k, k * 0.5, RAISED, 1.5, LINE);
    let a = (v - 0.5) * 1.5 * std::f32::consts::PI;
    let (s, c) = a.sin_cos();
    let (cx, cy, r) = (x + k * 0.5, y + k * 0.5, k * 0.34);
    let lit = if (v - 0.5).abs() < 0.01 { DIM } else { colour };
    p.rect(cx + s * r - 2.5, cy - c * r - 2.5, 5.0, 5.0, 2.5, lit);
}

fn mixer(p: &mut Painter, s: &mut State, r: Rect) {
    let cx = r.x + r.w * 0.5;
    let k = ((r.h - 120.0) / 4.0 - PAD).clamp(18.0, (r.w * 0.2).min(34.0));
    let top = r.y + PAD * 2.0;
    for d in 0..2 {
        let x = r.x + r.w * [0.22, 0.78][d] - k * 0.5;
        for (n, &label) in KNOBS.iter().enumerate() {
            let y = top + n as f32 * (k + PAD);
            knob(p, x, y, k, s.decks[d].knobs[n], DECK[d]);
            hit(s, x - 4.0, y - 2.0, k + 8.0, k + 4.0, Target::Knob(d, n));
            if d == 0 {
                p.text(cx - p.text_width(LABEL, label) * 0.5, y + k * 0.5 + 4.0, LABEL, label);
            }
        }
        let ft = top + 4.0 * (k + PAD);
        let fh = (r.y + r.h - 44.0 - ft).max(20.0);
        let fx = r.x + r.w * [0.22, 0.78][d];
        p.rect(fx - 2.0, ft, 4.0, fh, 2.0, RAISED);
        let v = s.decks[d].volume;
        p.rect(fx - 13.0, ft + (1.0 - v) * (fh - 12.0), 26.0, 12.0, 3.0, TEXT);
        hit(s, fx - 18.0, ft, 36.0, fh, Target::Volume(d));
        let peak = s.telemetry.decks[d].peak.min(1.0);
        let mx = fx + [18.0, -22.0][d];
        p.rect(mx, ft, 4.0, fh, 2.0, RAISED);
        let lit = fh * peak;
        p.rect(mx, ft + fh - lit, 4.0, lit, 2.0, if peak > 0.95 { DECK[0] } else { PLAY });
    }
    let y = r.y + r.h - 28.0;
    let w = r.w - 6.0 * PAD;
    p.rect(r.x + 3.0 * PAD, y + 5.0, w, 4.0, 2.0, RAISED);
    let xf = r.x + 3.0 * PAD + (s.crossfader + 1.0) * 0.5 * (w - 16.0);
    p.rect(xf, y - 4.0, 16.0, 22.0, 3.0, ACCENT);
    hit(s, r.x + 2.0 * PAD, y - 8.0, w + 2.0 * PAD, 30.0, Target::Crossfader);
}

fn library(p: &mut Painter, s: &mut State, r: Rect) {
    s.library = r;
    s.row_h = ROW_H;
    let inner = r.w - 24.0;
    let cols = [(0.0, "#"), (0.07, "NAME"), (0.62, "FOLDER")];
    for (f, name) in cols {
        p.text(r.x + 12.0 + inner * f, r.y + 20.0, HEADER, name);
    }
    p.rect(r.x + 8.0, r.y + 28.0, r.w - 16.0, 1.0, 0.0, LINE);
    let first = (s.scroll / ROW_H) as usize;
    let rows = ((r.h - 34.0) / ROW_H) as usize;
    for (n, &i) in s.shown.iter().skip(first).take(rows + 1).enumerate() {
        let y = r.y + 30.0 + n as f32 * ROW_H - (s.scroll % ROW_H);
        if y < r.y + 29.0 || y + ROW_H > r.y + r.h {
            continue;
        }
        let e = &s.entries[i as usize];
        let loaded = s.decks.iter().position(|d| d.loaded.as_ref().is_some_and(|l| l.entry.source == e.source));
        if let Some(d) = loaded {
            p.rect(r.x + 6.0, y, r.w - 12.0, ROW_H, 4.0, fade(DECK[d.min(1)], 0x28));
        }
        let base = y + ROW_H - 7.0;
        p.text(r.x + 12.0, base, LABEL, &(first + n + 1).to_string());
        p.text_in(r.x + 12.0 + inner * 0.07, base, inner * 0.54, CELL, &e.title);
        p.text_in(r.x + 12.0 + inner * 0.62, base, inner * 0.38, SUB, &e.folder);
    }
    if s.shown.is_empty() {
        p.text(r.x + 12.0, r.y + 52.0, SUB, "No matches");
    }
    hit(s, r.x, r.y, r.w, r.h, Target::Library);
}

fn lyrics(p: &mut Painter, r: Rect) {
    p.text(r.x + 12.0, r.y + 20.0, HEADER, "LYRICS");
    p.text_in(r.x + 12.0, r.y + 48.0, r.w - 24.0, LYRIC, "No synced lyrics for this track");
}

fn ring(p: &mut Painter, s: &mut State, r: Rect) {
    let size = r.w.min(r.h) - 2.0 * PAD;
    let (cx, cy) = (r.x + r.w * 0.5, r.y + r.h * 0.5);
    let outer = size * 0.5;
    let hub = outer * 0.42;
    let band = (outer - hub) * 0.5 - 4.0;
    p.rect(cx - outer, cy - outer, outer * 2.0, outer * 2.0, outer, RAISED);
    let steps = ((outer * std::f32::consts::TAU) / 3.0).clamp(120.0, 720.0) as usize;
    for d in 0..2 {
        let mid = hub + 4.0 + band * (1.5 - d as f32) + 2.0 * (1.0 - d as f32);
        let deck = &s.decks[d];
        let Some(l) = &deck.loaded else {
            p.text(cx - 40.0, cy - mid + 4.0, SUB, ["A empty", "B empty"][d]);
            continue;
        };
        let centre = (s.telemetry.decks[d].position / DETAIL_FRAMES as f64) as isize;
        for k in 0..steps {
            let off = k as isize - steps as isize / 2;
            let Some(&[v, lo, md, hi]) = usize::try_from(centre + off).ok().and_then(|i| l.detail.get(i)) else {
                continue;
            };
            let angle = off as f32 / steps as f32 * std::f32::consts::TAU;
            let (sn, cs) = angle.sin_cos();
            let a = f32::from(v) / 255.0 * band * 0.95;
            let sum = (u32::from(lo) + u32::from(md) + u32::from(hi)).max(1);
            let ch = |x: u8| (u32::from(x) * 255 / sum * 2).min(255);
            let alpha = if off < 0 { 0x70 } else { 0xff };
            let col = ch(lo) | (ch(md) << 8) | (ch(hi) << 16) | (alpha << 24);
            let mut t = -a;
            while t <= a {
                let rr = mid + t;
                p.rect(cx + sn * rr - 1.0, cy - cs * rr - 1.0, 2.0, 2.0, 0.0, col);
                t += 2.0;
            }
        }
    }
    p.rect(cx - 1.0, r.y + PAD, 2.0, outer - hub, 1.0, TEXT);
    p.outlined(cx - hub, cy - hub, hub * 2.0, hub * 2.0, hub, PANEL, 1.5, LINE);
    for d in 0..2 {
        let ui = &s.decks[d];
        let bpm = ui
            .loaded
            .as_ref()
            .and_then(|l| l.grid)
            .map_or("--".into(), |g| format!("{:.1}", g.bpm * (1.0 + f64::from(ui.rate) * 0.08)));
        let y = cy - hub * 0.35 + d as f32 * 18.0;
        let label = format!("{}  {bpm}", ["A", "B"][d]);
        let style = Style::new(13.0, Weight::Semibold, DECK[d]);
        p.text(cx - p.text_width(style, &label) * 0.5, y, style, &label);
    }
    let w = hub * 1.3;
    let y = cy + hub * 0.35;
    p.rect(cx - w * 0.5, y, w, 4.0, 2.0, LINE);
    let xf = cx - w * 0.5 + (s.crossfader + 1.0) * 0.5 * (w - 14.0);
    p.rect(xf, y - 8.0, 14.0, 20.0, 3.0, ACCENT);
    hit(s, cx - w * 0.5 - 6.0, y - 14.0, w + 12.0, 32.0, Target::Crossfader);
}

#[cfg(test)]
mod tests {
    use super::*;
    use sugar_layout::Layout;

    #[test]
    fn ids_match_their_modules() {
        for (i, m) in ALL.into_iter().enumerate() {
            assert_eq!(m as usize, i);
        }
    }

    #[test]
    fn portrait_keeps_ring_and_decks_on_phones() {
        let mut layout = Layout::new(&portrait());
        let mut out = Vec::new();
        for (w, h) in [(360.0, 780.0), (412.0, 915.0), (600.0, 1024.0), (800.0, 1280.0)] {
            layout.solve(inset(Rect::new(0.0, 0.0, w, h)), &mut out);
            for m in [M::Ring, M::DeckA, M::DeckB] {
                assert!(out.iter().any(|p| p.0 == m as Id), "{w}x{h}");
            }
        }
    }

    #[test]
    fn decks_and_mixer_survive_every_size() {
        let mut layout = Layout::new(&tree());
        let mut out = Vec::new();
        for (w, h) in
            [(780.0, 360.0), (915.0, 412.0), (1024.0, 600.0), (1280.0, 800.0), (1920.0, 1080.0), (3840.0, 2160.0)]
        {
            layout.solve(inset(Rect::new(0.0, 0.0, w, h)), &mut out);
            for m in [M::DeckA, M::Mixer, M::DeckB] {
                assert!(out.iter().any(|p| p.0 == m as Id), "{w}x{h}");
            }
        }
    }
}
