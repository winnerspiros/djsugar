// xone-k3-lib-core.js — bootstrap, tunables, patch names, timeline
// config, utilities, tokens, transfer curve, sysex math, beat
// math, K2 palette.
//
// Leaf module. Loads first. Defines XoneK3._requireLoad.

'use strict';

// Bootstrap. Mixxx's Qt engine evaluates every preset's script
// files into the same global object, so a top-level `var` here
// becomes a shared global for subsequent files. Only file that
// creates XoneK3.
// eslint-disable-next-line no-implicit-globals
var XoneK3 = (typeof XoneK3 !== 'undefined') ? XoneK3 : {};

(function() {
    XoneK3._requireLoad = function(file, prereqs) {
        var missing = [];
        for (var i = 0; i < prereqs.length; i++) {
            if (XoneK3[prereqs[i]] === undefined) missing.push(prereqs[i]);
        }
        if (missing.length > 0) {
            throw new Error('Xone K3: ' + file + ' requires ' +
                missing.join(', ') +
                ' — check the <scriptfiles> order in the preset .midi.xml');
        }
    };

    // ============================================================
    // TUNABLES
    // ============================================================
    XoneK3.SYSEX_THROTTLE_MS = 10;
    XoneK3.PULSE_CO_MS = 30;

    XoneK3.DEFAULT_RGB_BRIGHTNESS     = 110;
    XoneK3.DEFAULT_RGB_BRIGHTNESS_OFF = 12;

    XoneK3.CO_MATCH_EPSILON = 1 / 127;

    // Soft-takeover bridge tolerance, strictly larger than
    // CO_MATCH_EPSILON: LED "matched" always implies control is
    // live. The extra half-band is a harmless false-negative.
    XoneK3.TAKEOVER_EPSILON = 2 / 127;

    XoneK3.DEFAULT_CO_SCHEME = 'coMatch';

    XoneK3.K2_RG_THRESHOLD = 60;
    XoneK3.CURVE_GAIN = 4.0;
    XoneK3.RATIO_GAIN = 12.0;

    // Calibration for the K3. Presets that want different
    // values declare an override in init(); presets that don't
    // inherit these via bootCat's merge.
    XoneK3.HARDWARE_CALIB = {
        calibration:       'measuredRGB',
        calibrationMode:   'rms',
        wbGain:            [0.6, 1.0, 0.93],
        channelPerceptual: [1.35, 0.69, 0.19],
        hkBoost:           0.29,
        lumGamma:          0.32
    };

    XoneK3.DEFAULT_BAR_BEATS   = 4;
    XoneK3.DEFAULT_PHRASE_BARS = 4;
    XoneK3.DEFAULT_BEAT_DECK   = '$DECK_ACTIVE$';

    XoneK3.BEATJUMP_MIN = 1/32;
    XoneK3.BEATJUMP_MAX = 16;

    XoneK3.BEAT_REPAINT_THROTTLE_MS = 30;

    // If beat_active has been silent for at least this long, the
    // beat-edge guard in _onPlayposition releases and the pulse
    // phase falls back to a position-derived read. See
    // INTERNALS §10.
    XoneK3.BEAT_STALL_MS = 1500;

    // Milliseconds after the last metadata-relevant CO event
    // (track_loaded, track_samples, duration, file_bpm) during
    // which _trackMeta reads live but skips the cache write.
    // Mixxx doesn't guarantee ordering between these COs; the
    // window spans the last fire regardless of which one it was.
    XoneK3.META_SETTLE_MS = 300;

    // Boot settle. After the initial catch-up at boot, if any
    // boot-relevant CO (track_loaded, play_indicator, loop_enabled,
    // rate_ratio, reverse) fires within BOOT_SETTLE_MS of the last
    // fire, another catch-up is scheduled. Stops the silent-stale
    // bug where a slow CO tree leaves _loadedState[ch] or
    // st.playing wrong until the next unrelated event.
    XoneK3.BOOT_SETTLE_MS = 100;
    XoneK3.BOOT_MAX_ATTEMPTS = 5;

    XoneK3.LAYER_BUTTON_NOTE = 0x0C;
    XoneK3.SHIFT_BUTTON_NOTE = 0x0F;

    // Beatjump gate. A beatjump is blocked if less than this many
    // ms have elapsed since the last loop-touching event on its
    // channel AND the channel is in an active loop. Loop-touching
    // events: beatjumps and loop-boundary changes. 130 ms is the
    // empirically confirmed safe margin (120 worked, 60 leaked).
    XoneK3.BEATJUMP_GAP_MS = 130;

    // What a beat-enabled LED shows while its deck is playing /
    // paused. Same three values for both:
    //   'pulse' — bright first half, dim second (position-derived)
    //   'solid' — on-ramp + cascade, no brightness pulse
    //   'off'   — off-ramp, no cascade
    XoneK3.BEAT_PLAYING_MODE = 'pulse';
    XoneK3.BEAT_PAUSED_MODE  = 'solid';

    // ============================================================
    // PATCH NAMES
    // ============================================================
    XoneK3.PATCH_NAMES = {
        FOCUS_BASE:       'topShared',
        FOCUS_LAYOUT:     'fxLayout',      // + 1..4
        TWODECK_BASE:     'twoDeck',
        TWODECK_A_LAYOUT: 'fxLayoutA',     // + 1..4
        TWODECK_B_LAYOUT: 'fxLayoutB',     // + 1..4
        MINI_BASE:        'twoDeckMini',
        MINI_LAYOUT:      'fxMini',        // + 1..4

        CHORD:            'chord',
        CUE:              'cue',
        LOOP:             'loop',

        SELECT_AB:        'selectAB',
        FX_ROUTING:       'fxRouting'
    };

    // ============================================================
    // TIMELINE
    // ============================================================
    // TIME_SIG drives the beat cascade (bar / phrase boundaries)
    // and the layer/shift track-end countdown. See setTimeSig.
    XoneK3.TIME_SIG = null;

    XoneK3.setTimeSig = function(barBeats, phraseBars) {
        // Floor + reject non-finite. Fractional barBeats produce a
        // fractional phraseLen; the modulo positions in beatLevel
        // then only coincide with integer beat counts at a subset
        // of the expected positions, silently corrupting the
        // cascade. NaN/Infinity propagate the same way.
        barBeats = Math.floor(Number(barBeats));
        phraseBars = Math.floor(Number(phraseBars));
        if (!isFinite(barBeats) || barBeats < 1) return;
        if (!isFinite(phraseBars) || phraseBars < 1) return;
        XoneK3.TIME_SIG = { barBeats: barBeats, phraseBars: phraseBars };
        if (!XoneK3.controllers) return;
        for (var i = 0; i < XoneK3.controllers.length; i++) {
            XoneK3.controllers[i]._refreshAllLeds('timesig');
        }
    };

    XoneK3.clearTimeSig = function() {
        XoneK3.TIME_SIG = null;
        if (!XoneK3.controllers) return;
        for (var i = 0; i < XoneK3.controllers.length; i++) {
            XoneK3.controllers[i]._refreshAllLeds('timesig');
        }
    };

    // ============================================================
    // DEBUG PRINTS
    // ============================================================
    XoneK3.PRINT_TABLES = [];
    XoneK3._debugPrintsOn = false;
    XoneK3._warned = Object.create(null);
    XoneK3.registerPrintTable = function(table, name) {
        XoneK3.PRINT_TABLES.push({ table: table, name: name || 'unnamed' });
    };

    XoneK3._warnOnce = function(key, msg) {
        if (XoneK3._warned[key]) return;
        XoneK3._warned[key] = true;
        print('[k3] ' + msg);
    };

    XoneK3.debugPrint = function(key, vars) {
        if (!XoneK3._debugPrintsOn) return;
        var tpl = null;
        for (var i = 0; i < XoneK3.PRINT_TABLES.length; i++) {
            var t = XoneK3.PRINT_TABLES[i].table;
            if (t[key] !== undefined) { tpl = t[key]; break; }
        }
        if (tpl === null) return;
        if (vars) {
            for (var k in vars) {
                if (!vars.hasOwnProperty(k)) continue;
                tpl = tpl.split('{' + k + '}').join(vars[k]);
            }
        }
        print('[k3] ' + tpl);
    };

    XoneK3.registerPrintTable({
        overlayOn:  'overlay {name} ON',
        overlayOff: 'overlay {name} OFF'
    }, 'lib');

    // ============================================================
    // UTILITIES
    // ============================================================
    XoneK3.parseNum = function(s) {
        var v;
        if (typeof s === 'number') {
            v = s;
        } else if (typeof s !== 'string') {
            return NaN;
        } else {
            s = s.trim();
            if (s === '') return NaN;
            var p = s.split('/');
            if (p.length === 2) {
                var a = parseFloat(p[0]), b = parseFloat(p[1]);
                if (isNaN(a) || isNaN(b) || b === 0) return NaN;
                v = a / b;
            } else if (p.length === 1) {
                v = parseFloat(s);
            } else {
                return NaN;
            }
        }
        // Callers guard with isNaN, which accepts Infinity. A
        // config typo like args: ['1e999'] would write Infinity
        // to a CO (engine.setValue / setParameter paths).
        return isFinite(v) ? v : NaN;
    };

    XoneK3.readPath = function(obj, path) {
        if (obj === undefined || obj === null) return undefined;
        if (typeof path !== 'string' || path.indexOf('.') === -1) return obj[path];
        var parts = path.split('.'), cur = obj;
        for (var i = 0; i < parts.length; i++) {
            if (cur === undefined || cur === null) return undefined;
            cur = cur[parts[i]];
        }
        return cur;
    };

    XoneK3._selProp = function(kind, which) {
        return 'selected' + kind.charAt(0).toUpperCase() + kind.slice(1) + which;
    };

    // ============================================================
    // BINDING FIELD TABLE
    // ============================================================
    // One table describing every field the binding pipeline
    // recognises. Adding a field is one row — _BINDING_CFG_FIELDS,
    // _SNAPSHOT_FIELDS, _SNAPSHOT_IGNORED, _tokenFields and
    // _resolvedTargetHash all derive from this array.
    //
    // Row shape:
    //   name       cfg path (dot-paths allowed, e.g. 'colorScheme.on.group')
    //   hash       include in _hashBinding's field list
    //   token      emit from _tokenFields + resolve in _resolvedTargetHash;
    //              value is the label used in warnings
    //   snapshot   { dest, source?, transform? }
    //              source defaults to name; transform is a name in
    //              XoneK3._snapshotTransform
    //   ignored    reset-able binding field that _unbind /
    //              _rearmSoftTakeover do not read
    XoneK3._BINDING_FIELDS = [
        // ---- cfg fields (hashed) ----
        { name: 'group',        hash: true, token: 'group', snapshot: { dest: 'group' } },
        { name: 'inKey',        hash: true, token: 'inKey', snapshot: { dest: 'inKey' } },
        { name: 'outKey',       hash: true, token: 'outKey', ignored: true },
        { name: 'type',         hash: true, snapshot: { dest: 'wasPot', transform: 'isPot' } },
        { name: 'func',         hash: true },
        { name: 'args',         hash: true },
        { name: 'valueCurve',   hash: true },
        { name: 'softTakeover', hash: true, ignored: true },
        { name: 'noLED',        hash: true, snapshot: { dest: 'wasNoLED', source: '_noLED' } },
        { name: 'k2',           hash: true, snapshot: { dest: 'wasK2',    source: '_k2'    } },
        { name: 'colorScheme',  hash: true },

        // ---- tokenable cfg sub-fields (hashed as part of colorScheme) ----
        { name: 'colorScheme.on.group',              token: 'on.group' },
        { name: 'colorScheme.schemeFromState.group', token: 'schemeFromState.group' },
        { name: 'colorScheme.schemeFromState.co',    token: 'schemeFromState.co' },

        // ---- binding-side fields snapshotted by _snapshotBinding ----
        { name: '_rawCfg', snapshot: { dest: 'wasBound', transform: 'truthy' } },
        { name: 'idx',     snapshot: { dest: 'idx' } },

        // ---- binding-side fields, reset but not snapshotted ----
        { name: 'buttonMode',         ignored: true },
        { name: '_func',              ignored: true },
        { name: '_funcArgs',          ignored: true },
        { name: '_usedTokens',        ignored: true },
        { name: '_csActive',          ignored: true },
        { name: '_csFailed',          ignored: true },
        { name: '_csDeviation',       ignored: true },
        { name: '_csScheme',          ignored: true },
        { name: '_csSchemeFn',        ignored: true },
        { name: '_csSchemeMemo',      ignored: true },
        { name: '_csCompiledMemo',    ignored: true },
        { name: '_csKnob',            ignored: true },
        { name: '_csFromStateConn',   ignored: true },
        { name: '_csOn',              ignored: true },
        { name: '_valueConn',         ignored: true },
        { name: '_conns',             ignored: true },
        { name: '_valueCurve',        ignored: true },
        { name: '_hotcueColorCO',     ignored: true },
        { name: '_cachedHotcueColor', ignored: true },
        { name: '_shutdownHoldTimer', ignored: true }
    ];

    // Derived view: hashed cfg fields, in table order. Consumed by
    // _hashBinding and _auditBindingCfg.
    XoneK3._BINDING_CFG_FIELDS = (function() {
        var out = [];
        for (var i = 0; i < XoneK3._BINDING_FIELDS.length; i++) {
            if (XoneK3._BINDING_FIELDS[i].hash) out.push(XoneK3._BINDING_FIELDS[i].name);
        }
        return out;
    })();

    // Derived view: [source, dest, transform] triples consumed by
    // _snapshotBinding. transform is a name in
    // XoneK3._snapshotTransform, or null for identity.
    XoneK3._SNAPSHOT_FIELDS = (function() {
        var out = [];
        for (var i = 0; i < XoneK3._BINDING_FIELDS.length; i++) {
            var row = XoneK3._BINDING_FIELDS[i];
            if (!row.snapshot) continue;
            var s = row.snapshot;
            out.push([s.source || row.name, s.dest, s.transform || null]);
        }
        return out;
    })();

    // Derived view: reset-able fields _snapshotBinding doesn't
    // capture. Consumed by _auditSnapshotFields.
    XoneK3._SNAPSHOT_IGNORED = (function() {
        var out = [];
        for (var i = 0; i < XoneK3._BINDING_FIELDS.length; i++) {
            if (XoneK3._BINDING_FIELDS[i].ignored) out.push(XoneK3._BINDING_FIELDS[i].name);
        }
        return out;
    })();

    // Stable function identity. Same function → same id;
    // different closures never collide.
    XoneK3._funcIdCounter = 0;
    XoneK3._funcIdMap = new WeakMap();
    XoneK3._funcId = function(fn) {
        if (typeof fn !== 'function') return '';
        var id = XoneK3._funcIdMap.get(fn);
        if (id === undefined) {
            id = '_' + (XoneK3._funcIdCounter++);
            XoneK3._funcIdMap.set(fn, id);
        }
        return id;
    };

    // Recursive helpers, hoisted. Called only at runtime, after
    // XoneK3._stableStringify has been assigned.
    function _stableStringifyArray(arr, seen) {
        var parts = [];
        for (var i = 0; i < arr.length; i++) {
            parts.push(XoneK3._stableStringify(arr[i], seen));
        }
        return '[' + parts.join(',') + ']';
    }

    function _stableStringifyObject(obj, seen) {
        var keys = Object.keys(obj).sort();
        var parts = [];
        for (var k = 0; k < keys.length; k++) {
            var key = keys[k];
            parts.push(JSON.stringify(key) + ':' + XoneK3._stableStringify(obj[key], seen));
        }
        return '{' + parts.join(',') + '}';
    }

    XoneK3._stableStringify = function(o, seen) {
        if (o === null) return 'null';
        if (o === undefined) return 'undefined';
        var t = typeof o;
        if (t === 'number' || t === 'boolean') return String(o);
        if (t === 'string') return JSON.stringify(o);
        if (t === 'function') return 'fn:' + XoneK3._funcId(o);
        if (t === 'object') {
            seen = seen || new Set();
            if (seen.has(o)) {
                XoneK3._warnOnce('stableStringifyCycle',
                    '_stableStringify: cycle detected in binding cfg — ' +
                    'hashing truncated at the cycle. Fix the cfg reference.');
                return '"[cycle]"';
            }
            seen.add(o);
            try {
                if (Array.isArray(o)) return _stableStringifyArray(o, seen);
                return _stableStringifyObject(o, seen);
            } finally {
                seen.delete(o);
            }
        }
        return String(o);
    };

    XoneK3._hashBindingValue = function(field, v) {
        if (v === undefined) return '';
        if (field === 'func') return XoneK3._funcId(v);
        if (typeof v === 'object' && v !== null) return XoneK3._stableStringify(v);
        // JSON-quote strings so a '\u0001' inside a CO name can't
        // shift the field boundary in _hashBinding's join. JSON's
        // encoder escapes \u0001 to "\u0001", so the literal
        // separator byte never appears in the quoted output.
        if (typeof v === 'string') return JSON.stringify(v);
        return String(v);
    };

    XoneK3._hashBinding = function(cfg) {
        if (cfg === null || cfg === undefined) return null;
        var parts = [];
        for (var i = 0; i < XoneK3._BINDING_CFG_FIELDS.length; i++) {
            var f = XoneK3._BINDING_CFG_FIELDS[i];
            parts.push(XoneK3._hashBindingValue(f, cfg[f]));
        }
        // \u0001 can't appear in CO names or numeric output;
        // JSON-quoted strings escape it.
        return parts.join('\u0001');
    };

    XoneK3._auditBindingCfg = function(cfg, name) {
        if (!XoneK3._debugPrintsOn) return;
        for (var k in cfg) {
            if (!cfg.hasOwnProperty(k) || k.charAt(0) === '_') continue;
            if (XoneK3._BINDING_CFG_FIELDS.indexOf(k) === -1) {
                print('[k3] binding ' + name + ' has cfg key "' + k +
                    '" not in _BINDING_CFG_FIELDS — hash will miss it');
            }
        }
        if (cfg.accel !== undefined) {
            print('[k3] binding ' + name +
                ' declares accel: — removed from the runtime. ' +
                'Use a self-accelerating func.');
        }
    };

    // ============================================================
    // SELECT TOKENS
    // ============================================================
    // Resolved at bind time in group, inKey, outKey,
    // schemeFromState.group, schemeFromState.co, on.group.
    XoneK3.TOKENS = (function() {
        function channel(n) { return '[Channel' + n + ']'; }
        function unit(n)    { return '[EffectRack1_EffectUnit' + n + ']'; }
        function bare(n)    { return '' + n; }
        return [
            { token: '$DECK_A$',        state: 'selectedDeckA',     make: channel },
            { token: '$DECK_B$',        state: 'selectedDeckB',     make: channel },
            { token: '$DECK_ACTIVE$',   state: 'selectedDeckCurrent', make: channel },
            { token: '$DECK_ACTIVE_N$', state: 'selectedDeckCurrent', make: bare },
            { token: '$DECK_ALT$',      state: 'selectedDeckAlt',   make: channel },
            { token: '$DECK_ALT_N$',    state: 'selectedDeckAlt',   make: bare },
            { token: '$DECK_A_N$',      state: 'selectedDeckA',     make: bare },
            { token: '$DECK_B_N$',      state: 'selectedDeckB',     make: bare },

            { token: '$FX_A$',          state: 'selectedFxA',       make: unit },
            { token: '$FX_B$',          state: 'selectedFxB',       make: unit },
            { token: '$FX_ACTIVE$',     state: 'selectedFxCurrent', make: unit },
            { token: '$FX_ACTIVE_N$',   state: 'selectedFxCurrent', make: bare },
            { token: '$FX_ALT$',        state: 'selectedFxAlt',     make: unit },
            { token: '$FX_ALT_N$',      state: 'selectedFxAlt',     make: bare },
            { token: '$FX_A_N$',        state: 'selectedFxA',       make: bare },
            { token: '$FX_B_N$',        state: 'selectedFxB',       make: bare }
        ];
    })();

    // Token membership, built once. TOKENS is immutable after the
    // assignment above. _findUnknownTokens is called per-binding
    // from _validateBindings, so O(1) lookup beats scanning.
    var _KNOWN_TOKEN_SET = {};
    (function() {
        for (var i = 0; i < XoneK3.TOKENS.length; i++) {
            _KNOWN_TOKEN_SET[XoneK3.TOKENS[i].token] = true;
        }
    })();

    XoneK3.resolve = function(str, ctrl) {
        if (typeof str !== 'string') return str;
        if (str.indexOf('$') === -1) return str;
        for (var i = 0; i < XoneK3.TOKENS.length; i++) {
            var t = XoneK3.TOKENS[i], v = ctrl[t.state];
            if (v === undefined) continue;
            if (str.indexOf(t.token) === -1) continue;
            str = str.split(t.token).join(t.make(v, ctrl));
        }
        return str;
    };

    var _TOKEN_RE = /\$[A-Z0-9_]+\$/g;
    XoneK3._findUnknownTokens = function(str) {
        if (typeof str !== 'string' || str.indexOf('$') === -1) return [];
        var out = [];
        var seen = {};
        _TOKEN_RE.lastIndex = 0;
        var m;
        while ((m = _TOKEN_RE.exec(str)) !== null) {
            var tok = m[0];
            if (_KNOWN_TOKEN_SET[tok]) continue;
            if (seen[tok]) continue;
            seen[tok] = true;
            out.push(tok);
        }
        return out;
    };

    // Tokenable fields, in _BINDING_FIELDS order. Consumed by
    // _collectTokens (populates b._usedTokens) and _validateBindings
    // (unknown-token warnings).
    XoneK3._tokenFields = function(cfg) {
        var out = [];
        for (var i = 0; i < XoneK3._BINDING_FIELDS.length; i++) {
            var row = XoneK3._BINDING_FIELDS[i];
            if (!row.token) continue;
            var v = XoneK3.readPath(cfg, row.name);
            if (v === undefined) continue;
            var label = (typeof row.token === 'string') ? row.token : row.name;
            out.push([label, v]);
        }
        return out;
    };

    // ============================================================
    // LED TRANSFER CURVE
    // ============================================================
    // Monotonic [hw, luminance] table per channel. Non-monotonic
    // input is flattened to the previous value with a warn-once.
    // Normalised to [0..1] against the channel's own peak.
    function _normalizeTransferChannel(list, keyName) {
        var sorted = list.slice().sort(function(a, b) { return a[0] - b[0]; });
        var maxLum = 0;
        for (var i = 0; i < sorted.length; i++) {
            if (sorted[i][1] > maxLum) maxLum = sorted[i][1];
        }
        if (maxLum <= 0) return null;
        var cur = [], prev = -Infinity;
        for (var j = 0; j < sorted.length; j++) {
            var hw = Math.max(0, Math.min(127, sorted[j][0]));
            var lum = sorted[j][1] / maxLum;
            if (lum < prev) {
                XoneK3._warnOnce('transferNonMono:' + keyName + ':' + hw,
                    'transfer curve channel "' + keyName + '" not monotonic at hw=' + hw +
                    ' — flattening to previous value');
                lum = prev;
            }
            if (lum > 1) lum = 1;
            cur.push([hw, lum]);
            prev = lum;
        }
        return cur;
    }

    XoneK3._buildTransferRGB = function(obj) {
        if (!obj || !obj.r || !obj.g || !obj.b) return null;
        var out = {};
        var keys = ['r', 'g', 'b'];
        for (var k = 0; k < 3; k++) {
            var normalized = _normalizeTransferChannel(obj[keys[k]], keys[k]);
            if (!normalized) return null;
            out[keys[k]] = normalized;
        }
        return out;
    };

    // ============================================================
    // SYSEX
    // ============================================================
    function _clamp7f(x) { return Math.max(0, Math.min(0x7F, x | 0)); }

    XoneK3.noteToLEDIndex = function(n) {
        if (n >= 0x28 && n <= 0x33) { var i = n - 0x28, r = 2 - (i >> 2); return r * 4 + (i & 3); }
        if (n >= 0x18 && n <= 0x27) { var j = n - 0x18, rf = j >> 2; return 0x0C + (3 - rf) * 4 + (j & 3); }
        if (n === 0x0C) return 0x1C;
        if (n === 0x0F) return 0x1D;
        if (n >= 0x34 && n <= 0x37) return n - 0x34 + 0x1E;
        return -1;
    };

    XoneK3.buildSysExLED = function(idx, r, g, b, hw) {
        return [ 0xF0,0x00,0x00,0x1A,0x50,0x15,0x00,0x0E,
                 idx & 0x7F,
                 _clamp7f(r), _clamp7f(g), _clamp7f(b), _clamp7f(hw),
                 0xF7 ];
    };

    // ============================================================
    // BEAT MATH
    // ============================================================
    XoneK3.BEAT_HOLD = {};

    // Cascade chain per level: most-specific entry first, falling
    // through to the next field until a colour / brightness is
    // found. `null` means "the base beat block".
    var _BEAT_CHAIN = {
        prePhrase: ['prePhrase', 'phrase', 'bar', null],
        phrase:    ['phrase', 'bar', null],
        bar:       ['bar', null],
        beat:      [null]
    };

    function _colorHasBrightness(spec) {
        if (typeof spec === 'string') return spec.split(',').length >= 2;
        if (Array.isArray(spec)) return spec.length >= 4;
        return false;
    }

    // Resolve (barBeats, phraseBars) from TIME_SIG if set, else
    // from the beat block's own fields, else from the engine
    // defaults. Extracted so beatLevel reads as music, not as
    // defaulting rules.
    function _resolveSig(beat) {
        var sig = XoneK3.TIME_SIG;
        if (sig) {
            var bb = sig.barBeats;
            var pb = sig.phraseBars;
            var validBB = (typeof bb === 'number' && isFinite(bb) &&
                           bb >= 1 && bb === Math.floor(bb));
            var validPB = (typeof pb === 'number' && isFinite(pb) &&
                           pb >= 1 && pb === Math.floor(pb));
            if (validBB && validPB) return { barBeats: bb, phraseBars: pb };
            // Direct assignment bypasses setTimeSig's coercion;
            // warn-once and fall through to scheme defaults.
            XoneK3._warnOnce('timeSigInvalid',
                'XoneK3.TIME_SIG contains invalid barBeats/phraseBars — ' +
                'falling back to scheme defaults. Use setTimeSig() or ' +
                'provide integers >= 1.');
        }
        return {
            barBeats:   (typeof beat.barBeats   === 'number' && beat.barBeats   > 0)
                        ? beat.barBeats   : XoneK3.DEFAULT_BAR_BEATS,
            phraseBars: (typeof beat.phraseBars === 'number' && beat.phraseBars > 0)
                        ? beat.phraseBars : XoneK3.DEFAULT_PHRASE_BARS
        };
    }

    XoneK3.beatLevel = function(beat, count) {
        var sig = _resolveSig(beat);
        var barBeats = sig.barBeats;
        var phraseBars = sig.phraseBars;
        var phraseLen = barBeats * phraseBars;

        var preBeats;
        if (typeof beat.prePhraseBeats === 'number') preBeats = beat.prePhraseBeats;
        else preBeats = barBeats;
        if (preBeats < 0) preBeats = 0;
        if (preBeats >= phraseLen) preBeats = phraseLen - 1;
        var prePhraseStart = phraseLen - preBeats;

        // Modular position within the phrase, 0-based. Negative
        // counts (pre-marker playback) are handled by the
        // double-mod.
        var phrasePos = ((count - 1) % phraseLen + phraseLen) % phraseLen;
        var level;
        // `prePhraseBeats` marks the beat where the pre-phrase colour
        // starts, not a duration. With prePhraseBeats: 4 (and phraseLen
        // 16), only position 12 gets the prePhrase colour; 13–15 fall
        // through to bar/beat. If you want a zone, extend _BEAT_CHAIN
        // and this condition together.
        if (preBeats > 0 && phrasePos === prePhraseStart) level = 'prePhrase';
        else if (phrasePos === 0)                         level = 'phrase';
        else if (phrasePos % barBeats === 0)              level = 'bar';
        else                                              level = 'beat';

        if (beat.half && level === 'beat' && (((count % 2) + 2) % 2 === 1)) {
            return { skip: true };
        }

        var chain = _BEAT_CHAIN[level] || [null];
        var out = { level: level, color: null, brightness: null };
        for (var i = 0; i < chain.length; i++) {
            var key = chain[i];
            var e = (key === null) ? beat : beat[key];
            if (!e) continue;
            if (out.color === null && e.color !== undefined) out.color = e.color;
            if (out.brightness === null && e.brightness !== undefined) out.brightness = e.brightness;
        }
        return out;
    };

    XoneK3.applyBeat = function(entry, beat, count) {
        // Count 0 and negative counts are valid — the phrase origin
        // can sit anywhere in the track.
        if (!beat || typeof count !== 'number' || !isFinite(count)) return entry;
        var lc = XoneK3.beatLevel(beat, count);
        if (!lc) return entry;
        if (lc.skip) return XoneK3.BEAT_HOLD;
        if (lc.color === null && lc.brightness === null) return entry;

        // Reuse entry.rgb unless the cascade supplies a new colour.
        // Downstream _sendColor reads indices without mutating, so
        // borrowing the array avoids one allocation per beat LED
        // render for the common brightness-only case.
        var out = {
            rgb: entry.rgb,
            brightness: entry.brightness,
            brightnessOff: entry.brightnessOff,
            type: entry.type
        };
        if (lc.color !== null) {
            var ce = XoneK3.resolveColorEntry(lc.color);
            if (ce && ce.rgb) {
                out.rgb = ce.rgb.slice();
                if (lc.brightness === null && _colorHasBrightness(lc.color)) {
                    out.brightness = ce.brightness;
                }
            }
        }
        if (typeof lc.brightness === 'number') out.brightness = lc.brightness;
        return out;
    };

    // ============================================================
    // K3-MIDI PALETTE
    // ============================================================
    XoneK3.rgbToK2 = function(r, g, b, hw) {
        if (hw <= 0) return -1;
        var peak = Math.max(r, g, b);
        if (peak < 8) return -1;
        var drg = r - g;
        if (drg >  XoneK3.K2_RG_THRESHOLD) return 0;
        if (drg < -XoneK3.K2_RG_THRESHOLD) return 2;
        return 1;
    };

    XoneK3.idxToK2 = function(idx) {
        if (idx >= 0x00 && idx <= 0x0B) {
            var r = idx >> 2, c = idx & 3;
            return { base: 0x28 + (2 - r) * 4 + c, step: 36 };
        }
        if (idx >= 0x0C && idx <= 0x1B) {
            var p = idx - 0x0C;
            var rf = 3 - (p >> 2);
            return { base: 0x18 + rf * 4 + (p & 3), step: 36 };
        }
        if (idx === 0x1C) return { base: 0x0C, step: 4 };
        if (idx === 0x1D) return { base: 0x0F, step: 4 };
        if (idx >= 0x1E && idx <= 0x21) return { base: 0x34 + (idx - 0x1E), step: 36 };
        return null;
    };

    XoneK3.sendK2Note = function(ch, idx, r, g, b, hw) {
        var pos = XoneK3.idxToK2(idx); if (!pos) return;
        var bucket = XoneK3.rgbToK2(r, g, b, hw);
        if (bucket < 0) { midi.sendShortMsg(0x90 + ch, pos.base, 0x00); return; }
        var peak = Math.max(r, g, b);
        var vel = Math.max(4, Math.min(127, Math.round(hw * peak / 127)));
        midi.sendShortMsg(0x90 + ch, pos.base + bucket * pos.step, vel);
    };
})();