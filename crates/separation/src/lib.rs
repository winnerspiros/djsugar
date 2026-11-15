//! Stem separation inside DJ Sugar, without Python: Spleeter for stems within a
//! second or two of loading a track, HTDemucs for high-quality stems in the
//! background. Both run on burn, on the GPU (Metal on macOS, Vulkan on Linux)
//! or on the CPU. See openspec/changes/improve-stem-separation.

pub mod htdemucs;
pub mod spleeter;
pub mod stft;

/// burn's GPU backend: Metal on macOS, Vulkan elsewhere.
pub type Gpu = burn::backend::Wgpu;
/// burn's CPU backend, multi-threaded.
pub type Cpu = burn::backend::NdArray<f32>;

/// The sample rate both models were trained at.
pub const SAMPLE_RATE: u32 = 44_100;

pub use burn;
pub use demucs_core::dsp::resample::resample_channel as resample;
