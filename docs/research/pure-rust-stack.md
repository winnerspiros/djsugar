# Pure-Rust Dependency Stack for DJ Sugar

Cross-platform (Android, Linux, Windows, macOS) DJ application. **Goal: zero C/C++ code compiled or dlopened beyond the unavoidable OS/GPU-driver ABI.**

## Current C/System Dependencies (Audit)

| Crate | Version | C Dependency | Build Deps | Why |
|-------|---------|--------------|------------|-----|
| `wayland-sys` | 0.31.11 | libwayland (dlopen) | `cc`, `pkg-config` | winit wayland feature |
| `x11-dl` | 2.21.0 | libX11 (dlopen) | — | winit x11 feature |
| `xkbcommon-dl` | 0.4.2 | libxkbcommon (dlopen) | — | winit x11 keymap |
| `ash` | 0.38.0 | Vulkan loader (dlopen) | — | wgpu-hal vulkan backend |
| `renderdoc-sys` | 1.1.0 | RenderDoc (dlopen) | — | wgpu-hal renderdoc feature |
| `libloading` | 0.8.9 | libdl (system) | — | ash `loaded` feature |
| `khronos-egl` | 6.0.0 | libEGL (dlopen) | `libloading` | wgpu-hal gles backend |

---

## Recommended Pure-Rust Stack by Layer

### 1. Windowing & Event Loop (Per-OS)

| OS | Crate | Version | Pure Rust | Maturity | Notes |
|----|-------|---------|-----------|----------|-------|
| **Linux (X11)** | `x11rb` | 0.10+ | ✅ | Stable | Pure-Rust X11 protocol (XCB replacement). No `libxcb`/`libX11`. Use `RustConnection`. |
| **Linux (Wayland)** | `wayland-client` + `wayland-protocols` | 0.31+ / 0.31+ | ✅ | Stable | Pure-Rust Wayland client (generated from XML). No `libwayland`. Pair with `smithay-client-toolkit` for higher-level abstractions. |
| **Linux (Keymap)** | `xkbcommon-rs` (no dl) / `keymap` | — | ✅ | Emerging | Parse XKB keymaps in Rust. Avoid `xkbcommon-dl`. |
| **Android** | `android-activity` + `ndk` | 0.5+ / 0.9+ | ✅ | Stable | JNI glue + NDK syscalls. `GameActivity`/`SurfaceView` for 120 Hz VSync. No `ndk-glue`. |
| **Windows** | `windows-sys` / `windows` | 0.58+ / 0.58+ | ✅ | Stable | Win32 API from metadata. `windows-sys` = `no_std` friendly, zero deps. |
| **macOS / iOS** | `objc2` + `objc2-foundation` + `objc2-appkit` / `objc2-uikit` | 0.6+ | ✅ | Stable | Objective-C runtime via Rust. `CAMetalLayer`, `NSWindow`, `NSTextInputClient`. |
| **Web (WASM)** | `web-sys` + `wasm-bindgen` | 0.3+ | ✅ | Stable | `HtmlCanvasElement`, `WebGl2RenderingContext`, `GpuDevice`. |

**Recommendation:** Replace `winit` with **per-platform shells** behind a narrow `PlatformWindow` trait (see `rune` architecture). winit's X11/Wayland backends pull `wayland-sys`/`x11-dl`/`xkbcommon-dl`; its Android backend pulls `android-activity` but with version-locked `ndk-glue`. Writing thin shells (~4–6k LOC each) avoids all `-sys` crates and gives first-class text input, live resize, and accessibility.

**Verification:**
```bash
cargo tree -p x11rb -p wayland-client -p android-activity -p windows-sys -p objc2 --edges features | grep -E "(sys|dl|cc|cmake|bindgen)" || echo "CLEAN"
```

---

### 2. GPU / Graphics (wgpu)

| Backend | OS | Crate | Pure Rust | Feature Flag | Notes |
|---------|----|-------|-----------|--------------|-------|
| **Vulkan** | Linux, Windows, Android | `ash` | ⚠️ dlopen only | `vulkan` | `ash` uses `libloading` to `dlopen` `libvulkan.so`/`vulkan-1.dll` — **unavoidable driver ABI**. Disable `renderdoc` feature. |
| **Metal** | macOS, iOS | `metal` (via `objc2`) | ✅ | `metal` | Pure Rust via `objc2` bindings. No C. |
| **DX12** | Windows | `windows-sys` | ✅ | `dx12` | Pure Rust via `windows-sys` metadata. No C. |
| **GLES** | Linux, Android, Windows | `glow` / `khronos-egl` | ⚠️ dlopen | `gles` | `glow` dlopen's `libEGL`/`libGLESv2`. Prefer Vulkan/Metal/DX12. |
| **WebGPU** | WASM | `web-sys` | ✅ | `webgpu` | Browser native. |
| **Noop** | Testing | Built-in | ✅ | `noop` | Stub backend. |

