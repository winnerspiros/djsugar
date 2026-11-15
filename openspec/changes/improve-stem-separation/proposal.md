# Proposal

Planned for: **v0.5 · Stems and performance** (the first part, Spleeter on load, high-quality stems on request and the VirtualDJ stems pads, comes in v0.4; see tasks.md)

## Why

DJs use stems a lot. Today a track only has stems after SEPARATE STEMS runs Demucs, which has to be installed by hand with Python and takes minutes per track, so stems are something to prepare, not something to reach for mid-set. djay, rekordbox and VirtualDJ all have stems the moment a track loads.

## What Changes

- **Instant stems on load.** Every loaded track gets stems from a small, fast model (Spleeter 4-stem class, about 40 MB) within a second or two, starting where the track is cued and filling the rest of the track in a few seconds.
- **Studio-quality stems in the background.** A high-quality model (HTDemucs) then separates the track in the background, nearest the playhead first, and its stems replace the fast ones as they become ready, with a short crossfade so nothing clicks or jumps. The high-quality result is cached; the fast one is not.
- **Untouched stems sound like the original.** When every stem is at full level the deck plays the original track, not the sum of the stems. Turning a stem down removes only that stem from the original, so the fast model's artefacts are only heard in the part being taken out.
- **No install.** Both models run inside DJ Sugar (Rust, on the GPU through Metal on macOS and Vulkan on Linux, with a CPU fallback). Python and Demucs are no longer needed. Model files download on first use, not in the app download.
- **Separate ahead of time.** A list, crate or set can be separated in the background; tracks with cached high-quality stems show a stems mark.
- **Quality setting.** Settings > Stems: *Stems on load* (Instant + high quality / High quality only / Off), *Background quality* (High; *Best* is added later), and a *Stems cache* size limit.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `stems`: built-in two-tier separation (instant on load, high quality in the background), original-preserving mix, ahead-of-time separation, stems cache limit. Replaces "Separate with Demucs".

## Out of Scope

- A *Best* tier (BS-RoFormer or SCNet-XL, about 0.5–1 dB better than HTDemucs) — a follow-up once the two-tier pipeline works; see design.md.
- Six stems (guitar, piano) and kick/hi-hat by model; the existing filter split of drums into Kick and HiHat stays.
- Per-stem effects and acapella pads (`add-stem-fx-and-acapella`).
- Windows (v2.0) and NVIDIA CUDA-specific paths; NVIDIA on Linux runs through Vulkan.
- Writing separated stems back out as `.stem.mp4` files for other apps.

## Impact

crates/engine (stems.rs, loader.rs, engine.rs stem mix, cache.rs), a new `crates/separation` crate for the models and STFT, Settings > Stems, the STEMS pad page and the track browser (stems mark), website/book/src/guide/stems.md. New dependency: a Rust ML runtime (burn with its wgpu backend, or ONNX Runtime through `ort`; task 1 picks by benchmark). Model weights hosted on a GitHub release of DJ Sugar. See design.md and notes/stems-review.md.
