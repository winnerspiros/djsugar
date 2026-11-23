//! Loads a real CLAP binary (the `djsugar-test-clap` gain in
//! `crates/plugin-host/test-plugin`) through the helper.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use djsugar_plugin_host::{presets, scan, spawn, Options, Outcome, Plugin, ScanError, Transport};

const FRAMES: usize = 256;
const DEADLINE: Duration = Duration::from_millis(500);
const ID: &str = "io.github.winnerspiros.djsugar.test-gain";

fn helper() -> PathBuf {
    env!("CARGO_BIN_EXE_djsugar-plugin-host").into()
}

/// Builds the test plugin once, next to this test's own binaries.
fn test_plugin() -> PathBuf {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let target = helper().parent().unwrap().parent().unwrap().to_path_buf();
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let ok = std::process::Command::new(cargo)
            .args(["build", "-q", "-p", "djsugar-test-clap", "--target-dir"])
            .arg(&target)
            .status()
            .expect("cargo runs")
            .success();
        assert!(ok, "the test plugin builds");
        let name = if cfg!(windows) {
            "djsugar_test_clap.dll"
        } else if cfg!(target_os = "macos") {
            "libdjsugar_test_clap.dylib"
        } else {
            "libdjsugar_test_clap.so"
        };
        target.join("debug").join(name)
    })
    .clone()
}

fn start() -> (djsugar_plugin_host::Connection, djsugar_plugin_host::Helper) {
    start_from(None)
}

fn start_from(state: Option<Vec<u8>>) -> (djsugar_plugin_host::Connection, djsugar_plugin_host::Helper) {
    spawn(&Options {
        picks: Vec::new(),
        helper: helper(),
        plugin: Plugin::Clap { file: test_plugin(), id: ID.into() },
        sample_rate: 48_000,
        max_frames: 1024,
        title: None,
        state,
    })
    .expect("the CLAP plugin loads")
}

fn run(conn: &mut djsugar_plugin_host::Connection, transport: Transport) -> (Outcome, Vec<f32>, Vec<f32>) {
    let l: Vec<f32> = (0..FRAMES).map(|i| 0.1 + (i as f32 / 10.0).sin() * 0.5).collect();
    let r: Vec<f32> = l.iter().map(|x| -x).collect();
    let mut ol = vec![0.0; FRAMES];
    let mut or = vec![0.0; FRAMES];
    let out = conn.process([&l, &r], [&mut ol, &mut or], transport, DEADLINE);
    (out, ol, l)
}

#[test]
fn scanning_finds_the_effect() {
    let found = scan(&helper(), &test_plugin(), Duration::from_secs(10)).expect("scans");
    assert_eq!(found.len(), 1);
    let d = &found[0];
    assert_eq!(d.id, ID);
    assert_eq!(d.name, "DJ Sugar Test Gain");
    assert_eq!(d.vendor, "DJ Sugar");
    assert!(d.is_audio_effect(), "{:?}", d.features);
}

