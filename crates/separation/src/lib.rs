//! Stem separation inside DJ Sugar, without Python: Spleeter for stems within a
//! second or two of loading a track, HTDemucs for high-quality stems in the
//! background. Both run on burn, on the GPU (Metal on macOS, Vulkan on Linux)
//! or on the CPU. See openspec/changes/improve-stem-separation.

pub mod htdemucs;
pub mod spleeter;
pub mod stft;

use burn::tensor::{Device, DeviceKind};

/// The best GPU: Metal on macOS, Vulkan elsewhere.
pub fn gpu() -> Device {
    Device::wgpu(DeviceKind::DefaultDevice)
}

/// The CPU, multi-threaded.
pub fn cpu() -> Device {
    Device::flex()
}

/// The sample rate both models were trained at.
pub const SAMPLE_RATE: u32 = 44_100;

pub use burn;
pub use demucs_core::dsp::resample::resample_channel as resample;
