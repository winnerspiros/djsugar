# Design

## Context

DJ Sugar separates today by running Python Demucs (`htdemucs`) as a subprocess (`crates/engine/src/stems.rs`, `run_demucs`), writing four FLAC files to the cache, and loading them through the same path as NI stem files. `Stems` already fills progressively (chunks of 65,536 frames, `OnceLock` per chunk, 16-bit), and the engine plays `Σ gain × stem`.

A user request (2026-10-27): run Spleeter, which is old but very fast, when a track loads so there is always *something*, then run HTDemucs or BS-RoFormer in the background and swap it in. Use the GPU where it helps.

## Research: models

SDR is the usual separation quality score in dB (higher is better, +1 dB is clearly audible). "MUSDB" is the MUSDB18-HQ test set; "Multisong" is ZFTurbo's larger set. Speeds are as reported by the source, not yet measured on DJ Sugar's target machines.

| Model | Year | Avg SDR (MUSDB) | Size | Speed reported | Weights licence |
|---|---|---|---|---|---|
| **Spleeter 4stems** (Deezer) | 2019 | 5.9 (non-HQ set)¹ | 4 × 9.8 M params; 4 × 20 MB fp16 ONNX² | 100× real time on GPU³; 18× real time on CPU (30 s in 1.7 s)² | MIT (Deezer), small open question in deezer/spleeter#898² |
| Open-Unmix / DTTNet | 2019 / 2023 | 7.8 (DTTNet)⁴ | 5 M (×4) | — | UMX-L weights non-commercial; DTTNet unclear |
| **SCNet-small** | 2024 | 9.0⁵ | 10 M | 0.67 s per s of audio, single CPU thread, about half of HTDemucs⁵ | Code MIT; weights via ZFTurbo, unstated⁴ |
| **HTDemucs** (Meta, v4) | 2022 | ≈9.0; Multisong 9.16⁴ | 42 M, 84 MB⁶ | 3-min song: 47 s with PyTorch Metal for the 4-model `_ft` bag on M4 Pro, so ≈12 s for the single model⁷ | MIT (Meta) |
| HTDemucs_ft | 2022 | ≈9.2 | 4 × 84 MB = 333 MB⁶ | 4× slower than HTDemucs | MIT |
| **BS-RoFormer** 4-stem | 2023 | 9.65 (MUSDB) / 9.38 Multisong⁴ | ≈72 M per stem in the paper; 257 MB fp16 ONNX community export⁸ | Several times slower than HTDemucs (transformer) — unmeasured | ZFTurbo checkpoint MIT⁸; trained on MUSDB18-HQ, whose terms are research-oriented |
| SCNet-XL IHF | 2024 | 10.08⁴ | ≈42 M+ | — | unstated |
| Moises-Light | 2025 | 9.96 with 5 M (×4) params⁹ | small | no speed given | weights not released⁹ |

¹ SCNet paper table 2. ² huggingface.co/Best-Practice/spleeter-4stems-onnx. ³ research.deezer.com/projects/spleeter. ⁴ ZFTurbo Music-Source-Separation-Training, docs/pretrained_models.md. ⁵ arxiv 2401.13276. ⁶ github.com/nikhilunni/demucs-rs. ⁷ stemsplit.io/blog/htdemucs-ft-onnx-export (vendor write-up). ⁸ huggingface.co/silverdaw/bs-roformer-rhythm-onnx. ⁹ arxiv 2510.06785.

What this means:

- **Spleeter is the right fast tier.** Nothing else open is close on speed. It is a plain convolutional U-Net that only predicts *masks* on a magnitude spectrogram (the STFT is done outside the network), which suits every GPU runtime including Apple's Neural Engine. The masks for the four stems add up to one, so the four stems add back exactly to the original. Its weakness is bleed and a "watery" sound in the stems, which matters less with the mix rule below.
- **HTDemucs is the right first high-quality tier.** Clear MIT weights from Meta, one 84 MB model, and a working pure-Rust port on the GPU already exists (demucs-rs, Apache-2.0, burn with Metal and Vulkan). `_ft` costs four times the time for about 0.2 dB, not worth it in the background of a live set.
- **BS-RoFormer / SCNet-XL are the later *Best* tier.** They are about 0.6–1 dB better (most on vocals and drums), but are bigger, slower, and their weights come from community training runs with less clear terms. Better added as an optional download once the pipeline is proven.
- **SCNet-small** is a good alternative middle tier: HTDemucs quality at roughly half the cost. It is the fallback if HTDemucs turns out too slow on Intel Macs or older Linux GPUs.

## Research: runtimes

| Runtime | macOS GPU | Linux GPU | Ships as | Notes |
|---|---|---|---|---|
| **burn** (wgpu / CubeCL backend) | Metal | Vulkan (NVIDIA, AMD, Intel) | pure Rust, no extra libraries | HTDemucs already ported (demucs-rs). Can import ONNX at build time, so Spleeter's U-Net comes over directly. Its own wgpu device, separate from the waveform renderer's. Speed against CoreML/CUDA unproven. |
| **ort** (ONNX Runtime) + CoreML EP | CoreML: GPU and Neural Engine | CUDA (NVIDIA only), else CPU | C++ library, ≈20–40 MB per platform, prebuilt | Best path to the Neural Engine. CoreML compiles big graphs slowly (over 5 minutes for HTDemucs on first run⁷) and falls back to CPU for ops it lacks. Linux GPU only for NVIDIA with CUDA installed. |
| candle | Metal | CUDA | pure Rust | Every model hand-ported; no Vulkan, so no AMD/Intel GPUs on Linux. |
| Python (bundled) | MPS | CUDA | 1–2 GB of Python, Torch | Rejected: download size, signing and notarizing every .so, startup time. |

