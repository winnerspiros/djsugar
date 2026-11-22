//! Compares screenshots of the reference app with ours, screen by screen.
//! Usage: twin <dir>; pairs are <name>.ref.png and <name>.ours.png. Writes
//! <name>.cmp.png (reference, ours, heat map) and prints a score table, worst first.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

struct Img {
    w: usize,
    h: usize,
    px: Vec<[u8; 3]>,
}

fn load(p: &Path) -> Option<Img> {
    let dec = png::Decoder::new(std::io::BufReader::new(File::open(p).ok()?));
    let mut r = dec.read_info().ok()?;
    let mut buf = vec![0; r.output_buffer_size()?];
    let info = r.next_frame(&mut buf).ok()?;
    let n = info.color_type.samples();
    let px = buf[..info.buffer_size()].chunks(n).map(|c| if n >= 3 { [c[0], c[1], c[2]] } else { [c[0]; 3] }).collect();
    Some(Img { w: info.width as usize, h: info.height as usize, px })
}

/// Nearest sample at the same relative position, so different sizes still compare.
fn at(i: &Img, x: usize, y: usize, w: usize, h: usize) -> [u8; 3] {
    i.px[(y * i.h / h).min(i.h - 1) * i.w + (x * i.w / w).min(i.w - 1)]
}

/// Mean colour difference in percent, and the side by side image with the heat map.
fn compare(a: &Img, b: &Img) -> (f64, Vec<u8>, usize, usize) {
    let (w, h) = (a.w.min(640), a.h * a.w.min(640) / a.w);
    let mut out = vec![0u8; w * 3 * h * 3];
    let mut sum = 0u64;
    for y in 0..h {
        for x in 0..w {
            let (p, q) = (at(a, x, y, w, h), at(b, x, y, w, h));
            let d = (0..3).map(|c| u32::from(p[c].abs_diff(q[c]))).sum::<u32>() / 3;
            sum += u64::from(d);
            let heat = [d.min(255) as u8, (d * 2).min(255) as u8 / 3, 0];
            for (k, c) in [p, q, heat].into_iter().enumerate() {
                let o = (y * w * 3 + k * w + x) * 3;
                out[o..o + 3].copy_from_slice(&c);
            }
        }
    }
    (sum as f64 / (w * h) as f64 / 2.55, out, w * 3, h)
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "twin".into());
    let dir = Path::new(&dir);
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_str()?.strip_suffix(".ref.png").map(str::to_owned))
        .collect();
    names.sort();
    let mut rows = Vec::new();
    for n in &names {
        let (Some(a), Some(b)) = (load(&dir.join(format!("{n}.ref.png"))), load(&dir.join(format!("{n}.ours.png"))))
        else {
            rows.push((100.0, format!("{n}: missing on our side")));
            continue;
        };
        let (score, px, w, h) = compare(&a, &b);
        let f = BufWriter::new(File::create(dir.join(format!("{n}.cmp.png"))).expect("write"));
        let mut e = png::Encoder::new(f, w as u32, h as u32);
        e.set_color(png::ColorType::Rgb);
        e.write_header().and_then(|mut wr| wr.write_image_data(&px)).expect("encode");
        rows.push((score, format!("{n}: {score:.1}% different")));
    }
    rows.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mean = rows.iter().map(|r| r.0).sum::<f64>() / rows.len().max(1) as f64;
    for (_, r) in &rows {
        println!("{r}");
    }
    println!("{} screens, mean {mean:.1}% different", rows.len());
}
