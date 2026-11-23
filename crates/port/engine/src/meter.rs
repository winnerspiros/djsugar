//! The master's loudness meter, for recording and broadcasting: the audio thread copies the master into a ring buffer (a tap like the recorder's), a thread here empties it into a [`LiveLoudness`] and publishes `[Master],loudness_momentary`, `loudness_short_term` and `loudness_integrated` in LUFS,
//! `loudness_range` in LU and `true_peak` (the highest since the reset) in dBTP; [`NO_READING`] while there is nothing to read (silence, or too soon after a reset). `[Master],loudness_reset` starts integrated loudness,
//! the range and the true peak again, as does starting a recording or going on air; `[Master],loudness_target` holds the chosen target in LUFS, for the meter's colours.

use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use djsugar_control::{Behavior, ControlHandle, ControlRegistry};

use crate::loudness::LiveLoudness;
use crate::recorder::Feed;

/// The value a reading has while there is nothing to measure.
pub const NO_READING: f64 = -100.0;

/// The meter's target by default, in LUFS: internet radio, as AES TD1008
/// suggests for mixed music programmes.
pub const DEFAULT_LOUDNESS_TARGET: f64 = -16.0;

/// How often the thread empties the buffer.
const POLL: Duration = Duration::from_millis(30);
/// How often momentary, short-term and the true peak are published.
const FAST: Duration = Duration::from_millis(100);
/// How often integrated loudness and the range are worked out again.
const SLOW: Duration = Duration::from_millis(1000);

struct Readings {
    momentary: ControlHandle,
    short_term: ControlHandle,
    integrated: ControlHandle,
    range: ControlHandle,
    true_peak: ControlHandle,
    reset: ControlHandle,
}

fn reading(reg: &ControlRegistry, item: &str) -> ControlHandle {
    reg.create("[Master]", item, NO_READING, NO_READING, 30.0, Behavior::Indicator)
}

/// Creates the meter's controls and starts the thread that feeds them. The
/// thread ends when the registry goes.
pub(crate) fn start(registry: &Arc<ControlRegistry>, rate: f64, feed: Feed) {
    let readings = Readings {
        momentary: reading(registry, "loudness_momentary"),
        short_term: reading(registry, "loudness_short_term"),
        integrated: reading(registry, "loudness_integrated"),
        range: registry.create("[Master]", "loudness_range", NO_READING, NO_READING, 60.0, Behavior::Indicator),
        true_peak: reading(registry, "true_peak"),
        reset: registry.push("[Master]", "loudness_reset"),
    };
    registry.create("[Master]", "loudness_target", DEFAULT_LOUDNESS_TARGET, -40.0, 0.0, Behavior::Free);
    let weak = Arc::downgrade(registry);
    std::thread::Builder::new()
        .name("loudness-meter".into())
        .spawn(move || run(weak, rate, feed, readings))
        .expect("spawn loudness meter");
}

fn run(registry: Weak<ControlRegistry>, rate: f64, mut feed: Feed, r: Readings) {
    let mut meter = LiveLoudness::new(rate);
    let mut samples = Vec::with_capacity((rate * 0.2) as usize * 2);
    let mut reset_seen = r.reset.version();
    let mut was_live = false;
    let mut last_fast = Instant::now();
    let mut last_slow = Instant::now();
    while let Some(reg) = registry.upgrade() {
        samples.clear();
        feed.drain_into(&mut samples);
        for f in samples.as_chunks::<2>().0 {
            meter.push(*f);
        }
        // A press of Reset, or the start of a recording or broadcast.
        let v = r.reset.version();
        let pressed = v != reset_seen && (r.reset.get_bool() || v.wrapping_sub(reset_seen) >= 2);
        reset_seen = v;
        let live = is_live(&reg);
        drop(reg);
        if pressed || (live && !was_live) {
            meter.reset();
            r.integrated.set(NO_READING);
            r.range.set(NO_READING);
            r.true_peak.set(NO_READING);
            last_slow = Instant::now() - SLOW;
        }
        was_live = live;
        if last_fast.elapsed() >= FAST {
            last_fast = Instant::now();
            r.momentary.set(meter.momentary().unwrap_or(NO_READING));
            r.short_term.set(meter.short_term().unwrap_or(NO_READING));
            r.true_peak.set(meter.true_peak().unwrap_or(NO_READING));
        }
        if last_slow.elapsed() >= SLOW {
            last_slow = Instant::now();
            r.integrated.set(meter.integrated().unwrap_or(NO_READING));
            r.range.set(meter.range().unwrap_or(NO_READING));
        }
        std::thread::sleep(POLL);
    }
}

/// Recording or on air.
fn is_live(reg: &ControlRegistry) -> bool {
    let on = |group: &str, item: &str, at: f64| reg.get(group, item).is_some_and(|c| c.get() >= at);
    on("[Recording]", "status", 2.0) || on("[Shoutcast]", "enabled", 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recorder;

    fn wait_for(what: impl Fn() -> bool) -> bool {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            if what() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    #[test]
    fn publishes_the_master_loudness_and_resets() {
        let reg = ControlRegistry::new();
        let rate = 48_000.0;
        let (mut tap, feed) = recorder::tap(rate);
        feed.set_active(true);
        start(&reg, rate, feed);
        // Four seconds of a 1 kHz tone at -20 dBFS, as the audio thread
        // would hand it over.
        let amp = 10f32.powf(-20.0 / 20.0);
        let mut out = Vec::new();
        for i in 0..(4.0 * rate) as usize {
            let v = amp * (std::f32::consts::TAU * 1000.0 * i as f32 / rate as f32).sin();
            out.extend([v, v]);
            if out.len() == 2048 {
                tap.push(&out, 2);
                out.clear();
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        let get = |item: &str| reg.get("[Master]", item).unwrap().get();
        assert!(
            wait_for(|| (get("loudness_short_term") + 20.0).abs() < 0.2),
            "short-term {}",
            get("loudness_short_term")
        );
        assert!(wait_for(|| (get("loudness_integrated") + 20.0).abs() < 0.2));
        assert!((get("true_peak") + 20.0).abs() < 0.2, "{}", get("true_peak"));
        // Reset clears what counts from the reset on.
        let reset = reg.get("[Master]", "loudness_reset").unwrap();
        reset.set(1.0);
        reset.set(0.0);
        assert!(wait_for(|| get("loudness_integrated") == NO_READING));
        assert_eq!(get("loudness_target"), DEFAULT_LOUDNESS_TARGET);
    }
}
