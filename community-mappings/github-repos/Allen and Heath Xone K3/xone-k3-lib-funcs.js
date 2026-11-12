// xone-k3-lib-funcs.js — builders + XoneK3.FUNCS.
//
// Loaded by every preset, after lib-core and lib-schemes.
// Beatjump gating and the loop-touch stamping convention are
// documented at `_beatjumpAllowed` / `_touchLoop` (render) and
// INTERNALS §21.
//
// Held-press state
// ----------------
// This file no longer carries per-press state. Modifier states
// (shift, chord, loadArm) are tracked by the controller's
// transaction/modifier system (see lib-controller.js). Funcs
// that need to know "is X held" read `ctrl.modifiers.<name>`.
// See F.slipOrLoad and F.k3Select for the pattern.

'use strict';

(function() {
    if (typeof XoneK3 === 'undefined') {
        throw new Error('Xone K3: <filename> — xone-k3-lib-core.js must load first');
    }
    if (typeof XoneK3._requireLoad !== 'function') {
        throw new Error('Xone K3: xone-k3-lib-funcs.js — xone-k3-lib-core.js must load first');
    }
    XoneK3._requireLoad('xone-k3-lib-funcs.js',
        ['SCHEMES', 'SCHEME_DEFAULTS', '_funcId', '_warnOnce']);

    // ============================================================
    // MODULE HELPERS
    // ============================================================
    // "pressR held = load onto channel" branch shared by
    // k3Select / k3SelectActive / setActiveDeckTo. Reads the
    // controller's loadArm modifier, which is derived from
    // pressR's active transaction.
    function _loadSelectedOntoChannel(ctrl, channel) {
        if (!ctrl.modifiers.loadArm) return false;
        if (!(channel >= 1 && channel <= 4)) return false;
        engine.setValue('[Channel' + channel + ']', 'LoadSelectedTrack', 1);
        return true;
    }

    // The batch-edit toggleOverlay runs when activating an overlay
    // for the first time: strip every named sibling, then append.
    function _addOverlayWithSiblingClear(ctrl, name, siblings) {
        ctrl._batchPatches(function(patches) {
            if (siblings.length > 0) {
                for (var i = patches.length - 1; i >= 0; i--) {
                    if (patches[i] !== name && siblings.indexOf(patches[i]) !== -1) {
                        patches.splice(i, 1);
                    }
                }
            }
            patches.push(name);
        });
    }

    // ============================================================
    // CONFIG BUILDERS
    // ============================================================
    XoneK3.builders = {};

    XoneK3.builders._fxCs = function(opts) {
        var cs = {
            knob: opts.knob,
            scheme: XoneK3.builders.composeScheme(opts, undefined)
        };
        if (opts.schemeFromState !== undefined) {
            cs.schemeFromState = opts.schemeFromState;
        }
        return cs;
    };

    var EQ_BANDS = { high: 3, mid: 2, low: 1 };

    XoneK3.builders.composeScheme = function(opts, base) {
        if (!opts) return base;
        var ref = (opts.scheme !== undefined) ? opts.scheme : base;
        if (ref === undefined || ref === null) return null;

        var markerPosition = opts.markerPosition;
        if (markerPosition === undefined && opts.marker !== undefined) {
            markerPosition = XoneK3.builders.marker(opts.marker);
        }

        var hasOverride =
            opts.markerColor    !== undefined ||
            markerPosition      !== undefined ||
            opts.markerWidth    !== undefined ||
            opts.schemePosition !== undefined ||
            opts.reverse        !== undefined ||
            opts.curve          !== undefined ||
            opts.brightness     !== undefined ||
            opts.beat           !== undefined;

        if (typeof ref === 'function') {
            var capturedOpts = opts;
            var capturedBase = base;
            var capturedRef = ref;
            var warnedNull = false;

            if (!hasOverride) {
                return function() {
                    var v = capturedRef.call(this);
                    if ((v === undefined || v === null) && !warnedNull) {
                        warnedNull = true;
                        XoneK3._warnOnce('composeSchemeFnNull:' + XoneK3._funcId(capturedRef),
                            'composeScheme: scheme function returned ' +
                            (v === null ? 'null' : 'undefined') +
                            ' — LED will stay dark until it returns a usable ref');
                    }
                    return v;
                };
            }

            var lastInner = null, lastComposed = null;
            return function() {
                var inner = capturedRef.call(this);
                if (inner === undefined || inner === null) {
                    if (!warnedNull) {
                        warnedNull = true;
                        XoneK3._warnOnce('composeSchemeFnNull:' + XoneK3._funcId(capturedRef),
                            'composeScheme: scheme function returned ' +
                            (inner === null ? 'null' : 'undefined') +
                            ' — LED will stay dark until it returns a usable ref');
                    }
                    return null;
                }
                if (inner === lastInner) return lastComposed;
                lastInner = inner;
                lastComposed = XoneK3.builders.composeScheme(
                    Object.assign({}, capturedOpts, { scheme: inner }),
                    capturedBase);
                return lastComposed;
            };
        }

        if (typeof ref === 'string' && !hasOverride) return ref;

        var resolved;
        if (typeof ref === 'string') {
            var reg = XoneK3.SCHEMES[ref];
            if (!reg) {
                XoneK3._warnOnce('composeSchemeMissing:' + ref,
                    'composeScheme: scheme "' + ref + '" not found');
                return null;
            }
            resolved = Object.assign({}, reg);
        } else if (typeof ref === 'object' && !Array.isArray(ref)) {
            if (ref.patch) {
                XoneK3._warnOnce('composeSchemePatch:' + JSON.stringify(ref).slice(0, 60),
                    'composeScheme: scheme patches are no longer supported — ' +
                    'pass a plain scheme object or a name plus inline overrides. ' +
                    'Got: ' + JSON.stringify(ref).slice(0, 120));
                return null;
            }
            resolved = Object.assign({}, ref);
        } else if (Array.isArray(ref)) {
            // Colour spec array — valid in colorScheme.scheme, was
            // silently rejected here. Wrap as a single-stop scheme
            // with interp/calibration/coScheme off so the compiled
            // output matches _compileSingleColor's behaviour for a
            // bare colour spec. Overrides apply on top of the same
            // baseline.
            resolved = { colors: [ref], interp: false, calibration: false,
                         coScheme: null };
        } else {
            return null;
        }

        if (opts.markerColor    !== undefined) resolved.markerColor    = opts.markerColor;
        if (markerPosition      !== undefined) resolved.markerPosition = markerPosition;
        if (opts.markerWidth    !== undefined) resolved.markerWidth    = opts.markerWidth;
        if (opts.schemePosition !== undefined) resolved.schemePosition = opts.schemePosition;
        if (opts.reverse        !== undefined) resolved.reverse        = opts.reverse;
        if (opts.curve          !== undefined) resolved.curve          = opts.curve;
        if (opts.brightness     !== undefined) resolved.brightness     = opts.brightness;
        if (opts.beat           !== undefined) resolved.beat           = opts.beat;

        return resolved;
    };

    XoneK3.builders.marker = function(v) {
        if (v === undefined) return undefined;
        if (v === 'low')  return 0;
        if (v === 'mid')  return 63.5;
        if (v === 'high') return 127;
        return v;
    };

    XoneK3.builders.ledTrackButton = function(knob, scheme, state, marker, schemePosition) {
        var opts = { scheme: scheme };
        if (marker !== undefined) opts.marker = marker;
        if (schemePosition !== undefined) opts.schemePosition = schemePosition;
        var cs = { knob: knob, scheme: XoneK3.builders.composeScheme(opts) };
        if (state === 'on') cs.on = true;
        return { colorScheme: cs };
    };

    XoneK3.builders.eqKnob = function(deck, band) {
        return {
            group: '[EqualizerRack1_' + deck + '_Effect1]',
            inKey: 'parameter' + EQ_BANDS[band]
        };
    };

    XoneK3.builders.fxUnitKnob = function(unit, key) {
        return {
            group: '[EffectRack1_EffectUnit' + unit + ']',
            inKey: key
        };
    };

    XoneK3.builders.fxKnob = function(unit, effect, param) {
        return {
            group: '[EffectRack1_EffectUnit' + unit + '_Effect' + effect + ']',
            inKey: (param === 'meta') ? 'meta' : ('parameter' + param)
        };
    };

    XoneK3.builders.fxEnabled = function(unit, effect, opts) {
        opts = opts || {};
        if (opts.scheme === undefined && opts.schemeFromState === undefined) {
            print('[k3] fxEnabled unit=' + unit + ' effect=' + effect +
                ' missing scheme — pass scheme: \'catFX\' (or similar)');
        }
        return {
            group: '[EffectRack1_EffectUnit' + unit + '_Effect' + effect + ']',
            inKey: 'enabled', outKey: 'enabled',
            colorScheme: XoneK3.builders._fxCs(opts)
        };
    };

    XoneK3.builders.fxButton = function(unit, effect, param, opts) {
        opts = opts || {};
        if (opts.scheme === undefined && opts.schemeFromState === undefined) {
            print('[k3] fxButton unit=' + unit + ' effect=' + effect +
                  ' param=' + param + ' missing scheme — pass scheme: \'catFX\' (or similar)');
        }
        var key = 'button_parameter' + param;
        return {
            group: '[EffectRack1_EffectUnit' + unit + '_Effect' + effect + ']',
            inKey: key, outKey: key,
            colorScheme: XoneK3.builders._fxCs(opts)
        };
    };

    XoneK3.builders.fxHeadphone = function(unit, opts) {
        opts = opts || {};
        if (opts.scheme === undefined && opts.schemeFromState === undefined) {
            print('[k3] fxHeadphone unit=' + unit +
                  ' missing scheme — pass scheme: \'catFX\' (or similar)');
        }
        return {
            group: '[EffectRack1_EffectUnit' + unit + ']',
            inKey: 'group_[Headphone]_enable',
            outKey: 'group_[Headphone]_enable',
            colorScheme: XoneK3.builders._fxCs(opts)
        };
    };

    XoneK3.builders.fxGroupEnable = function(unit, deck, opts) {
        opts = opts || {};
        if (opts.scheme === undefined && opts.schemeFromState === undefined) {
            print('[k3] fxGroupEnable unit=' + unit + ' deck=' + deck +
                  ' missing scheme — pass scheme: \'catFX\' (or similar)');
        }
        var key = 'group_' + deck + '_enable';
        return {
            group: '[EffectRack1_EffectUnit' + unit + ']',
            inKey: key, outKey: key,
            colorScheme: XoneK3.builders._fxCs(opts)
        };
    };

    XoneK3.builders.fxMix = function(unit) {
        return {
            group: '[EffectRack1_EffectUnit' + unit + ']',
            inKey: 'mix'
        };
    };

    XoneK3.builders.qfxKnob = function(deck) {
        return {
            group: '[QuickEffectRack1_' + deck + ']',
            inKey: 'super1'
        };
    };

    // ============================================================
    // FUNCS
    // ============================================================
    XoneK3.FUNCS = {
        // ----------------------------------------------------------
        // Select / focus
        // ----------------------------------------------------------
        k3Select: function(args, ch, c, v, s) {
            if (!this._ctrl._isPress(v, s)) return;
            var kind = args[0], which = args[1], val = parseInt(args[2], 10);
            if (isNaN(val)) return;
            var ctrl = this._ctrl;
            if (kind === 'deck' && _loadSelectedOntoChannel(ctrl, val)) return;
            ctrl.setSelectedFor(kind, which, val);
        },
        k3SelectActive: function(args, ch, c, v, s) {
            if (!this._ctrl._isPress(v, s)) return;
            var kind = args[0];
            var val  = parseInt(args[1], 10);
            if (isNaN(val)) return;
            var ctrl = this._ctrl;
            if (kind === 'deck' && _loadSelectedOntoChannel(ctrl, val)) return;
            var slot = (kind === 'fx') ? ctrl.activeFx : ctrl.activeDeck;
            ctrl.setSelectedFor(kind, slot, val);
        },
        setActiveFxTo: function(args, ch, c, v, s) {
            if (!this._ctrl._isPress(v, s)) return;
            this._ctrl.setActiveFx(args[0]);
        },
        toggleActiveFx: function(args, ch, c, v, s) {
            if (!this._ctrl._isPress(v, s)) return;
            this._ctrl.setActiveFx(this._ctrl.activeFx === 'A' ? 'B' : 'A');
        },
        setActiveDeckTo: function(args, ch, c, v, s) {
            if (!this._ctrl._isPress(v, s)) return;
            var ctrl = this._ctrl;
            var which = args[0];
            if (which !== 'A' && which !== 'B') return;
            var channel = (which === 'A') ? ctrl.selectedDeckA : ctrl.selectedDeckB;
            if (_loadSelectedOntoChannel(ctrl, channel)) return;
            ctrl.setActiveDeck(which);
        },
        toggleActiveDeck: function(args, ch, c, v, s) {
            if (!this._ctrl._isPress(v, s)) return;
            this._ctrl.setActiveDeck(this._ctrl.activeDeck === 'A' ? 'B' : 'A');
        },

        // ----------------------------------------------------------
        // Layer toggle
        // ----------------------------------------------------------
        // Layer swap. Chord entry/exit is driven by the modifier
        // system — layer's press-time check is just "did chord
        // already take over?". Releases are handled by the
        // transaction model; this func never runs on release.
        layerButton: function(args, ch, c, v, s) {
            if (!this._ctrl._isPress(v, s)) return;
            if (this._ctrl.modifiers.chord) return;
            this._ctrl.replacePatch(args[0], args[1]);
        },

        // ----------------------------------------------------------
        // Overlays
        // ----------------------------------------------------------
        toggleOverlay: function(args, ch, c, v, s) {
            if (!this._ctrl._isPress(v, s)) return;
            var name = args[0];
            if (!name) return;
            var ctrl = this._ctrl;
            if (ctrl.activePatches.indexOf(name) !== -1) {
                ctrl._batchPatches(function(patches) {
                    var idx = patches.indexOf(name);
                    if (idx !== -1) patches.splice(idx, 1);
                });
                XoneK3.debugPrint('overlayOff', { name: name });
                return;
            }
            var siblings = Array.isArray(args[1]) ? args[1] : [];
            _addOverlayWithSiblingClear(ctrl, name, siblings);
            XoneK3.debugPrint('overlayOn', { name: name });
        },
        layerExitOverlay: function(args, ch, c, v, s) {
            if (!this._ctrl._isPress(v, s)) return;
            this._ctrl.removePatch(args[0]);
        },

        // Layer-aware overlay toggle. Picks the target name from
        // args[0] (cue layer) or args[1] (loop layer). Each overlay
        // toggles independently — both can sit in activePatches,
        // _host gates which is visible. No-op outside cue / loop.
        toggleLayerOverlay: function(args, ch, c, v, s) {
            if (!this._ctrl._isPress(v, s)) return;
            var ctrl = this._ctrl;
            var name = null;
            if      (ctrl._patchActive('cue'))  name = args[0];
            else if (ctrl._patchActive('loop')) name = args[1];
            if (!name) return;
            if (ctrl.activePatches.indexOf(name) !== -1) {
                ctrl.removePatch(name);
            } else {
                ctrl.addPatch(name);
            }
        },

        // Activate hotcue N on press; if unset, also write
        // PHRASE_MARKER_COLORS[0] as its colour. Order matters —
        // hotcue_X_color is ignored when the hotcue doesn't exist.
        hotcueSetPhraseColor: function(args, ch, c, v, s) {
            var n = parseInt(args[0], 10);
            if (isNaN(n) || n < 1) return;
            var ctrl = this._ctrl;
            if (!ctrl._isPress(v, s)) {
                engine.setValue(this.group, 'hotcue_' + n + '_activate', 0);
                return;
            }
            var markers = XoneK3.PHRASE_MARKER_COLORS;
            var color = (Array.isArray(markers) && markers.length > 0)
                ? markers[0] : null;
            var wasSet = engine.getValue(this.group, 'hotcue_' + n + '_status') > 0;
            engine.setValue(this.group, 'hotcue_' + n + '_activate', 1);
            if (!wasSet && color !== null) {
                engine.setValue(this.group, 'hotcue_' + n + '_color', color);
            }
        },

        // ----------------------------------------------------------
        // Top-half mode swap
        // ----------------------------------------------------------
        toggleTopMode: function(args, ch, c, v, s) {
            if (!this._ctrl._isPress(v, s)) return;
            var mode = this._ctrl.currentTopMode();
            if (mode === 'focus') this._ctrl.setTopMode('twoDeck');
            else                  this._ctrl.setTopMode('focus');
        },
        toggleMiniFx: function(args, ch, c, v, s) {
            if (!this._ctrl._isPress(v, s)) return;
            var mode = this._ctrl.currentTopMode();
            if      (mode === 'twoDeck')     this._ctrl.setTopMode('twoDeckMini');
            else if (mode === 'twoDeckMini') this._ctrl.setTopMode('twoDeck');
            // focus: no-op
        },

        // ----------------------------------------------------------
        // Slip toggle + load arm
        // ----------------------------------------------------------
        // Slip toggle, unless the track-load arm is held — then
        // the press loads onto the active deck's pinned channel.
        // Action fires on press, release is a no-op.
        slipOrLoad: function(args, ch, c, v, s) {
            if (!this._ctrl._isPress(v, s)) return;
            var ctrl = this._ctrl;
            if (_loadSelectedOntoChannel(ctrl, ctrl.selectedDeckCurrent)) return;
            var cur = engine.getValue(this.group, 'slip_enabled');
            engine.setValue(this.group, 'slip_enabled', cur ? 0 : 1);
        },

        // ----------------------------------------------------------
        // Encoders
        // ----------------------------------------------------------
        encPregainStep: function(args, ch, c, v, s) {
            var dir = (v === 1) ? 1 : -1;
            var step = XoneK3.parseNum(args[0]); if (isNaN(step)) step = 0.025;
            engine.setValue(this.group, this.inKey,
                            engine.getValue(this.group, this.inKey) + dir * step);
        },
        // BPM step, or head-mix step while modifiers.bpmPressHeld is
        // active (pressL held). The two-step behaviour is driven by
        // the modifier state, not by a per-encoder flag — pressL's
        // transaction is the single source of truth, so rebinds
        // and patch changes mid-hold don't leave stale state.
        //
        // Sets _bpmHeldTurned so F.bpmPress can suppress the shift+
        // release headSplit toggle when the user actually turned
        // the encoder during the hold.
        encBpmOrHeadMix: function(args, ch, c, v, s) {
            var dir = (v === 1) ? 1 : -1;
            var bpmStep = XoneK3.parseNum(args[0]); if (isNaN(bpmStep)) bpmStep = 0.1;
            var mixStep = XoneK3.parseNum(args[1]); if (isNaN(mixStep)) mixStep = 0.05;
            if (this._ctrl.modifiers.bpmPressHeld) {
                this._ctrl._bpmHeldTurned = true;
                engine.setValue('[Master]', 'headMix',
                                engine.getValue('[Master]', 'headMix') + dir * mixStep);
            } else {
                engine.setValue(this.group, this.inKey,
                                engine.getValue(this.group, this.inKey) + dir * bpmStep);
            }
        },

        // pressL release handler for the pressL+encoderL pair.
        //
        // On press: arms the pairing by clearing the turn flag —
        // head-mix behaviour comes from modifiers.bpmPressHeld,
        // derived from pressL's transaction.
        //
        // On release: if shift is held AND the encoder was not
        // turned during the hold, toggles [Master] headSplit.
        // Skips the toggle when the transaction is orphaned — a
        // rebind during the hold means the user's context has
        // changed under them, so we don't fire a user-facing side
        // effect from an abandoned press.
        //
        // The active-transaction guard rejects a stray note-off
        // (release with no matching press). During
        // _dispatchCapturedRelease the transaction is still
        // active, so the guard correctly passes on real releases
        // fired through the capture path.
        bpmPress: function(args, ch, c, v, s) {
            if (this._ctrl._isPress(v, s)) {
                this._ctrl._bpmHeldTurned = false;
                return;
            }
            if (!this._transaction || !this._transaction.active) return;
            var turned = this._ctrl._bpmHeldTurned;
            this._ctrl._bpmHeldTurned = false;
            if (this._transaction.orphaned) return;
            if (this._ctrl.modifiers.shift && !turned) {
                var cur = engine.getValue('[Master]', 'headSplit');
                engine.setValue('[Master]', 'headSplit', cur ? 0 : 1);
            }
        },
        encPlaylistNav: function(args, ch, c, v, s) {
            engine.setValue('[Playlist]',
                (v === 1) ? 'SelectNextTrack' : 'SelectPrevTrack', 1);
        },
        encQuickFxCycle: function(args, ch, c, v, s) {
            var grp = this.group;
            var len = XoneK3.QFX_CHAIN_LIST.length;
            if (len === 0) return;
            var cur = Math.floor(engine.getValue(grp, 'loaded_chain_preset'));
            if (cur < 0) cur = 0;                       // -1 = unsaved preset
            var next = (cur + (v === 1 ? 1 : -1) + len) % len;
            engine.setValue(grp, 'loaded_chain_preset', next);
        },
        encKeyShift: function(args, ch, c, v, s) {
            engine.setValue(this.group, 'pitch_adjust',
                engine.getValue(this.group, 'pitch_adjust') + ((v === 1) ? 1 : -1));
        },
        encKeySyncReset: function(args, ch, c, v, s) {
            var dir = (v === 1) ? 1 : -1;
            if (dir > 0) engine.setValue(this.group, 'sync_key', 1);
            else engine.setValue(this.group, 'reset_key', 1);
        },
        encKeySync: function(args, ch, c, v, s) {
            engine.setValue(this.group, 'sync_key', 1);
        },
        encKeyReset: function(args, ch, c, v, s) {
            engine.setValue(this.group, 'reset_key', 1);
        },
        encStepParam: function(args, ch, c, v, s) {
            var group = args[0], key = args[1];
            if (!group || !key) return;
            var dir = (v === 1) ? 1 : -1;
            var step = XoneK3.parseNum(args[2]); if (isNaN(step)) step = 0.02;
            var cur = engine.getParameter(group, key);
            engine.setParameter(group, key, Math.max(0, Math.min(1, cur + dir * step)));
        },

        // ----------------------------------------------------------
        // Loop movement — gated by _beatjumpAllowed when in loop
        // ----------------------------------------------------------
        // Beatjump encoder is loop-only. Outside a loop, the
        // encoder does nothing — no beatjump, no pulse timer.
        // Inside a loop, _beatjumpAllowed's 130ms gap caps the
        // rate well below the 30ms pulse width, so no drops.
        // The grid beatjump buttons (beatjumpButton) are not
        // gated here — they stay usable outside a loop for
        // track navigation.
        encBeatjump: function(args, ch, c, v, s) {
            var chNum = this._ctrl._channelOf(this.group);
            if (!chNum) return;
            var loopOn = engine.getValue('[Channel' + chNum + ']', 'loop_enabled') > 0;
            if (!loopOn) return;
            if (!this._ctrl._beatjumpAllowed(this.group, loopOn)) return;
            var co = (v === 1) ? 'beatjump_forward' : 'beatjump_backward';
            this._ctrl._pulseCo(this.group, co);
        },
        beatjumpButton: function(args, ch, c, v, s) {
            if (!this._ctrl._isPress(v, s)) return;
            if (!this._ctrl._beatjumpAllowed(this.group)) return;
            this._ctrl._pulseCo(this.group, args[0]);
        },
        reloopToggleResetAnchor: function(args, ch, c, v, s) {
            if (!this._ctrl._isPress(v, s)) return;
            engine.setValue(this.group, 'loop_anchor', 0);
            this._ctrl._pulseCo(this.group, 'reloop_toggle');
            this._ctrl._touchLoop(this.group);
        },
        beatloopActivateResetAnchor: function(args, ch, c, v, s) {
            if (!this._ctrl._isPress(v, s)) return;
            engine.setValue(this.group, 'loop_anchor', 0);
            this._ctrl._pulseCo(this.group, 'beatloop_activate');
            this._ctrl._touchLoop(this.group);
        },

        // Single-boundary move. "anchor" names the side that MOVES —
        // the opposite side stays put via loop_anchor.
        encLoopMove: function(args, ch, c, v, s) {
            var inSide = (args[0] === 'in');
            var step = XoneK3.parseNum(args[1]); if (isNaN(step)) step = 0.010;
            var dir = (v === 1) ? -1 : 1;
            var sign = inSide ? 1 : -1;
            var wantAnchor = inSide ? 1 : 0;
            if (engine.getValue(this.group, 'loop_anchor') !== wantAnchor)
                engine.setValue(this.group, 'loop_anchor', wantAnchor);
            engine.setValue(this.group, this.inKey,
                engine.getValue(this.group, this.inKey) + sign * dir * step);
            this._ctrl._touchLoop(this.group);
        },

        // ----------------------------------------------------------
        // Value setters
        // ----------------------------------------------------------
        pressSetValue: function(args, ch, c, v, s) {
            if (!this._ctrl._isPress(v, s)) return;
            engine.setValue(this.group, this.inKey, XoneK3.parseNum(args[0]));
            if (this.inKey === 'beatloop_size') this._ctrl._touchLoop(this.group);
        },
        encBeatjumpStepScale: function(args, ch, c, v, s) {
            var min = XoneK3.parseNum(args[0]); if (isNaN(min)) min = XoneK3.BEATJUMP_MIN;
            var max = XoneK3.parseNum(args[1]); if (isNaN(max)) max = XoneK3.BEATJUMP_MAX;
            var forward = (v === 1) ? -1 : 1;
            var cur = engine.getValue(this.group, 'beatjump_size');
            if (forward > 0) { if (cur <= min) return; engine.setValue(this.group, 'beatjump_size_halve', 1); }
            else { if (cur >= max) return; engine.setValue(this.group, 'beatjump_size_double', 1); }
        },

        // ----------------------------------------------------------
        // Shutdown
        // ----------------------------------------------------------
        shutdownHold: function(args, ch, c, v, s) {
            if (this._ctrl._shuttingDown) return;
            var self = this;
            if (this._ctrl._isPress(v, s)) {
                if (self._shutdownHoldTimer) {
                    engine.stopTimer(self._shutdownHoldTimer);
                    self._shutdownHoldTimer = null;
                }
                self._shutdownHoldTimer = engine.beginTimer(2000, function() {
                    self._shutdownHoldTimer = null;
                    print('Xone K3: user shutdown');
                    self._ctrl.shutdown();
                }, true);
            } else {
                if (self._shutdownHoldTimer) {
                    engine.stopTimer(self._shutdownHoldTimer);
                    self._shutdownHoldTimer = null;
                }
            }
        },

        // ----------------------------------------------------------
        // Timelines
        // ----------------------------------------------------------
        cycleDisplaySource: function(args, ch, c, v, s) {
            if (!this._ctrl._isPress(v, s)) return;
            var chNum = this._ctrl._channelOf(this.group);
            if (!chNum) return;
            var ctrl = this._ctrl;
            var st = ctrl._decks[chNum];

            if (!st.loopClock.running) return;

            st.displaySource = (st.displaySource === 'loop') ? 'track' : 'loop';
            if (ctrl.selectedDeckCurrent === chNum) {
                ctrl._refreshDisplayFlags();
            }
            ctrl._syncBeatCount(chNum);
            ctrl._refreshAllLeds('display-cycle');
        }
    };
})();
