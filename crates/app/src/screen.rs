use sugar_layout::{Axis, Id, Node, Rect, Size};
use sugar_ui::{Painter, Style, Weight, rgb};

pub const BACKGROUND: u32 = rgb(0x0e0a10);
const PANEL: u32 = rgb(0x17111a);
const RAISED: u32 = rgb(0x2a1f2f);
const LINE: u32 = rgb(0x34263a);
const DECK_A: u32 = rgb(0xff2e93);
const DECK_B: u32 = rgb(0x3fe0ff);
const ACCENT: u32 = rgb(0xffe45c);
const TEXT: u32 = rgb(0xf6ecf3);
const DIM: u32 = rgb(0x9c8796);
const CAP: u32 = rgb(0xdddddd);

const MENU_ITEM: Style = Style::new(12.5, Weight::Regular, TEXT);
const BRAND: Style = Style::new(14.0, Weight::Bold, DECK_A);
const HINT: Style = Style::new(13.0, Weight::Regular, DIM);
const HEADER: Style = Style::new(11.0, Weight::Semibold, DIM);
const TITLE: Style = Style::new(15.0, Weight::Semibold, TEXT);
const SUB: Style = Style::new(12.0, Weight::Regular, DIM);
const INFO: Style = Style::new(13.0, Weight::Semibold, ACCENT);
const PAD_LABEL: Style = Style::new(11.0, Weight::Semibold, DIM);
const KNOB_LABEL: Style = Style::new(9.5, Weight::Semibold, DIM);
const CELL: Style = Style::new(13.0, Weight::Regular, TEXT);
const KEY_CELL: Style = Style::new(13.0, Weight::Semibold, ACCENT);
const LYRIC: Style = Style::new(13.0, Weight::Regular, DIM);
const LYRIC_NOW: Style = Style::new(15.0, Weight::Bold, TEXT);

const MENU: [&str; 9] = ["File", "Edit", "View", "Deck", "Library", "Mix", "Tools", "Window", "Help"];
const MENU_H: f32 = 24.0;
const PAD: f32 = 6.0;
const GAP: f32 = 6.0;

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
}

const ALL: [M; 7] = [M::Top, M::Waves, M::DeckA, M::Mixer, M::DeckB, M::Library, M::Side];

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
            module(M::Waves, 280.0, 80.0, 0.35, open, 200),
            Node::group(
                Axis::Row,
                GAP,
                240,
                vec![
                    module(M::DeckA, 150.0, 170.0, 1.0, open, 240),
                    module(M::Mixer, 150.0, 170.0, 0.5, 340.0, 230),
                    module(M::DeckB, 150.0, 170.0, 1.0, open, 240),
                ],
            ),
            Node::group(
                Axis::Row,
                GAP,
                100,
                vec![module(M::Library, 240.0, 120.0, 1.0, open, 100), module(M::Side, 200.0, 120.0, 0.0, 280.0, 50)],
            ),
        ],
    )
}

pub fn inset(window: Rect) -> Rect {
    Rect::new(PAD, MENU_H + PAD, (window.w - 2.0 * PAD).max(0.0), (window.h - MENU_H - 2.0 * PAD).max(0.0))
}

pub fn paint(p: &mut Painter, window: Rect, placed: &[(Id, Rect)]) {
    p.rect(0.0, 0.0, window.w, MENU_H, 0.0, PANEL);
    let mut x = 10.0;
    for item in MENU {
        x += p.text(x, 16.5, MENU_ITEM, item) + 18.0;
    }
    for &(id, r) in placed {
        p.outlined(r.x, r.y, r.w, r.h, 8.0, PANEL, 1.0, LINE);
        match ALL[usize::from(id)] {
            M::Top => top(p, r),
            M::Waves => waves(p, r),
            M::DeckA => deck(p, r, DECK_A, ["A", "Nightdrive", "Kessler & Ama", "124.0  8A"]),
            M::DeckB => deck(p, r, DECK_B, ["B", "Copper Sun", "Diaz / Mbeki", "124.0  8B"]),
            M::Mixer => mixer(p, r),
            M::Library => library(p, r),
            M::Side => lyrics(p, r),
        }
    }
}

fn top(p: &mut Painter, r: Rect) {
    let x = r.x + 28.0 + p.text(r.x + 12.0, r.y + 22.0, BRAND, "DJ Sugar");
    p.text_in(x, r.y + 22.0, r.x + r.w - x - 12.0, HINT, "Search tracks, streaming, or type a command");
}

fn waves(p: &mut Painter, r: Rect) {
    let h = (r.h - 3.0 * PAD) * 0.5;
    p.rect(r.x + PAD, r.y + PAD, r.w - 2.0 * PAD, h, 4.0, RAISED);
    p.rect(r.x + PAD, r.y + 2.0 * PAD + h, r.w - 2.0 * PAD, h, 4.0, RAISED);
    p.rect(r.x + r.w * 0.5 - 1.0, r.y + PAD, 2.0, r.h - 2.0 * PAD, 1.0, TEXT);
}