**wgpu Cargo.toml (pure-Rust profile):**
```toml
wgpu = { version = "30", default-features = false, features = [
    "std", "parking_lot", "wgsl",
    # Platform backends (pick one per target via cfg):
    "dx12",      # Windows
    "metal",     # macOS/iOS
    "vulkan",    # Linux/Android/Windows (dlopen unavoidable)
    # "gles",    # Avoid: pulls khronos-egl (dlopen)
    # "webgpu",  # WASM only
] }
# Explicitly disable:
# renderdoc = []  # pulls renderdoc-sys
```

**Verification:**
```bash
cargo tree -p wgpu -p wgpu-hal -p ash -p metal -p windows-sys --edges features | grep -E "renderdoc|khronos-egl|glow" || echo "CLEAN"
```

---

### 3. Audio I/O

| Platform | Backend | Crate | Version | Pure Rust | Maturity | Notes |
|----------|---------|-------|---------|-----------|----------|-------|
| **Linux** | ALSA (ioctl) | `alsa-rs` / `alsa-pcm` (custom) | — | ✅ | Emerging | Pure-Rust ALSA via `ioctl` on `/dev/snd/*`. No `alsa-sys`. See `rodio`/`cpal` source for ioctl patterns. |
| **Linux** | PipeWire | `pipewire-rs` (via `spa-sys`) | — | ❌ | — | Pulls `spa-sys` (C). Avoid. |
| **Android** | AAudio | `ndk` + `aaudio-sys` (custom) | 0.9+ | ✅ | Stable | `ndk` crate provides AAudio bindings. No `oboe`/`cpal` default. |
| **Windows** | WASAPI | `windows-sys` (IAudioClient) | 0.58+ | ✅ | Stable | Pure Rust via `windows-sys` COM interfaces. No `cpal` default. |
| **macOS/iOS** | CoreAudio | `objc2-core-audio` (custom) | — | ✅ | Emerging | `objc2` + `AudioUnit`/`AudioToolbox` frameworks. No `coreaudio-sys`. |
| **Cross-platform (fallback)** | cpal | `cpal` | 0.15+ | ⚠️ | Stable | **Default pulls `alsa-sys` on Linux**. Use only with `no-default-features` + per-platform feature gates. |

**Recommendation:** Write **per-platform audio backends** behind a `AudioDevice` trait (same pattern as windowing). cpal's Linux default requires `alsa-sys` (C). For a DJ app needing low latency, direct WASAPI/CoreAudio/AAudio/ALSA-ioctl is superior anyway.

**Pure-Rust ALSA ioctl example (stub):**
```rust
// linux_audio.rs
use nix::sys::socket::{socket, AddressFamily, SockType, SockFlag};
// Open /dev/snd/pcmC0D0p, ioctl SNDRV_PCM_IOCTL_*
```

**Verification:**
```bash
cargo tree -p cpal --features "alsa_backend" | grep alsa-sys && echo "DIRTY" || echo "CLEAN"
cargo tree -p your-audio-crate | grep -E "(sys|dl|cc)" || echo "CLEAN"
```

---

### 4. MIDI / HID / BLE

| Protocol | Platform | Crate | Version | Pure Rust | Maturity | Notes |
|----------|----------|-------|---------|-----------|----------|-------|
| **MIDI (Linux)** | ALSA Sequencer | `midir` | 0.11+ | ❌ | Stable | **Pulls `alsa-sys`**. No pure-Rust ALSA sequencer crate exists yet. |
| **MIDI (Windows)** | WinMM / WinRT | `midir` (winrt feature) | 0.11+ | ✅ | Stable | WinRT via `windows-sys`. Pure Rust. |
| **MIDI (macOS)** | CoreMIDI | `midir` / `coremidi` | 0.11+ | ⚠️ | Stable | `coremidi` crate uses `objc2` — pure Rust. |
| **MIDI (Android)** | NDK AMidi | `midir` (android feature) | 0.11+ | ⚠️ | Beta | JNI via `ndk`. Pure Rust glue. |
| **BLE MIDI** | Cross-platform | `btleplug` | 0.11+ | ⚠️ | Stable | Linux: `bluez-dbus` (dbus C). Windows/macOS: pure via `windows-sys`/`objc2`. |
| **HID** | Cross-platform | `hidapi` | 0.2+ | ❌ | Stable | C library. Consider `device_query` (pure) for basic input. |

**Recommendation:**
- **Linux MIDI:** Write a pure-Rust ALSA sequencer client (parse `/proc/asound/seq/*` + ioctl). ~500 LOC.
- **BLE:** Use `btleplug` on Windows/macOS (pure). On Linux, either accept `bluez-dbus` or write a `bluer` (pure-Rust BlueZ) backend.
- **HID:** `device_query` for keyboard/mouse; for DJ controllers, consider `libusb` via `rusb` (pure Rust, `libusb` is system DLL/so — acceptable ABI boundary).

**Verification:**
```bash
cargo tree -p midir --features "alsa" | grep alsa-sys && echo "DIRTY"
cargo tree -p btleplug --features "bluez" | grep -E "(dbus|bluez-sys)" && echo "DIRTY"
```