**Recommendation: burn with the wgpu backend for both models, with burn's CPU backend as the fallback.** One runtime, no C++ dependency, the same two GPU APIs DJ Sugar already depends on, and AMD/Intel GPUs on Linux work too. Keep ONNX models as the interchange format so `ort` + CoreML can be swapped in if the benchmark in task 1 shows burn far behind (the main reason would be the Neural Engine for the Spleeter tier on Mac).

## Decisions

### Two tiers, one stem layout
Both tiers produce the same four stems in the same order as today (Drums, Bass, Other, Vocals), so the STEMS page, `[ChannelN_StemM]` controls, the Kick/HiHat split and controller mappings do not change.

### Fast tier: from the cue point outwards
On load, Spleeter separates the 12-second window around the cue point (or the playhead) first, then the rest of the track forwards, then backwards. A window is 512 STFT frames (≈11.9 s); expected cost well under a second per window on Apple Silicon CPU, less on GPU. The deck shows stems as available as soon as the first window is done; before that the pads are dim and the deck plays the original. Fast stems are kept in memory only.

### Background tier: nearest the playhead first, swapped by chunk
HTDemucs runs on a single background worker for all decks, in 7.8-second segments with overlap, starting at the playhead of the deck that is playing (the deck that is cued but not playing next, then the others). Each finished segment replaces the fast stems for that stretch. The stems store becomes two layers per chunk: playback reads the high-quality layer where it exists, else the fast one, and crossfades over 20 ms (equal power) at the first frame where the layer changes. Nothing changes under a held stem pad while the DJ is mid-gesture: the swap for a chunk waits until the playhead is past it or the gains are at rest.

### The mix keeps the original
The deck plays `original − Σ (1 − gainᵢ) × stemᵢ` instead of `Σ gainᵢ × stemᵢ`. With every stem at full level that is the original track bit for bit, so loading a track never makes it sound worse, and a weaker fast stem only colours the part being removed. Muting all stems still gives silence when the stems add up to the original (Spleeter exactly, HTDemucs within its small residual). NI stem files keep the sum, since their main mix is mastered separately.

### Caching and disk space
Only high-quality stems are cached, keyed as today (`cache::stems_dir`), and only three are stored: Other is derived as original − drums − bass − vocals, saving a quarter. FLAC 16-bit, so roughly 3 × the size of a lossless source (about 100 MB for a 6-minute track). A *Stems cache* limit (default 10 GB; a typical laptop has little free space) evicts the least recently loaded tracks; tracks in a prepared list are kept. Existing Demucs-made caches stay valid.

### GPU and the audio thread
Models never run on the audio thread. The worker uses its own wgpu device and submits small batches so the waveform renderer stays smooth; it pauses while the app reports dropped frames or audio underruns, and drops to the CPU backend when no GPU adapter is found. One deck's fast tier always has priority over background work.

### Model files
Downloaded on first use from a GitHub release asset of DJ Sugar, checked by SHA-256, stored in the app data folder: Spleeter ≈40 MB fp16 (or ≈80 MB fp32), HTDemucs 84 MB. Settings > Stems shows the download state. If the download fails, the old Demucs command path still works when Demucs is installed (kept for one release, then removed).

## Risks / Trade-offs

- **burn's speed is unproven for these models** → task 1 benchmarks burn-wgpu, burn-CPU, `ort` CPU and `ort` CoreML on a test Mac and on a Linux box before anything else is built. Pass bars: Spleeter first window < 1 s, full 6-min track < 10 s; HTDemucs full 6-min track < 60 s on M1-class (the existing spec's bar).
- **Memory**: fast and high-quality layers for a deck are about 2 × 250 MB for a 6-minute track at 16-bit. The fast layer is freed chunk by chunk as the high-quality one replaces it; four decks stay under about 1.5 GB.
- **GPU contention with waveforms** → separate device, small batches, pause on dropped frames; measure frame times during separation.
- **Intel Macs** have weak GPUs → Intel gets the fast tier on load and HTDemucs only in the background when idle or for prepared lists; SCNet-small is the fallback model if needed.
- **Licences**: Spleeter MIT (one open question upstream), HTDemucs MIT. All MUSDB-trained weights carry the dataset's research terms in their history; DJ Sugar is free and non-commercial, and the *Best* tier's community weights get a separate check before shipping.
- **Two wgpu versions** (Slint's wgpu 30 and burn's) may both end up in the binary → acceptable size cost; check in task 1.

## Open questions

1. HTDemucs as the background model now, with BS-RoFormer / SCNet-XL as an optional *Best* download later? (Recommended.) Or go straight to BS-RoFormer?
2. Stems on load for every track by default, or only when the STEMS page or a stem control is touched? (Recommended: every track; it costs a second of CPU/GPU.)
3. Stems cache default of 10 GB with least-recently-loaded eviction?
