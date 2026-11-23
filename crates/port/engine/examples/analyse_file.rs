//! Prints the tempo and beat grid DJ Sugar finds for audio files: `cargo run
//! `--neural`, Beat This! refines the grid too (models from `DJSUGAR_MODELS` or downloaded).
//! `--neural`, Beat This! refines the grid too (models from `DJSUGAR_MODELS` or

use std::path::Path;

use djsugar_engine::{analysis, beatthis, decode};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let neural = args.iter().any(|a| a == "--neural");
    for path in args.iter().filter(|a| !a.starts_with("--")) {
        let audio = match decode::decode(Path::new(path)) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("{e}");
                continue;
            }
        };
        let mut a = analysis::analyse(&audio, None);
        println!(
            "{path}\n  {:.1} s at {} Hz\n  DSP: {:.2} BPM, first beat {:.3} s, dynamic {}, unsure {}",
            audio.duration(),
            audio.sample_rate,
            a.bpm,
            a.first_beat,
            a.grid.is_dynamic(),
            a.grid_unsure
        );
        if neural {
            match beatthis::refine(&mut a, &audio, None) {
                Ok(()) => println!(
                    "  Beat This!: {:.2} BPM, first beat {:.3} s, dynamic {}, unsure {}",
                    a.bpm,
                    a.first_beat,
                    a.grid.is_dynamic(),
                    a.grid_unsure
                ),
                Err(e) => println!("  Beat This!: {e}"),
            }
        }
        for m in a.grid.markers() {
            println!("    marker {:.3} s at {:.3} BPM", m.seconds, m.bpm);
        }
    }
}
