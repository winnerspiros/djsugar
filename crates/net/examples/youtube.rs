//! Live check: search YouTube and download the first result's audio.
//! cargo run -p sugar-net --example youtube -- "query" dir
fn main() {
    let mut args = std::env::args().skip(1);
    let q = args.next().unwrap_or_else(|| "daft punk around the world".into());
    let dir = std::path::PathBuf::from(args.next().unwrap_or_else(|| ".".into()));
    let yt = sugar_net::youtube::YouTube::new("US", "");
    let page = match yt.search(&q, None) {
        Ok(p) => p,
        Err(e) => return println!("search failed: {e}"),
    };
    println!("{} hits, more: {}", page.hits.len(), page.more.is_some());
    for h in page.hits.iter().take(3) {
        println!("  {} | {} - {} ({} s)", h.id, h.artist, h.title, h.seconds);
    }
    let t = std::time::Instant::now();
    match yt.fetch(&page.hits[0].id, &dir, &|_, _| {}) {
        Ok(f) => println!(
            "downloaded {} ({} bytes) in {:?}",
            f.display(),
            std::fs::metadata(&f).map_or(0, |m| m.len()),
            t.elapsed()
        ),
        Err(e) => println!("fetch failed: {e}; resting: {}", yt.resting()),
    }
}
