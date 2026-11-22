use std::sync::Arc;

use crate::{Command, Track, new};

fn sine(rate: u32, hz: f32, secs: f32) -> Arc<Track> {
    let n = (rate as f32 * secs) as usize;
    let frames = (0..n)
        .map(|i| {
            let v = (i as f32 * hz * std::f32::consts::TAU / rate as f32).sin() * 0.5;
            [v, v]
        })
        .collect();
    Arc::new(Track::new(rate, frames))
}

fn rms(b: &[f32]) -> f32 {
    (b.iter().map(|x| x * x).sum::<f32>() / b.len() as f32).sqrt()
}

#[test]
fn silent_until_played() {
    let (mut c, mut e) = new(48000, 256);
    c.send(Command::Load(0, sine(48000, 440.0, 1.0)));
    let mut out = vec![1.0; 512];
    e.process(&mut out);
    assert!(out.iter().all(|&s| s == 0.0));
    c.send(Command::Play(0, true));
    e.process(&mut out);
    assert!(rms(&out) > 0.3);
    assert!(c.poll().decks[0].playing);
}

#[test]
fn position_advances_with_rate_and_resampling() {
    let (mut c, mut e) = new(48000, 256);
    c.send(Command::Load(0, sine(44100, 440.0, 2.0)));
    c.send(Command::Rate(0, 0.08));
    c.send(Command::Play(0, true));
    let mut out = vec![0.0; 960];
    for _ in 0..100 {
        e.process(&mut out);
    }
    let p = c.poll().decks[0].position;
    let want = 48000.0 * 1.08 * 44100.0 / 48000.0;
    assert!((p - want).abs() < 1.0, "{p} vs {want}");
}

#[test]
fn bass_kill_removes_low_tone() {
    let (mut c, mut e) = new(48000, 512);
    c.send(Command::Load(0, sine(48000, 60.0, 2.0)));
    c.send(Command::Play(0, true));
    let mut out = vec![0.0; 9600];
    e.process(&mut out);
    let open = rms(&out[4800..]);
    c.send(Command::Kill(0, [true, false, false]));
    e.process(&mut out);
    e.process(&mut out);
    let killed = rms(&out[4800..]);
    assert!(killed < open * 0.1, "{killed} vs {open}");
}

#[test]
fn crossfader_cuts_the_far_side() {
    let (mut c, mut e) = new(48000, 256);
    c.send(Command::Load(1, sine(48000, 440.0, 1.0)));
    c.send(Command::Play(1, true));
    c.send(Command::Crossfader(-1.0));
    let mut out = vec![0.0; 512];
    e.process(&mut out);
    assert_eq!(rms(&out), 0.0);
}

#[test]
fn replaced_tracks_are_freed_off_the_audio_thread() {
    let (mut c, mut e) = new(48000, 256);
    let t = sine(48000, 440.0, 0.1);
    c.send(Command::Load(0, t.clone()));
    let mut out = vec![0.0; 64];
    e.process(&mut out);
    c.send(Command::Load(0, sine(48000, 220.0, 0.1)));
    e.process(&mut out);
    assert_eq!(Arc::strong_count(&t), 2);
    c.poll();
    assert_eq!(Arc::strong_count(&t), 1);
}

fn pitch_hz(b: &[f32], rate: f32) -> f32 {
    let crossings = b
        .as_chunks::<2>()
        .0
        .iter()
        .map(|f| f[0])
        .collect::<Vec<_>>()
        .windows(2)
        .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
        .count();
    crossings as f32 * rate / (b.len() / 2) as f32
}

#[test]
fn key_lock_keeps_pitch_while_tempo_changes() {
    let (mut c, mut e) = new(48000, 512);
    c.send(Command::Load(0, sine(44100, 440.0, 4.0)));
    c.send(Command::KeyLock(0, true));
    c.send(Command::Rate(0, 0.25));
    c.send(Command::Play(0, true));
    let mut out = vec![0.0; 1024];
    for _ in 0..40 {
        e.process(&mut out);
    }
    let mut long = vec![0.0; 48000];
    e.process(&mut long);
    let hz = pitch_hz(&long, 48000.0);
    assert!((hz - 440.0).abs() < 8.0, "{hz}");
    let p = c.poll().decks[0].position;
    let want = (40.0 * 512.0 + 24000.0) * 1.25 * 44100.0 / 48000.0;
    assert!((p - want).abs() < 2.0, "{p} vs {want}");
}

#[test]
fn key_shift_moves_pitch_by_semitones() {
    let (mut c, mut e) = new(48000, 512);
    c.send(Command::Load(0, sine(48000, 440.0, 4.0)));
    c.send(Command::Key(0, 12.0));
    c.send(Command::Play(0, true));
    let mut out = vec![0.0; 1024];
    for _ in 0..40 {
        e.process(&mut out);
    }
    let mut long = vec![0.0; 48000];
    e.process(&mut long);
    let hz = pitch_hz(&long, 48000.0);
    assert!((hz - 880.0).abs() < 15.0, "{hz}");
}

#[test]
fn samples_play_once_and_are_freed() {
    let (mut c, mut e) = new(48000, 256);
    let t = sine(48000, 440.0, 0.01);
    c.send(Command::Sample(t.clone(), 1.0));
    let mut out = vec![0.0; 2048];
    e.process(&mut out);
    assert!(rms(&out[..960]) > 0.3);
    assert!(rms(&out[1000..]) == 0.0);
    c.poll();
    assert_eq!(Arc::strong_count(&t), 1);
}
