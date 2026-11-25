use sugar_layout::Rect;
use sugar_ui::{Painter, Style, Weight, rgb};

use crate::state::{DETAIL_FRAMES, GridOp, Hit, State, Target, View};
use sugar_engine::station::Scene;

use crate::theme::{self, Line, Mark};

const INK: u32 = rgb(0x000000);
/// Behind the library and its side lists.
const BROWSER: u32 = rgb(0x101112);
const SURFACE: u32 = rgb(0x161618);
const DEEP: u32 = rgb(0x0c0c0c);
const EDGE: u32 = rgb(0x28282b);
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
pub const DECK: [u32; 2] = [rgb(0x3a8dff), rgb(0xf0443a)];

const LOGO: Style = Style::new(16.0, Weight::Bold, rgb(0xe63380));
const TAB: Style = Style::new(11.0, Weight::Bold, TEXT);
const TITLE: Style = Style::new(14.0, Weight::Semibold, TEXT);
const ARTIST: Style = Style::new(10.0, Weight::Regular, GREY);
const SMALL: Style = Style::new(9.0, Weight::Semibold, GREY);
const TIME: Style = Style::new(14.0, Weight::Bold, TEXT);
const PAD_TEXT: Style = Style::new(10.0, Weight::Bold, TEXT);

const TOOLBAR: f32 = 36.0;
const ROW: f32 = 44.0;
/// The effects bar, tall enough for its knobs.
const FX_ROW: f32 = 68.0;

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
    let wide = w >= 1100.0;
    let bar = if wide {
        TOP_BAR
    } else if w < 720.0 || (tall && w < 1100.0) {
        TOOLBAR * 3.0
    } else {
        TOOLBAR
    };
    if wide {
        top_bar(p, s, Rect::new(0.0, 0.0, w, bar));
        footer(p, s, Rect::new(0.0, h - FOOTER, w, FOOTER));
        let r = Rect::new(0.0, bar, w, h - bar - FOOTER);
        if s.view == View::Dj { perform(p, s, r) } else { views(p, s, r, tall) }
    } else {
        toolbar(p, s, Rect::new(0.0, 0.0, w, bar));
        views(p, s, Rect::new(0.0, bar, w, h - bar), tall);
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
    toast(p, s, w, h);
    track_menu(p, s, w, h);
    list_menu(p, s, w, h);
    column_menu(p, s, w, h);
    chip_menu(p, s, w, h);
    tag_menu(p, s, w, h);
    fx_menu(p, s, w, h);
    crate::prefs::app_menu(p, s, w, h);
    crate::prefs::overlay(p, s, w, h);
}