#[test]
fn scanning_a_broken_file_fails_cleanly() {
    let dir = std::env::temp_dir().join(format!("djsugar-scan-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let bad = dir.join("not-a-plugin.clap");
    std::fs::write(&bad, b"this is not a library").unwrap();
    let err = scan(&helper(), &bad, Duration::from_secs(10)).unwrap_err();
    assert!(matches!(err, ScanError::Failed(_)), "{err:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_knob_takes_the_parameter_picked_for_it() {
    let (_conn, helper) = spawn(&Options {
        helper: helper(),
        plugin: Plugin::Clap { file: test_plugin(), id: ID.into() },
        sample_rate: 48_000,
        max_frames: 1024,
        title: None,
        state: None,
        // Tone (id 3) on the first knob.
        picks: vec![(0, 3)],
    })
    .expect("the CLAP plugin loads");
    let info = helper.info();
    let names: Vec<&str> = info.params.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Tone", "Gain", "Tempo out"]);
    assert_eq!(info.params[0].id, 3);
    // Every parameter, to pick from, in the plugin's own order.
    let all: Vec<&str> = info.all.iter().map(|(_, n)| n.as_str()).collect();
    assert_eq!(all, ["Tone", "Gain", "Tempo out"]);
}

#[test]
fn the_plugin_describes_itself() {
    let (_conn, helper) = start();
    let info = helper.info();
    assert_eq!(info.name, "DJ Sugar Test Gain");
    assert_eq!(info.latency, 64);
    // In its remote controls page's order, not its own.
    assert_eq!(info.params.len(), 3);
    assert_eq!(info.params[0].name, "Gain");
    assert_eq!(info.params[2].name, "Tone");
    assert_eq!((info.params[0].min, info.params[0].max), (0.0, 2.0));
    assert_eq!(info.params[0].value, 1.0);
}

#[test]
fn audio_and_parameters_go_through_the_plugin() {
    let (mut conn, _helper) = start();
    // Gain starts at 1: the plugin passes the audio as it is.
    let (out, got, input) = run(&mut conn, Transport::default());
    assert_eq!(out, Outcome::Wet);
    assert_eq!(got, input);
    // Half gain, sent as a parameter event.
    conn.set_param(0, 0.5);
    let (out, got, input) = run(&mut conn, Transport::default());
    assert_eq!(out, Outcome::Wet);
    assert!(got.iter().zip(&input).all(|(y, x)| (y - x * 0.5).abs() < 1e-6));
}

#[test]
fn the_plugins_sound_is_saved_and_brought_back() {
    let (mut conn, helper) = start();
    conn.set_param(0, 0.5);
    run(&mut conn, Transport::default());
    let state = helper.save_state().expect("the plugin saves its state");
    assert_eq!(state, 0.5f64.to_bits().to_le_bytes());
    drop((conn, helper));
    // A new instance starts with the saved gain, without being told it.
    let (mut conn, helper) = start_from(Some(state));
    let half = |conn: &mut _| {
        let (out, got, input) = run(conn, Transport::default());
        assert_eq!(out, Outcome::Wet);
        got.iter().zip(&input).all(|(y, x)| (y - x * 0.5).abs() < 1e-6)
    };
    assert!(half(&mut conn), "starts at the saved gain");
    // And a sound loaded while it plays takes over.
    helper.load_state(&1.0f64.to_bits().to_le_bytes()).expect("the plugin loads a state");
    let (_, got, input) = run(&mut conn, Transport::default());
    assert_eq!(got, input);
}

#[test]
fn the_plugins_own_presets_are_listed_and_load() {
    let dir = std::env::temp_dir().join(format!("djsugar-clap-presets-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("Sub")).unwrap();
    std::fs::write(dir.join("Sub/Half.gain"), "0.5").unwrap();
    std::fs::write(dir.join("notes.txt"), "not a preset").unwrap();
    // Only this test's listing reads it.
    std::env::set_var("DJSUGAR_TEST_PRESETS", &dir);
    let plugin = Plugin::Clap { file: test_plugin(), id: ID.into() };
    let found = presets(&helper(), &plugin, Duration::from_secs(20)).expect("the presets are listed");
    let names: Vec<&str> = found.iter().map(|(n, _)| n.as_str()).collect();
    // Another plugin's preset is left out; a file's is named after it.
    assert_eq!(names, ["Loud", "Quiet", "Half"]);
    let (mut conn, helper) = start();
    let gain = |helper: &djsugar_plugin_host::Helper| {
        let state = helper.save_state().expect("the plugin saves its state");
        f64::from_bits(u64::from_le_bytes(state.try_into().unwrap()))
    };
    helper.load_preset(&found[0].1).expect("a preset in the plugin loads");
    assert_eq!(gain(&helper), 1.5);
    helper.load_preset(&found[2].1).expect("a preset file loads");
    assert_eq!(gain(&helper), 0.5);
    assert!(helper.load_preset("nothing").is_err());
    run(&mut conn, Transport::default());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_deck_tempo_reaches_the_plugin() {
    let (mut conn, _helper) = start();
    conn.set_param(1, 1.0); // the test plugin writes the tempo it sees into sample 0
    let t = Transport { bpm: 124.0, beat: 17.5, playing: true };
    let (out, got, _) = run(&mut conn, t);
    assert_eq!(out, Outcome::Wet);
    assert_eq!(got[0], 124.0);
}

#[test]
fn a_missing_plugin_id_fails_to_start() {
    let err = spawn(&Options {
        picks: Vec::new(),
        helper: helper(),
        plugin: Plugin::Clap { file: test_plugin(), id: "no.such.plugin".into() },
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
