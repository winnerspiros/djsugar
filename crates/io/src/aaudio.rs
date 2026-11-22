#![allow(unsafe_code, reason = "AAudio hands the callback a raw buffer pointer")]

use ndk::audio::{
    AudioCallbackResult, AudioContentType, AudioDirection, AudioFormat, AudioPerformanceMode, AudioSharingMode,
    AudioStream, AudioStreamBuilder, AudioUsage,
};
use sugar_engine::Engine;

use crate::{Error, Format};

pub struct Stream(AudioStream);

impl std::fmt::Debug for Stream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Stream")
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        let _ = self.0.request_stop();
    }
}

fn err(e: impl std::fmt::Display) -> Error {
    Error(format!("aaudio: {e}"))
}

fn builder(sharing: AudioSharingMode) -> Result<AudioStreamBuilder, Error> {
    Ok(AudioStreamBuilder::new()
        .map_err(err)?
        .direction(AudioDirection::Output)
        .format(AudioFormat::PCM_Float)
        .channel_count(2)
        .performance_mode(AudioPerformanceMode::LowLatency)
        .sharing_mode(sharing)
        .usage(AudioUsage::Media)
        .content_type(AudioContentType::Music))
}

pub fn open(make: impl FnOnce(Format) -> Engine) -> Result<(Stream, Format), Error> {
    let probe = builder(AudioSharingMode::Exclusive)?
        .open_stream()
        .or_else(|_| builder(AudioSharingMode::Shared)?.open_stream().map_err(err))?;
    let rate = probe.sample_rate();
    let sharing = probe.sharing_mode();
    let burst = probe.frames_per_burst().max(32) as usize;
    drop(probe);

    let mut engine = make(Format { rate: rate as u32, burst, buffer: burst * 2 });
    let stream = builder(sharing)?
        .sample_rate(rate)
        .data_callback(Box::new(move |_, data, frames| {
            let out = unsafe { std::slice::from_raw_parts_mut(data.cast::<f32>(), frames as usize * 2) };
            engine.process(out);
            AudioCallbackResult::Continue
        }))
        .open_stream()
        .map_err(err)?;
    let buffer = stream.set_buffer_size_in_frames(burst as i32 * 2).map_or(burst * 2, |n| n as usize);
    stream.request_start().map_err(err)?;
    Ok((Stream(stream), Format { rate: rate as u32, burst, buffer }))
}
