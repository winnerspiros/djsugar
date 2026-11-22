use super::*;

// Module ids of the main screen in docs/mockup.
const BAR: u32 = 1;
const DECK_A: u32 = 2;
const HUB: u32 = 3;
const DECK_B: u32 = 4;
const LANE: u32 = 5;
const NEXT: u32 = 6;
const FX_A: u32 = 7;
const FX_B: u32 = 8;

/// The mockup's main screen as a layout tree (sizes in logical pixels).
fn main_screen() -> Node {
    Node::group(
        Axis::Column,
        8.0,
        u8::MAX,
        vec![
            Node::module(BAR, 280.0, 40.0, 0.0, 250),
            Node::group(
                Axis::Auto,
                12.0,
                240,
                vec![
                    Node::group(
                        Axis::Column,
                        6.0,
                        230,
                        vec![
                            Node::module(DECK_A, 150.0, 150.0, 1.0, 230),
                            Node::module(FX_A, 150.0, 28.0, 0.0, 40),
                        ],
                    ),
                    Node::module(HUB, 160.0, 160.0, 1.5, 220),
                    Node::group(
                        Axis::Column,
                        6.0,
                        230,
                        vec![
                            Node::module(DECK_B, 150.0, 150.0, 1.0, 230),
                            Node::module(FX_B, 150.0, 28.0, 0.0, 40),
                        ],
                    ),
                ],
            ),
            Node::module(LANE, 280.0, 56.0, 0.3, 120),
            Node::module(NEXT, 280.0, 72.0, 0.6, 60),
        ],
    )
}

const SIZES: &[(f32, f32)] = &[
    (360.0, 780.0),
    (780.0, 360.0),
    (412.0, 915.0),
    (915.0, 412.0),
    (600.0, 960.0),
    (800.0, 1280.0),
    (1024.0, 600.0),
    (1280.0, 800.0),
    (1366.0, 768.0),
    (1600.0, 900.0),
    (1920.0, 1080.0),
    (2560.0, 1440.0),
    (3840.0, 2160.0),
];

fn check(area: Rect, placed: &[(u32, Rect)]) {
    for (i, (a, ra)) in placed.iter().enumerate() {
        assert!(area.contains(ra), "{a} leaves {area:?}: {ra:?}");
        assert!(ra.w > 0.0 && ra.h > 0.0, "{a} is empty");
        for (b, rb) in &placed[i + 1..] {
            assert!(
                !ra.overlaps(rb),
                "{a} {ra:?} overlaps {b} {rb:?} in {area:?}"
            );
        }
    }
}

#[test]
fn every_screen_size_is_valid_and_keeps_the_decks() {
    for &(w, h) in SIZES {
        let area = Rect {
            x: 0.0,
            y: 0.0,
            w,
            h,
        };
        let placed = solve(&main_screen(), area);
        check(area, &placed);
        let ids: Vec<u32> = placed.iter().map(|p| p.0).collect();
        for must in [BAR, DECK_A, HUB, DECK_B] {
            assert!(ids.contains(&must), "{must} missing at {w}x{h}: {ids:?}");
        }
    }
}

#[test]
fn continuous_resize_never_breaks() {
    let mut w = 320.0;
    while w <= 3840.0 {
        let mut h = 320.0;
        while h <= 2160.0 {
            let area = Rect {
                x: 0.0,
                y: 0.0,
                w,
                h,
            };
            check(area, &solve(&main_screen(), area));
            h += 37.0;
        }
        w += 41.0;
    }
}

#[test]
fn wide_is_a_row_tall_is_a_column() {
    let get = |p: &[(u32, Rect)], id| p.iter().find(|x| x.0 == id).unwrap().1;
    let wide = solve(
        &main_screen(),
        Rect {
            x: 0.0,
            y: 0.0,
            w: 1600.0,
            h: 900.0,
        },
    );
    assert!(get(&wide, DECK_A).x < get(&wide, HUB).x && get(&wide, HUB).x < get(&wide, DECK_B).x);
    let tall = solve(
        &main_screen(),
        Rect {
            x: 0.0,
            y: 0.0,
            w: 412.0,
            h: 915.0,
        },
    );
    assert!(get(&tall, DECK_A).y < get(&tall, HUB).y);
}

#[test]
fn low_priority_goes_first() {
    // 780x360 fits everything once the FX rows (priority 40) are hidden.
    let ids = |w, h| {
        solve(
            &main_screen(),
            Rect {
                x: 0.0,
                y: 0.0,
                w,
                h,
            },
        )
        .iter()
        .map(|p| p.0)
        .collect::<Vec<u32>>()
    };
    assert_eq!(ids(780.0, 360.0), [BAR, DECK_A, HUB, DECK_B, LANE, NEXT]);
    // 30 px shorter, Up next (60) goes too; the lane (120) stays.
    let ids = ids(700.0, 330.0);
    assert_eq!(ids, [BAR, DECK_A, HUB, DECK_B, LANE]);
    assert!(ids.contains(&DECK_A) && ids.contains(&HUB));
}

#[test]
fn spare_space_respects_max() {
    let n = Node::group(
        Axis::Row,
        0.0,
        1,
        vec![
            Node::Module {
                id: 1,
                min: Size { w: 10.0, h: 10.0 },
                grow: 1.0,
                max: 50.0,
                priority: 1,
            },
            Node::module(2, 10.0, 10.0, 1.0, 1),
        ],
    );
    let p = solve(
        &n,
        Rect {
            x: 0.0,
            y: 0.0,
            w: 200.0,
            h: 10.0,
        },
    );
    assert_eq!(p[0].1.w, 50.0);
    assert_eq!(p[1].1.w, 150.0);
}
