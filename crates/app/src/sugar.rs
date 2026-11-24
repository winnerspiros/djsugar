use sugar_layout::Rect;
use sugar_ui::{Painter, Style, Weight, rgb};

use crate::state::{DETAIL_FRAMES, FX_PADS, GridOp, Hit, Mode, State, Target, View};
use sugar_engine::station::Scene;

use crate::theme::{self, Line, Mark};

const INK: u32 = rgb(0x000000);
const SURFACE: u32 = rgb(0x0d0e13);
const DEEP: u32 = rgb(0x0c0c0c);
const EDGE: u32 = rgb(0x2e2e2e);
const RAISED: u32 = rgb(0x2b2b2b);
const ACCENT: u32 = rgb(0xff4fa8);
const TEXT: u32 = rgb(0xe8e8e8);
const GREY: u32 = rgb(0xb0b0b0);
const DARK: u32 = rgb(0x555555);
const GREEN: u32 = rgb(0x00e676);
const RED: u32 = rgb(0xff6b6b);
const AMBER: u32 = rgb(0xffd54f);
pub const CUES: [u32; 8] = [
    rgb(0xe63380),
    rgb(0xff3b30),
    rgb(0xff9500),
    rgb(0xffd60a),
    rgb(0x30d158),
    rgb(0x40c8e0),
    rgb(0x2f7bff),
    rgb(0xaf52de),
];
const CUE_NAMES: [&str; 8] = ["Pink", "Red", "Orange", "Yellow", "Green", "Cyan", "Blue", "Purple"];
pub const DECK: [u32; 2] = [rgb(0x00b0ff), rgb(0xff6d00)];

const LOGO: Style = Style::new(16.0, Weight::Bold, rgb(0xe63380));
const TAB: Style = Style::new(11.0, Weight::Bold, TEXT);
const TITLE: Style = Style::new(14.0, Weight::Semibold, TEXT);
const ARTIST: Style = Style::new(10.0, Weight::Regular, GREY);
const SMALL: Style = Style::new(9.0, Weight::Semibold, GREY);
const TIME: Style = Style::new(14.0, Weight::Bold, TEXT);
const PAD_TEXT: Style = Style::new(10.0, Weight::Bold, TEXT);

const TOOLBAR: f32 = 36.0;
const ROW: f32 = 44.0;

pub const fn fade(c: u32, a: u32) -> u32 {
    c & 0x00ff_ffff | a << 24
}

pub fn hit(s: &mut State, r: Rect, target: Target) {
    s.hits.push(Hit { rect: r, target });
}

fn button(p: &mut Painter, s: &mut State, r: Rect, label: &str, on: bool, colour: u32, target: Target) {
    let down = s.pressed(target);
    theme::labelled(p, r, label, if r.h < 26.0 { 9.0 } else { 10.5 }, on, down, colour);
    hit(s, r, target);
}

pub fn paint(p: &mut Painter, s: &mut State, w: f32, h: f32) {
    s.hits.clear();
    p.rect(0.0, 0.0, w, h, 0.0, rgb(0x1e1e20));
    let tall = h > w * 1.1;
    let bar = if w < 720.0 || (tall && w < 1100.0) { TOOLBAR * 3.0 } else { TOOLBAR };
    toolbar(p, s, Rect::new(0.0, 0.0, w, bar));
    let mut y = bar;
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
            // The Classic phone layout: waveforms, decks around the mixer, the library below.
            let wh = (rest * 0.22).round().clamp(56.0, 96.0);
            waveforms(p, s, Rect::new(0.0, y, w, wh));
            let y = y + wh + 4.0;
            let lower = h - y;
            let dh = if tall {
                if lower < 900.0 { lower } else { (lower * 0.75).min(1000.0) }
            } else {
                crate::classic::height(lower * 0.62).max(150.0)
            }
            .min(lower);
            crate::classic::dj(p, s, Rect::new(0.0, y, w, dh), tall);
            if lower - dh > 60.0 {
                library(p, s, Rect::new(0.0, y + dh + 4.0, w, lower - dh - 4.0));
            }
        }
        View::Wave => {
            let bar = (rest * 0.38).round().clamp(86.0, 115.0);
            let lyr = if tall { (rest * 0.3).round() } else { 0.0 };
            waveforms(p, s, Rect::new(0.0, y, w, rest - bar - lyr));
            if lyr > 0.0 {
                lyrics(p, s, Rect::new(0.0, y + rest - bar - lyr, w, lyr));
            }
            minis(p, s, Rect::new(0.0, h - bar, w, bar));
        }
        View::Lib => {
            let m = minis(p, s, Rect::new(0.0, y, w, 96.0));
            library(p, s, Rect::new(0.0, y + m + 2.0, w, rest - m - 2.0));
        }
        View::Fx => {
            let wh = (rest * 0.2).round().clamp(48.0, 90.0);
            waveforms(p, s, Rect::new(0.0, y, w, wh));
            station(p, s, Rect::new(0.0, y + wh + 2.0, w, rest - wh - 2.0));
        }
        View::Stream => {
            let m = minis(p, s, Rect::new(0.0, y, w, 96.0));
            hub(p, s, Rect::new(0.0, y + m + 2.0, w, rest - m - 2.0));
        }
        View::Ctl => {
            let wh = (rest * 0.42).round();
            waveforms(p, s, Rect::new(0.0, y, w, wh));
            let lw = if tall { 0.0 } else { (w * 0.3).clamp(220.0, 380.0) };
            library(p, s, Rect::new(0.0, y + wh + 2.0, w - lw, rest - wh - 2.0));
            if lw > 0.0 {
                lyrics(p, s, Rect::new(w - lw, y + wh + 2.0, lw, rest - wh - 2.0));
            }
        }
    }
    if let Some(t) = s.view_at {
        let k = t.elapsed().as_secs_f32() / crate::state::VIEW_FADE.as_secs_f32();
        if k < 1.0 {
            let a = ((1.0 - k) * (1.0 - k) * 255.0) as u32;
            p.rect(0.0, bar, w, h - bar, 0.0, fade(rgb(0x000000), a));
        } else {
            s.view_at = None;
        }
    }
    browser(p, s, w, h, bar);
    cue_menu(p, s, w, h);
    grid_menu(p, s, w, h);
    picker(p, s, w, h);
    track_menu(p, s, w, h);
}

