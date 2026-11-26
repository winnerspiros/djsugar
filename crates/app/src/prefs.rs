//! Settings: a card with the pages down the left and the open page's options
//! on the right, and the app menu that opens it.

use sugar_layout::Rect;
use sugar_ui::{Painter, Style, Weight, rgb};

use crate::settings::{self, Kind, Opt, Row};
use crate::state::{State, Target, View};
use crate::sugar::{fade, hit};
use crate::theme::{self, Line, Mark};

const SIDE: u32 = rgb(0x141416);
const HOVER: u32 = rgb(0x3a3a3d);

/// The app menu, under its button: File, Edit, View and Help, each a submenu.
pub fn app_menu(p: &mut Painter, s: &mut State, w: f32, h: f32) {
    if !s.app_menu {
        return;
    }
    p.layer();
    let screen = Rect::new(0.0, 0.0, w, h);
    hit(s, screen, Target::AppMenu);
    let at = s.rect_of(Target::AppMenu).map_or((8.0, 44.0), |b| (b.x, b.y + b.h + 4.0));
    let lines = [
        Line::sub("File", 0),
        Line::sub("Edit", 3),
        Line::sub("View", 1),
        Line::sub("Help", 2),
        Line::Rule,
        Line::item("Settings", Target::Prefs(0)).hint("Ctrl+,"),
        Line::item("Quit", Target::Quit).hint("Ctrl+Q"),
    ];
    let (open, r) = theme::menu(p, s, at, screen, &lines);
    let Some(top) = open else { return };
    let sub: Vec<Line> = match s.submenu {
        Some(0) => vec![
            Line::item("Record Mix", Target::Record).mark(Mark::Check(s.recorder.is_some())).hint("Ctrl+R"),
            Line::item("Auto Mix", Target::AutoDj).mark(Mark::Check(s.auto.is_some())).hint("Shift+F12"),
            Line::Rule,
            Line::item("Load Selected on Deck A", Target::LoadPick(0)).hint("Ctrl+O"),
            Line::item("Load Selected on Deck B", Target::LoadPick(1)).hint("Ctrl+Shift+O"),
            Line::Rule,
            Line::item("Look for New Tracks", Target::Rescan),
            Line::item("Music Folders", Target::Prefs(4)),
        ],
        Some(3) => vec![
            Line::item("Find", Target::Find).hint("Ctrl+F"),
            Line::item("New Playlist", Target::NewPlaylist).hint("Ctrl+N"),
        ],
        Some(1) => {
            let mut v = vec![
                Line::item("Zoom Waveforms In", Target::WaveZoom(1)).hint("Ctrl+="),
                Line::item("Zoom Waveforms Out", Target::WaveZoom(-1)).hint("Ctrl+-"),
                Line::item("Reset Waveform Zoom", Target::WaveZoom(0)).hint("Ctrl+0"),
                Line::Rule,
                Line::item("Taller Waveforms", Target::WaveHeight(1)),
                Line::item("Shorter Waveforms", Target::WaveHeight(-1)),
                Line::item("Reset Waveform Height", Target::WaveHeight(0)),
                Line::Rule,
            ];
            let views = [
                ("Decks", View::Dj),
                ("Waveforms", View::Wave),
                ("Library", View::Lib),
                ("Effects Station", View::Fx),
                ("Streaming", View::Stream),
                ("Controller", View::Ctl),
            ];
            v.extend(views.into_iter().enumerate().map(|(i, (n, view))| {
                Line::item(n, Target::View(view)).mark(Mark::Check(s.view == view)).hint(format!("Ctrl+Alt+{}", i + 1))
            }));
            v.push(Line::Rule);
            v.push(Line::item("Effects Bar", Target::ToggleFx).mark(Mark::Check(s.show_fx)).hint("Ctrl+5"));
            v.push(Line::item("Samplers Bar", Target::ToggleSamplers).mark(Mark::Check(s.show_samplers)));
            v
        }
        _ => vec![
            Line::item("User Guide", Target::Open(0)),
            Line::item("Keyboard Shortcuts", Target::Prefs(3)),
            Line::Rule,
            Line::item("Report a Problem", Target::Open(1)),
            Line::item("Show the App's Files", Target::Open(2)),
            Line::Rule,
            Line::item("About DJ Sugar", Target::Prefs(7)),
        ],
    };
    theme::submenu(p, s, r, top, screen, &sub);
}

