# demucs-core, vendored for DJ Sugar

This is the `demucs-core` crate from [demucs-rs](https://github.com/nikhilunni/demucs-rs)
(Apache-2.0, see LICENSE) at commit `805d6f72ede20282832ac78c634e8767716c0b21`: a
Rust port of Meta's HTDemucs v4 on [burn](https://burn.dev), which runs on Metal,
Vulkan or the CPU. DJ Sugar's `crates/separation` uses it for high-quality stems.

It is not published on crates.io, and the workspace's cargo-deny rules allow no
git dependencies, so it lives here. Only `Cargo.toml` changed: a licence and
description field were added and the dev-dependencies (for its tests, which
are left out) were removed.
