# DJ Sugar

Open-source DJ software for Android, iOS, Windows, macOS and Linux, written in Rust.

This is the `rust` branch: a full rewrite of DJ Sugar. The previous C++/Qt version lives on
`main` and is the reference for every feature until this branch replaces it.

## Status

Work in progress. The plan and the state of every part are in
[docs/djsugar-rust-roadmap.md](docs/djsugar-rust-roadmap.md).

## Building

Needs a recent stable Rust, CMake and a C++ compiler. On Debian/Ubuntu also:

```sh
sudo apt install cmake libfontconfig-dev libxkbcommon-dev libasound2-dev libudev-dev libclang-dev
cargo run --release
```

Set `DJSUGAR_MUSIC=/path/to/music` to use a music folder other than `~/Music` (the variable is
renamed in the rebrand step of the roadmap).

## License and credits

GPL-2.0-or-later, see [LICENSE](LICENSE) and [NOTICE](NOTICE). Built copies are GPL-3.0 because
the UI uses [Slint](https://slint.dev).

The code base started from [openDJ](https://github.com/openwave-studio/opendj); controller
mappings and behaviour come from [Mixxx](https://mixxx.org). DJ Sugar is an independent project,
not affiliated with either.
