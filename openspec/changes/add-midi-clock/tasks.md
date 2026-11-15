# Tasks

## 1. Clock out

- [x] 1.1 engine: beat clock snapshot (heard-at host time, beat, tempo, playing) published per buffer through atomics after `sync`; shared reader for MIDI clock, OSC and the Groove Box
- [x] 1.2 Clock sender thread: schedule pulses ahead from the snapshot, sleep then spin to each pulse on every platform (through midir); virtual "DJ Sugar Clock" port on macOS and Linux
- [ ] 1.2b Only if hardware shows wobble: OS-timestamped back ends, Core MIDI with timestamps (macOS) and the ALSA sequencer queue (Linux)
- [x] 1.3 Start / Stop / Song Position on the downbeat, RESTART ON BAR, keep-running setting, per-output offset
- [x] 1.4 Jitter test harness (`cargo test -p djsugar-engine midi_clock -- --ignored --nocapture`: send time against due time in the clock thread), target < 0.5 ms spread idle; still to check with a drum machine's tempo display
- [x] 1.5 app: CLOCK OUT and RESTART ON BAR in the MASTER panel, Settings › Controllers › MIDI Clock (outputs, offsets, keep running); `[MidiClock]` controls; regenerate supported-controls.txt
- [x] 1.6 Update the user guide (Sync page: MIDI clock out)

## 2. Clock in and Link follow

- [x] 2.1 Clock input reader (its own connection on the picked input, opened by the clock thread; controllers still drop `0xF8`-`0xFC`), Song Position, smoothing filter with tests
- [x] 2.2 engine: `[Master],sync_leader_source` Deck / Link / Clock; SYNC follows Link's or the clock's tempo and phase through the existing lock-free path; Link stops leading while it is followed
- [x] 2.3 app: leader choice in the MASTER panel; regenerate supported-controls.txt
- [x] 2.4 Update the user guide (Sync page: following Link or a drum machine)

## Workflow follow-up

- Archive the change once its PRs are merged, so the main specs take its requirements.
