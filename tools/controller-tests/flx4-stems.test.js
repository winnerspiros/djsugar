const fs = require('fs'), vm = require('vm');
const src = fs.readFileSync(process.argv[2] || 'res/controllers/Pioneer-DDJ-FLX4-2.0.script.js', 'utf8');
const state = {};            // "group|key" -> value
const sets = [];
const engine = {
  getValue: (g, k) => state[g + '|' + k] ?? 0,
  setValue: (g, k, v) => { state[g + '|' + k] = v; sets.push([g, k, v]); },
  setParameter: (g, k, v) => { state[g + '|' + k] = v; },
  getParameter: (g, k) => state[g + '|' + k] ?? 0,
  makeConnection: () => ({ disconnect() {} }),
  makeUnbufferedConnection: () => ({ disconnect() {} }),
  startTimer: () => 1, stopTimer: () => {}, beginTimer: () => 1, stopTimer2: () => {}, beginTransaction() {}, endTransaction() {},
  log() {}, getSetting: () => 0,
};
const midi = { sendShortMsg() {}, sendSysexMsg() {} };
const script = { midiDebug() {}, samplerRegEx: /\[Sampler(\d+)\]/, toggleControl() {}, absoluteNonLinearLin() {}, absoluteLinear() {},
  deckFromGroup: () => 1, debug() {} };
const ctx = vm.createContext({ engine, midi, script, console, Math, print: () => {}, JSON, Date, setTimeout: () => {}, clearTimeout: () => {} });
vm.runInContext(src, ctx);
const P = vm.runInContext('PioneerDDJFLX4', ctx);
let ok = true; const check = (n, c) => { console.log((c ? 'PASS ' : 'FAIL ') + n); ok = ok && c; };
const stemOn = (i) => state[`[Channel1_Stem${i}]|mute`];
const stemSet = (g, i) => state[`${g}_Stem${i}]|mute`];
// 1) SAMPLER with no shift -> SAMPLER mode
P._shiftDeck1 = false;
P.padModeKeyPressed(0, 0x22, 0x7F, 0x90, '');
check('plain SAMPLER enters sampler mode', P.padMode['[Channel1]'] === 'sampler');
// 2) SHIFT + SAMPLER -> STEMS mode
P._shiftDeck1 = true;
P.padModeKeyPressed(0, 0x22, 0x7F, 0x90, '');
check('SHIFT+SAMPLER enters stems mode', P.padMode['[Channel1]'] === 'stems');
check('deck2 untouched', P.padMode['[Channel2]'] !== 'stems');
P._shiftDeck1 = false;
// 3) pad 1 press in stems mode toggles stem 1 mute on [Channel1]
state['[Channel1]|stem_count'] = 4;
for (let i = 1; i <= 4; i++) state[`[Channel1_Stem${i}]|mute`] = 0;
P.samplerPadPressed(0, 0x30, 0x7F, 0x97, '[Sampler1]');
check('pad 1 toggles stem 1 mute on', state['[Channel1_Stem1]|mute'] === 1);
check('sampler did NOT load/play', !sets.some(s => s[0] === '[Sampler1]'));
P.samplerPadPressed(0, 0x30, 0x7F, 0x97, '[Sampler1]');
check('pad 1 again unmutes', state['[Channel1_Stem1]|mute'] === 0);
// 4) shift + pad 3 -> only stem 3 active
P.samplerPadShiftPressed(0, 0x32, 0x7F, 0x98, '[Sampler3]');
check('shift+pad3 leaves only stem3 unmuted', state['[Channel1_Stem3]|mute'] === 0 && state['[Channel1_Stem1]|mute'] === 1 && state['[Channel1_Stem2]|mute'] === 1);
// 5) pad 5 toggles stem 1 FX via the key-shift note path too
P.pitchPadPressed(0, 0x74, 0x7F, 0x97, '[Channel1]');
check('key-shift note 0x74 in stems mode does not change pitch', !sets.some(s => s[1] === 'pitch' || s[1] === 'pitch_adjust' || s[1] === 'rate'));
check('key-shift note 0x74 in stems mode hits stem FX', sets.some(s => s[0].includes('Stem1') || s[0].includes('QuickEffectRack1_[Channel1_Stem1]')));
// 6) release is swallowed (no sampler stop)
const before = sets.length;
P.samplerPadPressed(0, 0x30, 0x00, 0x97, '[Sampler1]');
check('release in stems mode does nothing to samplers', !sets.slice(before).some(s => s[0] === '[Sampler1]'));
// 7) sampler mode still plays samplers
P.padMode['[Channel1]'] = 'sampler';
state['[Sampler1]|track_loaded'] = 1;
P.samplerPadPressed(0, 0x30, 0x7F, 0x97, '[Sampler1]');
P.samplerPadPressed(0, 0x30, 0x00, 0x97, '[Sampler1]');
check('sampler mode still uses the sampler', sets.some(s => s[0] === '[Sampler1]'));
console.log(ok ? 'ALL PASS' : 'SOME FAILED');
process.exit(ok ? 0 : 1);