---

### 5. Audio Decoding (Lossy/Lossless)

| Format | Crate | Version | Pure Rust | Gapless | Default | Maturity |
|--------|-------|---------|-----------|---------|---------|----------|
| **FLAC** | `symphonia-bundle-flac` | 0.5+ | ✅ | ✅ | Yes | Excellent |
| **MP3** | `symphonia-bundle-mp3` | 0.5+ | ✅ | ✅ | No | Excellent |
| **Vorbis** | `symphonia-codec-vorbis` | 0.5+ | ✅ | ✅ | Yes | Excellent |
| **Opus** | `symphonia-codec-opus` | 0.5+ | ✅ | ✅ | Yes | Excellent |
| **AAC-LC** | `symphonia-codec-aac` | 0.5+ | ✅ | ❌ | No | Great |
| **HE-AAC/v2** | `symphonia-codec-aac` (he-aac feature) | 0.5+ | ✅ | ❌ | No | Good |
| **ALAC** | `symphonia-codec-alac` | 0.5+ | ✅ | ✅ | No | Great |
| **WavPack** | `symphonia-codec-wavpack` | 0.5+ | ✅ | ✅ | Yes | Good |
| **PCM/WAV** | `symphonia-codec-pcm` / `symphonia-format-riff` | 0.5+ | ✅ | ✅ | Yes | Excellent |
| **MP4/M4A** | `symphonia-format-isomp4` | 0.5+ | ✅ | ✅ | No | Excellent |
| **OGG** | `symphonia-format-ogg` | 0.5+ | ✅ | ✅ | Yes | Excellent |
| **MKV/WebM** | `symphonia-format-mkv` | 0.5+ | ✅ | ✅ | Yes | Excellent |

**Recommendation:** **Symphonia** — single ecosystem, all pure Rust, modular features. Enable only needed codecs.

**Cargo.toml:**
```toml
symphonia = { version = "0.5", default-features = false, features = [
    "flac", "vorbis", "opus", "mp3", "aac", "alac", "wavpack",
    "isomp4", "ogg", "mkv", "riff", "pcm"
] }
```

**Verification:**
```bash
cargo tree -p symphonia -p symphonia-codec-aac -p symphonia-bundle-mp3 --edges features | grep -E "(sys|dl|cc|cmake|bindgen)" || echo "CLEAN"
```

---

### 6. Time-Stretch / Pitch-Shift

| Crate | Version | Algorithm | Pure Rust | Streaming | Maturity | Best For |
|-------|---------|-----------|-----------|-----------|----------|----------|
| `timestretch` | 0.7+ | Hybrid WSOLA + Phase Vocoder | ✅ | ✅ | Stable | **DJ/EDM** — transient-aware, presets (House, Techno, etc.) |
| `wsola` | 0.3+ | WSOLA only | ✅ | ✅ | Stable | Speech, simple tempo change, low CPU |
| `cathar` | 0.8+ | WSOLA + Phase Vocoder (selectable) | ✅ | ❌ | Stable | Offline processing, pitch shift |
| `rubato` | 0.18+ | Sinc resampling (not time-stretch) | ✅ | ✅ | Stable | Sample-rate conversion only |

**Recommendation:** **`timestretch`** — designed for electronic music, hybrid engine preserves transients (kicks) while stretching pads smoothly. Streaming API for real-time deck control.

**Verification:**
```bash
cargo tree -p timestretch -p wsola -p cathar --edges features | grep -E "(sys|dl|cc|cmake|bindgen|rustfft-sys)" || echo "CLEAN"
```

---

### 7. Text Shaping & Rendering

| Layer | Crate | Version | Pure Rust | Coverage | Maturity | Notes |
|-------|-------|---------|-----------|----------|----------|-------|
| **Shaping (Primary)** | `swash` | 0.1+ | ✅ | Full (HarfBuzz parity) | Stable | Used by `cosmic-text`, `oxitext`. TrueType hinting, color glyphs (COLRv1, CBDT). |
| **Shaping (Alt)** | `rustybuzz` | 0.16+ | ✅ | 2221/2252 HB tests | Stable | HarfBuzz 10.1 port. Slower than C HB. No layout/rendering. |
| **Shaping (New)** | `harfrust` | 0.1+ | ✅ | Google Fonts | Beta | New HB port in Rust. Used by `parley`/`cosmic-text` (v0.4+). |
| **Layout** | `cosmic-text` | 0.10+ | ✅ | BiDi, line break, fallback | Stable | Batteries-included: `harfrust` + `fontdb` + `swash`. |
| **Layout (Alt)** | `parley` | 0.4+ | ✅ | Knuth-Plass, BiDi | Stable | Lower-level. `harfrust` + `skrifa` + `kurbo`. |
| **Raster (Primary)** | `fontdue` | 0.12+ | ✅ | Basic Latin, fast | Stable | No shaping. Simple API. `no_std` capable. |
| **Raster (Hinting)** | `swash` (scale) | 0.1+ | ✅ | Full hinting | Stable | TrueType hinting via `zeno`. |
| **Raster (Alt)** | `ab_glyph` | 0.3+ | ✅ | Basic | Stable | Sharper hints on some fonts. |
| **Font DB** | `fontdb` | 0.13+ | ✅ | System fonts | Stable | Pure Rust font enumeration. |
| **Full Pipeline** | `oxitext` | 0.1+ | ✅ | Complete | Beta | COOLJAPAN stack: `swash`/`rustybuzz` + `fontdue`/`ab_glyph`/`swash` raster + ICU4X. `#![forbid(unsafe_code)]`. |

