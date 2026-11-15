# signalsmith-stretch 0.1.3, patched for DJ Sugar

This is the `signalsmith-stretch` 0.1.3 Rust wrapper from crates.io (MIT),
bundling Signalsmith Stretch 1.3.1 and Signalsmith Linear (MIT, see the
licence files). DJ Sugar's key lock and key shifts use it, through
`[patch.crates-io]` in the workspace `Cargo.toml`. The examples, the web and
command-line folders, the linear library's tests and the dev-dependencies are
left out; nothing else changed except the following.

## Split computation (src/wrapper.{h,cpp}, src/lib.rs)

The wrapper had no way to turn on the library's `splitComputation`, so each
deck's stretcher did a whole block's FFT work in the one audio buffer where
the block started (every 30 ms). Key-locked decks that lined up spiked the
worst-case buffer time. `Stretch::preset_default_split` is `presetDefault`
with `splitComputation = true`, which spreads that work over the interval
for one interval more output latency.

Remove this copy once the wrapper exposes `splitComputation` upstream.
