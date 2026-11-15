//! Streams a tone to an Icecast server, for trying broadcasting by hand:
//!
//! `cargo run -p djsugar-broadcast --example stream_tone -- HOST PORT MOUNT PASSWORD [opus|mp3] [SECONDS]`

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use djsugar_broadcast::{Broadcaster, Format, Settings, Source, Station, Target};
use djsugar_control::ControlRegistry;

struct Tone {
    at: u64,
    since: Instant,
    active: Arc<AtomicBool>,
}

const RATE: u32 = 44_100;

impl Source for Tone {
    fn drain_into(&mut self, out: &mut Vec<f32>) {
        if !self.active.load(Ordering::Acquire) {
            return;
        }
        let due = (self.since.elapsed().as_secs_f64() * RATE as f64) as u64;
        while self.at < due {
            let t = self.at as f64 / RATE as f64;
            let v = (0.3 * (std::f64::consts::TAU * 440.0 * t).sin()) as f32;
            out.extend([v, v]);
            self.at += 1;
        }
    }

    fn set_active(&self, on: bool) {
        self.active.store(on, Ordering::Release);
    }
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() < 4 {
        eprintln!("usage: stream_tone HOST PORT MOUNT PASSWORD [opus|mp3] [SECONDS]");
        std::process::exit(2);
    }
    let format = if a.get(4).map(String::as_str) == Some("mp3") {
        Format::Mp3
    } else {
        Format::Opus
    };
    let seconds: u64 = a.get(5).and_then(|s| s.parse().ok()).unwrap_or(10);
    let reg = ControlRegistry::new();
    let started = Instant::now();
    let source = Tone {
        at: 0,
        since: Instant::now(),
        active: Arc::default(),
    };
    let now_playing = Arc::new(move || {
        // A new "track" every five seconds.
        let n = started.elapsed().as_secs() / 5 + 1;
        Some(("DJ Sugar".to_string(), format!("Test tone {n}")))
    });
    let b = Broadcaster::new(&reg, RATE, Box::new(source), now_playing);
    b.configure(Settings {
        targets: vec![Target {
            station: Station {
                id: "example".into(),
                name: "Example".into(),
                host: a[0].clone(),
                port: a[1].parse().expect("port"),
                mount: a[2].clone(),
                format,
                stream_name: "DJ Sugar test".into(),
                ..Station::default()
            },
            password: a[3].clone(),
        }],
        ..Settings::default()
    });
    b.start().expect("on air");
    let end = Instant::now() + Duration::from_secs(seconds);
    while Instant::now() < end {
        std::thread::sleep(Duration::from_secs(1));
        for s in b.statuses() {
            println!("{:?} {} listeners={:?}", s.state, s.message, s.listeners);
        }
    }
    b.stop();
}
