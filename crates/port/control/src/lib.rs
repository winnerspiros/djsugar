//! Named controls, compatible with `[Group],key` addressing. Every knob, button and meter is a control looked up by name in a global, mutex-guarded
//! hash and changed through Qt signals. Here the names stay the same (so skins and controller mappings keep working), but a name is resolved once to
//! a [`ControlHandle`]. Reading or writing a handle is a single atomic operation: no locks, no allocation, safe on the real-time audio thread.

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

/// A control's address, e.g. `[Channel1],play`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConfigKey {
    pub group: String,
    pub item: String,
}

impl ConfigKey {
    pub fn new(group: impl Into<String>, item: impl Into<String>) -> Self {
        Self { group: group.into(), item: item.into() }
    }
}

impl fmt::Display for ConfigKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{}", self.group, self.item)
    }
}

/// How a control's value maps to a normalized 0..1 widget position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Behavior {
    /// Linear between `min` and `max` (faders, crossfader, rate).
    Linear,
    /// EQ/gain knob: 0 -> min, 0.5 -> default (unity), 1 -> max.
    /// Each half is linear, which matches how DJ EQ knobs centre on unity gain.
    CenteredOnDefault,
    /// On/off; widgets toggle it.
    Toggle,
    /// Momentary; widgets set 1 while held and 0 on release.
    Push,
    /// Read-only output such as a VU meter or play position.
    Indicator,
    /// Any value, no range. Used for controls a controller script asks for
    /// that nothing here implements yet, so the script still reads back what
    /// it wrote; its parameter is the value itself.
    Free,
}

#[derive(Debug)]
pub struct Control {
    key: ConfigKey,
    bits: AtomicU64,
    version: AtomicU32,
    /// Held by something driving the control on its own (Automix and the
    /// crossfader): writes from anyone else are ignored until it lets go.
    locked: AtomicBool,
    default: f64,
    min: f64,
    max: f64,
    behavior: Behavior,
}

impl Control {
    fn new(key: ConfigKey, default: f64, min: f64, max: f64, behavior: Behavior) -> Self {
        Self {
            key,
            bits: AtomicU64::new(default.to_bits()),
            version: AtomicU32::new(0),
            locked: AtomicBool::new(false),
            default,
            min,
            max,
            behavior,
        }
    }
}

/// A resolved control. Cheap to clone; all operations are lock-free.
#[derive(Clone, Debug)]
pub struct ControlHandle(Arc<Control>);

impl ControlHandle {
    pub fn key(&self) -> &ConfigKey {
        &self.0.key
    }

    pub fn behavior(&self) -> Behavior {
        self.0.behavior
    }

    pub fn default_value(&self) -> f64 {
        self.0.default
    }

    pub fn range(&self) -> (f64, f64) {
        (self.0.min, self.0.max)
    }

    pub fn get(&self) -> f64 {
        f64::from_bits(self.0.bits.load(Ordering::Relaxed))
    }

    pub fn get_bool(&self) -> bool {
        self.get() > 0.0
    }

    /// Sets the value, clamped to the control's range. Does nothing while
    /// the control is [locked](Self::lock).
    pub fn set(&self, value: f64) {
        if self.is_locked() {
            return;
        }
        self.force(value);
    }

    /// Sets the value even while the control is locked: for whatever holds
    /// the lock.
    pub fn force(&self, value: f64) {
        let v = value.clamp(self.0.min, self.0.max);
        let old = self.0.bits.swap(v.to_bits(), Ordering::Relaxed);
        if old != v.to_bits() {
            self.0.version.fetch_add(1, Ordering::Release);
        }
    }

    pub fn set_bool(&self, on: bool) {
        self.set(if on { 1.0 } else { 0.0 });
    }

    pub fn toggle(&self) {
        self.set_bool(!self.get_bool());
    }

    pub fn reset(&self) {
        self.set(self.0.default);
    }

    /// Locks or unlocks the control. While locked, [`set`](Self::set) and
    /// everything built on it (widgets, controller mappings, scripts) leave
    /// the value alone; only [`force`](Self::force) moves it.
    pub fn lock(&self, on: bool) {
        self.0.locked.store(on, Ordering::Release);
    }

    pub fn is_locked(&self) -> bool {
        self.0.locked.load(Ordering::Acquire)
    }

    /// Increments every time the value changes; lets pollers skip unchanged controls.
    pub fn version(&self) -> u32 {
        self.0.version.load(Ordering::Acquire)
    }

    /// The value as a 0..1 widget position.
    pub fn get_parameter(&self) -> f64 {
        value_to_parameter(self.0.behavior, self.get(), self.0.default, self.0.min, self.0.max)
    }

    /// Sets the value from a 0..1 widget position.
    pub fn set_parameter(&self, parameter: f64) {
        self.set(parameter_to_value(
            self.0.behavior,
            parameter.clamp(0.0, 1.0),
            self.0.default,
            self.0.min,
            self.0.max,
        ));
    }
}

fn value_to_parameter(behavior: Behavior, v: f64, default: f64, min: f64, max: f64) -> f64 {
    match behavior {
        Behavior::Free => v,
        Behavior::CenteredOnDefault => {
            if v <= default {
                if default > min {
                    0.5 * (v - min) / (default - min)
                } else {
                    0.5
                }
            } else if max > default {
                0.5 + 0.5 * (v - default) / (max - default)
            } else {
                0.5
            }
        }
        _ => {
            if max > min {
                (v - min) / (max - min)
            } else {
                0.0
            }
        }
    }
}