/// The settings card over a dimmed screen.
pub fn overlay(p: &mut Painter, s: &mut State, w: f32, h: f32) {
    let Some(page) = s.prefs else { return };
    p.layer();
    p.rect(0.0, 0.0, w, h, 0.0, fade(rgb(0x000000), 0xa0));
    hit(s, Rect::new(0.0, 0.0, w, h), Target::PrefsClose);
    let margin = if w < 700.0 { 8.0 } else { 48.0 };
    let cw = (w - 2.0 * margin).min(1000.0);
    let ch = (h - 2.0 * margin).min(700.0);
    let r = Rect::new((w - cw) * 0.5, (h - ch) * 0.5, cw, ch);
    p.glow(r.x, r.y + 8.0, r.w, r.h, 12.0, 24.0, fade(rgb(0x000000), 0xc0));
    p.outlined(r.x, r.y, r.w, r.h, 12.0, theme::PANEL, 1.0, theme::BORDER);
    hit(s, r, Target::MenuBody);
    // Pages down the left; on a narrow screen they shrink to their names.
    let sw = if cw < 560.0 { 120.0 } else { 220.0 };
    p.rect(r.x + 1.0, r.y + 1.0, sw, r.h - 2.0, 11.0, SIDE);
    p.rect(r.x + sw, r.y, 1.0, r.h, 0.0, theme::SEPARATOR);
    p.text(r.x + 14.0, r.y + 38.0, Style::new(20.0, Weight::Bold, theme::TEXT), "Settings");
    for (n, name) in settings::PAGES.iter().enumerate() {
        let b = Rect::new(r.x + 8.0, r.y + 60.0 + n as f32 * 40.0, sw - 16.0, 36.0);
        let on = n == page;
        let t = Target::Prefs(n as u8);
        if on || s.pressed(t) {
            p.rect(b.x, b.y, b.w, b.h, 8.0, HOVER);
        }
        let st = Style::new(
            14.0,
            if on { Weight::Semibold } else { Weight::Regular },
            if on { theme::ACCENT } else { theme::TEXT },
        );
        p.text_in(b.x + 10.0, b.y + 23.0, b.w - 16.0, st, name);
        hit(s, b, t);
    }
    let close = Rect::new(r.x + r.w - 40.0, r.y + 14.0, 26.0, 26.0);
    let down = s.pressed(Target::PrefsClose);
    p.rect(close.x, close.y, close.w, close.h, 13.0, if down { HOVER } else { theme::CONTROL });
    let (cx, cy) = (close.x + 13.0, close.y + 13.0);
    p.line((cx - 5.0, cy - 5.0), (cx + 5.0, cy + 5.0), 1.6, theme::DIM);
    p.line((cx + 5.0, cy - 5.0), (cx - 5.0, cy + 5.0), 1.6, theme::DIM);
    hit(s, close, Target::PrefsClose);
    let body = Rect::new(r.x + sw + 28.0, r.y + 16.0, r.w - sw - 28.0 - 20.0, r.h - 32.0);
    p.text(
        body.x,
        body.y + 24.0,
        Style::new(20.0, Weight::Bold, theme::TEXT),
        settings::PAGES[page.min(settings::PAGES.len() - 1)],
    );
    let rows = settings::page(page, &crate::library::roots(), s.out_rate, &s.audio_device());
    let top = body.y + 50.0;
    let bottom = body.y + body.h;
    let total: f32 = rows.iter().map(row_h).sum();
    s.prefs_max = (total - (bottom - top)).max(0.0);
    s.prefs_scroll = s.prefs_scroll.min(s.prefs_max);
    hit(s, Rect::new(body.x, top, body.w, bottom - top), Target::PrefsScroll);
    // Only rows wholly inside the card are drawn, so none spills over its edge.
    let mut y = top - s.prefs_scroll;
    for row in &rows {
        let rh = row_h(row);
        if y >= top - 0.5 && y + rh <= bottom + 0.5 {
            row_at(p, s, Rect::new(body.x, y, body.w, 0.0), row);
        }
        y += rh;
    }
    if s.prefs_max > 0.0 {
        // Where the page is scrolled to.
        let track = bottom - top;
        let th = (track * track / total).max(24.0);
        let ty = top + (track - th) * s.prefs_scroll / s.prefs_max;
        p.rect(r.x + r.w - 6.0, ty, 3.0, th, 1.5, fade(theme::DIM, 0x80));
    }
    choices(p, s, w, h);
}

/// One row of a page at the top of `r`; returns its height.
fn row_h(row: &Row) -> f32 {
    match row {
        Row::Heading(_) => 34.0,
        Row::Info(..) => 44.0,
        Row::Rescan | Row::Opt(_) => 52.0,
    }
}

fn row_at(p: &mut Painter, s: &mut State, r: Rect, row: &Row) -> f32 {
    let title = Style::new(13.0, Weight::Regular, theme::TEXT);
    let small = Style::new(11.5, Weight::Regular, theme::DIM);
    match row {
        Row::Heading(t) => {
            p.text(r.x, r.y + 26.0, Style::new(11.0, Weight::Semibold, theme::DIM), t);
            34.0
        }
        Row::Info(label, value) => {
            let h = 44.0;
            p.text(r.x, r.y + 27.0, title, label);
            let vw = p.text_width(small, value).min(r.w * 0.6);
            p.text_in(r.x + r.w - vw, r.y + 27.0, vw, small, value);
            p.rect(r.x, r.y + h - 1.0, r.w, 1.0, 0.0, theme::SEPARATOR);
            h
        }
        Row::Rescan => {
            let b = Rect::new(r.x, r.y + 12.0, 170.0, 28.0);
            let down = s.pressed(Target::Rescan);
            p.outlined(b.x, b.y, b.w, b.h, 6.0, if down { theme::PRESSED } else { theme::CONTROL }, 1.0, theme::BORDER);
            theme::centred(p, b, Style::new(12.0, Weight::Semibold, theme::TEXT), "Look for new tracks");
            hit(s, b, Target::Rescan);
            52.0
        }
        Row::Opt(o) => opt_row(p, s, r, *o, title, small),
    }
}

