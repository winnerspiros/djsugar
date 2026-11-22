//! Renders every view at phone, tablet, DeX and desktop sizes, drives typing and
//! focus loss, checks each control is on screen and big enough to touch, and
//! writes the frames as PNG to the directory given, for CI to keep.

use std::path::{Path, PathBuf};

use djsugar::{Key, Ui, read_dir_sample};
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

fn main() {
    let out = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "target/shots".into()));
    std::fs::create_dir_all(&out).expect("output dir");
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
            if !ui.tap(&tab) {
                faults.push(format!("{name}: no {view} tab"));
                continue;
            }
            ui.render(&mut r);
            ui.render(&mut r);
            let shown = ui.targets();
            for want in TOOLBAR {
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
        }
        let before = pixels(&r);
        r = Renderer::offscreen(w, h).expect("second renderer");
        ui.render(&mut r);
        if pixels(&r) != before {
            faults.push(format!("{name}: frame changes when the GPU is recreated, as after leaving the app"));
        }
        typing(&mut ui, &mut r, name, &mut faults);
        save(&r, &out.join(format!("{name}-typed.png")));
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
    ui.tap("View(Stream)");
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
    ui.key(Key::Backspace);
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
