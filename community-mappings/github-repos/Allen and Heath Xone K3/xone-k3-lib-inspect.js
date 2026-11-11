// xone-k3-lib-inspect.js — read-only runtime probes for K3Controller.
// Loaded after lib-controller and lib-funcs. No mutation.

'use strict';

(function() {
    if (typeof XoneK3 === 'undefined') {
        throw new Error('Xone K3: xone-k3-lib-inspect.js — xone-k3-lib-core.js must load first');
    }
    if (typeof XoneK3._requireLoad !== 'function') {
        throw new Error('Xone K3: xone-k3-lib-inspect.js — xone-k3-lib-core.js must load first');
    }
    XoneK3._requireLoad('xone-k3-lib-inspect.js',
        ['K3Controller', 'FUNCS', 'TOKENS', '_funcId']);

    function makeRow(width) {
        return function(label, value) {
            while (label.length < width) label += ' ';
            print('  ' + label + (value === undefined || value === null ? '—' : value));
        };
    }

    XoneK3._funcName = function(fn) {
        if (!fn) return null;
        for (var k in XoneK3.FUNCS) {
            if (XoneK3.FUNCS[k] === fn) return k;
        }
        return XoneK3._funcId(fn);
    };

    XoneK3.K3Controller.prototype.inspect = function(what, sub) {
        if (what === undefined || what === null || what === '') {
            this._inspectRoot();
            return;
        }
        if (what === 'patches') { this._inspectPatches(); return; }
        if (what === 'beat')    { this._inspectBeat();    return; }
        if (what === 'tokens')  { this._inspectTokens();  return; }
        if (what === 'dark')    { this._inspectDark();    return; }
        if (what === 'rate')    {
            print('[inspect] rate system removed — beatjump gated at ' +
                  XoneK3.BEATJUMP_GAP_MS + 'ms when in loop');
            return;
        }
        if (what === 'why') {
            if (!sub) {
                print('[inspect] why needs a binding name — inspect("why", "layer")');
                return;
            }
            this._inspectWhy(sub);
            return;
        }
        if (this.bindings[what]) { this._inspectBinding(what); return; }
        print('[inspect] no such target: ' + what);
    };

    XoneK3.K3Controller.prototype._inspectRoot = function() {
        var row = makeRow(24);

        print('');
        print('== Xone K3 "' + this.label + '" (ch 0x' +
            this.midiChannel.toString(16) + ') ==');

        row('activePatches:', this.activePatches.join(', ') || '(empty)');
        row('resolved stack:', this._buildStack().join(' → ') || '(empty)');
        row('top mode:', this.currentTopMode() || '—');
        row('modifiers:', JSON.stringify(this.modifiers));
        row('activeDeck:', this.activeDeck);
        row('activeFx:', this.activeFx);
        row('selectedDeckA/B:',
            this.selectedDeckA + '/' + this.selectedDeckB);
        row('selectedFxA/B:',
            this.selectedFxA + '/' + this.selectedFxB);
        row('selectedDeckCurrent/Alt:',
            this.selectedDeckCurrent + '/' + this.selectedDeckAlt);
        row('selectedFxCurrent/Alt:',
            this.selectedFxCurrent + '/' + this.selectedFxAlt);
        row('activeDeckVirtual:', this.activeDeckVirtual);
        row('activeLoopVirtual:', this.activeLoopVirtual);
        row('activeDisplaySource:', this.activeDisplaySource);

        var applied = 0, dark = 0;
        for (var name in this.bindings) {
            if (!this.bindings.hasOwnProperty(name)) continue;
            if (this._applied[name] !== undefined) applied++;
            else dark++;
        }
        print('');
        row('bindings active:', applied);
        row('bindings dark:', dark);
        row('beat LEDs:', this._beatLeds.length);
        row('beat clocks:', this._beatConns.length > 0
            ? ('open (' + this._beatConns.length + ' ch)')
            : 'closed');
        var k2 = 0;
        for (var idx in this.k2Indices) if (this.k2Indices[idx]) k2++;
        row('K2 indices:', k2);
        row('last loop touch:', this._lastLoopTouchMs
            ? JSON.stringify(this._lastLoopTouchMs)
            : '—');
        print('');
    };

    XoneK3.K3Controller.prototype._inspectBinding = function(name) {
        var b = this.bindings[name];
        var row = makeRow(14);

        print('');
        print('== ' + name + ' ==');
        row('type:', b.type);
        row('bound:', !!b._rawCfg);
        var txLabel = '—';
        if (b._transaction && b._transaction.active) {
            txLabel = b._transaction.orphaned ? 'active (orphaned)' : 'active';
        }
        row('tx:', txLabel);
        row('group:', b.group);
        row('inKey:', b.inKey);
        row('outKey:', b.outKey);
        row('buttonMode:', b.buttonMode);
        row('func:', XoneK3._funcName(b._func));
        row('args:', b._funcArgs && b._funcArgs.length
            ? JSON.stringify(b._funcArgs) : null);
        row('tokens:', b._usedTokens && b._usedTokens.length
            ? b._usedTokens.join(' ') : null);
        row('softTakeover:', b.softTakeover);
        row('noLED:', b._noLED);
        row('k2:', b._k2);
        row('conns:', b._conns.length + (b._valueConn ? 1 : 0));

        if (b._csFailed) {
            row('cs:', 'FAILED (see console for warning)');
        } else if (!b._csActive) {
            row('cs:', 'inactive');
        } else {
            var kind = b._csDeviation
                ? ('deviation (knob: ' + (b._csKnob ? b._csKnob._name : '?') + ')')
                : 'static';
            row('cs:', kind);
            if (b._csOn !== null && b._csOn !== undefined) {
                row('cs.on:', JSON.stringify(b._csOn));
            }
            if (b._csScheme) {
                row('cs.beat:', b._csScheme.beat ? 'yes' : 'no');
            }
        }
        print('');
    };

    XoneK3.K3Controller.prototype._inspectPatches = function() {
        print('');
        print('== patches ==');
        print('  active:  ' + (this.activePatches.join(', ') || '(empty)'));
        print('  stack:   ' + (this._buildStack().join(' → ') || '(empty)'));
        print('');
        var names = Object.keys(this.patches).sort();
        for (var i = 0; i < names.length; i++) {
            var n = names[i];
            var p = this.patches[n];
            if (!p || typeof p !== 'object') continue;
            var active = this.activePatches.indexOf(n) !== -1;
            var applies = active && this._patchActive(n);
            var mark = applies ? '•' : (active ? '·' : ' ');
            var notes = [];
            if (p._host) {
                notes.push('_host: ' + (Array.isArray(p._host)
                    ? p._host.join('|') : p._host));
            }
            if (p._shift) notes.push('_shift: ' + p._shift);
            if (p.patch) {
                notes.push('patch: ' + (Array.isArray(p.patch)
                    ? p.patch.join('|') : p.patch));
            }
            print('  ' + mark + ' ' + n +
                (notes.length ? ('   ' + notes.join('   ')) : ''));
        }
        print('');
    };

    XoneK3.K3Controller.prototype._inspectBeat = function() {
        print('');
        print('== beat ==');
        var open = this._beatConns.length > 0;
        print('  clocks:  ' + (open
            ? ('open (' + this._beatConns.length + ' channels)')
            : 'closed'));
        print('  loop tempo: real seconds (session clock)');
        print('');
        for (var ch = 1; ch <= 4; ch++) {
            var st = this._decks[ch];
            print('  deck ' + ch +
                '   count=' + st.count +
                '   active=' + st.active +
                '   hasBeat=' + st.hasBeat +
                '   playing=' + st.playing +
                '   loopActive=' + st.loopActive +
                '   displaySource=' + st.displaySource +
                '   origin=' + this._phraseOrigin[ch]);
            print('           loopClock=' +
                (st.loopClock.running ? 'running' : 'stopped') +
                ' pos=' + st.loopClock.pos.toFixed(4));
        }
        print('');
        if (this._beatLeds.length === 0) {
            print('  beat LEDs: (none)');
        } else {
            print('  beat LEDs:');
            for (var i = 0; i < this._beatLeds.length; i++) {
                var b = this._beatLeds[i];
                var scheme = b._csScheme;
                var raw = scheme && scheme.beat && scheme.beat.deck;
                var resolved = this._resolveDeckRef(raw);
                print('    ' + b._name +
                    '   scheme.beat.deck=' + (raw !== undefined ? raw : '(default)') +
                    ' → channel ' + (resolved || '?'));
            }
        }
        print('');
    };

    XoneK3.K3Controller.prototype._inspectTokens = function() {
        print('');
        print('== tokens ==');
        var counts = {};
        for (var ti = 0; ti < XoneK3.TOKENS.length; ti++) {
            counts[XoneK3.TOKENS[ti].token] = [];
        }
        for (var name in this.bindings) {
            if (!this.bindings.hasOwnProperty(name)) continue;
            var b = this.bindings[name];
            if (!b._usedTokens) continue;
            for (var i = 0; i < b._usedTokens.length; i++) {
                var tok = b._usedTokens[i];
                if (!counts[tok]) counts[tok] = [];
                counts[tok].push(name);
            }
        }
        var toks = Object.keys(counts).sort();
        for (var t = 0; t < toks.length; t++) {
            var tok = toks[t];
            var uses = counts[tok];
            if (uses.length === 0) {
                print('  ' + tok + '   (unused)');
            } else {
                print('  ' + tok + '   ' + uses.length + '  ' + uses.join(', '));
            }
        }
        print('');
    };

    XoneK3.K3Controller.prototype._inspectDark = function() {
        print('');
        print('== dark bindings ==');
        var any = false;
        var names = Object.keys(this.bindings).sort();
        for (var i = 0; i < names.length; i++) {
            var n = names[i];
            if (this._applied[n] !== undefined) continue;
            any = true;
            print('  ' + n);
        }
        if (!any) print('  (none)');
        print('');
    };

    // ============================================================
    // WHY — walk the pipeline for one binding
    // ============================================================
    // Read-only. Prints where in the render pipeline the binding
    // stopped.
    //
    //   XoneK3.controllers[0].inspect('why', 'layer')
    XoneK3.K3Controller.prototype._inspectWhy = function(name) {
        var b = this.bindings[name];
        if (!b) { print('[inspect] no such binding: ' + name); return; }

        var row = makeRow(24);

        var isOn = 0, onThrew = null;
        try { isOn = b._isOn(); } catch (e) { onThrew = e.message; }

        var ref = null, refThrew = null, refCompiled = null;
        if (b._csSchemeFn) {
            try {
                ref = b._csSchemeFn();
                if (ref) refCompiled = XoneK3.compileScheme(ref);
            } catch (e) { refThrew = e.message; }
        }

        var beatScheme = null;
        if (b._csCompiledMemo && b._csCompiledMemo.beat) beatScheme = b._csCompiledMemo;
        else if (b._csScheme && b._csScheme.beat) beatScheme = b._csScheme;
        else if (refCompiled && refCompiled.beat) beatScheme = refCompiled;

        var deckCh = beatScheme ? this._resolveDeckRef(beatScheme.beat.deck) : null;

        print('');
        print('== why: ' + name + ' ==');

        row('bound:', b._rawCfg ? 'yes' : 'NO — not in the active stack');
        row('idx:', b.idx >= 0
            ? (b.idx + ' (0x' + b.idx.toString(16) + ')')
            : (b.idx + ' — no LED'));
        row('type:', b.type);
        var txLabel = '—';
        if (b._transaction && b._transaction.active) {
            txLabel = b._transaction.orphaned ? 'active (orphaned)' : 'active';
        }
        row('tx:', txLabel);
        row('group:', b.group);
        row('inKey:', b.inKey);
        row('outKey:', b.outKey);
        row('func:', XoneK3._funcName(b._func));
        row('noLED:', b._noLED);
        row('k2:', b._k2);

        print('');
        print('  colorScheme:');
        row('  active:', b._csActive ? 'yes' : 'NO');
        row('  failed:', b._csFailed ? 'YES — see the console warning' : 'no');
        row('  deviation:', b._csDeviation
            ? 'yes (knob: ' + (b._csKnob ? b._csKnob._name : '?') + ')'
            : 'no');

        var src;
        if (b._csScheme && b._csSchemeFn) src = 'base + function';
        else if (b._csScheme)             src = 'base (static)';
        else if (b._csSchemeFn)           src = 'function only';
        else                              src = '—';
        row('  scheme source:', src);
        if (b._csScheme && b._csScheme.beat) row('  base has beat:', 'yes');

        if (b._csSchemeFn) {
            if (refThrew)                         row('  fn() →', 'THREW: ' + refThrew);
            else if (ref === undefined)           row('  fn() →', 'undefined');
            else if (ref === null)                row('  fn() →', 'null');
            else if (typeof ref === 'string')     row('  fn() →', "'" + ref + "'");
            else if (typeof ref === 'object')     row('  fn() →', 'object' +
                (Array.isArray(ref.colors)
                    ? (' colors[' + ref.colors.length + ']')
                    : ''));
            else                                  row('  fn() →', typeof ref);
            if (refCompiled && refCompiled.beat) row('  fn has beat:', 'yes');
        }

        if (b._csOn !== null && b._csOn !== undefined) {
            row('  on:', JSON.stringify(b._csOn));
        }

        if (b._csDeviation && b._csKnob) {
            row('  knob.group:', b._csKnob.group);
            row('  knob.inKey:', b._csKnob.inKey);
            row('  knob.pos:', b._csKnob._normalized === undefined
                ? 'NEVER REPORTED — LED shows boot-white'
                : b._csKnob._normalized.toFixed(3));
        }

        print('');
        print('  state:');
        if (onThrew) row('  isOn:', 'THREW: ' + onThrew);
        else         row('  isOn:', isOn);
        if ((b._csOn === null || b._csOn === undefined) && b.group && b.outKey) {
            row('  raw outKey:', engine.getValue(b.group, b.outKey));
        }

        print('');
        print('  clocks:');
        var clocksOpen = this._beatConns.length > 0;
        row('  open:', clocksOpen
            ? ('yes (' + this._beatConns.length + ' channels)')
            : 'NO — no beat LED attached');
        row('  attached LEDs:', this._beatLeds.length);

        if (beatScheme) {
            print('');
            print('  beat:');
            var rawDeck = beatScheme.beat.deck;
            row('  scheme.beat.deck:',
                (rawDeck === undefined ? '(default)' : String(rawDeck)) +
                (deckCh ? (' → channel ' + deckCh) : ' (unresolved)'));
            if (deckCh) {
                var st = this._decks[deckCh];
                row('  ch' + deckCh + '.hasBeat:', st.hasBeat);
                row('  ch' + deckCh + '.playing:', st.playing);
                row('  ch' + deckCh + '.active:',  st.active);
                row('  ch' + deckCh + '.count:',   st.count);
                row('  ch' + deckCh + '.source:',  st.displaySource);
            }
        }

        print('');
        print('  last LED write:');
        if (b.idx < 0) {
            row('  idx:', 'no LED mapped');
        } else if (this._ledCache[b.idx] !== undefined) {
            row('  idx ' + b.idx + ':',
                XoneK3._decodeLedCacheValue(this._ledCache[b.idx]) + '  (r:g:b:hw)');
        } else {
            row('  idx ' + b.idx + ':', 'nothing written yet');
        }

        print('');
        print('  verdict:');
        print('    ' + this._verdictFor(name, b, isOn, onThrew, beatScheme, deckCh));
        print('');
    };

    XoneK3.K3Controller.prototype._verdictFor = function(name, b, isOn, onThrew, beatScheme, deckCh) {
        if (!b._rawCfg) return 'binding is not in the active stack — check the patch tree.';
        if (b.idx < 0) return 'no physical LED is mapped to this control.';
        if (b._noLED) return 'noLED: true — this binding never sends LED frames.';
        if (b._csFailed) return 'colorScheme setup failed — check the console warning.';
        if (!b._csActive && !b.outKey) return 'no colorScheme and no outKey — nothing to render.';
        if (b._csDeviation && b._csKnob && b._csKnob._normalized === undefined)
            return 'deviation LED; knob has never reported, so it shows boot-white.';
        if (onThrew) return '_isOn() threw: ' + onThrew;

        if (isOn === 0) {
            // Order matches _evalCsOn's priority: patchActive →
            // co → state, then bare outKey when _csOn is null.
            if (b._csOn === false) return 'on: false — LED is intentionally off-ramp.';
            if (b._csOn && b._csOn.patchActive)
                return 'none of on.patchActive are active.';
            if (b._csOn && b._csOn.co)    return 'on.co "' + b._csOn.co + '" reads 0.';
            if (b._csOn && b._csOn.state) return 'on.state "' + b._csOn.state + '" is falsy.';
            if (b.outKey) return 'outKey "' + b.outKey + '" reads 0 — logicalOn is 0, LED is off.';
            return 'logicalOn is 0 with no obvious on/off source.';
        }

        if (beatScheme && deckCh) {
            var st = this._decks[deckCh];
            if (!st.hasBeat)
                return 'LED is on; beat has never fired on channel ' + deckCh +
                    '. Play the deck or check beat.deck.';
            if (!st.playing)
                return 'LED is on; beat has fired at least once, but deck is paused — ' +
                    'colour is parked, no pulse until play.';
        }

        return 'all preconditions met — LED should be lit. ' +
            'If not, check sysex throttle and _ledCache.';
    };

})();