//! The look shared by the decks, the mixer, the toolbar and the menus: near-black
//! panels with hairline borders, flat controls lit by a tint when on, soft shadows.

use sugar_layout::Rect;
use sugar_ui::{Painter, Style, Weight, rgb};

use crate::state::{State, Target};
use crate::sugar::{fade, hit};

pub const BACKDROP: u32 = rgb(0x0e0e10);
pub const PANEL: u32 = rgb(0x1a1a1c);
pub const RAISED: u32 = rgb(0x232326);
pub const CONTROL: u32 = rgb(0x161618);
pub const PRESSED: u32 = rgb(0x2c2c2f);
pub const BORDER: u32 = rgb(0x353538);
pub const SEPARATOR: u32 = rgb(0x28282b);
pub const WELL: u32 = rgb(0x0d0d0e);
pub const TEXT: u32 = rgb(0xf2f2f7);
pub const DIM: u32 = rgb(0x8e8e93);
pub const FAINT: u32 = rgb(0x55555c);
pub const ACCENT: u32 = rgb(0xff4fa8);

pub fn style(px: f32, weight: Weight, colour: u32) -> Style {
    Style::new(px.round().max(7.0), weight, colour)
}

pub fn centred(p: &mut Painter, r: Rect, st: Style, text: &str) {
    let w = p.text_width(st, text).min(r.w - 2.0);
    p.text_in(r.x + (r.w - w) * 0.5, r.y + r.h * 0.5 + st.px * 0.36, r.w - 2.0, st, text);
}

/// A flat control: a hairline frame, the tint washed in and outlined when on.
pub fn control(p: &mut Painter, r: Rect, radius: f32, on: bool, down: bool, tint: u32) {
    let (fill, edge) = match (down, on) {
        (true, true) => (fade(tint, 0x66), tint),
        (true, false) => (PRESSED, BORDER),
        (false, true) => (fade(tint, 0x2a), fade(tint, 0xd0)),
        (false, false) => (CONTROL, BORDER),
    };
    p.outlined(r.x, r.y, r.w, r.h, radius, fill, 1.0, edge);
    if !on && !down {
        p.rect(r.x + radius, r.y + 1.0, r.w - 2.0 * radius, 1.0, 0.0, fade(rgb(0xffffff), 0x0a));
    }
}

/// A labelled control; the label takes the tint when on.
pub fn labelled(p: &mut Painter, r: Rect, label: &str, px: f32, on: bool, down: bool, tint: u32) {
    let radius = (r.h * 0.28).clamp(4.0, 9.0);
    control(p, r, radius, on, down, tint);
    let ink = if on {
        tint
    } else if down {
        TEXT
    } else {
        DIM
    };
    centred(p, r, style(px, Weight::Semibold, ink), label);
}

/// A floating menu card over a shadow, with a tinted strip along its top.
pub fn card(p: &mut Painter, r: Rect, tint: u32) {
    p.glow(r.x, r.y + 8.0, r.w, r.h, 14.0, 24.0, fade(rgb(0x000000), 0xd0));
    p.outlined(r.x, r.y, r.w, r.h, 14.0, PANEL, 1.0, BORDER);
    p.rect(r.x + 14.0, r.y, r.w - 28.0, 2.0, 1.0, tint);
}

pub const MENU: u32 = rgb(0x232326);
pub const MENU_BORDER: u32 = rgb(0x353538);
pub const MENU_HOVER: u32 = rgb(0x3a3a3d);
const MENU_ROW: f32 = 26.0;
const MENU_SEP: f32 = 9.0;
const MENU_PAD: f32 = 6.0;
pub const MENU_W: f32 = 220.0;

/// A menu line: an action, a rule between groups, or a submenu opening beside it.
pub enum Line {
    Item {
        text: String,
        hint: String,
        target: Target,
        enabled: bool,
        mark: Mark,
    },
    Sub {
        text: String,
        hint: String,
        id: u8,
    },
    /// A label and a row of small buttons, as stars or colours.
    Picks {
        label: &'static str,
        picks: Vec<(Pick, Target)>,
    },
    Rule,
}

/// One of a [`Line::Picks`] row's buttons.
#[derive(Clone, Copy, PartialEq)]
pub enum Pick {
    /// A star, lit or not.
    Star(bool),
    /// A colour, chosen or not.
    Swatch(u32, bool),
    /// No colour, chosen or not.
    Clear(bool),
    /// A deck's number, usable or not.
    Deck(u8, bool),
}

