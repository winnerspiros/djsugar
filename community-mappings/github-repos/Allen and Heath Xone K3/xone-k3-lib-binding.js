// xone-k3-lib-binding.js — Binding primitive, its colour-scheme
// render path, the snapshot field list, and the transaction model
// for physical button presses.
//
// Loaded after lib-core and lib-schemes, before lib-controller
// (which constructs Bindings in _buildHardware).
//
// Session-scoped fields (_normalized, _ledObservers, _transaction,
// _togglePressActive, _lastOutKeyValue, _lastCoValue) are set in
// the constructor after _bindingDefaults() — _reset doesn't touch
// them.
//
// Transaction model
// -----------------
// A physical button press is a transaction. It is captured at
// physical press, dispatched at physical release, and survives
// rebinds (marked "orphaned") without dispatching early. This is
// what makes rebind-during-hold behave: the release fires against
// the config that was live when the button went down, not against
// whatever the binding happens to be now.
//
// The transaction also serves as the "is this physical control
// currently held" indicator for the modifier system (see
// lib-controller.js:_isHeld).
//
// _SNAPSHOT_FIELDS / _SNAPSHOT_IGNORED are derived from
// XoneK3._BINDING_FIELDS in lib-core.js. The transform names
// referenced from that table live here.

'use strict';

if (typeof XoneK3 === 'undefined') {
    throw new Error('Xone K3: xone-k3-lib-binding.js — xone-k3-lib-core.js must load first');
}
if (typeof XoneK3._requireLoad !== 'function') {
    throw new Error('Xone K3: xone-k3-lib-binding.js — xone-k3-lib-core.js must load first');
}
XoneK3._requireLoad('xone-k3-lib-binding.js',
    ['_funcId', 'resolve', 'readPath',
     '_tokenFields', 'CO_MATCH_EPSILON', 'BEAT_HOLD', 'applyBeat',
     '_BINDING_FIELDS', '_SNAPSHOT_FIELDS', '_SNAPSHOT_IGNORED']);

