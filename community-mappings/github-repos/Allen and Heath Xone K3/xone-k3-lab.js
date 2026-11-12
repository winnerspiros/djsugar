// xone-k3-lab.js — Xone K3 colorLab extension. Mixxx 2.5.
//
// Editor + slot browser for building LED colour schemes.
//
// State (`_lab`):
//   edit           — working buffer. Starts as { colors: [[0,0,0]] }.
//   slots[0..15]   — saved schemes. 0/1/2/3 are on/off/co/co-off.
//   previewSource  — 'edit' | 0..15 | registry-name string.
//   previewing     — true when edit mode should show previewSource
//                    on the strip instead of the edit buffer.
//   saveArmed      — next slot press writes preview into that slot.
//
// Renderers discover which cells they own by scanning the applied
// bindings for known func names — no hardcoded grid-to-meaning
// map. Rebinding a cell in config moves the painter with it.
// See `_findCellsByFunc`.
//
// Beat edit overlay: 'colorLabBeat'. _host = 'colorLab'. Preview
// LED pulses through the cascade when the beat is playing; shows
// the selected target's colour solid otherwise.

'use strict';

(function() {
    if (typeof XoneK3 === 'undefined') {
        throw new Error('Xone K3: <filename> — xone-k3-lib-core.js must load first');
    }

    if (typeof XoneK3._requireLoad !== 'function') {
        throw new Error('Xone K3: xone-k3-lib-core.js must load first');
    }
    XoneK3._requireLoad('xone-k3-lab.js',
        ['FUNCS', 'SCHEMES', 'K3Controller', 'compileScheme', 'resolveScheme',
         'sampleT', 'sampleSegs', 'scaleTarget', 'resolveColorEntry',
         'markerHit', 'BEAT_HOLD', 'CO_MATCH_EPSILON', 'RATIO_GAIN',
         'registerPrintTable', '_funcId']);

    // Curated scroll order for the lab editor. When defined,
    // shift + encoderL iterates this list instead of scanning
    // XoneK3.SCHEMES. Use it to control order and hide test schemes.
    // Requires cat-schemes.js to be loaded before colorlab.js.
    XoneK3.SCHEME_REGISTRY = [
        'myCatnip',
        'myCatnipSpread',
        'catFX',
        'filterNone',
        'filterEcho',
        'filterReverb',
        'filterSmoothGrowl',
        'filterBalance',
        'coMatch',
        'labDefault'
    ];

    XoneK3.LAB_SLOT = {
        ON: 0, OFF: 1, CO: 2, CO_OFF: 3,
        SAVE_START: 4, SAVE_END: 11,
        STRUCTURAL:   12,
        BAKED_INTERP: 13,
        BAKED:        14,
        EXPORT:       15
    };

    // Slot-grid cell colours, keyed by LAB_SLOT index.
    XoneK3.LAB_SLOT_COLOURS = {};
    XoneK3.LAB_SLOT_COLOURS[XoneK3.LAB_SLOT.ON]         = [0, 127, 0];
    XoneK3.LAB_SLOT_COLOURS[XoneK3.LAB_SLOT.OFF]        = [127, 0, 0];
    XoneK3.LAB_SLOT_COLOURS[XoneK3.LAB_SLOT.CO]         = [127, 127, 127];
    XoneK3.LAB_SLOT_COLOURS[XoneK3.LAB_SLOT.CO_OFF]     = [60, 60, 127];
    XoneK3.LAB_SLOT_COLOURS[XoneK3.LAB_SLOT.STRUCTURAL]   = [0, 0, 127];
    XoneK3.LAB_SLOT_COLOURS[XoneK3.LAB_SLOT.BAKED_INTERP] = [0, 127, 127];
    XoneK3.LAB_SLOT_COLOURS[XoneK3.LAB_SLOT.BAKED]        = [127, 0, 127];
    XoneK3.LAB_SLOT_COLOURS[XoneK3.LAB_SLOT.EXPORT]       = [127, 40, 0];

    // Segment-grid cell names, in segment-index order. The
    // renderer paints CALIB_TARGETS[i] with compiled segment i —
    // positional, not by discovery. _findCellsByFunc is used by
    // _claimedCells only, to skip cells another painter (slot
    // grid, beat-edit targets) owns. Rebinding a cell moves the
    // LED but breaks the segment mapping — do not rebind these.
    XoneK3.CALIB_TARGETS = [
        'button3_0',
        'button0_1','button1_1','button2_1','button3_1',
        'button0_2','button1_2','button2_2','button3_2',
        'grid0_0','grid1_0','grid2_0','grid3_0',
        'grid0_1','grid1_1','grid2_1','grid3_1',
        'grid0_2','grid1_2','grid2_2','grid3_2',
        'grid0_3','grid1_3'
    ];

    XoneK3.LAB_DUMP_ON_SHUTDOWN = true;
    XoneK3._modeSeen = { edit: false, slots: false };

    XoneK3.modePrint = function(line) {
        if (!XoneK3.MODE_PRINTS) return;
        print('[k3] ' + line);
    };

    XoneK3.modePrintFirst = function(mode, hint) {
        if (!XoneK3.MODE_PRINTS) return;
        if (XoneK3._modeSeen[mode]) return;
        XoneK3._modeSeen[mode] = true;
        print('[k3]   ' + hint);
        print('[k3]   silence: XoneK3.MODE_PRINTS = false  (xone-k3-lab.js)');
    };

    XoneK3.registerPrintTable({
        labSegment:      'segment {i}',
        labMarker:       'marker',
        labDeleteSeg:    'deleted segment {i}',
        labExtend:       'extended to {n} stops',
        labMarkerClear:  'marker cleared',
        labEditSeg:      'seg {i} {field} = {v}',
        labEditMarker:   'marker {field} = {v}',
        labCurve:        'curve = {v}',
        labBrightness:   'brightness = {v}',
        labBrightScale:  'brightnessScale = {v}',
        labSchemePos:    'schemePosition = {v}',
        labMarkerPos:    'markerPosition = {v}',
        labMarkerWidth:  'markerWidth = {v}',
        labRatio:        'ratio[{i}] = {v}',
        labRatioOn:      'ratio edit ON',
        labRatioOff:     'ratio edit OFF',
        labInterpOn:     'interp ON',
        labInterpOff:    'interp OFF',
        labCalibOn:      'calibration ON',
        labCalibOff:     'calibration OFF',
        labFillGaps:     'fill gaps ({n} anchors)',
        labReset:        '{field} = {v}',
        labPreviewOn:    'preview ON',
        labPreviewOff:   'preview OFF',
        labSlotPress:    'preview slot {i}',
        labSlotSaved:    'saved {from} -> slot {to}',
        labEditReturn:   'return to edit',
        labEditThis:     'edit := {from}',
        labLoadPreview:  'edit := {from}',
        labScroll:       'loaded "{name}"',
        labDump:         'dump {from}',
        labHk:           'hkBoost = {v}',
        labLum:          'lumGamma = {v}',
        labWbOn:         'white balance ON',
        labWbOff:        'white balance OFF',
        labBeatEnabled:  'beat enabled = {v}',
        labBeatBar:      'beat barBeats = {v}',
        labBeatPhrase:   'beat phraseBars = {v}',
        labBeatPre:      'beat prePhraseBeats = {v}',
        labBeatColor:    'beat.{level}.{field} = {v}',
        labBeatSelect:   'beat target = {level}',
        labBeatClear:    'beat cleared: {level}'
    }, 'lab');

    // ============================================================
    // FIELD EDIT
    // ============================================================
    function clamp127(x) { return Math.max(0, Math.min(127, x)); }

    XoneK3._applyFieldEdit = function(rgb, br, field, mode, value) {
        var isStep = (mode === 'step');
        var v;
        if (field === 'r') { rgb[0] = clamp127(isStep ? rgb[0] + value : value); v = rgb[0]; }
        else if (field === 'g') { rgb[1] = clamp127(isStep ? rgb[1] + value : value); v = rgb[1]; }
        else if (field === 'b') { rgb[2] = clamp127(isStep ? rgb[2] + value : value); v = rgb[2]; }
        else if (field === 'hw' || field === 'brightness') { br = clamp127(isStep ? br + value : value); v = br; }
        else return null;
        return { br: br, val: v };
    };

    // ── colorSet helpers ───────────────────────────────────────
    // `scheme.colorSet` is a parallel boolean array to
    // `scheme.colors`: true = real stop, false = gap.
    //
    // Lab editing state, not persisted — _cloneScheme strips it,
    // so save / bake / load / dump drop it. Reloaded schemes
    // are all-true (correct: saved as fully-realised stops).
    //
    // Cascade delete and fill both read this flag. Only black
    // cells can be false; non-black implies true.
    function _csArray(scheme) {
        if (!scheme.colorSet) {
            scheme.colorSet = [];
            for (var i = 0; i < (scheme.colors || []).length; i++) {
                scheme.colorSet.push(true);
            }
        }
        return scheme.colorSet;
    }
    function _extendTo(scheme, idx) {
        var cs = _csArray(scheme);
        while (scheme.colors.length <= idx) {
            scheme.colors.push([0, 0, 0]);
            cs.push(false);
        }
    }
    function _setColorSet(scheme, idx, val) {
        var cs = _csArray(scheme);
        if (cs.length <= idx) _extendTo(scheme, idx);
        cs[idx] = val;
    }
    function _isColorSet(scheme, idx) {
        if (!scheme.colorSet) return true;
        return scheme.colorSet[idx] !== false;
    }

    // ============================================================
    // BAKE
    // ============================================================
    // Bake levels:
    //   'structural'   — source stops, calibration live on reload.
    //                    curve / schemePosition / ratio /
    //                    brightnessScale kept as fields. Editable
    //                    after paste.
    //   'bakedInterp'  — source stops, calibration bisected into
    //                    brightness at bake time. interp forced
    //                    true. Reloads here with calibration off;
    //                    curve / schemePosition / ratio live.
    //   'baked'        — same, interp forced false.
    //   'export'       — same, plus white balance baked into rgb.
    //                    calibration off, brightnessScale absent.
    //                    Reloading here double-applies wb — for
    //                    external raw-sysex hand-off only.
    XoneK3.bakeScheme = function(ref, ctrl, level) {
        var raw = XoneK3.resolveScheme(ref);
        if (!raw || !raw.colors) return null;
        var compiled = XoneK3.compileScheme(raw);
        if (!compiled) return null;
        var segs = compiled.on;

        var bakeCal = (level === 'bakedInterp' || level === 'baked' ||
                       level === 'export');
        var bakeWb  = (level === 'export');

        // Flat-target reference used by the bisect. Computed once
        // per bake. Skip when the source has calibration off or
        // the controller has no calibration pipeline.
        var flatTarget = null;
        if (bakeCal && raw.calibration &&
            ctrl && ctrl.calibrationMode !== 'off') {
            flatTarget = ctrl._flatTarget(segs);
            flatTarget = XoneK3.scaleTarget(flatTarget, raw.brightnessScale);
        }

        var wb = (bakeWb && ctrl && ctrl.wbActive &&
                  ctrl.wbGain && ctrl.wbGain.length >= 3)
                 ? ctrl.wbGain : null;

        var newColors = segs.map(function(s) {
            var rgb = [s.color.rgb[0], s.color.rgb[1], s.color.rgb[2]];
            var br  = s.color.brightness;
            if (flatTarget !== null &&
                (rgb[0] > 0 || rgb[1] > 0 || rgb[2] > 0)) {
                br = ctrl._calibratedHw(rgb, flatTarget);
            }
            if (wb) {
                rgb[0] = Math.max(0, Math.min(127, Math.round(rgb[0] * wb[0])));
                rgb[1] = Math.max(0, Math.min(127, Math.round(rgb[1] * wb[1])));
                rgb[2] = Math.max(0, Math.min(127, Math.round(rgb[2] * wb[2])));
            }
            return [rgb[0], rgb[1], rgb[2], br];
        });

        var out = {
            colors: newColors,
            brightness: raw.brightness,
            interp: raw.interp,
            markerColor: raw.markerColor,
            markerPosition: raw.markerPosition,
            markerWidth: raw.markerWidth,
            offScheme: raw.offScheme,
            coScheme: raw.coScheme,
            reverse: raw.reverse,
            beat: raw.beat
        };

        if (level === 'bakedInterp') out.interp = true;
        if (level === 'baked')       out.interp = false;

        if (level === 'structural' &&
            typeof raw.brightnessScale === 'number' &&
            raw.brightnessScale !== 1) {
            out.brightnessScale = raw.brightnessScale;
        }
        if (bakeCal) out.calibration = false;

        // curve / schemePosition / ratio stay as fields on every
        // level — stop count is preserved, so they have to live
        // on the scheme.
        out.ratio = segs.map(function(s) { return s.end - s.start; });
        if (typeof raw.schemePosition === 'number' && raw.schemePosition !== 63.5) {
            out.schemePosition = raw.schemePosition;
        }
        if (typeof raw.curve === 'number' && raw.curve !== 0) {
            out.curve = raw.curve;
        }

        return out;
    };

    XoneK3.LabController = function(cfg) {
        XoneK3.K3Controller.call(this, cfg);

        this.ledTargetGroups = cfg.ledTargetGroups || {};
        this._labPreviewTarget = cfg.labPreviewTarget || null;
        this._markerTarget = cfg.markerTarget || null;
        this._labPreviewCO = cfg.labPreviewCO || null;
        this._previewCOConn = null;

        this._lab = {
            slots: new Array(16).fill(null),
            edit: { colors: [[0, 0, 0]] },
            previewSource: 'edit',
            previewing: false,
            saveArmed: false,
            previewPos: 0.5,
            previewKnown: false,
            previewOn: true,
            selectedSegment: 0,
            ratioEdit: false,
            schemeName: null,
            beatTarget: null,
            _version: 0
        };

        if (cfg.modePrints   !== undefined) XoneK3.MODE_PRINTS     = !!cfg.modePrints;
        if (cfg.debugPrints  !== undefined) XoneK3._debugPrintsOn  = !!cfg.debugPrints;
    };

    XoneK3.LabController.prototype = Object.create(XoneK3.K3Controller.prototype);
    XoneK3.LabController.prototype.constructor = XoneK3.LabController;

    // ============================================================
    // BINDING DISCOVERY
    // ============================================================
    // Reverse index: func id -> { cell: args }. Built lazily,
    // invalidated by _applied identity (reassigned on every
    // recompute).
    //
    // Keys are XoneK3._funcId(fn), NOT `fn` itself. Using `fn` as an
    // object key coerces through Function.prototype.toString(), and
    // every closure returned by the same expression (e.g. all the
    // _makeFieldEditor instances) stringifies identically — one
    // bucket, wrong cells painted.
    XoneK3.LabController.prototype._buildCellIndex = function() {
        var idx = {};
        var resolved = this._applied || {};
        for (var cell in resolved) {
            var cfg = resolved[cell];
            if (!cfg || !cfg.func) continue;
            var fid = XoneK3._funcId(cfg.func);
            if (!idx[fid]) idx[fid] = {};
            idx[fid][cell] = cfg.args ? cfg.args.slice() : [];
        }
        return idx;
    };

    XoneK3.LabController.prototype._findCellsByFunc = function(funcRefs) {
        if (this._cellIndexFor !== this._applied) {
            this._cellIndex = this._buildCellIndex();
            this._cellIndexFor = this._applied;
        }
        var out = {};
        var idx = this._cellIndex;
        for (var i = 0; i < funcRefs.length; i++) {
            var fid = XoneK3._funcId(funcRefs[i]);
            var cells = idx[fid];
            if (!cells) continue;
            for (var cell in cells) out[cell] = cells[cell];
        }
        return out;
    };

    XoneK3.LabController.prototype._claimedCells = function() {
        var claimed = new Set();
        var cells = this._findCellsByFunc(XoneK3.LAB_PAINTER_FUNCS);
        for (var c in cells) claimed.add(c);
        return claimed;
    };

    // ============================================================
    // STATE HELPERS
    // ============================================================
    // Deep clone, and strip lab editing state (colorSet) so it can
    // never leak into a slot, a bake, or a loaded scheme. This is
    // the single hook that keeps colorSet inside _lab.edit —
    // every save / load path uses _cloneScheme.
    XoneK3.LabController.prototype._cloneScheme = function(s) {
        if (!s) return null;
        var clone = JSON.parse(JSON.stringify(s));
        delete clone.colorSet;
        return clone;
    };

    XoneK3.LabController.prototype._editableScheme = function() {
        if (!this._patchActive('colorLab')) return null;
        return this._lab.edit;
    };

    XoneK3.LabController.prototype._previewScheme = function() {
        var s = this._lab.previewSource;
        if (s === 'edit') return this._lab.edit;
        if (typeof s === 'number') return this._lab.slots[s] || null;
        if (typeof s === 'string') {
            return XoneK3.SCHEMES[s] ? XoneK3.resolveScheme(s) : null;
        }
        return null;
    };

    XoneK3.LabController.prototype._labLEDScheme = function() {
        if (this._patchActive('colorLab')) {
            if (this._lab.previewing) return this._previewScheme();
            return this._lab.edit;
        }
        return this._previewScheme();
    };

    XoneK3.LabController.prototype._commitPreview = function() {
        if (!this._patchActive('colorLab')) return null;
        if (this._lab.previewSource !== 'edit') {
            XoneK3._warnOnce('labEditWhilePreviewing',
                'preview is preview-only — shift+pressR loads the preview ' +
                'into edit before edits take effect');
            return null;
        }
        return this._lab.edit;
    };

    XoneK3.LabController.prototype._withEdit = function(fn) {
        var scheme = this._commitPreview();
        if (!scheme) return;
        if (fn.call(this, scheme) === false) return;
        this._labRenderPreview();
    };

    XoneK3.LabController.prototype._previewLabel = function() {
        var s = this._lab.previewSource;
        if (s === 'edit') return 'edit';
        if (typeof s === 'number') {
            if (s === XoneK3.LAB_SLOT.STRUCTURAL)   return 'structural';
            if (s === XoneK3.LAB_SLOT.BAKED_INTERP) return 'baked+interp';
            if (s === XoneK3.LAB_SLOT.BAKED)        return 'baked';
            if (s === XoneK3.LAB_SLOT.EXPORT)       return 'export';
            return 'slot' + s;
        }
        return s;
    };

    XoneK3.LabController.prototype._isBeatEdit = function() {
        return this._patchActive('colorLabBeat');
    };

    // ============================================================
    // AUTO-BAKE
    // ============================================================
    XoneK3.LabController.prototype._applyAutoBake = function() {
        var src = this._lab.previewSource;
        var editable = (src === 'edit') ||
                       (typeof src === 'number' && src >= 0 && src <= 11);
        if (!editable) return;
        var preview = this._previewScheme();
        if (!preview) return;
        var S = XoneK3.LAB_SLOT;
        this._lab.slots[S.STRUCTURAL]   = XoneK3.bakeScheme(preview, this, 'structural');
        this._lab.slots[S.BAKED_INTERP] = XoneK3.bakeScheme(preview, this, 'bakedInterp');
        this._lab.slots[S.BAKED]        = XoneK3.bakeScheme(preview, this, 'baked');
        this._lab.slots[S.EXPORT]       = XoneK3.bakeScheme(preview, this, 'export');
    };

    // ============================================================
    // PREVIEW CHANGE
    // ============================================================
    XoneK3.LabController.prototype._setPreview = function(src, label) {
        this._lab.previewSource = src;
        this._lab.previewing = true;
        this._applyAutoBake();
        this._labRenderPreview();
        XoneK3.modePrint('preview = ' + (label || this._previewLabel()));
    };

    // ============================================================
    // SLOT ACTIONS
    // ============================================================
    XoneK3.LabController.prototype._selectSlot = function(idx) {
        if (this._lab.saveArmed) {
            var preview = this._previewScheme();
            if (!preview) {
                this._lab.saveArmed = false;
                XoneK3.modePrint('arm = OFF');
                XoneK3.debugPrint('labSlotPress', { i: idx });
                this._setPreview(idx, 'slot' + idx);
                this._refreshBinding('layer');
                return;
            }
            if (this._lab.previewSource !== idx) this._emitOverwriteBackup(idx);
            this._lab.slots[idx] = this._cloneScheme(XoneK3.resolveScheme(preview));
            XoneK3.debugPrint('labSlotSaved', { from: this._previewLabel(), to: idx });
            this._lab.saveArmed = false;
            XoneK3.modePrint('arm = OFF');
            this._lab.edit = this._cloneScheme(this._lab.slots[idx]);
            this._lab.previewSource = 'edit';
            this._lab.previewing = false;
            this._lab.selectedSegment = 0;
            this._lab.ratioEdit = false;
            this.replacePatch('slots', 'colorLab');
            return;
        }
        XoneK3.debugPrint('labSlotPress', { i: idx });
        this._setPreview(idx, 'slot' + idx);
    };

    XoneK3.LabController.prototype._toggleSaveArm = function() {
        this._lab.saveArmed = !this._lab.saveArmed;
        XoneK3.modePrint('arm = ' + (this._lab.saveArmed ? 'ON' : 'OFF'));
        this._labRenderGrid();
        this._refreshBinding('layer');
    };

    XoneK3.LabController.prototype._editReturn = function() {
        this._lab.saveArmed = false;
        this.replacePatch('slots', 'colorLab');
    };

    XoneK3.LabController.prototype._editThis = function() {
        var preview = this._previewScheme();
        if (!preview) return;
        XoneK3.debugPrint('labEditThis', { from: this._previewLabel() });
        this._lab.edit = this._cloneScheme(XoneK3.resolveScheme(preview));
        this._lab.previewSource = 'edit';
        this._lab.previewing = false;
        this._lab.selectedSegment = 0;
        this._lab.ratioEdit = false;
        this._lab.saveArmed = false;
        this.replacePatch('slots', 'colorLab');
    };

    XoneK3.LabController.prototype._loadPreview = function() {
        if (!this._patchActive('colorLab')) return;
        if (this._lab.previewSource === 'edit') return;
        var preview = this._previewScheme();
        if (!preview) return;
        XoneK3.debugPrint('labLoadPreview', { from: this._previewLabel() });
        this._lab.edit = this._cloneScheme(XoneK3.resolveScheme(preview));
        this._lab.previewSource = 'edit';
        this._lab.previewing = false;
        this._lab.selectedSegment = 0;
        this._lab.ratioEdit = false;
        this._labRenderPreview();
    };

    // ============================================================
    // COLOUR EDIT
    // ============================================================
    XoneK3.LabController.prototype._editColorField = function(field, mode, value) {
        if (this._isBeatEdit()) {
            if (!this._lab.beatTarget) return;
            if (mode === 'step') this._editBeatColor(field, value, undefined);
            else                 this._editBeatColor(field, 0, value);
            return;
        }
        this._withEdit(function(scheme) {
            return (this._lab.selectedSegment === -1)
                ? this._editMarkerTarget(scheme, field, mode, value)
                : this._editSegmentTarget(scheme, field, mode, value);
        });
    };

    XoneK3.LabController.prototype._editMarkerTarget = function(scheme, field, mode, value) {
        var defaultBr = (typeof scheme.brightness === 'number') ? scheme.brightness : 127;
        var src = XoneK3.resolveColorEntry(scheme.markerColor || 'off', defaultBr);
        if (!src) return false;

        var rgb = src.rgb ? src.rgb.slice() : [0, 0, 0];
        var onBr  = (typeof src.brightness    === 'number') ? src.brightness    : defaultBr;
        var offBr = (typeof src.brightnessOff === 'number') ? src.brightnessOff : null;

        var editBr = (field === 'hw' || field === 'brightness');
        var curBr = editBr
            ? (this._lab.previewOn ? onBr : (offBr !== null ? offBr : onBr))
            : onBr;
        var res = XoneK3._applyFieldEdit(rgb, curBr, field, mode, value);
        if (!res) return false;

        if (editBr) {
            if (this._lab.previewOn) onBr = res.br;
            else offBr = res.br;
        }

        var entry = [rgb[0], rgb[1], rgb[2], onBr];
        if (offBr !== null) entry.push(offBr);
        scheme.markerColor = entry;
        XoneK3.debugPrint('labEditMarker', { field: field, v: res.val });
        return true;
    };

    XoneK3.LabController.prototype._editSegmentTarget = function(scheme, field, mode, value) {
        var segIdx = this._lab.selectedSegment;
        if (!scheme.colors || segIdx < 0) return false;

        // Edit beyond the array extends it (all new cells are gaps).
        // Recreate-after-delete: user deleted N (cascade), then
        // turned a knob; extend brings N back as a gap, edit fills
        // it, non-black makes it colorSet true.
        if (segIdx >= scheme.colors.length) _extendTo(scheme, segIdx);
        if (segIdx >= scheme.colors.length) return false;

        var defaultBr = (typeof scheme.brightness === 'number') ? scheme.brightness : 127;
        var source = XoneK3.resolveColorEntry(scheme.colors[segIdx], defaultBr);
        if (!source) return false;

        var rgb = source.rgb ? source.rgb.slice() : [0, 0, 0];
        var res = XoneK3._applyFieldEdit(rgb, source.brightness, field, mode, value);
        if (!res) return false;

        // Preserve existing off-brightness. Format is
        // [r,g,b,on,off] — if off is set we must write on
        // explicitly even when it equals the default, so the
        // off value lands in position 4, not 3.
        var offBr = (typeof source.brightnessOff === 'number')
            ? source.brightnessOff : null;
        var entry = [rgb[0], rgb[1], rgb[2]];
        if (res.br !== defaultBr || offBr !== null) entry.push(res.br);
        if (offBr !== null) entry.push(offBr);
        scheme.colors[segIdx] = entry;
        // Invariant: non-black implies colorSet true.
        if (rgb[0] !== 0 || rgb[1] !== 0 || rgb[2] !== 0) {
            _setColorSet(scheme, segIdx, true);
        }
        XoneK3.debugPrint('labEditSeg', { i: segIdx, field: field, v: res.val });
        return true;
    };

    // ============================================================
    // BEAT EDIT
    // ============================================================
    XoneK3.LabController.prototype._editBeatColor = function(field, delta, absValue) {
        this._withEdit(function(scheme) {
            var level = this._lab.beatTarget;
            if (!level) return false;
            if (!scheme.beat) scheme.beat = { enabled: true };

            var tgt = (level === 'beat')
                ? scheme.beat
                : (scheme.beat[level] = scheme.beat[level] || {});

            var cur = XoneK3.resolveColorEntry(tgt.color || 'off');
            var rgb = (cur && cur.rgb) ? cur.rgb.slice() : [0, 0, 0];
            // Brightness may be embedded in the color spec
            // ('green, 60') or stored alongside it (brightness: 60).
            // The separate field wins when set; otherwise fall back
            // to the embedded value so an RGB edit doesn't zero it
            // to the default 127.
            var br  = (typeof tgt.brightness === 'number')
                ? tgt.brightness
                : ((cur && typeof cur.brightness === 'number') ? cur.brightness : 127);
            var isAbs = (typeof absValue === 'number');
            var res = XoneK3._applyFieldEdit(rgb, br, field,
                                             isAbs ? 'abs' : 'step',
                                             isAbs ? absValue : delta);
            if (!res) return false;

            tgt.color = [rgb[0], rgb[1], rgb[2]];
            tgt.brightness = res.br;

            XoneK3.debugPrint('labBeatColor', { level: level, field: field, v: res.val });
            return true;
        });
    };

    // ============================================================
    // DUMP — per-slot emitters
    // ============================================================
    XoneK3.LabController.prototype._emitOverwriteBackup = function(idx) {
        if (!this._lab.slots[idx]) return;
        var L = ['// ==================== overwriting slot ' + idx + ' ===================='];
        this._emitSlotDump(idx, L);
        print(L.join('\n'));
    };

    XoneK3.LabController.prototype._emitSlotDump = function(slotIdx, L) {
        var scheme = XoneK3.resolveScheme(this._lab.slots[slotIdx]);
        if (!scheme) return;
        var S = XoneK3.LAB_SLOT;
        var isMain = (slotIdx === S.ON);
        var baseName = 'slot' + slotIdx;

        var offScheme = null;
        var coScheme  = null;
        if (isMain) {
            var offSlot   = this._lab.slots[S.OFF];
            var coSlot    = this._lab.slots[S.CO];
            var coOffSlot = this._lab.slots[S.CO_OFF];
            if (offSlot) offScheme = XoneK3.resolveScheme(offSlot);
            if (coSlot) {
                coScheme = Object.assign({}, XoneK3.resolveScheme(coSlot));
                if (coOffSlot) coScheme.offScheme = XoneK3.resolveScheme(coOffSlot);
            }
        }

        _emitSchemeLines(L, baseName, scheme, offScheme, coScheme);
    };

    XoneK3.LabController.prototype._dumpSlot = function(slotIdx) {
        var L = [];
        this._emitSlotDump(slotIdx, L);
        print('\n' + L.join('\n'));
    };

    XoneK3.LabController.prototype._dumpPreview = function() {
        var src = this._lab.previewSource;
        if (typeof src === 'number') { this._dumpSlot(src); return; }
        var scheme;
        if (this._patchActive('colorLab') || src === 'edit') {
            scheme = this._lab.edit;
        } else if (typeof src === 'string') {
            scheme = XoneK3.resolveScheme(src);
        }
        if (!scheme) return;
        var L = [];
        _emitSchemeLines(L, 'NAME', XoneK3.resolveScheme(scheme));
        print('\n' + L.join('\n'));
    };

    XoneK3.LabController.prototype._dumpAllSlots = function() {
        var S = XoneK3.LAB_SLOT;
        var L = [];
        var emitted = {};
        var count = 0;
        for (var i = 0; i < 16; i++) {
            if (emitted[i]) continue;
            if (!this._lab.slots[i]) continue;
            L.push('// ==================== slot ' + i + ' ====================');
            this._emitSlotDump(i, L);
            count++;
            if (i === S.ON) {
                if (this._lab.slots[S.OFF])    emitted[S.OFF]    = true;
                if (this._lab.slots[S.CO])     emitted[S.CO]     = true;
                if (this._lab.slots[S.CO_OFF]) emitted[S.CO_OFF] = true;
            }
        }
        if (count === 0) print('// lab: no saved slots');
        else print('\n// lab: dumped ' + count + ' slot(s)\n' + L.join('\n'));
    };

    // ============================================================
    // OVERRIDES
    // ============================================================
    XoneK3.LabController.prototype._initExtra = function() {
        if (this.wbActive === undefined) this.wbActive = true;

        var def = XoneK3.resolveScheme('labDefault');
        if (def) this._lab.slots[0] = this._cloneScheme(def);

        var self = this;

        if (this._labPreviewCO && this._labPreviewCO.group && this._labPreviewCO.name) {
            this._previewCOConn = engine.makeConnection(
                this._labPreviewCO.group, this._labPreviewCO.name,
                function() { self._labRenderPreview(); });
        }

        // Fires on every beat_active transition for every open
        // channel. Only the compiled preview's own beat deck can
        // change what the preview LED shows — the previewPos,
        // marker, and markerBand are single-channel scalars. A
        // beat on channel 3 with a preview scheme bound to
        // channel 1 repaints an identical frame. Runtime
        // _onBeatActive already syncs the count via _repaintBeat
        // on every rising edge (throttle only trips above ~2000
        // BPM), so skipping the lab-side sync for non-preview
        // channels leaves deck state correct. beatCh === null
        // (unresolvable deck ref) falls through and does the
        // work — same as before.
        this._onBeat = function(ch) {
            var compiled = self._compiledPreview().compiled;
            if (!compiled || !compiled.beat) return;
            var beatCh = self._resolveDeckRef(compiled.beat.deck);
            if (beatCh && ch !== beatCh) return;
            self._syncBeatCount(ch);
            self._renderPreviewLED();
        };

        this._labRenderPreview();
        XoneK3.modePrint('mode = EDIT');
        XoneK3.modePrintFirst('edit', 'layer = slots, shift+R = load preview into edit');

        this._refreshAllLeds('lab-init');
    };

    XoneK3.LabController.prototype.shutdown = function() {
        if (this._shuttingDown) return;
        if (XoneK3.LAB_DUMP_ON_SHUTDOWN) {
            print('// lab: shutdown dump');
            this._dumpAllSlots();
        }
        XoneK3.K3Controller.prototype.shutdown.call(this);
    };

    XoneK3.LabController.prototype._shutdownExtra = function() {
        if (this._previewCOConn) { this._previewCOConn.disconnect(); this._previewCOConn = null; }
        this._onBeat = null;
    };

    XoneK3.LabController.prototype._onInput = function(b) {
        this._lab._version++;
    };

    XoneK3.LabController.prototype._afterRecompute = function() {
        this._labRenderPreview();
    };

    XoneK3.LabController.prototype._onPatchChange = function(from, to) {
        if (from === 'colorLab' && to !== 'colorLab') {
            this._lab.previewSource = 'edit';
            this._lab.previewing = false;
            this._lab.saveArmed = false;
            this._applyAutoBake();
            this._refreshBinding('layer');
        }
        if (to === 'slots') {
            XoneK3.modePrint('mode = SLOTS (preview=' + this._previewLabel() + ')');
            XoneK3.modePrintFirst('slots',
                'grid = preview, R = arm, shift+R = load into edit, shift+L = scroll');
            this._labRenderPreview();
        } else if (to === 'colorLab') {
            XoneK3.modePrint('mode = EDIT');
        }
    };

    XoneK3.LabController.prototype._needsBeatClocks = function() {
        return this._beatLeds.length > 0 || !!this._onBeat;
    };

    // ============================================================
    // LED PAINT HELPERS
    // ============================================================
    XoneK3.LabController.prototype._paintRGB = function(name, r, g, b, hw) {
        var bind = this.bindings[name];
        if (!bind || bind.idx < 0) return;
        this.sendLED(bind.idx, r, g, b, hw);
    };

    XoneK3.LabController.prototype._paintLED = function(name, colorSpec, onOrHw) {
        var entry = XoneK3.resolveColorEntry(colorSpec);
        if (!entry || !entry.rgb) return;
        var hw;
        if (typeof onOrHw === 'number') {
            hw = onOrHw;
        } else if (typeof onOrHw === 'boolean') {
            hw = onOrHw
                ? entry.brightness
                : ((entry.brightnessOff !== undefined && entry.brightnessOff !== null)
                    ? entry.brightnessOff : XoneK3.DEFAULT_RGB_BRIGHTNESS_OFF);
        } else {
            hw = entry.brightness;
        }
        this._paintRGB(name, entry.rgb[0], entry.rgb[1], entry.rgb[2], hw);
    };

    XoneK3.LabController.prototype._paintCellsSolid = function(cells, colorSpec) {
        var entry = XoneK3.resolveColorEntry(colorSpec);
        if (!entry || !entry.rgb) return;
        for (var cell in cells) {
            this._paintRGB(cell, entry.rgb[0], entry.rgb[1], entry.rgb[2], entry.brightness);
        }
    };

    // ============================================================
    // GRID / SLOT / STATE RENDER
    // ============================================================
    XoneK3.LabController.prototype._labRenderPreview = function() {
        this._labRenderStateLEDs();
        this._labRenderGrid();
        this._renderPreviewLED();
    };

    XoneK3.LabController.prototype._compiledPreview = function() {
        var v = this._lab._version;
        var src = this._lab.previewSource;
        if (this._compileCache &&
            this._compileCache.v === v &&
            this._compileCache.src === src) {
            return this._compileCache;
        }
        var preview = this._labLEDScheme();
        var resolved = preview ? XoneK3.resolveScheme(preview) : null;
        var compiled = resolved ? this._previewCompiled(resolved) : null;
        this._compileCache = { v: v, src: src, resolved: resolved, compiled: compiled };
        return this._compileCache;
    };

    XoneK3.LabController.prototype._labRenderGrid = function() {
        var onColorLab = this._patchActive('colorLab');
        var onSlots    = this._patchActive('slots');
        if (!onColorLab && !onSlots) return;
        var compiled = this._compiledPreview().compiled;
        this._renderSegmentGrid(compiled, onColorLab);
        if (onColorLab) this._renderMarkerLED(compiled);
        if (onSlots) {
            this._renderSlotLEDs();
        } else if (this._isBeatEdit()) {
            this._renderBeatGrid();
        }
    };

    XoneK3.LabController.prototype._renderSegmentGrid = function(compiled, onColorLab) {
        var targets = (this.ledTargetGroups && this.ledTargetGroups.calib) || [];
        var claimed = this._claimedCells();
        for (var i = 0; i < targets.length; i++) {
            var name = targets[i];
            if (claimed.has(name)) continue;
            var paint = this._segmentCellPaint(name, i, compiled, onColorLab);
            if (paint) {
                this._paintRGB(name, paint.rgb[0], paint.rgb[1], paint.rgb[2], paint.br);
            } else {
                this._paintRGB(name, 0, 0, 0, 0);
            }
        }
    };

    // Resolve one segment cell's paint decision. Returns
    // { rgb, br } to paint, or null for black.
    XoneK3.LabController.prototype._segmentCellPaint = function(name, i, compiled, onColorLab) {
        var applied = this._applied[name];
        if (applied == null || Object.keys(applied).length === 0) return null;
        if (!compiled || i >= compiled.on.length) return null;
        var c = compiled.on[i].color;
        // Shift held in edit mode: highlight real black stops so
        // they're findable for shift+press deletion. Gaps stay
        // black — they don't need finding. Only when the strip
        // is showing the edit buffer, never on a preview.
        var isBlack = (c.rgb[0] === 0 && c.rgb[1] === 0 && c.rgb[2] === 0);
        if (onColorLab && !this._lab.previewing && this.modifiers.shift && isBlack) {
            var rawScheme = this._editableScheme();
            if (rawScheme && _isColorSet(rawScheme, i)) {
                return { rgb: [127, 127, 127], br: 50 };
            }
        }
        return {
            rgb: c.rgb,
            br: this._solveHw(c.rgb, c.brightness, compiled.on, compiled)
        };
    };

    XoneK3.LabController.prototype._renderMarkerLED = function(compiled) {
        if (!this._markerTarget) return;
        if (!compiled || !compiled.marker) {
            this._paintRGB(this._markerTarget, 0, 0, 0, 0);
            return;
        }
        var m = compiled.marker;
        this._paintRGB(this._markerTarget, m.rgb[0], m.rgb[1], m.rgb[2], m.brightness);
    };

    XoneK3.LabController.prototype._renderSlotLEDs = function() {
        if (!this._patchActive('slots')) return;
        var S = XoneK3.LAB_SLOT;
        var cells = this._findCellsByFunc([XoneK3.FUNCS.labSlotPress]);
        for (var cell in cells) {
            var idx = parseInt(cells[cell][0], 10);
            if (isNaN(idx)) continue;
            var slot = this._lab.slots[idx];
            if (!slot) { this._paintRGB(cell, 0, 0, 0, 0); continue; }
            var col = XoneK3.LAB_SLOT_COLOURS[idx];
            if (!col) {
                col = (idx <= S.SAVE_END) ? [127, 127, 0] : [127, 0, 127];
            }
            var selected = (this._lab.previewSource === idx);
            var br = selected ? 127 : 20;
            this._paintRGB(cell, col[0], col[1], col[2], br);
        }
    };

    XoneK3.LabController.prototype._labRenderStateLEDs = function() {
        if (this._patchActive('colorLab')) {
            var beatOn = this._patchActive('colorLabBeat');
            this._paintCellsSolid(
                this._findCellsByFunc([XoneK3.FUNCS.labBeatEditToggle]),
                beatOn ? 'cyan, 60' : 'white, 20');
        }

        if (this.modifiers.shift && this._patchActive('colorLabTuning')) {
            this._paintLED('press1', 'white, 60');
            this._paintLED('press2', 'cyan, 60');
            this._paintLED('press3', 'green, 60');
            return;
        }

        if (this.modifiers.shift) {
            this._paintCellsSolid(
                this._findCellsByFunc([XoneK3.FUNCS.labToggleWb]),
                this.wbActive ? 'green, 80' : 'red, 80');
            return;
        }

        var preview = this._labLEDScheme();
        var resolved = preview ? XoneK3.resolveScheme(preview) : null;
        var interp = resolved ? (resolved.interp !== false) : true;
        var calibration = resolved ? (resolved.calibration !== false) : true;

        this._paintCellsSolid(
            this._findCellsByFunc([XoneK3.FUNCS.ledLabToggleCalibration]),
            calibration ? 'green, 80' : 'red, 80');
        this._paintCellsSolid(
            this._findCellsByFunc([XoneK3.FUNCS.ledLabToggleRatio]),
            this._lab.ratioEdit ? 'green, 80' : 'red, 80');
        this._paintCellsSolid(
            this._findCellsByFunc([XoneK3.FUNCS.ledLabToggleInterp]),
            interp ? 'green, 80' : 'red, 80');
    };

    XoneK3.LabController.prototype._renderBeatGrid = function() {
        var scheme = this._labLEDScheme();
        if (!scheme) return;
        scheme = XoneK3.resolveScheme(scheme);
        if (!scheme) return;
        var beat = scheme.beat || {};

        var targetCells = this._findCellsByFunc([XoneK3.FUNCS.labBeatSelect, XoneK3.FUNCS.labBeatClear]);
        for (var cell in targetCells) {
            var level = targetCells[cell][0];
            var sub = (level === 'beat') ? beat : beat[level];
            var entry = sub && sub.color
                ? XoneK3.resolveColorEntry(sub.color)
                : null;
            if (!entry || !entry.rgb) {
                this._paintLED(cell, 'white', false);
                continue;
            }
            var br = (typeof sub.brightness === 'number') ? sub.brightness : entry.brightness;
            this._paintRGB(cell, entry.rgb[0], entry.rgb[1], entry.rgb[2], br);
        }

        var toggleCells = this._findCellsByFunc([XoneK3.FUNCS.labBeatToggleEnabled]);
        var on = !(beat.enabled === false);
        for (var tcell in toggleCells) {
            this._paintRGB(tcell, on ? 0 : 127, on ? 127 : 0, 0, 100);
        }
    };

    // ============================================================
    // PREVIEW LED
    // ============================================================
    XoneK3.LabController.prototype._previewCompiled = function(resolved) {
        if (!resolved) return null;
        if (this._lab.previewSource !== XoneK3.LAB_SLOT.ON) {
            return XoneK3.compileScheme(resolved);
        }
        var S = XoneK3.LAB_SLOT;
        var offSlot   = this._lab.slots[S.OFF];
        var coSlot    = this._lab.slots[S.CO];
        var coOffSlot = this._lab.slots[S.CO_OFF];

        var overrides = {};
        if (offSlot) overrides.offScheme = offSlot;
        if (coSlot) {
            overrides.coScheme = coOffSlot
                ? Object.assign({}, coSlot, { offScheme: coOffSlot })
                : coSlot;
        }
        return XoneK3.compileScheme(resolved, overrides);
    };

    XoneK3.LabController.prototype._renderBeatStoppedTarget = function(name, resolved) {
        if (!this._isBeatEdit() || !this._lab.beatTarget) return false;
        var bBeat = resolved && resolved.beat;
        if (!bBeat) return false;
        var lvl = this._lab.beatTarget;
        var sub = (lvl === 'beat') ? bBeat : bBeat[lvl];
        if (sub && sub.color) {
            var e = XoneK3.resolveColorEntry(sub.color);
            if (e && e.rgb) {
                var ebr = (typeof sub.brightness === 'number') ? sub.brightness : e.brightness;
                this._paintRGB(name, e.rgb[0], e.rgb[1], e.rgb[2], ebr);
                return true;
            }
        }
        this._paintRGB(name, 60, 60, 60, 100);
        return true;
    };

    XoneK3.LabController.prototype._renderCoMatch = function(name, compiled, previewPos, showOn) {
        if (!this._labPreviewCO || !this._labPreviewCO.group || !this._labPreviewCO.name) return false;
        var steer = XoneK3.coMatchSample(
            compiled,
            this._labPreviewCO.group,
            this._labPreviewCO.name,
            previewPos,
            showOn);
        if (!steer) return false;
        var coEntry = this._sampleRamp(steer.ramp, steer.t, compiled.co, showOn, true);
        if (!coEntry || coEntry === XoneK3.BEAT_HOLD) return false;
        this._paintRGB(name, coEntry.rgb[0], coEntry.rgb[1], coEntry.rgb[2], coEntry.brightness);
        return true;
    };

    XoneK3.LabController.prototype._renderMarkerBand = function(name, compiled, showOn, devDir) {
        if (!XoneK3.markerHit(compiled, devDir)) return false;
        var mbr = showOn
            ? compiled.marker.brightness
            : (typeof compiled.marker.brightnessOff === 'number'
                ? compiled.marker.brightnessOff
                : compiled.marker.brightness);
        this._paintRGB(name, compiled.marker.rgb[0], compiled.marker.rgb[1], compiled.marker.rgb[2], mbr);
        return true;
    };

    XoneK3.LabController.prototype._renderPreviewRamp = function(name, compiled, previewPos, showOn) {
        var ramp = showOn ? compiled.on : compiled.off;
        var entry = this._sampleRamp(ramp, previewPos, compiled, showOn, false);
        if (entry === XoneK3.BEAT_HOLD) return;
        if (!entry) { this._paintRGB(name, 0, 0, 0, 0); return; }
        this._paintRGB(name, entry.rgb[0], entry.rgb[1], entry.rgb[2], entry.brightness);
    };

    XoneK3.LabController.prototype._renderPreviewLED = function() {
        var name = this._labPreviewTarget;
        if (!name) return;

        if (!this._lab.previewKnown) {
            this._paintRGB(name, 127, 127, 127, 127);
            return;
        }

        try {
            this._renderPreviewLedBody(name);
        } catch (e) {
            XoneK3._warnOnce('labPreviewThrow:' + (e && e.message ? e.message : 'unknown'),
                'lab preview render threw: ' +
                (e && e.message ? e.message : e) +
                ' — preview LED painted black');
            this._paintRGB(name, 0, 0, 0, 0);
        }
    };

    XoneK3.LabController.prototype._renderPreviewLedBody = function(name) {
        var c = this._compiledPreview();
        var resolved = c.resolved;
        var compiled = c.compiled;
        if (!compiled) { this._paintRGB(name, 0, 0, 0, 0); return; }

        var st = compiled.beat ? this._deckStateFor(compiled) : null;
        var beatPlaying = !!(compiled.beat && st && st.hasBeat);

        if (!beatPlaying && this._renderBeatStoppedTarget(name, resolved)) return;

        if (beatPlaying) {
            var ch = this._resolveDeckRef(compiled.beat.deck);
            if (ch) this._syncBeatCount(ch);
        }

        var previewPos = this._lab.previewPos;
        var logicalOn = this._lab.previewOn ? 1 : 0;
        var beatOffPhase = beatPlaying && st && !st.active;

        if (beatOffPhase && !logicalOn) {
            this._paintRGB(name, 0, 0, 0, 0);
            return;
        }
        var isOn = (logicalOn && !beatOffPhase) ? 1 : 0;

        this._previewFrame(name, compiled, previewPos, isOn);
    };

    XoneK3.LabController.prototype._previewFrame = function(name, compiled, previewPos, isOn) {
        if (this._renderCoMatch(name, compiled, previewPos, isOn)) return;
        var devDir = previewPos * 2 - 1;
        if (this._renderMarkerBand(name, compiled, isOn, devDir)) return;
        this._renderPreviewRamp(name, compiled, previewPos, isOn);
    };

    // ============================================================
    // SCALAR FIELD EDITORS
    // ============================================================
    XoneK3._makeFieldEditor = function(field, opts) {
        var min = opts.min, max = opts.max, def = opts.def;
        var step = opts.step || 1;
        var absScale = opts.absScale;
        var absOffset = opts.absOffset || 0;
        var dbg = opts.debug;
        return function(args, ch, c, v, s) {
            var binding = this;
            this._ctrl._withEdit(function(scheme) {
                var cur = (typeof scheme[field] === 'number') ? scheme[field] : def;
                var next;
                if (absScale !== null && absScale !== undefined &&
                    (binding.type === 'knob' || binding.type === 'fader')) {
                    next = binding._normalized * absScale + absOffset;
                } else {
                    next = cur + (v === 1 ? 1 : -1) * step;
                }
                scheme[field] = Math.max(min, Math.min(max, next));
                XoneK3.debugPrint(dbg, { v: scheme[field] });
                return true;
            });
        };
    };

    XoneK3._makeCtrlEditor = function(field, opts) {
        var min = opts.min, max = opts.max;
        var defStep = opts.step || 0.02;
        var dbg = opts.debug;
        return function(args, ch, c, v, s) {
            var ctrl = this._ctrl;
            var dir = (v === 1) ? 1 : -1;
            var step = XoneK3.parseNum(args[0]);
            if (isNaN(step)) step = defStep;
            ctrl[field] = Math.max(min, Math.min(max, ctrl[field] + dir * step));
            XoneK3.debugPrint(dbg, { v: ctrl[field].toFixed(2) });
            ctrl._labRenderPreview();
            ctrl._refreshAllLeds('lab-ctrl-edit');
        };
    };

    XoneK3.FUNCS.ledLabCurve = XoneK3._makeFieldEditor('curve',
        { min: -0.95, max: 0.95, def: 0, step: 0.025,
          absScale: 1.9, absOffset: -0.95, debug: 'labCurve' });
    XoneK3.FUNCS.ledLabBrightness = XoneK3._makeFieldEditor('brightness',
        { min: 0, max: 127, def: 127, step: 1, absScale: 127, debug: 'labBrightness' });
    XoneK3.FUNCS.ledLabSchemePosition = XoneK3._makeFieldEditor('schemePosition',
        { min: 0, max: 127, def: 63.5, step: 2, absScale: 127, debug: 'labSchemePos' });
    XoneK3.FUNCS.ledLabMarkerPosition = XoneK3._makeFieldEditor('markerPosition',
        { min: 0, max: 127, def: 63.5, step: 1, absScale: 127, debug: 'labMarkerPos' });
    XoneK3.FUNCS.ledLabMarkerWidth = XoneK3._makeFieldEditor('markerWidth',
        { min: 0, max: 127, def: 0, step: 1, debug: 'labMarkerWidth' });
    XoneK3.FUNCS.ledLabBrightnessScale = XoneK3._makeFieldEditor('brightnessScale',
        { min: 0.1, max: 3, def: 1, step: 0.05, debug: 'labBrightScale' });

    XoneK3.FUNCS.ledLabHk  = XoneK3._makeCtrlEditor('hkBoost',
        { min: 0,   max: 2,   step: 0.02, debug: 'labHk' });
    XoneK3.FUNCS.ledLabLum = XoneK3._makeCtrlEditor('lumGamma',
        { min: 0.1, max: 2,   step: 0.02, debug: 'labLum' });

    // ============================================================
    // SIMPLE FUNCS
    // ============================================================
    XoneK3.FUNCS.labSlotPress = function(args, ch, c, v, s) {
        if (!this._ctrl._isPress(v, s)) return;
        var idx = parseInt(args[0], 10);
        if (isNaN(idx) || idx < 0 || idx > 15) return;
        this._ctrl._selectSlot(idx);
    };
    XoneK3.FUNCS.labArmSave = function(args, ch, c, v, s) {
        if (!this._ctrl._isPress(v, s)) return;
        this._ctrl._toggleSaveArm();
    };
    XoneK3.FUNCS.labEditReturn = function(args, ch, c, v, s) {
        if (!this._ctrl._isPress(v, s)) return;
        this._ctrl._editReturn();
    };
    XoneK3.FUNCS.labEditThis = function(args, ch, c, v, s) {
        if (!this._ctrl._isPress(v, s)) return;
        this._ctrl._editThis();
    };
    XoneK3.FUNCS.labLoadPreview = function(args, ch, c, v, s) {
        if (!this._ctrl._isPress(v, s)) return;
        this._ctrl._loadPreview();
    };
    XoneK3.FUNCS.labTogglePreview = function(args, ch, c, v, s) {
        if (!this._ctrl._isPress(v, s)) return;
        this._ctrl._lab.previewOn = !this._ctrl._lab.previewOn;
        XoneK3.debugPrint(this._ctrl._lab.previewOn ? 'labPreviewOn' : 'labPreviewOff');
        this._ctrl._labRenderPreview();
    };

    XoneK3.FUNCS.labSchemeScroll = function(args, ch, c, v, s) {
        var ctrl = this._ctrl;
        var names;
        if (Array.isArray(XoneK3.SCHEME_REGISTRY)) {
            names = XoneK3.SCHEME_REGISTRY.slice();
        } else {
            var all = Object.keys(XoneK3.SCHEMES);
            names = [];
            for (var i = 0; i < all.length; i++) {
                var sc = XoneK3.SCHEMES[all[i]];
                if (sc && sc.colors) names.push(all[i]);
            }
        }
        if (names.length === 0) return;
        var dir = (v === 1) ? 1 : -1;
        var cur = names.indexOf(ctrl._lab.schemeName);
        if (cur < 0) cur = 0;
        var next = (cur + dir + names.length) % names.length;
        var name = names[next];
        ctrl._lab.schemeName = name;
        XoneK3.debugPrint('labScroll', { name: name });
        ctrl._setPreview(name, name);
    };

    XoneK3.FUNCS.labToggleWb = function(args, ch, c, v, s) {
        if (!this._ctrl._isPress(v, s)) return;
        var ctrl = this._ctrl;
        if (ctrl.wbActive === undefined) ctrl.wbActive = true;
        ctrl.wbActive = !ctrl.wbActive;
        XoneK3.debugPrint(ctrl.wbActive ? 'labWbOn' : 'labWbOff');
        ctrl._labRenderPreview();
        ctrl._refreshAllLeds('lab-wb');
    };

    XoneK3.FUNCS.ledSelectTarget = function(args, ch, c, v, s) {
        if (!this._ctrl._isPress(v, s)) return;
        var ctrl = this._ctrl;
        var scheme = ctrl._commitPreview();
        if (!scheme) return;
        var name = args[0];
        var targets = (ctrl.ledTargetGroups && ctrl.ledTargetGroups.calib) || [];
        var idx = targets.indexOf(name);
        if (idx < 0) { print('ledSelectTarget: ' + name + ' not in calib group'); return; }
        if (!scheme.colors) scheme.colors = [[0, 0, 0]];

        if (ctrl.modifiers.shift) {
            // Delete (shift+press).
            //
            //   real stop → cascade: remove the stop and every gap
            //               cell between its real neighbours, so
            //               the neighbours become adjacent.
            //   gap cell  → splice just that cell. Subsequent
            //               cells shift down by one. No cascade.
            //
            // Cell 0 protected. Selection jumps to cell 0.
            if (idx === 0) return;
            if (idx >= scheme.colors.length) return;

            if (_isColorSet(scheme, idx)) {
                var prevTrue = -1;
                for (var l = idx - 1; l >= 0; l--) {
                    if (_isColorSet(scheme, l)) { prevTrue = l; break; }
                }
                var nextTrue = -1;
                for (var r = idx + 1; r < scheme.colors.length; r++) {
                    if (_isColorSet(scheme, r)) { nextTrue = r; break; }
                }
                // Cell 0 is always true, so prevTrue >= 0 when
                // idx > 0.
                var start = prevTrue + 1;
                var end = (nextTrue === -1) ? scheme.colors.length : nextTrue;
                var removeCount = end - start;
                if (removeCount > 0) {
                    scheme.colors.splice(start, removeCount);
                    if (scheme.colorSet) scheme.colorSet.splice(start, removeCount);
                    if (scheme.ratio) scheme.ratio.splice(start, removeCount);
                }
            } else {
                scheme.colors.splice(idx, 1);
                if (scheme.colorSet) scheme.colorSet.splice(idx, 1);
                if (scheme.ratio) scheme.ratio.splice(idx, 1);
            }

            ctrl._lab.selectedSegment = 0;
            XoneK3.debugPrint('labDeleteSeg', { i: idx });
            ctrl._labRenderPreview();
            return;
        }

        // Unshifted press: extend to idx (new cells are gaps),
        // mark idx colorSet true, select it.
        _extendTo(scheme, idx);
        _setColorSet(scheme, idx, true);
        ctrl._lab.selectedSegment = idx;
        XoneK3.debugPrint('labSegment', { i: idx });
        ctrl._labRenderPreview();
    };

    XoneK3.FUNCS.ledSelectMarker = function(args, ch, c, v, s) {
        if (!this._ctrl._isPress(v, s)) return;
        var ctrl = this._ctrl;
        var scheme = ctrl._commitPreview();
        if (!scheme) return;
        if (ctrl.modifiers.shift) {
            scheme.markerColor = null;
            scheme.markerWidth = 0;
            XoneK3.debugPrint('labMarkerClear');
            ctrl._labRenderPreview();
            return;
        }
        ctrl._lab.selectedSegment = -1;
        XoneK3.debugPrint('labMarker');
        ctrl._labRenderPreview();
    };

    XoneK3.FUNCS.ledRGB = function(args, ch, c, v, s) {
        this._ctrl._editColorField(args[0], 'step', (v === 1) ? 1 : -1);
    };
    XoneK3.FUNCS.ledFader = function(args, ch, c, v, s) {
        var abs = Math.max(0, Math.min(127, Math.round(this._normalized * 127)));
        this._ctrl._editColorField(args[0], 'abs', abs);
    };

    XoneK3.FUNCS.ledRGBShow = function(args, ch, c, v, s) {
        if (!this._ctrl._isPress(v, s)) return;
        var ctrl = this._ctrl;
        var scheme = ctrl._editableScheme();
        if (!scheme) return;
        var segIdx = ctrl._lab.selectedSegment;
        if (segIdx === -1) {
            print('marker: ' + JSON.stringify(scheme.markerColor || 'unset') +
                  '  width=' + (scheme.markerWidth || 0) +
                  '  pos=' + (scheme.markerPosition || 63.5));
        } else if (!scheme.colors || segIdx >= scheme.colors.length) {
            print('segment ' + segIdx + ' out of range');
        } else {
            print('segment ' + segIdx + ': ' + JSON.stringify(scheme.colors[segIdx]));
        }
    };

    XoneK3.FUNCS.ledFillGaps = function(args, ch, c, v, s) {
        if (!this._ctrl._isPress(v, s)) return;
        var ctrl = this._ctrl;
        var scheme = ctrl._commitPreview();
        if (!scheme || !scheme.colors || scheme.colors.length === 0) return;
        var defaultBr = (typeof scheme.brightness === 'number') ? scheme.brightness : 127;
        // Anchors are colorSet cells only. Gaps (colorSet false) are
        // painted. colorSet blacks anchor; gap blacks don't.
        var anchors = [];
        for (var i = 0; i < scheme.colors.length; i++) {
            var r = XoneK3.resolveColorEntry(scheme.colors[i], defaultBr);
            if (!r || !r.rgb) continue;
            if (!_isColorSet(scheme, i)) continue;
            anchors.push({ pos: i, rgb: r.rgb, br: r.brightness });
        }
        if (anchors.length < 2) { print('ledFillGaps: need 2+ colorSet anchors'); return; }
        for (var a = 0; a < anchors.length - 1; a++) {
            var A = anchors[a], B = anchors[a + 1];
            var span = B.pos - A.pos;
            if (span <= 1) continue;
            for (var k = 1; k < span; k++) {
                var idx = A.pos + k;
                if (_isColorSet(scheme, idx)) continue;
                var t = k / span;
                scheme.colors[idx] = [
                    Math.round(A.rgb[0] * (1 - t) + B.rgb[0] * t),
                    Math.round(A.rgb[1] * (1 - t) + B.rgb[1] * t),
                    Math.round(A.rgb[2] * (1 - t) + B.rgb[2] * t),
                    Math.round(A.br * (1 - t) + B.br * t)
                ];
                // Painted cells are committed stops — mark set so
                // re-fill skips them. To re-fill a range, delete
                // those cells first (gap-splice back to blank), or
                // black them (which un-commits via edit auto-set
                // since black doesn't set the flag).
                _setColorSet(scheme, idx, true);
            }
        }
        XoneK3.debugPrint('labFillGaps', { n: anchors.length });
        ctrl._labRenderPreview();
    };

    XoneK3.FUNCS.ledLabRatio = function(args, ch, c, v, s) {
        var ctrl = this._ctrl;
        if (!ctrl._lab.ratioEdit) { XoneK3.debugPrint('labRatioOff'); return; }
        var scheme = ctrl._commitPreview();
        if (!scheme) return;
        var segCount = (scheme.colors || []).length;
        if (segCount > 9) {
            if (scheme.ratio) { scheme.ratio = null; print('ratio wiped (segments > 9)'); ctrl._labRenderPreview(); }
            return;
        }
        var idx = parseInt(args[0], 10);
        if (isNaN(idx) || idx < 0) return;
        if (!scheme.ratio) scheme.ratio = [];
        while (scheme.ratio.length <= idx) scheme.ratio.push(1);
        if (this.type === 'knob' || this.type === 'fader') {
            scheme.ratio[idx] = Math.pow(2, (this._normalized * 2 - 1) * XoneK3.RATIO_GAIN);
        } else {
            var dir = (v === 1) ? 1 : -1;
            scheme.ratio[idx] = Math.max(0.1, Math.min(10, (scheme.ratio[idx] || 1) + dir * 0.1));
        }
        XoneK3.debugPrint('labRatio', { i: idx, v: scheme.ratio[idx].toFixed(2) });
        ctrl._labRenderPreview();
    };

    XoneK3.FUNCS.ledLabToggleInterp = function(args, ch, c, v, s) {
        if (!this._ctrl._isPress(v, s)) return;
        var ctrl = this._ctrl;
        var scheme = ctrl._commitPreview(); if (!scheme) return;
        scheme.interp = !(scheme.interp !== false);
        XoneK3.debugPrint(scheme.interp ? 'labInterpOn' : 'labInterpOff');
        ctrl._labRenderPreview();
    };
    XoneK3.FUNCS.ledLabToggleCalibration = function(args, ch, c, v, s) {
        if (!this._ctrl._isPress(v, s)) return;
        var ctrl = this._ctrl;
        var scheme = ctrl._commitPreview(); if (!scheme) return;
        scheme.calibration = !(scheme.calibration !== false);
        XoneK3.debugPrint(scheme.calibration ? 'labCalibOn' : 'labCalibOff');
        ctrl._labRenderPreview();
    };
    XoneK3.FUNCS.ledLabToggleRatio = function(args, ch, c, v, s) {
        if (!this._ctrl._isPress(v, s)) return;
        var ctrl = this._ctrl;
        var scheme = ctrl._commitPreview(); if (!scheme) return;
        ctrl._lab.ratioEdit = !ctrl._lab.ratioEdit;
        if (!ctrl._lab.ratioEdit) scheme.ratio = null;
        XoneK3.debugPrint(ctrl._lab.ratioEdit ? 'labRatioOn' : 'labRatioOff');
        ctrl._labRenderPreview();
    };
    XoneK3.FUNCS.ledLabReset = function(args, ch, c, v, s) {
        if (!this._ctrl._isPress(v, s)) return;
        var field = args[0]; if (!field) return;
        var val = parseFloat(args[1]); if (isNaN(val)) return;
        var ctrl = this._ctrl;
        var scheme = ctrl._commitPreview(); if (!scheme) return;
        scheme[field] = val;
        XoneK3.debugPrint('labReset', { field: field, v: val });
        ctrl._labRenderPreview();
    };

    XoneK3.FUNCS.ledLabPreview = function(args, ch, c, v, s) {
        var ctrl = this._ctrl;
        if (this.type === 'knob' || this.type === 'fader') {
            ctrl._lab.previewPos = Math.max(0, Math.min(1, this._normalized));
        } else {
            var dir = (v === 1) ? 1 : -1;
            ctrl._lab.previewPos = Math.max(0, Math.min(1, ctrl._lab.previewPos + dir * 0.01));
        }
        ctrl._lab.previewKnown = true;
        ctrl._labRenderPreview();
    };
    XoneK3.FUNCS.ledLabPreviewWithCO = function(args, ch, c, v, s) {
        var ctrl = this._ctrl;
        var phys;
        if (this.type === 'knob' || this.type === 'fader') {
            phys = Math.max(0, Math.min(1, this._normalized));
        } else {
            var dir = (v === 1) ? 1 : -1;
            phys = Math.max(0, Math.min(1, ctrl._lab.previewPos + dir * 0.01));
        }
        ctrl._lab.previewPos = phys;
        ctrl._lab.previewKnown = true;
        if (this.group && this.inKey) engine.setParameter(this.group, this.inKey, phys);
        ctrl._labRenderPreview();
    };

    XoneK3.FUNCS.ledDumpScheme = function(args, ch, c, v, s) {
        if (!this._ctrl._isPress(v, s)) return;
        var ctrl = this._ctrl;
        XoneK3.debugPrint('labDump', { from: ctrl._previewLabel() });
        ctrl._dumpPreview();
    };

    XoneK3.FUNCS.labBeatEditToggle = function(args, ch, c, v, s) {
        if (!this._ctrl._isPress(v, s)) return;
        var ctrl = this._ctrl;
        if (ctrl._patchActive('colorLabBeat')) {
            ctrl._lab.beatTarget = null;
            ctrl.removePatch('colorLabBeat');
            XoneK3.debugPrint('overlayOff', { name: 'colorLabBeat' });
            return;
        }
        if (!ctrl._patchActive('colorLab')) return;
        var scheme = ctrl._commitPreview();
        if (!scheme) return;
        if (!scheme.beat) scheme.beat = { enabled: true };
        ctrl.addPatch('colorLabBeat');
        XoneK3.debugPrint('overlayOn', { name: 'colorLabBeat' });
    };

    XoneK3.FUNCS.labBeatSelect = function(args, ch, c, v, s) {
        if (!this._ctrl._isPress(v, s)) return;
        var level = args[0];
        if (!level) return;
        this._ctrl._lab.beatTarget = level;
        XoneK3.debugPrint('labBeatSelect', { level: level });
        this._ctrl._labRenderPreview();
    };

    XoneK3.FUNCS.labBeatClear = function(args, ch, c, v, s) {
        if (!this._ctrl._isPress(v, s)) return;
        var level = args[0];
        if (!level) return;
        var ctrl = this._ctrl;
        var scheme = ctrl._commitPreview();
        if (!scheme || !scheme.beat) return;
        if (level === 'beat') {
            delete scheme.beat.color;
            delete scheme.beat.brightness;
        } else {
            delete scheme.beat[level];
        }
        ctrl._lab.beatTarget = null;
        XoneK3.debugPrint('labBeatClear', { level: level });
        ctrl._labRenderPreview();
    };

    XoneK3.FUNCS.labBeatToggleEnabled = function(args, ch, c, v, s) {
        if (!this._ctrl._isPress(v, s)) return;
        var ctrl = this._ctrl;
        var scheme = ctrl._commitPreview();
        if (!scheme) return;
        if (!scheme.beat) scheme.beat = { enabled: true };
        var cur = (scheme.beat.enabled === false) ? false : true;
        scheme.beat.enabled = !cur;
        XoneK3.debugPrint('labBeatEnabled', { v: !cur });
        ctrl._labRenderPreview();
        ctrl._refreshAllLeds('lab-beat');
    };

    XoneK3.FUNCS.labBeatBarBeats = function(args, ch, c, v, s) {
        var ctrl = this._ctrl;
        var scheme = ctrl._commitPreview();
        if (!scheme) return;
        if (!scheme.beat) scheme.beat = { enabled: true };
        var cur = (typeof scheme.beat.barBeats === 'number') ? scheme.beat.barBeats : 4;
        var dir = (v === 1) ? 1 : -1;
        var next = Math.max(1, Math.min(32, cur + dir));
        scheme.beat.barBeats = next;
        XoneK3.debugPrint('labBeatBar', { v: next });
        ctrl._labRenderPreview();
    };

    XoneK3.FUNCS.labBeatPhraseBars = function(args, ch, c, v, s) {
        var ctrl = this._ctrl;
        var scheme = ctrl._commitPreview();
        if (!scheme) return;
        if (!scheme.beat) scheme.beat = { enabled: true };
        var cur = (typeof scheme.beat.phraseBars === 'number') ? scheme.beat.phraseBars : 4;
        var dir = (v === 1) ? 1 : -1;
        var next = Math.max(1, Math.min(32, cur + dir));
        scheme.beat.phraseBars = next;
        XoneK3.debugPrint('labBeatPhrase', { v: next });
        ctrl._labRenderPreview();
    };

    XoneK3.FUNCS.labBeatPrePhraseBeats = function(args, ch, c, v, s) {
        var ctrl = this._ctrl;
        var scheme = ctrl._commitPreview();
        if (!scheme) return;
        if (!scheme.beat) scheme.beat = { enabled: true };
        var bpb = scheme.beat.barBeats || 4;
        var ppb = scheme.beat.phraseBars || 4;
        var maxPre = bpb * ppb - 1;
        var cur = (typeof scheme.beat.prePhraseBeats === 'number') ? scheme.beat.prePhraseBeats : bpb;
        var dir = (v === 1) ? 1 : -1;
        var next = Math.max(0, Math.min(maxPre, cur + dir));
        scheme.beat.prePhraseBeats = next;
        XoneK3.debugPrint('labBeatPre', { v: next });
        ctrl._labRenderPreview();
    };

    // ============================================================
    // DUMP HELPERS
    // ============================================================
    // Emit format is valid JavaScript, ready to paste into
    // cat-schemes.js. Only non-default fields appear. Trailing
    // commas on scalar lines, none on the last array element —
    // valid JS, matches the codebase style.
    function _fmtDumpValue(v) {
        if (v === null || v === undefined) return 'null';
        if (Array.isArray(v)) return '[' + v.join(', ') + ']';
        if (typeof v === 'string') return "'" + v + "'";
        if (typeof v === 'object') return JSON.stringify(v);
        return String(v);
    }
    function _fmtDumpColorEntry(e) {
        if (e === null || e === undefined) return 'null';
        if (typeof e === 'string') return "'" + e + "'";
        if (Array.isArray(e)) return '[' + e.join(', ') + ']';
        if (typeof e === 'object' && e.color !== undefined) {
            var parts = ['color: ' + _fmtDumpValue(e.color)];
            if (e.ratio !== undefined) parts.push('ratio: ' + e.ratio);
            return '{ ' + parts.join(', ') + ' }';
        }
        return JSON.stringify(e);
    }
    function _fmtDumpBeatEntry(sub) {
        var parts = [];
        if (sub.color !== undefined)      parts.push('color: ' + _fmtDumpValue(sub.color));
        if (sub.brightness !== undefined) parts.push('brightness: ' + sub.brightness);
        return '{ ' + parts.join(', ') + ' }';
    }
    function _fmtDumpBeatLines(beat, indent) {
        var lines = [];
        var seen = {};
        var scalarKeys = ['color', 'brightness', 'barBeats', 'phraseBars',
                          'prePhraseBeats', 'half', 'deck'];
        var subKeys = ['bar', 'phrase', 'prePhrase'];
        var i, k;
        for (i = 0; i < scalarKeys.length; i++) {
            k = scalarKeys[i];
            if (beat[k] === undefined) continue;
            seen[k] = true;
            lines.push(indent + k + ': ' + _fmtDumpValue(beat[k]) + ',');
        }
        for (i = 0; i < subKeys.length; i++) {
            k = subKeys[i];
            if (beat[k] === undefined) continue;
            seen[k] = true;
            lines.push(indent + k + ': ' + _fmtDumpBeatEntry(beat[k]) + ',');
        }
        for (k in beat) {
            if (seen[k]) continue;
            lines.push(indent + k + ': ' + _fmtDumpValue(beat[k]) + ',');
        }
        return lines;
    }

    function _emitSchemeField(L, name, value, indent) {
        if (value === null) {
            L.push(indent + name + ': null,');
            return;
        }
        if (typeof value === 'string') {
            L.push(indent + name + ": '" + value + "',");
            return;
        }
        if (typeof value === 'object') {
            L.push(indent + name + ': {');
            _emitSchemeFields(L, value, indent + '    ');
            L.push(indent + '},');
        }
    }

    var _DUMP_SCALARS = [
        'curve', 'schemePosition', 'interp', 'calibration',
        'brightness', 'brightnessScale', 'markerPosition', 'markerWidth'
    ];

    function _emitSchemeFields(L, scheme, indent) {
        var D = XoneK3.SCHEME_DEFAULTS;
        function push(x) { L.push(indent + x); }
        push('colors: [');
        var colors = scheme.colors || [];
        for (var i = 0; i < colors.length; i++) {
            L.push(indent + '    ' + _fmtDumpColorEntry(colors[i]) +
                (i < colors.length - 1 ? ',' : ''));
        }
        push('],');
        if (scheme.ratio && scheme.ratio !== D.ratio) {
            var r = scheme.ratio.slice();
            while (r.length > 0 && r[r.length - 1] === 1) r.pop();
            if (r.length > 0) push('ratio: [' + r.join(', ') + '],');
        }
        for (var si = 0; si < _DUMP_SCALARS.length; si++) {
            var f = _DUMP_SCALARS[si];
            if (scheme[f] !== D[f]) push(f + ': ' + scheme[f] + ',');
        }
        if (scheme.markerColor !== D.markerColor) {
            push('markerColor: ' + _fmtDumpValue(scheme.markerColor) + ',');
        }
        if (scheme.offScheme !== D.offScheme && scheme.offScheme !== undefined && scheme.offScheme !== null) {
            _emitSchemeField(L, 'offScheme', scheme.offScheme, indent);
        }
        if (scheme.coScheme !== D.coScheme && scheme.coScheme !== undefined && scheme.coScheme !== null) {
            _emitSchemeField(L, 'coScheme', scheme.coScheme, indent);
        }
        if (scheme.reverse !== D.reverse) push('reverse: ' + scheme.reverse + ',');
        if (scheme.beat && scheme.beat.enabled !== false) {
            var dumpBeat = JSON.parse(JSON.stringify(scheme.beat));
            delete dumpBeat.enabled;
            if (Object.keys(dumpBeat).length > 0) {
                push('beat: {');
                var beatLines = _fmtDumpBeatLines(dumpBeat, indent + '    ');
                for (var bi = 0; bi < beatLines.length; bi++) L.push(beatLines[bi]);
                push('},');
            }
        }
    }

    function _emitSchemeLines(L, name, scheme, offOverride, coOverride) {
        var flat = Object.assign({}, scheme);
        if (offOverride !== undefined) flat.offScheme = offOverride;
        if (coOverride !== undefined)  flat.coScheme  = coOverride;
        L.push('XoneK3.SCHEMES.' + name + ' = {');
        _emitSchemeFields(L, flat, '    ');
        L.push('};');
        L.push('');
    }

    XoneK3._dump = {
        schemeLines:  _emitSchemeLines,
        schemeFields: _emitSchemeFields,
        schemeField:  _emitSchemeField,
        fmtValue:     _fmtDumpValue,
        fmtColorEntry:_fmtDumpColorEntry,
        fmtBeatLines: _fmtDumpBeatLines
    };
    XoneK3._emitSchemeLines = _emitSchemeLines;

    XoneK3.LAB_PAINTER_FUNCS = [
        XoneK3.FUNCS.labBeatSelect,
        XoneK3.FUNCS.labBeatClear,
        XoneK3.FUNCS.labBeatToggleEnabled,
        XoneK3.FUNCS.labBeatEditToggle,
        XoneK3.FUNCS.labSlotPress
    ];

    // Boot seed for slot 0 (the "on" slot). Black by design — the
    // slot grid paints slot 0 green regardless of contents, and
    // slot 0 needs to exist so it can be saved over without a
    // create step first. Lab-scoped: registered here so lib-schemes
    // stays engine-only.
    XoneK3.SCHEMES.labDefault = { colors: [[0, 0, 0]] };
})();