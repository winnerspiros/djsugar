// xone-k3-lib-schemes.js — scheme engine: palette, transfer
// curves, Mixxx colour lookup, colour-spec parsing, compile /
// sample pipeline.
//
// Loaded by every preset, before any preset's binding config.
// Preset-specific schemes, THEME, and QFX_CHAIN_LIST live in
// the preset files — see xone-k3-cat.js.

'use strict';

(function() {
    if (typeof XoneK3 === 'undefined') {
        throw new Error('Xone K3: <filename> — xone-k3-lib-core.js must load first');
    }

    if (typeof XoneK3._requireLoad !== 'function') {
        throw new Error('Xone K3: xone-k3-lib-schemes.js — xone-k3-lib-core.js must load first');
    }
    XoneK3._requireLoad('xone-k3-lib-schemes.js',
        ['CURVE_GAIN', 'DEFAULT_RGB_BRIGHTNESS',
         'DEFAULT_RGB_BRIGHTNESS_OFF', 'DEFAULT_CO_SCHEME',
         'CO_MATCH_EPSILON', 'RATIO_GAIN']);

    // ============================================================
    // PALETTE
    // ============================================================
    XoneK3.SYSEX_PALETTE = {
        off:        [0,0,0],
        red:        [0x7F,0,0],
        green:      [0,0x7F,0],
        blue:       [0,0,0x7F],
        amber:      [0x7F,0x40,0],
        orange:     [127,22,0],
        orange_red: [72,5,0],
        yellow:     [0x7F,0x7F,0],
        yellow2:    [0x7F,0x7F,0x20],
        lime:       [0x40,0x7F,0],
        cyan:       [0,0x7F,0x7F],
        sky:        [0x20,0x60,0x7F],
        sky2:       [0x20,0x50,0x7F],
        purple:     [0x40,0x20,0x7F],
        magenta:    [0x7F,0,0x7F],
        pink:       [0x7F,0x30,0x50],
        white:      [0x7F,0x7F,0x7F],
        teal:       [0,70,58],
        teal2:      [13,127,21],
        teal3:      [0,127,43],
        teal_fade:  [0x10,0x50,0x10],
        cool_white: [0x50,0x60,0x7F],
        dim:        [0x10,0x10,0x10],
        m_red:      [0x6F, 0x05, 0x00],
        m_orange:   [0x7F, 0x20, 0x00],
        m_yellow:   [0x7F, 0x56, 0x00],
        m_green:    [0x15, 0x57, 0x05],
        m_celeste:  [0x15, 0x7F, 0x4F],
        m_blue:     [0x00, 0x0F, 0x7F],
        m_purple:   [0x4F, 0x00, 0x5F],
        m_pink:     [0x6F, 0x3F, 0x3F],
        m_white:    [0x4F, 0x5F, 0x45]
    };

    // ============================================================
    // LED TRANSFER CURVES
    // ============================================================
    XoneK3.LED_TRANSFERS = {
        measuredRGB: {
            r: [
                [0,     0], [2,    45], [5,   138], [8,   245],
                [11,  350], [14,  500], [18,  660], [24,  900],
                [32, 1316], [42, 1867], [53, 2500], [64, 3320],
                [74, 4221], [85, 4720], [96, 5787], [106, 6500],
                [114, 6900], [120, 7400], [127, 8200]
            ],
            g: [
                [0,     0], [2,    68], [5,   195], [8,   340],
                [11,  520], [14,  700], [18,  970], [24, 1350],
                [32, 1937], [42, 2754], [53, 3783], [64, 4892],
                [74, 6524], [85, 7538], [96, 9316], [106, 10371],
                [114, 10800], [120, 11800], [127, 12700]
            ],
            b: [
                [0,      0], [2,    377], [5,   1140], [8,   2295],
                [11,  3430], [14,  5213], [18,  6600], [24,  9200],
                [32, 14586], [42, 20313], [53, 25430], [64, 30456],
                [74, 35542], [85, 38931], [96, 42822], [106, 44800],
                [114, 45800], [120, 47800], [127, 49500]
            ]
        }
    };

    // ============================================================
    // MIXXX HOTCUE COLOURS
    // ============================================================
    XoneK3.MIXXX_COLORS = {
        0xC50A08:[0x6F,0x05,0x00],0xFF8000:[0x7F,0x20,0x00],
        0xF8D200:[0x7F,0x56,0x00],0x32BE44:[0x15,0x67,0x05],
        0x42D4F4:[0x15,0x7F,0x4F],0x0044FF:[0x00,0x0F,0x7F],
        0xAF00CC:[0x4F,0x00,0x5F],0xFCA6D7:[0x7F,0x3F,0x3F],
        0xF2F2FF:[0x60,0x60,0x60],0x000000:[0x00,0x00,0x00],
        0xFF0000:[0x7F,0x00,0x00],0xFFFF00:[0x7F,0x7F,0x00],
        0x00FF00:[0x00,0x7F,0x00],0x00FFFF:[0x00,0x7F,0x7F],
        0x0000FF:[0x00,0x00,0x7F],0xFF00FF:[0x7F,0x00,0x7F],
        0xFF88FF:[0x7F,0x40,0x7F],0xFFFFFF:[0x7F,0x7F,0x7F],
        0x000088:[0x00,0x00,0x40],0x008800:[0x00,0x40,0x00],
        0x008888:[0x00,0x40,0x40],0x228800:[0x10,0x40,0x00],
        0x880000:[0x40,0x00,0x00],0x882200:[0x40,0x10,0x00],
        0x888800:[0x40,0x40,0x00],0x888888:[0x40,0x40,0x40],
        0x88FF00:[0x40,0x7F,0x00],0xFF8800:[0x7F,0x40,0x00]
    };

    XoneK3.mapMixxxColor = function(rgb) {
        if (!rgb || rgb <= 0) return null;
        if (XoneK3.MIXXX_COLORS[rgb]) return XoneK3.MIXXX_COLORS[rgb];
        return [
            Math.round(((rgb >> 16) & 0xFF) / 2),
            Math.round(((rgb >> 8)  & 0xFF) / 2),
            Math.round(( rgb        & 0xFF) / 2)
        ];
    };

    // ============================================================
    // COLOUR RESOLVER
    // ============================================================
    // Palette lookups read XoneK3.SYSEX_PALETTE directly. No
    // copy registry — a copy was only ever a boot-time snapshot,
    // and _parseNamedEntry runs at compile time anyway.

    // ---------- Colour-entry parsing ----------
    // A colour spec is normalised to a `parts` array (the same
    // shape a comma-separated string produces), then dispatched
    // by the shape of parts[0]:
    //   'mixxx'            type:'mixxx', rgb deferred to the pad
    //   numeric, len >= 3  explicit R,G,B [brightness [off]]
    //   anything else      named-palette lookup

    function _normalizeColorParts(entry) {
        if (typeof entry === 'string') {
            return entry.split(',').map(function(s) { return s.trim(); });
        }
        if (Array.isArray(entry)) {
            return entry.map(function(v) {
                return (typeof v === 'string') ? v.trim() : v;
            });
        }
        return null;
    }

    function _clampBr(v) { return Math.max(0, Math.min(127, v)); }

    function _pickBr(parts, i, fb) {
        if (parts.length <= i) return fb;
        var v = parseInt(parts[i], 10);
        return isNaN(v) ? fb : _clampBr(v);
    }

    function _pickBrOrNull(parts, i) {
        if (parts.length <= i) return null;
        var v = parseInt(parts[i], 10);
        return isNaN(v) ? null : _clampBr(v);
    }

    function _parseMixxxEntry(parts, DEF) {
        return {
            rgb: null,
            brightness:    _pickBr(parts, 1, DEF),
            brightnessOff: _pickBrOrNull(parts, 2),
            type: 'mixxx'
        };
    }

    function _parseRgbEntry(parts, DEF) {
        var r = parseInt(parts[0], 10);
        var g = parseInt(parts[1], 10);
        var b = parseInt(parts[2], 10);
        if (isNaN(r) || isNaN(g) || isNaN(b)) return null;
        return {
            rgb: [_clampBr(r), _clampBr(g), _clampBr(b)],
            brightness:    _pickBr(parts, 3, DEF),
            brightnessOff: _pickBrOrNull(parts, 4)
        };
    }

    function _parseNamedEntry(parts, DEF) {
        var first = (typeof parts[0] === 'string') ? parts[0].toLowerCase() : parts[0];
        var rgbNamed = XoneK3.SYSEX_PALETTE[first];
        if (!rgbNamed) return null;
        return {
            rgb: rgbNamed.slice(),
            brightness:    _pickBr(parts, 1, DEF),
            brightnessOff: _pickBrOrNull(parts, 2)
        };
    }

    XoneK3.resolveColorEntry = function(entry, defaultBr) {
        if (entry === undefined || entry === null) return null;
        var DEF = (typeof defaultBr === 'number')
            ? Math.max(0, Math.min(127, defaultBr))
            : XoneK3.DEFAULT_RGB_BRIGHTNESS;

        var parts = _normalizeColorParts(entry);
        if (!parts || parts.length === 0 || parts[0] === '') return null;

        if (parts[0] === 'mixxx') return _parseMixxxEntry(parts, DEF);

        var n0 = parseInt(parts[0], 10);
        if (!isNaN(n0) && parts.length >= 3) return _parseRgbEntry(parts, DEF);

        return _parseNamedEntry(parts, DEF);
    };

    // ============================================================
    // SCHEME REGISTRY
    // ============================================================
    // Presets extend this. Only coMatch ships here — it's the
    // engine's default co-steering ramp, referenced by
    // SCHEME_DEFAULTS.coScheme and used by any deviation LED
    // whose knob and CO disagree.
    XoneK3.SCHEMES = {};
    XoneK3._schemeCache = {};
    XoneK3._compiledCache = {};

    XoneK3.SCHEME_DEFAULTS = {
        colors: null,
        ratio: null,
        curve: 0,
        schemePosition: 63.5,
        interp: true,
        calibration: true,
        brightness: 127,
        brightnessScale: 1,
        markerColor: null,
        markerPosition: 63.5,
        markerWidth: 0,
        offScheme: null,
        coScheme: 'coMatch',
        reverse: false,
        beat: null
    };

    XoneK3.resolveScheme = function(ref) {
        if (typeof ref === 'string' && XoneK3._schemeCache[ref]) {
            return XoneK3._schemeCache[ref];
        }

        var raw;
        if (typeof ref === 'string') {
            raw = XoneK3.SCHEMES[ref];
            if (!raw) return null;
        } else if (ref && typeof ref === 'object' && !Array.isArray(ref)) {
            var refKey = JSON.stringify(ref).slice(0, 60);
            if (ref.patch) {
                XoneK3._warnOnce('schemePatch:' + refKey,
                    'scheme patch syntax removed — pass a plain scheme ' +
                    'object, or build one via composeScheme in config. Got: ' +
                    JSON.stringify(ref).slice(0, 120));
                return null;
            }
            if (!ref.colors && Object.keys(ref).length > 0) {
                XoneK3._warnOnce('schemeNoColors:' + refKey,
                    'scheme object has no colors — partials are no longer ' +
                    'supported. Got: ' + JSON.stringify(ref).slice(0, 120));
                return null;
            }
            raw = ref;
        } else {
            return null;
        }

        var out = Object.assign({}, XoneK3.SCHEME_DEFAULTS, raw);
        if (typeof ref === 'string') XoneK3._schemeCache[ref] = out;
        return out;
    };

    // ============================================================
    // SEGMENTS — colour stops to weighted ranges
    // ============================================================
    XoneK3._buildSegments = function(colors, ratio, brightness) {
        if (!Array.isArray(colors) || colors.length === 0) return null;
        var resolved = [];
        for (var i = 0; i < colors.length; i++) {
            var e = colors[i], colorSpec = e, ratioSpec = null;
            if (e && typeof e === 'object' && !Array.isArray(e) && e.color !== undefined) {
                colorSpec = e.color;
                if (e.ratio !== undefined) ratioSpec = e.ratio;
            }
            var c = XoneK3.resolveColorEntry(colorSpec, brightness);
            if (!c) {
                XoneK3._warnOnce('segUnresolved:' + String(colorSpec),
                    '_buildSegments: could not resolve colour spec ' +
                    JSON.stringify(colorSpec).slice(0, 60) +
                    ' — segment skipped. Check spelling or palette name.');
                continue;
            }
            var w;
            if (ratioSpec !== null) w = ratioSpec;
            else if (Array.isArray(ratio) && i < ratio.length && ratio[i] !== undefined) w = ratio[i];
            else w = 1;
            if (w <= 0) w = 0;
            resolved.push({ color: c, weight: w });
        }
        if (resolved.length === 0) return null;
        var total = 0;
        for (var j = 0; j < resolved.length; j++) total += resolved[j].weight;
        if (total <= 0) total = resolved.length;
        var cum = 0, segs = [];
        for (var k = 0; k < resolved.length; k++) {
            var w2 = resolved[k].weight / total;
            segs.push({ start: cum, end: cum + w2, color: resolved[k].color });
            cum += w2;
        }
        return segs;
    };

    // Symmetric warp around the 0.5 pivot. `k > 0` squishes
    // boundaries toward the pivot, `k < 0` expands them away.
    // Unity at k=0, t=0, and t=1.
    function _warp(t, k) {
        if (k === 0 || t <= 0 || t >= 1) return t;
        var e = Math.pow(2, -k * XoneK3.CURVE_GAIN);
        if (t < 0.5) {
            return 0.5 * Math.pow(t / 0.5, e);
        }
        var u = (t - 0.5) / 0.5;
        return 0.5 + 0.5 * (1 - Math.pow(1 - u, e));
    }
    XoneK3._warp = _warp;

    XoneK3.sampleT = function(compiled, t) {
        var m = compiled.tmap;
        if (!m) return t;
        // Shift the input axis first, clamp, then warp. This makes
        // schemePosition slide the whole curve — including its
        // pivot — through knob space, instead of sliding a
        // pre-warped signal. Out-of-range inputs clamp to 0 or 1
        // and hold the boundary segment colour. Reordering from
        // the old (warp → shift → clamp) also preserves the
        // curve's shape at every position: previously the steep
        // pivot at 0.5 was eaten by clamp before the shift could
        // move it.
        t = t - m.position;
        if (t < 0) t = 0;
        if (t > 1) t = 1;
        if (m.curve !== 0) t = _warp(t, m.curve);
        return t;
    };

    XoneK3._computeTmap = function(raw) {
        return {
            position: (raw.schemePosition - 63.5) / 127,
            curve: raw.curve || 0
        };
    };

    // Off-ramp derived from an on-ramp: keep each segment's bounds
    // and colour, drop brightness to the default off level. Used
    // when a scheme has no explicit offScheme. Boundaries matter
    // — a scheme that re-shapes its off-ramp would break
    // sample-position alignment with the on-ramp otherwise.
    XoneK3._generateOff = function(segs) {
        var offBr = XoneK3.DEFAULT_RGB_BRIGHTNESS_OFF, out = [];
        for (var i = 0; i < segs.length; i++) {
            var c = segs[i].color;
            out.push({ start: segs[i].start, end: segs[i].end,
                       color: { rgb: c.rgb, brightness: offBr, type: c.type } });
        }
        return out;
    };

    // ============================================================
    // COMPILE
    // ============================================================
    // Two shapes:
    //   full   — raw has `colors`, gets tmap / off / co / marker
    //            / beat
    //   single — bare colour spec ('red, 80' / [127,0,0]),
    //            hard-coded 1-segment ramp with no dynamics
    //
    // The dispatcher handles cache lookup. A bare colour spec is
    // cached by name when ref is a string and no overrides were
    // passed. Cached values are never null (both helpers early-
    // return null before the cache write), matching the previous
    // behaviour.

    XoneK3._compileFullScheme = function(raw, overrides, seen) {
        if (overrides) raw = Object.assign({}, raw, overrides);

        var onSegs = XoneK3._buildSegments(raw.colors, raw.ratio, raw.brightness);
        if (!onSegs) return null;

        var offSegs = null;
        if (raw.offScheme) {
            var offC = XoneK3.compileScheme(raw.offScheme, undefined, seen);
            if (offC) offSegs = offC.on;
        }
        if (!offSegs) offSegs = XoneK3._generateOff(onSegs);

        var coCompiled = null;
        if (raw.coScheme !== null && raw.coScheme !== false) {
            var coName = raw.coScheme || XoneK3.DEFAULT_CO_SCHEME;
            if (coName) coCompiled = XoneK3.compileScheme(coName, undefined, seen);
        }

        var marker = raw.markerColor
            ? XoneK3.resolveColorEntry(raw.markerColor, raw.brightness)
            : null;
        if (marker &&
            (marker.brightnessOff === undefined || marker.brightnessOff === null)) {
            marker.brightnessOff = XoneK3.DEFAULT_RGB_BRIGHTNESS_OFF;
        }

        return {
            on: onSegs, off: offSegs, co: coCompiled, marker: marker,
            tmap: XoneK3._computeTmap(raw),
            markerPosition: (typeof raw.markerPosition === 'number') ? raw.markerPosition : 63.5,
            markerWidth:    (typeof raw.markerWidth    === 'number') ? raw.markerWidth    : 0,
            interp:         raw.interp !== false,
            calibration:    raw.calibration !== false,
            brightnessScale:(typeof raw.brightnessScale === 'number') ? raw.brightnessScale : 1,
            reverse:        raw.reverse === true,
            beat: (raw.beat && raw.beat.enabled !== false) ? raw.beat : null
        };
    };

    XoneK3._compileSingleColor = function(ref) {
        var entry = XoneK3.resolveColorEntry(ref);
        if (!entry) return null;
        var offBr = (typeof entry.brightnessOff === 'number')
            ? entry.brightnessOff : XoneK3.DEFAULT_RGB_BRIGHTNESS_OFF;
        return {
            on:  [{ start: 0, end: 1, color: entry }],
            off: [{ start: 0, end: 1,
                    color: { rgb: entry.rgb, brightness: offBr, type: entry.type } }],
            co: null, marker: null,
            tmap: { position: 0, curve: 0 },
            markerPosition: 63.5, markerWidth: 0,
            interp: false, calibration: false, brightnessScale: 1,
            reverse: false, beat: null
        };
    };

    XoneK3.compileScheme = function(ref, overrides, seen) {
        var cacheable = (typeof ref === 'string') && !overrides;
        if (cacheable && XoneK3._compiledCache[ref]) return XoneK3._compiledCache[ref];

        // Cycle detection. A scheme that names itself (directly or
        // through a chain) in offScheme / coScheme would otherwise
        // recurse until the JS stack overflows on first render.
        // Strings match by value, objects by reference. The cycle
        // is broken at the innermost back-reference: that
        // referencing field compiles to null, the outer call
        // finishes normally.
        seen = seen || new Set();
        if (seen.has(ref)) {
            if (typeof ref === 'string') {
                XoneK3._warnOnce('schemeCycle:' + ref,
                    'scheme "' + ref + '" references itself in its ' +
                    'offScheme / coScheme chain — cycle broken, the ' +
                    'referencing field renders as null');
            }
            return null;
        }
        seen.add(ref);

        var raw = XoneK3.resolveScheme(ref);
        var compiled;
        if (raw && raw.colors && raw.colors.length > 0) {
            compiled = XoneK3._compileFullScheme(raw, overrides, seen);
        } else {
            compiled = XoneK3._compileSingleColor(ref);
        }

        seen.delete(ref);
        if (cacheable && compiled) XoneK3._compiledCache[ref] = compiled;
        return compiled;
    };

    // ============================================================
    // SAMPLING
    // ============================================================
    XoneK3.markerHit = function(compiled, devDir) {
        var at = (compiled.markerPosition / 127) * 2 - 1;
        var half = (compiled.markerWidth || 0) / 127;
        return half > 0 && !!compiled.marker && Math.abs(devDir - at) <= half;
    };

    XoneK3.scaleTarget = function(target, scale) {
        if (target === null) return null;
        if (typeof scale !== 'number' || scale === 1) return target;
        return target * scale;
    };

    // Snap sample: pick the segment whose span contains t (end-
    // inclusive, last-segment fallback).
    //
    // `mixxx`-typed stops have null rgb — they only work on the
    // static single-colour path (`_renderStatic` → `_sendColor`,
    // which resolves rgb from the pad's hotcue colour CO at
    // render time). Sampling can't use them, so they're skipped;
    // the previous valid stop's colour is used instead (or null /
    // black LED when there is none). Warn once so the user can
    // see why a stop is being ignored.
    function _sampleSnap(segs, t) {
        // Index-tracked instead of wrapper-object-tracked to
        // avoid allocating a fresh { rgb, brightness, type } on
        // every iteration before the match.
        var lastValidIdx = -1;
        for (var i = 0; i < segs.length; i++) {
            var c = segs[i].color;
            if (!c || c.rgb === null) {
                XoneK3._warnOnce('sampleSegsMixxx',
                    'sampling a scheme containing a `mixxx` colour stop — ' +
                    '`mixxx` is only supported on static single-colour LEDs. ' +
                    'The stop is skipped during sampling.');
                if (t <= segs[i].end || i === segs.length - 1) {
                    if (lastValidIdx === -1) return null;
                    var lv = segs[lastValidIdx].color;
                    return {
                        rgb: [lv.rgb[0], lv.rgb[1], lv.rgb[2]],
                        brightness: lv.brightness,
                        type: lv.type
                    };
                }
                continue;
            }
            lastValidIdx = i;
            if (t <= segs[i].end || i === segs.length - 1) {
                return {
                    rgb: [c.rgb[0], c.rgb[1], c.rgb[2]],
                    brightness: c.brightness,
                    type: c.type
                };
            }
        }
        return null;
    }

    // Blend sample: linear-interp between the two segment centres
    // surrounding t. Centre-based means the colour of segment i
    // sits at (start+end)/2, so the blend region between two
    // segments straddles the boundary between them. Previous
    // behaviour preserved exactly — including the "before falls
    // through to after" and "return before if after is null" edge
    // cases at t<firstCentre and t>lastCentre.
    //
    // Null-rgb (mixxx) stops are skipped — see _sampleSnap.
    function _sampleBlend(segs, t) {
        // Index-tracked before/after — avoids the intermediate
        // { t, color } wrapper allocation per candidate segment.
        // Behaviour is identical: `before` is the last centre <= t,
        // `after` is the first centre >= t.
        var beforeIdx = -1, beforeT = 0;
        var afterIdx  = -1, afterT  = 0;
        for (var j = 0; j < segs.length; j++) {
            var c = segs[j].color;
            if (!c || c.rgb === null) {
                XoneK3._warnOnce('sampleSegsMixxx',
                    'sampling a scheme containing a `mixxx` colour stop — ' +
                    '`mixxx` is only supported on static single-colour LEDs. ' +
                    'The stop is skipped during sampling.');
                continue;
            }
            var centre = (segs[j].start + segs[j].end) / 2;
            if (centre <= t) { beforeIdx = j; beforeT = centre; }
            if (centre >= t && afterIdx === -1) { afterIdx = j; afterT = centre; }
        }
        if (beforeIdx === -1 && afterIdx === -1) return null;
        if (beforeIdx === -1) { beforeIdx = afterIdx; beforeT = afterT; }
        if (afterIdx  === -1) { afterIdx  = beforeIdx; afterT  = beforeT; }
        var cA = segs[beforeIdx].color;
        if (beforeIdx === afterIdx || beforeT === afterT) {
            return {
                rgb: [cA.rgb[0], cA.rgb[1], cA.rgb[2]],
                brightness: cA.brightness,
                type: cA.type
            };
        }
        var lt = (t - beforeT) / (afterT - beforeT);
        var cB = segs[afterIdx].color;
        return {
            rgb: [
                Math.round(cA.rgb[0]*(1-lt)+cB.rgb[0]*lt),
                Math.round(cA.rgb[1]*(1-lt)+cB.rgb[1]*lt),
                Math.round(cA.rgb[2]*(1-lt)+cB.rgb[2]*lt)
            ],
            brightness: Math.round(cA.brightness*(1-lt)+cB.brightness*lt),
            type: cA.type || cB.type
        };
    }

    XoneK3.sampleSegs = function(segs, t, interp) {
        if (!segs || segs.length === 0) return null;
        if (t < 0) t = 0;
        if (t > 1) t = 1;
        return interp ? _sampleBlend(segs, t) : _sampleSnap(segs, t);
    };

    // Shared by Binding._renderCoMatch and LabController._renderCoMatch.
    // When a deviation LED's physical knob position disagrees with
    // its bound CO by more than CO_MATCH_EPSILON, the LED shows the
    // matching gradient so the user can nudge until they line up.
    // Returns the ramp to sample plus the t-point along it, or
    // null when the knob and CO already agree (or when the scheme
    // has no co-ramp, or the group / key are missing).
    //
    //   compiled  the outer compiled scheme (needs .co)
    //   group     CO group to read the target position from
    //   key       CO name to read
    //   physical  knob position, 0..1
    //   isOn      current LED on-state; picks on-ramp or off-ramp
    //
    // Caller feeds the return through _sampleRamp / _sendSampled
    // with skipBeat=true — co-match bypasses the beat cascade.
    XoneK3.coMatchSample = function(compiled, group, key, physical, isOn, cachedCoVal) {
        if (!compiled || !compiled.co) return null;
        if (!group || !key) return null;
        var coVal = (typeof cachedCoVal === 'number')
            ? cachedCoVal
            : engine.getParameter(group, key);
        var dev = physical - coVal;
        if (dev < -1) dev = -1;
        if (dev > 1)  dev = 1;
        if (Math.abs(dev) <= XoneK3.CO_MATCH_EPSILON) return null;
        return {
            ramp: isOn ? compiled.co.on : compiled.co.off,
            t: (dev + 1) / 2
        };
    };

    // ============================================================
    // ENGINE DEFAULT — co-steering ramp
    // ============================================================
    // Default co-steering ramp — shown when a deviation LED's
    // knob disagrees with its bound CO. Referenced by
    // SCHEME_DEFAULTS.coScheme. Override per-scheme with
    // coScheme: 'name' or coScheme: null to disable.
    XoneK3.SCHEMES.coMatch = {
        colors: [
            [100, 55, 55],
            [100, 69, 69],
            [95, 79, 32],
            [57, 41, 12]
        ],
        ratio: [30, 20, 20, 30],
        curve: -0.75,
        markerWidth: 0,
        coScheme: null
    };
})();