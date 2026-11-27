//! Packs a model file's weights to 8 bits each: `cargo run -p djsugar-engine --example pack -- IN OUT`.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [from, to] = args.as_slice() else { return eprintln!("pack IN OUT") };
    let whole = std::fs::read(from).expect("the file to pack");
    let packed = djsugar_engine::packed::pack(&whole).expect("weights to pack");
    println!("{from}: {} -> {} bytes", whole.len(), packed.len());
    std::fs::write(to, packed).expect("the packed file");
}
