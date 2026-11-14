//! The Mixer strips' input picker (`MixerStrip` in `ui/deck.slint`): the
//! ▼ on a strip, or + for a new one, lists its interface's channel pairs
//! then its single channels, each with a live meter, so the input that
//! moves is the one to pick. Picking saves the strip's route and opens the
//! inputs again, as Settings › Audio does.
//!
//! The popup's own timer asks for fresh levels (`InstrumentsUi.picker-tick`)
//! while it is open; once those stop, the interface is no longer metered.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use opendj_engine::inputs::GROOVE_BOX;
use opendj_engine::{
    channel_names, input_devices, AudioEngine, AudioSetup, InputRole, InputRoute, Role,
};
use slint::{ComponentHandle, Model, ModelRc, VecModel};

use crate::prefs::channel_choices;
use crate::{instruments, settings, AppWindow, InstrumentsUi, PickerRow};

/// How long after the popup's last tick the interface stays metered.
const IDLE: Duration = Duration::from_secs(1);
/// How fast the meters fall, in meter units a second (the decks' rate).
const FALL: f32 = 1.5;

/// The picker as last opened.
#[derive(Default)]
struct Picker {
    /// The strip, from 1: `[AuxiliaryN]`.
    strip: usize,
    /// The interface listed, as [`InputRoute::device`] names it.
    device: String,
    /// Each choice's first channel and whether it is mono.
    choices: Vec<(u16, bool)>,
    /// Each channel's meter, 0 to 1, falling between ticks.
    meters: Vec<f32>,
    last_tick: Option<Instant>,
    /// The engine is metering `device` for the picker.
    watching: bool,
}

