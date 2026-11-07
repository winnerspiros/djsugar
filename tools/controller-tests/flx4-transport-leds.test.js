// Node test for the FLX4 mapping script: stem LEDs in STEMS mode and the
// pause -> play path. No device needed. Usage: node flx4-transport-leds.test.js [script]
const fs = require('fs'), vm = require('vm');
const file = process.argv[2] || 'res/controllers/Pioneer-DDJ-FLX4-2.0.script.js';
const src = fs.readFileSync(file, 'utf8');
const state = {}, calls = [], sent = [];
const engine = {
  getValue: (g, k) => state[g + '|' + k] ?? 0,
  setValue: (g, k, v) => { state[g + '|' + k] = v; calls.push(['setValue', g, k, v]); },
  setParameter: (g, k, v) => { state[g + '|' + k] = v; },
  getParameter: (g, k) => state[g + '|' + k] ?? 0,
  makeConnection: () => ({ disconnect() {} }),
  makeUnbufferedConnection: () => ({ disconnect() {} }),
  scratchEnable: (d, ...a) => calls.push(['scratchEnable', d]),
  scratchDisable: (d, ramp) => calls.push(['scratchDisable', d, !!ramp]),
  scratchTick() {}, brake() {}, softStart() {},
  beginTimer: () => 1, startTimer: () => 1, stopTimer() {}, log() {}, getSetting: () => 0,
  beginTransaction() {}, endTransaction() {},
};
const midi = { sendShortMsg: (st, n, v) => sent.push([st, n, v]), sendSysexMsg() {} };
const script = { midiDebug() {}, toggleControl: (g, k) => { state[g + '|' + k] = state[g + '|' + k] ? 0 : 1; calls.push(['toggle', g]); },
  samplerRegEx: /\[Sampler(\d+)\]/, absoluteNonLinearLin() {}, absoluteLinear() {}, deckFromGroup: () => 1, debug() {} };
const ctx = vm.createContext({ engine, midi, script, console, Math, JSON, Date, print() {}, setTimeout() {}, clearTimeout() {} });
vm.runInContext(src, ctx);
const P = vm.runInContext('PioneerDDJFLX4', ctx);
let ok = true; const check = (n, c) => { console.log((c ? 'PASS ' : 'FAIL ') + n); ok = ok && c; };

// ---- 1) Stem LEDs in STEMS mode
state['[Channel1]|stem_count'] = 4;
for (let i = 1; i <= 4; i++) state[`[Channel1_Stem${i}]|mute`] = 0;
P.padMode['[Channel1]'] = 'stems';
sent.length = 0;
P.stemMuteChanged(0, '[Channel1_Stem1]', null);          // unmuted -> lit
check('stem1 unmuted lights the sampler-note pad 1 (0x30)', sent.some(m => m[1] === 0x30 && m[2] === 0x7F));
check('stem1 unmuted lights the key-shift-note pad 1 (0x70)', sent.some(m => m[1] === 0x70 && m[2] === 0x7F));
sent.length = 0;
P.stemMuteChanged(1, '[Channel1_Stem1]', null);          // muted -> dark
check('stem1 muted darkens pad 1 on both note sets', sent.some(m => m[1] === 0x30 && m[2] === 0) && sent.some(m => m[1] === 0x70 && m[2] === 0));
P.stemFxChanged(1, '[QuickEffectRack1_[Channel1_Stem2]]', null); // fx on -> pad 6 lit
check('stem2 FX on lights pad 6 (0x35)', sent.some(m => m[1] === 0x35 && m[2] === 0x7F));
// 2) outside STEMS mode, sampler notes must not be touched
P.padMode['[Channel1]'] = 'sampler';
sent.length = 0;
P.stemMuteChanged(0, '[Channel1_Stem1]', null);
check('sampler mode: sampler-note LEDs untouched', !sent.some(m => m[1] === 0x30));
check('no keyboard-mode LEDs (stems are SHIFT+SAMPLER only)', !sent.some(m => m[1] === 0x40 || m[1] === 0x44));

// ---- 3) Pause -> play
state['[Channel1]|play'] = 1; state['[Channel1]|track_loaded'] = 1;
P._vinylWanted = [false, false];
// jog touch while playing, release while playing -> ramp allowed (vinyl mode on = scratch)
P._vinylWanted = [true, false];
P.jogTouch(0, 0x36, 0x7F, 0x90, '[Channel1]');
P.jogTouch(0, 0x36, 0x00, 0x90, '[Channel1]');
const relPlaying = calls.filter(c => c[0] === 'scratchDisable').pop();
check('release while playing ramps the scratch out', relPlaying && relPlaying[2] === true);
// pause the deck (vinyl mode stays on, so the jog scratches while paused)
P.playPressed(0, 0x0B, 0x7F, 0x90, '[Channel1]');
check('pause sets play=0', state['[Channel1]|play'] === 0);
// touch and release while paused -> no ramp, scratch off immediately
calls.length = 0;
P.jogTouch(0, 0x36, 0x7F, 0x90, '[Channel1]');
P.jogTouch(0, 0x36, 0x00, 0x90, '[Channel1]');
const relPaused = calls.filter(c => c[0] === 'scratchDisable').pop();
check('release while paused does NOT ramp', relPaused && relPaused[2] === false);
// press play straight after: scratch must be dropped first, then play starts
calls.length = 0;
P.playPressed(0, 0x0B, 0x7F, 0x90, '[Channel1]');
check('play starts the deck', state['[Channel1]|play'] === 1);
// 4) play while a scratch is still engaged (paused, touched, not released yet)
state['[Channel1]|play'] = 0;
P._scratchEnabled[0] = true;
calls.length = 0;
P.playPressed(0, 0x0B, 0x7F, 0x90, '[Channel1]');
const firstCall = calls[0];
check('engaged scratch is dropped before play, without ramp', firstCall && firstCall[0] === 'scratchDisable' && firstCall[2] === false);
check('deck plays after that', state['[Channel1]|play'] === 1);
console.log(ok ? 'ALL PASS' : 'SOME FAILED');
process.exit(ok ? 0 : 1);
