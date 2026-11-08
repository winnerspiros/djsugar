// Node test for the FLX4 mapping script: PAD FX (held pads), KEYBOARD mode,
// BEAT FX select/beats and Fader Start. No device needed.
// Usage (from the repo root): node tools/controller-tests/flx4-pads-fx.test.js [script]
const fs = require('fs'), vm = require('vm');
const file = process.argv[2] || 'res/controllers/Pioneer-DDJ-FLX4-2.0.script.js';
const src = fs.readFileSync(file, 'utf8');
const state = {}, calls = [], sent = [];
const engine = {
  getValue: (g, k) => state[g + '|' + k] ?? 0,
  setValue: (g, k, v) => { state[g + '|' + k] = v; calls.push([g, k, v]); },
  setParameter: (g, k, v) => { state[g + '|' + k] = v; },
  getParameter: (g, k) => state[g + '|' + k] ?? 0,
  makeConnection: () => ({ disconnect() {} }),
  makeUnbufferedConnection: () => ({ disconnect() {} }),
  scratchEnable() {}, scratchDisable() {}, scratchTick() {}, brake() {}, softStart() {},
  beginTimer: () => 1, startTimer: () => 1, stopTimer() {}, log() {}, getSetting: () => 0,
};
const midi = { sendShortMsg: (st, n, v) => sent.push([st, n, v]), sendSysexMsg() {} };
const script = { midiDebug() {},
  toggleControl: (g, k) => engine.setValue(g, k, engine.getValue(g, k) ? 0 : 1),
  triggerControl: (g, k) => { engine.setValue(g, k, 1); engine.setValue(g, k, 0); },
  samplerRegEx: /\[Sampler(\d+)\]/, absoluteNonLinearLin() {}, absoluteLinear() {}, deckFromGroup: () => 1, debug() {} };
const ctx = vm.createContext({ engine, midi, script, console, Math, JSON, Date, print() {}, setTimeout() {}, clearTimeout() {} });
vm.runInContext(src, ctx);
const P = vm.runInContext('PioneerDDJFLX4', ctx);
let ok = true; const check = (n, c) => { console.log((c ? 'PASS ' : 'FAIL ') + n); ok = ok && c; };
const last = (g, k) => calls.filter(c => c[0] === g && c[1] === k).map(c => c[2]);

// ---- PAD FX: pad held = effect on, release = off
P.padMode['[Channel1]'] = 'padfx1';
P.padFxPadPressed(0, 0x12, 0x7F, 0x97, '[Channel1]');
check('PAD FX 1 pad 3 press -> [PadFx_Channel1],pad3 = 1', state['[PadFx_Channel1]|pad3'] === 1);
check('held pad LED lit', sent.some(m => m[0] === 0x97 && m[1] === 0x12 && m[2] === 0x7F));
P.padFxPadPressed(0, 0x12, 0x00, 0x97, '[Channel1]');
check('release -> pad3 = 0', state['[PadFx_Channel1]|pad3'] === 0);
P.padMode['[Channel2]'] = 'padfx2';
P.padFxPadPressed(0, 0x55, 0x7F, 0x99, '[Channel2]');
check('PAD FX 2 pad 6 on deck 2 -> [PadFx_Channel2],pad14', state['[PadFx_Channel2]|pad14'] === 1);
P.padFxPadPressed(0, 0x55, 0x00, 0x99, '[Channel2]');

// ---- KEYBOARD: first pad picks (and sets) the hot cue, then pads play it pitched
P.padMode['[Channel1]'] = 'keyboard';
P.keyboardCue['[Channel1]'] = 0;
calls.length = 0;
P.keyboardPad(0, 0x42, 0x7F, 0x97, '[Channel1]');
check('first pad sets empty hot cue 3', last('[Channel1]', 'hotcue_3_set').includes(1));
P.keyboardPad(0, 0x40, 0x7F, 0x97, '[Channel1]');
check('next pad plays hot cue 3', last('[Channel1]', 'hotcue_3_gotoandplay').includes(1));
check('pad 1 = +4 semitones', state['[Channel1]|pitch_adjust'] === 4);
P.keyboardPadShift(0, 0x47, 0x7F, 0x98, '[Channel1]');
P.keyboardPad(0, 0x40, 0x7F, 0x97, '[Channel1]');
check('SHIFT + pad 8 moves the range down', state['[Channel1]|pitch_adjust'] === 0);

// ---- BEAT FX: select steps the type, beats halve/double
state['[BeatFx]|effect'] = 0;
P.beatFxSelectPressed(0, 0, 0x7F);
check('BEAT FX SELECT -> next effect', state['[BeatFx]|effect'] === 1);
P.beatFxSelectShiftPressed(0, 0, 0x7F);
P.beatFxSelectShiftPressed(0, 0, 0x7F);
check('SHIFT + SELECT wraps back to the last effect', state['[BeatFx]|effect'] === P.BEAT_FX_COUNT - 1);
P._beatFxBeatIndex = 5;
P.beatFxLeftPressed(0, 0, 0x7F);
check('BEAT ◀ steps 1 -> 3/4', state['[BeatFx]|beats'] === 0.75);
P.beatFxRightPressed(0, 0, 0x7F);
P.beatFxRightPressed(0, 0, 0x7F);
check('BEAT ▶ doubles to 2', state['[BeatFx]|beats'] === 2);

// ---- Fader Start
state['[Channel1]|play'] = 0;
P.faderStartPlay(0, 0x66, 0x7F, 0x90, '[Channel1]');
check('SHIFT + fader up plays', state['[Channel1]|play'] === 1);
calls.length = 0;
P.faderStartCue(0, 0x52, 0x7F, 0x90, '[Channel1]');
check('SHIFT + fader down returns to cue', last('[Channel1]', 'cue_gotoandstop').includes(1));

// ---- Pause sends only play = 0 (never a cue action)
state['[Channel1]|play'] = 1; state['[Channel1]|track_loaded'] = 1;
P._vinylWanted = [false, false];
calls.length = 0;
P.playPressed(0, 0x0B, 0x7F, 0x90, '[Channel1]');
check('pause sets play=0', state['[Channel1]|play'] === 0);
check('pause touches no cue control', !calls.some(c => /cue/.test(c[1])));

console.log(ok ? 'ALL PASS' : 'SOME FAILED');
process.exit(ok ? 0 : 1);