/// What sits before a line's text.
#[derive(Clone, Copy, PartialEq)]
pub enum Mark {
    None,
    Check(bool),
    Swatch(u32),
}

impl Line {
    pub fn item(text: impl Into<String>, target: Target) -> Self {
        Line::Item { text: text.into(), hint: String::new(), target, enabled: true, mark: Mark::None }
    }

    #[must_use]
    pub fn hint(mut self, h: impl Into<String>) -> Self {
        match &mut self {
            Line::Item { hint, .. } | Line::Sub { hint, .. } => *hint = h.into(),
            Line::Picks { .. } | Line::Rule => {}
        }
        self
    }

    #[must_use]
    pub fn enabled(mut self, on: bool) -> Self {
        if let Line::Item { enabled, .. } = &mut self {
            *enabled = on;
        }
        self
    }

    #[must_use]
    pub fn mark(mut self, m: Mark) -> Self {
        if let Line::Item { mark, .. } = &mut self {
            *mark = m;
        }
        self
    }

    pub fn sub(text: impl Into<String>, id: u8) -> Self {
        Line::Sub { text: text.into(), hint: String::new(), id }
    }

    fn height(&self) -> f32 {
        if matches!(self, Line::Rule) { MENU_SEP } else { MENU_ROW }
    }
}

/// The height a menu of these lines takes.
pub fn menu_height(lines: &[Line]) -> f32 {
    lines.iter().map(Line::height).sum::<f32>() + 2.0 * MENU_PAD
}

/// Draws a menu with its top left at `(x, y)`, kept inside `bounds`; returns where the
/// open submenu's line is, for the submenu to open beside it, and the menu's rectangle.
pub fn menu(p: &mut Painter, s: &mut State, (x, y): (f32, f32), bounds: Rect, lines: &[Line]) -> (Option<f32>, Rect) {
    let h = menu_height(lines);
    let w = MENU_W.min(bounds.w - 8.0);
    let r = Rect::new(
        x.clamp(bounds.x + 4.0, bounds.x + bounds.w - w - 4.0),
        y.clamp(bounds.y + 4.0, (bounds.y + bounds.h - h - 4.0).max(bounds.y + 4.0)),
        w,
        h,
    );
    p.glow(r.x, r.y + 4.0, r.w, r.h, 6.0, 12.0, fade(rgb(0x000000), 0xa0));
    p.outlined(r.x, r.y, r.w, r.h, 6.0, MENU, 1.0, MENU_BORDER);
    hit(s, r, Target::MenuBody);
    let mut ly = r.y + MENU_PAD;
    let mut open_at = None;
    let text = Style::new(13.0, Weight::Regular, TEXT);
    let small = Style::new(12.0, Weight::Regular, DIM);
    for line in lines {
        let row = Rect::new(r.x + MENU_PAD, ly, r.w - 2.0 * MENU_PAD, line.height());
        ly += row.h;
        let (label, hint, lit, enabled, mark, chevron) = match line {
            Line::Rule => {
                p.rect(row.x - MENU_PAD, row.y + 4.0, r.w, 1.0, 0.0, MENU_BORDER);
                continue;
            }
            Line::Picks { label, picks } => {
                picks_row(p, s, row, label, picks);
                continue;
            }
            Line::Item { text, hint, target, enabled, mark } => {
                let down = *enabled && s.pressed(*target);
                if *enabled {
                    hit(s, row, *target);
                }
                (text, hint, down, *enabled, *mark, false)
            }
            Line::Sub { text, hint, id } => {
                let open = s.submenu == Some(*id);
                if open {
                    open_at = Some(row.y - MENU_PAD);
                }
                hit(s, row, Target::Submenu(*id));
                (text, hint, open || s.pressed(Target::Submenu(*id)), true, Mark::None, true)
            }
        };
        if lit {
            p.rect(row.x, row.y, row.w, row.h, 4.0, MENU_HOVER);
        }
        let mut tx = row.x + 10.0;
        let cy = row.y + row.h * 0.5;
        match mark {
            Mark::None => {}
            Mark::Check(on) => {
                if on {
                    p.line((tx, cy), (tx + 3.5, cy + 3.5), 1.6, TEXT);
                    p.line((tx + 3.5, cy + 3.5), (tx + 10.0, cy - 4.0), 1.6, TEXT);
                }
                tx += 22.0;
            }
            Mark::Swatch(c) => {
                p.rect(tx, cy - 6.0, 12.0, 12.0, 6.0, c);
                tx += 22.0;
            }
        }
        let right = row.x + row.w - if chevron { 26.0 } else { 10.0 };
        let hw = if hint.is_empty() { 0.0 } else { p.text_width(small, hint).min(row.w * 0.42) };
        if hw > 0.0 {
            p.text_in(right - hw, cy + small.px * 0.36, hw, small, hint);
        }
        let ink = if enabled { text } else { Style { colour: DIM, ..text } };
        p.text_in(tx, cy + text.px * 0.36, right - tx - if hw > 0.0 { hw + 8.0 } else { 0.0 }, ink, label);
        if chevron {
            let k = 3.5;
            let ex = row.x + row.w - 14.0;
            p.line((ex - k * 0.6, cy - k), (ex + k * 0.6, cy), 1.5, DIM);
            p.line((ex + k * 0.6, cy), (ex - k * 0.6, cy + k), 1.5, DIM);
        }
    }
    (open_at, r)
}

