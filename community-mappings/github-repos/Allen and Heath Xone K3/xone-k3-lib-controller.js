// xone-k3-lib-controller.js — K3Controller core: constructor,
// patch resolution, focus, top-half mode table, binding
// application, and the modifier-state system.
//
// Loaded BEFORE lib-controller-colour.js, lib-controller-beat.js,
// lib-controller-render.js, lib-controller-boot.js. The other four
// extend K3Controller.prototype and require K3Controller to exist.
// Inspection probes live in lib-inspect.js.
//
// Modifier states
// ---------------
// Named held-states declared in cfg.modifierStates. Each modifier
// is active while every binding in its heldBy list has an active
// transaction (see lib-binding.js). State changes fire optional
// hooks (patchActivate, recompute, onActivate/onDeactivate) and
// batch into at most one recompute.
//
// Two-axis shape:
//   - State change → patch change (patchActivate) OR recompute
//   - Idempotent: every press/release event enforces that
//     activePatches matches modifier state for patchActivate
//     modifiers. This handles cases where a config func removes
//     a modifier-owned patch out of band (e.g. snake).
//
// Shipped modifiers: shift, chord, loadArm.
// Replaces: isShifted, _chordActive, loadState.pressed, pressHeld,
// _bpmHeldEncoder, _bpmHeldOwner.

'use strict';

if (typeof XoneK3 === 'undefined') {
    throw new Error('Xone K3: xone-k3-lib-controller.js — xone-k3-lib-core.js must load first');
}
if (typeof XoneK3._requireLoad !== 'function') {
    throw new Error('Xone K3: xone-k3-lib-controller.js — xone-k3-lib-core.js must load first');
}
XoneK3._requireLoad('xone-k3-lib-controller.js',
    ['Binding', '_bindingDefaults', '_SNAPSHOT_FIELDS',
     '_snapshotTransform', '_buildTransferRGB',
     '_tokenFields', '_BINDING_FIELDS', 'PATCH_NAMES']);