(function() {
    // ============================================================
    // RESET-ABLE BINDING FIELD FACTORY
    // ============================================================
    // Every field wiped on _reset (and therefore also set at
    // construction). Add a new reset-able field here AND to
    // XoneK3._BINDING_FIELDS in lib-core.js (as either snapshot
    // or ignored) so the boot-time audit stays clean.
    XoneK3._bindingDefaults = function() {
        return {
            group:       undefined,
            inKey:       undefined,
            outKey:      undefined,
            buttonMode:  'toggle',
            softTakeover: false,
            _func:       undefined,
            _funcArgs:   [],
            _usedTokens: [],
            _rawCfg:     null,
            _noLED:      false,
            _k2:         undefined,
            _csActive:          false,
            _csFailed:          false,
            _csDeviation:       false,
            _csScheme:          null,
            _csSchemeFn:        null,
            _csSchemeMemo:      null,
            _csCompiledMemo:    null,
            _csKnob:            null,
            _csFromStateConn:   null,
            _csOn:              null,
            _valueConn:  null,
            _conns:      [],
            _valueCurve: 1,
            _hotcueColorCO:      undefined,
            _cachedHotcueColor:  undefined,
            _shutdownHoldTimer: null
        };
    };

    // ============================================================
    // BINDING
    // ============================================================
    // Colour-scheme state (all reset by _reset):
    //   _csActive        colorScheme present; setup ran (ok or failed)
    //   _csFailed        setup failed; LED stays black
    //   _csDeviation     colorScheme.knob present; LED samples knob
    //   _csScheme        compiled static scheme (from cs.scheme)
    //   _csSchemeFn      live function scheme (fn, or schemeFromState)
    //   _csSchemeMemo    last fn() return — ref-identity for recompile
    //   _csCompiledMemo  compile of _csSchemeMemo
    //   _csKnob          sampled knob ref (deviation only)
    //   _csOn            parsed on: condition
    //   _csFromStateConn CO connection for schemeFromState.co
    //
    // Transaction state machine (session-scoped; survives _reset):
    //   idle ──press(v>0, push)──▶ active      (captures cfg)
    //   active ──rebind──▶ active+orphaned     (cfg differs)
    //   active ──release──▶ idle               (dispatch captured)
    //   * ──shutdown──▶ abandoned              (no dispatch)
    //
    // _transaction.active gates the push-mode dispatch path; a
    // binding rebound mid-hold dispatches the press-time func via
    // _dispatchCapturedRelease. See INTERNALS §2.5.
    XoneK3.Binding = function(name, spec, ctrl) {
        this._name = name;
        this._ctrl = ctrl;
        this.midi = spec.midi;
        this.type = spec.type;
        this.idx = spec.idx;

        var d = XoneK3._bindingDefaults();
        for (var k in d) {
            if (d.hasOwnProperty(k)) this[k] = d[k];
        }
        // Pots want soft-takeover on; buttons don't.
        if (spec.type === 'knob' || spec.type === 'fader') {
            this.softTakeover = true;
        }

        // Session-scoped — survive _reset.
        this._normalized = undefined;
        this._ledObservers = [];

        // Toggle-mode press-state flag, used by the bounce guard in
        // input(). Session-scoped — survives _reset so a rebind
        // during a hold doesn't lose track of press state and fire
        // a spurious toggle from the leftover physical press.
        this._togglePressActive = false;

        // Cached outKey CO value, seeded at bind, kept fresh by the
        // outKey listener. Removes a per-render CO read from _isOn.
        // Undefined means "not seeded yet" — _isOn falls back to a
        // live read.
        this._lastOutKeyValue = undefined;

        // Cached getParameter value for the binding's own CO. Only
        // populated for knob/fader types (the only valid targets for
        // a colorScheme.knob reference). Kept fresh by the value
        // listener. Removes a per-render getParameter read from the
        // co-match path. Undefined means "not seeded yet" or "not a
        // pot" — coMatchSample falls back to a live read.
        this._lastCoValue = undefined;

        // Physical-button transaction. Persists across _reset and
        // across rebinds (as "orphaned"). Set on physical press,
        // dispatched on physical release. See the file header.
        this._transaction = {
            active: false,
            orphaned: false,
            group: undefined,
            inKey: undefined,
            func: undefined,
            funcArgs: [],
            buttonMode: 'toggle'
        };
    };

    XoneK3.Binding.prototype._scaleValue = function(v) {
        if (v > 63) return (v - 1) / 126;
        return v / 128;
    };

    // Named predicate so the shutdown gate at each call site reads
    // as intent rather than plumbing. The MIDI-choke-point check
    // lives in K3Controller.sendLED — this is the render and
    // input-dispatch side.
    XoneK3.Binding.prototype._isShuttingDown = function() {
        return this._ctrl._shuttingDown === true;
    };

    // Note-on vs CC/note-off discriminator. Self-documents the
    // status-nibble test in XoneK3._wireInputs.
    XoneK3.Binding.prototype._isNoteOn = function() {
        return (this.midi[0] & 0xF0) === 0x90;
    };

    // ============================================================
    // TRANSACTION
    // ============================================================
    XoneK3.Binding.prototype._captureTransaction = function() {
        var t = this._transaction;
        t.active = true;
        t.orphaned = false;
        t.group = this.group;
        t.inKey = this.inKey;
        t.func = this._func;
        t.funcArgs = (this._funcArgs || []).slice();
        t.buttonMode = this.buttonMode;
    };

    // Fire the captured release against the captured config.
    // Temporarily installs the captured fields on `this` so the
    // func sees the same surface it saw at press time, then
    // restores. Any throw is caught and warned; state is restored
    // via try/finally.
    //
    // Funcs may read `this._transaction.orphaned` during dispatch
    // to suppress user-facing side effects when the press was
    // orphaned by a rebind.
    XoneK3.Binding.prototype._dispatchCapturedRelease = function() {
        var t = this._transaction;
        var saved = this._captureCurrentSurface();

        try {
            this._installSurface(t);
            if (t.func) {
                t.func.call(this, t.funcArgs, undefined, undefined, 0, undefined);
            } else {
                this._defaultInput(0, undefined);
            }
        } catch (e) {
            XoneK3._warnOnce('txReleaseThrow:' + this._name,
                'captured release threw on ' + this._name + ': ' +
                (e && e.message ? e.message : e));
        } finally {
            this._installSurface(saved);
        }
    };

    // Snapshot the dispatch-facing fields into a plain object,
    // then reinstall. The transaction holds the press-time
    // config; the binding may have been rebound mid-hold.
    // Temporarily installing the captured config makes the
    // release see the same surface it saw at press time (func,
    // args, group, inKey, buttonMode).
    XoneK3.Binding.prototype._captureCurrentSurface = function() {
        return {
            group:      this.group,
            inKey:      this.inKey,
            func:       this._func,
            funcArgs:   this._funcArgs,
            buttonMode: this.buttonMode
        };
    };

    XoneK3.Binding.prototype._installSurface = function(s) {
        this.group      = s.group;
        this.inKey      = s.inKey;
        this._func      = s.func;
        this._funcArgs  = s.funcArgs;
        this.buttonMode = s.buttonMode;
    };

    // ============================================================
    // INPUT — physical event dispatch
    // ============================================================
    // Pipeline:
    //   1. Shutdown gate — preset-reload window, no CO writes
    //   2. Value-side-effects (normalize knob, clear toggle on
    //      release)
    //   3. Snapshot pre-capture state for the bounce guard
    //   4. Open transaction on first push press
    //   5. Release with active transaction → dispatch captured
    //   6. Bounce guard → skip if hardware re-fire
    //   7. Dispatch func / default input
    //   8. Cache refresh + observer sweep
    XoneK3.Binding.prototype.input = function(ch, cc, v, s) {
        // Preset reload: Mixxx calls old shutdown (sets _shuttingDown)
        // then immediately wires new handlers. Physical input in that
        // window reaches the outgoing binding, which would write
        // stale COs (_defaultInput has no gate). Momentary presses
        // self-cancel via the matching release; toggles don't and
        // leave the old CO stuck at 1.
        if (this._isShuttingDown()) return;
        if (this._ctrl._onInput) this._ctrl._onInput(this);

        this._readPhysicalInput(v);

        var isPush = (this.type === 'button' && this.buttonMode === 'push');
        // Snapshot pre-capture state. _maybeOpenTransaction sets
        // active=true on first press; the push-mode bounce guard
        // must test the state BEFORE capture, or it blocks the
        // first press too.
        var wasActive = this._transaction.active;

        this._maybeOpenTransaction(v, isPush);

        // Physical release with an active transaction: dispatch
        // the captured release against the press-time config.
        // Happens even if the binding's current buttonMode is
        // 'toggle' — the transaction was captured when this was
        // a push binding, and it must be closed.
        if (v === 0 && this._transaction.active) {
            this._dispatchCapturedRelease();
            this._transaction.active = false;
            this._ctrl._onBindingRelease(this._name);
            this._refreshObservers();
            return;
        }

        if (this._isBounce(v, isPush, wasActive)) return;

        this._dispatchInput(ch, cc, v, s);
        this._postInputRefresh();
    };

    // Absorb value-dependent side-effects of a physical event:
    // knobs/faders update _normalized, buttons clear the toggle
    // press-state on release.
    //
    // Clearing on ANY release (not just when mode is toggle)
    // closes the toggle → push → toggle rebind case: a binding
    // pressed as toggle, rebound to push mid-hold, then released,
    // then rebound back to toggle would otherwise leave
    // _togglePressActive stuck true and swallow the next press.
    // Session-scoped — survives _reset.
    XoneK3.Binding.prototype._readPhysicalInput = function(v) {
        if (this.type === 'knob' || this.type === 'fader') {
            this._normalized = this._scaleValue(v);
        }
        if (this.type === 'button' && v === 0) {
            this._togglePressActive = false;
        }
    };

    // Open a transaction on the first push-mode press. Subsequent
    // v>0 events while the transaction is active are bounces
    // (see _isBounce) and don't re-capture.
    XoneK3.Binding.prototype._maybeOpenTransaction = function(v, isPush) {
        if (v > 0 && isPush && !this._transaction.active) {
            this._captureTransaction();
            this._ctrl._onBindingPress(this._name);
        }
    };

    // Bounce guards. A v>0 event while the transaction/press
    // state is already active is a hardware bounce — skip func
    // dispatch so per-press flags (_bpmHeldTurned, shutdownHold's
    // timer) aren't reset mid-hold.
    //
    // Push-mode: test pre-capture state (wasActive is snapshotted
    // before _maybeOpenTransaction). Testing the current
    // _transaction.active would return true on the very first
    // press, because the capture just set it.
    //
    // Toggle-mode: toggles don't capture transactions, so track
    // press state separately. A v>0 while _togglePressActive is
    // true is the same physical press; cleared on release in
    // _readPhysicalInput. Also checks _transaction.active — a
    // transaction captured by an earlier push-mode press (rebind
    // to toggle mid-hold) must skip here; the real release still
    // dispatches through _dispatchCapturedRelease.
    XoneK3.Binding.prototype._isBounce = function(v, isPush, wasActive) {
        if (v > 0 && isPush && wasActive) return true;
        if (this.type === 'button' && !isPush) {
            if (v > 0) {
                if (this._togglePressActive) return true;
                if (this._transaction.active) return true;
                this._togglePressActive = true;
            }
        }
        return false;
    };

    // Dispatch the func (or default input). When a transaction is
    // active, dispatch through it — a modifier-driven patch swap
    // inside _onBindingPress must not route the press through the
    // new binding's func while the release (via
    // _dispatchCapturedRelease) fires the old one. When no swap
    // happened the two are identical.
    //
    // A throwing func or engine call must not escape the MIDI
    // handler — Mixxx logs it or crashes depending on version,
    // and the observer repaint is skipped so LED state diverges
    // until the next input.
    XoneK3.Binding.prototype._dispatchInput = function(ch, cc, v, s) {
        try {
            var dispatchFn   = this._transaction.active
                ? this._transaction.func
                : this._func;
            var dispatchArgs = this._transaction.active
                ? this._transaction.funcArgs
                : this._funcArgs;
            if (dispatchFn) dispatchFn.call(this, dispatchArgs, ch, cc, v, s);
            else            this._defaultInput(v, s);
        } catch (e) {
            XoneK3._warnOnce('inputThrow:' + this._name,
                'binding func threw on ' + this._name + ': ' +
                (e && e.message ? e.message : e));
        }
    };

    // Refresh the cached CO value for knob/fader bindings, then
    // repaint observers. The value listener (armed in
    // _armOwnConn) fires on the next event-loop turn, so without
    // this the observer sweep renders against the pre-write
    // cached value — every deviation LED sampling this knob
    // flashes a spurious co-match gradient until the listener
    // catches up.
    //
    // Read back once per input event, not per render. Soft-
    // takeover rejection reads back the old CO, so co-match
    // shows until takeover is crossed, exactly as before.
    XoneK3.Binding.prototype._postInputRefresh = function() {
        if ((this.type === 'knob' || this.type === 'fader') &&
            this.group && this.inKey) {
            try { this._lastCoValue = engine.getParameter(this.group, this.inKey); }
            catch (e) { this._lastCoValue = undefined; }
        }
        this._refreshObservers();
    };

    XoneK3.Binding.prototype._defaultInput = function(v, s) {
        if (!this.group || !this.inKey) return;
        if (this.type === 'button') {
            if (this.buttonMode === 'push') {
                engine.setValue(this.group, this.inKey, v > 0 ? 1 : 0);
            } else if (v > 0) {
                var cur = engine.getValue(this.group, this.inKey);
                engine.setValue(this.group, this.inKey, cur ? 0 : 1);
            }
        } else if (this.type === 'knob' || this.type === 'fader') {
            engine.setParameter(this.group, this.inKey,
                this._applyValueCurve(this._normalized));
        }
    };

    XoneK3.Binding.prototype._applyValueCurve = function(n) {
        var c = this._valueCurve;
        if (!c || c === 1) return n;
        if (n <= 0) return 0;
        if (n >= 1) return 1;
        return Math.pow(n, c);
    };

    XoneK3.Binding.prototype.addObserver = function(led) {
        if (this._ledObservers.indexOf(led) === -1) this._ledObservers.push(led);
    };

    XoneK3.Binding.prototype.removeObserver = function(led) {
        var i = this._ledObservers.indexOf(led);
        if (i >= 0) this._ledObservers.splice(i, 1);
    };

    // A single throwing observer (bad scheme, transient CO miss)
    // must not starve the rest of the loop.
    XoneK3.Binding.prototype._refreshObservers = function() {
        for (var i = 0; i < this._ledObservers.length; i++) {
            var obs = this._ledObservers[i];
            try {
                obs._refresh();
            } catch (e) {
                XoneK3._warnOnce('obsRefreshThrow:' + obs._name,
                    'observer repaint threw on ' + obs._name + ': ' +
                    (e && e.message ? e.message : e));
            }
        }
    };

    XoneK3.Binding.prototype._reset = function() {
        this._ctrl._detachBeatLed(this);

        if (this._valueConn) { this._valueConn.disconnect(); }
        for (var i = 0; i < this._conns.length; i++) {
            if (this._conns[i]) this._conns[i].disconnect();
        }
        if (this._csFromStateConn) { this._csFromStateConn.disconnect(); }
        if (this._csKnob) { this._csKnob.removeObserver(this); }
        if (this._shutdownHoldTimer) {
            engine.stopTimer(this._shutdownHoldTimer);
        }

        // _transaction is session-scoped — not wiped here. The
        // transaction must survive rebinds.
        var d = XoneK3._bindingDefaults();
        for (var k in d) {
            if (d.hasOwnProperty(k)) this[k] = d[k];
        }
    };

    XoneK3.Binding.prototype._armOwnConn = function() {
        if (this._valueConn) { this._valueConn.disconnect(); this._valueConn = null; }
        if (this.group === undefined || this.inKey === undefined) return;
        var self = this;
        // Only knobs and faders need the cached getParameter value
        // for the co-match path — they're the only valid targets
        // for a colorScheme.knob reference. Buttons and encoders
        // skip the extra read.
        var isPot = (this.type === 'knob' || this.type === 'fader');
        // The callback value (`v`, unused here) is the raw CO value.
        // _lastCoValue feeds co-match's deviation maths, which needs
        // the normalised parameter (0..1) — `getParameter` applies
        // the range mapping for ranged COs (rate, pitch_adjust). For
        // standard [0,1] parameters (EQ, pregain, volume) the two
        // are identical, but using `v` directly would break custom
        // bindings to ranged COs. Keep the live read.
        this._valueConn = engine.makeConnection(this.group, this.inKey, function() {
            if (isPot) {
                try { self._lastCoValue = engine.getParameter(self.group, self.inKey); }
                catch (e) { self._lastCoValue = undefined; }
            }
            self._refreshObservers();
        });
        // Seed: Mixxx fires the listener on change, not on setup.
        // The first render after bind would find _lastCoValue
        // undefined and do a live read that never gets reused.
        // Seed here so the cache is valid from the first render
        // onward.
        if (isPot) {
            try { this._lastCoValue = engine.getParameter(this.group, this.inKey); }
            catch (e) { this._lastCoValue = undefined; }
        }
    };

    // Shutdown gate lives at the actual MIDI choke point
    // (K3Controller.sendLED). Checking here too would be
    // redundant — sendLED would return immediately either way.
    XoneK3.Binding.prototype._send = function(r, g, b, hw) {
        if (this.idx < 0) return;
        if (this._noLED) return;
        this._ctrl.sendLED(this.idx, r, g, b, hw, this._k2);
    };

    // ============================================================
    // ON-STATE DISPATCH
    // ============================================================
    XoneK3.Binding.prototype._isOn = function() {
        if (this._csOn !== null && this._csOn !== undefined) {
            if (this._csOn === true)  return 1;
            if (this._csOn === false) return 0;
            return this._evalCsOn(this._csOn);
        }
        if (this.group && this.outKey) {
            if (this._lastOutKeyValue !== undefined) {
                return this._lastOutKeyValue > 0 ? 1 : 0;
            }
            return engine.getValue(this.group, this.outKey) > 0 ? 1 : 0;
        }
        return 0;
    };

    XoneK3.Binding.prototype._evalCsOn = function(s) {
        if (s.patchActive !== undefined) return this._evalPatchActive(s.patchActive);

        var v;
        if (s.co !== undefined)         v = this._evalOnCoValue(s);
        else if (s.state !== undefined) v = this._evalOnStateValue(s);
        else                            v = undefined;

        return this._matchOnValue(s, v);
    };

    XoneK3.Binding.prototype._evalPatchActive = function(patchActive) {
        if (!Array.isArray(patchActive)) {
            return this._ctrl._patchActive(patchActive) ? 1 : 0;
        }
        for (var i = 0; i < patchActive.length; i++) {
            if (this._ctrl._patchActive(patchActive[i])) return 1;
        }
        return 0;
    };

    XoneK3.Binding.prototype._evalOnCoValue = function(s) {
        if (s.group === undefined) return undefined;
        if (s.cachedValue !== undefined) return s.cachedValue;
        return engine.getValue(s.group, s.co);
    };

    XoneK3.Binding.prototype._evalOnStateValue = function(s) {
        var v = XoneK3.readPath(this._ctrl, s.state);
        if (s.equals === undefined && s.contains === undefined) {
            var truthyByConstruction =
                (typeof v === 'number' && v > 0) ||
                (typeof v === 'string' && v.length > 0);
            if (truthyByConstruction) {
                XoneK3._warnOnce('csOnTruthyState:' + this._name,
                    'colorScheme.on.state "' + s.state + '" on ' + this._name +
                    ' resolves to a value (' + JSON.stringify(v) + ')' +
                    ' that is truthy by construction with no equals — the LED' +
                    ' will always be on. Add equals: V for a slot / mode comparison.');
            }
        }
        return v;
    };

    XoneK3.Binding.prototype._matchOnValue = function(s, v) {
        if (s.equals !== undefined) return (v === s.equals) ? 1 : 0;
        if (s.contains !== undefined) {
            if (Array.isArray(v)) return (v.indexOf(s.contains) !== -1) ? 1 : 0;
            if (typeof v === 'string') return (v.indexOf(s.contains) !== -1) ? 1 : 0;
            return 0;
        }
        return v ? 1 : 0;
    };

    // ============================================================
    // REFRESH / RENDER
    // ============================================================
    XoneK3.Binding.prototype._refresh = function() {
        if (this.idx < 0) return;
        if (this._noLED) return;
        if (this._isShuttingDown()) return;

        // CO observers registered via _bindConns,
        // _wireCsOnConnection, and _buildCsFromStateClosure call
        // _refresh directly with no wrapper. A throw would escape
        // through Mixxx's CO event loop. Catch here so every
        // _refresh call site is protected regardless of caller.
        // The external wrappers (_safeBinding, _refreshBeatLeds's
        // inline try/catch) stay for their own pre/post work but
        // no longer see throws from this body.
        try {
            if (this._csActive) {
                this._refreshColorScheme();
                return;
            }
            this._renderOutKey();
        } catch (e) {
            XoneK3._warnOnce('refreshThrow:' + this._name,
                'refresh threw on ' + this._name + ': ' +
                (e && e.message ? e.message : e));
        }
    };

    // Fallback render for bindings with no colorScheme — a plain
    // on/off white from outKey. Called from _refresh when
    // _csActive is false.
    XoneK3.Binding.prototype._renderOutKey = function() {
        if (!this.group || !this.outKey) return;
        var v = engine.getValue(this.group, this.outKey) > 0;
        this._send(v ? 127 : 0, v ? 127 : 0, v ? 127 : 0, v ? 127 : 0);
    };

    // ============================================================
    // COLOUR SEND
    // ============================================================
    XoneK3.Binding.prototype._sendColor = function(entry, isOn) {
        if (!entry) { this._send(0, 0, 0, 0); return; }
        var rgb = entry.rgb, type = entry.type || 'rgb';
        if (type === 'mixxx') {
            if (!isOn) { this._send(0, 0, 0, 0); return; }
            rgb = this._resolveMixxxColor();
            if (!rgb) { this._send(0, 0, 0, 0); return; }
        }
        var hw = isOn ? entry.brightness
                      : (entry.brightnessOff !== undefined && entry.brightnessOff !== null
                         ? entry.brightnessOff : entry.brightness);
        if (!rgb) { this._send(0, 0, 0, 0); return; }
        this._send(rgb[0], rgb[1], rgb[2], hw);
    };

    XoneK3.Binding.prototype._resolveMixxxColor = function() {
        // The colour CO name is precomputed at bind time
        // (_bindConns), and its value is seeded + kept current by
        // the colour listener. Falls back to a live read only
        // before the seed has landed.
        if (!this._hotcueColorCO) return null;
        if (!this.group) return null;
        var mx = this._cachedHotcueColor;
        if (mx === undefined) {
            try { mx = engine.getValue(this.group, this._hotcueColorCO); }
            catch (e) { return null; }
        }
        return XoneK3.mapMixxxColor(mx);
    };

    XoneK3.Binding.prototype._sendSampled = function(ramp, t, compiled, isOn, skipBeat) {
        var entry = this._ctrl._sampleRamp(ramp, t, compiled, isOn, skipBeat);
        if (entry === XoneK3.BEAT_HOLD) return;
        this._sendColor(entry, isOn);
    };

    XoneK3.Binding.prototype._beatStateFor = function(compiled) {
        if (!compiled || !compiled.beat) return null;
        var st = this._ctrl._deckStateFor(compiled);
        if (!st || !st.hasBeat) return null;
        return st;
    };

    XoneK3.Binding.prototype._isBeatOffPhase = function(compiled) {
        var st = this._beatStateFor(compiled);
        if (!st) return false;
        var mode = st.playing ? XoneK3.BEAT_PLAYING_MODE : XoneK3.BEAT_PAUSED_MODE;
        if (mode === 'solid') return false;
        if (mode === 'off')   return true;
        return !st.active;
    };

    XoneK3.Binding.prototype._renderStatic = function(scheme, isOn) {
        var ramp = isOn ? scheme.on : scheme.off;
        if (!ramp || ramp.length === 0) { this._send(0, 0, 0, 0); return; }
        var entry = ramp[0].color;
        entry = this._applyBeatStatic(entry, scheme, isOn);
        if (entry === XoneK3.BEAT_HOLD) return;
        this._sendColor(entry, isOn);
    };

    XoneK3.Binding.prototype._renderCoMatch = function(knob, scheme, isOn, physical) {
        if (!knob.group || !knob.inKey) return false;
        var steer = XoneK3.coMatchSample(scheme, knob.group, knob.inKey,
                                          physical, isOn, knob._lastCoValue);
        if (!steer) return false;
        this._sendSampled(steer.ramp, steer.t, scheme.co, isOn, true);
        return true;
    };

    XoneK3.Binding.prototype._renderMarker = function(scheme, isOn, devDir) {
        if (!XoneK3.markerHit(scheme, devDir)) return false;
        var entry = this._applyBeatStatic(scheme.marker, scheme, isOn);
        if (entry === XoneK3.BEAT_HOLD) return true;
        this._sendColor(entry, isOn);
        return true;
    };

    // Boot-white fallback for deviation LEDs whose knob has never
    // reported. Shared object — _sendColor reads indices only,
    // no mutation. Hoisted out of the per-render path.
    var BOOT_WHITE_ENTRY = { rgb: [127, 127, 127], brightness: 127, brightnessOff: 127 };

    XoneK3.Binding.prototype._renderDeviation = function(scheme, isOn) {
        var knob = this._csKnob;
        if (!knob || knob._normalized === undefined) {
            this._sendColor(BOOT_WHITE_ENTRY, isOn);
            return;
        }
        var physical = knob._normalized;
        if (this._renderCoMatch(knob, scheme, isOn, physical)) return;

        var dev = (physical - 0.5) * 2;
        var devDir = scheme.reverse ? -dev : dev;
        if (this._renderMarker(scheme, isOn, devDir)) return;

        var t = (devDir + 1) / 2;
        var ramp = isOn ? scheme.on : scheme.off;
        this._sendSampled(ramp, t, scheme, isOn, false);
    };

    // ============================================================
    // SCHEME EVALUATION
    // ============================================================
    XoneK3.Binding.prototype._evaluateScheme = function() {
        var scheme = this._csScheme;
        if (this._csSchemeFn) {
            var ref = this._csSchemeFn();
            if (ref !== undefined && ref !== null) {
                if (ref !== this._csSchemeMemo) {
                    this._warnIfFreshScheme(ref);
                    this._csSchemeMemo = ref;
                    this._csCompiledMemo = this._ctrl._resolveSchemeRef(ref);
                }
                if (this._csCompiledMemo) scheme = this._csCompiledMemo;
            }
        }
        return scheme;
    };

    XoneK3.Binding.prototype._warnIfFreshScheme = function(ref) {
        // Hoist the warn-once check above the expensive stringify
        // calls. Once warned, the two _stableStringify calls would
        // otherwise run on every render forever for a buggy config.
        var key = 'csSchemeFresh:' + this._name;
        if (XoneK3._warned[key]) return;

        var memo = this._csSchemeMemo;
        if (ref === null || ref === undefined) return;
        if (typeof ref !== 'object') return;
        if (memo === null || memo === undefined || typeof memo !== 'object') return;

        // Arrays are valid scheme refs (single-colour ramps). Treat
        // them like objects here — a fresh array each call is the
        // same recompile trap as a fresh object.
        var a = XoneK3._stableStringify(ref);
        var b = XoneK3._stableStringify(memo);
        if (a !== b) return;
        XoneK3._warnOnce(key,
            'colorScheme scheme function on ' + this._name +
            ' returns a fresh object/array each call — LED ' +
            'recompiles every render. Wrap with ' +
            'XoneK3.builders.composeScheme or hoist the value.');
    };

    // Render pipeline:
    //   1. Bail on _csFailed (setup error — LED stays black)
    //   2. Resolve logicalOn (on: condition / outKey / 0) and scheme
    //   3. Bail if scheme is unresolved (with diagnostics)
    //   4. Beat-off-phase gate — deck paused-off blacks the LED
    //   5. Render — deviation (knob) or static (ramp sample)
    XoneK3.Binding.prototype._refreshColorScheme = function() {
        if (this._csFailed) { this._send(0, 0, 0, 0); return; }

        var logicalOn = this._isOn();
        var scheme = this._evaluateScheme();
        if (!scheme) {
            if (this._csSchemeFn) {
                XoneK3._warnOnce('csDarkFromFn:' + this._name,
                    'colorScheme scheme function on ' + this._name +
                    ' returned null/undefined and no base scheme is set — LED stays dark.');
            }
            this._send(0, 0, 0, 0);
            return;
        }

        var beatOffPhase = this._isBeatOffPhase(scheme);
        if (beatOffPhase && !logicalOn) { this._send(0, 0, 0, 0); return; }
        var isOn = (logicalOn && !beatOffPhase) ? 1 : 0;

        if (this._csDeviation) this._renderDeviation(scheme, isOn);
        else                   this._renderStatic(scheme, isOn);
    };

    XoneK3.Binding.prototype._applyBeatStatic = function(entry, compiled, isOn) {
        if (!isOn) return entry;
        var st = this._beatStateFor(compiled);
        if (!st) return entry;
        return XoneK3.applyBeat(entry, compiled.beat, st.count);
    };

    // ============================================================
    // SNAPSHOT TRANSFORM
    // ============================================================
    XoneK3._snapshotTransform = function(transform, v) {
        if (transform === 'isPot')  return (v === 'knob' || v === 'fader');
        if (transform === 'truthy') return !!v;
        return v;
    };

    XoneK3._auditSnapshotFields = function() {
        if (!XoneK3._debugPrintsOn) return;
        var defaults = XoneK3._bindingDefaults();
        var sources = {};
        for (var i = 0; i < XoneK3._SNAPSHOT_FIELDS.length; i++) {
            sources[XoneK3._SNAPSHOT_FIELDS[i][0]] = true;
        }
        for (var k in defaults) {
            if (!defaults.hasOwnProperty(k)) continue;
            if (sources[k]) continue;
            if (XoneK3._SNAPSHOT_IGNORED.indexOf(k) !== -1) continue;
            print('[k3] reset-able Binding field "' + k +
                  '" not classified for _snapshotBinding');
        }
    };
})();