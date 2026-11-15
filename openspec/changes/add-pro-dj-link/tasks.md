# Tasks

## 1. Test rig and Watch

- [ ] 1.1 `crates/prodjlink` (djsugar-prodjlink): discovery, status, beat and mixer packet decoding behind a trait; fixtures from Deckbench and djl-analysis
- [ ] 1.2 Deckbench integration test harness on Linux (pinned release, started and stopped by the test), run in CI
- [ ] 1.3 Watch: passive listener bound per interface; players list with model, number, play state, tempo, master, on air, loaded track; dbserver metadata queries with backoff when refused
- [ ] 1.4 Events: tempo master beats to OSC `/beat`, `/prodjlink/N/...` events, now-playing feed option, History rows for tracks played on players
- [ ] 1.5 app: Settings › Controllers › Pro DJ Link (switch, interface, number, players list, Beta label); `[ProDjLink]` controls; regenerate supported-controls.txt

## 2. Follow

- [ ] 2.1 Join as a virtual player: number claim (5 and up), keep-alives, status every 200 ms unicast to peers
- [ ] 2.2 engine: `LeaderSource::ProDjLink`; tempo, beat and bar from the master's beats and status through the lock-free leader path, smoothed as MIDI clock in is
- [ ] 2.3 app: Pro DJ Link in the MASTER panel leader choice; Deckbench test of a tempo change and bar alignment

## 3. Lead

- [ ] 3.1 Tempo master hand-off both ways (request, accept, successor in 0x9f, 3 s cancel), sync commands obeyed, beats sent while master
- [ ] 3.2 Lead allowed setting; Deckbench scenarios for take, give back, stopped master and refusal

## 4. Show tools

- [ ] 4.1 dbserver answers for DJ Sugar's own decks: title, artist, album, BPM, key, duration, artwork; unknown requests answered without an error
- [ ] 4.2 Deckbench and Beat Link Trigger checks of the metadata

## 5. DJM

- [ ] 5.1 Read on-air and fader start from a DJM; fader start plays the deck on that channel in external mixer mode

## 6. Docs

- [ ] 6.1 User guide: Pro DJ Link page (what it does, setup, player numbers, Beta, troubleshooting)

## Workflow follow-up

- Archive the change once its last PR is merged, so the main specs take its requirements.