(function() {
    var PN = XoneK3.PATCH_NAMES;

    var EMPTY_ARGS = [];

    // ============================================================
    // TOP-HALF MODE TABLE
    // ============================================================
    XoneK3.TOP_MODES = {};
    XoneK3.TOP_MODES.focus = {
        owns: [
            PN.FOCUS_BASE,
            PN.FOCUS_LAYOUT + '1', PN.FOCUS_LAYOUT + '2',
            PN.FOCUS_LAYOUT + '3', PN.FOCUS_LAYOUT + '4'
        ],
        activate: function(ctrl) {
            return [
                PN.FOCUS_BASE,
                PN.FOCUS_LAYOUT + ctrl.selectedFxCurrent
            ];
        }
    };
    XoneK3.TOP_MODES.twoDeck = {
        owns: [
            PN.TWODECK_BASE,
            PN.TWODECK_A_LAYOUT + '1', PN.TWODECK_A_LAYOUT + '2',
            PN.TWODECK_A_LAYOUT + '3', PN.TWODECK_A_LAYOUT + '4',
            PN.TWODECK_B_LAYOUT + '1', PN.TWODECK_B_LAYOUT + '2',
            PN.TWODECK_B_LAYOUT + '3', PN.TWODECK_B_LAYOUT + '4'
        ],
        activate: function(ctrl) {
            return [
                PN.TWODECK_BASE,
                PN.TWODECK_A_LAYOUT + ctrl.selectedFxA,
                PN.TWODECK_B_LAYOUT + ctrl.selectedFxB
            ];
        }
    };
    XoneK3.TOP_MODES.twoDeckMini = {
        owns: [
            PN.MINI_BASE,
            PN.MINI_LAYOUT + '1', PN.MINI_LAYOUT + '2',
            PN.MINI_LAYOUT + '3', PN.MINI_LAYOUT + '4'
        ],
        activate: function(ctrl) {
            return [
                PN.MINI_BASE,
                PN.MINI_LAYOUT + ctrl.selectedFxCurrent
            ];
        }
    };

    // ============================================================
    // STATE FACTORIES
    // ============================================================
    function freshVirtualClock() {
        return { running: false, pos: 0, lastTs: 0 };
    }

    // Per-channel timeline + beat state. Replaced wholesale by
    // _resetBeat and _closeBeatClocks; not mutated in place.
    //
    //   count            1-based cascade beat number (derived)
    //   active           pulse phase — true in first half of beat
    //   hasBeat          position is meaningful (deck has seen beats)
    //   loopActive       live loop_enabled snapshot
    //   playing          live play_indicator snapshot
    //   displaySource    'track' or 'loop'
    //   loopClock        accumulated virtual position (loop mode)
    //   lastBeatRepaint  throttle timestamp (beat_active edges)
    //   lastBeatEdge     last beat_active fire timestamp
    //   lastPositionSync throttle timestamp (playposition ticks)
    //   lastPlayPos      last playposition value (direction detect)
    //   hotcueState      {slot → {color, pos}} phrase-marker cache
    function freshDeckState() {
        return {
            count: 1, active: false, hasBeat: false, loopActive: false,
            playing: false,
            displaySource: 'track',
            loopClock: freshVirtualClock(),
            lastBeatRepaint: 0, lastBeatEdge: 0, lastPositionSync: 0,
            lastPlayPos: undefined,
            hotcueState: {}
        };
    }

    XoneK3._freshDeckState = freshDeckState;

    // ============================================================
    // CONTROLLER
    // ============================================================
    XoneK3.K3Controller = function(cfg) {
        this.label = cfg.label;
        this.midiChannel = cfg.midiChannel;

        // Clamp to 1..4. A non-numeric, out-of-range, or zero
        // initial value would otherwise resolve $DECK_A$ etc. to
        // [Channel<bad>] and silently break every channel-scoped
        // func on that slot (_channelOf rejects 1..4 only).
        function clamp1to4(v, fb) {
            var n = parseInt(v, 10);
            if (!isFinite(n) || n < 1) return fb;
            if (n > 4) return 4;
            return n;
        }
        this.selectedDeckA = clamp1to4(cfg.initialDeckA, 1);
        this.selectedDeckB = clamp1to4(cfg.initialDeckB, 2);
        this.selectedFxA  = clamp1to4(cfg.initialFxA,  1);
        this.selectedFxB  = clamp1to4(cfg.initialFxB,  2);

        this.activeDeck = (cfg.initialActiveDeck === 'B') ? 'B' : 'A';
        this.activeFx   = (cfg.initialActiveFx   === 'B') ? 'B' : 'A';

        this.selectedDeckCurrent = this.selectedDeckA;
        this.selectedDeckAlt     = this.selectedDeckB;
        this.selectedFxCurrent   = this.selectedFxA;
        this.selectedFxAlt       = this.selectedFxB;

        this.activePatches = (cfg.initialPatches || []).slice();
        this.patches = cfg.patches || {};

        // Modifier-state system. Declared per-config; state is
        // derived from transaction activity on the heldBy list.
        this.modifierStates = cfg.modifierStates || {};
        this.modifiers = {};
        for (var mn in this.modifierStates) {
            if (this.modifierStates.hasOwnProperty(mn)) this.modifiers[mn] = false;
        }

        this._lastLoopTouchMs = { 1: 0, 2: 0, 3: 0, 4: 0 };

        // Set true if encoderL is turned while modifiers.bpmPressHeld
        // is active (pressL held). Read+cleared by F.bpmPress on
        // release to suppress the shift+pressL headSplit toggle when
        // the user actually turned the encoder during the hold.
        this._bpmHeldTurned = false;

        this._shuttingDown = false;

        this._batchPatchesActive = false;

        this._recomputeDepth = 0;
        this._recomputeCompletedAt = 0;
        this._refreshAllLedsDepth = 0;

        this._resolvedTargetHashCache = new WeakMap();

        this.sysex = (cfg.sysex === false) ? false : true;

        var cp = cfg.channelPerceptual;
        if (!Array.isArray(cp) || cp.length < 3) {
            XoneK3._warnOnce('channelPerceptualShape:' + this.label,
                'channelPerceptual must be a 3-element array on ' + this.label +
                ' — got ' + JSON.stringify(cp) + '; using [1,1,1]');
            cp = [1, 1, 1];
        }
        this.channelPerceptual = cp.slice();

        // Same validation pattern as channelPerceptual. `wbGain`
        // is read by _wbRGB, whose own guard only catches short
        // arrays at use time — `undefined` crashes the slice
        // here, and a string ('foo') passes .slice() but feeds
        // NaN through _channelEmit, silently degrading calibrated
        // schemes. Warn-once + neutral fallback matches the
        // channelPerceptual precedent.
        var wb = cfg.wbGain;
        if (!Array.isArray(wb) || wb.length < 3) {
            XoneK3._warnOnce('wbGainShape:' + this.label,
                'wbGain must be a 3-element array on ' + this.label +
                ' — got ' + JSON.stringify(wb) + '; using [1,1,1]');
            wb = [1, 1, 1];
        }
        this.wbGain = wb.slice();

        this.hkBoost           = cfg.hkBoost;
        this.lumGamma          = cfg.lumGamma;
        this.calibrationMode   = (cfg.calibrationMode === 'rms') ? 'rms' : 'off';
        this.wbActive          = (cfg.wbActive === false) ? false : true;

        this._phraseOrigin = { 1: 0, 2: 0, 3: 0, 4: 0 };

        this._loadedState = { 1: false, 2: false, 3: false, 4: false };
        this._loadSettlingAt = { 1: 0, 2: 0, 3: 0, 4: 0 };

        // Boot settle tracking. Each boot-relevant CO fire stamps
        // _bootSettledAt[ch] while _bootSettling[ch] is true. The
        // boot timer reschedules _catchUpChannel while the last
        // stamp is younger than BOOT_SETTLE_MS, capped at
        // BOOT_MAX_ATTEMPTS. See lib-controller-beat.js.
        this._bootSettledAt = { 1: 0, 2: 0, 3: 0, 4: 0 };
        this._bootAttempt   = { 1: 0, 2: 0, 3: 0, 4: 0 };
        this._bootSettling  = { 1: false, 2: false, 3: false, 4: false };

        this.activeDeckVirtual   = false;
        this.activeLoopVirtual   = false;
        this.activeDisplaySource = 'track';

        this._decks = {
            1: freshDeckState(),
            2: freshDeckState(),
            3: freshDeckState(),
            4: freshDeckState()
        };
        this._rateCache = { 1: null, 2: null, 3: null, 4: null };
        this._reverseCache = { 1: 0, 2: 0, 3: 0, 4: 0 };
        this._refreshActiveAlt();

        this._beatLeds = [];
        this._beatConns = [];
        this._playConns = [];
        this._onBeat = null;

        this._ledCache = {};
        this._sysexQueue = [];
        this._sysexQueueMap = {};
        this._sysexTimer = null;

        this._transfer = null;
        if (typeof cfg.calibration === 'string') {
            var curve = (XoneK3.LED_TRANSFERS || {})[cfg.calibration];
            if (curve && curve.r && curve.g && curve.b) {
                this._transfer = XoneK3._buildTransferRGB(curve);
            } else {
                print('Xone K3: calibration curve "' + cfg.calibration +
                      '" missing for ' + this.label + '; calibration off.');
            }
        }
        if (!this._transfer) this.calibrationMode = 'off';

        this._trackMetaCache = {};
        this._hwCache = {};
        this._hwCacheSize = 0;
        this._flatCache = {};
        this._flatCacheSize = 0;
        this._calibCurveName = (typeof cfg.calibration === 'string')
            ? cfg.calibration : 'none';

        this._buildHardware();

        this.k2Indices = {};
        for (var pname in this.patches) {
            if (!this.patches.hasOwnProperty(pname)) continue;
            var patch = this.patches[pname];
            if (!patch || typeof patch !== 'object') continue;
            for (var bname in patch) {
                if (!patch.hasOwnProperty(bname)) continue;
                if (bname.charAt(0) === '_') continue;
                if (bname === 'patch') continue;
                var bcfg = patch[bname];
                if (!bcfg || typeof bcfg !== 'object') continue;
                if (bcfg.k2 !== true) continue;
                var b = this.bindings[bname];
                if (b && b.idx >= 0) this.k2Indices[b.idx] = true;
            }
        }
        if (Array.isArray(cfg.k2Indices)) {
            for (var ki = 0; ki < cfg.k2Indices.length; ki++) {
                this.k2Indices[cfg.k2Indices[ki]] = true;
            }
        }

        this._applied = {};
        this._appliedHashes = {};
        this._resolvedPatchCache = {};

        for (var pi = 0; pi < this.activePatches.length; pi++) {
            var pWant = this._resolveFxPatchName(this.activePatches[pi]);
            if (pWant) this.activePatches[pi] = pWant;
        }
    };

    // ============================================================
    // HARDWARE
    // ============================================================
    XoneK3.K3Controller.prototype._buildHardware = function() {
        var ch = this.midiChannel;
        var self = this;
        var bindings = {};

        function add(name, status, note, type) {
            var idx = XoneK3.noteToLEDIndex(note);
            bindings[name] = new XoneK3.Binding(
                name,
                { midi: [status + ch, note], type: type, idx: idx },
                self
            );
        }

        for (var i = 0; i < 4; i++) {
            add('encoder' + i, 0xB0, 0x00 + i, 'encoder');
            add('fader' + i,   0xB0, 0x10 + i, 'fader');
            add('press' + i,   0x90, 0x34 + i, 'button');
        }
        for (var r = 0; r < 3; r++) {
            for (var c = 0; c < 4; c++) {
                add('knob' + c + '_' + r, 0xB0, 0x04 + r * 4 + c, 'knob');
                add('button' + c + '_' + r, 0x90, 0x30 - r * 4 + c, 'button');
            }
        }
        add('encoderL', 0xB0, 0x14, 'encoder');
        add('encoderR', 0xB0, 0x15, 'encoder');
        add('pressL',   0x90, 0x0D, 'button');
        add('pressR',   0x90, 0x0E, 'button');

        for (var r2 = 0; r2 < 4; r2++) {
            for (var c2 = 0; c2 < 4; c2++) {
                add('grid' + c2 + '_' + r2, 0x90, 0x24 - r2 * 4 + c2, 'button');
            }
        }

        add('layer', 0x90, XoneK3.LAYER_BUTTON_NOTE, 'button');
        add('shift', 0x90, XoneK3.SHIFT_BUTTON_NOTE, 'button');

        this.bindings = bindings;
    };

    // ============================================================
    // MODIFIERS
    // ============================================================
    XoneK3.K3Controller.prototype._isHeld = function(name) {
        var b = this.bindings[name];
        return !!(b && b._transaction && b._transaction.active);
    };

    XoneK3.K3Controller.prototype._onBindingPress = function(name) {
        try {
            this._updateModifiers();
        } catch (e) {
            XoneK3._warnOnce('updateModifiersPressThrow:' + name,
                '_onBindingPress(' + name + ') threw: ' +
                (e && e.message ? e.message : e));
        }
    };

    XoneK3.K3Controller.prototype._onBindingRelease = function(name) {
        try {
            this._updateModifiers();
        } catch (e) {
            XoneK3._warnOnce('updateModifiersReleaseThrow:' + name,
                '_onBindingRelease(' + name + ') threw: ' +
                (e && e.message ? e.message : e));
        }
    };

    // True if any active patch declares _noModifiers. Suppresses
    // modifier-driven patch enforcement (snake removes chord out
    // of band and must not have it re-added while it's running).
    XoneK3.K3Controller.prototype._hasNoModifierPatch = function() {
        for (var i = 0; i < this.activePatches.length; i++) {
            var p = this.patches[this.activePatches[i]];
            if (p && p._noModifiers && this._patchActive(this.activePatches[i])) return true;
        }
        return false;
    };

    // Recompute the modifier set from current transaction state.
    // Two effects per modifier, applied in one pass:
    //   - State change: fire onActivate/onDeactivate, schedule
    //     patch add/remove via patchActivate, or set recompute.
    //   - Idempotent patch sync: for patchActivate modifiers,
    //     ensure activePatches matches modifier state even when
    //     the state didn't change (config func may have removed
    //     the patch out of band — snake does this).
    //
    // Split into _deriveModifierChanges (pure transition
    // computation + inline hooks) and _applyModifierChanges
    // (batch + recompute). Patch additions / removals batch into
    // one _batchPatches, which owns the recompute. Any changes
    // with recompute: true but no patch change trigger a
    // standalone _recompute.
    XoneK3.K3Controller.prototype._updateModifiers = function() {
        var changes = this._deriveModifierChanges();
        this._applyModifierChanges(changes);
    };

    // Walk modifierStates; derive active-state from transaction
    // membership on heldBy. State transitions fire
    // onActivate/onDeactivate hooks inline. Independently,
    // patch-activation deltas are collected for the caller to
    // batch.
    //
    // Patch-activation is idempotent: even when state didn't
    // change, check activePatches against current state. Config
    // funcs (snake) may remove an owned patch out of band; this
    // pass restores it.
    //
    // Returns { patchChanges, needsRecompute }. patchChanges is
    // a list of { name, active }. needsRecompute is true iff
    // some state changed under a recompute: true modifier —
    // used when no patch delta is queued so the caller still
    // needs a repaint.
    XoneK3.K3Controller.prototype._deriveModifierChanges = function() {
        var patchChanges = [];
        var needsRecompute = false;
        var suppressPatchActivate = this._hasNoModifierPatch();

        for (var name in this.modifierStates) {
            if (!this.modifierStates.hasOwnProperty(name)) continue;
            var cfg = this.modifierStates[name];
            var heldBy = cfg.heldBy || [];
            var active = heldBy.length > 0;
            for (var i = 0; i < heldBy.length; i++) {
                if (!this._isHeld(heldBy[i])) { active = false; break; }
            }
            var prev = this.modifiers[name];
            if (prev !== active) {
                this.modifiers[name] = active;
                if (cfg.recompute) needsRecompute = true;
                if (active && typeof cfg.onActivate === 'function') {
                    cfg.onActivate.call(this, this);
                }
                if (!active && typeof cfg.onDeactivate === 'function') {
                    cfg.onDeactivate.call(this, this);
                }
            }

            // Idempotent patch enforcement. Fires whether or not
            // the state changed — config funcs may remove the
            // patch out of band.
            if (cfg.patchActivate && !suppressPatchActivate) {
                var have = (this.activePatches.indexOf(cfg.patchActivate) !== -1);
                if (have !== active) {
                    patchChanges.push({ name: cfg.patchActivate, active: active });
                }
            }
        }
        return { patchChanges: patchChanges, needsRecompute: needsRecompute };
    };

    // Apply derived changes. Patch deltas batch into one
    // _batchPatches (which owns the recompute). A standalone
    // recompute fires when no patch deltas queued but the
    // recompute flag was raised by a state change.
    XoneK3.K3Controller.prototype._applyModifierChanges = function(changes) {
        var patchChanges = changes.patchChanges;
        if (patchChanges.length > 0) {
            this._batchPatches(function(patches) {
                for (var p = 0; p < patchChanges.length; p++) {
                    var pc = patchChanges[p];
                    if (pc.active) {
                        if (patches.indexOf(pc.name) === -1) patches.push(pc.name);
                    } else {
                        var idx = patches.indexOf(pc.name);
                        if (idx !== -1) patches.splice(idx, 1);
                    }
                }
            });
        } else if (changes.needsRecompute) {
            this._recompute();
        }
    };

    // ============================================================
    // PATCH RESOLUTION
    // ============================================================
    XoneK3.K3Controller.prototype._parentList = function(name) {
        var p = this.patches[name];
        if (!p || !p.patch) return [];
        return Array.isArray(p.patch) ? p.patch : [p.patch];
    };

    XoneK3.K3Controller.prototype._resolvePatch = function(name, seen) {
        var cached = this._resolvedPatchCache[name];
        if (cached !== undefined) return cached;

        var p = this.patches[name];
        if (!p) return {};
        seen = seen || {};
        if (seen[name]) throw new Error('Patch cycle: ' + name);
        seen[name] = true;
        var out = {};
        var parents = this._parentList(name);
        for (var i = 0; i < parents.length; i++) {
            Object.assign(out, this._resolvePatch(parents[i], seen));
        }
        var own = Object.assign({}, p);
        delete own.patch;
        delete own._host;
        Object.assign(out, own);
        seen[name] = false;
        this._resolvedPatchCache[name] = out;
        return out;
    };

    XoneK3.K3Controller.prototype._collectShiftPatches = function(name, out, seen) {
        var p = this.patches[name];
        if (!p || seen[name]) return;
        seen[name] = true;
        var parents = this._parentList(name);
        for (var i = 0; i < parents.length; i++) {
            this._collectShiftPatches(parents[i], out, seen);
        }
        if (p._shift && out.indexOf(p._shift) === -1) out.push(p._shift);
    };

    XoneK3.K3Controller.prototype._patchActive = function(name) {
        if (this.activePatches.indexOf(name) === -1) return false;
        var p = this.patches[name];
        if (!p) return false;
        if (!p._host) return true;
        var hosts = Array.isArray(p._host) ? p._host : [p._host];
        for (var i = 0; i < hosts.length; i++) {
            if (this.activePatches.indexOf(hosts[i]) !== -1) return true;
        }
        return false;
    };

    // Resolve the active patch stack:
    //   1. Walk activePatches, filter by _patchActive, hold
    //      chord aside. A _noModifiers patch that's active sets
    //      the suppress flag.
    //   2. Append shift companions (deduped) if shift is held —
    //      unless suppressed.
    //   3. Append chord last (if held and not suppressed).
    XoneK3.K3Controller.prototype._buildStack = function() {
        var stack = [];
        var hasChord = false;
        var suppressModifiers = false;

        for (var i = 0; i < this.activePatches.length; i++) {
            var name = this.activePatches[i];
            var p = this.patches[name];
            if (!p) continue;
            var active = this._patchActive(name);
            // A _noModifiers patch whose _host isn't satisfied must
            // not suppress the modifier system — it isn't visible,
            // and _hasNoModifierPatch already gates on _patchActive.
            if (p._noModifiers && active) suppressModifiers = true;
            if (!active) continue;
            if (name === PN.CHORD) { hasChord = true; continue; }
            if (stack.indexOf(name) === -1) stack.push(name);
        }

        if (this.modifiers.shift && !suppressModifiers) {
            var shiftPatches = [], seen = {};
            for (var j = 0; j < stack.length; j++) {
                this._collectShiftPatches(stack[j], shiftPatches, seen);
            }
            for (var k = 0; k < shiftPatches.length; k++) {
                if (stack.indexOf(shiftPatches[k]) === -1) stack.push(shiftPatches[k]);
            }
        }

        if (hasChord && !suppressModifiers) stack.push(PN.CHORD);

        return stack;
    };

    XoneK3.K3Controller.prototype._resolveStack = function(stack) {
        var out = {};
        for (var i = 0; i < stack.length; i++) {
            Object.assign(out, this._resolvePatch(stack[i], {}));
        }
        return out;
    };

    // ============================================================
    // RECOMPUTE
    // ============================================================
    XoneK3.K3Controller.prototype._recompute = function() {
        this._recomputeDepth++;
        try {
            var resolved = this._resolveStack(this._buildStack());
            this._applyResolved(resolved);
            // _applyResolved wraps each binding in try/catch; the
            // subclass hook and the LED sweep do not. A throwing
            // _afterRecompute (custom LabController override) or a
            // throw out of _syncBeatClocks would otherwise skip the
            // trailing _refreshAllLeds and leave
            // _recomputeCompletedAt unset — causing spurious
            // "outside a recompute" warnings on the next refresh.
            try { this._afterRecompute(); }
            catch (e) {
                XoneK3._warnOnce('afterRecomputeThrow:' + this.label,
                    '_afterRecompute threw on ' + this.label + ': ' +
                    (e && e.message ? e.message : e));
            }
            try { this._refreshAllLeds(); }
            catch (e) {
                XoneK3._warnOnce('recomputeRefreshThrow:' + this.label,
                    '_refreshAllLeds threw on ' + this.label + ': ' +
                    (e && e.message ? e.message : e));
            }
            this._recomputeCompletedAt = Date.now();
        } finally {
            this._recomputeDepth--;
        }
    };

    // Subclass hooks — no-ops in base. Overridden by LabController
    // (auto-bake on patch change, MIDI-input hooks) and any custom
    // controller subclass.
    XoneK3.K3Controller.prototype._afterRecompute = function() {};
    XoneK3.K3Controller.prototype._onInput = function(b) {};
    XoneK3.K3Controller.prototype._onPatchChange = function(from, to) {};

    XoneK3.K3Controller.prototype._safeBinding = function(
            fn, thisArg, args, name, keyPrefix, msgPrefix) {
        try {
            fn.apply(thisArg, args);
        } catch (e) {
            XoneK3._warnOnce(
                keyPrefix + name,
                msgPrefix + name + ' threw: ' + (e && e.message ? e.message : e));
        }
    };

    XoneK3.K3Controller.prototype._refreshBinding = function(name) {
        var b = this.bindings[name];
        if (!b) return;
        this._safeBinding(b._refresh, b, EMPTY_ARGS, b._name,
            'refreshBindingThrow:', '_refreshBinding: ');
    };

    // ============================================================
    // APPLY RESOLVED
    // ============================================================
    // Hash-diff binding apply. Identity check reuses the previous
    // hash when the resolved cfg object is the same reference as
    // last pass — _resolvePatch memoizes per name, so a repeated
    // recompute over an unchanged stack returns the same cfg
    // objects. _applied itself holds those references; no
    // separate cfg map is needed.
    XoneK3.K3Controller.prototype._applyResolved = function(resolved) {
        var oldHashes = this._appliedHashes || {};
        var oldCfgs   = this._applied || {};
        var newHashes = {};

        var skipRefresh = this._recomputeDepth > 0;

        this._unbindRemoved(resolved, oldHashes, skipRefresh);

        for (var name in resolved) {
            if (!resolved.hasOwnProperty(name)) continue;
            if (name.charAt(0) === '_') continue;
            var cfg = resolved[name];
            var h = (oldCfgs[name] === cfg)
                ? oldHashes[name]
                : XoneK3._hashBinding(cfg);
            newHashes[name] = h;
            if (oldHashes[name] === h) continue;
            this._applyOneSafe(name, cfg, skipRefresh);
        }

        this._applied = resolved;
        this._appliedHashes = newHashes;
    };

    XoneK3.K3Controller.prototype._unbindRemoved = function(resolved, oldHashes, skipRefresh) {
        for (var oldName in oldHashes) {
            if (!oldHashes.hasOwnProperty(oldName)) continue;
            if (oldName.charAt(0) === '_') continue;
            if (resolved[oldName] !== undefined) continue;
            this._applyOneSafe(oldName, null, skipRefresh);
        }
    };

    XoneK3.K3Controller.prototype._applyOneSafe = function(name, cfg, skipRefresh, fingerprint) {
        try {
            this._applyBinding(name, cfg, fingerprint, skipRefresh);
        } catch (e) {
            var isUnbind = (cfg === null || cfg === undefined);
            var key    = (isUnbind ? 'applyUnbindThrow:' : 'applyBindThrow:') + name;
            var prefix = '_applyBinding(' + name +
                         (isUnbind ? ', null' : '') + ') threw: ';
            XoneK3._warnOnce(key, prefix + (e && e.message ? e.message : e));
        }
    };

    XoneK3.K3Controller.prototype._snapshotBinding = function(b) {
        var snap = {};
        for (var i = 0; i < XoneK3._SNAPSHOT_FIELDS.length; i++) {
            var src = XoneK3._SNAPSHOT_FIELDS[i][0];
            var dst = XoneK3._SNAPSHOT_FIELDS[i][1];
            var tr  = XoneK3._SNAPSHOT_FIELDS[i][2];
            snap[dst] = XoneK3._snapshotTransform(tr, b[src]);
        }
        return snap;
    };

    function argsEqual(a, b) {
        if (a === b) return true;
        if (!a || !b) return (a === b);
        if (a.length !== b.length) return false;
        for (var i = 0; i < a.length; i++) {
            if (a[i] !== b[i]) return false;
        }
        return true;
    }

    XoneK3.K3Controller.prototype._orphanTransaction = function(b) {
        var t = b._transaction;
        if (!t || !t.active) return;
        if (t.orphaned) return;
        var same =
            t.group      === b.group &&
            t.inKey      === b.inKey &&
            t.func       === b._func &&
            t.buttonMode === b.buttonMode &&
            argsEqual(t.funcArgs, b._funcArgs);
        if (!same) t.orphaned = true;
    };

    XoneK3.K3Controller.prototype._unbind = function(b, prev) {
        if (prev.wasPot && prev.group && prev.inKey) {
            engine.softTakeoverIgnoreNextValue(prev.group, prev.inKey);
        }
        if (prev.wasBound && !prev.wasNoLED && prev.idx >= 0) {
            this.sendLED(prev.idx, 0, 0, 0, 0, prev.wasK2);
        }
    };

    XoneK3.K3Controller.prototype._bindCore = function(b, cfg) {
        b._rawCfg = cfg;
        b.group   = XoneK3.resolve(cfg.group, this);
        b.inKey   = XoneK3.resolve(cfg.inKey, this);
        b.outKey  = cfg.outKey ? XoneK3.resolve(cfg.outKey, this) : undefined;
        b.buttonMode = (cfg.type === 'push') ? 'push' : 'toggle';
        b.softTakeover = (cfg.softTakeover !== undefined)
            ? cfg.softTakeover
            : (b.type === 'knob' || b.type === 'fader');
        b._noLED = cfg.noLED === true;
        b._k2 = (cfg.k2 === true) ? true : undefined;
        b._valueCurve = (typeof cfg.valueCurve === 'number' && cfg.valueCurve > 0)
            ? cfg.valueCurve : 1;
    };

    XoneK3.K3Controller.prototype._collectTokens = function(b, cfg) {
        var sources = XoneK3._tokenFields(cfg);
        for (var ti = 0; ti < XoneK3.TOKENS.length; ti++) {
            var tok = XoneK3.TOKENS[ti].token;
            for (var i = 0; i < sources.length; i++) {
                if (sources[i][1] !== undefined &&
                    typeof sources[i][1] === 'string' &&
                    sources[i][1].indexOf(tok) !== -1) {
                    b._usedTokens.push(tok);
                    break;
                }
            }
        }
    };

    XoneK3.K3Controller.prototype._rearmSoftTakeover = function(b, prev) {
        if (!prev.wasPot || !b.group || !b.inKey || !b.softTakeover) return;
        if (b.group === prev.group && b.inKey === prev.inKey) return;
        engine.softTakeover(b.group, b.inKey, false);
        engine.softTakeover(b.group, b.inKey, true);

        if (b._normalized !== undefined) {
            var co = engine.getParameter(b.group, b.inKey);
            if (Math.abs(co - b._normalized) <= XoneK3.TAKEOVER_EPSILON) {
                engine.setParameter(b.group, b.inKey, co);
            }
        }
    };

    XoneK3.K3Controller.prototype._bindFunc = function(b, cfg) {
        if (!cfg.func) return;
        var f = cfg.func;
        if (typeof f !== 'function') {
            XoneK3._warnOnce('funcType:' + b._name,
                'binding func must be a function reference, got ' +
                typeof f + ' on ' + b._name);
            return;
        }
        b._func = f;
        b._funcArgs = cfg.args || [];
    };

    XoneK3.K3Controller.prototype._bindConns = function(b) {
        b._armOwnConn();
        if (b.outKey && b.group) {
            var m = b.outKey.match(/^(hotcue_\d+)_status$/);
            if (m) {
                // Precompute the colour CO name so _resolveMixxxColor
                // doesn't regex-match on every render, and cache the
                // colour value on the binding so the render path
                // doesn't re-read the CO either. The listener keeps
                // the cache current; the seed covers the window
                // between bind and the first CO fire.
                b._hotcueColorCO = m[1] + '_color';
                b._conns.push(engine.makeConnection(b.group, b._hotcueColorCO, function(v) {
                    b._cachedHotcueColor = v;
                    b._refresh();
                }));
                try { b._cachedHotcueColor = engine.getValue(b.group, b._hotcueColorCO); }
                catch (e) { b._cachedHotcueColor = undefined; }
            }
        }
        if (b.group && b.outKey && (b._csOn === null || b._csOn === undefined)) {
            b._conns.push(engine.makeConnection(b.group, b.outKey, function(v) {
                b._lastOutKeyValue = v;
                b._refresh();
            }));
            // Seed the cache with a live read. Mixxx doesn't fire
            // the listener on setup — only on change — so the first
            // _refresh (in _applyBinding, right after _bindConns)
            // would otherwise find the cache undefined and do a
            // live read that never gets reused. Seeding here means
            // the cache is valid from the first render onward.
            try {
                b._lastOutKeyValue = engine.getValue(b.group, b.outKey);
            } catch (e) {
                b._lastOutKeyValue = undefined;
            }
        }
    };

    // ============================================================
    // RESOLVED-TARGET HASH
    // ============================================================
    XoneK3.K3Controller.prototype._tokenFingerprint = function() {
        var parts = [];
        for (var i = 0; i < XoneK3.TOKENS.length; i++) {
            parts.push(this[XoneK3.TOKENS[i].state]);
        }
        return parts.join('|');
    };

    XoneK3.K3Controller.prototype._resolvedTargetHash = function(cfg, fingerprint) {
        if (!cfg) return '';
        if (fingerprint === undefined) fingerprint = this._tokenFingerprint();

        var cached = this._resolvedTargetHashCache.get(cfg);
        if (cached && cached.fingerprint === fingerprint) return cached.value;

        var parts = [];
        for (var i = 0; i < XoneK3._BINDING_FIELDS.length; i++) {
            var row = XoneK3._BINDING_FIELDS[i];
            if (!row.token) continue;
            var v = XoneK3.readPath(cfg, row.name);
            if (v === undefined) v = '';
            parts.push(XoneK3.resolve(v, this) || '');
        }
        var value = parts.join('\u0001');
        this._resolvedTargetHashCache.set(cfg, { fingerprint: fingerprint, value: value });
        return value;
    };

    // Bind pipeline:
    //   1. Snapshot prev state, reset binding
    //   2. Unbind path (cfg null) — orphan tx, clear LED, bail
    //   3. Bind path:
    //      a. Core cfg (group/inKey/type/buttonMode/softTakeover)
    //      b. Tokens, softTakeover bridge, func
    //      c. Colour scheme + beat LED attach
    //      d. CO connections
    //      e. Resolved-target hash for future re-apply skip
    //      f. Render (skipped when caller passes skipRefresh)
    XoneK3.K3Controller.prototype._applyBinding = function(name, cfg, fingerprint, skipRefresh) {
        var b = this.bindings[name];
        if (!b) return;

        var prev = this._snapshotBinding(b);
        b._reset();

        if (cfg === null || cfg === undefined) {
            this._orphanTransaction(b);
            this._unbind(b, prev);
            b._resolvedTargetHash = undefined;
            return;
        }

        this._bindCore(b, cfg);
        XoneK3._auditBindingCfg(cfg, name);
        this._collectTokens(b, cfg);
        this._rearmSoftTakeover(b, prev);
        this._bindFunc(b, cfg);
        this._orphanTransaction(b);

        if (cfg.colorScheme !== undefined) {
            this._setupColorScheme(b, cfg.colorScheme);
        }
        if (b._csSchemeFn || (b._csScheme && b._csScheme.beat)) {
            this._attachBeatLed(b);
        }

        this._bindConns(b);

        b._resolvedTargetHash = this._resolvedTargetHash(cfg, fingerprint);

        if (!skipRefresh) b._refresh();
    };

    XoneK3.K3Controller.prototype._resolveSchemeRef = function(ref) {
        if (ref === undefined || ref === null) return null;
        return XoneK3.compileScheme(ref);
    };

    // ============================================================
    // PATCH LIST MUTATIONS
    // ============================================================
    // Batch-patch pipeline:
    //   1. Give mutator a copy; throw if already active (nest)
    //   2. Diff before/after; bail if unchanged
    //   3. Copy result back into activePatches
    //   4. One recompute (patches are already final)
    //   5. Fire per-patch change hooks AFTER recompute
    XoneK3.K3Controller.prototype._batchPatches = function(mutator) {
        if (this._batchPatchesActive) {
            throw new Error('_batchPatches cannot nest');
        }

        var copy = this.activePatches.slice();
        var before = this.activePatches;

        var returned;
        this._batchPatchesActive = true;
        try {
            returned = mutator(copy);
        } catch (e) {
            print('[k3] _batchPatches mutator threw: ' + (e && e.message ? e.message : e));
            throw e;
        } finally {
            this._batchPatchesActive = false;
        }
        var result = Array.isArray(returned) ? returned : copy;

        var diff = this._diffPatches(before, result);
        if (!diff) return;

        this.activePatches.length = 0;
        for (var m = 0; m < result.length; m++) this.activePatches.push(result[m]);

        this._recompute();

        for (var r = 0; r < diff.removed.length; r++) {
            this._onPatchChange(diff.removed[r], null);
        }
        for (var a = 0; a < diff.added.length; a++) {
            this._onPatchChange(null, diff.added[a]);
        }
    };

    XoneK3.K3Controller.prototype._diffPatches = function(before, after) {
        var same = (before.length === after.length);
        if (same) {
            for (var i = 0; i < before.length; i++) {
                if (before[i] !== after[i]) { same = false; break; }
            }
        }
        if (same) return null;

        var beforeSet = {};
        for (var b = 0; b < before.length; b++) beforeSet[before[b]] = true;
        var afterSet = {};
        for (var a = 0; a < after.length; a++) afterSet[after[a]] = true;

        var added = [], removed = [];
        for (var r = 0; r < before.length; r++) {
            if (!afterSet[before[r]]) removed.push(before[r]);
        }
        for (var n = 0; n < after.length; n++) {
            if (!beforeSet[after[n]]) added.push(after[n]);
        }
        return { added: added, removed: removed };
    };

    XoneK3.K3Controller.prototype.addPatch = function(name) {
        if (this._batchPatchesActive) {
            throw new Error('addPatch called inside a _batchPatches mutator — mutate the array argument instead');
        }
        if (!this.patches[name]) { print('Unknown patch: ' + name); return; }
        if (this.activePatches.indexOf(name) !== -1) return;
        this.activePatches.push(name);
        this._recompute();
        this._onPatchChange(null, name);
    };

    XoneK3.K3Controller.prototype.removePatch = function(name) {
        if (this._batchPatchesActive) {
            throw new Error('removePatch called inside a _batchPatches mutator — mutate the array argument instead');
        }
        var i = this.activePatches.indexOf(name);
        if (i === -1) return;
        this.activePatches.splice(i, 1);
        this._recompute();
        this._onPatchChange(name, null);
    };

    XoneK3.K3Controller.prototype.replacePatch = function(from, to) {
        if (this._batchPatchesActive) {
            throw new Error('replacePatch called inside a _batchPatches mutator — mutate the array argument instead');
        }
        if (from === to) return;
        if (!this.patches[to]) { print('Unknown patch: ' + to); return; }
        var i = this.activePatches.indexOf(from);
        if (i === -1) return;
        this.activePatches[i] = to;
        this._recompute();
        this._onPatchChange(from, to);
    };

    // ============================================================
    // TOP-HALF MODES
    // ============================================================
    XoneK3.K3Controller.prototype.currentTopMode = function() {
        for (var mode in XoneK3.TOP_MODES) {
            if (!XoneK3.TOP_MODES.hasOwnProperty(mode)) continue;
            if (this._patchActive(XoneK3.TOP_MODES[mode].owns[0])) return mode;
        }
        return null;
    };

    XoneK3.K3Controller.prototype.setTopMode = function(mode) {
        var target = XoneK3.TOP_MODES[mode];
        if (!target) { print('[k3] unknown top mode: ' + mode); return; }
        var ownedByAnyMode = {};
        for (var m in XoneK3.TOP_MODES) {
            if (!XoneK3.TOP_MODES.hasOwnProperty(m)) continue;
            var owns = XoneK3.TOP_MODES[m].owns;
            for (var i = 0; i < owns.length; i++) ownedByAnyMode[owns[i]] = true;
        }
        var ctrl = this;
        this._batchPatches(function(patches) {
            var insertAt = patches.length;
            for (var j = patches.length - 1; j >= 0; j--) {
                if (ownedByAnyMode[patches[j]]) {
                    patches.splice(j, 1);
                    insertAt = j;
                }
            }
            var adds = target.activate(ctrl);
            for (var k = 0; k < adds.length; k++) {
                patches.splice(insertAt + k, 0, adds[k]);
            }
        });
    };

    // ============================================================
    // LED REPAINT
    // ============================================================
    XoneK3.K3Controller.prototype._refreshAllLeds = function(why) {
        if (this._refreshAllLedsDepth > 0) return;
        this._refreshAllLedsDepth++;
        try {
            if (this._recomputeDepth === 0 &&
                this._recomputeCompletedAt &&
                (Date.now() - this._recomputeCompletedAt) < 10 &&
                why === undefined) {
                XoneK3._warnOnce('redundantRefresh:' + this.label,
                    this.label + ': _refreshAllLeds called outside a recompute ' +
                    'within 10ms of one finishing — pass a reason string if intentional');
            }
            this._syncBeatClocks();
            for (var k in this.bindings) {
                if (!this.bindings.hasOwnProperty(k)) continue;
                var b = this.bindings[k];
                if (b.idx < 0) continue;
                this._safeBinding(b._refresh, b, EMPTY_ARGS, b._name,
                    'refreshAllThrow:', '_refreshAllLeds: ');
            }
        } finally {
            this._refreshAllLedsDepth--;
        }
    };

    XoneK3.K3Controller.prototype._validateBindings = function() {
        if (this._validated) return;
        this._validated = true;

        var issues = 0;
        var seen = {};

        for (var name in this.bindings) {
            if (!this.bindings.hasOwnProperty(name)) continue;
            var b = this.bindings[name];
            if (!b._rawCfg) continue;
            var sources = XoneK3._tokenFields(b._rawCfg);

            for (var s = 0; s < sources.length; s++) {
                var field = sources[s][0];
                var val   = sources[s][1];
                if (typeof val !== 'string') continue;
                var unknown = XoneK3._findUnknownTokens(val);
                for (var u = 0; u < unknown.length; u++) {
                    var key = name + '|' + field + '|' + unknown[u];
                    if (seen[key]) continue;
                    seen[key] = true;
                    XoneK3._warnOnce('token:' + key,
                        'unknown token ' + unknown[u] +
                        ' in ' + b._name + '.' + field);
                    issues++;
                }
            }
        }

        for (var modName in this.modifierStates) {
            if (!this.modifierStates.hasOwnProperty(modName)) continue;
            var mod = this.modifierStates[modName];
            var heldBy = mod.heldBy || [];
            for (var h = 0; h < heldBy.length; h++) {
                if (!this.bindings[heldBy[h]]) {
                    XoneK3._warnOnce('modifierHeldBy:' + modName + ':' + heldBy[h],
                        'modifier "' + modName + '" references unknown binding "' +
                        heldBy[h] + '"');
                    issues++;
                }
            }
            if (mod.patchActivate && !this.patches[mod.patchActivate]) {
                XoneK3._warnOnce('modifierPatch:' + modName,
                    'modifier "' + modName + '" references unknown patch "' +
                    mod.patchActivate + '"');
                issues++;
            }
        }

        this._validatePhraseHotcueSlots();

        if (issues > 0) {
            print('[k3] ' + issues + ' binding warning(s) — check the tokens above.');
        }
    };

    // ============================================================
    // FOCUS
    // ============================================================
    XoneK3.K3Controller.prototype._afterFocusChanged = function(tokens, reason) {
        this._refreshActiveAlt();
        this._reapplyByTokens(tokens, { skipRefresh: true });
        if (!this._syncFxLayout().swapped) this._refreshAllLeds(reason);
    };

    XoneK3.K3Controller.prototype.setSelectedFor = function(kind, which, value) {
        if (value < 1 || value > 4) return;
        var stateProp = XoneK3._selProp(kind, which);
        if (this[stateProp] === undefined || this[stateProp] === value) return;
        this[stateProp] = value;

        var upper = kind.toUpperCase();
        var tokens = [
            '$' + upper + '_' + which + '$',
            '$' + upper + '_' + which + '_N$'
        ];
        if (kind === 'deck') {
            if (which === this.activeDeck) tokens.push('$DECK_ACTIVE$', '$DECK_ACTIVE_N$');
            else                          tokens.push('$DECK_ALT$',    '$DECK_ALT_N$');
        } else {
            if (which === this.activeFx)  tokens.push('$FX_ACTIVE$', '$FX_ACTIVE_N$');
            else                          tokens.push('$FX_ALT$',    '$FX_ALT_N$');
        }

        this._afterFocusChanged(tokens, 'focus-' + kind);
    };

    XoneK3.K3Controller.prototype.setActiveDeck = function(slot) {
        if (slot !== 'A' && slot !== 'B') return;
        if (this.activeDeck === slot) return;
        this.activeDeck = slot;
        this._afterFocusChanged(
            ['$DECK_ACTIVE$', '$DECK_ACTIVE_N$', '$DECK_ALT$', '$DECK_ALT_N$'],
            'focus-deck');
    };

    XoneK3.K3Controller.prototype.setActiveFx = function(slot) {
        if (slot !== 'A' && slot !== 'B') return;
        if (this.activeFx === slot) return;
        this.activeFx = slot;
        this._afterFocusChanged(
            ['$FX_ACTIVE$', '$FX_ACTIVE_N$', '$FX_ALT$', '$FX_ALT_N$'],
            'focus-fx');
    };

    XoneK3.K3Controller.prototype._refreshActiveAlt = function() {
        var isDeckB = (this.activeDeck === 'B');
        var isFxB   = (this.activeFx   === 'B');
        this.selectedDeckCurrent = isDeckB ? this.selectedDeckB : this.selectedDeckA;
        this.selectedDeckAlt     = isDeckB ? this.selectedDeckA : this.selectedDeckB;
        this.selectedFxCurrent   = isFxB   ? this.selectedFxB   : this.selectedFxA;
        this.selectedFxAlt       = isFxB   ? this.selectedFxA   : this.selectedFxB;
        this._refreshDisplayFlags();
    };

    XoneK3.K3Controller.prototype._reapplyByTokens = function(tokens, opts) {
        var fingerprint = null;

        for (var name in this.bindings) {
            var b = this.bindings[name];
            if (!b._usedTokens || !b._rawCfg) continue;
            var hit = false;
            for (var i = 0; i < tokens.length; i++) {
                if (b._usedTokens.indexOf(tokens[i]) !== -1) { hit = true; break; }
            }
            if (!hit) continue;
            if (fingerprint === null) fingerprint = this._tokenFingerprint();
            var h = this._resolvedTargetHash(b._rawCfg, fingerprint);
            if (b._resolvedTargetHash === h) continue;
            this._applyOneSafe(name, b._rawCfg, true, fingerprint);
        }
        if (!(opts && opts.skipRefresh)) {
            this._refreshAllLeds('token-reapply');
        }
    };

    // Return-value convention:
    //   undefined   — name isn't an fx-pattern (not mine, skip)
    //   null        — fx-pattern but target patch is missing
    //   string      — the swap target
    XoneK3.K3Controller.prototype._resolveFxPatchName = function(name) {
        var m = /^fx(Layout[AB]?|Mini)(\d)$/.exec(name);
        if (m) {
            var kind = m[1];
            var want;
            if      (kind === 'LayoutA') want = PN.TWODECK_A_LAYOUT + this.selectedFxA;
            else if (kind === 'LayoutB') want = PN.TWODECK_B_LAYOUT + this.selectedFxB;
            else if (kind === 'Mini')    want = PN.MINI_LAYOUT    + this.selectedFxCurrent;
            else                         want = PN.FOCUS_LAYOUT   + this.selectedFxCurrent;
            return this.patches[want] ? want : null;
        }
        return undefined;
    };

    XoneK3.K3Controller.prototype._syncFxLayout = function() {
        var swaps = [];
        var sawMissing = false;
        var missingNames = [];
        for (var i = 0; i < this.activePatches.length; i++) {
            var from = this.activePatches[i];
            var want = this._resolveFxPatchName(from);
            if (want === undefined) continue;
            if (want === null) {
                sawMissing = true;
                if (missingNames.indexOf(from) === -1) missingNames.push(from);
                continue;
            }
            if (want === from) continue;
            swaps.push({ from: from, to: want });
        }
        if (sawMissing) {
            XoneK3._warnOnce('fxLayoutMissing:' + missingNames.join(','),
                '_syncFxLayout: fx-pattern patch ' + missingNames.join(',') +
                ' has no matching target — check the patches: table');
        }
        if (swaps.length === 0) {
            return { swapped: false, reason: sawMissing ? 'missing' : 'noop' };
        }
        this._batchPatches(function(patches) {
            for (var s = 0; s < swaps.length; s++) {
                var idx = patches.indexOf(swaps[s].from);
                if (idx !== -1) patches[idx] = swaps[s].to;
            }
        });
        return { swapped: true, reason: 'swapped' };
    };

    XoneK3.K3Controller.prototype._isPress = function(v, s) { return v > 0; };
})();