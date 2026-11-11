// xone-k3-cat-schemes.js — Cat's THEME, QFX chain list,
// phrase-marker colours, colour ramps, and the scheme-builder
// helpers that the top / bottom / overlay files consume.
//
// Loads after lib-funcs.js. Registers on XoneK3.CAT so later cat
// files pull helpers by name.

'use strict';

(function() {
    if (typeof XoneK3 === 'undefined') {
        throw new Error('Xone K3: <filename> — xone-k3-lib-core.js must load first');
    }
    if (typeof XoneK3._requireLoad !== 'function') {
        throw new Error('Xone K3: xone-k3-cat-schemes.js — xone-k3-lib-core.js must load first');
    }
    XoneK3._requireLoad('xone-k3-cat-schemes.js',
        ['FUNCS', 'builders', 'SCHEMES', 'SCHEME_DEFAULTS']);

    var B = XoneK3.builders;
    var CAT = XoneK3.CAT = XoneK3.CAT || {};

    // ============================================================
    // THEME — deck and FX number colours.
    //
    // Emitters read THEME via deferred function schemes, so a
    // THEME mutation at runtime propagates on the next repaint.
    // ============================================================
    XoneK3.THEME = {
        deck: { 1: 'green, 127', 2: 'red, 93', 3: [24, 24, 127, 127], 4: [127, 127, 32, 110] },
        fx:   { 1: 'magenta, 115', 2: 'yellow, 115', 3: 'cyan, 110', 4: 'orange, 127' }
    };

    // ============================================================
    // QFX_CHAIN_LIST — must match Preferences → Effects →
    // Quick Effect Chains. Indexed (0-based) from
    // loaded_chain_preset.
    // ============================================================
    XoneK3.QFX_CHAIN_LIST = [
        'filterNone',
        'filterReverb', 'filterSmoothGrowl', 'filterEcho', 'filterFlanger',
        'filterMoog', 'filterBalance'
    ];

    // ============================================================
    // PHRASE_MARKER_COLORS — hotcue colours that mark phrase starts.
    //
    // Origin derives positionally: the last marker at or before
    // the playhead, or — before the first marker — the next one
    // ahead (negative count reaching zero, which is why _beatF
    // has no pos<0 guard). Frozen during an active loop; re-derived
    // on the first tick after it stops.
    //
    // Place markers on beat boundaries — a marker mid-beat floors
    // to the start of its beat.
    //
    // Only hotcues listed in XoneK3.PHRASE_HOTCUE_SLOTS are
    // scanned (default [9, 10, 11, 12] — the hotcue2 overlay's
    // top row; set before boot). Values are the integer RGB hexes
    // Mixxx stores on [ChannelN],hotcue_X_color. Empty list =
    // feature off.
    // ============================================================
    XoneK3.PHRASE_MARKER_COLORS = [0xF2F2FF];

    // Hotcue slots scanned for phrase-marker colours. Default is
    // [9, 10, 11, 12] (the hotcue2 overlay's top row). Change
    // to any 1..32 slots; duplicates and out-of-range entries are
    // dropped with a warn-once at boot. Empty list = feature off.
    //
    // XoneK3.PHRASE_HOTCUE_SLOTS = [9, 10, 11, 12];

    // ============================================================
    // SCHEMES — Cat's colour ramps.
    // ============================================================
    function countdownVariant(deck, beatColor, barColor) {
        return {
            colors: [[0, 0, 0]],
            beat: {
                color: beatColor,
                barBeats: 4,
                phraseBars: 4,
                prePhraseBeats: 4,
                half: false,
                bar:       { color: barColor },
                phrase:    { color: 'white, 127' },
                prePhrase: { color: 'magenta, 127' },
                deck: deck
            }
        };
    }

    Object.assign(XoneK3.SCHEMES, {
        myCatnip: {
            colors: [[28,28,127], [28,28,69], [28,69,28], [28,127,28]],
            curve: -0.7,
            markerColor: [127, 0, 0, 93],
            markerWidth: 2
        },
        myCatnipSpread: {
            colors: [[28,28,127], [28,28,69], [28,69,28], [28,127,28]],
            curve: 0.3,
            markerColor: [127, 0, 0, 93],
            markerWidth: 2
        },
        catFX: {
            colors: [[28,28,127], [28,28,69], [28,69,28], [28,127,28]],
            curve: -0.7,
            markerColor: [127, 0, 0, 93],
            markerWidth: 2
        },
// QFX scheme colours — avoid cyan / magenta / yellow / orange
// (reserved for other UI).
        filterNone:        { colors: ['off'], coScheme: null },
        filterReverb:       { 
                            colors: [
                                [1, 0, 0],
                                [30, 0, 0],
                                [110, 0, 0]
                            ],
                            calibration: false,
                            curve: 0.3,
                            markerColor: [110, 0, 0],
                            markerPosition: 0,
                            markerWidth: 2
                            },   
        filterSmoothGrowl: { 
                            colors: [
                                [0, 1, 0],
                                [0, 30, 0],
                                [0, 103, 0]
                            ],
                            calibration: false,
                            curve: 0.3,
                            markerColor: [0, 127, 0],
                            markerPosition: 0,
                            markerWidth: 2
                            },        
        filterEcho:        { 
                            colors: [
                                [0, 0, 1],
                                [0, 0, 30],
                                [0, 0, 103]
                            ],
                            calibration: false,
                            curve: 0.3,
                            markerColor: [0, 0, 127],
                            markerPosition: 0,
                            markerWidth: 2
                            },
        filterFlanger:     {
                            colors: [
                                [2, 1, 0],
                                [30, 25, 0],
                                [127, 110, 0]
                            ],
                            calibration: false,
                            curve: 0.2,
                            markerColor: [127, 127, 0],
                            markerPosition: 0,
                            markerWidth: 2
                            },
        filterMoog:        {
                            colors: [
                                [127, 40, 19],
                                [35, 10, 5],
                                [3, 1, 1],
                                [35, 10, 5],
                                [127, 40, 19]
                            ],
                            calibration: false,
                            curve: 0.2,
                            markerColor: [127, 40, 19],
                            markerWidth: 2
                            },   
        filterBalance:     {
                            colors: [
                                [110, 110, 110],
                                [30, 20, 30],
                                [2, 1, 1],
                                [30, 20, 30],
                                [110, 110, 110]
                            ],
                            calibration: false,
                            curve: 0.2,
                            markerColor: [110, 110, 110],
                            markerWidth: 2 
                            },
        syncOn:    { colors: ['amber, 80'] },
        keylockOn: { colors: ['white, 64'] },
        overlayWhite: { colors: ['white, 60, 0'] },
        overlayChord: { colors: ['white, 80'] },

        // Track-end countdown. Four phases per deck family, keyed
        // on bars remaining. See countdownScheme() below.
        countdownIdle: { colors: [[0, 0, 0]] },

        countdownA1: countdownVariant('$DECK_A$', 'green',  'cyan'),
        countdownA2: countdownVariant('$DECK_A$', 'yellow', 'cyan'),
        countdownA3: countdownVariant('$DECK_A$', 'amber',  'cyan'),
        countdownA4: countdownVariant('$DECK_A$', 'red',    'yellow'),

        countdownB1: countdownVariant('$DECK_B$', 'green',  'cyan'),
        countdownB2: countdownVariant('$DECK_B$', 'yellow', 'cyan'),
        countdownB3: countdownVariant('$DECK_B$', 'amber',  'cyan'),
        countdownB4: countdownVariant('$DECK_B$', 'red',    'yellow')
    });

    // ============================================================
    // SCHEME HELPERS
    // ============================================================
    // Deferred read of THEME.fx[n]. Called on every render of any
    // binding that references it, so a THEME.fx mutation propagates
    // on the next repaint. Memoized by the current THEME value so a
    // series of renders with no THEME change returns the same
    // composed object — the binding's ref-identity memo then skips
    // the recompile.
    var _fxSchemeCache = {};
    var _fxSchemeKey = {};

    function fxScheme(n) {
        // Key on array *contents*, not reference. THEME.fx[n] may
        // be a string or an array; reference equality misses an
        // in-place array mutation (THEME.fx[3][0] = 100) and would
        // return the stale composed scheme. String entries pass
        // through unchanged — join('|') is only paid on arrays.
        var entry = XoneK3.THEME.fx[n];
        var key = Array.isArray(entry) ? entry.join('|') : entry;
        if (_fxSchemeKey[n] === key && _fxSchemeCache[n]) {
            return _fxSchemeCache[n];
        }
        var composed = B.composeScheme({
            scheme: 'catFX',
            markerColor: entry
        });
        _fxSchemeCache[n] = composed;
        _fxSchemeKey[n] = key;
        return composed;
    }

    // Content-keyed raw THEME lookup. Emitters that hand the THEME
    // value straight to the binding as a colour spec (slipScheme,
    // activeSlotButton, deckSelect, fxSelect, FX_COLOR_MAP) would
    // otherwise return the same array reference after an in-place
    // mutation (THEME.deck[3][0] = 100), so the binding's
    // ref-identity memo (_evaluateScheme) would skip recompile and
    // leave the LED stale. Content-keying returns a fresh slice per
    // content change, matching what fxScheme already does for
    // composed FX schemes.
    var _themeSchemeCache = { deck: {}, fx: {} };
    var _themeSchemeKey   = { deck: {}, fx: {} };
    function themeScheme(kind, n) {
        var entry = XoneK3.THEME[kind][n];
        if (typeof entry === 'string') return entry;
        if (!Array.isArray(entry)) return entry;
        var key = entry.join('|');
        if (_themeSchemeKey[kind][n] === key && _themeSchemeCache[kind][n]) {
            return _themeSchemeCache[kind][n];
        }
        _themeSchemeCache[kind][n] = entry.slice();
        _themeSchemeKey[kind][n]   = key;
        return _themeSchemeCache[kind][n];
    }

    // Deferred map — entries are functions, not baked values. The
    // desugared schemeFromState closure calls them per render, so a
    // THEME mutation lands on the next repaint.
    function _deferredMap(mk) {
        var out = {};
        [1, 2, 3, 4].forEach(function(n) {
            out[n] = function() { return mk(n); };
        });
        return out;
    }

    var FX_SCHEME_MAP = _deferredMap(fxScheme);
    var FX_COLOR_MAP  = _deferredMap(function(n) { return themeScheme('fx', n); });

    // No base scheme — the schemeFromState closure is the whole
    // scheme, drawn from the deferred map above.
    function fxSchemeFor(stateProp) {
        return {
            schemeFromState: { state: stateProp, map: FX_SCHEME_MAP }
        };
    }

    // Live scheme function for a track-end countdown. Re-invoked on
    // every render of the bound LED, including every beat edge.
    //
    // The A/B suffix is derived from the binding's declared group
    // token. `$DECK_A$` in group → countdownA*, `$DECK_B$` →
    // countdownB*. `$DECK_ACTIVE$` reads the controller's current
    // activeDeck each render, so the countdown follows focus.
    //
    // Phases measured in bars (4 beats each) remaining:
    //   > 64   phase1 (green)
    //   > 32   phase2 (yellow)
    //   > 16   phase3 (amber)
    //   <= 16  phase4 (red)
    //
    // Idle (black) when the deck isn't playing, the tempo is
    // unknown, or the binding's group doesn't name a known slot.
    function countdownScheme() {
        return function() {
            var grp = this.group;
            if (!grp || grp.indexOf('[Channel') !== 0) return 'countdownIdle';

            var ctrl = this._ctrl;
            if (!ctrl) return 'countdownIdle';

            var rawGroup = this._rawCfg && this._rawCfg.group;
            var suffix = null;
            if (typeof rawGroup === 'string') {
                if      (rawGroup.indexOf('$DECK_A$') !== -1) suffix = 'A';
                else if (rawGroup.indexOf('$DECK_B$') !== -1) suffix = 'B';
                else if (rawGroup.indexOf('$DECK_ACTIVE$') !== -1) suffix = ctrl.activeDeck;
            }
            if (!suffix) return 'countdownIdle';

            var ch = ctrl._channelOf(grp);
            if (!ch) return 'countdownIdle';

            // Duration and file_bpm are cached per channel — both
            // are fixed for a loaded track and only change on
            // track_loaded. Content bounds are read fresh; markers
            // can move mid-playback.
            var meta = ctrl._trackMeta(ch);
            var dur = meta.duration;
            var bpmFile = meta.fileBpm;
            if (!(dur > 0)) return 'countdownIdle';
            if (!bpmFile) return 'countdownIdle';

            var bounds = ctrl._getContentBounds(ch);
            if (!bounds) return 'countdownIdle';

            // st.lastPlayPos is set on every playposition fire (before
            // the throttle gate), so it's accurate to the most recent
            // tick — 30ms granularity against a bar-scale countdown.
            // Fall back to a live read only before the first tick after
            // load / reset, when lastPlayPos is still undefined.
            var st = ctrl._decks[ch];
            var pp = (st && st.lastPlayPos !== undefined)
                ? st.lastPlayPos
                : engine.getValue(grp, 'playposition');
            var remainingNorm = bounds.end - pp;
            if (remainingNorm < 0) remainingNorm = 0;

            // Bar length comes from TIME_SIG when set (same priority
            // _resolveSig uses for the beat cascade), else the
            // countdown schemes' baked 4. Without this, a 3/4 or
            // 7/8 time signature puts the cascade and the countdown
            // out of sync — the cascade counts in 3-beat bars while
            // the countdown still treats 4 beats as a bar.
            var barBeats = (XoneK3.TIME_SIG && XoneK3.TIME_SIG.barBeats > 0)
                ? XoneK3.TIME_SIG.barBeats
                : 4;

            var beatsLeft = remainingNorm * dur * bpmFile / 60;
            var barsLeft  = beatsLeft / barBeats;

            if (barsLeft <= 16) return 'countdown' + suffix + '4';
            if (barsLeft <= 32) return 'countdown' + suffix + '3';
            if (barsLeft <= 64) return 'countdown' + suffix + '2';
            return 'countdown' + suffix + '1';
        };
    }

    // Slip / toggle-active-deck button LED. Colour is the deck
    // number sitting in the currently-active slot, read from
    // THEME.deck. The on/off brightness follows slip_enabled via
    // the binding's outKey — do not set on:, that would override
    // slip state.
    //
    // Keyed on selectedDeckCurrent (channel 1..4) rather than
    // activeDeck ('A'/'B'), so the LED shows the theme colour of
    // whichever channel is actually loaded into the active slot.
    // A user who pins channel 3 into slot A sees the deck-3 colour
    // on the slip button, not slot A's abstract magenta.
    //
    // setSelectedFor / setActiveDeck both fire _reapplyByTokens,
    // and _reapplyByTokens refreshes the LED when the resolved
    // target changes. The state read is live (readPath), so a
    // channel swap on the active slot repaints the slip button in
    // the new deck's colour.
    function slipScheme() {
        return {
            schemeFromState: {
                state: 'selectedDeckCurrent',
                map: {
                    1: function() { return themeScheme('deck', 1); },
                    2: function() { return themeScheme('deck', 2); },
                    3: function() { return themeScheme('deck', 3); },
                    4: function() { return themeScheme('deck', 4); }
                }
            }
        };
    }

    // Play-button LED. On/off follows play_indicator via outKey;
    // colour tells the user which timeline the active deck is
    // reading from. Green = track, yellow = loop.
    var PLAY_LED_SCHEME = {
        schemeFromState: {
            state: 'activeDisplaySource',
            map: { track: 'green', loop: 'yellow' }
        }
    };

    // ============================================================
    // EXPORTS
    // ============================================================
    CAT.fxScheme = fxScheme;
    CAT.themeScheme = themeScheme;
    CAT.fxSchemeFor = fxSchemeFor;
    CAT.FX_SCHEME_MAP = FX_SCHEME_MAP;
    CAT.FX_COLOR_MAP = FX_COLOR_MAP;
    CAT.countdownScheme = countdownScheme;
    CAT.slipScheme = slipScheme;
    CAT.PLAY_LED_SCHEME = PLAY_LED_SCHEME;
})();