fn parameter_to_value(behavior: Behavior, p: f64, default: f64, min: f64, max: f64) -> f64 {
    match behavior {
        Behavior::Free => p,
        Behavior::CenteredOnDefault => {
            if p <= 0.5 {
                min + (default - min) * (p / 0.5)
            } else {
                default + (max - default) * ((p - 0.5) / 0.5)
            }
        }
        _ => min + (max - min) * p,
    }
}

/// The set of all controls. Lookups by name take a read lock, so resolve
/// names once (at startup or when a skin/mapping loads) and keep the handles.
#[derive(Default)]
pub struct ControlRegistry {
    controls: RwLock<HashMap<ConfigKey, ControlHandle>>,
    aliases: RwLock<HashMap<ConfigKey, ConfigKey>>,
}

impl ControlRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Creates a control, or returns the existing one with that name.
    pub fn create(
        &self,
        group: &str,
        item: &str,
        default: f64,
        min: f64,
        max: f64,
        behavior: Behavior,
    ) -> ControlHandle {
        let key = ConfigKey::new(group, item);
        let mut map = self.controls.write().unwrap();
        map.entry(key.clone())
            .or_insert_with(|| ControlHandle(Arc::new(Control::new(key, default, min, max, behavior))))
            .clone()
    }

    pub fn toggle(&self, group: &str, item: &str) -> ControlHandle {
        self.create(group, item, 0.0, 0.0, 1.0, Behavior::Toggle)
    }

    pub fn push(&self, group: &str, item: &str) -> ControlHandle {
        self.create(group, item, 0.0, 0.0, 1.0, Behavior::Push)
    }

    pub fn indicator(&self, group: &str, item: &str, max: f64) -> ControlHandle {
        self.create(group, item, 0.0, 0.0, max, Behavior::Indicator)
    }

    /// A control with no range; see [`Behavior::Free`].
    pub fn free(&self, group: &str, item: &str) -> ControlHandle {
        self.create(group, item, 0.0, f64::MIN, f64::MAX, Behavior::Free)
    }

    /// Makes `alias` resolve to `target`, like `[Master]` -> `[Main]` rename.
    pub fn insert_alias(&self, alias: ConfigKey, target: ConfigKey) {
        self.aliases.write().unwrap().insert(alias, target);
    }

    pub fn get(&self, group: &str, item: &str) -> Option<ControlHandle> {
        let mut key = ConfigKey::new(group, item);
        if let Some(target) = self.aliases.read().unwrap().get(&key) {
            key = target.clone();
        }
        self.controls.read().unwrap().get(&key).cloned()
    }

    pub fn len(&self) -> usize {
        self.controls.read().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn keys(&self) -> Vec<ConfigKey> {
        let mut keys: Vec<_> = self.controls.read().unwrap().keys().cloned().collect();
        keys.sort();
        keys
    }

    /// The names that resolve to another control, sorted; see [`Self::insert_alias`].
    pub fn alias_keys(&self) -> Vec<ConfigKey> {
        let mut keys: Vec<_> = self.aliases.read().unwrap().keys().cloned().collect();
        keys.sort();
        keys
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_by_name_and_alias() {
        let reg = ControlRegistry::new();
        let play = reg.toggle("[Channel1]", "play");
        assert!(reg.get("[Channel1]", "play").is_some());
        assert!(reg.get("[Channel1]", "nope").is_none());
        reg.create("[Main]", "gain", 1.0, 0.0, 5.0, Behavior::CenteredOnDefault);
        reg.insert_alias(ConfigKey::new("[Master]", "gain"), ConfigKey::new("[Main]", "gain"));
        assert_eq!(reg.get("[Master]", "gain").unwrap().get(), 1.0);
        play.set_bool(true);
        assert!(reg.get("[Channel1]", "play").unwrap().get_bool());
    }

    #[test]
    fn clamps_and_counts_changes() {
        let reg = ControlRegistry::new();
        let xf = reg.create("[Master]", "crossfader", 0.0, -1.0, 1.0, Behavior::Linear);
        let v0 = xf.version();
        xf.set(3.0);
        assert_eq!(xf.get(), 1.0);
        xf.set(1.0);
        assert_eq!(xf.version(), v0 + 1);
    }

    #[test]
    fn a_locked_control_only_moves_when_forced() {
        let reg = ControlRegistry::new();
        let xf = reg.create("[Master]", "crossfader", 0.0, -1.0, 1.0, Behavior::Linear);
        xf.lock(true);
        xf.set(-1.0);
        xf.set_parameter(1.0);
        xf.reset();
        assert_eq!(xf.get(), 0.0);
        xf.force(0.5);
        assert_eq!(xf.get(), 0.5);
        xf.lock(false);
        xf.set(-1.0);
        assert_eq!(xf.get(), -1.0);
    }

    #[test]
    fn eq_knob_centres_on_unity() {
        let reg = ControlRegistry::new();
        let eq =
            reg.create("[EqualizerRack1_[Channel1]_Effect1]", "parameter1", 1.0, 0.0, 4.0, Behavior::CenteredOnDefault);
        assert_eq!(eq.get_parameter(), 0.5);
        eq.set_parameter(1.0);
        assert_eq!(eq.get(), 4.0);
        eq.set_parameter(0.25);
        assert_eq!(eq.get(), 0.5);
        eq.set_parameter(0.0);
        assert_eq!(eq.get(), 0.0);
    }
}
