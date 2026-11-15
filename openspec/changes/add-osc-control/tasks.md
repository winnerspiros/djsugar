# Tasks

## 1. OSC

- [x] 1.1 `crates/osc` (djsugar-osc): UDP thread with `rosc`; address ↔ control table built from the registry; set and query; local-only and send-only modes; tests with a real socket
- [x] 1.2 Feedback: per-client subscriptions with pattern matching, throttled to 30 Hz per control, send targets list
- [x] 1.3 Events: `/beat` and `/phrase` from the master deck's play position (the one the waveforms draw, read on an `osc events` thread every 5 ms; switch to the beat clock snapshot once add-midi-clock 1.1 exists) and phrase changes; `/deck/N/track` on load; `/deck/N/cue` on hot cue
- [x] 1.4 Bonjour advert `_osc._udp` with `mdns-sd`, only while open to the network (the phone remote's mDNS step can use the same crate)
- [x] 1.5 app: Settings › Controllers › OSC (switch, port, targets, local only, send only, status line)
- [x] 1.6 User guide: OSC page (address rules, events, Bonjour, a two-deck layout to build in TouchOSC as a table of addresses and ranges; the address list is supported-controls.txt read with the rule, not a separate generated page)

## Workflow follow-up

- Archive the change once its PR is merged, so the main specs take its requirements.
