// xone-k3-lib-controller-render.js — LED writes, sysex queue,
// calibration math, ramp sampling, beatjump gate.
//
// Loaded after lib-controller-beat.js. Extends K3Controller.prototype.

'use strict';

if (typeof XoneK3 === 'undefined') {
    throw new Error('Xone K3: xone-k3-lib-controller-render.js — xone-k3-lib-core.js must load first');
}
if (typeof XoneK3._requireLoad !== 'function') {
    throw new Error('Xone K3: xone-k3-lib-controller-render.js — xone-k3-lib-core.js must load first');
}
XoneK3._requireLoad('xone-k3-lib-controller-render.js',
    ['K3Controller', 'buildSysExLED', 'sendK2Note',
     'sampleT', 'sampleSegs', 'applyBeat', 'BEAT_HOLD', 'scaleTarget']);

(function() {
    // Per-boot counter for segments-array ids. Cleared in _bootWith.
    XoneK3._segsCacheCounter = 0;

    // ============================================================
    // LED WRITE + CALIBRATION
    // ============================================================
    // Shared 3-element scratch for _wbRGB / _channelEmit. Every
    // caller unpacks indices immediately and discards the array;
    // none holds it past the current call. No re-entrancy between
    // these two functions.
    var _WB_SCRATCH = [0, 0, 0];

    XoneK3.K3Controller.prototype._wbRGB = function(r, g, b) {
        if (!this.wbActive) {
            _WB_SCRATCH[0] = r; _WB_SCRATCH[1] = g; _WB_SCRATCH[2] = b;
            return _WB_SCRATCH;
        }
        var wb = this.wbGain;
        if (!wb || wb.length < 3) {
            _WB_SCRATCH[0] = r; _WB_SCRATCH[1] = g; _WB_SCRATCH[2] = b;
            return _WB_SCRATCH;
        }
        _WB_SCRATCH[0] = Math.max(0, Math.min(127, Math.round(r * wb[0])));
        _WB_SCRATCH[1] = Math.max(0, Math.min(127, Math.round(g * wb[1])));
        _WB_SCRATCH[2] = Math.max(0, Math.min(127, Math.round(b * wb[2])));
        return _WB_SCRATCH;
    };

    // Per-index dedupe + throttle queue for RGB sysex frames.
    // Cache key is a packed 28-bit int (r<<21 | g<<14 | b<<7 | hw) —
    // same equality as the old 'r:g:b:hw' string, no per-render
    // allocation. Decode via XoneK3._decodeLedCacheValue when debugging.
    XoneK3.K3Controller.prototype._sendSysExLED = function(idx, r, g, b, hw) {
        var val = (((r & 0x7F) << 21) | ((g & 0x7F) << 14) |
                   ((b & 0x7F) << 7) | (hw & 0x7F)) >>> 0;
        if (this._ledCache[idx] === val) return;
        this._ledCache[idx] = val;

        var msg = XoneK3.buildSysExLED(idx, r, g, b, hw);
        if (XoneK3.SYSEX_THROTTLE_MS <= 0) { midi.sendSysexMsg(msg, msg.length); return; }

        this._enqueueSysEx(idx, msg);
        this._startSysExDrain();
    };

    // Reverse the packed-int encoding used by _sendSysExLED, for
    // inspect('why', name) output. Pass-through for anything that
    // isn't a number (defensive — nothing writes strings any more).
    XoneK3._decodeLedCacheValue = function(v) {
        if (typeof v !== 'number') return String(v);
        var r  = (v >>> 21) & 0x7F;
        var g  = (v >>> 14) & 0x7F;
        var b  = (v >>> 7)  & 0x7F;
        var hw =  v          & 0x7F;
        return r + ':' + g + ':' + b + ':' + hw;
    };

    // Replace-or-append in the queue, O(1). _sysexQueue holds
    // idxes in drain order; _sysexQueueMap holds idx -> msg for
    // the same entries. When the same idx is already queued,
    // update its msg in place (order preserved, fresher value).
    // Cap at 128 — an overrun drops the oldest entry, invalidates
    // its _ledCache slot (the frame was never sent; without the
    // invalidation, a future render with the same frame would be
    // a cache hit and the LED would never receive that value),
    // and removes it from the map.
    XoneK3.K3Controller.prototype._enqueueSysEx = function(idx, msg) {
        if (this._sysexQueueMap[idx] !== undefined) {
            this._sysexQueueMap[idx] = msg;
            return;
        }
        this._sysexQueue.push(idx);
        this._sysexQueueMap[idx] = msg;
        if (this._sysexQueue.length > 128) {
            var dropped = this._sysexQueue.shift();
            delete this._sysexQueueMap[dropped];
            delete this._ledCache[dropped];
        }
    };

    XoneK3.K3Controller.prototype._startSysExDrain = function() {
        if (this._sysexTimer) return;
        var self = this;
        this._sysexTimer = engine.beginTimer(
            Math.max(1, XoneK3.SYSEX_THROTTLE_MS),
            function() { self._sysExDrainTick(); },
            false);
    };

    // One frame per tick. Stops the timer (and nulls it) once the
    // queue empties. No _shuttingDown bail — the drain runs to
    // completion on purpose; shutdown polls the queue length and
    // relies on this.
    XoneK3.K3Controller.prototype._sysExDrainTick = function() {
        if (this._sysexQueue.length === 0) {
            this._stopSysExDrain();
            return;
        }
        var idx = this._sysexQueue.shift();
        var msg = this._sysexQueueMap[idx];
        delete this._sysexQueueMap[idx];
        if (msg) midi.sendSysexMsg(msg, msg.length);
        if (this._sysexQueue.length === 0) {
            this._stopSysExDrain();
        }
    };

    XoneK3.K3Controller.prototype._stopSysExDrain = function() {
        if (this._sysexTimer) {
            engine.stopTimer(this._sysexTimer);
            this._sysexTimer = null;
        }
    };

    XoneK3.K3Controller.prototype.sendLED = function(idx, r, g, b, hw, k2Override) {
        if (this._shuttingDown) return;
        if (idx < 0) return;
        var useK2 = (typeof k2Override === 'boolean') ? k2Override : !this.sysex;
        if (useK2) { XoneK3.sendK2Note(this.midiChannel, idx, r, g, b, hw); return; }
        if (this.calibrationMode !== 'off' && this.wbActive) {
            var wbRgb = this._wbRGB(r, g, b);
            r = wbRgb[0]; g = wbRgb[1]; b = wbRgb[2];
        }
        this._sendSysExLED(idx, r, g, b, hw);
    };

    // Short messages inline; sysex goes through the queue so
    // shutdown can wait for the last blank frame. Both sweeps run
    // unconditionally — a previous session may have left K2 notes
    // lit, and sysex blanks can't reach them.
    XoneK3.K3Controller.prototype._blankLeds = function() {
        this._stopSysExDrain();
        this._sysexQueue = [];
        this._sysexQueueMap = {};
        this._ledCache = {};

        this._blankShortMsgSweep();

        if (this.sysex !== false) {
            this._blankSysexSweep();
        }
    };

    XoneK3.K3Controller.prototype._blankShortMsgSweep = function() {
        for (var n = 0x0C; n <= 0x37; n++) {
            midi.sendShortMsg(0x90 + this.midiChannel, n, 0x00);
        }
    };

    XoneK3.K3Controller.prototype._blankSysexSweep = function() {
        for (var idx = 0; idx <= 0x21; idx++) {
            if (this.k2Indices && this.k2Indices[idx]) continue;
            this._sendSysExLED(idx, 0, 0, 0, 0);
        }
    };

    // Pulse a CO high then low. Duration is PULSE_CO_MS. Used by
    // beatjump / reloop / beatloop-activate to fire momentary
    // COs.
    XoneK3.K3Controller.prototype._pulseCo = function(group, co) {
        engine.setValue(group, co, 1);
        engine.beginTimer(XoneK3.PULSE_CO_MS, function() {
            engine.setValue(group, co, 0);
        }, true);
    };

    // ============================================================
    // BEATJUMP GATE
    // ============================================================
    // A beatjump is gated on its channel's loop being active.
    // Outside a loop: always allowed, no stamp. Inside a loop:
    // blocked when less than BEATJUMP_GAP_MS has elapsed since
    // the last loop-touching event on the same channel; on a
    // pass, records the beatjump as the new "last touch" so two
    // rapid in-loop beatjumps also gate each other.
    //
    // `loopOn` is an optional pass-through: encBeatjump already
    // read the CO for its loop-only gate, so it passes the value
    // here to skip a second `engine.getValue` (CO mutex) on the
    // encoder spin path. beatjumpButton omits it and the gate
    // reads fresh.
    XoneK3.K3Controller.prototype._beatjumpAllowed = function(group, loopOn) {
        var ch = this._channelOf(group);
        if (!ch) return true;
        if (loopOn === undefined) {
            loopOn = engine.getValue('[Channel' + ch + ']', 'loop_enabled') > 0;
        }
        var now = Date.now();
        if (loopOn) {
            var last = this._lastLoopTouchMs[ch];
            if (last && now - last < XoneK3.BEATJUMP_GAP_MS) {
                if (XoneK3._debugPrintsOn) {
                    print('[k3] beatjump gated: ' + (now - last) + 'ms / ' +
                          XoneK3.BEATJUMP_GAP_MS + 'ms');
                }
                return false;
            }
            this._lastLoopTouchMs[ch] = now;
        }
        return true;
    };

    // Record a loop-touching event on this group's channel.
    // Beatjump will gate against this timestamp when the channel
    // is in an active loop.
    XoneK3.K3Controller.prototype._touchLoop = function(group) {
        var ch = this._channelOf(group);
        if (ch) this._lastLoopTouchMs[ch] = Date.now();
    };

    // ============================================================
    // CALIBRATION MATH
    // ============================================================
    function _saturation(rgb) {
        var mx = Math.max(rgb[0], rgb[1], rgb[2]);
        if (mx <= 0) return 0;
        var mn = Math.min(rgb[0], rgb[1], rgb[2]);
        return (mx - mn) / mx;
    }

    // Monotonic [hw, lum] lookup. table is sorted by hw and has
    // no guaranteed spacing; the old code scanned linearly.
    // Binary search matches the previous first-match-wins semantics
    // exactly, including duplicate hw entries (first match wins,
    // via lower-bound on the [1..last] sub-slice — the original
    // loop's `hw <= table[i+1][0]` predicate).
    function _sampleTransfer(table, hw) {
        if (hw <= table[0][0]) return table[0][1];
        var last = table.length - 1;
        if (hw >= table[last][0]) return table[last][1];

        // Find first j in [1, last] with table[j][0] >= hw. That's
        // the j the original `for i; hw <= table[i+1][0]` loop
        // landed on. i = j - 1 is the interval's left endpoint.
        var lo = 1, hi = last;
        while (lo < hi) {
            var mid = (lo + hi) >> 1;
            if (table[mid][0] >= hw) hi = mid;
            else lo = mid + 1;
        }
        var j = lo;
        var i = j - 1;
        var span = table[j][0] - table[i][0];
        if (span <= 0) return table[i][1];
        var t = (hw - table[i][0]) / span;
        return table[i][1] * (1 - t) + table[j][1] * t;
    }

    var _EMIT_SCRATCH = [0, 0, 0];

    XoneK3.K3Controller.prototype._channelEmit = function(rgb, hw) {
        var wbRgb = this._wbRGB(rgb[0], rgb[1], rgb[2]);
        var t = this._transfer, w = this.channelPerceptual;
        if (!t) {
            var lv = hw / 127;
            _EMIT_SCRATCH[0] = (wbRgb[0] / 127) * lv * w[0];
            _EMIT_SCRATCH[1] = (wbRgb[1] / 127) * lv * w[1];
            _EMIT_SCRATCH[2] = (wbRgb[2] / 127) * lv * w[2];
            return _EMIT_SCRATCH;
        }
        _EMIT_SCRATCH[0] = (wbRgb[0] / 127) * _sampleTransfer(t.r, hw) * w[0];
        _EMIT_SCRATCH[1] = (wbRgb[1] / 127) * _sampleTransfer(t.g, hw) * w[1];
        _EMIT_SCRATCH[2] = (wbRgb[2] / 127) * _sampleTransfer(t.b, hw) * w[2];
        return _EMIT_SCRATCH;
    };

    XoneK3.K3Controller.prototype._emitAt = function(rgb, hw) {
        var c = this._channelEmit(rgb, hw);
        var Y = c[0] + c[1] + c[2];
        if (Y <= 0) return 0;
        var P = (this.lumGamma === 1.0) ? Y : Math.pow(Y, this.lumGamma);
        return P * (1 + this.hkBoost * _saturation(rgb));
    };

    XoneK3.K3Controller.prototype._solveHw = function(rgb, hw, ramp, compiled) {
        if (!compiled.calibration) return hw;
        if (this.calibrationMode === 'off') return hw;
        if (rgb[0] === 0 && rgb[1] === 0 && rgb[2] === 0) return hw;
        var target = this._flatTarget(ramp);
        if (target === null) return hw;
        target = XoneK3.scaleTarget(target, compiled.brightnessScale);
        return this._calibratedHw(rgb, target);
    };

    XoneK3.K3Controller.prototype._calibFingerprint = function() {
        return [
            this._calibCurveName,
            this.calibrationMode,
            this.wbActive ? 'on' : 'off',
            this.wbGain.join(','),
            this.channelPerceptual.join(','),
            this.hkBoost,
            this.lumGamma
        ].join('|');
    };

    XoneK3.K3Controller.prototype._flatTarget = function(segs) {
        if (!segs || segs.length === 0) return null;
        if (this.calibrationMode === 'off') return null;

        // Segments-array cache ids are boot-scoped. The arrays
        // themselves live in XoneK3._compiledCache, reset per boot.
        if (segs._cacheId === undefined) {
            segs._cacheId = ++XoneK3._segsCacheCounter;
        }
        var cache = this._flatCache;
        var entry = cache[segs._cacheId];
        var fingerprint = this._calibFingerprint();
        if (entry && entry.key === fingerprint) return entry.value;

        var value = this._minSegmentEmit(segs);
        if (this._flatCacheSize >= 1024) {
            // Evict one entry instead of clearing the cache.
            // Clear-all triggered a 1024-entry _minSegmentEmit
            // recompute burst on the next render. One-entry
            // eviction bounds the recompute to a single miss.
            // _cacheId keys are integer-like, so V8 yields them
            // in ascending order — this evicts the oldest entry.
            for (var ek in cache) { delete cache[ek]; break; }
            this._flatCacheSize--;
        }
        cache = this._flatCache;
        cache[segs._cacheId] = { key: fingerprint, value: value };
        this._flatCacheSize++;
        return value;
    };

    // The flat-target solve's minimum-luminance scan. Skips
    // black stops (0,0,0) — they can't be the flat target's
    // reference point.
    //
    // Null-rgb (mixxx) stops are skipped too — they have no
    // emission to measure. The caller (_flatTarget) is only
    // reached from _solveHw for full schemes (calibration: true);
    // _compileSingleColor sets calibration: false, so a single-
    // colour mixxx never hits this path, but a mixed ramp can.
    XoneK3.K3Controller.prototype._minSegmentEmit = function(segs) {
        var minEmit = Infinity;
        for (var i = 0; i < segs.length; i++) {
            var c = segs[i].color;
            if (!c || !c.rgb) continue;
            if (c.rgb[0] === 0 && c.rgb[1] === 0 && c.rgb[2] === 0) continue;
            var e = this._emitAt(c.rgb, c.brightness);
            if (e < minEmit) minEmit = e;
        }
        return isFinite(minEmit) ? minEmit : null;
    };

    XoneK3.K3Controller.prototype._calibratedHw = function(rgb, targetLum) {
        if (targetLum <= 0) return 0;
        var key = rgb[0] + ':' + rgb[1] + ':' + rgb[2] + ':' + targetLum.toFixed(6);
        var cached = this._hwCache[key];
        if (cached !== undefined) return cached;

        var value = this._bisectHw(rgb, targetLum);

        // Evict one entry instead of clearing the whole cache.
        // Clear-all triggered a 1024-entry recompute burst on the
        // next render — 1024 × 12 bisects, enough to drop a frame.
        // One-entry eviction bounds the recompute to a single miss.
        // Key identity doesn't matter — any single eviction keeps
        // the size bound. Cache key is unchanged; solved values are
        // identical to before.
        if (this._hwCacheSize >= 1024) {
            for (var ek in this._hwCache) { delete this._hwCache[ek]; break; }
            this._hwCacheSize--;
        }
        this._hwCache[key] = value;
        this._hwCacheSize++;
        return value;
    };

    // Bisect hw over [0, 127] against targetLum. 12 iterations
    // resolves the interval to < 0.1 — one bit finer than the
    // 7-bit hw space, so the returned integer is stable under
    // small luminance perturbations.
    XoneK3.K3Controller.prototype._bisectHw = function(rgb, targetLum) {
        var lo = 0, hi = 127;
        for (var i = 0; i < 12; i++) {
            var mid = (lo + hi) / 2;
            if (this._emitAt(rgb, mid) < targetLum) lo = mid;
            else hi = mid;
        }
        return Math.round((lo + hi) / 2);
    };

    XoneK3.K3Controller.prototype._sampleRamp = function(ramp, t, compiled, isOn, skipBeat) {
        var tw = XoneK3.sampleT(compiled, t);
        var entry = XoneK3.sampleSegs(ramp, tw, compiled.interp);
        if (!entry) return null;
        if (isOn) {
            entry.brightness = this._solveHw(entry.rgb, entry.brightness, ramp, compiled);
        }
        if (!skipBeat && isOn && compiled.beat) {
            var st = this._deckStateFor(compiled);
            if (st && st.hasBeat) {
                var beat = XoneK3.applyBeat(entry, compiled.beat, st.count);
                if (beat === XoneK3.BEAT_HOLD) return XoneK3.BEAT_HOLD;
                entry = beat;
            }
        }
        return entry;
    };
})();