**Recommendation:** **`cosmic-text`** for turnkey (shaping + layout + raster + fallback + BiDi). **`oxitext`** if you need pipeline control, SIMD raster, SDF atlas, or COLRv1/SVG color glyphs. Both 100% pure Rust.

**Verification:**
```bash
cargo tree -p cosmic-text -p oxitext -p swash -p rustybuzz -p fontdue --edges features | grep -E "(sys|dl|cc|cmake|bindgen|freetype|harfbuzz)" || echo "CLEAN"
```

---

### 8. Embedded Database

| Crate | Version | Pure Rust | Type | ACID | MVCC | Maturity | Notes |
|-------|---------|-----------|------|------|------|----------|-------|
| `redb` | 4.6+ | ✅ | K/V (B+tree) | ✅ | ✅ | Beta | LMDB-inspired. Zero-copy. File format stable. No `no_std` fs. |
| `native_db` | 0.8+ | ✅ | Document (on redb) | ✅ | ✅ | Alpha | Typed models, auto-migration, real-time subscriptions. Requires `native_model`. |
| `powdb` | 0.29+ | ✅ (engine) | Document (PowQL) | ✅ | ✅ | Alpha | Nested JSON-like. Windows **not supported** (mmap Unix-only). Server pulls TLS. |
| `sled` | 0.34+ | ✅ | K/V (LSM) | ✅ | ❌ | Stable | Higher write amplification. Larger files. |
| `fjall` | 0.7+ | ✅ | K/V (LSM) | ✅ | ✅ | Beta | Fast, tiered storage. |

**Recommendation:** **`redb`** for raw K/V (proven, fast reads, pure Rust). **`native_db`** if you want typed documents + migrations + subscriptions (built on `redb`). Both pure Rust.

**Verification:**
```bash
cargo tree -p redb -p native_db -p native_model --edges features | grep -E "(sys|dl|cc|cmake|bindgen|sqlite|libc)" || echo "CLEAN"
```

---

### 9. Networking & TLS

| Layer | Crate | Version | Pure Rust | Notes |
|-------|-------|---------|-----------|-------|
| **TCP/UDP** | `tokio` / `async-std` / `smol` | — | ✅ | `mio` (epoll/kqueue/IOCP) is pure Rust. |
| **TLS (Client/Server)** | `rustls` | 0.23+ | ⚠️ | **Default pulls `aws-lc-rs` (C/asm)**. Must disable default features. |
| **Crypto Provider (Pure)** | `moirai-crypto` | 0.11+ | ✅ | `rustls` `CryptoProvider` via RustCrypto. `#![forbid(unsafe_code)]`. No `ring`, no `aws-lc-rs`. TLS 1.2/1.3. |
| **Crypto Provider (Pure)** | `rustls-pure` | 0.15+ | ✅ | TLS 1.3 only. PQ X25519MLKEM768. Wycheproof tested. |
| **Crypto Provider (Pure)** | `rind-rustls` | 0.23+ | ✅ | **Fastest** — Rust `asm!` (x86_64, aarch64, arm32). Verified via `s2n-bignum` + Lean proofs. Drop-in for `ring`/`aws-lc-rs` APIs. |
| **Crypto Provider (Pure)** | `oxitls-adapter-rustls-rustcrypto` | 0.3+ | ✅ | OxiTLS stack default. Per-config provider (no global install). |
| **HTTP Client** | `reqwest` | 0.12+ | ⚠️ | Default uses `rustls` + `aws-lc-rs`. Use `default-features = false, features = ["rustls-tls", "rustls-no-provider"]` + install pure provider. |
| **QUIC** | `quinn` | 0.11+ | ⚠️ | Pulls `rustls` default. Same provider fix. |

**rustls Cargo.toml (pure-Rust):**
```toml
rustls = { version = "0.23", default-features = false, features = ["std", "tls12", "logging"] }
# Pick ONE provider:
moirai-crypto = "0.11"   # RustCrypto, TLS 1.2/1.3, audited
# rustls-pure = "0.15"   # TLS 1.3 only, PQ, Wycheproof
# rind-rustls = "0.23"   # Fastest, verified asm!, ring/aws-lc API compat
```

**Install at startup:**
```rust
moirai_crypto::default_provider().install_default().unwrap();
// or rind_rustls::default_provider()?.install_default().unwrap();
```

