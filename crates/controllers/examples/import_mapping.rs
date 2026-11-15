//! Imports mapping files from other DJ apps and prints what came across:
//! `cargo run -p djsugar-controllers --example import_mapping -- FILE...`
//! With `--write DIR` each mapping is also saved as `.djsugar-mapping`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn main() {
    let mut out: Option<PathBuf> = None;
    let mut files = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--write" {
            out = args.next().map(PathBuf::from);
        } else {
            files.push(a);
        }
    }
    for f in files {
        println!("== {f}");
        match djsugar_controllers::import::import_file(Path::new(&f)) {
            Err(e) => println!("  {e}"),
            Ok(i) => {
                println!("  {}", i.summary());
                for (k, s) in &i.skipped_shortcuts {
                    println!("  key {k} — {s}");
                }
                for n in &i.notes {
                    println!("  note: {n}");
                }
                for m in &i.mappings {
                    println!(
                        "  {}: {} rows, {} lights",
                        m.name,
                        m.rows.len(),
                        m.lights.len()
                    );
                    let mut why: BTreeMap<String, usize> = BTreeMap::new();
                    for r in m.rows.iter().filter(|r| r.off) {
                        *why.entry(format!("{} — {}", r.name, r.note)).or_default() += 1;
                    }
                    for l in m.lights.iter().filter(|l| l.off) {
                        *why.entry(format!("light {} — {}", l.name, l.note))
                            .or_default() += 1;
                    }
                    for (w, n) in why {
                        println!("    {n:>4}× {w}");
                    }
                    if let Some(dir) = &out {
                        let path =
                            dir.join(format!("{}.djsugar-mapping", m.name.replace('/', "-")));
                        m.write(&path).expect("write");
                    }
                }
            }
        }
    }
}
