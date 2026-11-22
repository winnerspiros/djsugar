# DJ Sugar

DJ software written in Rust for Android, Linux, Windows and macOS. This is a new codebase,
written from scratch.

- Design and plan: [docs/DESIGN.md](docs/DESIGN.md)
- Screen mockup: [docs/mockup/index.html](docs/mockup/index.html). Open it in a browser and
  resize the window to see the layout adapt; renders at seven screen sizes are in
  docs/mockup/*.png.

## Crates

| Crate | Purpose |
| --- | --- |
| core | Musical time, tempo and key types |
| layout | Fluid layout solver: places modules for any window size |

## Build

```
cargo test --workspace
```
