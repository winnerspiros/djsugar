//! Loads a real VST3 bundle (the `djsugar-test-vst3` gain in
//! `crates/plugin-host/test-vst3`) through the helper.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use djsugar_plugin_host::{scan, spawn, Connection, Helper, Options, Outcome, Plugin, Transport};

const FRAMES: usize = 256;
const DEADLINE: Duration = Duration::from_millis(500);

fn helper() -> PathBuf {
    env!("CARGO_BIN_EXE_djsugar-plugin-host").into()
}

/// Builds the test plugin once and lays it out as a `.vst3` bundle.
fn bundle() -> PathBuf {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let target = helper().parent().unwrap().parent().unwrap().to_path_buf();
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let ok = std::process::Command::new(cargo)
            .args(["build", "-q", "-p", "djsugar-test-vst3", "--target-dir"])
            .arg(&target)
            .status()
            .expect("cargo runs")
            .success();
        assert!(ok, "the test plugin builds");
        let (built, inner) = if cfg!(windows) {
            ("djsugar_test_vst3.dll", "Contents/x86_64-win/DJSugarTestGain.vst3".to_string())
        } else if cfg!(target_os = "macos") {
            ("libdjsugar_test_vst3.dylib", "Contents/MacOS/DJSugarTestGain".to_string())
        } else {
            ("libdjsugar_test_vst3.so", format!("Contents/{}-linux/DJSugarTestGain.so", std::env::consts::ARCH))
        };
        let dir = target.join(format!("vst3-test-{}", std::process::id()));
        let bundle = dir.join("DJSugarTestGain.vst3");
        let lib = bundle.join(inner);
        std::fs::create_dir_all(lib.parent().unwrap()).unwrap();
        std::fs::copy(target.join("debug").join(built), &lib).unwrap();
        bundle
    })
    .clone()
}

fn id() -> String {
    let found = scan(&helper(), &bundle(), Duration::from_secs(10)).expect("scans");
    found[0].id.clone()
}

fn start() -> (Connection, Helper) {
    spawn(&Options {
        picks: Vec::new(),
        helper: helper(),
        plugin: Plugin::Vst3 { file: bundle(), id: id() },
        sample_rate: 48_000,
        max_frames: 1024,
        title: None,
        state: None,
    })
    .expect("the VST3 plugin loads")
}

fn run(conn: &mut Connection, transport: Transport) -> (Outcome, Vec<f32>, Vec<f32>) {
    let l: Vec<f32> = (0..FRAMES).map(|i| 0.1 + (i as f32 / 10.0).sin() * 0.5).collect();
    let r: Vec<f32> = l.iter().map(|x| -x).collect();
    let mut ol = vec![0.0; FRAMES];
    let mut or = vec![0.0; FRAMES];
    let out = conn.process([&l, &r], [&mut ol, &mut or], transport, DEADLINE);
    (out, ol, l)
}

#[test]
fn scanning_finds_the_effect() {
    let found = scan(&helper(), &bundle(), Duration::from_secs(10)).expect("scans");
    assert_eq!(found.len(), 1, "only the processor, not the controller");
    let d = &found[0];
    assert_eq!(d.id.len(), 32);
    assert_eq!(d.name, "DJ Sugar Test Gain");
    assert_eq!(d.vendor, "DJ Sugar");
    assert!(d.is_audio_effect(), "{:?}", d.features);
}

#[test]
fn the_plugin_describes_itself() {
    let (_conn, helper) = start();
    let info = helper.info();
    assert_eq!(info.name, "DJ Sugar Test Gain");
    assert_eq!(info.latency, 64);
    assert_eq!(info.params.len(), 2);
    assert_eq!(info.params[0].name, "Gain");
    assert_eq!((info.params[0].min, info.params[0].max), (0.0, 1.0));
    assert_eq!(info.params[0].value, 1.0);
}

#[test]
fn audio_and_parameters_go_through_the_plugin() {
    let (mut conn, _helper) = start();
    let (out, got, input) = run(&mut conn, Transport::default());
    assert_eq!(out, Outcome::Wet);
    assert_eq!(got, input);
    conn.set_param(0, 0.5);
    let (out, got, input) = run(&mut conn, Transport::default());
    assert_eq!(out, Outcome::Wet);
    assert!(got.iter().zip(&input).all(|(y, x)| (y - x * 0.5).abs() < 1e-6));
}

#[test]
fn the_deck_tempo_reaches_the_plugin() {
    let (mut conn, _helper) = start();
    conn.set_param(1, 1.0);
    let t = Transport { bpm: 126.0, beat: 3.0, playing: true };
    let (out, got, _) = run(&mut conn, t);
    assert_eq!(out, Outcome::Wet);
    assert_eq!(got[0], 126.0);
}

#[test]
fn a_missing_class_fails_to_start() {
    let err = spawn(&Options {
        picks: Vec::new(),
        helper: helper(),
        plugin: Plugin::Vst3 { file: bundle(), id: "00000000000000000000000000000000".into() },
        sample_rate: 48_000,
        max_frames: 1024,
        title: None,
        state: None,
    })
    .err()
    .expect("refused");
    assert!(err.to_string().contains("has no plugin"), "{err}");
}

#[test]
fn notes_reach_the_plugin() {
    use djsugar_plugin_host::Midi;
    let (mut conn, _helper) = start();
    conn.set_param(1, 1.0); // the test plugin writes the first note's key into sample 1
    let silent = vec![0.0f32; FRAMES];
    let mut ol = vec![0.0; FRAMES];
    let mut or = vec![0.0; FRAMES];
    let out = conn.process_events(
        [&silent, &silent],
        [&mut ol, &mut or],
        Transport::default(),
        &[Midi::note_on(10, 0, 60, 100), Midi::note_off(20, 0, 60)],
        DEADLINE,
    );
    assert_eq!(out, Outcome::Wet);
    assert_eq!(ol[1], 60.0);
    // No notes, no key.
    let out = conn.process_events([&silent, &silent], [&mut ol, &mut or], Transport::default(), &[], DEADLINE);
    assert_eq!(out, Outcome::Wet);
    assert_eq!(ol[1], -1.0);
}
