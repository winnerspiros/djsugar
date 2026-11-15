//! The Controller Editor's view of three common controllers: script-driven
//! controls get their meaning from the sandbox probe.

use djsugar_control::ControlRegistry;
use djsugar_controllers::default_mappings_dir;
use djsugar_controllers::view::{build, MappingView};

fn view(file: &str) -> MappingView {
    let reg = ControlRegistry::new();
    let _engine = djsugar_engine::engine::Engine::new(&reg, 44_100.0);
    build(&default_mappings_dir().unwrap().join(file), reg).unwrap()
}

fn dump(v: &MappingView) {
    if std::env::var_os("DUMP").is_none() {
        return;
    }
    for r in &v.rows {
        eprintln!(
            "{:14} | {:28} | {:10} | {:22} | {:16} | {}",
            r.function.category, r.function.name, r.function.target, r.control, r.layer, r.midi
        );
    }
    for l in &v.lights {
        eprintln!(
            "LIGHT {:14} | {:28} | {:10} | {:22} | {}",
            l.function.category, l.function.name, l.function.target, l.control, l.midi
        );
    }
    eprintln!("notes: {:?}", v.notes);
}

fn has(v: &MappingView, name: &str, target: &str, control: &str, layer: &str) -> bool {
    v.rows.iter().any(|r| {
        r.function.name == name
            && r.function.target == target
            && r.control == control
            && r.layer == layer
    })
}

#[test]
fn ddj_flx4() {
    let v = view("Pioneer-DDJ-FLX4.midi.xml");
    dump(&v);
    assert!(has(&v, "Play / pause", "Deck 1", "PLAY/PAUSE", ""));
    assert!(has(&v, "Hot cue 2", "Deck 1", "PAD 2", "Hot cue"));
    assert!(has(
        &v,
        "Clear hot cue 2",
        "Deck 2",
        "PAD 2",
        "Hot cue + SHIFT"
    ));
    // Script bindings, found by trying them.
    assert!(has(&v, "Tempo", "Deck 1", "TEMPO", ""));
    assert!(has(&v, "Jump 4 beats back", "Deck 1", "PAD 1", "Beat jump"));
    assert!(v
        .rows
        .iter()
        .any(|r| r.function.name == "Channel fader" && r.midi == "CC 13, 33 · ch 1"));
    assert!(v
        .lights
        .iter()
        .any(|l| l.function.name == "Play / pause" || l.control == "PLAY/PAUSE"));
}

#[test]
fn xone_k2() {
    let v = view("Allen and Heath Xone K2.midi.xml");
    dump(&v);
    // Only the device's channel (15) is listed.
    assert!(v.rows.iter().all(|r| r.midi.ends_with("ch 15")));
    assert!(v
        .rows
        .iter()
        .any(|r| r.function.category == "Mixer" && r.control.starts_with("FADER")));
    assert!(
        v.rows.iter().any(|r| r.layer.contains("SHIFT")),
        "SHIFT layer found"
    );
}

#[test]
fn midi_fighter_twister() {
    let v = view("DJ TechTools MIDI Fighter Twister.midi.xml");
    dump(&v);
    assert!(has(&v, "High EQ", "Deck 1", "ENCODER 2", "Bank 1"));
    assert!(has(&v, "Gain", "Deck 2", "ENCODER 4", "Bank 1"));
    assert_eq!(v.options.len(), 11);
    assert!(v
        .options
        .iter()
        .any(|o| o.label == "Use gain knob as vu meter"));
}

/// The DDJ-SB3 sends its own messages while SHIFT is held, and its script
/// also keeps track of SHIFT: each message shows what it does in the layer
/// the controller sends it in, never "SHIFT + SHIFT".
#[test]
fn ddj_sb3_shift_layers() {
    let v = view("Pioneer-DDJ-SB3.midi.xml");
    dump(&v);
    assert!(v.rows.iter().all(|r| !r.layer.contains("SHIFT + SHIFT")));
    assert!(has(&v, "Play / pause", "Deck 1", "PLAY/PAUSE", ""));
    assert!(!has(&v, "Play / pause", "Deck 1", "PLAY/PAUSE", "SHIFT"));
    assert!(has(&v, "Key lock", "Deck 1", "KEY LOCK", ""));
}

/// Every popular controller's mapping reads with a picture and no
/// script errors.
#[test]
fn looked_after_mappings_read_cleanly() {
    for c in djsugar_controllers::support::all() {
        let Some(file) = c.file else { continue };
        // HID mappings are all script: the editor doesn't show them.
        if file.ends_with(".hid.xml") {
            continue;
        }
        let v = view(&file);
        assert!(!v.rows.is_empty(), "{file}");
        let failed: Vec<&String> = v.notes.iter().filter(|n| n.contains("failed")).collect();
        assert!(failed.is_empty(), "{file}: {failed:?}");
    }
}
