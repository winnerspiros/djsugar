// xone-k3-lib-controller-colour.js — colour-scheme parser for
// K3Controller: on: shapes, base scheme resolution, schemeFromState
// closures, and the _setupColorScheme entry point.
//
// Loaded after lib-controller.js, before lib-controller-beat.js.
// Extends K3Controller.prototype. Called from _applyBinding.

'use strict';

(function() {
    if (typeof XoneK3 === 'undefined') {
        throw new Error('Xone K3: xone-k3-lib-controller-colour.js — xone-k3-lib-core.js must load first');
    }
    if (typeof XoneK3._requireLoad !== 'function') {
        throw new Error('Xone K3: xone-k3-lib-controller-colour.js — xone-k3-lib-core.js must load first');
    }
    XoneK3._requireLoad('xone-k3-lib-controller-colour.js',
        ['K3Controller', 'resolve', '_warnOnce']);

    // Fields recognised inside colorScheme.on's conditional form.
    // Anything else in the object warns once per binding and is
    // ignored — keeping the list at module scope makes "adding a
    // new on: field means editing exactly this" obvious.
    var KNOWN_ON = ['patchActive', 'state', 'equals', 'contains', 'co', 'group'];

    // ============================================================
    // on: PARSING
    // ============================================================
    XoneK3.K3Controller.prototype._parseCsOn = function(b, cs) {
        if (cs.on === undefined || cs.on === null) return;
        if (cs.on === true)  { b._csOn = true;  return; }
        if (cs.on === false) { b._csOn = false; return; }
        if (typeof cs.on !== 'object' || Array.isArray(cs.on)) {
            XoneK3._warnOnce('csOnShape:' + b._name,
                'colorScheme.on must be true, false, or a conditional object on ' + b._name +
                ' — got ' + (Array.isArray(cs.on) ? 'array' : typeof cs.on));
            return;
        }

        var o = {};
        if (cs.on.patchActive !== undefined) o.patchActive = cs.on.patchActive;
        if (cs.on.state       !== undefined) o.state = cs.on.state;
        if (cs.on.equals      !== undefined) o.equals = cs.on.equals;
        if (cs.on.contains    !== undefined) o.contains = cs.on.contains;
        if (cs.on.group !== undefined && cs.on.co === undefined) {
            XoneK3._warnOnce('csOnGroupNoCo:' + b._name,
                'colorScheme.on on ' + b._name +
                ' has `group` but no `co` — group is ignored');
        }
        if (cs.on.co !== undefined) {
            var g = cs.on.group ? XoneK3.resolve(cs.on.group, this) : b.group;
            if (!g) {
                XoneK3._warnOnce('csOnCoGroup:' + b._name,
                    'colorScheme.on.co on ' + b._name +
                    ' has no group and the binding has no group either');
                return;
            }
            o.co = cs.on.co;
            o.group = g;
        }

        this._warnUnknownCsOnFields(b, cs.on);

        if (Object.keys(o).length === 0) {
            XoneK3._warnOnce('csOnEmpty:' + b._name,
                'colorScheme.on has no recognised fields on ' + b._name +
                ' — on: ignored; LED falls back to outKey (or stays off if none).');
            return;
        }

        // equals / contains without a state / co / patchActive
        // source: _evalCsOn passes `undefined` to _matchOnValue,
        // which never matches. LED would silently stay off.
        var hasSource = (o.patchActive !== undefined ||
                         o.state       !== undefined ||
                         o.co          !== undefined);
        if (!hasSource) {
            var orphan = (o.equals !== undefined) ? 'equals' : 'contains';
            XoneK3._warnOnce('csOnNoSource:' + b._name,
                'colorScheme.on on ' + b._name + ' has `' + orphan + '` ' +
                'but no `state`, `co`, or `patchActive` to compare against ' +
                '— no match possible; LED renders its off-ramp.');
        }
        if (o.patchActive !== undefined &&
            (o.equals !== undefined || o.contains !== undefined)) {
            XoneK3._warnOnce('csOnPatchActiveCmp:' + b._name,
                'colorScheme.on on ' + b._name +
                ' has `equals`/`contains` alongside `patchActive` — ' +
                'comparators are ignored when patchActive is the source');
        }
        if (o.patchActive !== undefined &&
            (o.co !== undefined || o.state !== undefined)) {
            XoneK3._warnOnce('csOnPatchActiveSource:' + b._name,
                'colorScheme.on on ' + b._name +
                ' has both `patchActive` and `co`/`state` — patchActive ' +
                'wins, the other source is ignored');
        }

        // Assign _csOn before wiring. Mixxx currently defers the
        // CO callback, so the order didn't matter in practice, but
        // a synchronous fire (future Mixxx, test mock) would read
        // _csOn === null and fall through to the outKey branch in
        // _isOn. Ordering costs nothing.
        b._csOn = o;
        this._wireCsOnConnection(b, o);
    };

    XoneK3.K3Controller.prototype._warnUnknownCsOnFields = function(b, csOn) {
        for (var k in csOn) {
            if (!csOn.hasOwnProperty(k)) continue;
            if (KNOWN_ON.indexOf(k) === -1) {
                XoneK3._warnOnce('csOnUnknown:' + b._name + ':' + k,
                    'colorScheme.on on ' + b._name +
                    ' has unknown field "' + k + '" — ignored');
            }
        }
    };

    XoneK3.K3Controller.prototype._wireCsOnConnection = function(b, o) {
        if (!o.co || !o.group) return;
        b._conns.push(engine.makeConnection(o.group, o.co, function(v) {
            o.cachedValue = v;
            b._refresh();
        }));
        // Seed the cache with a live read. Mixxx doesn't fire the
        // listener on setup — only on change — so the first
        // _refresh would otherwise find the cache undefined and do
        // a live read that never gets reused.
        try {
            o.cachedValue = engine.getValue(o.group, o.co);
        } catch (e) {
            o.cachedValue = undefined;
        }
    };

    // ============================================================
    // BASE SCHEME
    // ============================================================
    // Returns true iff cs.scheme is a function (i.e. a live scheme
    // closure the binding invokes per render). Compiles the static
    // case into b._csScheme and returns false. Sets b._csFailed on
    // an unresolvable static scheme. Safe to call with no base
    // scheme — returns false without touching b._csScheme (the
    // caller's hasFromState path handles that case).
    XoneK3.K3Controller.prototype._parseSchemeValue = function(b, cs) {
        if (cs.scheme === undefined || cs.scheme === null) return false;
        if (typeof cs.scheme === 'function') return true;

        var comp = this._resolveSchemeRef(cs.scheme);
        if (!comp) {
            XoneK3._warnOnce('csUnresolved:' + b._name,
                'colorScheme: cannot resolve scheme for ' + b._name +
                ' — ' + String(JSON.stringify(cs.scheme)).slice(0, 120));
            b._csFailed = true;
            return false;
        }
        b._csScheme = comp;
        return false;
    };

    // ============================================================
    // schemeFromState
    // ============================================================
    // Caller guarantees `schemeFromState` is a non-array object
    // (see _parseCsScheme's `hasFromState` guard), so the shape
    // re-check and the return value are both dead.
    XoneK3.K3Controller.prototype._parseSchemeFromState = function(b, cs, schemeIsFn) {
        if (schemeIsFn) {
            XoneK3._warnOnce('csDoubleDynamic:' + b._name,
                'colorScheme on ' + b._name +
                ' has both schemeFromState and scheme: function' +
                ' — using schemeFromState');
        }
        var closure = this._buildCsFromStateClosure(b, cs.schemeFromState);
        if (closure) b._csSchemeFn = closure;
    };

    XoneK3.K3Controller.prototype._parseCsScheme = function(b, cs) {
        var hasBase = cs.scheme !== undefined && cs.scheme !== null;
        var fromState = cs.schemeFromState;
        var hasFromState = fromState && typeof fromState === 'object' &&
                        !Array.isArray(fromState);

        if (!hasBase && !hasFromState) {
            XoneK3._warnOnce('csMissing:' + b._name,
                'colorScheme: missing scheme for ' + b._name +
                (cs.knob !== undefined ? ' (deviation)' : '') +
                " — pass scheme: 'name' or a colour spec, or schemeFromState");
            b._csFailed = true;
            return;
        }

        // schemeFromState present but not an object — silently
        // dropped by the previous code when a base scheme was
        // also set (the "missing scheme" branch above only fires
        // when BOTH are missing). Warn so the user sees why their
        // conditional scheme isn't taking effect.
        if (cs.schemeFromState !== undefined && cs.schemeFromState !== null &&
            !hasFromState) {
            XoneK3._warnOnce('csFromStateShape:' + b._name,
                'colorScheme.schemeFromState on ' + b._name +
                ' must be an object — got ' +
                (Array.isArray(fromState) ? 'array' : typeof fromState) +
                '. Ignoring it; using the base scheme only.');
        }

        b._csDeviation = (cs.knob !== undefined && cs.knob !== null);

        var baseIsFn = this._parseSchemeValue(b, cs);
        if (b._csFailed) return;

        if (hasFromState) {
            this._parseSchemeFromState(b, cs, baseIsFn);
        } else if (baseIsFn) {
            b._csSchemeFn = cs.scheme;
        }

        this._wireCsKnob(b, cs);
    };

    // Deviation-LED binding: attach the knob the LED samples its
    // ramp position from. Runs only after the scheme half has
    // been validated — a knob reference on a scheme-less binding
    // would be meaningless.
    XoneK3.K3Controller.prototype._wireCsKnob = function(b, cs) {
        if (cs.knob === undefined || cs.knob === null) return;
        var knob = this.bindings[cs.knob];
        if (!knob) {
            XoneK3._warnOnce('csKnobMissing:' + b._name,
                'colorScheme: knob "' + cs.knob + '" not found for ' + b._name);
            b._csFailed = true;
            return;
        }
        // Only knobs and faders carry _normalized. A button /
        // encoder reference would freeze the LED on boot-white
        // forever (_renderDeviation's
        // `knob._normalized === undefined` branch).
        if (knob.type !== 'knob' && knob.type !== 'fader') {
            XoneK3._warnOnce('csKnobType:' + b._name,
                'colorScheme: knob "' + cs.knob + '" on ' + b._name +
                ' is type "' + knob.type + '" — must be knob or fader');
            b._csFailed = true;
            return;
        }
        b._csKnob = knob;
        knob.addObserver(b);
    };

    XoneK3.K3Controller.prototype._buildCsFromStateClosure = function(b, sf) {
        var sd = {};

        if (sf.state && sf.co) {
            XoneK3._warnOnce('csFsBoth:' + b._name,
                'colorScheme.schemeFromState on ' + b._name +
                ' has both state and co — using state, ignoring co');
        }
        if (sf.state) {
            sd.state = sf.state;
        } else if (sf.co) {
            sd.co = XoneK3.resolve(sf.co, this);
            sd.group = sf.group ? XoneK3.resolve(sf.group, this) : b.group;
            if (!sd.group) {
                XoneK3._warnOnce('csFsCoNoGroup:' + b._name,
                    'colorScheme.schemeFromState.co on ' + b._name +
                    ' has no group and the binding has no group either — LED will stay dark.');
                return null;
            }
        }
        if (sf.map) sd.map = sf.map;
        else if (sf.list) sd.list = sf.list.slice();
        if (!sd.state && !sd.co) {
            XoneK3._warnOnce('csFsEmpty:' + b._name,
                'colorScheme.schemeFromState on ' + b._name +
                ' has neither state nor co — LED will stay dark');
            return null;
        }
        // A state/co source without a map or list produces a
        // closure that always returns undefined. That silently
        // falls back to the base scheme (if set) — no
        // csDarkFromFn warning because the base masks it — so
        // warn here at parse time.
        if (!sd.map && !sd.list) {
            XoneK3._warnOnce('csFsNoMap:' + b._name,
                'colorScheme.schemeFromState on ' + b._name +
                ' has a source but no map/list — closure always returns ' +
                'undefined; base scheme (or dark) renders instead.');
        }

        if (sd.co && sd.group) {
            b._csFromStateConn = engine.makeConnection(sd.group, sd.co,
                function(v) {
                    sd.cachedValue = v;
                    b._refresh();
                });
            // Seed the cache with a live read — see
            // _wireCsOnConnection for the rationale.
            try {
                sd.cachedValue = engine.getValue(sd.group, sd.co);
            } catch (e) {
                sd.cachedValue = undefined;
            }
        }

        var ctrl = this;
        return function() {
            var stateVal;
            if (sd.co) {
                stateVal = sd.cachedValue !== undefined
                    ? sd.cachedValue
                    : engine.getValue(sd.group, sd.co);
            } else {
                stateVal = XoneK3.readPath(ctrl, sd.state);
            }

            var entryRef;
            if (sd.map) {
                entryRef = sd.map[stateVal];
            } else if (sd.list) {
                var sidx = Math.floor(stateVal);
                if (!isFinite(sidx)) sidx = 0;
                if (sidx < 0) sidx = 0;
                if (sidx >= sd.list.length) sidx = sd.list.length - 1;
                entryRef = sd.list[sidx];
            }

            if (typeof entryRef === 'function') entryRef = entryRef();
            return entryRef;
        };
    };

    // ============================================================
    // SETUP ENTRY POINT
    // ============================================================
    XoneK3.K3Controller.prototype._setupColorScheme = function(b, cs) {
        if (!cs || typeof cs !== 'object' || Array.isArray(cs)) {
            XoneK3._warnOnce('csShape:' + b._name,
                'colorScheme must be an object on ' + b._name +
                ' — got ' + (cs === null ? 'null' : typeof cs));
            b._csActive = false;
            return;
        }
        b._csActive = true;
        b._csFailed = false;
        b._csDeviation = false;
        b._csScheme = null;
        b._csSchemeFn = null;
        b._csOn = null;
        this._parseCsOn(b, cs);
        this._parseCsScheme(b, cs);
    };
})();