# Tasks

## 1. Deck outputs (send decks to a DAW)

- [x] 1.1 engine: output roles Deck 1-4, Sampler, Groove Box; render buffer sized once for the widest layout; only routed pairs written (each Groove Box plays on its deck, so it comes out of that deck's output; no separate role)
- [x] 1.2 engine: Clean / Pre-fader / Post-fader taps in `Engine::process`, resolved per configuration, not per frame; `[Master],deck_tap`
- [x] 1.3 Tests: tap contents (EQ kill, crossfader), Post-fader taps sum to master; benchmark no routes vs four routes (< 1%, no-route case unchanged)
- [x] 1.4 app: Settings › Audio › Outputs rows for the new roles, tap choice, split-device warning; regenerate supported-controls.txt
- [x] 1.5 User guide: "Send decks to a DAW" (Ableton with BlackHole/Loopback, VB-Cable/ASIO, PipeWire/JACK)

## 2. External mixer mode

- [x] 2.1 engine: `[Master],external_mixer` bypasses EQ, filter, fader, crossfader, master FX and limiter for decks; mixer controls ignored
- [x] 2.2 app: "Mix on" switch (Settings › Audio › Mixer: DJ Sugar / External, above the outputs; External shows fixed Deck 1-4, Headphones and Sampler rows); slim deck strips with meters replacing the mixer in 2-Deck and 4-Deck layouts; Automix and limiter disabled with notes
- [x] 2.3 Sampler and Groove Box fall back to Deck 1's pair; mic/aux not mixed; recorder input source option (Settings › Audio › Recording input: master mix or an Auxiliary input, `[Master],record_source`)
- [ ] 2.4 Manual check with an interface into a hardware mixer (or Ableton as the "mixer")
- [x] 2.5 User guide: "External mixer" page; regenerate supported-controls.txt

## Workflow follow-up

- Archive the change once its PRs are merged, so the main specs take its requirements.
- add-ableton-set-export layer 1 and add-dvs should reuse the deck taps.