pub fn install(app: &AppWindow, engine: Rc<AudioEngine>) {
    let picker = Rc::new(RefCell::new(Picker::default()));
    let ui = app.global::<InstrumentsUi>();
    {
        let (picker, engine, weak) = (picker.clone(), engine.clone(), app.as_weak());
        ui.on_open_picker(move |strip| {
            let Some(app) = weak.upgrade() else { return };
            // + also offers the instrument plugin and the mic.
            app.global::<InstrumentsUi>().set_picker_adding(strip == 0);
            let setup = settings::audio_setup();
            let strip = match usize::try_from(strip) {
                Ok(n) if (1..=InputRole::AUXILIARIES.len()).contains(&n) => n,
                _ => match next_free(&setup) {
                    Some(n) => n,
                    None => return,
                },
            };
            let device = device_for(&setup, strip, &input_names());
            let channels = input_devices()
                .iter()
                .find(|d| d.name == device)
                .map_or(0, |d| d.channels);
            let mut p = picker.borrow_mut();
            *p = Picker {
                strip,
                choices: channel_choices(InputRole::AUXILIARIES[strip - 1], channels),
                meters: vec![0.0; channels as usize],
                last_tick: Some(Instant::now()),
                watching: !device.is_empty() || channels > 0,
                device,
            };
            if p.watching {
                engine.watch_input(Some(&p.device));
            }
            show(&app, &p, &setup, &engine.status(), delay(&engine, p.strip));
        });
    }
    {
        let (picker, engine, weak) = (picker.clone(), engine.clone(), app.as_weak());
        ui.on_picker_tick(move || {
            let Some(app) = weak.upgrade() else { return };
            let mut p = picker.borrow_mut();
            p.last_tick = Some(Instant::now());
            let levels = engine.input_levels();
            let peaks = levels
                .iter()
                .find(|(name, _)| *name == p.device)
                .map(|(_, peaks)| peaks.as_slice())
                .unwrap_or_default();
            let fall = FALL * 0.05;
            for (c, m) in p.meters.iter_mut().enumerate() {
                let now = peaks.get(c).copied().map_or(0.0, meter);
                *m = now.max(*m - fall);
            }
            show(
                &app,
                &p,
                &settings::audio_setup(),
                &engine.status(),
                delay(&engine, p.strip),
            );
        });
    }
    {
        let (picker, engine, weak) = (picker.clone(), engine.clone(), app.as_weak());
        ui.on_pick(move |index| {
            let mut setup = settings::audio_setup();
            let p = picker.borrow();
            let Some(&(channel, mono)) = usize::try_from(index).ok().and_then(|i| p.choices.get(i))
            else {
                return;
            };
            let role = InputRole::AUXILIARIES[p.strip - 1];
            setup.inputs.retain(|r| r.role != role);
            setup.inputs.push(InputRoute {
                role,
                device: p.device.clone(),
                channel,
                mono,
            });
            settings::set_audio_setup(&setup);
            engine.set_inputs(&setup);
            if let Some(app) = weak.upgrade() {
                instruments::show_routes(&app, &engine.input_status(), &engine.status());
            }
        });
    }
    {
        let (picker, engine, weak) = (picker.clone(), engine.clone(), app.as_weak());
        ui.on_pick_groove(move || {
            let mut setup = settings::audio_setup();
            let p = picker.borrow();
            if !(1..=InputRole::AUXILIARIES.len()).contains(&p.strip) {
                return;
            }
            set_groove(&mut setup, p.strip);
            settings::set_audio_setup(&setup);
            engine.set_inputs(&setup);
            if let Some(app) = weak.upgrade() {
                instruments::show_routes(&app, &engine.input_status(), &engine.status());
            }
        });
    }
    {
        let (engine, weak) = (engine.clone(), app.as_weak());
        ui.on_rename_strip(move |number, name| {
            let Some(n) = usize::try_from(number).ok().filter(|n| (1..=8).contains(n)) else {
                return;
            };
            settings::set_mixer_name(n - 1, &name);
            if let Some(app) = weak.upgrade() {
                instruments::show_routes(&app, &engine.input_status(), &engine.status());
            }
        });
    }
    {
        let (engine, weak) = (engine.clone(), app.as_weak());
        ui.on_color_strip(move |number, color| {
            let Some(n) = usize::try_from(number).ok().filter(|n| (1..=8).contains(n)) else {
                return;
            };
            settings::set_mixer_color(n - 1, color);
            if let Some(app) = weak.upgrade() {
                instruments::show_routes(&app, &engine.input_status(), &engine.status());
            }
        });
    }
    {
        let (engine, weak) = (engine.clone(), app.as_weak());
        ui.on_delete_strip(move |number| {
            let Some(n) = usize::try_from(number).ok().filter(|n| (1..=8).contains(n)) else {
                return;
            };
            let mut setup = settings::audio_setup();
            delete(&mut setup, n);
            settings::set_audio_setup(&setup);
            settings::set_mixer_name(n - 1, "");
            settings::set_mixer_color(n - 1, -1);
            engine.set_inputs(&setup);
            if let Some(app) = weak.upgrade() {
                instruments::show_routes(&app, &engine.input_status(), &engine.status());
            }
        });
    }
    // The popup closed: stop metering the interface for it.
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(500),
        move || {
            let mut p = picker.borrow_mut();
            if p.watching && p.last_tick.is_none_or(|t| t.elapsed() > IDLE) {
                p.watching = false;
                engine.watch_input(None);
            }
        },
    );
    std::mem::forget(timer);
}

