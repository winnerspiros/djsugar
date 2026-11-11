// xone-k3-onek3.js — oneK3 preset: Cat's layers, twoDeck boot.
//
// Loads Cat's layer tree verbatim. Boots with twoDeck as the top
// half instead of topShared + fxLayout<n>. Same physical controls,
// same bottom half, same overlays — only the starting top half
// differs.
//
// Chord pressR swaps to focus mode and back (F.toggleTopMode).

'use strict';

(function() {
    if (typeof XoneK3 === 'undefined') {
        throw new Error('Xone K3: <filename> — xone-k3-lib-core.js must load first');
    }
    
    if (typeof XoneK3._requireLoad !== 'function') {
        throw new Error('Xone K3: xone-k3-onek3.js — xone-k3-cat.js must load first');
    }
    XoneK3._requireLoad('xone-k3-onek3.js', ['bootCat', 'CAT_LAYERS']);

    XoneK3.init = function() {
        XoneK3.bootCat({
            label: 'oneK3',
            initialPatches: ['twoDeck', 'fxLayoutA1', 'fxLayoutB1', 'cue']
        });
    };
    XoneK3.shutdown = function() { XoneK3.halt(); };
})();