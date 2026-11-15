# femtovg 0.27.0, patched for DJ Sugar

This is femtovg 0.27.0 from crates.io (MIT or Apache-2.0, see the licence
files), used through `[patch.crates-io]` in the workspace `Cargo.toml`. Slint
draws the UI with it. The examples, tests, book and dev-dependencies are left
out; nothing else changed except the following.

## Pipeline cache (src/renderer/wgpu.rs)

At the end of every flush, femtovg dropped each cached render pipeline that
flush had not used. A frame is several flushes (the window, plus each layer
Slint draws to a texture, such as clipped or shadowed items), and they use
different pipelines, so the same pipelines were rebuilt and their shaders
recompiled every frame. Profiling an idle DJ Sugar window showed most of its
CPU there.

Now a pipeline is dropped only after going unused for
`PIPELINE_IDLE_FLUSHES` flushes in a row.

Remove this copy once a femtovg release fixes the cache upstream.
