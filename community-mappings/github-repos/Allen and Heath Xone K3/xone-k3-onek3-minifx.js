// xone-k3-onek3-minifx.js — oneK3 miniFX preset: Cat's layers,
// twoDeckMini boot.
//
// Loads Cat's layer tree verbatim. Boots with twoDeckMini as the
// top half — two pinned decks, one focused FX unit spanning both
// middle columns.
//
// Chord pressL swaps to twoDeck, pressR swaps to focus. Same
// layer tree as Cat, Kitten, and oneK3.

'use strict';

(function() {
    if (typeof XoneK3 === 'undefined') {
        throw new Error('Xone K3: <filename> — xone-k3-lib-core.js must load first');
    }
    
    if (typeof XoneK3._requireLoad !== 'function') {
        throw new Error('Xone K3: xone-k3-onek3-minifx.js — xone-k3-cat.js must load first');
    }
    XoneK3._requireLoad('xone-k3-onek3-minifx.js', ['bootCat', 'CAT_LAYERS']);

    XoneK3.init = function() {
        XoneK3.bootCat({
            label: 'oneK3 miniFX',
            initialPatches: ['twoDeckMini', 'fxMini1', 'cue']
        });
    };
    XoneK3.shutdown = function() { XoneK3.halt(); };
})();