/// A label, then buttons as wide as the row allows.
fn picks_row(p: &mut Painter, s: &mut State, row: Rect, label: &str, picks: &[(Pick, Target)]) {
    let cy = row.y + row.h * 0.5;
    let st = Style::new(12.0, Weight::Regular, DIM);
    p.text(row.x + 10.0, cy + 4.3, st, label);
    let x0 = row.x + (p.text_width(st, label) + 20.0).max(58.0);
    let wide = picks.iter().any(|(k, _)| matches!(k, Pick::Deck(..)));
    let step = ((row.x + row.w - x0) / picks.len().max(1) as f32).min(if wide { 30.0 } else { 22.0 });
    for (n, &(pick, target)) in picks.iter().enumerate() {
        let b = Rect::new(x0 + n as f32 * step, row.y, step, row.h);
        let (bx, r) = (b.x + step * 0.5, 6.0);
        if s.pressed(target) {
            p.rect(b.x, b.y + 2.0, b.w, b.h - 4.0, 4.0, MENU_HOVER);
        }
        match pick {
            Pick::Star(lit) => star(p, (bx, cy), 6.5, if lit { rgb(0xffd60a) } else { rgb(0x5a5a5e) }),
            Pick::Swatch(c, on) => {
                if on {
                    p.rect(bx - r - 2.0, cy - r - 2.0, 2.0 * r + 4.0, 2.0 * r + 4.0, r + 2.0, TEXT);
                }
                p.rect(bx - r, cy - r, 2.0 * r, 2.0 * r, r, c);
            }
            Pick::Clear(on) => {
                p.outlined(bx - r, cy - r, 2.0 * r, 2.0 * r, r, MENU, 1.0, if on { TEXT } else { DIM });
                p.line((bx - 3.0, cy - 3.0), (bx + 3.0, cy + 3.0), 1.3, DIM);
                p.line((bx + 3.0, cy - 3.0), (bx - 3.0, cy + 3.0), 1.3, DIM);
            }
            Pick::Deck(n, on) => {
                p.outlined(b.x + 2.0, cy - 10.0, b.w - 4.0, 20.0, 5.0, MENU, 1.0, if on { DIM } else { MENU_HOVER });
                let st = Style::new(12.0, Weight::Semibold, if on { TEXT } else { DIM });
                let d = n.to_string();
                p.text(bx - p.text_width(st, &d) * 0.5, cy + 4.3, st, &d);
                if !on {
                    continue;
                }
            }
        }
        hit(s, b, target);
    }
}

/// A five pointed star's outline as lines.
pub fn star(p: &mut Painter, (cx, cy): (f32, f32), r: f32, c: u32) {
    let at = |k: usize| {
        let a = -std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::PI / 5.0;
        let rr = if k.is_multiple_of(2) { r } else { r * 0.45 };
        (cx + rr * a.cos(), cy + rr * a.sin())
    };
    for k in 0..10 {
        p.line(at(k), at(k + 1), 1.4, c);
    }
    if c != rgb(0x5a5a5e) {
        for k in 0..10 {
            p.line((cx, cy), at(k), 2.6, c);
        }
    }
}

/// A submenu beside the line at `top` of the menu `parent`, on whichever side has room.
pub fn submenu(p: &mut Painter, s: &mut State, parent: Rect, top: f32, bounds: Rect, lines: &[Line]) {
    let w = MENU_W.min(bounds.w - 8.0);
    let x = if parent.x + parent.w + 4.0 + w <= bounds.x + bounds.w {
        parent.x + parent.w + 4.0
    } else {
        (parent.x - w - 4.0).max(bounds.x + 4.0)
    };
    menu(p, s, (x, top), bounds, lines);
}
