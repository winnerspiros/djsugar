// xone-k3-colorlab.js — Xone K3 colorLab. Mixxx 2.5.
//
// Two modes, switched by the layer button:
//   EDIT  (patch 'colorLab')     segment / marker / ratio / beat edits
//   SLOTS (patch 'slots')        grid previews & saves; edits are no-ops
//
// Overlays declare _host — they only apply while their host patch
// is active. The flag persists in activePatches across switches.
//
// Chord: shift + layer held, then press3 held 2s → shutdown.
// Releasing shift, layer, or press3 kills the timer.
//
// Shift state comes from the modifier system — the shift binding
// is push-mode with no func; modifierStates.shift derives the
// state from its transaction.

'use strict';

(function() {
    if (typeof XoneK3 === 'undefined') {
        throw new Error('Xone K3: <filename> — xone-k3-lib-core.js must load first');
    }

    if (typeof XoneK3._requireLoad !== 'function') {
        throw new Error('Xone K3: xone-k3-colorlab.js — xone-k3-lib-core.js must load first');
    }
    XoneK3._requireLoad('xone-k3-colorlab.js',
    ['FUNCS', 'bootLab', 'LabController']);

    var F = XoneK3.FUNCS;

    // ============================================================
    // TOP — editor, unshifted
    // ============================================================
    var COLORLAB_TOP = {
        // Push-mode modifier-holder. No func — modifierStates.shift
        // derives the state from this binding's transaction. LED
        // tracks modifiers.shift directly.
        shift: { type: 'push',
                 colorScheme: {
                     scheme: { colors: ['red, 80, 16'] },
                     on: { state: 'modifiers.shift' }
                 } },

        encoder0: { func: F.ledRGB, args: ['r'] },
        encoder1: { func: F.ledRGB, args: ['g'] },
        encoder2: { func: F.ledRGB, args: ['b'] },
        encoder3: { func: F.ledRGB, args: ['brightness'] },
        fader0:   { func: F.ledFader, args: ['r'] },
        fader1:   { func: F.ledFader, args: ['g'] },
        fader2:   { func: F.ledFader, args: ['b'] },
        fader3:   { func: F.ledFader, args: ['brightness'] },

        knob0_0: { group: '[QuickEffectRack1_[Channel1]]', inKey: 'super1', func: F.ledLabPreviewWithCO },
        knob1_0: { func: F.ledLabMarkerPosition },
        knob2_0: { func: F.ledLabSchemePosition },
        knob3_0: { func: F.ledLabRatio, args: [0] },
        knob0_1: { func: F.ledLabRatio, args: [1] },
        knob1_1: { func: F.ledLabRatio, args: [2] },
        knob2_1: { func: F.ledLabRatio, args: [3] },
        knob3_1: { func: F.ledLabRatio, args: [4] },
        knob0_2: { func: F.ledLabRatio, args: [5] },
        knob1_2: { func: F.ledLabRatio, args: [6] },
        knob2_2: { func: F.ledLabRatio, args: [7] },
        knob3_2: { func: F.ledLabRatio, args: [8] },

        button0_0: { func: F.labTogglePreview, noLED: true },
        button1_0: { func: F.ledLabReset, args: ['markerPosition', 63.5], colorScheme: { scheme: 'red', on: false } },
        button2_0: { func: F.ledLabReset, args: ['schemePosition', 63.5], colorScheme: { scheme: 'green', on: false } },
        button3_0: { func: F.ledSelectTarget, args: ['button3_0'], noLED: true },
        button0_1: { func: F.ledSelectTarget, args: ['button0_1'], noLED: true },
        button1_1: { func: F.ledSelectTarget, args: ['button1_1'], noLED: true },
        button2_1: { func: F.ledSelectTarget, args: ['button2_1'], noLED: true },
        button3_1: { func: F.ledSelectTarget, args: ['button3_1'], noLED: true },
        button0_2: { func: F.ledSelectTarget, args: ['button0_2'], noLED: true },
        button1_2: { func: F.ledSelectTarget, args: ['button1_2'], noLED: true },
        button2_2: { func: F.ledSelectTarget, args: ['button2_2'], noLED: true },
        button3_2: { func: F.ledSelectTarget, args: ['button3_2'], noLED: true },

        press0: { func: F.ledRGBShow,              colorScheme: { scheme: 'white', on: false } },
        press1: { func: F.ledLabToggleCalibration, noLED: true },
        press2: { func: F.ledLabToggleRatio,       noLED: true },
        press3: { func: F.ledLabToggleInterp,      noLED: true }
    };

    // ============================================================
    // TOP — editor, shifted
    // ============================================================
    var COLORLAB_TOP_SHIFT = {
        encoder0: { func: F.ledLabCurve },
        encoder1: { func: F.ledLabBrightness },
        encoder2: { func: F.ledLabBrightnessScale, args: [0.01] },
        encoder3: { },

        knob0_0: { func: F.ledLabPreview },

        button1_0: { func: F.ledLabReset, args: ['brightness', 127],    colorScheme: { scheme: 'white', on: false } },
        button2_0: { func: F.ledLabReset, args: ['brightnessScale', 1], colorScheme: { scheme: 'white', on: false } },

        press0: { func: F.ledDumpScheme, colorScheme: { scheme: 'cyan', on: false } },
        press1: { func: F.labToggleWb, noLED: true },
        press3: { func: F.toggleOverlay, args: ['colorLabTuning'], noLED: true }
    };
    COLORLAB_TOP._shift = 'colorLabTopShift';

    // ============================================================
    // TUNING OVERLAY
    // ============================================================
    var COLORLAB_TUNING = {
        _host: 'colorLabTop',
        _shift: 'colorLabTuningShift'
    };

    var COLORLAB_TUNING_SHIFT = {
        encoder1: { func: F.ledLabHk,  args: [0.01] },
        encoder2: { func: F.ledLabLum, args: [0.01] }
    };

    // ============================================================
    // BEAT EDIT OVERLAY
    // ============================================================
    var COLORLAB_BEAT = {
        _host: 'colorLab',
        encoderL: { func: F.labBeatBarBeats },
        encoderR: { func: F.labBeatPhraseBars },

        grid0_0: { func: F.labBeatSelect, args: ['beat'],      noLED: true },
        grid1_0: { func: F.labBeatSelect, args: ['bar'],       noLED: true },
        grid2_0: { func: F.labBeatSelect, args: ['prePhrase'], noLED: true },
        grid3_0: { func: F.labBeatSelect, args: ['phrase'],    noLED: true },

        grid0_1: { func: F.labBeatToggleEnabled, noLED: true },

        grid1_1: { },
        grid2_1: { },
        grid3_1: { },
        grid0_2: { },
        grid1_2: { },
        grid2_2: { },
        grid3_2: { },
        grid0_3: { },
        grid1_3: { },
        grid3_3: { }
    };

    var COLORLAB_BEAT_SHIFT = {
        encoderR: { func: F.labBeatPrePhraseBeats },
        grid0_0: { func: F.labBeatClear, args: ['beat'],      noLED: true },
        grid1_0: { func: F.labBeatClear, args: ['bar'],       noLED: true },
        grid2_0: { func: F.labBeatClear, args: ['prePhrase'], noLED: true },
        grid3_0: { func: F.labBeatClear, args: ['phrase'],    noLED: true }
    };
    COLORLAB_BEAT._shift = 'colorLabBeatShift';

    // ============================================================
    // BOTTOM — editor, unshifted
    // ============================================================
    var COLORLAB_BOTTOM = {
        layer: { func: F.layerButton, args: ['colorLab', 'slots'], 
                 type: 'push', colorScheme: { scheme: 'yellow', on: false } },
        encoderL: { func: F.ledLabMarkerWidth },
        pressL:   { func: F.ledFillGaps },

        grid0_0: { func: F.ledSelectTarget, args: ['grid0_0'], noLED: true },
        grid1_0: { func: F.ledSelectTarget, args: ['grid1_0'], noLED: true },
        grid2_0: { func: F.ledSelectTarget, args: ['grid2_0'], noLED: true },
        grid3_0: { func: F.ledSelectTarget, args: ['grid3_0'], noLED: true },
        grid0_1: { func: F.ledSelectTarget, args: ['grid0_1'], noLED: true },
        grid1_1: { func: F.ledSelectTarget, args: ['grid1_1'], noLED: true },
        grid2_1: { func: F.ledSelectTarget, args: ['grid2_1'], noLED: true },
        grid3_1: { func: F.ledSelectTarget, args: ['grid3_1'], noLED: true },
        grid0_2: { func: F.ledSelectTarget, args: ['grid0_2'], noLED: true },
        grid1_2: { func: F.ledSelectTarget, args: ['grid1_2'], noLED: true },
        grid2_2: { func: F.ledSelectTarget, args: ['grid2_2'], noLED: true },
        grid3_2: { func: F.ledSelectTarget, args: ['grid3_2'], noLED: true },
        grid0_3: { func: F.ledSelectTarget, args: ['grid0_3'], noLED: true },
        grid1_3: { func: F.ledSelectTarget, args: ['grid1_3'], noLED: true },
        grid2_3: { func: F.labBeatEditToggle, noLED: true },
        grid3_3: { func: F.ledSelectMarker, noLED: true }
    };

    // ============================================================
    // BOTTOM — editor, shifted
    // ============================================================
    // pressR loads the current preview into edit without leaving
    // the mode. Shutdown lives on the chord (shift+layer+press3).
    var COLORLAB_BOTTOM_SHIFT = {
        encoderL: { func: F.labSchemeScroll },
        pressR:   { func: F.labLoadPreview }
    };
    COLORLAB_BOTTOM._shift = 'colorLabShift';

    // ============================================================
    // BOTTOM — slots mode
    // ============================================================
    function slotBtn(idx) { return { func: F.labSlotPress, args: [idx], noLED: true }; }

    var SLOTS = {
        layer: { func: F.layerButton, args: ['slots', 'colorLab'], type: 'push',
                 colorScheme: { scheme: 'purple', on: { state: '_lab.saveArmed' } } },
        pressL: { func: F.labEditReturn },
        pressR: { func: F.labArmSave },

        grid0_0: slotBtn(0),  grid0_1: slotBtn(1),  grid0_2: slotBtn(2),  grid0_3: slotBtn(3),
        grid1_0: slotBtn(4),  grid1_1: slotBtn(5),  grid1_2: slotBtn(6),  grid1_3: slotBtn(7),
        grid2_0: slotBtn(8),  grid2_1: slotBtn(9),  grid2_2: slotBtn(10), grid2_3: slotBtn(11),
        grid3_0: slotBtn(12), grid3_1: slotBtn(13), grid3_2: slotBtn(14), grid3_3: slotBtn(15)
    };

    var SLOTS_SHIFT = {
        encoderL: { func: F.labSchemeScroll },
        pressR:   { func: F.labEditThis, noLED: true }
    };
    SLOTS._shift = 'slotsShift';

    // ============================================================
    // CHORD
    // ============================================================
    var CHORD = {
        press3: { func: F.shutdownHold }
    };

    // ============================================================
    // BOOT
    // ============================================================
    XoneK3.init = function() {
        XoneK3.bootLab([Object.assign({}, XoneK3.HARDWARE_CALIB, {
            label: 'colorLab',
            midiChannel: 0x0E,

            initialPatches: ['colorLabTop', 'colorLab'],

            patches: {
                colorLabTop:         COLORLAB_TOP,
                colorLabTopShift:    COLORLAB_TOP_SHIFT,
                colorLab:            COLORLAB_BOTTOM,
                colorLabShift:       COLORLAB_BOTTOM_SHIFT,
                slots:               SLOTS,
                slotsShift:          SLOTS_SHIFT,
                colorLabTuning:      COLORLAB_TUNING,
                colorLabTuningShift: COLORLAB_TUNING_SHIFT,
                colorLabBeat:        COLORLAB_BEAT,
                colorLabBeatShift:   COLORLAB_BEAT_SHIFT,
                chord:               CHORD
            },

            modifierStates: {
                shift: { heldBy: ['shift'], recompute: true },
                chord: { heldBy: ['shift', 'layer'], patchActivate: 'chord' }
            },

            labPreviewTarget: 'button0_0',
            markerTarget:     'grid3_3',
            labPreviewCO: { group: '[QuickEffectRack1_[Channel1]]', name: 'super1' },

            ledTargetGroups: { calib: XoneK3.CALIB_TARGETS },

            modePrints:  true,
            debugPrints: true
        })]);
    };
    XoneK3.shutdown = function() { XoneK3.halt(); };
})();