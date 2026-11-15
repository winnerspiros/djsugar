# updates-and-diagnostics Specification

## Purpose
Keeping DJ Sugar current and giving testers something to report with: the update check and installer, logs and crash reports, and the feedback links.

## Requirements

### Requirement: Update check
At start DJ Sugar SHALL ask GitHub once for the latest release (pre-releases skipped) unless Settings › Help "Check for updates" is off, sending nothing but the request. A version dismissed once SHALL not be shown again, and the banner SHALL never show while a deck plays.

#### Scenario: New version
- **WHEN** a newer release exists and no deck plays
- **THEN** a banner offers it

### Requirement: One-click install
The macOS app and the AppImage SHALL install an update themselves: the download must match GitHub's checksum and, on macOS, be notarized and signed by the same developer. DJ Sugar SHALL restart once the decks stop. Other installs SHALL get the release page.

#### Scenario: Bad checksum
- **WHEN** a downloaded update doesn't match its checksum
- **THEN** it is not installed and the DJ is told

### Requirement: Logs and crash reports
When not started from a terminal, output SHALL go to `logs/djsugar.log` in the settings folder (previous run kept as `djsugar.1.log`), and a panic on any thread SHALL write `logs/crash-<time>.txt` with the backtrace.

#### Scenario: Crash report
- **WHEN** a thread panics
- **THEN** a crash file with the backtrace is in the logs folder

### Requirement: Feedback
Settings › Help SHALL link to report a problem and suggest a feature with the version filled in, and open the logs folder.

#### Scenario: Report
- **WHEN** Report… is pressed
- **THEN** GitHub's issue form opens with the DJ Sugar version filled in