/// The views as the narrow toolbar picks them, in the area under the top bar.
fn views(p: &mut Painter, s: &mut State, area: Rect, tall: bool) {
    let (w, h) = (area.w, area.y + area.h);
    let mut y = area.y;
    if s.show_fx {
        effects(p, s, Rect::new(0.0, y, w, FX_ROW));
        y += FX_ROW;
    }
    if s.show_samplers {
        samplers(p, s, Rect::new(0.0, y, w, ROW));
        y += ROW;
    }
    let rest = h - y;
    match s.view {
        View::Dj => {
            // The Classic phone layout: waveforms, decks around the mixer, the library below.
            let tall_by = 1.0 + 0.2 * f32::from(s.wave_height);
            let wh = (rest * 0.22 * tall_by).round().clamp(40.0, (160.0 * tall_by).max(56.0)).min(rest * 0.5);
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
}

/// The wide Perform mode: the waveforms, the decks around the mixer, the effects and
/// samplers bars, then the library with the side panel on its right.
fn perform(p: &mut Painter, s: &mut State, r: Rect) {
    const STEPS: [f32; 10] = [99.0, 123.0, 144.0, 167.0, 189.0, 209.0, 229.0, 249.0, 271.0, 291.0];
    let mut y = r.y;
    let bottom = r.y + r.h;
    if !s.lib_expanded {
        if s.show_waves {
            let wh = STEPS[(s.wave_height + 1).clamp(0, 9) as usize].min(r.h * 0.25).round();
            waveforms(p, s, Rect::new(r.x, y, r.w, wh));
            y += wh;
            p.rect(r.x, y, r.w, 1.0, 0.0, theme::SEPARATOR);
            y += 1.0;
        }
        let dh = crate::sugardeck::HEIGHT.min((bottom - y) * 0.6).max(190.0).min(bottom - y);
        crate::sugardeck::row(p, s, Rect::new(r.x, y, r.w, dh));
        y += dh;
        if s.show_fx && bottom - y > FX_ROW + 60.0 {
            p.rect(r.x, y, r.w, 1.0, 0.0, theme::SEPARATOR);
            effects(p, s, Rect::new(r.x, y + 1.0, r.w, FX_ROW));
            y += FX_ROW + 1.0;
        }
        if s.show_samplers && bottom - y > ROW + 60.0 {
            p.rect(r.x, y, r.w, 1.0, 0.0, theme::SEPARATOR);
            samplers(p, s, Rect::new(r.x, y + 1.0, r.w, ROW));
            y += ROW + 1.0;
        }
    }
    if bottom - y < 60.0 {
        return;
    }
    p.rect(r.x, y, r.w, 1.0, 0.0, theme::SEPARATOR);
    let lh = bottom - y - 1.0;
    let side = if s.side_panel { (r.w * 0.24).clamp(260.0, 360.0) } else { 0.0 };
    library(p, s, Rect::new(r.x, y + 1.0, r.w - side, lh));
    if side > 0.0 {
        p.rect(r.x + r.w - side, y + 1.0, 1.0, lh, 0.0, theme::SEPARATOR);
        side_panel(p, s, Rect::new(r.x + r.w - side + 1.0, y + 1.0, side - 1.0, lh));
    }
}

/// The side panel's tabs, in order.
pub const SIDE_TABS: [&str; 3] = ["Info", "Cues", "Lyrics"];

/// The side panel right of the library: the selected track's details, the hot cues
/// of the deck playing, or the lyrics.
fn side_panel(p: &mut Painter, s: &mut State, r: Rect) {
    p.rect(r.x, r.y, r.w, r.h, 0.0, INK);
    let th = 36.0;
    let tw = (r.w - 16.0) / SIDE_TABS.len() as f32;
    p.outlined(r.x + 8.0, r.y + 6.0, r.w - 16.0, th - 8.0, 8.0, theme::WELL, 1.0, theme::BORDER);
    for (n, label) in SIDE_TABS.iter().enumerate() {
        let b = Rect::new(r.x + 8.0 + n as f32 * tw, r.y + 6.0, tw, th - 8.0);
        let on = usize::from(s.side_tab) == n;
        if on {
            p.outlined(b.x + 2.0, b.y + 2.0, b.w - 4.0, b.h - 4.0, 6.0, theme::RAISED, 1.0, theme::BORDER);
        }
        let ink = Style { colour: if on { TEXT } else { theme::DIM }, ..TAB };
        p.text(b.x + (b.w - p.text_width(ink, label)) * 0.5, b.y + b.h * 0.5 + 4.0, ink, label);
        hit(s, b, Target::SideTab(n as u8));
    }
    let body = Rect::new(r.x, r.y + th + 4.0, r.w, r.h - th - 4.0);
    match s.side_tab {
        0 => side_info(p, s, body),
        1 => side_cues(p, s, body),
        _ => lyrics(p, s, body),
    }
}

fn side_info(p: &mut Painter, s: &mut State, r: Rect) {
    let Some(i) = s.selected.map(|i| i as usize).filter(|&i| i < s.entries.len()) else {
        p.text_in(r.x + 12.0, r.y + 20.0, r.w - 24.0, ARTIST, "Select a track to see its details");
        return;
    };
    let mut y = r.y + 8.0;
    for line in info_lines(s, i) {
        if let Line::Item { text, hint, .. } = line {
            if y + 34.0 > r.y + r.h {
                break;
            }
            p.text(r.x + 12.0, y + 11.0, Style { colour: theme::FAINT, ..SMALL }, &hint);
            p.text_in(r.x + 12.0, y + 27.0, r.w - 24.0, Style { px: 13.0, ..TITLE }, &text);
            y += 36.0;
        }
    }
}

fn side_cues(p: &mut Painter, s: &mut State, r: Rect) {
    let d = (0..2).find(|&d| s.telemetry.decks[d].playing).unwrap_or(0);
    let Some(l) = s.decks[d].loaded.as_ref() else {
        p.text_in(r.x + 12.0, r.y + 20.0, r.w - 24.0, ARTIST, "Load a track to see its cues");
        return;
    };
    p.text_in(
        r.x + 12.0,
        r.y + 14.0,
        r.w - 24.0,
        Style { colour: DECK[d], ..SMALL },
        &format!("DECK {}  {}", ["A", "B"][d], l.entry.title),
    );
    let mut y = r.y + 26.0;
    for n in 0..8 {
        let b = Rect::new(r.x + 8.0, y, r.w - 16.0, 34.0);
        let at = s.decks[d].cues[n];
        let colour = cue_colour(s, d, n);
        p.outlined(b.x, b.y, b.w, b.h, 6.0, SURFACE, 1.0, EDGE);
        p.rect(b.x + 4.0, b.y + 5.0, 4.0, b.h - 10.0, 2.0, if at.is_some() { colour } else { theme::FAINT });
        p.text(b.x + 16.0, b.y + 21.0, TAB, &(n + 1).to_string());
        let when = at.map_or_else(
            || "Empty".to_string(),
            |f| {
                let t = s.decks[d].seconds(f);
                format!("{}:{:05.2}", (t / 60.0) as u32, t % 60.0)
            },
        );
        let ink = if at.is_some() { TEXT } else { theme::FAINT };
        p.text(b.x + 40.0, b.y + 21.0, Style { colour: ink, ..ARTIST }, &when);
        hit(s, b, Target::SideCue(d, n));
        y += 38.0;
        if y + 34.0 > r.y + r.h {
            break;
        }
    }
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
    // The app menu button: three lines.
    let mb = Rect::new(r.x + 6.0, r.y + 6.0, TOOLBAR - 12.0, TOOLBAR - 12.0);
    if s.app_menu || s.pressed(Target::AppMenu) {
        p.rect(mb.x, mb.y, mb.w, mb.h, 6.0, theme::PRESSED);
    }
    for i in 0..3 {
        p.rect(mb.x + mb.w * 0.25, mb.y + mb.h * 0.5 - 6.0 + i as f32 * 5.5, mb.w * 0.5, 1.6, 0.8, TEXT);
    }
    hit(s, mb, Target::AppMenu);
    let mut x = logo(p, mb.x + mb.w + 6.0, r.y + TOOLBAR * 0.5) + 12.0;
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
    s.search_in_bar = false;
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
    search_box(p, s, Rect::new(sx, r.y + 5.0, r.w - sx - 8.0, h - 2.0));
}

/// The search field, with its magnifier, the query and its count of matches.
fn search_box(p: &mut Painter, s: &mut State, r: Rect) {
    let focus = s.search_focus || !s.query.is_empty();
    let edge = if focus { ACCENT } else { theme::BORDER };
    p.outlined(r.x, r.y, r.w, r.h, 6.0, theme::WELL, 1.0, edge);
    let (gx, gy) = (r.x + 15.0, r.y + r.h * 0.5 - 1.0);
    let ink = if focus { ACCENT } else { theme::DIM };
    p.outlined(gx - 4.5, gy - 4.5, 9.0, 9.0, 4.5, 0, 1.5, ink);
    p.line((gx + 3.5, gy + 3.5), (gx + 6.5, gy + 6.5), 1.5, ink);
    hit(s, r, Target::SearchFocus);
    let (tx, base) = (r.x + 28.0, r.y + r.h * 0.5 + 4.0);
    if s.query.is_empty() {
        let hint = s.status.as_deref().unwrap_or("Search tracks");
        p.text_in(tx, base, r.w - 36.0, Style { colour: theme::FAINT, ..ARTIST }, hint);
    } else {
        let qw = p.text_in(tx, base, r.w - 68.0, Style { px: 12.0, ..TITLE }, &s.query);
        p.rect(tx + 1.0 + qw, base - 11.0, 1.5, 15.0, 0.0, ACCENT);
        let n = s.shown.len().to_string();
        p.text(r.x + r.w - 10.0 - p.text_width(SMALL, &n), base, SMALL, &n);
    }
}

/// A rounded top bar button, outlined in its colour while on.
fn pill(p: &mut Painter, s: &mut State, r: Rect, label: &str, on: bool, colour: u32, target: Target) {
    let fill = if s.pressed(target) {
        theme::PRESSED
    } else if on {
        fade(colour, 0x24)
    } else {
        theme::RAISED
    };
    p.outlined(r.x, r.y, r.w, r.h, 6.0, fill, 1.0, if on { colour } else { theme::RAISED });
    let ink = Style { colour: if on { colour } else { theme::DIM }, ..TAB };
    p.text(r.x + (r.w - p.text_width(ink, label)) * 0.5, r.y + r.h * 0.5 + 4.0, ink, label);
    hit(s, r, target);
}

/// Height of the top bar on a wide window.
pub const TOP_BAR: f32 = 52.0;
/// Height of the footer on a wide window.
pub const FOOTER: f32 = 36.0;

/// The top bar on a wide window: logo, menu and the three modes on the left, the
/// search in the middle, and the panel switches, recorder and master level on the right.
fn top_bar(p: &mut Painter, s: &mut State, r: Rect) {
    p.rect(r.x, r.y, r.w, r.h, 0.0, INK);
    p.rect(r.x, r.y + r.h - 1.0, r.w, 1.0, 0.0, theme::SEPARATOR);
    let cy = r.y + r.h * 0.5;
    let mut x = logo(p, r.x + 14.0, cy) + 14.0;
    let mb = Rect::new(x, r.y + 9.0, 34.0, 34.0);
    let fill = if s.app_menu || s.pressed(Target::AppMenu) { theme::PRESSED } else { theme::RAISED };
    p.rect(mb.x, mb.y, mb.w, mb.h, 6.0, fill);
    for i in 0..3 {
        p.rect(mb.x + 10.0, cy - 6.0 + i as f32 * 5.5, 14.0, 1.6, 0.8, TEXT);
    }
    hit(s, mb, Target::AppMenu);
    x += mb.w + 10.0;
    // The modes: performing on the decks, preparing tracks, or just listening through the library.
    let modes = [("Perform", View::Dj), ("Prepare", View::Ctl), ("Listen", View::Lib)];
    let tw = 78.0;
    p.outlined(x, r.y + 10.0, tw * 3.0, 32.0, 16.0, theme::CONTROL, 1.0, theme::BORDER);
    for (n, (label, v)) in modes.into_iter().enumerate() {
        let b = Rect::new(x + n as f32 * tw, r.y + 10.0, tw, 32.0);
        if s.view == v {
            p.rect(b.x + 3.0, b.y + 3.0, b.w - 6.0, b.h - 6.0, 13.0, ACCENT);
        } else if s.pressed(Target::View(v)) {
            p.rect(b.x + 3.0, b.y + 3.0, b.w - 6.0, b.h - 6.0, 13.0, theme::PRESSED);
        }
        let ink = Style { colour: if s.view == v { rgb(0x151329) } else { theme::DIM }, ..TAB };
        p.text(b.x + (b.w - p.text_width(ink, label)) * 0.5, cy + 4.0, ink, label);
        hit(s, b, Target::View(v));
    }
    let left = x + tw * 3.0 + 16.0;
    // From the right: settings, the master level, then the switches while they fit.
    let g = Rect::new(r.x + r.w - 42.0, r.y + 9.0, 34.0, 34.0);
    let fill = if s.pressed(Target::Prefs(0)) { theme::PRESSED } else { theme::RAISED };
    p.rect(g.x, g.y, g.w, g.h, 6.0, fill);
    let (gx, gy) = (g.x + g.w * 0.5, cy);
    for k in 0..6 {
        let (sn, cs) = (k as f32 * std::f32::consts::FRAC_PI_3).sin_cos();
        p.line((gx + cs * 5.0, gy + sn * 5.0), (gx + cs * 8.0, gy + sn * 8.0), 2.4, theme::DIM);
    }
    p.outlined(gx - 5.5, gy - 5.5, 11.0, 11.0, 5.5, fill, 1.8, theme::DIM);
    hit(s, g, Target::Prefs(0));
    let search_min = 220.0;
    let mut right = g.x - 8.0;
    let mw = 190.0;
    if right - mw > left + search_min {
        right -= mw;
        let m = Rect::new(right, r.y + 9.0, mw, 34.0);
        let fill = if s.pressed(Target::Prefs(2)) { theme::PRESSED } else { theme::RAISED };
        p.rect(m.x, m.y, m.w, m.h, 6.0, fill);
        p.text(m.x + 12.0, cy + 4.0, Style { colour: theme::DIM, ..SMALL }, "MASTER");
        let vx = m.x + 60.0;
        let vw = m.w - 72.0;
        for (c, &v) in s.telemetry.main_peak.iter().enumerate() {
            let y = cy - 5.0 + c as f32 * 6.0;
            p.rect(vx, y, vw, 4.0, 2.0, theme::WELL);
            let v = v.clamp(0.0, 1.0);
            let colour = if v > 0.95 {
                RED
            } else if v > 0.8 {
                AMBER
            } else {
                GREEN
            };
            p.rect(vx, y, vw * v, 4.0, 2.0, colour);
        }
        hit(s, m, Target::Prefs(2));
        right -= 8.0;
    }
    let rec = s.recorder.as_ref().map(|r| {
        let t = r.frames / u64::from(s.out_rate.max(1));
        format!("REC {}:{:02}", t / 60, t % 60)
    });
    let items: [(&str, bool, u32, Target); 5] = [
        (rec.as_deref().unwrap_or("REC"), s.recorder.is_some(), RED, Target::Record),
        ("Automix", s.auto.is_some(), ACCENT, Target::AutoDj),
        ("Samplers", s.show_samplers, ACCENT, Target::ToggleSamplers),
        ("FX", s.show_fx, theme::ACCENT, Target::ToggleFx),
        ("Waveforms", s.show_waves, ACCENT, Target::ToggleWaves),
    ];
    for (label, on, colour, t) in items {
        let bw = p.text_width(TAB, label) + 28.0;
        if right - bw < left + search_min {
            break;
        }
        right -= bw;
        pill(p, s, Rect::new(right, r.y + 9.0, bw, 34.0), label, on, colour, t);
        right -= 8.0;
    }
    // The search, centred on the window while there is room, else in the gap.
    let sw = (right - left - 8.0).min(560.0);
    let sx = ((r.w - sw) * 0.5).clamp(left, right - sw);
    s.search_in_bar = true;
    search_box(p, s, Rect::new(sx, r.y + 9.0, sw, 34.0));
}

/// The footer on a wide window: expand the library, what the app is busy with, and the side panel.
fn footer(p: &mut Painter, s: &mut State, r: Rect) {
    p.rect(r.x, r.y, r.w, r.h, 0.0, theme::PANEL);
    p.rect(r.x, r.y, r.w, 1.0, 0.0, theme::SEPARATOR);
    let b = Rect::new(r.x + 8.0, r.y + 5.0, 150.0, r.h - 10.0);
    let label = if s.lib_expanded { "Collapse Browser" } else { "Expand Browser" };
    pill(p, s, b, label, s.lib_expanded, ACCENT, Target::ToggleExpand);
    let busy = s.status.clone().unwrap_or_else(|| tracks(s.shown.len()));
    p.text_in(b.x + b.w + 16.0, r.y + r.h * 0.5 + 4.0, r.w - 360.0, Style { colour: theme::DIM, ..ARTIST }, &busy);
    let b = Rect::new(r.x + r.w - 158.0, r.y + 5.0, 150.0, r.h - 10.0);
    pill(p, s, b, "Side Panel", s.side_panel, ACCENT, Target::ToggleSide);
}

/// Each deck's effect unit: three slots in series, each its effect's name (tap for the list),
/// an on button and its knob, then the unit's dry/wet mix.
fn effects(p: &mut Painter, s: &mut State, r: Rect) {
    p.rect(r.x, r.y, r.w, r.h, 0.0, theme::PANEL);
    let half = r.w * 0.5;
    let top = (r.h * 0.46).round();
    let kinds = djsugar_engine::AVAILABLE_EFFECTS.len() as u8 + 1;
    let name_ink = Style::new(12.0, Weight::Regular, theme::DIM);
    for d in 0..2 {
        let x0 = r.x + d as f32 * half;
        let mix_w = 84.0_f32.min(half * 0.2);
        let cell = ((half - mix_w) / 3.0).floor();
        let (slots, mix) = s.fx[d];
        for (i, &(kind, on, meta)) in slots.iter().enumerate() {
            let c = Rect::new(x0 + i as f32 * cell, r.y, cell, r.h);
            p.rect(c.x + c.w - 1.0, c.y, 1.0, c.h, 0.0, theme::SEPARATOR);
            // The effect before and the one after, either side of the name and its list.
            let step = 26.0;
            for (side, by) in [(0.0, kinds - 1), (1.0, 1)] {
                let b = Rect::new(c.x + 4.0 + side * (c.w - step - 9.0), c.y + 2.0, step, top - 2.0);
                let t = Target::FxPick(d, i, (kind + by) % kinds);
                if s.pressed(t) {
                    p.rect(b.x, b.y + 2.0, b.w, b.h - 4.0, 5.0, theme::PRESSED);
                }
                let (ax, ay, k) = (b.x + b.w * 0.5, b.y + b.h * 0.5, if side == 0.0 { 1.0 } else { -1.0 });
                p.line((ax + 2.0 * k, ay - 4.0), (ax - 2.0 * k, ay), 1.4, theme::DIM);
                p.line((ax - 2.0 * k, ay), (ax + 2.0 * k, ay + 4.0), 1.4, theme::DIM);
                hit(s, b, t);
            }
            let name = djsugar_engine::effect_name(f64::from(kind));
            let name = if name.is_empty() { "No effect" } else { name };
            let nb = Rect::new(c.x + 4.0 + step, c.y + 2.0, c.w - 2.0 * step - 9.0, top - 2.0);
            if s.pressed(Target::FxName(d, i)) || s.fx_menu == Some((d, i)) {
                p.rect(nb.x, nb.y + 2.0, nb.w, nb.h - 4.0, 5.0, theme::PRESSED);
            }
            let ink = Style { colour: if kind > 0 { TEXT } else { theme::DIM }, ..name_ink };
            let tw = p.text_width(ink, name).min(nb.w - 18.0);
            let tx = nb.x + (nb.w - tw - 12.0) * 0.5;
            p.text_in(tx, nb.y + nb.h * 0.5 + 4.0, nb.w - 18.0, ink, name);
            let (ax, ay) = (tx + tw + 8.0, nb.y + nb.h * 0.5);
            p.line((ax - 3.0, ay - 1.5), (ax, ay + 1.5), 1.3, theme::DIM);
            p.line((ax, ay + 1.5), (ax + 3.0, ay - 1.5), 1.3, theme::DIM);
            hit(s, nb, Target::FxName(d, i));
            let low = Rect::new(c.x, c.y + top, c.w - 1.0, c.h - top);
            let ob = Rect::new(low.x + 8.0, low.y + (low.h - 22.0) * 0.5 - 1.0, 36.0, 22.0);
            let lit = on && kind > 0;
            let fill = if lit {
                DECK[d]
            } else if s.pressed(Target::FxOn(d, i)) {
                theme::PRESSED
            } else {
                theme::PANEL
            };
            p.outlined(ob.x, ob.y, ob.w, ob.h, 5.0, fill, 1.0, if lit { DECK[d] } else { theme::BORDER });
            let ink = Style { colour: if lit { rgb(0x151329) } else { theme::DIM }, ..TAB };
            p.text(ob.x + (ob.w - p.text_width(ink, "ON")) * 0.5, ob.y + 15.0, ink, "ON");
            hit(s, ob, Target::FxOn(d, i));
            let kw = (low.h - 8.0).min(30.0);
            let kb = Rect::new(low.x + low.w - kw - 26.0, low.y + (low.h - kw) * 0.5 + 1.0, kw, kw + 10.0);
            let letter = Style { colour: theme::DIM, ..SMALL };
            p.text(kb.x - 12.0, kb.y + kw * 0.5 + 3.0, letter, "D");
            p.text(kb.x + kw + 5.0, kb.y + kw * 0.5 + 3.0, letter, "W");
            knob(p, s, kb, meta, DECK[d], "", Target::FxMeta(d, i));
        }
        let m = Rect::new(x0 + 3.0 * cell, r.y, half - 3.0 * cell, r.h);
        let label = Style { colour: DECK[d], ..SMALL };
        let text = ["FX 1 MIX", "FX 2 MIX"][d];
        p.text(m.x + (m.w - p.text_width(label, text)) * 0.5, m.y + top * 0.5 + 4.0, label, text);
        let kw = (m.h - top - 8.0).min(30.0);
        knob(
            p,
            s,
            Rect::new(m.x + (m.w - kw) * 0.5, m.y + top + (m.h - top - kw) * 0.5 + 1.0, kw, kw + 10.0),
            mix,
            DECK[d],
            "",
            Target::FxMix(d),
        );
    }
    p.rect(r.x, r.y + top, r.w, 1.0, 0.0, theme::SEPARATOR);
    p.rect(r.x + half - 1.0, r.y, 1.0, r.h, 0.0, theme::BORDER);
}

/// The effects a slot can load, the loaded one ticked.
fn fx_menu(p: &mut Painter, s: &mut State, w: f32, h: f32) {
    let Some((d, slot)) = s.fx_menu else { return };
    p.layer();
    let screen = Rect::new(0.0, 0.0, w, h);
    hit(s, screen, Target::MenuClose);
    let now = s.fx[d].0[slot].0;
    let mut lines = vec![Line::item("Empty", Target::FxPick(d, slot, 0)).mark(Mark::Check(now == 0)), Line::Rule];
    lines.extend(
        djsugar_engine::AVAILABLE_EFFECTS
            .iter()
            .zip(1u8..)
            .map(|(name, k)| Line::item(*name, Target::FxPick(d, slot, k)).mark(Mark::Check(now == k))),
    );
    theme::menu(p, s, s.menu_at, screen, &lines);
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
    p.rect(r.x, r.y, r.w, r.h, 0.0, rgb(0x101012));
    let deck = &s.decks[d];
    let pos = s.telemetry.decks[d].position;
    let mid = r.y + r.h * 0.5;
    let speed = 1.0 + deck.rate * s.settings.rate_range() as f32;
    let step = speed * 2f32.powi(-i32::from(s.wave_zoom));
    let per_px = DETAIL_FRAMES as f64 * 0.5 * f64::from(step);
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
                    p.rect(r.x + x, r.y, 1.0, r.h, 0.0, if bar { rgb(0x34343a) } else { rgb(0x1e1e22) });
                    if bar && x > 0.0 {
                        let label = ((n / 4.0) as i64 + 1).to_string();
                        p.text(r.x + x + 4.0, r.y + 11.0, Style { colour: rgb(0xb4b4bc), ..SMALL }, &label);
                    }
                }
                n += 1.0;
            }
        }
        let warn = f64::from(s.settings.end_warning());
        let left_s = (l.track.frames.len() as f64 - pos) / f64::from(l.track.rate) / f64::from(speed.max(0.01));
        if warn > 0.0 && s.telemetry.decks[d].playing && left_s < warn && ((left_s * 2.0) as u64).is_multiple_of(2) {
            p.rect(r.x, r.y, r.w, r.h, 0.0, fade(rgb(0xff2d2d), 0x30));
            p.rect(r.x, r.y, r.w, 2.0, 0.0, rgb(0xff2d2d));
            p.rect(r.x, r.y + r.h - 2.0, r.w, 2.0, 0.0, rgb(0xff2d2d));
        }
        let cols = r.w as usize;
        let centre = pos / DETAIL_FRAMES as f64;
        let half = r.h * 0.47;
        for c in 0..cols {
            let i = centre + f64::from((c as f32 - cols as f32 * 0.5) * step);
            let Some(&[v, lo, md, hi]) = (i >= 0.0).then(|| l.detail.get(i as usize)).flatten() else { continue };
            let past = (c as f32) < cols as f32 * 0.5;
            let a = if past { 0x90 } else { 0xff };
            let x = r.x + c as f32;
            let amp = f32::from(v) / 255.0 * half;
            let total = (f32::from(lo) + f32::from(md) + f32::from(hi)).max(1.0);
            let lo_h = amp * (f32::from(lo) / total).sqrt().min(1.0);
            let md_h = amp * (f32::from(md) / total).sqrt().min(1.0) * 0.8;
            let hi_h = amp * (f32::from(hi) / total).sqrt().min(1.0) * 0.6;
            // Warm layers: the whole level in deep orange, the bass in orange, the mids in amber, the highs pale gold.
            p.rect(x, mid - amp, 1.0, amp * 2.0, 0.0, fade(rgb(0xa8380e), a));
            p.rect(x, mid - lo_h, 1.0, lo_h * 2.0, 0.0, fade(rgb(0xe8621a), a));
            p.rect(x, mid - md_h, 1.0, md_h * 2.0, 0.0, fade(rgb(0xf5a02a), a));
            p.rect(x, mid - hi_h, 1.0, hi_h * 2.0, 0.0, fade(rgb(0xffd98a), a));
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
    p.rect(r.x, r.y + r.h, r.w, 1.0, 0.0, rgb(0x2a2a30));
    p.rect(r.x + r.w * 0.5 - 1.0, r.y, 2.0, r.h, 1.0, TEXT);
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

/// The library's rail: the collection, playlists, streaming services and folders, each under its heading.
/// The sidebar tree: collection, playlists, streaming services and folders, each a
/// section that folds away under its heading.
fn sidebar(p: &mut Painter, s: &mut State, r: Rect) {
    use crate::state::Shelf;
    p.rect(r.x, r.y, r.w, r.h, 0.0, BROWSER);
    p.rect(r.x + r.w - 1.0, r.y, 1.0, r.h, 0.0, EDGE);
    let rh = 26.0;
    let bottom = r.y + r.h;
    let mut y = r.y + 4.0;
    let count = Style { colour: theme::FAINT, ..SMALL };
    let row = |p: &mut Painter,
               s: &mut State,
               y: &mut f32,
               (label, n): (&str, Option<usize>),
               t: Target,
               on: bool,
               tint: Option<u32>| {
        if *y + rh > bottom {
            return;
        }
        let b = Rect::new(r.x + 6.0, *y, r.w - 12.0, rh - 2.0);
        if on {
            p.outlined(b.x, b.y, b.w, b.h, b.h * 0.5, fade(ACCENT, 0x30), 1.0, fade(ACCENT, 0xb0));
        } else if s.pressed(t) {
            p.rect(b.x, b.y, b.w, b.h, b.h * 0.5, fade(ACCENT, 0x18));
        }
        let mut tx = b.x + 22.0;
        if let Some(c) = tint {
            p.rect(tx, b.y + b.h * 0.5 - 4.0, 8.0, 8.0, 4.0, c);
            tx += 14.0;
        }
        let mut right = b.x + b.w - 6.0;
        if let Some(n) = n {
            let n = n.to_string();
            let count = Style { colour: if on { TEXT } else { count.colour }, ..count };
            right -= p.text_width(count, &n);
            p.text(right, b.y + b.h * 0.5 + 4.0, count, &n);
            right -= 6.0;
        }
        let ink = Style { colour: if on { TEXT } else { GREY }, ..ARTIST };
        p.text_in(tx, b.y + b.h * 0.5 + 4.0, right - tx, ink, label);
        hit(s, b, t);
        *y += rh;
    };
    // A heading with its fold arrow; returns whether the section is open.
    let head = |p: &mut Painter, s: &mut State, y: &mut f32, name: &str, bit: u8| -> bool {
        if *y + 18.0 + rh > bottom {
            return false;
        }
        let open = s.folded & (1 << bit) == 0;
        let b = Rect::new(r.x + 6.0, *y, r.w - 12.0, 20.0);
        let (ax, ay) = (b.x + 8.0, b.y + 10.0);
        if open {
            p.line((ax - 3.5, ay - 1.5), (ax, ay + 2.0), 1.4, GREY);
            p.line((ax, ay + 2.0), (ax + 3.5, ay - 1.5), 1.4, GREY);
        } else {
            p.line((ax - 1.5, ay - 3.5), (ax + 2.0, ay), 1.4, GREY);
            p.line((ax + 2.0, ay), (ax - 1.5, ay + 3.5), 1.4, GREY);
        }
        p.text(b.x + 18.0, b.y + 14.0, SMALL, name);
        hit(s, b, Target::Fold(bit));
        *y += 22.0;
        open
    };
    let lib = s.view != View::Stream;
    if head(p, s, &mut y, "COLLECTION", 0) {
        let all = s.entries.len().saturating_sub(crate::state::SAMPLES.len());
        row(
            p,
            s,
            &mut y,
            ("All Tracks", Some(all)),
            Target::Shelf(Shelf::Tracks),
            lib && s.shelf == Shelf::Tracks,
            None,
        );
        let hist = s.history.len();
        row(
            p,
            s,
            &mut y,
            ("History", Some(hist)),
            Target::Shelf(Shelf::History),
            lib && s.shelf == Shelf::History,
            None,
        );
    }
    if head(p, s, &mut y, "PREPARATION", 4) {
        let counts = s.stage_counts();
        for (&(name, c), n) in crate::state::STAGES.iter().zip(1u8..) {
            let sh = Shelf::Stage(n);
            let count = counts[usize::from(n) - 1];
            row(p, s, &mut y, (name, Some(count)), Target::Shelf(sh), lib && s.shelf == sh, Some(rgb(c)));
        }
    }
    if head(p, s, &mut y, "PLAYLISTS", 1) {
        let lists: Vec<(String, usize)> = s.playlists.iter().map(|(n, k)| (n.clone(), k.len())).collect();
        for (n, (name, len)) in lists.iter().enumerate() {
            if s.naming == Some(Shelf::Playlist(n)) && y + rh <= bottom {
                name_field(p, s, Rect::new(r.x + 6.0, y, r.w - 12.0, rh - 2.0));
                y += rh;
                continue;
            }
            let sh = Shelf::Playlist(n);
            row(p, s, &mut y, (name, Some(*len)), Target::Shelf(sh), lib && s.shelf == sh, None);
        }
        if y + rh <= bottom {
            let b = Rect::new(r.x + 6.0, y + 2.0, r.w - 12.0, rh - 4.0);
            button(p, s, b, "+ NEW PLAYLIST", false, ACCENT, Target::NewPlaylist);
            y += rh + 2.0;
        }
    }
    if head(p, s, &mut y, "STREAMING", 2) {
        for sv in crate::hub::Service::ALL {
            let on = !lib && s.hub.service == sv;
            row(p, s, &mut y, (sv.label(), None), Target::StreamFrom(sv), on, Some(rgb(sv.colour())));
        }
    }
    if !s.tags.is_empty() && head(p, s, &mut y, "TAGS", 5) {
        let tags: Vec<(String, usize)> = s.tags.iter().map(|(n, k)| (n.clone(), k.len())).collect();
        for (n, (name, len)) in tags.iter().enumerate() {
            if s.naming == Some(Shelf::Tag(n)) && y + rh <= bottom {
                name_field(p, s, Rect::new(r.x + 6.0, y, r.w - 12.0, rh - 2.0));
                y += rh;
                continue;
            }
            let sh = Shelf::Tag(n);
            row(p, s, &mut y, (name, Some(*len)), Target::Shelf(sh), lib && s.shelf == sh, None);
        }
    }
    let folders: Vec<String> = s.folders().into_iter().map(str::to_string).collect();
    if !folders.is_empty() && head(p, s, &mut y, "FOLDERS", 3) {
        for (n, f) in folders.iter().enumerate() {
            let sh = Shelf::Folder(n);
            row(p, s, &mut y, (f, None), Target::Shelf(sh), lib && s.shelf == sh, None);
        }
    }
}

/// The latest message, over every page near the bottom, fading out.
fn toast(p: &mut Painter, s: &mut State, w: f32, h: f32) {
    let Some((msg, at)) = &s.toast else { return };
    let age = at.elapsed();
    if age >= crate::state::TOAST {
        return;
    }
    let left = (crate::state::TOAST - age).as_secs_f32();
    let alpha = (left.min(0.5) * 2.0 * 255.0) as u32;
    let st = Style::new(13.0, Weight::Semibold, fade(TEXT, alpha));
    let tw = p.text_width(st, msg).min(w - 48.0);
    let r = Rect::new((w - tw) * 0.5 - 16.0, h - 64.0, tw + 32.0, 36.0);
    p.layer();
    p.outlined(r.x, r.y, r.w, r.h, 18.0, fade(theme::MENU, alpha), 1.0, fade(theme::MENU_BORDER, alpha));
    p.text_in(r.x + 16.0, r.y + 23.0, tw, st, msg);
}

/// A held playlist's menu: load its first track, move it, empty it, delete it.
/// The field a list's or tag's new name is typed in, with its caret.
fn name_field(p: &mut Painter, s: &mut State, b: Rect) {
    p.outlined(b.x, b.y, b.w, b.h, 5.0, rgb(0x141417), 1.0, ACCENT);
    let ink = Style { colour: TEXT, ..ARTIST };
    let tw = p.text_width(ink, &s.name_buf).min(b.w - 30.0);
    p.text_in(b.x + 22.0, b.y + b.h * 0.5 + 4.0, b.w - 30.0, ink, &s.name_buf);
    p.rect(b.x + 23.0 + tw, b.y + 5.0, 1.5, b.h - 10.0, 0.0, ACCENT);
    hit(s, b, Target::NameField);
}

/// A held tag's menu: rename or delete it.
fn tag_menu(p: &mut Painter, s: &mut State, w: f32, h: f32) {
    let Some(n) = s.tag_menu else { return };
    let Some((name, count)) = s.tags.get(n).map(|(n, k)| (n.clone(), k.len())) else { return };
    p.layer();
    let screen = Rect::new(0.0, 0.0, w, h);
    hit(s, screen, Target::MenuClose);
    let lines = [
        Line::item(name, Target::MenuBody).enabled(false).hint(tracks(count)),
        Line::Rule,
        Line::item("Rename", Target::RenameTag(n)),
        Line::item("Delete tag", Target::DeleteTag(n)),
    ];
    theme::menu(p, s, s.menu_at, screen, &lines);
}

/// A chip's editor: how near in tempo for Near BPM, how many stars for Rated.
fn chip_menu(p: &mut Painter, s: &mut State, w: f32, h: f32) {
    let Some(chip) = s.chip_menu else { return };
    p.layer();
    let screen = Rect::new(0.0, 0.0, w, h);
    hit(s, screen, Target::MenuClose);
    let lines: Vec<Line> = if chip == 0 {
        let mut v = vec![Line::item("Tempo within", Target::MenuBody).enabled(false), Line::Rule];
        v.extend(crate::state::NEAR_BPM.iter().enumerate().map(|(n, pc)| {
            Line::item(format!("{pc}%"), Target::ChipSet(0, n as u8)).mark(Mark::Check(usize::from(s.near_bpm) == n))
        }));
        v
    } else {
        let mut v = vec![Line::item("At least", Target::MenuBody).enabled(false), Line::Rule];
        v.extend((1..=5u8).map(|n| {
            let label = if n == 1 { "1 star".to_string() } else { format!("{n} stars") };
            Line::item(label, Target::ChipSet(2, n)).mark(Mark::Check(s.min_stars == n))
        }));
        v
    };
    theme::menu(p, s, s.menu_at, screen, &lines);
}

/// Which columns the track list shows; the title always stays.
fn column_menu(p: &mut Painter, s: &mut State, w: f32, h: f32) {
    use crate::library::Column as C;
    if !s.column_menu {
        return;
    }
    p.layer();
    let screen = Rect::new(0.0, 0.0, w, h);
    hit(s, screen, Target::MenuClose);
    let mut lines: Vec<Line> = [
        (C::Index, "Number"),
        (C::Artist, "Artist"),
        (C::Rating, "Rating"),
        (C::Bpm, "BPM"),
        (C::Key, "Key"),
        (C::Time, "Length"),
    ]
    .into_iter()
    .map(|(c, n)| Line::item(n, Target::ToggleColumn(c)).mark(Mark::Check(s.hidden_cols & (1 << c as u8) == 0)))
    .collect();
    lines.push(Line::Rule);
    lines.push(Line::item("Reset Columns", Target::ResetColumns).enabled(s.hidden_cols != 0));
    theme::menu(p, s, s.menu_at, screen, &lines);
}

fn list_menu(p: &mut Painter, s: &mut State, w: f32, h: f32) {
    use crate::state::ListOp as L;
    let Some(n) = s.list_menu else { return };
    let Some((name, count)) = s.playlists.get(n).map(|(n, k)| (n.clone(), k.len())) else { return };
    p.layer();
    let screen = Rect::new(0.0, 0.0, w, h);
    hit(s, screen, Target::MenuClose);
    let last = s.playlists.len() - 1;
    let lines = [
        Line::item(name, Target::MenuBody).enabled(false).hint(tracks(count)),
        Line::Rule,
        Line::item("Load first on deck 1", Target::ListOp(n, L::Load(0))).enabled(count > 0),
        Line::item("Load first on deck 2", Target::ListOp(n, L::Load(1))).enabled(count > 0),
        Line::Rule,
        Line::item("Move up", Target::ListOp(n, L::Up)).enabled(n > 0),
        Line::item("Move down", Target::ListOp(n, L::Down)).enabled(n < last),
        Line::Rule,
        Line::item("Rename", Target::ListOp(n, L::Rename)),
        Line::item("Clear", Target::ListOp(n, L::Clear)).enabled(count > 0),
        Line::item("Delete playlist", Target::ListOp(n, L::Delete)),
    ];
    theme::menu(p, s, s.menu_at, screen, &lines);
}

/// A held track's menu where it was held: load it to a deck, or add it to playlists.
fn track_menu(p: &mut Painter, s: &mut State, w: f32, h: f32) {
    use crate::theme::Pick;
    let Some(i) = s.track_menu else { return };
    p.layer();
    let screen = Rect::new(0.0, 0.0, w, h);
    hit(s, screen, Target::MenuClose);
    let e = &s.entries[i as usize];
    let key = crate::state::entry_key(e);
    let saved = s.store.get(&key).unwrap_or_default();
    let title = if e.artist.is_empty() { e.title.to_string() } else { format!("{} - {}", e.artist, e.title) };
    let held = s.playlists.iter().filter(|(_, k)| k.contains(&key)).count();
    let worn = s.tags.iter().filter(|(_, k)| k.contains(&key)).count();
    let free = |d: usize| !(s.settings.on(crate::settings::Opt::Protect) && s.telemetry.decks[d].playing);
    let stars = (1..=5u8).map(|n| (Pick::Star(n <= saved.rating), Target::MenuRate(n))).collect();
    let mut colours = vec![(Pick::Clear(saved.colour == 0), Target::MenuColour(0))];
    colours
        .extend((1..=8u8).map(|n| (Pick::Swatch(CUES[usize::from(n) - 1], saved.colour == n), Target::MenuColour(n))));
    let mut lines = vec![
        Line::item(title, Target::MenuBody).enabled(false),
        Line::Picks { label: "Rating", picks: stars },
        Line::Picks { label: "Color", picks: colours },
        Line::Picks {
            label: "Stage",
            picks: crate::state::STAGES
                .iter()
                .zip(1u8..)
                .map(|(&(_, c), n)| (Pick::Swatch(rgb(c), saved.stage == n), Target::MenuStage(n)))
                .collect(),
        },
        Line::Rule,
        Line::Picks {
            label: "Load on Deck",
            picks: (0..2).map(|d| (Pick::Deck(d as u8 + 1, free(d)), Target::MenuDeck(d))).collect(),
        },
        Line::Picks {
            label: "Play on Deck",
            picks: (0..2).map(|d| (Pick::Deck(d as u8 + 1, free(d)), Target::MenuPlay(d))).collect(),
        },
        Line::sub("Add to playlist", 0).hint(if held > 0 { format!("In {held}") } else { String::new() }),
        Line::sub("Tags", 4).hint(if worn > 0 { worn.to_string() } else { String::new() }),
    ];
    if matches!(s.shelf, crate::state::Shelf::Playlist(_)) {
        lines.push(Line::item("Remove from this list", Target::MenuRemove));
    }
    lines.push(Line::Rule);
    if !e.artist.is_empty() {
        lines.push(Line::item("Go to Artist", Target::MenuArtist));
    }
    if !e.folder.is_empty() {
        lines.push(Line::item("Go to Folder", Target::MenuFolder));
    }
    if held > 0 {
        lines.push(Line::sub("Show in playlist", 2));
    }
    lines.extend([
        Line::sub("Show Info", 3),
        Line::sub("Sort by", 1),
        Line::Rule,
        Line::item("Analyze Track", Target::MenuAnalyze),
        Line::item("Look for New Tracks", Target::Rescan),
    ]);
    let (open, r) = theme::menu(p, s, s.menu_at, screen, &lines);
    let Some(top) = open else { return };
    let sub: Vec<Line> = match s.submenu {
        Some(1) => {
            use crate::library::Column as C;
            let cols = [
                (C::Index, "Order added"),
                (C::Title, "Title"),
                (C::Artist, "Artist"),
                (C::Bpm, "BPM"),
                (C::Key, "Key"),
                (C::Time, "Length"),
                (C::Rating, "Rating"),
            ];
            let (now, up) = s.sort;
            cols.into_iter()
                .map(|(c, n)| {
                    let hint = if c == now { if up { "Up" } else { "Down" } } else { "" };
                    Line::item(n, Target::Sort(c)).mark(Mark::Check(c == now)).hint(hint.to_string())
                })
                .collect()
        }
        Some(2) => s
            .playlists
            .iter()
            .enumerate()
            .filter(|(_, (_, keys))| keys.contains(&key))
            .map(|(n, (name, _))| Line::item(name.clone(), Target::Shelf(crate::state::Shelf::Playlist(n))))
            .collect(),
        Some(3) => info_lines(s, i as usize),
        Some(4) => {
            let mut tags: Vec<Line> = s
                .tags
                .iter()
                .enumerate()
                .map(|(n, (name, keys))| {
                    Line::item(name.clone(), Target::MenuTag(n)).mark(Mark::Check(keys.contains(&key)))
                })
                .collect();
            if !tags.is_empty() {
                tags.push(Line::Rule);
            }
            tags.push(Line::item("New tag", Target::NewTag));
            tags
        }
        _ => {
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
            lists
        }
    };
    theme::submenu(p, s, r, top, screen, &sub);
}

/// What is known of a track, as lines to read.
fn info_lines(s: &State, i: usize) -> Vec<Line> {
    let e = &s.entries[i];
    let m = s.meta.get(i).copied().flatten();
    let mut rows: Vec<(&str, String)> = vec![("Title", e.title.to_string())];
    if !e.artist.is_empty() {
        rows.push(("Artist", e.artist.to_string()));
    }
    if !e.folder.is_empty() {
        rows.push(("Folder", e.folder.to_string()));
    }
    if let Some(m) = m {
        if m.bpm > 0.0 {
            rows.push(("BPM", format!("{:.1}", m.bpm)));
        }
        if let Some(k) = m.key {
            rows.push(("Key", s.settings.key(k)));
        }
        let secs = m.secs as u32;
        rows.push(("Length", format!("{}:{:02}", secs / 60, secs % 60)));
    }
    if let crate::library::Source::File(p) = &e.source {
        if let Some(ext) = p.extension() {
            rows.push(("Format", ext.to_string_lossy().to_uppercase()));
        }
        if let Ok(md) = std::fs::metadata(p) {
            rows.push(("Size", format!("{:.1} MB", md.len() as f64 / 1e6)));
        }
    }
    rows.into_iter().map(|(k, v)| Line::item(v, Target::MenuBody).hint(k).enabled(false)).collect()
}

/// The filter chips over the track list; returns the room left under them.
fn chips(p: &mut Painter, s: &mut State, r: Rect) -> Rect {
    let h = 30.0;
    let st = Style::new(11.0, Weight::Semibold, GREY);
    let mut x = r.x + 12.0;
    p.text(x, r.y + h * 0.5 + 4.0, Style { colour: TEXT, ..st }, "Filter");
    x += p.text_width(st, "Filter") + 10.0;
    for (n, name) in crate::state::FILTERS.iter().enumerate() {
        // Chips with a value say it, and carry a mark that holding opens their editor.
        let label = match n {
            0 => format!("{name} {}%", crate::state::NEAR_BPM[usize::from(s.near_bpm)]),
            2 if s.min_stars > 1 => format!("{name} {}+", s.min_stars),
            _ => (*name).to_string(),
        };
        let name = label.as_str();
        let editable = matches!(n, 0 | 2);
        let w = p.text_width(st, name) + if editable { 32.0 } else { 20.0 };
        if x + w > r.x + r.w - 8.0 {
            break;
        }
        let on = s.filters & (1 << n) != 0;
        let t = Target::Filter(n as u8);
        let b = Rect::new(x, r.y + 5.0, w, h - 10.0);
        let fill = if on {
            fade(ACCENT, 0x30)
        } else if s.pressed(t) {
            rgb(0x1c1c1f)
        } else {
            INK
        };
        p.outlined(b.x, b.y, b.w, b.h, b.h * 0.5, fill, 1.0, if on { ACCENT } else { EDGE });
        p.text(b.x + 10.0, b.y + b.h * 0.5 + 4.0, Style { colour: if on { TEXT } else { GREY }, ..st }, name);
        if editable {
            let (ax, ay) = (b.x + b.w - 13.0, b.y + b.h * 0.5);
            p.line((ax - 3.0, ay - 1.5), (ax, ay + 1.5), 1.3, GREY);
            p.line((ax, ay + 1.5), (ax + 3.0, ay - 1.5), 1.3, GREY);
        }
        hit(s, b, t);
        x += w + 6.0;
    }
    Rect::new(r.x, r.y + h, r.w, r.h - h)
}

pub fn library(p: &mut Painter, s: &mut State, r: Rect) {
    let r = if r.h > 220.0 && !s.search_in_bar { library_head(p, s, r) } else { r };
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
    p.rect(r.x, r.y, r.w, r.h, 0.0, BROWSER);
    let r = if r.h > 200.0 { chips(p, s, r) } else { r };
    let bar = 40.0;
    let list = Rect::new(r.x, r.y, r.w, r.h - bar);
    s.library = list;
    s.row_h = 26.0;
    let wide = r.w >= 600.0;
    let cols: &[(C, &str, f32)] = if wide {
        &[
            (C::Index, "#", 0.05),
            (C::Title, "TITLE", 0.37),
            (C::Artist, "ARTIST", 0.22),
            (C::Rating, "RATING", 0.1),
            (C::Bpm, "BPM", 0.08),
            (C::Key, "KEY", 0.07),
            (C::Time, "TIME", 0.11),
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
    // Hidden columns give their share to the title.
    let gone: f32 = cols.iter().filter(|c| s.hidden_cols & (1 << c.0 as u8) != 0).map(|c| c.2).sum();
    let cols: Vec<(C, &str, f32)> = cols
        .iter()
        .filter(|c| s.hidden_cols & (1 << c.0 as u8) == 0)
        .map(|&(c, n, f)| (c, n, if c == C::Title { f + gone } else { f }))
        .collect();
    let inner = r.w - 24.0;
    // Figures keep to their own width on a wide list; the names take what that frees.
    let cap = |c: C| match c {
        C::Index => 48.0,
        C::Rating => 96.0,
        C::Bpm | C::Key | C::Time => 76.0,
        _ => f32::INFINITY,
    };
    let freed: f32 = cols.iter().map(|&(c, _, f)| (f - cap(c) / inner).max(0.0)).sum();
    let names: f32 = cols.iter().filter(|c| cap(c.0).is_infinite()).map(|c| c.2).sum();
    let cols: Vec<(C, &str, f32)> = cols
        .into_iter()
        .map(|(c, n, f)| (c, n, if cap(c).is_infinite() { f + freed * f / names } else { f.min(cap(c) / inner) }))
        .collect();
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
        let colour = s.store.get(&crate::state::entry_key(e)).map_or(0, |v| v.colour);
        if colour > 0 {
            p.rect(r.x + 1.0, y + 2.0, 3.0, row - 4.0, 1.5, CUES[usize::from(colour) - 1]);
        }
        // The picked row reads in plain white over its highlight.
        let picked = s.selected == Some(i);
        if picked {
            p.rect(r.x + 4.0, y, r.w - 8.0, row, 3.0, fade(ACCENT, 0x58));
        } else if let Some(d) = deck {
            p.rect(r.x + 6.0, y, r.w - 12.0, row, 4.0, fade(DECK[d.min(1)], 0x50));
        } else if n % 2 == 1 {
            p.rect(r.x + 4.0, y, r.w - 8.0, row, 0.0, rgb(0x161819));
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
                    Style { colour: if picked { TEXT } else { AMBER }, ..cell },
                    &m.filter(|m| m.bpm > 0.0).map_or(String::new(), |m| format!("{:.1}", m.bpm)),
                ),
                C::Key => p.text(
                    cx,
                    base,
                    Style { colour: if picked { TEXT } else { GREEN }, ..cell },
                    &m.and_then(|m| m.key).map_or(String::new(), |k| s.settings.key(k)),
                ),
                C::Time => p.text(cx, base, Style { colour: GREY, ..cell }, &m.map_or(String::new(), |m| mmss(m.secs))),
                C::Rating => {
                    let stars = s.store.get(&crate::state::entry_key(e)).map_or(0, |v| v.rating);
                    for k in 0..5u8 {
                        let c = if k < stars {
                            rgb(0xffd60a)
                        } else if picked {
                            fade(TEXT, 0x70)
                        } else {
                            rgb(0x5a5a5e)
                        };
                        theme::star(p, (cx + 5.0 + f32::from(k) * 12.0, y + row * 0.5), 4.5, c);
                    }
                    0.0
                }
            };
        }
    }
    if s.shown.is_empty() {
        p.text(r.x + 12.0, r.y + 52.0, ARTIST, "No matches");
    }
    let y = r.y + r.h - bar;
    p.rect(r.x, y, r.w, bar, 0.0, theme::PANEL);
    p.rect(r.x, y, r.w, 1.0, 0.0, theme::SEPARATOR);
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
        let label = ["DECK A", "DECK B"][d];
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
    let head = format!("DECK {} SAMPLE PAD {bank}{}", ["A", "B"][d], pad + 1);
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
            p.rect(x + 8.0, r.y + bar - 3.0, tw - 16.0, 2.0, 1.0, rgb(sv.colour()));
        }
        hit(s, Rect::new(x, r.y, tw, bar), Target::HubService(sv));
        x += tw;
    }
    let fx = x + 12.0;
    let fw = (r.w - fx - 110.0).max(120.0);
    let field = Rect::new(fx, r.y + 6.0, fw, bar - 12.0);
    p.outlined(field.x, field.y, field.w, field.h, 4.0, DEEP, 1.0, if s.hub.focus { ACCENT } else { EDGE });
    let (shown, ink) = if s.hub.query.is_empty() && !s.hub.focus {
        (format!("Search {}", s.hub.service.title()), DARK)
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
            "Searching".to_string()
        } else {
            format!(
                "Search {}, then send a result to deck 1 or 2. Audio is cached for next time.",
                s.hub.service.title()
            )
        };
        let msg = msg.as_str();
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
                None => format!("DECK {}", ["A", "B"][d]),
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
        p.text_in(r.x + 12.0, r.y + 44.0, r.w - 24.0, ARTIST, "No lyrics for the loaded tracks");
        return;
    };
    let Some(l) = &s.decks[d].loaded else { return };
    let at = s.decks[d].seconds(s.telemetry.decks[d].position);
    let cur = l.lyrics.current(at);
    let line_h = 22.0;
    let rows = ((r.h - 36.0) / line_h).max(1.0) as usize;
    let first = cur.unwrap_or(0).saturating_sub(rows / 3);
    for (n, line) in l.lyrics.lines.iter().enumerate().skip(first).take(rows) {
        let (x, y, w) = (r.x + 12.0, r.y + 44.0 + (n - first) as f32 * line_h, r.w - 24.0);
        if Some(n) != cur {
            let past = cur.is_some_and(|c| n < c);
            p.text_in(x, y, w, Style { px: 12.5, colour: if past { DARK } else { GREY }, ..ARTIST }, &line.text);
            continue;
        }
        // The current line as karaoke: sung words in the deck colour, the word being sung filling in.
        let f = l.lyrics.fill(n, at);
        let lit = Style::new(14.0, Weight::Bold, DECK[d]);
        let dim = Style { colour: TEXT, ..lit };
        let mut cx = x;
        if !f.sung.is_empty() {
            cx += p.text_in(cx, y, w, lit, &f.sung);
            if !f.sung.ends_with(' ') {
                cx += p.text_width(lit, " ");
            }
        }
        if !f.word.is_empty() {
            let ww = p.text_width(dim, &f.word);
            let mix = fade(DECK[d], 0x60 + (159.0 * f.part.clamp(0.0, 1.0)) as u32);
            p.text_in(cx, y, (x + w - cx).max(0.0), Style { colour: mix, ..lit }, &f.word);
            cx += ww + p.text_width(dim, " ");
        }
        if !f.rest.is_empty() && cx < x + w {
            p.text_in(cx, y, x + w - cx, dim, &f.rest);
        }
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
                parts.push(format!("{:.2} BPM", g.bpm * (1.0 + f64::from(deck.rate) * s.settings.rate_range())));
            }
            if let Some(k) = l.key {
                parts.push(s.settings.key(k));
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

/// A count of tracks in words.
fn tracks(n: usize) -> String {
    if n == 1 { "1 track".to_string() } else { format!("{n} tracks") }
}