/// The picker's rows: the interface's pairs then single channels, each
/// with its meters, the strip's own choice marked. The header names the
/// interface, and says when it runs on its own clock: not the main
/// output's (`clock`, the engine's status).
fn show(app: &AppWindow, p: &Picker, setup: &AudioSetup, clock: &str, delay: Option<f64>) {
    let role = InputRole::AUXILIARIES[p.strip - 1];
    let saved = setup
        .inputs
        .iter()
        .find(|r| r.role == role && r.device == p.device);
    let rows: Vec<PickerRow> = p
        .choices
        .iter()
        .enumerate()
        .map(|(i, &(c, mono))| {
            let level = |k: u16| p.meters.get(usize::from(k)).copied().unwrap_or(0.0);
            PickerRow {
                label: label(c, mono).into(),
                l: level(c),
                r: if mono { -1.0 } else { level(c + 1) },
                current: saved.is_some_and(|r| r.channel == c && (r.channels() == 1) == mono),
                gap: mono && i > 0 && !p.choices[i - 1].1,
            }
        })
        .collect();
    let ui = app.global::<InstrumentsUi>();
    ui.set_picker_strip(p.strip as i32);
    ui.set_picker_groove(
        setup
            .inputs
            .iter()
            .any(|r| r.role == role && r.device == GROOVE_BOX),
    );
    let header = if p.device.is_empty() && p.choices.is_empty() {
        "No input device".to_string()
    } else if p.device.is_empty() {
        "System default".to_string()
    } else if own_clock(&p.device, clock) {
        format!("{} · own clock, may drift", p.device)
    } else {
        p.device.clone()
    };
    // How far behind the strip's input is heard, while it is open.
    ui.set_picker_device(match delay {
        Some(ms) => format!("{header} · {ms:.0} ms behind").into(),
        None => header.into(),
    });
    // Fresh meters arrive 20 times a second: the rows change in place, so
    // the one under the pointer keeps its hover and a click lands on it.
    let model = ui.get_picker_rows();
    match model.as_any().downcast_ref::<VecModel<PickerRow>>() {
        Some(m) if m.row_count() == rows.len() => {
            for (i, row) in rows.into_iter().enumerate() {
                if m.row_data(i).as_ref() != Some(&row) {
                    m.set_row_data(i, row);
                }
            }
        }
        _ => ui.set_picker_rows(ModelRc::new(VecModel::from(rows))),
    }
}

/// How far behind strip `strip`'s input (from 1) is heard, in ms.
fn delay(engine: &AudioEngine, strip: usize) -> Option<f64> {
    let role = InputRole::AUXILIARIES.get(strip.checked_sub(1)?)?;
    engine.input_delays().get(role.index()).copied().flatten()
}

/// `device` is another interface than the main output's, as the engine's
/// status names it.
fn own_clock(device: &str, clock: &str) -> bool {
    let main = instruments::clock_device(clock);
    !device.is_empty() && !main.is_empty() && device != main
}

/// "1/2" for a pair from channel 1, "3" for channel 3 alone, as the
/// strip's input box shows them.
fn label(first: u16, mono: bool) -> String {
    channel_names(first, if mono { 1 } else { 2 })
        .trim_start_matches("In ")
        .replace(" & ", "/")
}

/// A peak (linear) as a meter reading, 0 at -40 dB to 1 at full scale, as
/// the engine's meters read.
fn meter(peak: f32) -> f32 {
    if peak <= 1e-4 {
        0.0
    } else {
        ((20.0 * peak.log10() + 40.0) / 40.0).clamp(0.0, 1.0)
    }
}

/// The input devices' names.
fn input_names() -> Vec<String> {
    input_devices().into_iter().map(|d| d.name).collect()
}

/// The first aux input without a device: where + adds a strip.
fn next_free(setup: &AudioSetup) -> Option<usize> {
    InputRole::AUXILIARIES
        .iter()
        .position(|role| !setup.inputs.iter().any(|r| r.role == *role))
        .map(|i| i + 1)
}

/// Strip `strip` (from 1) plays the Mixer deck's Groove Box: one strip
/// at a time, so another strip playing it goes back to having no input.
/// Takes channel `strip` (from 1) out of the Mixer: its input goes.
fn delete(setup: &mut AudioSetup, strip: usize) {
    let role = InputRole::AUXILIARIES[strip - 1];
    setup.inputs.retain(|r| r.role != role);
}

fn set_groove(setup: &mut AudioSetup, strip: usize) {
    let role = InputRole::AUXILIARIES[strip - 1];
    setup
        .inputs
        .retain(|r| r.role != role && r.device != GROOVE_BOX);
    setup.inputs.push(InputRoute {
        role,
        device: GROOVE_BOX.to_string(),
        channel: 0,
        mono: false,
    });
}