**Verification:**
```bash
cargo tree -p rustls -p moirai-crypto -p rind-rustls --edges features | grep -E "(ring|aws-lc|cc|nasm|cmake)" || echo "CLEAN"
cargo build --target x86_64-unknown-linux-musl 2>&1 | grep -E "error.*linker|cc1" && echo "DIRTY" || echo "MUSL OK"
```

---

### 10. JSON

| Crate | Version | Pure Rust | SIMD | API Compat | Maturity | Notes |
|-------|---------|-----------|------|------------|----------|-------|
| `serde_json` | 1.0+ | ✅ | No | Standard | Stable | Baseline. 12 `unsafe` sites (inherited). |
| `sonic-rs` | 0.5+ | ✅ | Yes (packed_simd) | Serde-compat | Stable | Requires `-C target-cpu=native`. x86_64/aarch64 only. |
| `rusty_json_turbo` | 0.1+ | ✅ | No | **Byte-identical to serde_json** | Beta | Fork of `serde_json`. 1.21–1.89x faster. `no_std+alloc`, WASM. Same crate name. |
| `simd-json` | 0.18+ | ✅ | Yes | Tape → struct | Stable | Two-stage parse. Slower than sonic/serde on struct deserialize. |
| `nosj` | 0.1+ | ✅ | Yes | Event-based (no Value) | Alpha | Streaming Sink/Reader/Writer. 2–19x serde_json. No DOM. |

**Recommendation:** **`rusty_json_turbo`** — drop-in `serde_json` replacement, byte-identical output, faster, pure Rust, `no_std+alloc`, tested on WASM. If you need SIMD and control target CPU: `sonic-rs`.

**Verification:**
```bash
cargo tree -p rusty_json_turbo -p sonic-rs --edges features | grep -E "(sys|dl|cc|cmake|bindgen|packed_simd)" || echo "CLEAN"
```

---

### 11. Image Decoding

| Format | Crate | Version | Pure Rust | Encode | Maturity | Notes |
|--------|-------|---------|-----------|--------|----------|-------|
| **PNG** | `png` / `zune-png` | 0.18+ / 0.5+ | ✅ | ✅ | Stable | `png` = reference. `zune-png` = faster, modular. |
| **JPEG** | `zune-jpeg` / `jpeg-decoder` / `jpeg-encoder` | 0.5+ | ✅ | ✅ | Stable | `zune-jpeg` = pure decode. `jpeg-decoder` = CMYK, progressive, DCT scaling. `jpeg-encoder` = baseline. |
| **WebP** | `image-webp` | 0.9+ | ✅ | Lossless only | Stable | Lossy needs `libwebp` (C). Lossless VP8L is pure. |
| **AVIF** | `ravif` (+ `rav1e`) / `avif-parse` + `re_rav1d` | 0.1+ | ✅ | ✅ | Beta | `rav1e` = pure Rust encoder (slow). `re_rav1d` = dav1d port (pure Rust decoder). Heavy compile. |
| **JPEG XL** | `jxl-oxide` / `jixel` | 0.1+ | ✅ | ✅ | Beta | Decode conformant. Encode native only (lossy VarDCT / lossless Modular). |
| **TIFF** | `tiff` | 0.8+ | ✅ | ✅ | Stable | LZW/Deflate/PackBits/JPEG. |
| **GIF/BMP/ICO** | `image` (gif/bmp features) | 0.25+ | ✅ | ✅ | Stable | First frame only for GIF. |
| **HDR/EXR** | `exr` / `zune-hdr` | 0.16+ / 0.5+ | ✅ | ✅ | Stable | Half/f32, DWAA/DWAB. |
| **Unified** | `zune-image` / `imgio` / `rusty_image` | 0.5+ / 0.1+ / 0.1+ | ✅ | ✅ | Beta | `zune-image` = modular codecs. `imgio` = GPU textures (BC/ASTC/ETC). `rusty_image` = small formats + header probe. |

**Recommendation:** **`zune-image`** for modular, fast decoding (enable only needed format features). **`imgio`** if you need GPU texture compression (BC1-7, ASTC, ETC, Basis Universal). **`rusty_image`** for header probing + small formats (QOI, BMP, TGA, HDR, ICO, PNM).

**Verification:**
```bash
cargo tree -p zune-image -p zune-jpeg -p zune-png -p jxl-oxide -p ravif --edges features | grep -E "(sys|dl|cc|cmake|bindgen|dav1d|libwebp|rav1d)" || echo "CLEAN"
```

---

### 12. ML / Stems Inference

