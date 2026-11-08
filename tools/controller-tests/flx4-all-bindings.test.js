// Node smoke test for the FLX4 mapping: runs init(), then presses and
// releases every script binding of the XML (with and without SHIFT), then
// shutdown(). Fails on any exception or on a binding whose function is
// missing. Every engine call is accepted, so it checks the script, not the
// engine. Usage (from the repo root):
//   node tools/controller-tests/flx4-all-bindings.test.js
const fs = require('fs'), vm = require('vm');
const xml = fs.readFileSync('res/controllers/Pioneer-DDJ-FLX4.midi.xml', 'utf8');
const scriptFile = /filename="([^"]+)"/.exec(xml)[1];
const src = fs.readFileSync('res/controllers/' + scriptFile, 'utf8');

const state = {};
const conn = () => ({ disconnect() {}, trigger() {}, isConnected: true });
const known = {
  getValue: (g, k) => state[g + '|' + k] ?? 0,
  setValue: (g, k, v) => { state[g + '|' + k] = v; },
  getParameter: (g, k) => state[g + '|' + k] ?? 0,
  setParameter: (g, k, v) => { state[g + '|' + k] = v; },
  makeConnection: conn, makeUnbufferedConnection: conn, connectControl: conn,
  beginTimer: () => 1, startTimer: () => 1, stopTimer() {},
  getSetting: () => undefined, isScratching: () => false,
};
// Any other engine function exists and does nothing
const engine = new Proxy(known, { get: (t, p) => (p in t ? t[p] : () => 0) });
const midi = new Proxy({}, { get: () => () => {} });
const script = {
  midiDebug() {}, debug() {}, deckFromGroup: g => +(/\d+/.exec(g) || [1])[0],
  toggleControl: (g, k) => engine.setValue(g, k, engine.getValue(g, k) ? 0 : 1),
  triggerControl: (g, k) => { engine.setValue(g, k, 1); engine.setValue(g, k, 0); },
  samplerRegEx: /\[Sampler(\d+)\]/, channelRegEx: /\[Channel(\d+)\]/,
  absoluteLin: () => 0, absoluteNonLin: () => 0, absoluteNonLinear: () => 0,
  absoluteNonLinearLin: () => 0, absoluteLinear: () => 0, posMod: (a, m) => ((a % m) + m) % m,
};
const ctx = vm.createContext({ engine, midi, script, console, Math, JSON, Date,
  print() {}, setTimeout() {}, clearTimeout() {}, components: undefined });
vm.runInContext(src, ctx);
const P = vm.runInContext('PioneerDDJFLX4', ctx);

let ok = true;
const fail = (what, e) => { ok = false; console.log('FAIL ' + what + ': ' + (e && e.stack ? e.stack.split('\n').slice(0, 2).join(' | ') : e)); };
try { P.init('DDJ-FLX4', false); console.log('PASS init'); } catch (e) { fail('init', e); }

const re = /<control>[\s\S]*?<group>([^<]*)<\/group>[\s\S]*?<key>([^<]*)<\/key>[\s\S]*?<status>([^<]*)<\/status>[\s\S]*?<midino>([^<]*)<\/midino>[\s\S]*?<\/control>/g;
let m, count = 0;
while ((m = re.exec(xml))) {
  const [, group, key, status, midino] = m;
  if (!key.startsWith('PioneerDDJFLX4.')) { continue; }
  const name = key.slice('PioneerDDJFLX4.'.length);
  const fn = name.split('.').reduce((o, p) => (o ? o[p] : undefined), P);
  if (typeof fn !== 'function') { fail(`${key} (${status} ${midino})`, 'not a function'); continue; }
  for (const deckPlaying of [0, 1]) {
    state[group + '|play'] = deckPlaying;
    state[group + '|track_loaded'] = 1;
    for (const value of [0x7F, 0x00, 0x41, 0x3F]) {
      try { fn.call(P, 0, parseInt(midino), value, parseInt(status), group); } catch (e) { fail(`${key} ${status} ${midino} v=${value}`, e); }
    }
  }
  count++;
}
try { P.shutdown(); console.log('PASS shutdown'); } catch (e) { fail('shutdown', e); }
console.log(`${count} script bindings exercised`);
console.log(ok ? 'ALL PASS' : 'SOME FAILED');
process.exit(ok ? 0 : 1);