/// The interface the picker lists for `strip`: its own, else the one the
/// other strips use, else the main output's when it has inputs, else the
/// first input device.
fn device_for(setup: &AudioSetup, strip: usize, devices: &[String]) -> String {
    let role = InputRole::AUXILIARIES[strip - 1];
    let has = |name: &str| devices.iter().any(|d| d == name);
    // A strip playing the Groove Box lists the interface the others use.
    if let Some(r) = setup
        .inputs
        .iter()
        .find(|r| r.role == role && r.device != GROOVE_BOX)
    {
        return r.device.clone();
    }
    let others = setup
        .inputs
        .iter()
        .filter(|r| InputRole::AUXILIARIES.contains(&r.role) && has(&r.device));
    if let Some(r) = others.into_iter().next() {
        return r.device.clone();
    }
    if let Some(r) = setup
        .routes
        .iter()
        .find(|r| r.role == Role::Master && has(&r.device))
    {
        return r.device.clone();
    }
    devices.first().cloned().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aux(n: usize, device: &str) -> InputRoute {
        InputRoute {
            role: InputRole::AUXILIARIES[n - 1],
            device: device.into(),
            channel: 0,
            mono: false,
        }
    }

    #[test]
    fn a_strip_lists_its_own_interface_then_the_others_then_the_main_output() {
        let devices = [
            "MacBook Pro Microphone".to_string(),
            "M4".into(),
            "iPhone".into(),
        ];
        let mut setup = AudioSetup::default();
        // Nothing set up: the first input device.
        assert_eq!(device_for(&setup, 1, &devices), "MacBook Pro Microphone");
        // The main output's interface, when it has inputs.
        setup.routes.push(opendj_engine::Route {
            role: Role::Master,
            device: "M4".into(),
            pair: 0,
        });
        assert_eq!(device_for(&setup, 2, &devices), "M4");
        // The other strips' interface.
        setup.inputs.push(aux(1, "iPhone"));
        assert_eq!(device_for(&setup, 2, &devices), "iPhone");
        // Its own, even when not connected.
        setup.inputs.push(aux(2, "Old interface"));
        assert_eq!(device_for(&setup, 2, &devices), "Old interface");
    }

    #[test]
    fn another_interface_runs_on_its_own_clock() {
        let clock = "M4 · 48 kHz · 256 samples (5.3 ms)";
        assert!(!own_clock("M4", clock));
        assert!(own_clock("iPhone", clock));
        assert!(!own_clock("", clock));
        assert!(!own_clock("iPhone", "No audio output"));
    }

    #[test]
    fn plus_adds_the_first_free_strip() {
        let mut setup = AudioSetup::default();
        assert_eq!(next_free(&setup), Some(1));
        setup.inputs.push(aux(1, "M4"));
        setup.inputs.push(aux(3, "M4"));
        assert_eq!(next_free(&setup), Some(2));
        for n in 1..=8 {
            setup.inputs.push(aux(n, "M4"));
        }
        assert_eq!(next_free(&setup), None);
    }

    #[test]
    fn one_strip_at_a_time_plays_the_groove_box() {
        let devices = ["M4".to_string()];
        let mut setup = AudioSetup::default();
        setup.inputs.push(aux(1, "M4"));
        setup.inputs.push(aux(2, "M4"));
        set_groove(&mut setup, 2);
        assert_eq!(setup.inputs.len(), 2);
        // Its picker still lists the interface, to go back to it.
        assert_eq!(device_for(&setup, 2, &devices), "M4");
        // Moved to a new strip, the old one has no input again.
        set_groove(&mut setup, 3);
        let grooves: Vec<_> = setup
            .inputs
            .iter()
            .filter(|r| r.device == GROOVE_BOX)
            .map(|r| r.role)
            .collect();
        assert_eq!(grooves, [InputRole::AUXILIARIES[2]]);
        assert_eq!(next_free(&setup), Some(2));
    }

    #[test]
    fn deleting_a_channel_takes_only_its_input() {
        let mut setup = AudioSetup::default();
        setup.inputs.push(aux(1, "M4"));
        setup.inputs.push(aux(2, "M4"));
        delete(&mut setup, 1);
        let roles: Vec<_> = setup.inputs.iter().map(|r| r.role).collect();
        assert_eq!(roles, [InputRole::AUXILIARIES[1]]);
        assert_eq!(next_free(&setup), Some(1));
    }

    #[test]
    fn choices_read_as_the_input_box_shows_them() {
        assert_eq!(label(0, false), "1/2");
        assert_eq!(label(2, true), "3");
        assert!((meter(1.0) - 1.0).abs() < 1e-6);
        assert!((meter(0.1) - 0.5).abs() < 1e-3);
        assert_eq!(meter(0.0), 0.0);
    }
}