/// The brand mark: a glazed donut with sprinkles, then DJ in grey and SUGAR in pink.
/// Returns the right edge.
fn logo(p: &mut Painter, x: f32, cy: f32) -> f32 {
    let rad = 11.0;
    let cx = x + rad;
    p.disc(cx, cy, rad, rgb(0xd5944a), rgb(0xb3722d));
    p.disc(cx, cy, rad * 0.8, rgb(0xff99ca), rgb(0xe63380));
    for (i, colour) in [0xffffff, 0x7ad7ff, 0xfff27a, 0x9cff8a, 0xffffff, 0xc49bff].into_iter().enumerate() {
        let a = i as f32 * 1.047 + 0.4;
        let (sn, cs) = a.sin_cos();
        let (px, py) = (cx + sn * rad * 0.58, cy - cs * rad * 0.58);
        p.line((px - cs * 1.5, py - sn * 1.5), (px + cs * 1.5, py + sn * 1.5), 1.5, rgb(colour));
    }
    p.disc(cx, cy, rad * 0.3, rgb(0x2c2e34), rgb(0x1b1c20));
    p.disc(cx, cy, rad * 0.1, rgb(0xcfd4dc), rgb(0x0e1117));
    let st = Style { colour: rgb(0xb0b0b0), ..LOGO };
    let tx = cx + rad + 8.0;
    let w = p.text(tx, cy + st.px * 0.36, st, "DJ ");
    let w2 = p.text(tx + w, cy + st.px * 0.36, Style { colour: ACCENT, ..LOGO }, "SUGAR");
    tx + w + w2
}

