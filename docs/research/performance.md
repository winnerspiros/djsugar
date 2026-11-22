# DJ Sugar Performance Optimization Playbook

> **Target platforms:** Android (arm64, Snapdragon 8 Gen 2 / Samsung S23 Ultra primary), Linux, Windows, macOS (Apple Silicon + Intel)
> **Goal:** Maximum optimization across compilation, runtime, audio thread, GPU, memory, and startup.
> **Style:** Facts only, concise, cited, config snippets included.

---

## Table of Contents

1. [Compiler & Build Flags](#1-compiler--build-flags)
2. [PGO & BOLT](#2-pgo--bolt)
3. [Linker Choice](#3-linker-choice)
4. [Allocator Choice](#4-allocator-choice)
5. [Binary Size](#5-binary-size)
6. [SIMD Strategy](#6-simd-strategy)
7. [Audio Thread: Priority & Realtime APIs](#7-audio-thread-priority--realtime-apis)
8. [CPU Affinity on big.LITTLE](#8-cpu-affinity-on-biglittle)
9. [Denormals (FTZ/DAZ)](#9-denormals-ftzdaz)
10. [Buffer Sizes & Latency Targets](#10-buffer-sizes--latency-targets)
11. [Allocation/Lock-Free Callbacks](#11-allocationlock-free-callbacks)
12. [Android ADPF & Thermal](#12-android-adpf--thermal)
13. [GPU: wgpu Backend & Mobile Optimization](#13-gpu-wgpu-backend--mobile-optimization)
14. [Memory: mmap & Cache Layouts](#14-memory-mmap--cache-layouts)
15. [Startup Time](#15-startup-time)

---

## 1. Compiler & Build Flags

### What
Rustc codegen flags controlling optimization level, LTO, codegen units, panic strategy, and target-specific features.

### Why
Default `--release` uses `opt-level=3`, `codegen-units=16`, `panic=unwind`. Tuning these yields 10-30% gains for compute-heavy audio/DSP workloads.

### Measured Gains
| Config | Typical Gain | Source |
|--------|--------------|--------|
| `opt-level=3` + `lto=fat` + `codegen-units=1` | +20-30% over default release | [rustc book](https://doc.rust-lang.org/rustc/profile-guided-optimization.html) |
| `panic=abort` | -5-15% binary size, avoids unwind tables | [Perf Book](https://nnethercote.github.io/perf-book/build-configuration.html) |
| `target-cpu=native` (x86-64-v3) | +10-25% SIMD-heavy code | [ParrotOS](https://parrotsec.org/docs/configuration/parrot-optimized-builds/) |
| `target-cpu=apple-m1` / `armv8.2-a+dotprod+fp16` | +15-30% on Apple Silicon / Snapdragon 8 Gen 2 | [multiversed](https://docs.rs/multiversed/latest/multiversed/) |

### Config Snippet (`.cargo/config.toml`)

```toml
[build]
rustflags = [
  "-C", "opt-level=3",
  "-C", "lto=fat",
  "-C", "codegen-units=1",
  "-C", "panic=abort",
  "-C", "strip=symbols",
]

# Per-platform target-cpu/target-feature
[target.aarch64-linux-android]
rustflags = ["-C", "target-feature=+v8.2a,+dotprod,+fp16,+rcpc,+aes,+sha2,+crc"]

[target.aarch64-apple-darwin]
rustflags = ["-C", "target-cpu=apple-m1"]

[target.x86_64-unknown-linux-gnu]
rustflags = ["-C", "target-cpu=x86-64-v3"]

[target.x86_64-pc-windows-msvc]
rustflags = ["-C", "target-cpu=x86-64-v3"]

[target.x86_64-apple-darwin]
rustflags = ["-C", "target-cpu=native"]
```

**Notes:**
- Snapdragon 8 Gen 2 (Kryo CPU) supports ARMv8.2-A + dotprod + fp16 + AES + SHA2 + CRC. rustc has no kryo300 CPU; the cores are Cortex-X3/A715/A710/A510, so tune with features, not one core's target-cpu: `target-feature=+dotprod,+fp16,+aes,+sha2,+crc`.
- Apple Silicon M1+ baseline includes NEON, CRC, RDM, dotprod, fp16, AES, SHA2, SHA3, FHM, FCMA. Use `target-cpu=apple-m1`.
- x86-64-v3 = AVX2 + FMA + BMI1/2 (Haswell 2013+, Zen 1 2017+). Avoid v4 (AVX-512) on consumer Intel (E-cores lack it).

---

## 2. PGO & BOLT

### What
Profile-Guided Optimization (PGO) uses runtime profiles to guide inlining, branch prediction, and code layout. BOLT is a post-link binary optimizer that reorders blocks/functions using `perf` profiles.

### Why
PGO adds 10-50% on top of LTO for hot paths. BOLT adds another 5-15%. Combined: 2-4x total improvement typical; 5-10x achievable with effort.

### Measured Gains
| Stage | Typical Improvement | Source |
|-------|---------------------|--------|
| Release (`-O3`) | 3-5x baseline | [rust-optimization](https://www.scribd.com/document/1023441484/rust-optimization) |
| + LTO | +20-30% | [rust-optimization](https://www.scribd.com/document/1023441484/rust-optimization) |
| + PGO | +10-50% | [tsrs benchmarks](https://github.com/oxc-project/tsrs/blob/main/notes/perf-pgo.md) |
| + BOLT | +5-15% | [Android toolchain](https://android-developers.googleblog.com/2023/12/faster-rust-toolchains-for-android.html) |

**tsrs real data (Apple M-series):** PGO cut CPU time 7-14%, instructions 13-14% across projects.

### Config Snippet

```toml
# Cargo.toml
[profile.release]
lto = "fat"
codegen-units = 1
panic = "abort"
strip = "symbols"

# For PGO: use cargo-pgo
# 1. cargo install cargo-pgo
# 2. cargo pgo instrument --release
# 3. Run instrumented binary on representative workloads (DJ mixing, stem separation, UI)
# 4. cargo pgo optimize --release
```

```bash
# BOLT post-link (Linux/aarch64 only)
# Requires: llvm-bolt, perf, --emit-relocs at link time
RUSTFLAGS="-C link-arg=-Wl,--emit-relocs" cargo pgo optimize --release
perf record -g -o perf.data ./target/release/djsugar <workload>
perf2bolt -p perf.data -o perf.fdata ./target/release/djsugar
llvm-bolt ./target/release/djsugar -o ./target/release/djsugar.bolt \
  -data=perf.fdata \
  -reorder-blocks=ext-tsp \
  -reorder-functions=hfsort \
  -split-functions \
  -split-all-cold \
  -split-eh \
  -dyno-stats
```

**Android note:** Google uses BOLT on rustc toolchain (~24.7% speedup, ~10.9% size increase). For apps, BOLT works on ELF (Linux/Android NDK), not macOS/Windows.

---

## 3. Linker Choice

### What
`mold` (Rust), `lld` (LLVM), `wild` (new parallel linker), system `ld`.

### Why
Link time dominates debug builds. For release, mold is 2-16x faster than lld. Binary size difference is negligible (<1%).

### Measured Gains (August 2026 benchmarks, mold v3.0)
| Program | lld | mold | Speedup |
|---------|-----|------|---------|
| Chromium 145 (4.5 GiB) | 16.64s | 1.65s | **10.1x** |
| Chromium 145 ARM64 (4.76 GiB) | 20.91s | 1.83s | **11.4x** |
| TensorFlow 2.21 (9.55 GiB) | 50.73s | 3.15s | **16.1x** |
| Firefox 149 ARM64 (2.43 GiB) | 6.40s | 0.93s | **6.9x** |

Source: [mold README](https://github.com/rui314/mold)

### Config Snippet

```toml
# .cargo/config.toml
[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]

[target.aarch64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]

[target.x86_64-pc-windows-msvc]
# mold doesn't support MSVC COFF; use lld-link
rustflags = ["-C", "link-arg=-fuse-ld=lld"]

[target.aarch64-linux-android]
# NDK uses lld by default; mold not available
```

```bash
# Install mold
cargo install mold --locked  # or distro package
# Verify
mold --version
```

---

## 4. Allocator Choice

### What
Global allocator replacement. Options: system, jemalloc (tikv-jemallocator), mimalloc (C), rusty_alloc (pure Rust), snmalloc (C).

### Why
Default system allocator is conservative. jemalloc/mimalloc reduce fragmentation, improve cache locality. **mimalloc is C** — prefer pure-Rust `rusty_alloc` for supply-chain safety and WASM/embedded support.

### Measured Gains (rusty_alloc vs alternatives)
| Platform | vs mimalloc | vs jemalloc | vs system | Source |
|----------|-------------|-------------|-----------|--------|
| Linux servers | 0.52-0.83x allocator instr | 0.84-0.98x whole program | 0.81-0.99x | [rusty_alloc](https://github.com/Remade-With-Rust/rusty_alloc) |
| Rust apps (dedicated fast path) | — | — | **up to 27% fewer whole-program instr** | [rusty_alloc](https://github.com/Remade-With-Rust/rusty_alloc) |
| WASM (churn) | — | — | **4.9-7.3x faster** | [rusty_alloc](https://github.com/Remade-With-Rust/rusty_alloc) |
| ESP32-S3 | — | — | **2.1-3.9x faster** | [rusty_alloc](https://github.com/Remade-With-Rust/rusty_alloc) |

**Trade-off:** rusty_alloc retains freed memory (like mimalloc). Set `RUSTY_ALLOC_PURGE_DELAY` to return to OS.

### Config Snippet

```toml
# Cargo.toml
[dependencies]
rusty_alloc = "2.2"  # pure Rust, unified across all targets
# OR for Linux/macOS only (C-based, mature):
# tikv-jemallocator = { version = "0.7", target = "'cfg(not(target_env = \"msvc\"))'" }
```

```rust
// main.rs or lib.rs
#[global_allocator]
static GLOBAL: rusty_alloc::GlobalAlloc = rusty_alloc::GlobalAlloc::new();

// Optional: tune purge delay (ms) to return memory to OS
// std::env::set_var("RUSTY_ALLOC_PURGE_DELAY", "1000");
```

**For audio thread:** Consider a dedicated arena/bump allocator (e.g., `bumpalo`) for per-callback temporary buffers to avoid global allocator contention entirely.

---

## 5. Binary Size

### What
Strip symbols, LTO, panic=abort, opt-level=z/s, remove debug info.

### Why
Smaller binaries = faster cold start, less disk I/O, better instruction cache utilization.

### Measured Gains
| Technique | Size Reduction | Source |
|-----------|----------------|--------|
| `strip=symbols` + `lto=fat` | 30-50% | [Perf Book](https://nnethercote.github.io/perf-book/build-configuration.html) |
| `panic=abort` | 5-15% | [Perf Book](https://nnethercote.github.io/perf-book/build-configuration.html) |
| `opt-level=z` | 10-20% vs `opt-level=3` | rustc docs |
| `cargo strip` (post-build) | Additional 5-10% | |

### Config Snippet

```toml
# Cargo.toml - minimal size profile
[profile.release-small]
inherits = "release"
opt-level = "z"
lto = "fat"
codegen-units = 1
panic = "abort"
strip = "symbols"
debug = false

# Build: cargo build --profile release-small
```

```bash
# Post-build strip (if not using strip=symbols)
cargo install cargo-strip
cargo strip --release
```

---

## 6. SIMD Strategy

### What
Portable SIMD via `std::simd` (stable since 1.77) or `core::arch` intrinsics, with runtime dispatch via `multiversion`/`multiversed`.

### Why
`std::simd` is portable, type-safe, and auto-vectorizes. `core::arch` gives absolute control. Runtime dispatch lets you ship one binary that picks optimal impl per CPU.

### Measured Gains
| Approach | Throughput Gain (f32 DSP) | Source |
|----------|---------------------------|--------|
| `std::simd` / `packed_simd` | 4-8x scalar | [phosphor](https://github.com/joshjetson/phosphor/blob/main/architect.md) |
| `multiversed` (x86-64-v3 + aarch64-basic) | Near-native per arch | [multiversed](https://docs.rs/multiversed/latest/multiversed/) |
| Hand-written NEON (dotprod/fp16) | 2-4x scalar on ARM | [archmage](https://docs.rs/archmage/latest/aarch64-unknown-linux-gnu/src/archmage/detect.rs.html) |

### Config Snippet

```toml
# Cargo.toml
[dependencies]
stdsimd = { version = "0.1", features = ["stable"] }  # or use std::simd directly (Rust 1.77+)
multiversed = { version = "0.1", features = ["x86-64-v3", "aarch64-basic"] }
# For runtime feature detection without multiversioning:
archmage = "0.9"
```

```rust
// lib.rs - multiversioned dot product (audio mixing, gain, etc.)
use multiversed::multiversed;
use std::simd::{f32x8, SimdFloat};

#[multiversed]  // Uses cargo features: x86-64-v3, aarch64-basic by default
pub fn simd_dot(a: &[f32], b: &[f32]) -> f32 {
    let mut sum = f32x8::splat(0.0);
    for (chunk_a, chunk_b) in a.chunks_exact(8).zip(b.chunks_exact(8)) {
        sum += f32x8::from_slice(chunk_a) * f32x8::from_slice(chunk_b);
    }
    sum.reduce_sum() + a.chunks_exact(8).remainder().iter().zip(b.chunks_exact(8).remainder()).map(|(x,y)| x*y).sum::<f32>()
}

// For ARM-specific dotprod (int8) / fp16:
#[cfg(target_arch = "aarch64")]
#[multiversed("aarch64+neon+dotprod", "aarch64+neon+fp16")]
pub fn simd_dot_fp16(a: &[u16], b: &[u16]) -> f32 {
    // Use core::arch::aarch64::* for vdotq_f32, vcvtn_f16_f32, etc.
    unimplemented!()
}
```

**Runtime dispatch:** `multiversed` emits `#[multiversion]` attributes; dispatch is automatic via `multiversion` crate. No manual `is_x86_feature_detected!` needed.

**Compile-time fallback:** For `#[no_std]` or debugging, disable with `features = ["force-disable"]`.

---

## 7. Audio Thread: Priority & Realtime APIs

### What
OS-specific APIs to elevate audio callback thread to realtime/high priority, avoiding preemption by UI/GC.

### Why
Audio glitches (xruns) occur when callback misses deadline. Realtime priority + pinned core = deterministic latency.

### Per-OS Config

| OS | API | Priority Level | Source |
|----|-----|----------------|--------|
| **Android** | `AAudio` callback + `AAUDIO_PERFORMANCE_MODE_LOW_LATENCY` + `AAUDIO_SHARING_MODE_EXCLUSIVE` | `ANDROID_PRIORITY_AUDIO` (via JNI) | [AAudio docs](https://developer.android.com/ndk/guides/audio/aaudio/aaudio) |
| **Linux** | `pthread_setschedparam` + `SCHED_FIFO` + `rtkit` (desktop) | 95-99 (rtkit limits) | [Kernel docs](https://source.android.com/docs/core/perf/performance-hint-api) |
| **Windows** | `AvSetMmThreadCharacteristics` / `AvSetMmThreadPriority` (MMCSS) | "Pro Audio" task | [Microsoft Learn](https://learn.microsoft.com/windows/win32/api/avrt/nf-avrt-avsetmmthreadcharacteristicsa) |
| **macOS** | `pthread_set_qos_class_self_np` + `QOS_CLASS_USER_INTERACTIVE` / `os_workgroup` | User Interactive | [Apple docs](https://developer.apple.com/documentation/kernel/1387446-thread_policy_set) |

### Config Snippets

**Android (AAudio + Performance Hint):**
```c
// NDK C++ / JNI
AAudioStreamBuilder_setPerformanceMode(builder, AAUDIO_PERFORMANCE_MODE_LOW_LATENCY);
AAudioStreamBuilder_setSharingMode(builder, AAUDIO_SHARING_MODE_EXCLUSIVE);
AAudioStreamBuilder_setCallback(builder, audio_callback, user_data);
// Buffer size = 2 * burst size (double-buffer)
int32_t burst = AAudioStream_getFramesPerBurst(stream);
AAudioStream_setBufferSizeInFrames(stream, burst * 2);

// ADPF Performance Hint Session (API 31+)
APerformanceHintManager* mgr = APerformanceHint_getManager();
int32_t tids[1] = { gettid() };
int64_t target_ns = 1_000_000_000 / 48;  // 48 FPS = ~20.8ms; audio callback ~2-5ms
APerformanceHintSession* session = APerformanceHint_createSession(mgr, tids, 1, target_ns);

// Per frame:
APerformanceHint_reportActualWorkDuration(session, actual_ns);
APerformanceHint_updateTargetWorkDuration(session, target_ns);
```

**Linux (rtkit + SCHED_FIFO):**
```rust
// Requires CAP_SYS_NICE or rtkit DBus
use nix::sched::{sched_setscheduler, SchedParam, SchedPolicy::SCHED_FIFO};
use nix::unistd::gettid;

fn setup_realtime_audio() -> Result<(), nix::Error> {
    let param = SchedParam::new(90);  // rtkit typically allows up to 95
    unsafe { sched_setscheduler(gettid(), SCHED_FIFO, param) }?;
    Ok(())
}
```

**Windows (MMCSS):**
```rust
// windows-rs crate
use windows::Win32::Media::Audio::{
    AvSetMmThreadCharacteristicsA, AvSetMmThreadPriority, AVRT_PRIORITY_CRITICAL
};

fn setup_realtime_audio() {
    let mut task_index = 0;
    let handle = unsafe { AvSetMmThreadCharacteristicsA(b"Pro Audio\0".as_ptr() as _, &mut task_index) };
    if !handle.is_null() {
        unsafe { AvSetMmThreadPriority(handle, AVRT_PRIORITY_CRITICAL) };
    }
}
```

**macOS (QoS + Workgroup):**
```rust
// pthread QoS
use libc::{pthread_set_qos_class_self_np, qos_class_t, QOS_CLASS_USER_INTERACTIVE};

fn setup_realtime_audio() {
    unsafe { pthread_set_qos_class_self_np(QOS_CLASS_USER_INTERACTIVE, 0) };
}

// For audio workgroup (Core Audio):
// Use AVAudioEngine / AudioUnit with `kAudioUnitRenderAction_PostRender`
// and `pthread_workgroup_join_np` for worker threads.
```

---

## 8. CPU Affinity on big.LITTLE

### What
Pin audio thread to performance (big) cores; isolate from efficiency (LITTLE) cores.

### Why
big.LITTLE (Snapdragon 8 Gen 2: 1x Cortex-X3 + 2x Cortex-A715 + 2x Cortex-A710 + 3x Cortex-A510) — scheduling audio on LITTLE cores causes xruns.

### Measured Gains
- Pinning to big cores reduces worst-case latency 3-5x on Android.
- ADPF Performance Hint + thermal API lets OS migrate appropriately.

### Config Snippet

**Android (via ADPF + sched_setaffinity):**
```c
// Get CPU topology: /sys/devices/system/cpu/cpu*/topology/core_id, core_type
// Pin audio thread to performance cores (core_type = "performance")
cpu_set_t mask;
CPU_ZERO(&mask);
// Snapdragon 8 Gen 2: CPUs 0-4 are perf (X3 + A715x2 + A710x2), 5-7 are efficiency
for (int cpu = 0; cpu < 5; ++cpu) CPU_SET(cpu, &mask);
sched_setaffinity(0, sizeof(mask), &mask);

// Combine with ADPF hint session so OS knows workload
```

**Linux (taskset / cpusets):**
```bash
# Reserve CPUs 0-3 for audio (assuming 8-core big.LITTLE)
cset shield --cpu=0-3 --kthread=on
cset proc --exec -- /path/to/djsugar --audio-thread-cpus=0-3
```

**Rust (cross-platform affinity):**
```rust
use core_affinity::{set_for_current, get_core_ids};

fn pin_audio_thread(perf_core_ids: &[usize]) {
    if let Some(id) = perf_core_ids.first() {
        set_for_current(*id);
    }
}

// Detect big cores at startup (Linux/Android):
fn detect_perf_cores() -> Vec<usize> {
    // Parse /sys/devices/system/cpu/cpu*/topology/core_type
    // "performance" vs "efficiency"
    // Fallback: assume first N cores are big
}
```

---

## 9. Denormals (FTZ/DAZ)

### What
Flush-to-Zero (FTZ) and Denormals-Are-Zero (DAZ) CPU flags to prevent subnormal FP slowdown (100x+ on x86).

### Why
Recursive filters, reverb tails, decay envelopes generate subnormals when signal goes quiet. On x86, this spikes CPU from 1% to 90%.

### Measured Gains
- x86: **100x speedup** on denormal-heavy code (reverb tails, IIR filters).
- ARM (aarch64): FTZ enabled by default on most cores; no action needed.

### Config Snippet

**x86_64 (per audio thread, RAII guard):**
```rust
// Place at TOP of audio callback, or spawn-time for dedicated audio thread
#[cfg(target_arch = "x86_64")]
mod ftz {
    use std::arch::x86_64::{_mm_getcsr, _mm_setcsr, _MM_FLUSH_ZERO_ON, _MM_SET_FLUSH_ZERO_MODE};

    pub struct DenormalGuard(u32);

    impl DenormalGuard {
        pub fn new() -> Self {
            unsafe {
                // FTZ (bit 15) + DAZ (bit 6) = 0x8040
                let csr = _mm_getcsr();
                _mm_setcsr(csr | 0x8040);
                // Also set flush-zero mode for SSE/AVX
                _MM_SET_FLUSH_ZERO_MODE(_MM_FLUSH_ZERO_ON);
                DenormalGuard(csr)
            }
        }
    }

    impl Drop for DenormalGuard {
        fn drop(&mut self) {
            unsafe { _mm_setcsr(self.0); }
        }
    }
}

// Usage in audio callback:
fn audio_callback(&mut self, buffer: &mut [f32]) {
    #[cfg(target_arch = "x86_64")]
    let _guard = ftz::DenormalGuard::new();
    // ... DSP processing
}
```

**ARM64 (no action needed — FPCR.FZ=1 by default on Apple/Qualcomm):**
```rust
#[cfg(target_arch = "aarch64")]
mod ftz {
    pub struct DenormalGuard;
    impl DenormalGuard { pub fn new() -> Self { DenormalGuard } }
    impl Drop for DenormalGuard { fn drop(&mut self) {} }
}
```

**Alternative (manual flush in filter state):**
```rust
// In each IIR/biquad filter, after processing block:
fn flush_denormals(state: &mut [f32]) {
    for x in state {
        if x.is_subnormal() { *x = 0.0; }
    }
}
```

---

## 10. Buffer Sizes & Latency Targets

### What
Audio callback buffer size (frames), sample rate, target latency per platform.

### Why
Smaller buffers = lower latency but higher xrun risk. Target: **<10ms round-trip** for DJ use (scratching, cueing).

### Measured Targets
| Platform | API | Buffer Frames | Sample Rate | Latency (ms) | Source |
|----------|-----|---------------|-------------|--------------|--------|
| Android (AAudio) | LowLatency + Exclusive | 2 * burst (typically 192-240) | 48kHz | **4-5ms** | [AAudio](https://developer.android.com/ndk/guides/audio/aaudio/aaudio) |
| Linux (PipeWire/JACK) | Pro Audio | 128-256 | 48/96kHz | **2.7-5.3ms** | [PipeWire](https://pipewire.org) |
| Windows (WASAPI Exclusive) | Event-driven | 128-256 | 48/96kHz | **2.7-5.3ms** | [WASAPI](https://learn.microsoft.com/windows/win32/coreaudio/wasapi) |
| macOS (Core Audio) | AudioUnit / AVAudioEngine | 128-256 | 48/96kHz | **2.7-5.3ms** | [Core Audio](https://developer.apple.com/documentation/coreaudio) |

**Android burst sizes:** Typically 96, 128, 192, 240 frames. Use `AAudioStream_getFramesPerBurst() * 2`.

### Config Snippet

```rust
// Cross-platform buffer config
pub struct AudioConfig {
    pub sample_rate: u32,      // 48000 or 96000
    pub buffer_frames: u32,    // Platform-specific
    pub num_channels: u32,     // 2 (stereo)
}

impl AudioConfig {
    pub fn for_platform() -> Self {
        #[cfg(target_os = "android")]
        {
            // Query at runtime via AAudio
            AudioConfig { sample_rate: 48000, buffer_frames: 240, num_channels: 2 }
        }
        #[cfg(target_os = "linux")]
        {
            AudioConfig { sample_rate: 48000, buffer_frames: 256, num_channels: 2 }
        }
        #[cfg(target_os = "windows")]
        {
            AudioConfig { sample_rate: 48000, buffer_frames: 256, num_channels: 2 }
        }
        #[cfg(target_os = "macos")]
        {
            AudioConfig { sample_rate: 48000, buffer_frames: 256, num_channels: 2 }
        }
    }

    pub fn latency_ms(&self) -> f32 {
        (self.buffer_frames as f32 / self.sample_rate as f32) * 1000.0
    }
}
```

**Latency tuning loop (Android AAudio):**
```c
int32_t prev_xruns = 0;
int32_t burst = AAudioStream_getFramesPerBurst(stream);
int32_t buf_size = burst * 2;
while (running) {
    int32_t xruns = AAudioStream_getXRunCount(stream);
    if (xruns > prev_xruns) {
        buf_size += burst;
        AAudioStream_setBufferSizeInFrames(stream, buf_size);
        prev_xruns = xruns;
    }
    // If stable for N callbacks, try reducing
}
```

---

## 11. Allocation/Lock-Free Callbacks

### What
Zero allocation, zero locks in audio callback. Use pre-allocated ring buffers, lock-free SPSC queues, object pools.

### Why
Any allocation/lock in callback = priority inversion risk = xrun.

### Patterns

| Pattern | Crate | Use Case |
|---------|-------|----------|
| Lock-free SPSC ring | `crossbeam-channel` (bounded), `heapless::spsc`, `ringbuf` | Audio → UI, UI → Audio |
| Bump arena per callback | `bumpalo` | Temp DSP buffers |
| Object pool | `object_pool`, `pooling` | Reusable filter states |
| Pre-allocated vec | `Vec::with_capacity` + `set_len` | Output buffers |

### Config Snippet

```rust
// Lock-free SPSC for audio → analysis worker
use crossbeam_channel::bounded;
use std::sync::Arc;

pub struct AudioEngine {
    // SPSC: audio thread produces, analysis thread consumes
    analysis_tx: crossbeam_channel::Sender<AnalysisFrame>,
    analysis_rx: crossbeam_channel::Receiver<AnalysisFrame>,

    // Pre-allocated output buffer (no alloc in callback)
    output_buffer: Vec<f32>,

    // Bump arena for per-callback temp allocations
    arena: bumpalo::Bump,
}

impl AudioEngine {
    pub fn new() -> Self {
        let (tx, rx) = bounded(64);  // 64 frames max queue
        Self {
            analysis_tx: tx,
            analysis_rx: rx,
            output_buffer: vec![0.0; 4096],  // max buffer size
            arena: bumpalo::Bump::new(),
        }
    }

    pub fn audio_callback(&mut self, out: &mut [f32]) {
        // Zero alloc: reuse pre-allocated buffer
        let buf = &mut self.output_buffer[..out.len()];

        // Bump arena for temp DSP (auto-reset per callback)
        let arena = &self.arena;
        let temp_vec: &mut [f32] = arena.alloc_slice_fill_copy(out.len(), 0.0);

        // ... DSP processing using temp_vec ...

        // Non-blocking send to analysis (try_send)
        let _ = self.analysis_tx.try_send(AnalysisFrame {
            rms: compute_rms(buf),
            peak: compute_peak(buf),
        });

        out.copy_from_slice(buf);
        arena.reset();  // O(1), no dealloc
    }
}
```

**Never do in callback:**
- `Vec::push`, `Box::new`, `String::from`
- `Mutex::lock`, `RwLock::read`, `channel.recv()`
- File I/O, `println!`, `log::info!`
- `std::thread::spawn`

---

## 12. Android ADPF & Thermal

### What
Android Dynamic Performance Framework: Performance Hint API + Thermal API + Game Mode.

### Why
Prevents thermal throttling by telling OS workload duration + monitoring headroom. Enables sustained performance.

### Measured Gains
- MediaTek: **12% power reduction**, **25 min longer sessions** [MediaTek](https://developer.android.com/stories/games/mediatek-adpf)
- UNISOC: Stabilizes frame rate during intense sessions [UNISOC](https://developer.android.com/stories/games/unisoc-adpf)

### API Levels
| API | Min SDK | Feature |
|-----|---------|---------|
| Thermal API | 30 (Android 11) | `getThermalHeadroom()` |
| Performance Hint | 31 (Android 12) | `PerformanceHintManager` |
| Power Efficiency Mode | 35 (Android 15) | `setPowerEfficiencyMode()` |

### Config Snippet

```rust
// Rust JNI bindings for ADPF (use jni crate)
#[cfg(target_os = "android")]
mod adpf {
    use jni::JNIEnv;
    use jni::objects::{JClass, JObject};
    use jni::sys::{jlong, jint, jboolean};

    // Thermal headroom (seconds until throttle)
    pub fn get_thermal_headroom(env: &mut JNIEnv, _: JClass) -> f32 {
        let power_mgr = env.get_system_service("power").unwrap();
        let headroom: f32 = env.call_method(power_mgr, "getThermalHeadroom", "()F", &[]).unwrap().f().unwrap();
        headroom  // e.g., 45.0 = 45 seconds at current load
    }

    // Create performance hint session
    pub fn create_hint_session(env: &mut JNIEnv, _: JClass, target_fps: jint) -> jlong {
        let mgr = env.call_static_method(
            "android/os/PerformanceHintManager", "getManager", "()Landroid/os/PerformanceHintManager;", &[]
        ).unwrap().l().unwrap();
        let tids = env.new_int_array(1).unwrap();
        env.set_int_array_region(&tids, 0, &[std::thread::current().id().as_u64() as i32]).unwrap();
        let target_ns = (1_000_000_000i64 / target_fps as i64) as jlong;
        let session = env.call_method(mgr, "createHintSession", "([IJ)Landroid/os/PerformanceHintManager$Session;", &[tids.into(), target_ns.into()]).unwrap().l().unwrap();
        env.new_global_ref(session).unwrap().as_raw() as jlong
    }

    // Report actual frame duration
    pub fn report_frame_duration(env: &mut JNIEnv, _: JClass, session_ref: jlong, actual_ns: jlong) {
        let session = unsafe { JObject::from_raw(session_ref as _) };
        env.call_method(session, "reportActualWorkDuration", "(J)V", &[actual_ns.into()]).unwrap();
    }
}

// Usage in audio callback (48kHz, 256 frames = 5.33ms target):
fn audio_callback(&mut self, out: &mut [f32]) {
    let start = std::time::Instant::now();
    // ... process ...
    let actual_ns = start.elapsed().as_nanos() as i64;

    #[cfg(target_os = "android")]
    {
        adpf::report_frame_duration(&mut ndk_context::jnienv(), 0, self.hint_session, actual_ns);
    }

    // If thermal headroom < 2x frame time, reduce quality (lower FFT size, fewer stems)
    #[cfg(target_os = "android")]
    if adpf::get_thermal_headroom(&mut ndk_context::jnienv(), 0) < 0.010 {  // 10ms
        self.reduce_quality();
    }
}
```

**Manifest:**
```xml
<!-- AndroidManifest.xml -->
<uses-permission android:name="android.permission.FOREGROUND_SERVICE" />
<uses-permission android:name="android.permission.FOREGROUND_SERVICE_DATA_SYNC" />
<!-- For thermal: no extra permission needed -->
```

---

## 13. GPU: wgpu Backend & Mobile Optimization

### What
wgpu backend selection, present mode, frame pacing, tile-based rendering (TBDR) optimization for Adreno/Mali/Apple GPU.

### Why
Mobile GPUs (Tile-Based Deferred Rendering) need different patterns: avoid overdraw, use transient attachments, fuse passes, minimize DRAM traffic.

### Backend Priority per Platform
| Platform | Preferred Backend | Fallback |
|----------|-------------------|----------|
| Android | Vulkan (Adreno/Mali) | GLES3 |
| Linux | Vulkan | DX12 (WSL), Metal (MoltenVK) |
| Windows | DX12 | Vulkan |
| macOS | Metal | Vulkan (MoltenVK) |

### Present Modes (wgpu)
| Mode | Tearing | Latency | Best For |
|------|---------|---------|----------|
| `Fifo` (VSync) | No | 1-2 frames | Default, battery |
| `Mailbox` | No | 1 frame | Low latency, no tear |
| `Immediate` | Yes | 0 frames | Benchmarking only |
| `AutoVsync` / `AutoNoVsync` | Auto | Auto | Cross-platform |

**For DJ app (redraw-on-demand):** Use `PresentMode::AutoNoVsync` → falls back to `Immediate` (no queue) or `Mailbox`. Only call `request_redraw()` on UI change.

### Tile-Based Optimization (Adreno/Mali/Apple)

| Technique | Effect | Source |
|-----------|--------|--------|
| Transient attachments (VK_LAZILY_ALLOCATED / MTLStorageMode::Memoryless) | Keeps G-buffer in tile memory, no DRAM round-trip | [wgpu-tiled](https://github.com/infosia/wgpu) |
| Multi-subpass render passes | Depth/normal/color stay on-chip across subpasses | [wgpu-tiled](https://github.com/infosia/wgpu) |
| Input attachments (`subpass_input` in WGSL) | Read previous subpass at fragment pos, no texture sample | [wgpu-tiled](https://github.com/infosia/wgpu) |
| Fuse passes (compute + fragment) | Eliminate intermediate buffer write+read | [Vulkan mobile](https://vulkan.lunarg.com/doc/view/latest/windows/antora/tutorial/latest/Advanced_Vulkan_Compute/12_Mobile_and_Embedded_Compute/03_mobile_optimization.html) |
| FP16 storage (RG16F, R11G11B10F) | Half bandwidth, half power | [Vulkan mobile](https://vulkan.lunarg.com/doc/view/latest/windows/antora/tutorial/latest/Advanced_Vulkan_Compute/12_Mobile_and_Embedded_Compute/03_mobile_optimization.html) |

### Config Snippet

```rust
// wgpu instance + surface config
use wgpu::{Instance, Backends, PowerPreference, PresentMode, SurfaceConfiguration};

pub fn create_instance() -> Instance {
    Instance::new(wgpu::InstanceDescriptor {
        backends: Backends::PRIMARY,  // Vulkan/Metal/DX12
        flags: wgpu::InstanceFlags::default(),
        backend_options: wgpu::BackendOptions::default(),
    })
}

pub fn configure_surface(
    surface: &wgpu::Surface,
    device: &wgpu::Device,
    size: winit::dpi::PhysicalSize<u32>,
    is_mobile: bool,
) -> SurfaceConfiguration {
    let caps = surface.get_capabilities(device);
    let format = caps.formats[0];  // prefer sRGB

    // Present mode: AutoNoVsync for redraw-on-demand (lowest latency when drawing)
    let present_mode = if is_mobile {
        PresentMode::AutoNoVsync  // Falls back to Immediate or Mailbox
    } else {
        PresentMode::AutoVsync
    };

    SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: size.width,
        height: size.height,
        present_mode,
        alpha_mode: caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: if is_mobile { 1 } else { 2 },  // Min latency on mobile
    }
}

// Redraw-on-demand (winit)
fn request_redraw_when_needed(window: &winit::window::Window, needs_redraw: bool) {
    if needs_redraw {
        window.request_redraw();
    }
    // No continuous render loop
}
```

**Mobile shader tips (WGSL):**
```wgsl
// Use FP16 where precision allows (mobile bandwidth saver)
@group(0) @binding(0) var<storage, read> positions: array<vec2<f16>>;
@group(0) @binding(1) var<storage, read> colors: array<vec4<f16>>;

// Transient attachment pattern (requires wgpu-tiled fork or WebGPU extension)
// Standard wgpu: use resolve targets + store_op = StoreOp::Discard for intermediates
```

---

## 14. Memory: mmap & Cache Layouts

### What
Memory-map decoded audio files (zero-copy), cache layout computations, use `memmap2` for cross-platform mmap.

### Why
Avoids double-buffering (disk → page cache → app buffer). Enables random access for scratching/seeking. Reduces startup I/O.

### Measured Gains
- `memmap2` + `symphonia` (via `oxiaudio-decode` mmap feature): Zero-copy decode, OS handles paging.
- Chunked mmap (64MB pages, 1GB segments) scales to large libraries.

### Config Snippet

```toml
# Cargo.toml
[dependencies]
memmap2 = "0.9"
oxiaudio-decode = { version = "0.2", features = ["mmap"] }  # symphonia-backed, pure Rust
# OR raw symphonia + memmap2
symphonia = { version = "0.5", features = ["vorbis", "flac", "mp3", "opus", "aac"] }
```

```rust
use memmap2::Mmap;
use std::fs::File;
use oxiaudio_decode::{decode_file_mmap, AudioBuffer};

pub struct MappedAudioFile {
    mmap: Mmap,
    buffer: AudioBuffer<f32>,  // decoded interleaved f32
    sample_rate: u32,
    channels: u16,
}

impl MappedAudioFile {
    pub fn open(path: &std::path::Path) -> Result<Self, std::io::Error> {
        let file = File::open(path)?;
        // SAFETY: file lives as long as mmap
        let mmap = unsafe { Mmap::map(&file)? };

        // Decode directly from mmap (zero-copy read, symphonia reads from &[u8])
        let buffer = decode_file_mmap(path)  // requires "mmap" feature
            .map_err(|e| std::io::Error::other(e))?;

        Ok(Self {
            sample_rate: buffer.sample_rate,
            channels: buffer.channels as u16,
            buffer,
            mmap,
        })
    }

    // Random access for scratching: O(1) slice
    pub fn slice(&self, start_frame: usize, frames: usize) -> &[f32] {
        let start = start_frame * self.channels as usize;
        let end = start + frames * self.channels as usize;
        &self.buffer.data[start..end]
    }
}

// Cache layout computations (e.g., waveform overviews, beat grids)
pub struct LayoutCache {
    // Key: (file_id, zoom_level) -> Vec<f32> (min/max per pixel)
    cache: dashmap::DashMap<(u64, u32), Vec<f32>>,
}

impl LayoutCache {
    pub fn get_or_compute(&self, file_id: u64, zoom: u32, compute: impl FnOnce() -> Vec<f32>) -> Vec<f32> {
        self.cache.entry((file_id, zoom)).or_insert_with(compute).clone()
    }
}
```

**Startup:** Spawn background worker to mmap + decode on-demand (priority queue: visible tracks first).

---

## 15. Startup Time

### What
Cold-start latency from process launch to first audio callback + UI frame.

### Why
DJ app must be ready instantly. Target: **<500ms** cold start on modern devices.

### Techniques & Gains
| Technique | Gain | Source |
|-----------|------|--------|
| `lto=fat` + `codegen-units=1` | 20-30% smaller, faster init | rustc |
| `strip=symbols` + `panic=abort` | 10-20% smaller | Perf Book |
| Lazy initialization (defer DB, library scan) | 50-80% perceived | — |
| Pre-warmed mmap (background thread) | Parallel I/O | — |
| `cargo-pgo` profile includes startup | 10-15% | PGO data |
| Minimize dynamic linking (static musl on Linux) | Avoids ld.so | — |

### Config Snippet

```rust
// main.rs - lazy initialization pattern
use once_cell::sync::Lazy;
use std::sync::Arc;

static AUDIO_ENGINE: Lazy<Arc<AudioEngine>> = Lazy::new(|| {
    // This runs on FIRST access, not at startup
    Arc::new(AudioEngine::new())
});

static LIBRARY_INDEX: Lazy<LibraryIndex> = Lazy::new(|| {
    // Defer to background thread after first frame
    LibraryIndex::new_empty()
});

fn main() {
    // 1. Minimal work before first frame
    let app = App::new();
    app.run();  // Shows UI immediately

    // 2. Spawn background initialization
    std::thread::spawn(|| {
        // Warm up audio engine (triggers JIT, allocates pools)
        let _ = AUDIO_ENGINE.warmup();

        // Build library index incrementally
        LIBRARY_INDEX.build_incremental(|progress| {
            // Send progress to UI via channel
        });
    });
}
```

```toml
# Cargo.toml - startup profile
[profile.release-startup]
inherits = "release"
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = "symbols"
# Split debug info for faster link (optional)
split-debuginfo = "packed"
```

---

## Summary Checklist

| Category | Done? | Verification |
|----------|-------|--------------|
| Compiler flags per platform | ☐ | `cargo build --release --target=... && cargo size` |
| PGO + BOLT pipeline | ☐ | `hyperfine` before/after on DJ workload |
| mold linker (Linux/Android) | ☐ | `time cargo build` comparison |
| rusty_alloc global allocator | ☐ | `heaptrack` / `alloc-trace` |
| Binary size < 10MB (striped) | ☐ | `ls -lh target/release/djsugar` |
| SIMD multiversioned (x86-64-v3 + aarch64-basic) | ☐ | `cargo asm` verify vectors |
| Audio thread realtime priority | ☐ | `chrt -p <pid>` / `SCHED_FIFO` |
| big.LITTLE pin to perf cores | ☐ | `cat /proc/<pid>/status` Cpus_allowed |
| FTZ/DAZ guard in callback | ☐ | `perf stat -e fp_ret_sse_avx_ops.all` |
| Buffer size = 2 * burst (Android) | ☐ | `AAudioStream_getFramesPerBurst` |
| Zero alloc/lock in callback | ☐ | `heaptrack` / manual audit |
| ADPF hint session + thermal | ☐ | `dumpsys thermal` / log headroom |
| wgpu AutoNoVsync + redraw-on-demand | ☐ | Frame time < 16ms, no continuous GPU |
| Transient attachments / fused passes | ☐ | RenderDoc / `adb shell dumpsys gfxinfo` |
| mmap audio + layout cache | ☐ | Startup I/O `strace -c` |
| Startup < 500ms cold | ☐ | `time ./target/release/djsugar --benchmark-startup` |

---

## References

1. **rustc optimization flags** — <https://doc.rust-lang.org/rustc/profile-guided-optimization.html>
2. **Perf Book: Build Configuration** — <https://nnethercote.github.io/perf-book/build-configuration.html>
3. **Android Rust Toolchain (PGO+BOLT)** — <https://android-developers.googleblog.com/2023/12/faster-rust-toolchains-for-android.html>
4. **mold Linker Benchmarks** — <https://github.com/rui314/mold>
5. **rusty_alloc Pure Rust Allocator** — <https://github.com/Remade-With-Rust/rusty_alloc>
6. **multiversed SIMD Presets** — <https://docs.rs/multiversed/latest/multiversed/>
7. **AAudio Low Latency** — <https://developer.android.com/ndk/guides/audio/aaudio/aaudio>
8. **ADPF Performance Hint API** — <https://source.android.com/docs/core/perf/performance-hint-api>
9. **ADPF Thermal API** — <https://developer.android.com/games/optimize/adpf/thermal>
10. **Denormal Handling (phosphor)** — <https://github.com/joshjetson/phosphor/blob/main/architect.md>
11. **wgpu-tiled Mobile TBDR** — <https://github.com/infosia/wgpu>
12. **Vulkan Mobile Optimization** — <https://vulkan.lunarg.com/doc/view/latest/windows/antora/tutorial/latest/Advanced_Vulkan_Compute/12_Mobile_and_Embedded_Compute/03_mobile_optimization.html>
13. **oxiaudio-decode mmap** — <https://docs.rs/oxiaudio-decode/latest/>
14. **BOLT Documentation** — <https://llvm.googlesource.com/llvm-project/+/refs/heads/main/bolt/README.md>
15. **ParrotOS Optimized Builds** — <https://parrotsec.org/docs/configuration/parrot-optimized-builds/>

---

*Generated for DJ Sugar (djsugar-next) — Snapdragon 8 Gen 2 / Samsung S23 Ultra primary target.*

---

## iOS / iPadOS Performance Optimization

### Compiler Flags (iOS Targets)

**Target-CPU per Apple SoC:**
| Device | SoC | Rustc Target-CPU | Baseline Features |
|--------|-----|------------------|-------------------|
| iPhone 14 Pro / 15 Pro / 15 Pro Max | A16 / A17 Pro | `apple-a16` / `apple-a17` (rustc 1.85+) | NEON, dotprod, fp16, AES, SHA2, CRC, RDM, RCPC |
| iPhone 13 / 14 / SE3 | A15 | `apple-a15` | Same |
| iPhone 12 | A14 | `apple-a14` | Same |
| All ARM64 iOS (fallback) | — | — | `+neon,+fp-armv8,+dotprod,+fp16,+aes,+sha2,+crc,+rcpc,+rdm` |

**`.cargo/config.toml` — iOS additions:**
```toml
[target.aarch64-apple-ios]
rustflags = [
  "-C", "target-cpu=apple-a16",  # or baseline features
  "-C", "lto=fat",
  "-C", "codegen-units=1",
  "-C", "panic=abort",
  "-C", "strip=symbols",
  "-C", "link-arg=-Wl,-dead_strip",
  "-C", "link-arg=-Wl,-no_uuid",
]

[target.aarch64-apple-ios-sim]
rustflags = [
  "-C", "target-cpu=native",  # Apple Silicon host
  "-C", "lto=fat",
  "-C", "codegen-units=1",
  "-C", "panic=abort",
  "-C", "strip=symbols",
  "-C", "link-arg=-Wl,-dead_strip",
]
```

**PGO on iOS:** `cargo-pgo` works cross-target. Profile on device/simulator via `cargo-dinghy` or TestFlight build. Instrument → run DJ workload (mixing, stems, UI) → optimize.

### Audio Thread: Realtime Priority + Audio Workgroups

**iOS/macOS (QoS + os_workgroup):**
```rust
// 1. Set thread QoS to User Interactive (highest)
unsafe { pthread_set_qos_class_self_np(QOS_CLASS_USER_INTERACTIVE, 0) };

// 2. Get device workgroup from RemoteIO AudioUnit
let mut workgroup: os_workgroup_t = std::ptr::null_mut();
let mut size = std::mem::size_of::<os_workgroup_t>();
AudioUnitGetProperty(
    remote_io_unit,
    kAudioOutputUnitProperty_OSWorkgroup,
    kAudioUnitScope_Global,
    0,
    &mut workgroup as *mut _ as *mut _,
    &mut size,
);

// 3. In aux DSP thread: join workgroup
let mut token = os_workgroup_join_token_s::default();
os_workgroup_join(workgroup, &mut token);
// ... DSP work ...
os_workgroup_leave(workgroup, &mut token);
```

**Key APIs (iOS 14+):**
- `kAudioOutputUnitProperty_OSWorkgroup` → `os_workgroup_t`
- `os_workgroup_join` / `os_workgroup_leave` (libc)
- `pthread_set_qos_class_self_np` with `QOS_CLASS_USER_INTERACTIVE`
- `AVAudioEngine` / `AVAudioSinkNode` for Swift-friendly path; same workgroup via `AUAudioUnit.osWorkgroup`

**Measured gains:** Workgroup-joined threads stay on P-cores, 10× fewer xruns at 1ms buffer (JUCE forum reports).

### Buffer Sizes & Latency Targets (iOS)

| API | Buffer Frames | Sample Rate | Latency (ms) | Notes |
|-----|---------------|-------------|--------------|-------|
| **RemoteIO / AVAudioEngine** | 128–512 (IOBufferDuration × SR) | 44.1/48kHz | **2.7–11.6ms** | `AVAudioSession.setPreferredIOBufferDuration(0.005)` → ~5ms |
| **AAudio (Android)** | 2 × burst | 48kHz | 4–5ms | Reference |

**iOS config snippet:**
```rust
pub struct AudioConfig {
    pub sample_rate: u32,
    pub buffer_frames: u32,
    pub num_channels: u32,
}

impl AudioConfig {
    pub fn for_platform() -> Self {
        #[cfg(target_os = "ios")]
        {
            // Query at runtime after AVAudioSession activation
            AudioConfig { sample_rate: 48000, buffer_frames: 256, num_channels: 2 }
        }
        #[cfg(target_os = "macos")]
        {
            AudioConfig { sample_rate: 48000, buffer_frames: 256, num_channels: 2 }
        }
        #[cfg(target_os = "android")]
        {
            AudioConfig { sample_rate: 48000, buffer_frames: 240, num_channels: 2 }
        }
        #[cfg(target_os = "linux")]
        {
            AudioConfig { sample_rate: 48000, buffer_frames: 256, num_channels: 2 }
        }
        #[cfg(target_os = "windows")]
        {
            AudioConfig { sample_rate: 48000, buffer_frames: 256, num_channels: 2 }
        }
    }
}
```

### Thermal Management (iOS 11+)

| API | Type | Values | Action Threshold |
|-----|------|--------|------------------|
| `ProcessInfo.thermalState` | Property (poll) | `.nominal` (0), `.fair` (1), `.serious` (2), `.critical` (3) | ≥ `.serious`: reduce quality |
| `ProcessInfoThermalStateDidChangeNotification` | Notification (push) | Same enum | Immediate reaction |

**Rust integration:**
```rust
use objc2_foundation::{ProcessInfo, NSNotificationCenter, MainThreadMarker, ns_string};

fn current_thermal_state() -> u8 {
    unsafe { ProcessInfo::processInfo().thermalState() as u8 }
}

// Background monitor thread (poll 2s)
std::thread::spawn(|| loop {
    std::thread::sleep(Duration::from_secs(2));
    match current_thermal_state() {
        2 => reduce_quality(QualityLevel::Medium),   // .serious
        3 => reduce_quality(QualityLevel::Low),      // .critical
        _ => {}
    }
});

// Or notification observer (main thread)
let mtm = MainThreadMarker::new().unwrap();
let nc = NSNotificationCenter::defaultCenter();
unsafe {
    nc.addObserverForName(
        Some(ns_string!("ProcessInfoThermalStateDidChangeNotification")),
        None, None, mtm.queue(),
        |_| { /* react immediately */ }
    );
}
```

**Quality reduction actions:**
- Lower FFT size (2048 → 1024)
- Disable stem separation inference
- Reduce UI frame rate (60 → 30 fps)
- Decrease oversampling factor

### Startup Time (iOS)

**Additional constraints vs Android:**
- Codesign + entitlements verification at launch (~50–100ms)
- `UIApplicationMain` initialization (Objective-C runtime)
- Metal device creation (`MTLCreateSystemDefaultDevice`)
- AVAudioSession activation (first audio callback)

**Target:** <800ms cold start (includes codesign verify).

**Optimizations:**
| Technique | Gain | iOS-Specific Notes |
|-----------|------|-------------------|
| `lto=fat` + `codegen-units=1` | 20–30% | Required for size + speed |
| `strip=symbols` + `panic=abort` | 10–20% | Standard |
| Lazy init (audio engine, library index) | 50–80% perceived | Defer until after first UI frame |
| Pre-warm mmap on background thread | Parallel I/O | Start in `applicationDidFinishLaunching` |
| `cargo-pgo` profile includes startup | 10–15% | Profile on device via TestFlight |
| Split debug info (`split-debuginfo=packed`) | Faster link | Xcode 15+ |

**main.rs pattern:**
```rust
use once_cell::sync::Lazy;

static AUDIO_ENGINE: Lazy<Arc<AudioEngine>> = Lazy::new(|| Arc::new(AudioEngine::new()));
static LIBRARY_INDEX: Lazy<LibraryIndex> = Lazy::new(LibraryIndex::new_empty);

#[cfg(target_os = "ios")]
fn main() {
    // Minimal work before first frame
    let app = App::new();
    app.run();  // Shows UI immediately via UIApplicationMain
    
    // Background warmup (spawned from AppDelegate equivalent)
    std::thread::spawn(|| {
        let _ = AUDIO_ENGINE.warmup();
        LIBRARY_INDEX.build_incremental(|p| send_to_ui(p));
    });
}
```

### GPU: Metal TBDR Optimization (Apple GPU)

| Technique | Effect | wgpu Support |
|-----------|--------|--------------|
| **Transient attachments** (`MTLStorageMode::Memoryless`) | Keeps G-buffer in tile memory, no DRAM round-trip | `wgpu-tiled` fork or `store_op = StoreOp::Discard` |
| **Multi-subpass render passes** | Depth/normal/color stay on-chip | `wgpu-tiled` / WebGPU extension |
| **FP16 storage** (`RG16F`, `R11G11B10F`) | Half bandwidth, half power | Standard wgpu: `TextureFormat::Rg16Float` |
| **Fused passes (compute + fragment)** | Eliminate intermediate buffer | Manual via `ComputePass` + `RenderPass` |
| **PresentMode::AutoNoVsync** | Lowest latency for redraw-on-demand | Standard wgpu |

**Mobile shader tips (WGSL):**
```wgsl
// Use FP16 where precision allows
@group(0) @binding(0) var<storage, read> positions: array<vec2<f16>>;
@group(0) @binding(1) var<storage, read> colors: array<vec4<f16>>;

// Discard intermediates (tile memory only)
@fragment fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f16> {
    // ...
}
```

### App Store Compliance (Performance-Related)

| Guideline | Requirement | Verification |
|-----------|-------------|--------------|
| **2.4.2** | No excessive battery/heat/background abuse | Profile with `Instruments` → Energy Log |
| **2.5.2** | No downloaded executable code | Binary audit: `otool -L`, `codesign -dvvv` |
| **2.5.4** | Background audio mode only for actual playback | `UIBackgroundModes:audio` + RemoteIO callback running in background |

### Verification Checklist (iOS Additions)

| Category | Done? | Verification |
|----------|-------|--------------|
| target-cpu=apple-a16 (or baseline features) | ☐ | `cargo build --release --target aarch64-apple-ios && cargo asm` |
| Audio thread QoS = UserInteractive | ☐ | `pthread_get_qos_class_self_np` |
| Audio workgroup joined (aux threads) | ☐ | `os_workgroup_join` returns 0 |
| IOBufferDuration ≤ 5ms (256 frames @ 48kHz) | ☐ | `AVAudioSession.ioBufferDuration` |
| Thermal monitor (ProcessInfo.thermalState) | ☐ | Simulate via `Instruments` → Thermal State |
| Metal transient attachments / FP16 textures | ☐ | `Instruments` → GPU / Metal System Trace |
| Background audio works (RemoteIO callback fires) | ☐ | Test: lock screen, audio continues |
| No JIT / no downloaded code | ☐ | `codesign -dvvv`, binary scan for `wasmtime`/`dlopen` |
| Startup < 800ms cold (device) | ☐ | `time xcrun simctl launch booted <bundle-id>` |