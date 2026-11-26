//! Renders every view at phone, tablet, DeX and desktop sizes, drives typing and
//! focus loss, checks each control is on screen and big enough to touch, and
//! writes the frames as PNG to the directory given, for CI to keep.

use std::path::{Path, PathBuf};

use djsugar::{Key, Mods, Ui, read_dir_sample};
use sugar_ui::Renderer;

/// Physical pixels and the scale the app draws at there: phones use 0.45 of
/// their density like DJ Sugar does, desktops their own scale.
const SIZES: [(&str, u32, u32, f32); 7] = [
    ("phone-portrait", 1080, 2400, 2.625 * 0.45),
    ("small-portrait", 720, 1600, 2.0 * 0.45),
    ("phone-landscape", 2400, 1080, 2.625 * 0.45),
    ("tablet-portrait", 1600, 2560, 2.0 * 0.45),
    ("dex-window", 1536, 1050, 1.5 * 0.45),
    ("laptop", 1280, 800, 1.0),
    ("desktop", 1920, 1080, 1.0),
];
const TOOLBAR: [&str; 7] = ["Browse(0)", "Browse(1)", "ToggleFx", "ToggleSamplers", "AutoDj", "Record", "SearchFocus"];
const FRAMES: u32 = 30;
const VIEWS: [&str; 6] = ["DJ", "WAVE", "LIB", "FX", "STREAM", "CTL"];

fn save(r: &Renderer<'_>, path: &Path) {
    r.read(|p| {
        let mut rgba = Vec::with_capacity((p.width * p.height * 4) as usize);
        for row in p.bytes.chunks(p.stride).take(p.height as usize) {
            for px in row[..p.width as usize * 4].as_chunks::<4>().0 {
                rgba.extend_from_slice(&[px[2], px[1], px[0], 255]);
            }
        }
        let file = std::io::BufWriter::new(std::fs::File::create(path).expect("create png"));
        let mut enc = png::Encoder::new(file, p.width, p.height);
        enc.set_color(png::ColorType::Rgba);
        enc.write_header().and_then(|mut w| w.write_image_data(&rgba)).expect("write png");
    });
}

fn pixels(r: &Renderer<'_>) -> Vec<u8> {
    let mut v = Vec::new();
    r.read(|p| v.extend_from_slice(p.bytes));
    v
}

/// Ten seconds of 440 Hz, so the library always has a track to hold.
fn tone(path: &std::path::Path) {
    let rate = 44_100u32;
    let data: Vec<u8> = (0..rate * 10)
        .flat_map(|i| {
            ((((i as f32) * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 8000.0) as i16).to_le_bytes()
        })
        .collect();
    let mut f = Vec::with_capacity(44 + data.len());
    f.extend_from_slice(b"RIFF");
    f.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    f.extend_from_slice(b"WAVEfmt ");
    for v in [16u32, 0x0001_0001, rate, rate * 2, 0x0010_0002] {
        f.extend_from_slice(&v.to_le_bytes());
    }
    f.extend_from_slice(b"data");
    f.extend_from_slice(&(data.len() as u32).to_le_bytes());
    f.extend_from_slice(&data);
    std::fs::write(path, f).expect("tone");
}

/// A 64 by 64 cover beside the tone: a pink to blue sweep with a light disc in the middle.
fn cover(path: &std::path::Path) {
    let n = 64u32;
    let mut px = Vec::with_capacity((n * n * 4) as usize);
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = (x as i32 - 32, y as i32 - 32);
            let disc = dx * dx + dy * dy < 14 * 14;
            let t = (x + y) * 255 / (2 * n);
            px.extend_from_slice(&if disc {
                [240, 240, 245, 255]
            } else {
                [255 - t as u8, 40, 120 + t as u8 / 2, 255]
            });
        }
    }
    let file = std::io::BufWriter::new(std::fs::File::create(path).expect("cover"));
    let mut enc = png::Encoder::new(file, n, n);
    enc.set_color(png::ColorType::Rgba);
    enc.write_header().and_then(|mut w| w.write_image_data(&px)).expect("cover");
}

