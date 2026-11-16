//! Times DJ Sugar's stem separation on this machine.
//!
//!     cargo run --release -p stem-bench -- TRACK [options]
//!
//! Options:
//!   --weights DIR   where spleeter-4stems.safetensors and htdemucs.safetensors
//!                   are (default: models/)
//!   --spleeter / --htdemucs   run only that model (default: both)
//!   --gpu / --cpu   run only on that backend (default: both)
//!   --from SECONDS  where Spleeter starts, as if cued there (default: 0)
//!   --seconds N     use only the first N seconds of the track
//!   --out DIR       write the stems as WAV files
//!   --compare DIR   score Spleeter's stems against DIR/<stem>.wav (SDR, dB)

use std::path::{Path, PathBuf};
use std::time::Instant;

use djsugar_separation::burn::tensor::Device;
use djsugar_separation::{htdemucs, spleeter, SAMPLE_RATE};

struct Options {
    track: PathBuf,
    weights: PathBuf,
    spleeter: bool,
    htdemucs: bool,
    gpu: bool,
    cpu: bool,
    from: f64,
    seconds: Option<f64>,
    out: Option<PathBuf>,
    compare: Option<PathBuf>,
}

fn options() -> Result<Options, String> {
    let mut args = std::env::args().skip(1);
    let mut o = Options {
        track: PathBuf::new(),
        weights: PathBuf::from("models"),
        spleeter: false,
        htdemucs: false,
        gpu: false,
        cpu: false,
        from: 0.0,
        seconds: None,
        out: None,
        compare: None,
    };
    let value = |args: &mut dyn Iterator<Item = String>, name: &str| {
        args.next().ok_or(format!("{name} needs a value"))
    };
    while let Some(a) = args.next() {
        match a.as_str() {
            "--weights" => o.weights = value(&mut args, &a)?.into(),
            "--spleeter" => o.spleeter = true,
            "--htdemucs" => o.htdemucs = true,
            "--gpu" => o.gpu = true,
            "--cpu" => o.cpu = true,
            "--from" => o.from = value(&mut args, &a)?.parse().map_err(|_| "bad --from")?,
            "--seconds" => {
                o.seconds = Some(value(&mut args, &a)?.parse().map_err(|_| "bad --seconds")?)
            }
            "--out" => o.out = Some(value(&mut args, &a)?.into()),
            "--compare" => o.compare = Some(value(&mut args, &a)?.into()),
            _ if a.starts_with("--") => return Err(format!("unknown option {a}")),
            _ => o.track = a.into(),
        }
    }
    if o.track.as_os_str().is_empty() {
        return Err("usage: stem-bench TRACK [--weights DIR] [--spleeter] [--htdemucs] [--gpu] [--cpu] [--from S] [--seconds N] [--out DIR] [--compare DIR]".into());
    }
    if !o.spleeter && !o.htdemucs {
        o.spleeter = true;
        o.htdemucs = true;
    }
    if !o.gpu && !o.cpu {
        o.gpu = true;
        o.cpu = true;
    }
    Ok(o)
}

fn read_weights(dir: &Path, name: &str, how: &str) -> Result<Vec<u8>, String> {
    let path = dir.join(name);
    std::fs::read(&path).map_err(|e| format!("{}: {e}\n  get it with: {how}", path.display()))
}

fn write_stems(dir: &Path, prefix: &str, names: &[&str], stems: &[[Vec<f32>; 2]]) {
    std::fs::create_dir_all(dir).expect("output folder");
    for (name, [l, r]) in names.iter().zip(stems) {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: SAMPLE_RATE,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let path = dir.join(format!("{prefix}-{name}.wav"));
        let mut w = hound::WavWriter::create(&path, spec).expect("WAV file");
        for (a, b) in l.iter().zip(r) {
            w.write_sample(*a).unwrap();
            w.write_sample(*b).unwrap();
        }
        w.finalize().unwrap();
    }
}

/// Signal-to-distortion ratio of `estimate` against `reference`, in dB.
fn sdr(reference: &[f32], estimate: &[f32]) -> f64 {
    let (mut s, mut e) = (0.0f64, 0.0f64);
    for (r, x) in reference.iter().zip(estimate) {
        s += (*r as f64).powi(2);
        e += (*r as f64 - *x as f64).powi(2);
    }
    10.0 * (s / e.max(1e-20)).log10()
}

fn read_wav(path: &Path) -> Result<[Vec<f32>; 2], String> {
    let mut r = hound::WavReader::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let samples: Vec<f32> = match r.spec().sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().map(Result::unwrap).collect(),
        hound::SampleFormat::Int => {
            let scale = (1u64 << (r.spec().bits_per_sample - 1)) as f32;
            r.samples::<i32>()
                .map(|s| s.unwrap() as f32 / scale)
                .collect()
        }
    };
    Ok([
        samples.iter().step_by(2).copied().collect(),
        samples.iter().skip(1).step_by(2).copied().collect(),
    ])
}

