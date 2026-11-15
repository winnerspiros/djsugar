# platforms Specification

## Purpose
Where DJ Sugar runs and how it is built and shipped: macOS and Linux, the release builds, signing, where its files live, and its licence.

## Requirements

### Requirement: Supported platforms
DJ Sugar SHALL run on macOS 11+ (Apple Silicon and Intel) and Linux x86_64 with glibc 2.35+, ALSA and Vulkan. Windows SHALL NOT be supported before v2.0.

#### Scenario: Linux
- **WHEN** the AppImage runs on Ubuntu 22.04
- **THEN** DJ Sugar starts

### Requirement: Release builds
A release tag SHALL build a macOS zip (signed and notarized), a Linux AppImage and a Linux tarball, and publish `docs/release-notes.md` as the release body.

#### Scenario: Mac opens without workarounds
- **WHEN** the macOS zip is downloaded and opened
- **THEN** macOS opens it without a quarantine warning

### Requirement: Files on disk
Settings, the library, logs and add-ons SHALL live in `~/Library/Application Support/DJSugar` on macOS and `$XDG_CONFIG_HOME/djsugar` elsewhere (`DJSUGAR_CONFIG` overrides); the analysis cache in `~/Library/Caches/DJSugar` or `$XDG_CACHE_HOME/djsugar` (`DJSUGAR_CACHE`).

#### Scenario: Custom config
- **WHEN** DJ Sugar starts with `DJSUGAR_CONFIG=/tmp/x`
- **THEN** settings and the library are read from and written to /tmp/x

### Requirement: Licence
Source SHALL be GPL-2.0-or-later and built copies GPL-3.0 (Slint). Mixxx's controller mappings SHALL be kept as shipped with credit; no Mixxx C++ is copied.

#### Scenario: NOTICE
- **WHEN** a release is built
- **THEN** LICENSE and NOTICE are included
