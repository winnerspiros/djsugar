use sugar_layout::{Axis, Id, Node, Rect};
use sugar_ui::{Painter, rgb};

pub const BACKGROUND: u32 = rgb(0x0e0a10);
const PANEL: u32 = rgb(0x17111a);
const RAISED: u32 = rgb(0x2a1f2f);
const LINE: u32 = rgb(0x34263a);
const DECK_A: u32 = rgb(0xff2e93);
const DECK_B: u32 = rgb(0x3fe0ff);
const ACCENT: u32 = rgb(0xffe45c);

const MENU_H: f32 = 24.0;
const PAD: f32 = 6.0;
const GAP: f32 = 6.0;

#[derive(Clone, Copy)]
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

pub fn tree() -> Node {
    use M::*;
    Node::group(
        Axis::Column,
        GAP,
        u8::MAX,
        vec![
            Node::module(Top as Id, 280.0, 34.0, 0.0, 250),
            Node::module(Waves as Id, 280.0, 80.0, 0.35, 200),
            Node::group(
                Axis::Row,
                GAP,
                240,
                vec![
                    Node::module(DeckA as Id, 150.0, 170.0, 1.0, 240),
                    Node::Module {
                        id: Mixer as Id,
                        min: sugar_layout::Size { w: 150.0, h: 170.0 },
                        grow: 0.5,
                        max: 340.0,
                        priority: 230,
                    },
                    Node::module(DeckB as Id, 150.0, 170.0, 1.0, 240),
                ],
            ),
            Node::group(
                Axis::Row,
                GAP,
                100,
                vec![
                    Node::module(Library as Id, 240.0, 120.0, 1.0, 100),
                    Node::Module {
                        id: Side as Id,
                        min: sugar_layout::Size { w: 200.0, h: 120.0 },
                        grow: 0.0,
                        max: 260.0,
                        priority: 50,
                    },
                ],
            ),
        ],
    )
}

pub fn inset(window: Rect) -> Rect {
    Rect::new(
        PAD,
        MENU_H + PAD,
        (window.w - 2.0 * PAD).max(0.0),
        (window.h - MENU_H - 2.0 * PAD).max(0.0),
    )
}

pub fn paint(p: &mut Painter, window: Rect, placed: &[(Id, Rect)]) {
    p.rect(0.0, 0.0, window.w, MENU_H, 0.0, PANEL);
    for &(id, r) in placed {
        p.outlined(r.x, r.y, r.w, r.h, 8.0, PANEL, 1.0, LINE);
        match id {
            x if x == M::Waves as Id => {
                let h = (r.h - 3.0 * PAD) / 2.0;
                p.rect(r.x + PAD, r.y + PAD, r.w - 2.0 * PAD, h, 4.0, RAISED);
                p.rect(
                    r.x + PAD,
                    r.y + 2.0 * PAD + h,
                    r.w - 2.0 * PAD,
                    h,
                    4.0,
                    RAISED,
                );
                p.rect(
                    r.x + r.w * 0.5 - 1.0,
                    r.y + PAD,
                    2.0,
                    r.h - 2.0 * PAD,
                    1.0,
                    rgb(0xffffff),
                );
            }
            x if x == M::DeckA as Id => deck(p, r, DECK_A),
            x if x == M::DeckB as Id => deck(p, r, DECK_B),
            x if x == M::Mixer as Id => mixer(p, r),
            _ => {}
        }
    }
}

fn deck(p: &mut Painter, r: Rect, colour: u32) {
    let jog = (r.h * 0.55).min(r.w * 0.4);
    p.outlined(
        r.x + PAD * 2.0,
        r.y + r.h - jog - PAD * 2.0,
        jog,
        jog,
        jog * 0.5,
        RAISED,
        2.0,
        colour,
    );
    let px = r.x + jog + PAD * 4.0;
    let pw = r.w - jog - PAD * 6.0;
    if pw < 40.0 {
        return;
    }
    let ph = (r.h * 0.45 - PAD) / 2.0;
    let cw = (pw - 3.0 * PAD) / 4.0;
    for i in 0..8 {
        let (col, row) = ((i % 4) as f32, (i / 4) as f32);
        let x = px + col * (cw + PAD);
        let y = r.y + r.h - PAD * 2.0 - (2.0 - row) * (ph + PAD) + PAD;
        let stroke = if i == 0 { colour } else { LINE };
        p.outlined(x, y, cw, ph, 6.0, RAISED, 1.0, stroke);
    }
    p.rect(
        r.x + PAD * 2.0,
        r.y + PAD * 2.0,
        (r.w * 0.5).min(260.0),
        14.0,
        3.0,
        RAISED,
    );
}

fn mixer(p: &mut Painter, r: Rect) {
    let cx = r.x + r.w * 0.5;
    for (i, colour) in [DECK_A, DECK_B].into_iter().enumerate() {
        let x = if i == 0 {
            r.x + r.w * 0.25
        } else {
            r.x + r.w * 0.75
        };
        let k = (r.w * 0.12).min(28.0);
        for n in 0..4 {
            p.outlined(
                x - k * 0.5,
                r.y + PAD * 2.0 + n as f32 * (k + PAD),
                k,
                k,
                k * 0.5,
                RAISED,
                2.0,
                colour,
            );
        }
        let top = r.y + PAD * 2.0 + 4.0 * (k + PAD);
        let h = (r.h - (top - r.y) - 50.0).max(10.0);
        p.rect(x - 3.0, top, 6.0, h, 3.0, RAISED);
        p.rect(x - 12.0, top + h * 0.3, 24.0, 12.0, 3.0, rgb(0xdddddd));
    }
    let y = r.y + r.h - 26.0;
    p.rect(r.x + PAD * 3.0, y, r.w - PAD * 6.0, 6.0, 3.0, RAISED);
    p.rect(cx - 7.0, y - 8.0, 14.0, 22.0, 3.0, ACCENT);
}

#[cfg(test)]
mod tests {
    use super::*;
    use sugar_layout::Layout;

    #[test]
    fn decks_and_mixer_survive_every_size() {
        let mut layout = Layout::new(&tree());
        let mut out = Vec::new();
        for (w, h) in [
            (780.0, 360.0),
            (915.0, 412.0),
            (1024.0, 600.0),
            (1280.0, 800.0),
            (1920.0, 1080.0),
            (3840.0, 2160.0),
        ] {
            let window = Rect::new(0.0, 0.0, w, h);
            layout.solve(inset(window), &mut out);
            for m in [M::DeckA, M::Mixer, M::DeckB] {
                assert!(out.iter().any(|p| p.0 == m as Id), "{w}x{h}");
            }
        }
    }
}