fn deck(p: &mut Painter, r: Rect, colour: u32, [name, title, artist, info]: [&str; 4]) {
    let right = r.x + r.w - 12.0;
    let iw = p.text_width(INFO, info);
    p.text(right - iw, r.y + 26.0, INFO, info);
    let x = r.x + 22.0 + p.text(r.x + 12.0, r.y + 26.0, Style::new(18.0, Weight::Bold, colour), name);
    p.text_in(x, r.y + 26.0, right - iw - 12.0 - x, TITLE, title);
    p.text_in(x, r.y + 44.0, right - x, SUB, artist);

    let jog = (r.h * 0.55).min(r.w * 0.4).min(r.h - 64.0);
    p.outlined(r.x + 2.0 * PAD, r.y + r.h - jog - 2.0 * PAD, jog, jog, jog * 0.5, RAISED, 2.0, colour);
    let px = r.x + jog + 4.0 * PAD;
    let pw = r.w - jog - 6.0 * PAD;
    if pw < 40.0 {
        return;
    }
    let ph = (r.h * 0.45 - PAD) * 0.5;
    let cw = (pw - 3.0 * PAD) * 0.25;
    for (i, label) in ["1", "2", "3", "4", "5", "6", "7", "8"].into_iter().enumerate() {
        let x = px + (i % 4) as f32 * (cw + PAD);
        let y = r.y + r.h - 2.0 * PAD - (2 - i / 4) as f32 * (ph + PAD) + PAD;
        p.outlined(x, y, cw, ph, 6.0, RAISED, 1.0, if i == 0 { colour } else { LINE });
        p.text(x + 6.0, y + ph - 6.0, PAD_LABEL, label);
    }
}

fn mixer(p: &mut Painter, r: Rect) {
    let cx = r.x + r.w * 0.5;
    let k = (r.w * 0.12).min(28.0);
    for (side, colour) in [(0.25, DECK_A), (0.75, DECK_B)] {
        let x = r.x + r.w * side;
        for n in 0..4 {
            let y = r.y + 2.0 * PAD + n as f32 * (k + PAD);
            p.outlined(x - k * 0.5, y, k, k, k * 0.5, RAISED, 2.0, colour);
        }
        let top = r.y + 2.0 * PAD + 4.0 * (k + PAD);
        let h = (r.h - (top - r.y) - 50.0).max(10.0);
        p.rect(x - 3.0, top, 6.0, h, 3.0, RAISED);
        p.rect(x - 12.0, top + h * 0.3, 24.0, 12.0, 3.0, CAP);
    }
    for (n, label) in ["TRIM", "HI", "MID", "LOW"].into_iter().enumerate() {
        let y = r.y + 2.0 * PAD + n as f32 * (k + PAD) + k * 0.5 + 3.5;
        p.text(cx - p.text_width(KNOB_LABEL, label) * 0.5, y, KNOB_LABEL, label);
    }
    let y = r.y + r.h - 26.0;
    p.rect(r.x + 3.0 * PAD, y, r.w - 6.0 * PAD, 6.0, 3.0, RAISED);
    p.rect(cx - 7.0, y - 8.0, 14.0, 22.0, 3.0, ACCENT);
}

const COLUMNS: [(&str, f32); 7] =
    [("#", 0.0), ("TITLE", 0.05), ("ARTIST", 0.38), ("BPM", 0.62), ("KEY", 0.72), ("TIME", 0.82), ("", 1.0)];
const TRACKS: [[&str; 6]; 4] = [
    ["1", "Paper Lanterns", "Oksana Vey", "122", "8A", "5:48"],
    ["2", "Low Tide Static", "Marrow Club", "126", "9A", "6:40"],
    ["3", "Halflight", "Tomi Reyes", "125", "9B", "5:31"],
    ["4", "Rooftop 4AM", "Kessler & Ama", "124", "8A", "6:44"],
];

fn library(p: &mut Painter, r: Rect) {
    let inner = r.w - 24.0;
    let x = |i: usize| r.x + 12.0 + inner * COLUMNS[i].1;
    for (i, (name, _)) in COLUMNS[..6].iter().enumerate() {
        p.text(x(i), r.y + 22.0, HEADER, name);
    }
    for (row, track) in TRACKS.iter().enumerate() {
        let y = r.y + 46.0 + row as f32 * 22.0;
        if y > r.y + r.h - 8.0 {
            break;
        }
        for (i, value) in track.iter().enumerate() {
            p.text_in(x(i), y, x(i + 1) - x(i) - 8.0, if i == 4 { KEY_CELL } else { CELL }, value);
        }
    }
}

fn lyrics(p: &mut Painter, r: Rect) {
    let w = r.w - 24.0;
    p.text(r.x + 12.0, r.y + 22.0, HEADER, "LYRICS");
    p.text_in(r.x + 12.0, r.y + 50.0, w, LYRIC, "headlights on the overpass");
    p.text_in(r.x + 12.0, r.y + 76.0, w, LYRIC_NOW, "we drive until the city sleeps");
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