/// A setting: its title and what it does, the control on the right.
fn opt_row(p: &mut Painter, s: &mut State, r: Rect, o: Opt, title: Style, small: Style) -> f32 {
    let spec = settings::spec(o);
    let h = 52.0;
    let v = s.settings.get(o);
    let cw = match spec.kind {
        Kind::Switch => 38.0,
        Kind::Segmented(c) => c.iter().map(|t| p.text_width(small, t) + 24.0).sum::<f32>(),
        Kind::Menu(_) => 160.0,
    }
    .min(r.w * 0.5);
    let tw = r.w - cw - 16.0;
    p.text_in(r.x, r.y + 20.0, tw, title, spec.title);
    p.text_in(r.x, r.y + 37.0, tw, small, spec.detail);
    let c = Rect::new(r.x + r.w - cw, r.y + (h - 26.0) * 0.5, cw, 26.0);
    match spec.kind {
        Kind::Switch => {
            let sw = Rect::new(c.x, c.y + 2.0, 38.0, 22.0);
            let on = v != 0;
            p.rect(sw.x, sw.y, sw.w, sw.h, 11.0, if on { theme::ACCENT } else { rgb(0x3a3a3e) });
            let kx = if on { sw.x + sw.w - 20.0 } else { sw.x + 2.0 };
            p.rect(kx, sw.y + 2.0, 18.0, 18.0, 9.0, rgb(0xf2f2f7));
            // The whole row flips it, as a phone's settings do.
            hit(s, Rect::new(r.x, r.y, r.w, h), Target::Pref(o));
        }
        Kind::Segmented(names) => {
            p.outlined(c.x, c.y, c.w, c.h, 6.0, theme::CONTROL, 1.0, theme::BORDER);
            let mut x = c.x;
            for (i, n) in names.iter().enumerate() {
                let bw = (p.text_width(small, n) + 24.0).min(c.w / names.len() as f32 * 1.6);
                let b = Rect::new(x, c.y, bw, c.h);
                let on = usize::from(v) == i;
                if on {
                    p.rect(b.x + 2.0, b.y + 2.0, b.w - 4.0, b.h - 4.0, 4.0, HOVER);
                }
                let st = Style::new(
                    12.0,
                    if on { Weight::Semibold } else { Weight::Regular },
                    if on { theme::TEXT } else { theme::DIM },
                );
                theme::centred(p, b, st, n);
                hit(s, b, Target::PrefPick(o, i as u8));
                x += bw;
            }
        }
        Kind::Menu(names) => {
            let open = s.pref_menu.is_some_and(|m| m.0 == o);
            p.outlined(c.x, c.y, c.w, c.h, 6.0, theme::WELL, 1.0, if open { theme::ACCENT } else { theme::BORDER });
            let now = names.get(usize::from(v)).copied().unwrap_or("");
            p.text_in(c.x + 10.0, c.y + 17.0, c.w - 30.0, Style::new(12.5, Weight::Regular, theme::TEXT), now);
            // A down chevron.
            let (ex, ey) = (c.x + c.w - 14.0, c.y + 13.0);
            p.line((ex - 4.0, ey - 2.0), (ex, ey + 2.0), 1.5, theme::DIM);
            p.line((ex, ey + 2.0), (ex + 4.0, ey - 2.0), 1.5, theme::DIM);
            hit(s, c, Target::Pref(o));
        }
    }
    p.rect(r.x, r.y + h - 1.0, r.w, 1.0, 0.0, theme::SEPARATOR);
    h
}

/// A setting's choices, opened under its field.
fn choices(p: &mut Painter, s: &mut State, w: f32, h: f32) {
    let Some((o, x, y)) = s.pref_menu else { return };
    let Kind::Menu(names) = settings::spec(o).kind else { return };
    p.layer();
    hit(s, Rect::new(0.0, 0.0, w, h), Target::PrefsClose);
    let now = usize::from(s.settings.get(o));
    let lines: Vec<Line> = names
        .iter()
        .enumerate()
        .map(|(i, n)| Line::item(*n, Target::PrefPick(o, i as u8)).mark(Mark::Check(i == now)))
        .collect();
    theme::menu(p, s, (x, y), Rect::new(0.0, 0.0, w, h), &lines);
}
