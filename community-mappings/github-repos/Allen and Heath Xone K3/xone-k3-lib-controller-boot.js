// xone-k3-lib-controller-boot.js — init / shutdown, MIDI wiring,
// boot, halt.
//
// Loaded after lib-controller-render.js. Extends
// K3Controller.prototype. Defines XoneK3.boot / bootLab / halt.

'use strict';

if (typeof XoneK3 === 'undefined') {
    throw new Error('Xone K3: xone-k3-lib-controller-boot.js — xone-k3-lib-core.js must load first');
}
if (typeof XoneK3._requireLoad !== 'function') {
    throw new Error('Xone K3: xone-k3-lib-controller-boot.js — xone-k3-lib-core.js must load first');
}
XoneK3._requireLoad('xone-k3-lib-controller-boot.js', ['K3Controller']);

(function() {
    // ============================================================
    // INIT / SHUTDOWN
    // ============================================================
    XoneK3.K3Controller.prototype.init = function() {
        this._blankLeds();
        this._recompute();
        var self = this;
        this._initExtra();
        this._validateBindings();

        // Delayed beat-clock reopen. The initial _recompute can
        // race the CO tree on Mixxx startup — connections
        // register but never fire. 250 ms lets the tree settle.
        engine.beginTimer(250, function() {
            if (self._shuttingDown) return;
            self._closeBeatClocks();
            self._syncBeatClocks();
            self._syncFxLayout();
            self._startBootSettle();
        }, true);
    };
    XoneK3.K3Controller.prototype._initExtra = function() {};
    XoneK3.K3Controller.prototype._shutdownExtra = function() {};

    // Sets _shuttingDown (blocks every other send path),
    // enqueues the blanks, then polls until the queue drains
    // before tearing down beat clocks. _sendSysExLED's drain
    // timer has no _shuttingDown bail — it runs to completion on
    // purpose, and this poll relies on that.
    XoneK3.K3Controller.prototype.shutdown = function() {
        if (this._shuttingDown) return;
        this._shuttingDown = true;

        this._blankLeds();

        var self = this;
        var elapsedWait = 0;
        var tick = 20;
        var maxWait = 2000;

        function finalize() {
            if (self._sysexQueue.length > 0 && elapsedWait < maxWait) {
                elapsedWait += tick;
                engine.beginTimer(tick, finalize, true);
                return;
            }
            self._stopSysExDrain();
            self._closeBeatClocks();
            self._shutdownExtra();
        }

        engine.beginTimer(tick, finalize, true);
    };

    // ============================================================
    // MIDI WIRING
    // ============================================================
    XoneK3._wireInputs = function(ctrl) {
        var noteOff = 0x80 + ctrl.midiChannel;
        for (var name in ctrl.bindings) {
            if (!ctrl.bindings.hasOwnProperty(name)) continue;
            (function(b) {
                function cb(_ch, _cc, v, s) {
                    if (b.input) b.input(_ch, _cc, v, s);
                }
                midi.makeInputHandler(b.midi[0], b.midi[1], cb);
                if ((b.midi[0] & 0xF0) === 0x90) {
                    midi.makeInputHandler(noteOff, b.midi[1], cb);
                }
            })(ctrl.bindings[name]);
        }
    };

    // ============================================================
    // BOOT / HALT
    // ============================================================
    XoneK3.controllers = [];

    XoneK3._bootWith = function(devices, Ctor, label) {
        print('\n=== Xone K3 INIT' + (label ? ' (' + label + ')' : '') + ' ===');
        // Shut down any controllers from a previous boot (console
        // init() without a preset reload). Mixxx's controller
        // framework calls shutdown before init on preset reload,
        // so this only matters for manual re-init — but the old
        // controllers' timers and CO connections would otherwise
        // leak and keep firing alongside the new ones.
        XoneK3.controllers.forEach(function(c) { c.shutdown(); });
        XoneK3.controllers = [];
        XoneK3._schemeCache = {};
        XoneK3._compiledCache = {};
        XoneK3._warned = Object.create(null);
        XoneK3._modeSeen = { edit: false, slots: false };
        XoneK3.MODE_PRINTS = false;
        XoneK3._funcIdCounter = 0;
        XoneK3._funcIdMap = new WeakMap();
        XoneK3._segsCacheCounter = 0;

        if (!Array.isArray(devices)) devices = [devices];
        XoneK3.TIME_SIG = (devices[0] && devices[0].timeSig)
            ? devices[0].timeSig : null;
        devices.forEach(function(d) {
            print('  spawning ' + d.label + ' ch 0x' + d.midiChannel.toString(16));
            try {
                var ctrl = new Ctor(d);
                ctrl.init();
                XoneK3._wireInputs(ctrl);
                XoneK3.controllers.push(ctrl);
            } catch (e) {
                XoneK3._warnOnce('bootDeviceThrow:' + (d.label || '?'),
                    'device "' + (d.label || '?') + '" failed to boot: ' +
                    (e && e.message ? e.message : e));
            }
        });
        XoneK3._auditSnapshotFields();
    };

    XoneK3.boot    = function(devices) { XoneK3._bootWith(devices, XoneK3.K3Controller, ''); };
    XoneK3.bootLab = function(devices) { XoneK3._bootWith(devices, XoneK3.LabController, 'colorLab'); };

    XoneK3.halt = function() {
        print('\n=== Xone K3 SHUTDOWN ===');
        XoneK3.controllers.forEach(function(ctrl) { ctrl.shutdown(); });
        XoneK3.controllers = [];
        print('=== shutdown requested (teardown continues async) ===');
    };
})();