| Crate | Version | Pure Rust | Backends | ONNX | Training | Maturity | Best For |
|-------|---------|-----------|----------|------|----------|----------|----------|
| `tract` | 0.22+ | ✅ | CPU | ✅ (~85% ops) | ❌ | Stable | Edge/embedded ONNX. No GPU. WASM. |
| `candle` | 0.9+ | ✅ | CPU, CUDA, Metal | Weight loading | Exp. | Stable | LLM/vision inference. Small binary. |
| `burn` | 0.21+ | ✅ | WGPU (Vulkan/Metal/DX12/WASM), CUDA, ROCm, NDArray, LibTorch | ✅ (via `burn-onnx` → Rust code) | ✅ | Stable | Full framework. Multi-backend. `no_std` core. |
| `rten` | 0.26+ | ✅ | CPU | ✅ | ❌ | Beta | Pure-Rust ONNX engine. |
| `oxionnx` | 0.1+ | ✅ | CPU | ✅ | ❌ | Alpha | Pure-Rust ONNX with control flow + quantization. |
| `wonnx` | 0.1+ | ✅ | WebGPU | ✅ | ❌ | Alpha | WebGPU-accelerated ONNX for native + web. |
| `mistral.rs` | 0.1+ | ✅ | CPU, CUDA, Metal | ❌ (GGUF/safetensors) | ❌ | Beta | Quantized LLM inference. |
| `ort` | 2.0+ | ❌ | CUDA, TRT, DirectML, CoreML, OpenVINO | ✅ | ❌ | RC | **Wraps ONNX Runtime C++**. Use `ort-tract` / `ort-candle` backends for pure Rust. |

**Recommendation:**
- **Stems separation (ONNX models):** `tract` (CPU-only, pure, small) or `burn` + `burn-onnx` (GPU via WGPU, converts ONNX to Rust code at build time).
- **LLM/Transformers:** `candle` or `mistral.rs`.
- **Full training + inference:** `burn`.

**ort alternative backend (pure Rust):**
```rust
// Before any ort use:
ort::set_api(ort_tract::api()); // or ort_candle::api()
```

**Verification:**
```bash
cargo tree -p tract -p candle -p burn -p burn-onnx -p rten --edges features | grep -E "(sys|dl|cc|cmake|bindgen|libtorch|onnxruntime)" || echo "CLEAN"
```

---

### 13. Compression

| Algorithm | Crate | Version | Pure Rust | Stream | Maturity | Notes |
|-----------|-------|---------|-----------|--------|----------|-------|
| **DEFLATE/zlib/gzip** | `miniz_oxide` (via `flate2` rust_backend) | 0.7+ / 1.0+ | ✅ | ✅ | Stable | Default `flate2` backend. RFC 1950/1951. |
| **DEFLATE (fast)** | `omnizip-deflate` / `omnizip-libdeflate` | 0.1+ | ✅ | ✅ | Beta | `miniz_oxide` compat. Optimized decode. |
| **Zstandard** | `ruzstd` | 0.1+ | ✅ | ✅ | Beta | ZSTD frame format. Level 1 (fastest) encode. |
| **LZ4** | `lz4_flex` | 0.11+ | ✅ | ✅ | Stable | Block + frame. HC (optimal parse) via feature. |
| **Snappy** | `snap` / `omnizip-snappy` | 1.0+ / 0.1+ | ✅ | ✅ | Stable | Raw block. ~500 MB/s. |
| **Brotli** | `brotli` | 3.4+ | ✅ | ✅ | Stable | Quality 11 (best ratio). |
| **LZMA2/XZ** | `omnizip-lzma` | 0.1+ | ✅ | ✅ | Beta | XZ container. |
| **BZip2** | `bzip2` (pure-Rust impl) | 0.4+ | ✅ | ✅ | Stable | Pure Rust port. |
| **Unified** | `compcol` / `limnifs-core` | 0.1+ / 0.1+ | ✅ | ✅ | Alpha | Multi-codec registry. Cross-validated vs CLI tools. |

**Recommendation:** **`flate2` with `rust_backend` (default)** for DEFLATE. **`ruzstd`** for Zstd. **`lz4_flex`** for LZ4. All pure Rust, stable, well-tested.

**Verification:**
```bash
cargo tree -p flate2 -p miniz_oxide -p ruzstd -p lz4_flex -p brotli --edges features | grep -E "(sys|dl|cc|cmake|bindgen|zlib|zstd-sys)" || echo "CLEAN"
```

---

## Cargo-Deny / Verification Recipe

### 1. `deny.toml` (workspace root)

