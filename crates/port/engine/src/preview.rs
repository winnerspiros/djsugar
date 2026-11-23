//! The preview player, `[PreviewDeck1]`: listens to a track in the headphones without loading it on a deck. It plays into the cue (headphone) mix
//! only, so the audience never hears it. Controls: `play`, `playposition` (0 to 1; set it to seek), `duration`, `track_loaded` and `volume`. The
//! audio arrives over a wait-free queue, and audio it is done with goes back over another to be freed off the audio thread, as deck tracks do.

use std::sync::Arc;

use djsugar_control::{Behavior, ControlHandle, ControlRegistry};

use crate::track::Audio;

pub const PREVIEW_GROUP: &str = "[PreviewDeck1]";

/// The other side of the player's queues, for [`crate::AudioEngine::preview`].
pub(crate) struct PreviewInput {
    audio: rtrb::Producer<(Option<Arc<Audio>>, bool)>,
    done: rtrb::Consumer<Arc<Audio>>,
}

impl PreviewInput {
    /// Puts `audio` in the player at its start, playing if `play`, or
    /// empties it. Frees audio the player has finished with.
    pub(crate) fn send(&mut self, audio: Option<Arc<Audio>>, play: bool) {
        while self.done.pop().is_ok() {}
        let _ = self.audio.push((audio, play));
    }
}

pub(crate) struct Preview {
    audio: Option<Arc<Audio>>,
    /// Play position in source frames.
    pos: f64,
    /// Source frames per output frame.
    step: f64,
    rate: f64,
    play: ControlHandle,
    position: ControlHandle,
    duration: ControlHandle,
    loaded: ControlHandle,
    volume: ControlHandle,
    /// The position this player last published, so a different one is a seek.
    written: f64,
    gain: f32,
    incoming: rtrb::Consumer<(Option<Arc<Audio>>, bool)>,
    done: rtrb::Producer<Arc<Audio>>,
}

impl Preview {
    pub(crate) fn new(reg: &ControlRegistry, sample_rate: f64) -> (Self, PreviewInput) {
        let g = PREVIEW_GROUP;
        let (audio_tx, incoming) = rtrb::RingBuffer::new(8);
        let (done, done_rx) = rtrb::RingBuffer::new(16);
        (
            Preview {
                audio: None,
                pos: 0.0,
                step: 1.0,
                rate: sample_rate,
                play: reg.toggle(g, "play"),
                position: reg.create(g, "playposition", 0.0, 0.0, 1.0, Behavior::Linear),
                duration: reg.indicator(g, "duration", 86_400.0),
                loaded: reg.indicator(g, "track_loaded", 1.0),
                volume: reg.create(g, "volume", 1.0, 0.0, 1.0, Behavior::Linear),
                written: 0.0,
                gain: 0.0,
                incoming,
                done,
            },
            PreviewInput { audio: audio_tx, done: done_rx },
        )
    }

    fn len(&self) -> f64 {
        self.audio.as_ref().map_or(0.0, |a| a.len() as f64)
    }

    /// Takes new audio and seeks; once per buffer, before [`Preview::next`].
    pub(crate) fn update(&mut self) {
        while let Ok((audio, play)) = self.incoming.pop() {
            if let Some(old) = std::mem::replace(&mut self.audio, audio) {
                // If the queue is full it is freed here; rare and harmless.
                let _ = self.done.push(old);
            }
            self.pos = 0.0;
            self.written = 0.0;
            self.position.set(0.0);
            self.play.set_bool(play && self.audio.is_some());
            self.step = self.audio.as_ref().map_or(1.0, |a| a.sample_rate as f64 / self.rate);
        }
        self.loaded.set_bool(self.audio.is_some());
        self.duration.set(self.audio.as_ref().map_or(0.0, |a| a.duration()));
        let wanted = self.position.get();
        if (wanted - self.written).abs() > 1e-9 {
            self.pos = wanted.clamp(0.0, 1.0) * self.len();
            self.written = wanted;
        }
        if self.audio.is_none() {
            self.play.set_bool(false);
        }
    }

    /// The next frame of the preview, for the cue mix.
    #[inline]
    pub(crate) fn next(&mut self) -> [f32; 2] {
        let playing = self.play.get_bool();
        // A short ramp, so starting and stopping don't click.
        let target = if playing { self.volume.get() as f32 } else { 0.0 };
        self.gain += (target - self.gain) * 0.002;
        let Some(audio) = &self.audio else {
            return [0.0; 2];
        };
        if self.gain < 1e-4 && !playing {
            return [0.0; 2];
        }
        let i = self.pos as usize;
        let t = (self.pos - i as f64) as f32;
        let (a, b) = (audio.frame(i), audio.frame(i + 1));
        if playing {
            self.pos += self.step;
        }
        let g = self.gain;
        [(a[0] + (b[0] - a[0]) * t) * g, (a[1] + (b[1] - a[1]) * t) * g]
    }

    /// Publishes where it is, and stops at the end; once per buffer.
    pub(crate) fn publish(&mut self) {
        let len = self.len();
        if len <= 0.0 {
            return;
        }
        if self.pos >= len {
            self.pos = 0.0;
            self.play.set_bool(false);
        }
        self.written = self.pos / len;
        self.position.set(self.written);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plays_from_where_it_is_set_and_stops_at_the_end() {
        let reg = ControlRegistry::new();
        let (mut p, mut input) = Preview::new(&reg, 100.0);
        let frames: Vec<[f32; 2]> = (0..100).map(|i| [i as f32 / 100.0; 2]).collect();
        input.send(Some(Arc::new(Audio::from_frames(100, frames))), false);
        p.update();
        assert!(reg.get(PREVIEW_GROUP, "track_loaded").unwrap().get_bool());
        assert_eq!(reg.get(PREVIEW_GROUP, "duration").unwrap().get(), 1.0);
        // Stopped, it is silent.
        assert_eq!(p.next(), [0.0; 2]);

        reg.get(PREVIEW_GROUP, "playposition").unwrap().set(0.5);
        reg.get(PREVIEW_GROUP, "play").unwrap().set(1.0);
        p.update();
        let mut last = [0.0; 2];
        for _ in 0..40 {
            last = p.next();
        }
        p.publish();
        assert!(last[0] > 0.0);
        let at = reg.get(PREVIEW_GROUP, "playposition").unwrap().get();
        assert!((at - 0.9).abs() < 1e-6, "{at}");

        for _ in 0..20 {
            p.next();
        }
        p.publish();
        assert!(!reg.get(PREVIEW_GROUP, "play").unwrap().get_bool());

        input.send(None, true);
        p.update();
        assert!(!reg.get(PREVIEW_GROUP, "track_loaded").unwrap().get_bool());
    }
}
