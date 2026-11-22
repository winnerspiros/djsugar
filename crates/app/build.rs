use std::path::Path;

fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, base, out);
        } else if p.extension().is_some_and(|x| x == "ogg" || x == "wav") {
            out.push(p.strip_prefix(base).unwrap_or(&p).to_string_lossy().replace('\\', "/"));
        }
    }
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../res/samples");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut list = Vec::new();
    walk(&root, &root, &mut list);
    list.sort_by_key(|p| (!p.starts_with("djsugar/"), p.clone()));
    let body: String = list.iter().map(|p| format!("{p:?},")).collect();
    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("samples.rs");
    std::fs::write(out, format!("pub const SAMPLES: &[&str] = &[{body}];")).unwrap();
}