fn spleeter_run(
    label: &str,
    model: &impl spleeter::Net,
    audio: [&[f32]; 2],
    o: &Options,
) -> Result<(), String> {
    let from = (o.from * SAMPLE_RATE as f64) as usize;
    let length = audio[0].len() as f64 / SAMPLE_RATE as f64;
    // The first run includes compiling GPU kernels; the second is what a
    // DJ sees on every load after the first.
    for run in ["first run", "second run"] {
        let t = Instant::now();
        let mut first = None;
        let stems = spleeter::separate(model, audio, from, |n, _| {
            if n == 0 {
                first = Some(t.elapsed().as_secs_f64());
            }
        });
        let total = t.elapsed().as_secs_f64();
        println!(
            "  {label} {run}: first window (~12 s of audio) {:.2} s, whole track {:.2} s ({:.1}x real time)",
            first.unwrap_or(total),
            total,
            length / total
        );
        if run == "second run" {
            if let Some(dir) = &o.out {
                write_stems(dir, &format!("spleeter-{label}"), &spleeter::STEMS, &stems);
            }
            if let Some(dir) = &o.compare {
                for (name, [l, r]) in spleeter::STEMS.iter().zip(&stems) {
                    let [rl, rr] = read_wav(&dir.join(format!("{name}.wav")))?;
                    let n = rl.len().min(l.len());
                    let score = (sdr(&rl[..n], &l[..n]) + sdr(&rr[..n], &r[..n])) / 2.0;
                    println!("    {name}: {score:.1} dB against {}", dir.display());
                }
            }
        }
    }
    Ok(())
}

fn htdemucs_run(
    label: &str,
    weights: &[u8],
    device: &Device,
    audio: [&[f32]; 2],
    o: &Options,
) -> Result<(), String> {
    let t = Instant::now();
    let model = htdemucs::HtDemucs::load(weights, device)?;
    println!("  {label}: loaded in {:.2} s", t.elapsed().as_secs_f64());
    let t = Instant::now();
    model.warm_up();
    println!("  {label}: warmed up in {:.2} s", t.elapsed().as_secs_f64());
    let length = audio[0].len() as f64 / SAMPLE_RATE as f64;
    let t = Instant::now();
    let stems = model.separate(audio)?;
    let total = t.elapsed().as_secs_f64();
    println!(
        "  {label}: whole track {total:.1} s ({:.1}x real time)",
        length / total
    );
    if let Some(dir) = &o.out {
        write_stems(dir, &format!("htdemucs-{label}"), &htdemucs::STEMS, &stems);
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("stem-bench: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let o = options()?;
    let t = Instant::now();
    let audio = djsugar_engine::decode::decode(&o.track)?;
    let mut channels: [Vec<f32>; 2] = [Vec::new(), Vec::new()];
    for i in 0..audio.len() {
        let [l, r] = audio.frame(i);
        channels[0].push(l);
        channels[1].push(r);
    }
    if audio.sample_rate != SAMPLE_RATE {
        for c in &mut channels {
            *c = djsugar_separation::resample(c, audio.sample_rate, SAMPLE_RATE)?;
        }
    }
    if let Some(s) = o.seconds {
        let n = (s * SAMPLE_RATE as f64) as usize;
        for c in &mut channels {
            c.truncate(n);
        }
    }
    let length = channels[0].len() as f64 / SAMPLE_RATE as f64;
    println!(
        "{}: {length:.1} s of audio, decoded in {:.2} s",
        o.track.display(),
        t.elapsed().as_secs_f64()
    );
    let audio = [&channels[0][..], &channels[1][..]];
    let gpu = djsugar_separation::gpu();
    let cpu = djsugar_separation::cpu();

    if o.spleeter {
        println!("Spleeter (instant stems):");
        let w = read_weights(
            &o.weights,
            "spleeter-4stems.safetensors",
            "scripts/models/convert_spleeter.py",
        )?;
        if o.gpu {
            let t = Instant::now();
            let model = spleeter::gpu::Net::load(&w, &gpu)?;
            println!("  gpu: loaded in {:.2} s", t.elapsed().as_secs_f64());
            spleeter_run("gpu", &model, audio, &o)?;
        }
        if o.cpu {
            let t = Instant::now();
            let model = spleeter::cpu::Net::load(&w)?;
            println!("  cpu: loaded in {:.2} s", t.elapsed().as_secs_f64());
            spleeter_run("cpu", &model, audio, &o)?;
        }
    }
    if o.htdemucs {
        println!("HTDemucs (high quality):");
        let w = read_weights(
            &o.weights,
            "htdemucs.safetensors",
            &format!(
                "curl -L -o htdemucs.safetensors {}",
                htdemucs::download_url(&htdemucs::HTDEMUCS)
            ),
        )?;
        if o.gpu {
            htdemucs_run("gpu", &w, &gpu, audio, &o)?;
        }
        if o.cpu {
            htdemucs_run("cpu", &w, &cpu, audio, &o)?;
        }
    }
    Ok(())
}
