//! Live check: search each open service and fetch the first hit's audio.
//! cargo run -p djsugar-stream --example probe -- "query" /tmp/dir
use djsugar_stream::{audius, download, youtube};

fn main() {
    let mut args = std::env::args().skip(1);
    let q = args.next().unwrap_or_else(|| "lofi house".into());
    let dir = std::path::PathBuf::from(args.next().unwrap_or_else(|| ".".into()));
    match audius::search(&q) {
        Ok(h) if !h.is_empty() => {
            println!("audius {} hits, first: {} - {}", h.len(), h[0].artist, h[0].title);
            let to = dir.join("audius.mp3");
            match download(&audius::stream(&h[0].id), &to, |_, _| {}) {
                Ok(n) => println!("audius downloaded {n} bytes"),
                Err(e) => println!("audius download failed: {e}"),
            }
        }
        r => println!("audius: {r:?}"),
    }
    match youtube::search(&q) {
        Ok(h) if !h.is_empty() => {
            println!("youtube {} hits, first: {} - {} ({:?}s)", h.len(), h[0].artist, h[0].title, h[0].seconds);
            match youtube::stream(&h[0].id, &youtube::clients(None)) {
                Ok(s) => {
                    let to = dir.join(format!("youtube.{}", s.ext));
                    match download(&s, &to, |_, _| {}) {
                        Ok(n) => println!("youtube downloaded {n} bytes as {}", s.ext),
                        Err(e) => println!("youtube download failed: {e}"),
                    }
                }
                Err(e) => println!("youtube stream failed: {e}"),
            }
        }
        r => println!("youtube: {r:?}"),
    }
}
