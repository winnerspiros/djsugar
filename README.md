# DJ Sugar

DJ software written in Rust for Android, Linux, Windows and macOS. This is a new codebase,
written from scratch.

- Design and plan: [docs/DESIGN.md](docs/DESIGN.md)
- Target screens at seven window sizes: [docs/screens](docs/screens)

## Crates

| Crate | Purpose |
| --- | --- |
| core | Musical time, tempo and key types |
| layout | Fluid layout solver: places modules for any window size |

## Build

```
cargo test --workspace
```
