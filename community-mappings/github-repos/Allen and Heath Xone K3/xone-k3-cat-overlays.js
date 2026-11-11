// xone-k3-cat-overlays.js — Cat's two routing overlays and the
// chord.
//
// Shift companions are shallow clones of their base, so grid
// cells don't get shadowed by cueShift / loopShift when shift
// is held. Chord sits on top of everything — see _buildStack.
//
// Chord patch is added/removed by the modifier system
// (modifierStates.chord with patchActivate: 'chord'). The CHORD
// patch itself declares no `layer` binding — layer's press does
// nothing under chord (F.layerButton returns early when
// modifiers.chord is true).

'use strict';

(function() {
    if (typeof XoneK3 === 'undefined') {
        throw new Error('Xone K3: <filename> — xone-k3-lib-core.js must load first');
    }
    if (typeof XoneK3._requireLoad !== 'function') {
        throw new Error('Xone K3: <filename> — xone-k3-lib-core.js must load first');
    }
    XoneK3._requireLoad('xone-k3-cat-overlays.js',
        ['FUNCS', 'builders', 'CAT']);

    var F = XoneK3.FUNCS;
    var CAT = XoneK3.CAT;

    // ============================================================
    // CELL EMITTERS
    // ============================================================
    function deckSelect(slot, n) {
        return {
            func: F.k3Select, args: ['deck', slot, n],
            colorScheme: {
                scheme: function() { return CAT.themeScheme('deck', n); },
                on: { state: 'selectedDeck' + slot, equals: n }
            }
        };
    }

    function fxSelect(slot, n) {
        return {
            func: F.k3Select, args: ['fx', slot, n],
            colorScheme: {
                scheme: function() { return CAT.themeScheme('fx', n); },
                on: { state: 'selectedFx' + slot, equals: n }
            }
        };
    }

    function routeTo(unit, ch) {
        var key = 'group_[Channel' + ch + ']_enable';
        return {
            group: '[EffectRack1_EffectUnit' + unit + ']',
            inKey: key, outKey: key,
            // themeScheme content-keys the slice so an in-place
            // THEME.fx mutation propagates on the next repaint
            // instead of getting short-circuited by the scheme
            // ref-identity memo in _evaluateScheme.
            colorScheme: {
                scheme: function() { return CAT.themeScheme('fx', unit); }
            }
        };
    }

    // ============================================================
    // SELECT_AB — deck / FX assignment grid
    // ============================================================
    var SELECT_AB = {
        _host: ['twoDeck', 'twoDeckMini', 'topShared'],
        layer: { func: F.layerExitOverlay, args: ['selectAB'],
                 colorScheme: { scheme: 'overlayWhite' } },
        // pressR inherited from CUE — modifier-holder for loadArm.

        grid0_0: deckSelect('A', 1), grid1_0: deckSelect('A', 2),
        grid2_0: deckSelect('A', 3), grid3_0: deckSelect('A', 4),
        grid0_1: deckSelect('B', 1), grid1_1: deckSelect('B', 2),
        grid2_1: deckSelect('B', 3), grid3_1: deckSelect('B', 4),

        grid0_2: fxSelect('A', 1),   grid1_2: fxSelect('A', 2),
        grid2_2: fxSelect('A', 3),   grid3_2: fxSelect('A', 4),
        grid0_3: fxSelect('B', 1),   grid1_3: fxSelect('B', 2),
        grid2_3: fxSelect('B', 3),   grid3_3: fxSelect('B', 4)
    };

    // ============================================================
    // FX_ROUTING — FX unit group enables
    // ============================================================
    var FX_ROUTING = {
        _host: ['twoDeck', 'twoDeckMini', 'topShared'],
        layer: { func: F.layerExitOverlay, args: ['fxRouting'],
                 colorScheme: { scheme: 'overlayWhite' } },

        grid0_0: routeTo(1, 1), grid1_0: routeTo(1, 2),
        grid2_0: routeTo(1, 3), grid3_0: routeTo(1, 4),
        grid0_1: routeTo(2, 1), grid1_1: routeTo(2, 2),
        grid2_1: routeTo(2, 3), grid3_1: routeTo(2, 4),
        grid0_2: routeTo(3, 1), grid1_2: routeTo(3, 2),
        grid2_2: routeTo(3, 3), grid3_2: routeTo(3, 4),
        grid0_3: routeTo(4, 1), grid1_3: routeTo(4, 2),
        grid2_3: routeTo(4, 3), grid3_3: routeTo(4, 4)
    };

    // Shift companions are shallow clones so their grid cells
    // don't get shadowed by cueShift / loopShift when shift is held.
    var SELECT_AB_SHIFT  = Object.assign({}, SELECT_AB);
    var FX_ROUTING_SHIFT = Object.assign({}, FX_ROUTING);
    SELECT_AB._shift  = 'selectABShift';
    FX_ROUTING._shift = 'fxRoutingShift';

    // ============================================================
    // CHORD
    // ============================================================
    // Added / removed by modifiers.chord (see bootCat's
    // modifierStates). No `layer` binding — layer's press is a
    // no-op under chord (F.layerButton returns early).
    var CHORD = {
        press0: { func: F.toggleOverlay, args: ['fxRouting', ['selectAB']],
                  colorScheme: { scheme: 'overlayChord' } },
        press1: { func: F.toggleOverlay, args: ['selectAB', ['fxRouting']],
                  colorScheme: { scheme: 'overlayChord' } },
        press3: { func: F.shutdownHold },
        pressR: { func: F.toggleTopMode,
                  colorScheme: { scheme: 'overlayChord',
                                 on: { patchActive: ['twoDeck', 'twoDeckMini'] } } },
        pressL: { func: F.toggleMiniFx,
                  colorScheme: { scheme: 'overlayChord',
                                 on: { patchActive: 'twoDeckMini' } } }
    };

    // ============================================================
    // EXPORTS
    // ============================================================
    CAT.SELECT_AB = SELECT_AB;
    CAT.SELECT_AB_SHIFT = SELECT_AB_SHIFT;
    CAT.FX_ROUTING = FX_ROUTING;
    CAT.FX_ROUTING_SHIFT = FX_ROUTING_SHIFT;
    CAT.CHORD = CHORD;
})();