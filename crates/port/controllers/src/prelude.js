// The `engine`, `midi`, `controller`, `console` and `ColorMapper` objects
// the stock app gives controller scripts, built on a few native functions
// (`native`). Follows the stock mappings' script interface. `hid` is set
// for HID mappings, whose `controller` sends HID reports.
(function(native, settings, hid) {
    "use strict";

    const connections = new Map();
    // midi.makeInputHandler callbacks, by (status << 8) | control.
    const inputHandlers = new Map();
    const timers = new Map();

    const numberOrNaN = v => (typeof v === "boolean" ? (v ? 1 : 0) : Number(v));

    class Connection {
        constructor(id, group, name, callback) {
            this.id = id;
            this.group = group;
            this.name = name;
            this.callback = callback;
            this.isConnected = true;
        }
        disconnect() {
            if (this.isConnected) {
                this.isConnected = false;
                connections.delete(this.id);
                native.unwatch(this.id);
                return true;
            }
            return false;
        }
        trigger() {
            this.callback(native.get(this.group, this.name), this.group, this.name);
        }
    }

    const makeConnection = function(group, name, callback) {
        if (typeof group !== "string" || typeof name !== "string") {
            // the stock app finds no control named "undefined" and connects nothing.
            native.log(`Tried to connect to a control (${group}, ${name}) that doesn't exist`);
            return undefined;
        }
        if (typeof callback !== "function") {
            native.log(`Tried to connect (${group}, ${name}) to an invalid callback`);
            return undefined;
        }
        const id = native.watch(String(group), String(name));
        const c = new Connection(id, group, name, callback);
        connections.set(id, c);
        return c;
    };

    const toFunction = function(callback) {
        if (typeof callback === "string") {
            // Legacy scripts pass code to evaluate later.
            return new Function(`return (${callback}).apply(this, arguments);`);
        }
        return callback;
    };

    globalThis.engine = {
        getValue: (group, name) => native.get(String(group), String(name)),
        setValue(group, name, value) {
            const v = numberOrNaN(value);
            if (Number.isNaN(v)) {
                native.log(`Script tried setting (${group}, ${name}) to NaN`);
                return;
            }
            native.set(String(group), String(name), v);
        },
        getParameter: (group, name) => native.getParameter(String(group), String(name)),
        setParameter(group, name, value) {
            const v = numberOrNaN(value);
            if (!Number.isNaN(v)) {
                native.setParameter(String(group), String(name), v);
            }
        },
        getParameterForValue: (group, name, value) =>
            native.parameterForValue(String(group), String(name), numberOrNaN(value)),
        reset: (group, name) => native.reset(String(group), String(name)),
        getDefaultValue: (group, name) => native.defaultValue(String(group), String(name)),
        getDefaultParameter: (group, name) => native.defaultParameter(String(group), String(name)),
        makeConnection,
        makeUnbufferedConnection: makeConnection,
        connectControl(group, name, callback, disconnect) {
            const fn = toFunction(callback);
            if (disconnect) {
                for (const c of Array.from(connections.values())) {
                    if (c.group === group && c.name === name &&
                            (c.callback === fn || c.source === callback)) {
                        c.disconnect();
                    }
                }
                return true;
            }
            const c = makeConnection(group, name, fn);
            if (c) {
                c.source = callback;
            }
            return c;
        },
        // Runs the connections on the next poll; scripts
        // often trigger during init before what the callbacks use exists.
        trigger: (group, name) => native.trigger(String(group), String(name)),
        log: message => native.log(String(message)),
        beginTimer(interval, callback, oneShot) {
            const fn = typeof callback === "string" ? new Function(callback) : callback;
            if (typeof fn !== "function") {
                throw new Error("Invalid timer callback provided to engine.beginTimer");
            }
            const id = native.timerStart(Math.max(20, Number(interval) || 0), !!oneShot);
            timers.set(id, {fn, oneShot: !!oneShot});
            return id;
        },
        stopTimer(id) {
            if (timers.delete(id)) {
                native.timerStop(id);
            }
        },
        softTakeover: (group, name, set) => native.softTakeover(String(group), String(name), !!set),
        softTakeoverIgnoreNextValue: (group, name) =>
            native.softTakeoverIgnoreNextValue(String(group), String(name)),
        softTakeoverWillIgnore: (group, name, parameter) =>
            native.softTakeoverWillIgnore(String(group), String(name), Number(parameter)),
        scratchEnable(deck, intervalsPerRev, rpm, alpha, beta, ramp) {
            native.scratchEnable(Number(deck), Number(intervalsPerRev), Number(rpm),
                Number(alpha) || 0, Number(beta) || 0, ramp === undefined ? true : !!ramp);
        },
        scratchTick: (deck, interval) => native.scratchTick(Number(deck), Number(interval)),
        scratchDisable: (deck, ramp) => native.scratchDisable(Number(deck), ramp === undefined ? true : !!ramp),
        isScratching: deck => native.isScratching(Number(deck)),
        brake: (deck, activate, factor, rate) =>
            native.ramp(Number(deck), "brake", !!activate, factor === undefined ? 1 : Number(factor),
                rate === undefined ? 1 : Number(rate)),
        spinback: (deck, activate, factor, rate) =>
            native.ramp(Number(deck), "spinback", !!activate, factor === undefined ? 1.8 : Number(factor),
                rate === undefined ? -10 : Number(rate)),
        softStart: (deck, activate, factor) =>
            native.ramp(Number(deck), "softStart", !!activate, factor === undefined ? 1 : Number(factor), 1),
        isBrakeActive: deck => native.rampActive(Number(deck), "brake"),
        isSpinbackActive: deck => native.rampActive(Number(deck), "spinback"),
        isSoftStartActive: deck => native.rampActive(Number(deck), "softStart"),
        getSetting(name) {
            if (!(name in settings)) {
                native.log(`Unknown controllerSetting (${name}) returning undefined`);
                return undefined;
            }
            return settings[name];
        },
    };

    globalThis.midi = {
        sendShortMsg(status, byte1, byte2) {
            native.send([status & 0xFF, byte1 & 0xFF, byte2 & 0xFF]);
        },
        sendSysexMsg(data, length) {
            const bytes = Array.from(data, b => Number(b) & 0xFF);
            native.send(length === undefined ? bytes : bytes.slice(0, length));
        },
    };
    globalThis.midi.send = globalThis.midi.sendSysexMsg;
    // the stock app names the controller's script object both `midi` and `controller`.
    globalThis.controller = globalThis.midi;
    if (hid) {
        // Bytes from an array, ArrayBuffer or typed array.
        const bytesOf = data => {
            if (data instanceof ArrayBuffer) {
                return Array.from(new Uint8Array(data));
            }
            if (ArrayBuffer.isView(data)) {
                return Array.from(new Uint8Array(data.buffer, data.byteOffset, data.byteLength));
            }
            return Array.from(data || [], b => Number(b) & 0xFF);
        };
        const sendOutputReport = (reportID, data, useNonSkippingFIFO) =>
            native.hidOutput(Number(reportID) & 0xFF, bytesOf(data), !!useNonSkippingFIFO);
        // the stock HID script object.
        globalThis.controller = globalThis.midi = {
            send(dataList, length, reportID, useNonSkippingFIFO) {
                sendOutputReport(reportID || 0, dataList, useNonSkippingFIFO);
            },
            sendOutputReport,
            getInputReport: reportID =>
                new Uint8Array(native.hidGetInput(Number(reportID) & 0xFF)).buffer,
            sendFeatureReport: (reportID, data) =>
                native.hidSendFeature(Number(reportID) & 0xFF, bytesOf(data)),
            getFeatureReport: reportID =>
                new Uint8Array(native.hidGetFeature(Number(reportID) & 0xFF)).buffer,
        };
    }
    // DJ Sugar: makes note `from` on MIDI channel `channel` (0-15) act as note
    // `to`, in both directions: pressing it runs `to`'s bindings, and lights
    // sent to `to` show on it. `to === from` undoes it.
    globalThis.midi.routeNote = function(channel, from, to) {
        native.routeNote(Number(channel), Number(from), Number(to));
    };
    // calls `callback(channel, control, value, status)` for each
    // message with this status and control; returns a handle to disconnect.
    globalThis.midi.makeInputHandler = function(status, control, callback) {
        if (typeof callback !== "function") {
            throw new Error("Invalid callback provided to midi.makeInputHandler");
        }
        status = Number(status);
        control = Number(control);
        if (!(status >= 0x80 && status <= 0xFF && control >= 0 && control <= 0x7F)) {
            throw new Error("Invalid status or control passed to midi.makeInputHandler. " +
                `Please pass status >= 0x80 and control <= 0x7F. status=${status},control=${control}`);
        }
        if (!native.inputHandlerAllowed(status, control)) {
            native.log(`Ignoring anonymous JS function for status=${status},control=${control} ` +
                "because a previous XML binding exists");
            return undefined;
        }
        const key = (status << 8) | control;
        const entry = {callback};
        if (!inputHandlers.has(key)) {
            inputHandlers.set(key, []);
        }
        inputHandlers.get(key).push(entry);
        native.inputHandlers(status, control, 1);
        return {
            disconnect() {
                const list = inputHandlers.get(key) || [];
                const i = list.indexOf(entry);
                if (i < 0) {
                    return false;
                }
                list.splice(i, 1);
                native.inputHandlers(status, control, -1);
                return true;
            },
        };
    };

    const log = (...args) => native.log(args.map(a => (typeof a === "string" ? a : String(a))).join(" "));
    globalThis.console = {log, info: log, warn: log, error: log, debug: () => {}};
    globalThis.print = message => native.log(String(message));

    // Perceptual colour distance.
    const colorDistance = (a, b) => {
        const [ar, ag, ab] = [(a >> 16) & 0xFF, (a >> 8) & 0xFF, a & 0xFF];
        const [br, bg, bb] = [(b >> 16) & 0xFF, (b >> 8) & 0xFF, b & 0xFF];
        const meanRed = Math.trunc((ar + br) / 2);
        const dr = ar - br;
        const dg = ag - bg;
        const db = ab - bb;
        return Math.sqrt((((512 + meanRed) * dr * dr) >> 8) + 4 * dg * dg + (((767 - meanRed) * db * db) >> 8));
    };
    globalThis.ColorMapper = class ColorMapper {
        constructor(availableColors) {
            this.colors = Object.entries(availableColors || {}).map(([k, v]) => [Number(k), v]);
            this.cache = new Map();
        }
        nearest(colorCode) {
            if (this.cache.has(colorCode)) {
                return this.cache.get(colorCode);
            }
            let best;
            let bestDistance = Infinity;
            for (const entry of this.colors) {
                const d = colorDistance(colorCode, entry[0]);
                if (d < bestDistance) {
                    bestDistance = d;
                    best = entry;
                }
            }
            this.cache.set(colorCode, best);
            return best;
        }
        getNearestColor(colorCode) {
            const c = this.nearest(Number(colorCode))[0];
            return {red: (c >> 16) & 0xFF, green: (c >> 8) & 0xFF, blue: c & 0xFF};
        }
        getValueForNearestColor(colorCode) {
            return this.nearest(Number(colorCode))[1];
        }
    };

    // Called from Rust.
    return {
        fire(id, value) {
            const c = connections.get(id);
            if (c) {
                c.callback(value, c.group, c.name);
            }
        },
        input(status, control, value) {
            for (const entry of Array.from(inputHandlers.get((status << 8) | control) || [])) {
                entry.callback(status & 0x0F, control, value, status);
            }
        },
        timer(id) {
            const t = timers.get(id);
            if (t) {
                if (t.oneShot) {
                    timers.delete(id);
                }
                t.fn();
            }
        },
    };
})
