fn main() {
    for p in std::env::args().skip(1) {
        match djsugar_engine::decode::decode(std::path::Path::new(&p)) {
            Ok(a) => println!("{p}: {:.1} s", a.frames() as f64 / f64::from(a.rate())),
            Err(e) => println!("{p}: {e}"),
        }
    }
}