fn toolbar(p: &mut Painter, s: &mut State, r: Rect) {
    p.rect(r.x, r.y, r.w, r.h, 0.0, theme::PANEL);
    p.rect(r.x, r.y + r.h - 1.0, r.w, 1.0, 0.0, theme::SEPARATOR);
    let two = r.h > TOOLBAR;
    let mut x = logo(p, r.x + 8.0, r.y + TOOLBAR * 0.5) + 12.0;
    let views = [
        ("DJ", View::Dj),
        ("WAVE", View::Wave),
        ("LIB", View::Lib),
        ("FX", View::Fx),
        ("STREAM", View::Stream),
        ("CTL", View::Ctl),
    ];
    let tw = if two {
        ((r.w - x - 6.0) / views.len() as f32).min(64.0)
    } else if r.w < 760.0 {
        44.0
    } else {
        58.0
    };
    // The views as a segmented control: the open one raised, with an accent underline.
    p.outlined(x, r.y + 4.0, tw * views.len() as f32, TOOLBAR - 8.0, 9.0, theme::WELL, 1.0, theme::BORDER);
    for (n, (label, v)) in views.into_iter().enumerate() {
        let b = Rect::new(x + n as f32 * tw, r.y + 4.0, tw, TOOLBAR - 8.0);
        if s.view == v {
            p.glow(b.x + 2.0, b.y + 3.0, b.w - 4.0, b.h - 4.0, 7.0, 4.0, fade(rgb(0x000000), 0x90));
            p.outlined(b.x + 2.0, b.y + 2.0, b.w - 4.0, b.h - 4.0, 7.0, theme::RAISED, 1.0, theme::BORDER);
            p.rect(b.x + b.w * 0.5 - 8.0, b.y + b.h - 5.0, 16.0, 2.0, 1.0, ACCENT);
        } else if s.pressed(Target::View(v)) {
            p.rect(b.x + 2.0, b.y + 2.0, b.w - 4.0, b.h - 4.0, 7.0, theme::PRESSED);
        }
        let ink = Style { colour: if s.view == v { TEXT } else { theme::DIM }, ..TAB };
        p.text(b.x + (b.w - p.text_width(ink, label)) * 0.5, b.y + b.h * 0.5 + 4.0, ink, label);
        hit(s, b, Target::View(v));
    }
    x += tw * views.len() as f32 + 8.0;
    let r = if two {
        x = r.x + 6.0;
        p.rect(r.x, r.y + TOOLBAR - 1.0, r.w, 1.0, 0.0, theme::SEPARATOR);
        Rect::new(r.x, r.y + TOOLBAR, r.w, TOOLBAR)
    } else {
        r
    };
    let h = r.h - 8.0;
    let rec = s.recorder.as_ref().map(|r| {
        let t = r.frames / u64::from(s.out_rate.max(1));
        format!("REC {}:{:02}", t / 60, t % 60)
    });
    let rec_label = rec.as_deref().unwrap_or("REC");
    let items: [(&str, bool, Target); 6] = [
        ("LOAD 1", s.browser == Some(0), Target::Browse(0)),
        ("LOAD 2", s.browser == Some(1), Target::Browse(1)),
        ("FX", s.show_fx, Target::ToggleFx),
        ("SMPL", s.show_samplers, Target::ToggleSamplers),
        ("AUTO", s.auto.is_some(), Target::AutoDj),
        (rec_label, s.recorder.is_some(), Target::Record),
    ];
    for (label, on, t) in items {
        let bw = if label.len() > 6 {
            78.0
        } else if label.len() > 4 {
            58.0
        } else {
            46.0
        };
        let bw = if two { (r.w - 32.0) / 6.0 } else { bw };
        let colour = if matches!(t, Target::Record) { RED } else { ACCENT };
        if !two && x + bw > r.w - 110.0 {
            continue;
        }
        button(p, s, Rect::new(x, r.y + 4.0, bw, h), label, on, colour, t);
        x += bw + 4.0;
    }
    let r = if two {
        p.rect(r.x, r.y + TOOLBAR - 1.0, r.w, 1.0, 0.0, EDGE);
        Rect::new(r.x, r.y + TOOLBAR, r.w, TOOLBAR)
    } else {
        r
    };
    let sx = if two { r.x + 6.0 } else { (r.w - 300.0).max(x + 2.0) };
    let sw = r.w - sx - 8.0;
    let focus = s.search_focus || !s.query.is_empty();
    let fh = h - 2.0;
    p.outlined(sx, r.y + 5.0, sw, fh, fh * 0.5, theme::WELL, 1.0, if focus { ACCENT } else { theme::BORDER });
    // A magnifier ahead of the text.
    let (mx, my) = (sx + 15.0, r.y + 5.0 + fh * 0.5 - 1.0);
    p.outlined(mx - 4.5, my - 4.5, 9.0, 9.0, 4.5, 0, 1.5, if focus { ACCENT } else { theme::DIM });
    p.line((mx + 3.5, my + 3.5), (mx + 6.5, my + 6.5), 1.5, if focus { ACCENT } else { theme::DIM });
    hit(s, Rect::new(sx, r.y, sw, r.h), Target::SearchFocus);
    let tx = sx + 28.0;
    if s.query.is_empty() {
        let hint = s.status.as_deref().unwrap_or("Search tracks");
        p.text_in(tx, r.y + 22.0, sw - 36.0, Style { colour: theme::FAINT, ..ARTIST }, hint);
    } else {
        let qw = p.text_in(tx, r.y + 22.0, sw - 68.0, Style { px: 12.0, ..TITLE }, &s.query);
        p.rect(tx + 1.0 + qw, r.y + 11.0, 1.5, 15.0, 0.0, ACCENT);
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
        let cols = r.w as usize;
        let centre = pos / DETAIL_FRAMES as f64;
        let half = r.h * 0.47;
        for c in 0..cols {
            let i = centre + f64::from((c as f32 - cols as f32 * 0.5) * speed);
            let Some(&[v, lo, md, hi]) = (i >= 0.0).then(|| l.detail.get(i as usize)).flatten() else { continue };
            let past = (c as f32) < cols as f32 * 0.5;
            let a = if past { 0x70 } else { 0xff };
            let x = r.x + c as f32;
            let amp = f32::from(v) / 255.0 * half;
            let total = (f32::from(lo) + f32::from(md) + f32::from(hi)).max(1.0);
            let lo_h = amp * (f32::from(lo) / total).sqrt().min(1.0);
            let md_h = amp * (f32::from(md) / total).sqrt().min(1.0) * 0.8;
            let hi_h = amp * (f32::from(hi) / total).sqrt().min(1.0) * 0.6;
            p.rect(x, mid - amp, 1.0, amp * 2.0, 0.0, fade(rgb(0x1e3c78), a));
            p.rect(x, mid - lo_h, 1.0, lo_h * 2.0, 0.0, fade(rgb(0xd8304a), a));
            p.rect(x, mid - md_h, 1.0, md_h * 2.0, 0.0, fade(rgb(0x3ccf6e), a));
            p.rect(x, mid - hi_h, 1.0, hi_h * 2.0, 0.0, fade(rgb(0x7fd8ff), a));
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
    p.glow(r.x + r.w * 0.5 - 1.0, r.y, 2.0, r.h, 1.0, 4.0, fade(DECK[d], 0xc0));
    p.rect(r.x + r.w * 0.5 - 1.0, r.y, 2.0, r.h, 1.0, TEXT);
    p.rect(r.x + 4.0, r.y + 4.0, 3.0, r.h - 8.0, 1.5, DECK[d]);
}

pub fn cue_colour(s: &State, d: usize, i: usize) -> u32 {
    CUES[usize::from(s.decks[d].cue_colours[i]) % 8]
}

/// A hot cue's menu where it was held: its colour in a submenu, and delete.
fn cue_menu(p: &mut Painter, s: &mut State, w: f32, h: f32) {
    let Some((d, i, x, y)) = s.cue_menu else { return };
    p.layer();
    let screen = Rect::new(0.0, 0.0, w, h);
    hit(s, screen, Target::CueMenuClose);
    let now = usize::from(s.decks[d].cue_colours[i]) % CUES.len();
    let lines = [
        Line::item(format!("Hot cue {}", i + 1), Target::MenuBody).enabled(false).hint(format!("Deck {}", d + 1)),
        Line::Rule,
        Line::sub("Colour", 0).hint(CUE_NAMES[now]),
        Line::Rule,
        Line::item("Delete cue", Target::CueClear),
    ];
    let (open, r) = theme::menu(p, s, (x, y), screen, &lines);
    if let Some(top) = open {
        let colours: Vec<Line> = CUES
            .iter()
            .zip(CUE_NAMES)
            .enumerate()
            .map(|(n, (&c, name))| {
                Line::item(name, Target::CueColour(n as u8)).mark(Mark::Swatch(c)).hint(if n == now {
                    "Current"
                } else {
                    ""
                })
            })
            .collect();
        theme::submenu(p, s, r, top, screen, &colours);
    }
}

/// The beat grid's menu beside the deck's GRID button.
fn grid_menu(p: &mut Painter, s: &mut State, w: f32, h: f32) {
    let Some(d) = s.grid_menu else { return };
    p.layer();
    let screen = Rect::new(0.0, 0.0, w, h);
    hit(s, screen, Target::GridClose);
    let at = s.rect_of(Target::Grid(d)).map_or(((w - theme::MENU_W) * 0.5, h * 0.2), |b| (b.x, b.y + b.h + 4.0));
    let locked = s.decks[d].grid_locked;
    let edits = !locked;
    let bpm = s.decks[d].beat().map_or_else(String::new, |(g, _)| format!("{:.2} BPM", g.bpm));
    let op = |text: &str, o: GridOp, on: bool| Line::item(text, Target::GridOp(o)).enabled(on);
    let lines = [
        Line::item(format!("Beat grid, deck {}", d + 1), Target::MenuBody).enabled(false).hint(bpm),
        Line::Rule,
        op("Downbeat here", GridOp::Here, edits),
        op("Move earlier", GridOp::Earlier, edits).hint("-10 ms"),
        op("Move later", GridOp::Later, edits).hint("+10 ms"),
        op("Shift half a beat", GridOp::Half, edits),
        Line::Rule,
        op("Halve BPM", GridOp::Halve, edits).hint("/2"),
        op("Double BPM", GridOp::Double, edits).hint("x2"),
        op("Match other deck", GridOp::Match, edits),
        Line::Rule,
        op("Undo", GridOp::Undo, edits && !s.decks[d].grid_undo.is_empty()),
        op("Lock grid", GridOp::Lock, true).mark(Mark::Check(locked)),
    ];
    theme::menu(p, s, at, screen, &lines);
}

fn clock(secs: f64) -> String {
    let t = secs.max(0.0);
    format!("{:02}:{:04.1}", (t / 60.0) as u32, t % 60.0)
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

fn mmss(secs: f32) -> String {
    let t = secs.max(0.0) as u32;
    format!("{}:{:02}", t / 60, t % 60)
}

/// Library sources: tracks, samples, history and the playlists.
fn sidebar(p: &mut Painter, s: &mut State, r: Rect) {
    use crate::state::Shelf;
    p.rect(r.x, r.y, r.w, r.h, 0.0, INK);
    p.rect(r.x + r.w - 1.0, r.y, 1.0, r.h, 0.0, EDGE);
    let mut items: Vec<(String, Shelf)> =
        vec![("Tracks".into(), Shelf::Tracks), (format!("History  {}", s.history.len()), Shelf::History)];
    for (n, (name, keys)) in s.playlists.iter().enumerate() {
        items.push((format!("{name}  {}", keys.len()), Shelf::Playlist(n)));
    }
    let rh = 28.0;
    let mut y = r.y + 6.0;
    p.text(r.x + 12.0, y + 12.0, SMALL, "LIBRARY");
    y += 18.0;
    for (n, (label, sh)) in items.iter().enumerate() {
        if n == 2 {
            p.text(r.x + 12.0, y + 14.0, SMALL, "PLAYLISTS");
            y += 20.0;
        }
        if y + rh > r.y + r.h - rh {
            break;
        }
        let on = s.shelf == *sh;
        let b = Rect::new(r.x + 6.0, y, r.w - 12.0, rh - 2.0);
        if on {
            p.rect(b.x, b.y, b.w, b.h, 5.0, fade(ACCENT, 0x30));
            p.rect(b.x, b.y + 4.0, 3.0, b.h - 8.0, 1.5, ACCENT);
        }
        p.text_in(
            b.x + 10.0,
            b.y + b.h * 0.5 + 4.0,
            b.w - 14.0,
            Style { colour: if on { TEXT } else { GREY }, ..ARTIST },
            label,
        );
        hit(s, b, Target::Shelf(*sh));
        y += rh;
    }
    if items.len() == 2 {
        p.text(r.x + 12.0, y + 14.0, SMALL, "PLAYLISTS");
        y += 20.0;
    }
    let b = Rect::new(r.x + 6.0, (y + 4.0).min(r.y + r.h - rh), r.w - 12.0, rh - 4.0);
    button(p, s, b, "+ NEW PLAYLIST", false, ACCENT, Target::NewPlaylist);
}

/// A held track's menu where it was held: load it to a deck, or add it to playlists.
fn track_menu(p: &mut Painter, s: &mut State, w: f32, h: f32) {
    let Some(i) = s.track_menu else { return };
    p.layer();
    let screen = Rect::new(0.0, 0.0, w, h);
    hit(s, screen, Target::MenuClose);
    let e = &s.entries[i as usize];
    let key = crate::state::entry_key(e);
    let title = if e.artist.is_empty() { e.title.to_string() } else { format!("{} - {}", e.artist, e.title) };
    let held = s.playlists.iter().filter(|(_, k)| k.contains(&key)).count();
    let lines = [
        Line::item(title, Target::MenuBody).enabled(false),
        Line::Rule,
        Line::item("Load on deck 1", Target::MenuDeck(0)).enabled(!s.telemetry.decks[0].playing),
        Line::item("Load on deck 2", Target::MenuDeck(1)).enabled(!s.telemetry.decks[1].playing),
        Line::Rule,
        Line::sub("Add to playlist", 0).hint(if held > 0 { format!("In {held}") } else { String::new() }),
    ];
    let (open, r) = theme::menu(p, s, s.menu_at, screen, &lines);
    if let Some(top) = open {
        let mut lists: Vec<Line> = s
            .playlists
            .iter()
            .enumerate()
            .map(|(n, (name, keys))| {
                Line::item(name.clone(), Target::MenuToggle(n)).mark(Mark::Check(keys.contains(&key)))
            })
            .collect();
        if lists.is_empty() {
            lists.push(Line::item("No playlists yet", Target::MenuBody).enabled(false));
        }
        lists.push(Line::Rule);
        lists.push(Line::item("New playlist", Target::NewPlaylist));
        theme::submenu(p, s, r, top, screen, &lists);
    }
}

pub fn library(p: &mut Painter, s: &mut State, r: Rect) {
    let r = if r.h > 220.0 { library_head(p, s, r) } else { r };
    let r = if r.w >= 640.0 {
        let sw = (r.w * 0.2).clamp(150.0, 220.0);
        sidebar(p, s, Rect::new(r.x, r.y, sw, r.h));
        Rect::new(r.x + sw, r.y, r.w - sw, r.h)
    } else {
        r
    };
    table(p, s, r);
}

fn table(p: &mut Painter, s: &mut State, r: Rect) {
    use crate::library::Column as C;
    p.rect(r.x, r.y, r.w, r.h, 0.0, rgb(0x000000));
    let bar = 40.0;
    let list = Rect::new(r.x, r.y, r.w, r.h - bar);
    s.library = list;
    s.row_h = 22.0;
    let wide = r.w >= 600.0;
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
    let y = r.y + r.h - bar;
    p.rect(r.x, y, r.w, bar, 0.0, rgb(0x0c0c0c));
    let Some(i) = s.selected else {
        p.text(r.x + 12.0, y + 25.0, Style { colour: GREY, ..TITLE }, "Select a track to load");
        return;
    };
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

/// The library's header in the DJ view: source tabs, then the search line.
fn library_head(p: &mut Painter, s: &mut State, r: Rect) -> Rect {
    let tab = Style::new(11.0, Weight::Bold, ACCENT);
    let mut x = r.x + 12.0;
    for (label, on, t) in
        [("LOCAL MUSIC", true, Target::View(View::Lib)), ("STREAMING HUB", false, Target::View(View::Stream))]
    {
        let w = p.text_width(tab, label);
        let st = Style { colour: if on { ACCENT } else { rgb(0xff99ca) }, ..tab };
        p.text(x, r.y + 17.0, st, label);
        if on {
            p.rect(x, r.y + 22.0, w, 2.0, 1.0, ACCENT);
        }
        hit(s, Rect::new(x - 6.0, r.y, w + 12.0, 26.0), t);
        x += w + 30.0;
    }
    let y = r.y + 28.0;
    let sh = 34.0;
    let focus = s.search_focus || !s.query.is_empty();
    p.rect(r.x + 8.0, y + sh - 2.0, r.w - 16.0, 2.0, 1.0, if focus { ACCENT } else { fade(ACCENT, 0x80) });
    if s.query.is_empty() {
        p.text_in(
            r.x + 16.0,
            y + 22.0,
            r.w - 32.0,
            Style::new(13.0, Weight::Regular, GREY),
            "Search songs, artists, albums",
        );
    } else {
        let qw = p.text_in(r.x + 16.0, y + 22.0, r.w - 40.0, Style::new(13.0, Weight::Regular, TEXT), &s.query);
        p.rect(r.x + 17.0 + qw, y + 9.0, 1.5, 16.0, 0.0, ACCENT);
    }
    hit(s, Rect::new(r.x, y, r.w, sh), Target::SearchFocus);
    Rect::new(r.x, y + sh + 4.0, r.w, r.h - sh - 32.0)
}

/// Length the TIME knob picks for the scene, as the page shows it.
fn station_time(scene: Scene, v: f32) -> String {
    const ECHO: [&str; 6] = ["1/8", "1/4", "3/8", "1/2", "3/4", "1"];
    const SWEEP: [&str; 6] = ["1", "2", "4", "8", "16", "32"];
    const SLICE: [&str; 5] = ["1/16", "1/8", "1/4", "1/2", "1"];
    let at = |n: usize| ((v * n as f32) as usize).min(n - 1);
    match scene {
        Scene::Flanger | Scene::Phaser => format!("{} BEATS", SWEEP[at(6)]),
        Scene::Roll | Scene::Gater => format!("{} BEAT", SLICE[at(5)]),
        Scene::Pitch => format!("{:.0} ST", 1.0 + 23.0 * v),
        Scene::Noise | Scene::Drive | Scene::Filter => format!("{:.0}%", v * 100.0),
        Scene::ReverbUp | Scene::ReverbDown => format!("SIZE {:.0}", v * 100.0),
        _ => format!("{} BEAT", ECHO[at(6)]),
    }
}

/// The FX station page: scenes, the XY pad, time, isolator and release.
fn station(p: &mut Painter, s: &mut State, r: Rect) {
    const VIOLET: u32 = rgb(0xb04cff);
    let pad = 8.0;
    let r = Rect::new(r.x + pad, r.y + pad, r.w - pad * 2.0, r.h - pad * 2.0);
    let st = s.station;
    // Scenes, four by four
    let gw = (r.w * 0.34).min(420.0);
    let (cols, rows) = (4.0, 4.0);
    let (bw, bh) = ((gw - 3.0 * 5.0) / cols, ((r.h - 3.0 * 5.0) / rows).min(56.0));
    for (i, sc) in Scene::ALL.iter().enumerate() {
        let (c, rr) = ((i % 4) as f32, (i / 4) as f32);
        let b = Rect::new(r.x + c * (bw + 5.0), r.y + rr * (bh + 5.0), bw, bh);
        let colour = if i < 6 {
            ACCENT
        } else if i < 9 {
            VIOLET
        } else {
            DECK[0]
        };
        button(p, s, b, sc.name(), st.scene == *sc, colour, Target::Scene(i));
    }
    // Right column
    let cw = (r.w * 0.26).clamp(170.0, 280.0);
    let cx = r.x + r.w - cw;
    p.outlined(cx, r.y, cw, r.h, 8.0, SURFACE, 1.0, EDGE);
    let head = Style::new(13.0, Weight::Bold, TEXT);
    p.text(cx + 10.0, r.y + 20.0, head, "FX STATION");
    let bpm = format!("{:.1} BPM", st.bpm);
    let bs = Style::new(12.0, Weight::Bold, AMBER);
    p.text(cx + cw - 10.0 - p.text_width(bs, &bpm), r.y + 20.0, bs, &bpm);
    let k = ((cw - 20.0) / 3.0).min((r.h - 120.0) / 2.0).clamp(36.0, 70.0);
    let time = station_time(st.scene, st.time);
    knob(p, s, Rect::new(cx + 10.0, r.y + 30.0, k, k + 12.0), st.time, AMBER, "TIME", Target::StationTime);
    let ts = Style::new(15.0, Weight::Bold, AMBER);
    p.text(cx + 20.0 + k, r.y + 30.0 + k * 0.5 + 5.0, ts, &time);
    let iso_y = r.y + 30.0 + k + 18.0;
    p.text(cx + 10.0, iso_y + 10.0, SMALL, "ISOLATE");
    for (n, label) in ["LOW", "MID", "HIGH"].into_iter().enumerate() {
        let kr = Rect::new(cx + 10.0 + n as f32 * (cw - 20.0) / 3.0, iso_y + 16.0, k, k + 12.0);
        knob(p, s, kr, (st.iso[n] + 1.0) * 0.5, RED, label, Target::Iso(n));
    }
    let ry = iso_y + 16.0 + k + 18.0;
    let rh = ((r.y + r.h - ry - 22.0) / 1.0).clamp(24.0, 44.0);
    p.text(cx + 10.0, ry + 10.0, SMALL, "RELEASE");
    let rw = (cw - 20.0 - 10.0) / 3.0;
    for (n, label) in ["BRAKE", "ECHO", "SPIN"].into_iter().enumerate() {
        let b = Rect::new(cx + 10.0 + n as f32 * (rw + 5.0), ry + 16.0, rw, rh);
        button(p, s, b, label, false, GREEN, Target::Release(n));
    }
    // XY pad in between
    let xr = Rect::new(r.x + gw + 10.0, r.y, cx - (r.x + gw) - 20.0, r.h);
    let live = s.pressed(Target::Xy);
    if live {
        p.glow(xr.x, xr.y, xr.w, xr.h, 10.0, 12.0, fade(ACCENT, 0x60));
    }
    p.gradient(xr.x, xr.y, xr.w, xr.h, 10.0, rgb(0x10121a), rgb(0x07080c), 1.0, if live { ACCENT } else { EDGE });
    for n in 1..4 {
        let f = n as f32 / 4.0;
        p.rect(xr.x + xr.w * f, xr.y + 6.0, 1.0, xr.h - 12.0, 0.0, rgb(0x1a1c24));
        p.rect(xr.x + 6.0, xr.y + xr.h * f, xr.w - 12.0, 1.0, 0.0, rgb(0x1a1c24));
    }
    let label = Style::new(11.0, Weight::Bold, DARK);
    p.text(xr.x + 10.0, xr.y + xr.h - 10.0, label, "AMOUNT");
    p.text(xr.x + 10.0, xr.y + 18.0, label, "DEPTH");
    let name = Style::new(20.0, Weight::Bold, if live { TEXT } else { fade(TEXT, 0x50) });
    let sn = st.scene.name();
    p.text(xr.x + (xr.w - p.text_width(name, sn)) * 0.5, xr.y + xr.h * 0.5 + 7.0, name, sn);
    let (px, py) = (xr.x + st.xy.0 * xr.w, xr.y + (1.0 - st.xy.1) * xr.h);
    if live {
        p.rect(px, xr.y + 4.0, 1.0, xr.h - 8.0, 0.0, fade(ACCENT, 0x80));
        p.rect(xr.x + 4.0, py, xr.w - 8.0, 1.0, 0.0, fade(ACCENT, 0x80));
        p.glow(px - 14.0, py - 14.0, 28.0, 28.0, 14.0, 10.0, fade(ACCENT, 0xa0));
        p.disc(px, py, 12.0, rgb(0xffc2e2), ACCENT);
    } else {
        p.outlined(px - 10.0, py - 10.0, 20.0, 20.0, 10.0, 0, 1.5, fade(ACCENT, 0x80));
    }
    hit(s, xr, Target::Xy);
}

/// Sample picker over everything: tap a sound to hear it on the pad.
fn picker(p: &mut Painter, s: &mut State, w: f32, h: f32) {
    let Some((d, pad)) = s.picker else { return };
    p.layer();
    p.rect(0.0, 0.0, w, h, 0.0, fade(rgb(0x000000), 0xb0));
    hit(s, Rect::new(0.0, 0.0, w, h), Target::PickerClose);
    let r = Rect::new(20.0, 20.0, w - 40.0, h - 40.0);
    theme::card(p, r, DECK[d]);
    hit(s, r, Target::Picker);
    let bank = ["A", "B", "C", "D"][s.bank[d] % 4];
    let head = format!("DECK {} SAMPLE PAD {bank}{}", d + 1, pad + 1);
    p.text(r.x + 12.0, r.y + 22.0, TAB, &head);
    let current = s.pads[d][s.bank[d] * 8 + pad];
    let name = crate::state::sample_name(s, current);
    let cs = Style { colour: DECK[d], ..TAB };
    p.text(r.x + 24.0 + p.text_width(TAB, &head), r.y + 22.0, cs, &name);
    let done = Rect::new(r.x + r.w - 82.0, r.y + 6.0, 72.0, 24.0);
    button(p, s, done, "DONE", true, DECK[d], Target::PickerClose);
    let list = Rect::new(r.x + 8.0, r.y + 38.0, r.w - 16.0, r.h - 46.0);
    let cols = ((list.w / 150.0) as usize).max(2);
    let cell = (list.w / cols as f32, 30.0);
    s.picker_grid = crate::state::Picker { cols, cell };
    hit(s, list, Target::Picker);
    let first = (s.picker_scroll / cell.1) as usize * cols;
    let rows = (list.h / cell.1) as usize + 2;
    for i in first..(first + rows * cols).min(crate::state::SAMPLES.len()) {
        let (c, row) = (i % cols, i / cols);
        let b = Rect::new(
            list.x + c as f32 * cell.0,
            list.y + row as f32 * cell.1 - s.picker_scroll,
            cell.0 - 4.0,
            cell.1 - 4.0,
        );
        if b.y < list.y - 1.0 || b.y + b.h > list.y + list.h + 1.0 {
            continue;
        }
        let on = i == current;
        p.gradient(
            b.x,
            b.y,
            b.w,
            b.h,
            6.0,
            if on { fade(DECK[d], 0x50) } else { theme::RAISED },
            if on { fade(DECK[d], 0x28) } else { theme::CONTROL },
            1.0,
            if on { DECK[d] } else { theme::BORDER },
        );
        let label = crate::state::sample_name(s, i);
        p.text_in(
            b.x + 8.0,
            b.y + b.h * 0.5 + 4.0,
            b.w - 12.0,
            Style { colour: if on { TEXT } else { GREY }, ..SMALL },
            &label,
        );
    }
}

pub const HUB_ROW: f32 = 44.0;

/// Streaming hub: service tabs, search, results with a load button per deck.
fn hub(p: &mut Painter, s: &mut State, r: Rect) {
    use crate::hub::Service;
    p.rect(r.x, r.y, r.w, r.h, 0.0, INK);
    let bar = 40.0;
    p.rect(r.x, r.y + bar - 1.0, r.w, 1.0, 0.0, EDGE);
    let mut x = r.x + 10.0;
    for sv in Service::ALL {
        let on = s.hub.service == sv;
        let tw = p.text_width(TAB, sv.name()) + 24.0;
        p.text(x + 12.0, r.y + 24.0, Style { colour: if on { TEXT } else { GREY }, ..TAB }, sv.name());
        if on {
            p.rect(x + 8.0, r.y + bar - 3.0, tw - 16.0, 2.0, 1.0, rgb(0xff0033));
        }
        x += tw;
    }
    let fx = x + 12.0;
    let fw = (r.w - fx - 110.0).max(120.0);
    let field = Rect::new(fx, r.y + 6.0, fw, bar - 12.0);
    p.outlined(field.x, field.y, field.w, field.h, 4.0, DEEP, 1.0, if s.hub.focus { ACCENT } else { EDGE });
    let (shown, ink) = if s.hub.query.is_empty() && !s.hub.focus {
        ("Search YouTube".to_string(), DARK)
    } else {
        (format!("{}{}", s.hub.query, if s.hub.focus { "|" } else { "" }), TEXT)
    };
    p.text_in(field.x + 10.0, field.y + field.h * 0.5 + 4.0, field.w - 20.0, Style { colour: ink, ..ARTIST }, &shown);
    hit(s, field, Target::HubFocus);
    let busy = s.hub.searching;
    button(
        p,
        s,
        Rect::new(field.x + field.w + 8.0, field.y, 90.0, field.h),
        if busy { "SEARCHING" } else { "SEARCH" },
        busy,
        ACCENT,
        Target::HubSearch,
    );
    let status_h = if s.hub.status.is_empty() { 0.0 } else { 24.0 };
    if status_h > 0.0 {
        let status = s.hub.status.clone();
        p.text_in(r.x + 12.0, r.y + bar + 16.0, r.w - 24.0, Style { colour: AMBER, ..ARTIST }, &status);
    }
    let list = Rect::new(r.x, r.y + bar + status_h, r.w, r.h - bar - status_h);
    s.hub_area = list;
    if s.hub.hits.is_empty() {
        let msg = if busy {
            "Searching"
        } else {
            "Search YouTube, then send a result to deck 1 or 2. Audio is cached for next time."
        };
        p.text(
            list.x + (list.w - p.text_width(ARTIST, msg)) * 0.5,
            list.y + 40.0,
            Style { colour: GREY, ..ARTIST },
            msg,
        );
        return;
    }
    hit(s, list, Target::HubList);
    let first = (s.hub.scroll / HUB_ROW).floor().max(0.0) as usize;
    let rows = (list.h / HUB_ROW).ceil() as usize + 1;
    let bw = 52.0;
    for i in first..(first + rows).min(s.hub.hits.len()) {
        let y = list.y + i as f32 * HUB_ROW - s.hub.scroll;
        if y < list.y - 1.0 || y + HUB_ROW > list.y + list.h + 1.0 {
            continue;
        }
        if i % 2 == 1 {
            p.rect(list.x, y, list.w, HUB_ROW, 0.0, rgb(0x121216));
        }
        let h = &s.hub.hits[i];
        let (title, artist, secs) = (h.title.clone(), h.artist.clone(), h.seconds);
        let tw = list.w - 2.0 * (bw + 6.0) - 90.0;
        p.text_in(list.x + 12.0, y + 18.0, tw, Style::new(12.5, Weight::Semibold, TEXT), &title);
        p.text_in(list.x + 12.0, y + 34.0, tw, ARTIST, &artist);
        let t = mmss(secs as f32);
        p.text(list.x + list.w - 2.0 * (bw + 6.0) - 12.0 - p.text_width(ARTIST, &t), y + 26.0, ARTIST, &t);
        for (d, &colour) in DECK.iter().enumerate() {
            let b = Rect::new(list.x + list.w - (2 - d) as f32 * (bw + 6.0), y + 7.0, bw, HUB_ROW - 14.0);
            let getting = s.hub.fetching.iter().find(|f| f.0 == s.hub.hits[i].id && f.1 == d).map(|f| f.2);
            let label = match getting {
                Some(f) => format!("{:.0}%", f * 100.0),
                None => format!("DECK {}", d + 1),
            };
            button(p, s, b, &label, getting.is_some(), colour, Target::HubLoad(i, d));
            if let Some(f) = getting {
                p.rect(b.x + 2.0, b.y + b.h - 3.0, (b.w - 4.0) * f, 2.0, 1.0, colour);
            }
        }
    }
}

/// DJ Sugar's `section.svg`: a black frame lit along the top, shaded along the bottom.
fn section(p: &mut Painter, r: Rect, fill: u32) {
    p.outlined(r.x, r.y, r.w, r.h, 0.0, fill, 1.0, rgb(0x000000));
    p.rect(r.x + 1.0, r.y + 1.0, r.w - 2.0, 2.0, 0.0, fade(rgb(0xffffff), 0x10));
    p.rect(r.x + 1.0, r.y + r.h - 3.0, r.w - 2.0, 2.0, 0.0, fade(rgb(0x000000), 0x20));
    p.rect(r.x + 1.0, r.y + 1.0, 2.0, r.h - 2.0, 0.0, fade(rgb(0xffffff), 0x08));
    p.rect(r.x + r.w - 3.0, r.y + 1.0, 2.0, r.h - 2.0, 0.0, fade(rgb(0xffffff), 0x08));
}

/// DJ Sugar's `knob.svg`: shadow, a #303030 to #181818 body inside a lit rim,
/// then a #404040 to #282828 cap.
fn skin_knob(p: &mut Painter, cx: f32, cy: f32, rad: f32) {
    p.disc(cx, cy, rad, fade(rgb(0x000000), 0x80), fade(rgb(0x000000), 0x80));
    p.outlined(cx - rad * 0.95, cy - rad * 0.95, rad * 1.9, rad * 1.9, rad * 0.95, 0, 1.0, fade(rgb(0xffffff), 0x10));
    let b = rad * 0.9;
    p.gradient(cx - b, cy - b, b * 2.0, b * 2.0, b, rgb(0x303030), rgb(0x181818), 0.0, 0);
    let c = rad * 0.71;
    p.outlined(cx - c - 1.0, cy - c - 1.0, c * 2.0 + 2.0, c * 2.0 + 2.0, c + 1.0, 0, 1.0, fade(rgb(0x000000), 0x40));
    p.gradient(cx - c, cy - c, c * 2.0, c * 2.0, c, rgb(0x404040), rgb(0x282828), 0.0, 0);
    p.outlined(cx - c, cy - c, c * 2.0, c * 2.0, c, 0, 1.0, fade(rgb(0xffffff), 0x14));
}

/// Both decks' summaries side by side, or stacked when a column is too narrow; returns the height used.
fn minis(p: &mut Painter, s: &mut State, r: Rect) -> f32 {
    if r.w >= 560.0 {
        for d in 0..2 {
            mini_deck(p, s, Rect::new(r.x + d as f32 * r.w * 0.5, r.y, r.w * 0.5, r.h), d);
        }
        return r.h;
    }
    let h = (r.h * 0.5).max(52.0);
    for d in 0..2 {
        mini_deck(p, s, Rect::new(r.x, r.y + d as f32 * h, r.w, h), d);
    }
    2.0 * h
}

/// Synced lyrics of the playing deck, the current line in the deck colour.
fn lyrics(p: &mut Painter, s: &State, r: Rect) {
    section(p, r, INK);
    p.text(r.x + 12.0, r.y + 20.0, SMALL, "LYRICS");
    let has = |d: usize| s.decks[d].loaded.as_ref().is_some_and(|l| !l.lyrics.lines.is_empty());
    let Some(d) = (0..2).find(|&d| s.telemetry.decks[d].playing && has(d)).or_else(|| (0..2).find(|&d| has(d))) else {
        p.text_in(r.x + 12.0, r.y + 44.0, r.w - 24.0, ARTIST, "No synced lyrics for the loaded tracks");
        return;
    };
    let Some(l) = &s.decks[d].loaded else { return };
    let cur = l.lyrics.current(s.decks[d].seconds(s.telemetry.decks[d].position));
    let line_h = 22.0;
    let rows = ((r.h - 36.0) / line_h).max(1.0) as usize;
    let first = cur.unwrap_or(0).saturating_sub(rows / 3);
    for (n, (_, text)) in l.lyrics.lines.iter().enumerate().skip(first).take(rows) {
        let y = r.y + 44.0 + (n - first) as f32 * line_h;
        let style = if Some(n) == cur { Style::new(14.0, Weight::Bold, DECK[d]) } else { Style { px: 12.5, ..ARTIST } };
        p.text_in(r.x + 12.0, y, r.w - 24.0, style, text);
    }
}

/// The track picker a LOAD button opens: the library over everything below the toolbar.
fn browser(p: &mut Painter, s: &mut State, w: f32, h: f32, top: f32) {
    let Some(d) = s.browser else { return };
    p.layer();
    let r = Rect::new(4.0, top + 4.0, w - 8.0, h - top - 8.0);
    hit(s, r, Target::CloseBrowser);
    section(p, r, INK);
    p.outlined(r.x, r.y, r.w, r.h, 0.0, 0, 1.5, DECK[d]);
    let head = Style::new(14.0, Weight::Bold, DECK[d]);
    let x = r.x + 24.0 + p.text(r.x + 12.0, r.y + 25.0, head, ["Load to deck 1", "Load to deck 2"][d]);
    let (q, st) = if s.query.is_empty() {
        ("Type to search", Style { colour: DARK, ..TITLE })
    } else {
        (s.query.as_str(), TITLE)
    };
    let q = q.to_string();
    let tw = p.text_in(x, r.y + 25.0, r.w - x - 90.0, st, &q);
    if !s.query.is_empty() {
        p.rect(x + tw + 2.0, r.y + 12.0, 1.5, 17.0, 0.0, ACCENT);
    }
    button(p, s, Rect::new(r.x + r.w - 78.0, r.y + 7.0, 70.0, 26.0), "CLOSE", false, ACCENT, Target::CloseBrowser);
    library(p, s, Rect::new(r.x + 1.0, r.y + 40.0, r.w - 2.0, r.h - 41.0));
}

fn transport(p: &mut Painter, s: &mut State, r: Rect, d: usize) {
    let playing = s.telemetry.decks[d].playing;
    let bh = ((r.h - 4.0) * 0.5).min(r.w * 0.9);
    let play = Rect::new(r.x, r.y + r.h - 2.0 * bh - 4.0, r.w, bh);
    let down = s.pressed(Target::Play(d));
    let (fill, edge) = match (playing, down) {
        (true, _) => (fade(GREEN, 0x30), GREEN),
        (false, true) => (rgb(0x2c2c33), rgb(0x3a3a42)),
        (false, false) => (RAISED, EDGE),
    };
    p.outlined(play.x, play.y, play.w, play.h, 5.0, fill, 1.0, edge);
    let ink = if playing { GREEN } else { TEXT };
    let (cx, cy, k) = (play.x + play.w * 0.5, play.y + play.h * 0.5, (bh * 0.2).min(10.0));
    if playing {
        p.rect(cx - k * 0.8, cy - k, k * 0.55, k * 2.0, 1.0, ink);
        p.rect(cx + k * 0.25, cy - k, k * 0.55, k * 2.0, 1.0, ink);
    } else {
        for i in 0..(k * 2.0) as i32 {
            let fy = i as f32;
            let len = (k * 1.7) * (1.0 - (fy - k).abs() / k);
            p.rect(cx - k * 0.6, cy - k + fy, len, 1.0, 0.0, ink);
        }
    }
    hit(s, play, Target::Play(d));
    let cue = Rect::new(r.x, play.y + bh + 4.0, r.w, bh);
    let held = s.pressed(Target::Cue(d));
    let (fill, edge, ink) = if held { (fade(AMBER, 0x40), AMBER, AMBER) } else { (RAISED, EDGE, TEXT) };
    p.outlined(cue.x, cue.y, cue.w, cue.h, 5.0, fill, 1.0, edge);
    let cs = Style { colour: ink, ..TAB };
    p.text(cue.x + (cue.w - p.text_width(cs, "CUE")) * 0.5, cue.y + cue.h * 0.5 + 4.0, cs, "CUE");
    hit(s, cue, Target::Cue(d));
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

fn knob(p: &mut Painter, s: &mut State, r: Rect, v: f32, colour: u32, label: &str, target: Target) {
    let k = r.w.min(r.h - 10.0);
    let (cx, cy) = (r.x + r.w * 0.5, r.y + k * 0.5);
    let rad = k * 0.5;
    let thick = (k * 0.09).max(2.0);
    p.arc(cx, cy, rad, thick, 0.625, 0.75, rgb(0x0c0c0c));
    let (a, b) = if v >= 0.5 { (0.5, v) } else { (v, 0.5) };
    if (v - 0.5).abs() >= 0.01 {
        p.arc(cx, cy, rad, thick, 0.625 + a * 0.75, (b - a) * 0.75, colour);
    }
    let body = rad - thick - 2.0;
    skin_knob(p, cx, cy, body);
    let ang = (0.625 + v * 0.75) * std::f32::consts::TAU;
    let (sn, cs) = ang.sin_cos();
    p.line((cx + sn * body * 0.25, cy - cs * body * 0.25), (cx + sn * body * 0.85, cy - cs * body * 0.85), 2.0, TEXT);
    let lw = p.text_width(SMALL, label);
    p.text(r.x + (r.w - lw) * 0.5, r.y + k + 9.0, SMALL, label);
    hit(s, r, target);
}
