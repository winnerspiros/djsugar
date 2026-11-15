//! Which MIDI keyboard and channel play the instrument ("MIDI from" on the
//! instrument deck): any by default, kept in settings, applied in the
//! controllers' routing.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use djsugar_controllers::routing;
use djsugar_controllers::Controllers;
use slint::{ComponentHandle, ModelRc, SharedString};

use crate::settings;
use crate::{AppWindow, InstrumentsUi};

struct State {
    /// The keyboards the menu lists: those connected, and the one picked
    /// even while it is unplugged.
    keyboards: Vec<String>,
    device: Option<String>,
    channel: u8,
}

impl State {
    fn show(&self, ui: &InstrumentsUi) {
        let names: Vec<SharedString> = self
            .keyboards
            .iter()
            .map(|k| crate::short_port_name(k).into())
            .collect();
        ui.set_keyboards(ModelRc::from(names.as_slice()));
        let device = self.device.as_ref().map_or(0, |d| {
            let key = djsugar_controllers::device_key(d);
            self.keyboards
                .iter()
                .position(|k| djsugar_controllers::device_key(k) == key)
                .map_or(0, |i| i + 1)
        });
        ui.set_midi_device(device as i32);
        ui.set_midi_channel(self.channel as i32);
    }

    /// The menu's keyboards from those connected now.
    fn list(&mut self, connected: Vec<String>) -> bool {
        let mut list = connected;
        if let Some(d) = &self.device {
            let key = djsugar_controllers::device_key(d);
            if !list
                .iter()
                .any(|k| djsugar_controllers::device_key(k) == key)
            {
                list.push(d.clone());
            }
        }
        let changed = list != self.keyboards;
        self.keyboards = list;
        changed
    }
}

/// Applies the saved choice and runs the menu. The returned timer keeps
/// the keyboard list current; keep it alive.
pub fn install(app: &AppWindow, controllers: Option<Rc<Controllers>>) -> slint::Timer {
    let (device, channel) = settings::instrument_midi_from();
    routing::set_instrument_source(device.as_deref(), channel);
    let state = Rc::new(RefCell::new(State {
        keyboards: Vec::new(),
        device,
        channel,
    }));
    let connected = {
        let controllers = controllers.clone();
        move || {
            controllers
                .as_ref()
                .map(|c| c.keyboards())
                .unwrap_or_default()
        }
    };
    {
        let mut s = state.borrow_mut();
        s.list(connected());
        s.show(&app.global::<InstrumentsUi>());
    }
    {
        let (weak, state) = (app.as_weak(), state.clone());
        app.global::<InstrumentsUi>()
            .on_set_midi_from(move |device, channel| {
                let Some(app) = weak.upgrade() else {
                    return;
                };
                let mut s = state.borrow_mut();
                s.device = usize::try_from(device - 1)
                    .ok()
                    .and_then(|i| s.keyboards.get(i).cloned());
                s.channel = channel.clamp(0, 16) as u8;
                routing::set_instrument_source(s.device.as_deref(), s.channel);
                settings::set_instrument_midi_from(s.device.as_deref(), s.channel);
                s.show(&app.global::<InstrumentsUi>());
            });
    }
    let timer = slint::Timer::default();
    let weak = app.as_weak();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_secs(1),
        move || {
            let Some(app) = weak.upgrade() else {
                return;
            };
            let mut s = state.borrow_mut();
            if s.list(connected()) {
                s.show(&app.global::<InstrumentsUi>());
            }
        },
    );
    timer
}
