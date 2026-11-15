# DJ Sugar on Windows

Windows ships as a preview from DJ Sugar 0.4: 64-bit Windows 10 and 11 on x64
PCs. Windows on ARM runs the x64 build through emulation (fine for testing,
for example in UTM on an Apple Silicon Mac); a native ARM64 build can come
later.

## Downloads

Each release has two Windows files, made by `scripts/bundle-windows.ps1`:

- `DJSugar-VERSION-windows-x64-setup.exe`: installs for the current user into
  `%LOCALAPPDATA%\Programs\DJ Sugar`, with a Start menu entry and an
  uninstaller. No administrator needed. DJ Sugar installed this way updates
  itself: it downloads the next setup, checks its SHA-256 against GitHub's,
  and runs it silently when the decks stop.
- `djsugar-VERSION-windows-x64.zip`: portable. Unzip anywhere and run
  `DJSugar.exe`. It doesn't update itself; the banner links to the release.

The setup is built with Inno Setup 6 from `res/windows/djsugar.iss`. Its
`AppId` must never change, or a new setup installs a second DJ Sugar instead
of updating the first.

## Where DJ Sugar keeps things

- Settings, library and logs: `%APPDATA%\DJ Sugar` (logs in `logs\djsugar.log`,
  the previous run in `djsugar.1.log`, crash reports as `crash-*.txt`).
- Analysis cache and stems: `%LOCALAPPDATA%\DJ Sugar`.

## Audio

The Settings > Audio device lists show Windows Audio (WASAPI, shared mode)
devices and, after them, ASIO drivers as `ASIO: <driver name>`. Pick an
ASIO driver for the lowest latency with a DJ controller or audio interface
that ships one; the buffer size is usually set in the driver's own control
panel. ASIO4ALL works for devices without an ASIO driver.

The build downloads Steinberg's ASIO SDK (GPLv3) through cpal's `asio`
feature, which also needs LLVM (libclang) for bindgen; the GitHub Windows
runners have it. Built copies of DJ Sugar were already GPL-3.0 because of
Slint, so this changes nothing about the licence (see NOTICE).

## Controllers

Windows lets only one app at a time open a MIDI device. If VirtualDJ,
rekordbox or another DJ app is running and holding the controller, DJ Sugar
can't open it; the log says so. Quit the other app and press Refresh in
Settings > Controllers.

## Code signing

Windows builds are unsigned for now, so Windows SmartScreen shows "Windows
protected your PC" on first run (More info > Run anyway), and the release
notes say so. SignPath Foundation's free signing for open source projects
turned DJ Sugar's application down (2026-10-29), so signing waits for a
later release. When it comes back, either route below fits the release
workflow.

SignPath (if a new application is accepted later):

1. Apply at https://signpath.org (open source program) with the repository
   URL. They check the project and its build; approval takes a few weeks.
2. Once approved, install the SignPath GitHub App on the repository and add
   the `SIGNPATH_API_TOKEN` repository secret, plus the organization and
   project ids they give you.
3. The `windows` job in `.github/workflows/release.yml` then uploads the
   unsigned zip contents and setup as an artifact, submits it with
   `signpath/github-action-submit-signing-request`, and publishes the signed
   files it gets back.

Azure Trusted Signing (about $10 a month, open to individuals in the US
and Canada) also works: `scripts/bundle-windows.ps1` runs `SIGN_COMMAND`
(for example a `signtool sign /dlib ...` line) on `DJSugar.exe` and on the
setup when that variable is set.

Once DJ Sugar is signed, the in-app updater also requires each new setup to
carry a valid signature from the same publisher as the running copy.

## Testing

CI builds and tests every pull request on `windows-latest`, and the
`windows-app` job attaches the setup and zip to the run. To try a build:
open the pull request's Checks tab, pick the CI run, and download
`djsugar-windows` from its artifacts.