```toml
# deny.toml
[advisories]
db-path = ".cargo/advisory-db"
vulnerability = "deny"
unmaintained = "warn"
yanked = "warn"
notice = "warn"

[licenses]
allow = ["MIT", "Apache-2.0", "BSD-3-Clause", "BSD-2-Clause", "ISC", "Zlib", "Unlicense", "MIT OR Apache-2.0"]
deny = ["GPL-3.0", "AGPL-3.0", "LGPL-3.0"]

[bans]
multiple-versions = "warn"
wildcards = "deny"
highlight = "all"

# FORBID all -sys crates and C build deps
deny = [
    # C sys crates
    { name = "wayland-sys", version = "*" },
    { name = "x11-dl", version = "*" },
    { name = "xkbcommon-dl", version = "*" },
    { name = "alsa-sys", version = "*" },
    { name = "libc", version = "*" },  # often pulled by -sys crates
    { name = "libloading", version = "*" },
    { name = "renderdoc-sys", version = "*" },
    { name = "ash", version = "*" },  # only for vulkan (dlopen unavoidable)
    { name = "khronos-egl", version = "*" },
    { name = "glow", version = "*" },
    { name = "spa-sys", version = "*" },
    { name = "bluez-sys", version = "*" },
    { name = "dbus", version = "*" },
    { name = "coremidi-sys", version = "*" },
    { name = "coreaudio-sys", version = "*" },
    { name = "freetype-sys", version = "*" },
    { name = "harfbuzz-sys", version = "*" },
    { name = "png-sys", version = "*" },
    { name = "jpeg-sys", version = "*" },
    { name = "zlib-sys", version = "*" },
    { name = "zstd-sys", version = "*" },
    { name = "lzma-sys", version = "*" },
    { name = "bzip2-sys", version = "*" },
    { name = "sqlite", version = "*" },  # pulls libsqlite3-sys
    { name = "rusqlite", version = "*" },
    { name = "ring", version = "*" },
    { name = "aws-lc-rs", version = "*" },
    { name = "aws-lc-sys", version = "*" },
    { name = "onnxruntime-sys", version = "*" },
    { name = "ort", version = "*" },  # pulls onnxruntime
    { name = "libtorch", version = "*" },
    { name = "tch", version = "*" },
    # Build deps that invoke C toolchain
    { name = "cc", version = "*" },
    { name = "cmake", version = "*" },
    { name = "bindgen", version = "*" },
    { name = "pkg-config", version = "*" },
    { name = "nasm-rs", version = "*" },
]

# Allow specific crates that are FALSE POSITIVES (pure Rust despite name)
allow = [
    { name = "windows-sys" },      # Pure metadata, no C
    { name = "windows" },          # Pure metadata
    { name = "objc2" },            # Pure Rust objc runtime
    { name = "objc2-foundation" },
    { name = "objc2-appkit" },
    { name = "objc2-uikit" },
    { name = "objc2-core-audio" },
    { name = "ndk" },              # Pure Rust JNI/NDK bindings
    { name = "android-activity" }, # Pure Rust JNI glue
    { name = "raw-window-handle" },# Pure Rust types
    { name = "wgpu-types" },       # Pure Rust
    { name = "naga" },             # Pure Rust shader compiler
    { name = "miniz_oxide" },      # Pure Rust DEFLATE
    { name = "rusty_json_turbo" }, # Pure Rust serde_json fork
]

[build-deps]
# Deny any build.rs that runs cc/cmake/bindgen
deny = ["cc", "cmake", "bindgen", "pkg-config", "nasm-rs"]
```

### 2. CI Verification Script

```bash
#!/usr/bin/env bash
# verify-pure-rust.sh
set -euo pipefail

echo "=== Checking for forbidden -sys crates ==="
cargo tree --edges features --depth 100 | grep -E "\-sys\b" && {
    echo "FAIL: -sys crates found"
    exit 1
} || echo "OK: No -sys crates"

echo "=== Checking for C build deps (cc/cmake/bindgen) ==="
cargo tree --edges features --depth 100 | grep -E "\b(cc|cmake|bindgen|pkg-config|nasm)\b" && {
    echo "FAIL: C build deps found"
    exit 1
} || echo "OK: No C build deps"

echo "=== Checking for dlopen crates ==="
cargo tree --edges features --depth 100 | grep -E "\b(libloading|dlopen|dlib)\b" && {
    echo "WARN: dlopen crates found (may be unavoidable for GPU drivers)"
}

echo "=== cargo-deny check ==="
cargo deny check advisories licenses bans sources

echo "=== Cross-target build test (musl) ==="
rustup target add x86_64-unknown-linux-musl 2>/dev/null || true
cargo build --target x86_64-unknown-linux-musl --workspace 2>&1 | tail -20

echo "=== All checks passed ==="
```

### 3. Per-Crate Allowlist (for unavoidable GPU ABI)

```toml
# In deny.toml [bans] section - ONLY these exceptions:
allow = [
    # GPU driver ABI - UNAVOIDABLE
    { name = "ash", version = "*" },           # Vulkan loader dlopen
    { name = "libloading", version = "*" },    # ash dependency
    { name = "khronos-egl", version = "*" },   # GLES/EGL dlopen (if gles backend used)
    # Platform syscalls - UNAVOIDABLE OS boundary
    { name = "libc", version = "*" },          # Raw syscalls (errno, etc.)
    # Proc-macro / codegen - build-time only
    { name = "syn", version = "*" },
    { name = "quote", version = "*" },
    { name = "proc-macro2", version = "*" },
    { name = "unicode-ident", version = "*" },
]
```

---

## Migration Checklist (djsugar-next)