fn main() {
    let out = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "target/shots".into()));
    std::fs::create_dir_all(&out).expect("output dir");
    // Saved state goes to a fresh folder, so runs neither read nor change the DJ's own.
    let data = out.join("data");
    let _ = std::fs::remove_dir_all(&data);
    djsugar::set_data_dir(data);
    if std::env::var_os("DJSUGAR_MUSIC").is_none() {
        let music = out.join("music");
        std::fs::create_dir_all(&music).expect("music dir");
        tone(&music.join("Shots - Tone.wav"));
        cover(&music.join("cover.png"));
        // Run again with the folder in the library's roots.
        let me = std::env::current_exe().expect("own path");
        let status = std::process::Command::new(me)
            .args(std::env::args_os().skip(1))
            .env("DJSUGAR_MUSIC", &music)
            .status()
            .expect("run again");
        std::process::exit(status.code().unwrap_or(1));
    }
    let mut faults = Vec::new();
    let mut perf = Vec::new();
    for (name, w, h, scale) in SIZES {
        let mut r = Renderer::offscreen(w, h).expect("offscreen renderer");
        let mut ui = Ui::new(read_dir_sample);
        ui.resize(w, h, scale);
        ui.render(&mut r);
        let t = std::time::Instant::now();
        for _ in 0..FRAMES {
            ui.render(&mut r);
        }
        let ms = t.elapsed().as_secs_f64() * 1e3 / f64::from(FRAMES);
        perf.push(format!("| {name} | {w}x{h} | {ms:.2} |"));
        for view in VIEWS {
            let tab = format!("View({})", view_name(view));
            // Wide windows show three modes; the rest of the views are on Ctrl+Alt+1 to 6, as in the View menu.
            let wide = w as f32 / scale >= 1100.0;
            if wide {
                let n = VIEWS.iter().position(|v| *v == view).unwrap_or(0);
                let digit = char::from(b'1' + n as u8);
                ui.key(Key::Char(digit), Mods { ctrl: true, alt: true, shift: false });
            } else if !ui.tap(&tab) {
                faults.push(format!("{name}: no {view} tab"));
                continue;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
            ui.render(&mut r);
            ui.render(&mut r);
            let shown = ui.targets();
            for want in TOOLBAR.iter().filter(|t| !wide || !t.starts_with("Browse")) {
                if !shown.iter().any(|(t, _)| t == want) {
                    faults.push(format!("{name} {view}: toolbar lost {want}"));
                }
            }
            for (t, [x, y, tw, th]) in shown {
                let (lw, lh) = (w as f32 / scale, h as f32 / scale);
                if x < -0.5 || y < -0.5 || x + tw > lw + 0.5 || y + th > lh + 0.5 {
                    faults.push(format!("{name} {view}: {t} off screen at {x:.0},{y:.0} {tw:.0}x{th:.0}"));
                }
                if (tw < 20.0 || th < 18.0) && !t.starts_with("Library") && !t.starts_with("Wave") {
                    faults.push(format!("{name} {view}: {t} too small to touch, {tw:.0}x{th:.0}"));
                }
            }
            save(&r, &out.join(format!("{name}-{}.png", view.to_lowercase())));
            // The pad aimed at a deck, where it has the room to offer that; then back at the master.
            if view == "FX" && ui.tap("XyAim") {
                ui.render(&mut r);
                ui.render(&mut r);
                save(&r, &out.join(format!("{name}-fx-deck.png")));
                ui.tap("XyAim");
                ui.tap("XyAim");
            }
        }
        menu(&mut ui, &mut r, (name, scale), &mut faults);
        save(&r, &out.join(format!("{name}-menu.png")));
        settings(&mut ui, &mut r, &out, name, &mut faults);
        let before = pixels(&r);
        r = Renderer::offscreen(w, h).expect("second renderer");
        ui.render(&mut r);
        if pixels(&r) != before {
            faults.push(format!("{name}: frame changes when the GPU is recreated, as after leaving the app"));
        }
        typing(&mut ui, &mut r, name, &mut faults);
        save(&r, &out.join(format!("{name}-typed.png")));
        ui.tap("View(Lib)");
        ui.key(Key::Escape, Mods::default());
        ui.key(Key::Escape, Mods::default());
        ui.render(&mut r);
        if let Some((_, [x, y, w, _])) = ui.targets().into_iter().find(|(t, _)| t == "Library") {
            let (px, py) = ((x + w * 0.3) * scale, (y + 11.0) * scale);
            ui.pointer_down(8, px, py);
            ui.pointer_up(8, py);
            ui.render(&mut r);
        }
        if ui.tap("LoadSelected(0)") {
            for _ in 0..30 {
                ui.render(&mut r);
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            ui.tap("View(Dj)");
            ui.render(&mut r);
            for pad in ["Pad(0, 0)", "Pad(0, 1)"] {
                ui.tap(pad);
                ui.render(&mut r);
            }
            ui.render(&mut r);
            save(&r, &out.join(format!("{name}-loaded.png")));
            // Wide windows: the side panel's tabs, each must open.
            if ui.tap("ToggleSide") {
                ui.render(&mut r);
                for (n, tab) in ["automix", "info", "cues", "lyrics"].iter().enumerate() {
                    if !ui.tap(&format!("SideTab({n})")) {
                        faults.push(format!("{name}: no side panel {tab} tab"));
                    }
                    ui.render(&mut r);
                    // A track's menu puts it on Automix's list, where it shows with a way off again.
                    if n == 0
                        && let Some((_, [x, y, w, _])) = ui.targets().into_iter().find(|(t, _)| t == "Library")
                    {
                        ui.context((x + w * 0.3) * scale, (y + 11.0) * scale);
                        ui.render(&mut r);
                        if !ui.tap("MenuQueue") {
                            faults.push(format!("{name}: track menu has no Add to Automix"));
                        }
                        ui.tap("Cycle(MixStyle)");
                        ui.render(&mut r);
                        // The Order button opens its menu, and a pick closes it.
                        ui.tap("OrderMenu");
                        ui.render(&mut r);
                        save(&r, &out.join(format!("{name}-automix-order.png")));
                        // A list of one has nothing to order, so its menu can only be closed.
                        if !ui.tap("MixOrder(5)") && !ui.tap("MenuClose") {
                            faults.push(format!("{name}: no Automix order menu"));
                        }
                        ui.render(&mut r);
                        // Running, Automix marks what it holds: the crossfader and the mix to come.
                        ui.tap("AutoDj");
                        ui.render(&mut r);
                        save(&r, &out.join(format!("{name}-automix-on.png")));
                        ui.tap("AutoDj");
                        ui.render(&mut r);
                        if !ui.targets().iter().any(|(t, _)| t == "QueueRemove(0)") {
                            faults.push(format!("{name}: the added track is not on the Automix list"));
                        }
                    }
                    save(&r, &out.join(format!("{name}-side-{tab}.png")));
                }
                ui.tap("ToggleSide");
                ui.render(&mut r);
            }
            // A new playlist starts named by typing: the name must stick.
            if ui.tap("NewPlaylist") {
                ui.render(&mut r);
                if ui.targets().iter().any(|(t, _)| t == "NameField") {
                    for _ in 0..12 {
                        ui.key(Key::Backspace, Mods::default());
                    }
                    ui.text("Warm Up");
                    ui.render(&mut r);
                    save(&r, &out.join(format!("{name}-naming.png")));
                    ui.key(Key::Enter, Mods::default());
                    ui.render(&mut r);
                    if ui.typing() || ui.targets().iter().any(|(t, _)| t == "NameField") {
                        faults.push(format!("{name}: Enter did not end naming"));
                    }
                } else {
                    faults.push(format!("{name}: new playlist has no name field"));
                }
            }
            // The pads' stems layer.
            if ui.tap("Mode(0, Stems)") {
                ui.render(&mut r);
                save(&r, &out.join(format!("{name}-stems.png")));
                ui.tap("Mode(0, Cues)");
            }
            // The effects bar: a slot's list opens, a pick loads it and turns it on.
            if !ui.targets().iter().any(|(t, _)| t == "FxName(0, 0)") {
                ui.key(Key::Char('5'), Mods { ctrl: true, ..Mods::default() });
                ui.render(&mut r);
            }
            if ui.tap("FxName(0, 0)") {
                ui.render(&mut r);
                save(&r, &out.join(format!("{name}-fx.png")));
                if !ui.tap("FxPick(0, 0, 4)") {
                    faults.push(format!("{name}: no effect list"));
                }
                ui.render(&mut r);
                save(&r, &out.join(format!("{name}-fx-on.png")));
            } else {
                faults.push(format!("{name}: no effects bar"));
            }
            // Right-click the Near BPM chip: its editor must open and a choice must close it.
            if let Some((_, [x, y, w, h])) = ui.targets().into_iter().find(|(t, _)| t == "Filter(0)") {
                ui.context((x + w * 0.5) * scale, (y + h * 0.5) * scale);
                ui.render(&mut r);
                save(&r, &out.join(format!("{name}-chip.png")));
                if !ui.tap("ChipSet(0, 2)") {
                    faults.push(format!("{name}: no Near BPM chip editor"));
                }
                ui.tap("Filter(0)");
                ui.render(&mut r);
            }
            // Right-click a heading, hide BPM: its heading must go, and Reset must bring it back.
            if let Some((_, [x, y, w, h])) = ui.targets().into_iter().find(|(t, _)| t == "Sort(Title)") {
                // Columns persist, so compare with what was shown before rather than assume.
                let before = ui.targets().iter().any(|(t, _)| t == "Sort(Bpm)");
                ui.context((x + w * 0.5) * scale, (y + h * 0.5) * scale);
                ui.render(&mut r);
                save(&r, &out.join(format!("{name}-columns.png")));
                if !ui.tap("ToggleColumn(Bpm)") {
                    faults.push(format!("{name}: no column chooser"));
                }
                ui.render(&mut r);
                if ui.targets().iter().any(|(t, _)| t == "Sort(Bpm)") == before {
                    faults.push(format!("{name}: toggling the BPM column changed nothing"));
                }
                ui.key(Key::Escape, Mods::default());
                ui.render(&mut r);
                if let Some((_, [x, y, w, h])) = ui.targets().into_iter().find(|(t, _)| t == "Sort(Title)") {
                    ui.context((x + w * 0.5) * scale, (y + h * 0.5) * scale);
                    ui.render(&mut r);
                    if !ui.tap("ResetColumns") {
                        ui.tap("ToggleColumn(Bpm)");
                    }
                    ui.key(Key::Escape, Mods::default());
                    ui.render(&mut r);
                }
            }
        }
    }
    let table = format!("| size | pixels | ms per frame, software GPU |\n|---|---|---|\n{}\n", perf.join("\n"));
    print!("{table}");
    std::fs::write(out.join("perf.md"), table).expect("perf table");
    faults.sort();
    faults.dedup();
    for f in &faults {
        println!("::error::{f}");
    }
    println!("{} frames, {} faults", SIZES.len() * (VIEWS.len() + 1), faults.len());
    if !faults.is_empty() {
        std::process::exit(1);
    }
}

fn view_name(v: &str) -> &str {
    match v {
        "DJ" => "Dj",
        "WAVE" => "Wave",
        "LIB" => "Lib",
        "FX" => "Fx",
        "STREAM" => "Stream",
        _ => "Ctl",
    }
}

/// Tap the hub field, type, lose focus, come back and type more: the text must survive.
fn typing(ui: &mut Ui, r: &mut Renderer<'_>, name: &str, faults: &mut Vec<String>) {
    if !ui.tap("View(Stream)") {
        ui.key(Key::Char('5'), Mods { ctrl: true, alt: true, shift: false });
    }
    ui.render(r);
    if !ui.tap("HubFocus") {
        faults.push(format!("{name}: no hub search field"));
        return;
    }
    if !ui.typing() {
        faults.push(format!("{name}: tapping the search field does not start typing"));
    }
    ui.text("daft ");
    ui.release_all();
    ui.render(r);
    ui.tap("HubFocus");
    ui.text("punkx");
    ui.key(Key::Backspace, Mods::default());
    ui.render(r);
    if !ui.typing() {
        faults.push(format!("{name}: focus lost after a release"));
    }
    ui.tap("View(Dj)");
    ui.render(r);
    if ui.typing() {
        faults.push(format!("{name}: still typing after leaving the field"));
    }
    ui.tap("View(Stream)");
    ui.render(r);
    if ui.query() != "daft punk" {
        faults.push(format!("{name}: typed \"{}\", wanted \"daft punk\"", ui.query()));
    }
}

/// Holds a library track for its menu and opens the playlist submenu.
fn menu(ui: &mut Ui, r: &mut Renderer<'_>, (name, scale): (&str, f32), faults: &mut Vec<String>) {
    ui.tap("View(Lib)");
    // The typed shot left a search; clear it so every track shows.
    ui.key(Key::Escape, Mods::default());
    ui.key(Key::Escape, Mods::default());
    // The scan finds tracks off the main thread; give it time.
    for _ in 0..30 {
        ui.render(r);
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let Some((_, [x, y, w, _])) = ui.targets().into_iter().find(|(t, _)| t == "Library") else {
        faults.push(format!("{name}: no library to hold"));
        return;
    };
    // The first track's row.
    let (px, py) = ((x + w * 0.3) * scale, (y + 11.0) * scale);
    ui.pointer_down(7, px, py);
    std::thread::sleep(std::time::Duration::from_millis(700));
    ui.render(r);
    ui.pointer_up(7, py);
    ui.render(r);
    if !ui.tap("Submenu(1)") {
        faults.push(format!("{name}: no track menu"));
        return;
    }
    ui.render(r);
    ui.render(r);
    if !ui.targets().iter().any(|(t, _)| t == "Sort(Bpm)") {
        faults.push(format!("{name}: no Sort by submenu"));
    }
    // Tags: a new one from the track menu is named by typing, then listed in the sidebar.
    if ui.tap("Submenu(4)") {
        ui.render(r);
        if ui.tap("NewTag") {
            ui.render(r);
            for _ in 0..8 {
                ui.key(Key::Backspace, Mods::default());
            }
            ui.text("Peak");
            ui.render(r);
            ui.key(Key::Enter, Mods::default());
            ui.render(r);
            if ui.typing() {
                faults.push(format!("{name}: Enter did not end tag naming"));
            }
        } else {
            faults.push(format!("{name}: no New tag"));
        }
    } else {
        faults.push(format!("{name}: no Tags submenu"));
    }
}

/// The app menu with its View submenu, then the Decks settings page with a menu open.
fn settings(ui: &mut Ui, r: &mut Renderer<'_>, out: &Path, name: &str, faults: &mut Vec<String>) {
    ui.key(Key::Escape, Mods::default());
    ui.render(r);
    let steps: [(&str, &str); 9] = [
        ("AppMenu", ""),
        ("Submenu(1)", "appmenu"),
        ("Prefs(0)", "settings-audio"),
        ("Prefs(1)", ""),
        ("Pref(RateRange)", "settings"),
        ("PrefsClose", ""),
        ("Prefs(2)", "settings-mixer"),
        ("Prefs(3)", "settings-keyboard"),
        ("Prefs(4)", "settings-controllers"),
    ];
    for (tap, shot) in steps {
        if !ui.tap(tap) {
            faults.push(format!("{name}: no {tap}"));
            return;
        }
        ui.render(r);
        ui.render(r);
        if !shot.is_empty() {
            save(r, &out.join(format!("{name}-{shot}.png")));
        }
    }
    ui.tap("PrefsClose");
    ui.tap("PrefsClose");
    ui.render(r);
}
