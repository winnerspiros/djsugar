//! Takes sound from the microphone, for a pad sample of the user's own.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use cpal::traits::DeviceTrait;

use crate::output::{find_input, input_stream, no_audio};

/// The longest take, in seconds; what comes after is not kept.
pub const LONGEST: f32 = 60.0;

/// A take in progress. The device is held by a thread of its own, since a stream may not leave the thread that
/// opened it.
pub struct Capture {
    stop: Arc<AtomicBool>,
    taken: Arc<AtomicUsize>,
    pub rate: u32,
    thread: JoinHandle<Vec<f32>>,
}

type Take = Arc<Mutex<Vec<f32>>>;

fn open(taken: Arc<AtomicUsize>) -> Result<(cpal::Stream, u32, Take), String> {
    let device = if no_audio() { None } else { find_input("") }.ok_or("No microphone found")?;
    let config = device.default_input_config().map_err(|e| format!("Could not open the microphone: {e}"))?;
    let (channels, rate) = (usize::from(config.channels()).max(1), config.sample_rate());
    let most = (LONGEST * rate as f32) as usize;
    // All its room up front: the device's thread must not wait for memory.
    let take: Take = Arc::new(Mutex::new(Vec::with_capacity(most)));
    let lost = Arc::new(AtomicBool::new(false));
    let stream = input_stream(&device, &config, None, &lost, {
        let take = take.clone();
        move |data: &[f32]| {
            let Ok(mut take) = take.try_lock() else { return };
            let room = most - take.len();
            take.extend(data.chunks_exact(channels).take(room).map(|f| f.iter().sum::<f32>() / channels as f32));
            taken.store(take.len(), Ordering::Release);
        }
    })
    .map_err(|e| format!("Could not open the microphone: {e}"))?;
    Ok((stream, rate, take))
}

impl Capture {
    /// Opens the default microphone and starts taking from it.
    pub fn start() -> Result<Self, String> {
        let (stop, taken) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicUsize::new(0)));
        let (tell, told) = channel();
        let thread = std::thread::Builder::new()
            .name("capture".into())
            .spawn({
                let (stop, taken) = (stop.clone(), taken.clone());
                move || match open(taken) {
                    Ok((stream, rate, take)) => {
                        let _ = tell.send(Ok(rate));
                        while !stop.load(Ordering::Acquire) {
                            std::thread::sleep(Duration::from_millis(15));
                        }
                        drop(stream);
                        take.lock().map(|mut t| std::mem::take(&mut *t)).unwrap_or_default()
                    }
                    Err(why) => {
                        let _ = tell.send(Err(why));
                        Vec::new()
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        let rate = told.recv().map_err(|_| "Could not open the microphone".to_string())??;
        Ok(Self { stop, taken, rate, thread })
    }

    /// How much has been taken so far, in seconds.
    pub fn seconds(&self) -> f32 {
        self.taken.load(Ordering::Acquire) as f32 / self.rate.max(1) as f32
    }

    /// Ends the take and hands it over: one channel, at [`Capture::rate`].
    pub fn stop(self) -> Vec<f32> {
        self.stop.store(true, Ordering::Release);
        self.thread.join().unwrap_or_default()
    }
}