| Layer | Current | Target | Effort | Blockers |
|-------|---------|--------|--------|----------|
| Windowing (Linux X11) | `winit` (x11) | `x11rb` | Medium | Custom event loop, keymap |
| Windowing (Linux Wayland) | `winit` (wayland) | `wayland-client` + `smithay-client-toolkit` | Medium | Text input v3, fractional scaling |
| Windowing (Android) | `winit` (android) | `android-activity` + custom `GameActivity` | Low | JNI glue, SurfaceView |
| Windowing (Windows) | `winit` (win32) | `windows-sys` | Low | DPI, DirectComposition |
| Windowing (macOS) | `winit` (appkit) | `objc2` + custom | Medium | `NSTextInputClient`, `CAMetalLayer` |
| GPU (Vulkan) | `wgpu` (vulkan) | `wgpu` (vulkan, no renderdoc) | **Done** | — |
| GPU (Metal) | `wgpu` (metal) | `wgpu` (metal) | **Done** | — |
| GPU (DX12) | `wgpu` (dx12) | `wgpu` (dx12) | **Done** | — |
| Audio I/O (Linux) | `cpal` (alsa) | Custom ALSA ioctl | High | No existing crate |
| Audio I/O (Android) | `cpal` (oboe/aaudio) | `ndk` AAudio | Medium | — |
| Audio I/O (Windows) | `cpal` (wasapi) | `windows-sys` WASAPI | Medium | — |
| Audio I/O (macOS) | `cpal` (coreaudio) | `objc2-core-audio` | High | No existing crate |
| MIDI (Linux) | `midir` (alsa) | Custom ALSA sequencer | High | No existing crate |
| MIDI (Windows) | `midir` (winmm) | `midir` (winrt) | Low | Feature flag |
| MIDI (macOS) | `midir` (coremidi) | `coremidi` (objc2) | Low | — |
| MIDI (Android) | `midir` (amidi) | `midir` (android) | Low | — |
| Decoding | — | `symphonia` | Low | Feature selection |
| Time-stretch | — | `timestretch` | Low | — |
| Text | — | `cosmic-text` or `oxitext` | Medium | Font fallback |
| Database | — | `redb` / `native_db` | Low | — |
| TLS | `rustls` (aws-lc-rs) | `rustls` + `moirai-crypto` | Low | Provider install |
| JSON | `serde_json` | `rusty_json_turbo` | Low | Drop-in |
| Images | — | `zune-image` | Low | Feature selection |
| ML Stems | — | `tract` or `burn` | Medium | Model conversion |
| Compression | — | `flate2` (miniz_oxide) + `ruzstd` + `lz4_flex` | Low | — |

---

## Summary

| Category | Pure-Rust Choice | Unavoidable C/ABI |
|----------|------------------|-------------------|
| Windowing | Per-platform shells (`x11rb`, `wayland-client`, `android-activity`, `windows-sys`, `objc2`) | None |
| GPU | `wgpu` (Metal/DX12 pure, Vulkan dlopen) | `libvulkan` / `vulkan-1.dll` (driver) |
| Audio I/O | Per-platform (ALSA ioctl, AAudio/ndk, WASAPI/windows-sys, CoreAudio/objc2) | None |
| MIDI | WinRT/macOS/Android pure; Linux custom ALSA sequencer | None |
| BLE MIDI | `btleplug` (Windows/macOS pure); Linux `bluer` or accept `bluez-dbus` | Linux BlueZ D-Bus (system daemon) |
| Decoding | `symphonia` (all codecs) | None |
| Time-stretch | `timestretch` (hybrid WSOLA+PV) | None |
| Text | `cosmic-text` / `oxitext` (`harfrust`/`swash`/`fontdue`) | None |
| Database | `redb` / `native_db` | None |
| TLS | `rustls` + `moirai-crypto` / `rind-rustls` / `rustls-pure` | None |
| JSON | `rusty_json_turbo` (drop-in) | None |
| Images | `zune-image` / `imgio` / `rusty_image` | None |
| ML Stems | `tract` (CPU) / `burn` (GPU via WGPU) | None |
| Compression | `miniz_oxide` / `ruzstd` / `lz4_flex` / `brotli` | None |

**Verification command (run in CI):**
```bash
cargo deny check bans && \
cargo tree --workspace --edges features | grep -E "\-sys\b" | grep -vE "windows-sys|objc2|ndk|android-activity|raw-window-handle|wgpu-types|naga|miniz_oxide|rusty_json_turbo" && exit 1 || echo "PURE RUST VERIFIED"
```

## Maturity check (crates.io, 2026-11-22)

Crates below have very few downloads or appeared in the last weeks. Do not adopt them without a code review; prefer writing our own or the established option.

| Crate | Downloads | Verdict |
| --- | --- | --- |
| rusty_json_turbo | 99 | reject; use serde_json or own parser |
| imgio | 41 | reject; use zune-image |
| rusty_image | 81 | reject; use zune-image |
| moirai-crypto | 528 | review before use |
| oxitext | 841 | review; harfrust + swash are established |
| oxiaudio-decode | 723 | reject; use symphonia |
| rustls-pure | 1224 | review before use |
