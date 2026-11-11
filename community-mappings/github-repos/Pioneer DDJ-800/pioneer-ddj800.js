var PioneerDdj800 = (function (exports) {
    'use strict';

    /******************************************************************************
    Copyright (c) Microsoft Corporation.

    Permission to use, copy, modify, and/or distribute this software for any
    purpose with or without fee is hereby granted.

    THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH
    REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY
    AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT,
    INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM
    LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR
    OTHER TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR
    PERFORMANCE OF THIS SOFTWARE.
    ***************************************************************************** */
    /* global Reflect, Promise, SuppressedError, Symbol */

    var extendStatics = function(d, b) {
        extendStatics = Object.setPrototypeOf ||
            ({ __proto__: [] } instanceof Array && function (d, b) { d.__proto__ = b; }) ||
            function (d, b) { for (var p in b) if (Object.prototype.hasOwnProperty.call(b, p)) d[p] = b[p]; };
        return extendStatics(d, b);
    };

    function __extends(d, b) {
        if (typeof b !== "function" && b !== null)
            throw new TypeError("Class extends value " + String(b) + " is not a constructor or null");
        extendStatics(d, b);
        function __() { this.constructor = d; }
        d.prototype = b === null ? Object.create(b) : (__.prototype = b.prototype, new __());
    }

    typeof SuppressedError === "function" ? SuppressedError : function (error, suppressed, message) {
        var e = new Error(message);
        return e.name = "SuppressedError", e.error = error, e.suppressed = suppressed, e;
    };

    var MidiControl = /** @class */ (function () {
        function MidiControl(name, scaled, callback) {
            this.name = name;
            this.scaled = scaled;
            this.callback = callback;
            this.lastValue = 0;
        }
        MidiControl.prototype.offerValue = function (name, value) {
            if (name != this.name)
                return;
            var scaledValue = this.scaled ? value / 0x7F : value;
            if (this.callback.onNewValue)
                this.callback.onNewValue(scaledValue);
            if (this.lastValue === scaledValue)
                return;
            if (this.callback.onValueChanged)
                this.callback.onValueChanged(scaledValue);
            this.lastValue = scaledValue;
        };
        return MidiControl;
    }());

    var Button = /** @class */ (function (_super) {
        __extends(Button, _super);
        function Button(name, callback) {
            return _super.call(this, name, false, {
                onNewValue: function (value) {
                    if (callback.onNewValue)
                        callback.onNewValue(value);
                },
                onValueChanged: function (value) {
                    if (value > 0) {
                        if (callback.onPressed)
                            callback.onPressed();
                    }
                    else {
                        if (callback.onReleased)
                            callback.onReleased();
                    }
                    if (callback.onValueChanged)
                        callback.onValueChanged(value);
                }
            }) || this;
        }
        return Button;
    }(MidiControl));

    var DeckMidiControl = /** @class */ (function (_super) {
        __extends(DeckMidiControl, _super);
        function DeckMidiControl(deckIndex, name, scaled, callback) {
            var _this = _super.call(this, deckIndex + name, scaled, callback) || this;
            _this.scaled = scaled;
            return _this;
        }
        return DeckMidiControl;
    }(MidiControl));

    var FineMidiControl = /** @class */ (function (_super) {
        __extends(FineMidiControl, _super);
        function FineMidiControl(name, callback) {
            var _this = _super.call(this, name, true, callback) || this;
            _this.name = name;
            _this.lastValueMsb = 0;
            _this.lastValueLsb = 0;
            _this.nameMsb = name + "Msb";
            _this.nameLsb = name + "Lsb";
            return _this;
        }
        FineMidiControl.prototype.offerValue = function (name, value) {
            var newValue;
            if (name === this.nameMsb) {
                // tslint:disable-next-line: no-bitwise
                newValue = ((value << 7) + this.lastValueLsb) / 0x3FFF;
                this.lastValueMsb = value;
            }
            else if (name === this.nameLsb) {
                // tslint:disable-next-line: no-bitwise
                newValue = ((this.lastValueMsb << 7) + value) / 0x3FFF;
                this.lastValueLsb = value;
            }
            else {
                return;
            }
            if (this.callback.onNewValue)
                this.callback.onNewValue(newValue);
            if (newValue === this.lastValue)
                return;
            if (this.callback.onValueChanged)
                this.callback.onValueChanged(newValue);
            this.lastValue = newValue;
        };
        return FineMidiControl;
    }(MidiControl));

    var DeckFineMidiControl = /** @class */ (function (_super) {
        __extends(DeckFineMidiControl, _super);
        function DeckFineMidiControl(deckIndex, name, callback) {
            return _super.call(this, deckIndex + name, callback) || this;
        }
        return DeckFineMidiControl;
    }(FineMidiControl));

    var DeckButton = /** @class */ (function (_super) {
        __extends(DeckButton, _super);
        function DeckButton(deckIndex, name, callback) {
            return _super.call(this, deckIndex + name, callback) || this;
        }
        return DeckButton;
    }(Button));

    var MidiMapping = /** @class */ (function () {
        function MidiMapping() {
        }
        MidiMapping.initReversedMapping = function () {
            for (var statusGroupKey in MidiMapping.mapping) {
                var statusGroup = MidiMapping.mapping[statusGroupKey];
                for (var midiNo in statusGroup) {
                    var controlName = statusGroup[midiNo];
                    MidiMapping.reversedMapping[controlName] = [statusGroupKey, midiNo]; // idk
                }
            }
        };
        MidiMapping.getMidiForControl = function (controlName) {
            return MidiMapping.reversedMapping[controlName];
        };
        MidiMapping.mapping = {
            0xb6: {
                0x1f: "CrossfaderMsb",
                0x3f: "CrossfaderLsb",
                0x0d: "HeadphoneMsb",
                0x2d: "HeadphoneLsb",
                0x0c: "HeadphoneMixMsb",
                0x2c: "HeadphoneMixLsb",
                0x08: "MasterMsb",
                0x28: "MasterLsb",
                0x40: "TraxEncoder",
                0x64: "TraxEncoderShifted",
                0x17: "0FilterMsb",
                0x37: "0FilterLsb",
                0x18: "1FilterMsb",
                0x38: "1FilterLsb",
            },
            0x96: {
                0x41: "TraxButton",
                0x7a: "LibraryView",
                0x65: "LibraryBack",
                0x46: "0Load",
                0x5d: "0LoadShifted",
                0x47: "1Load",
                0x6d: "1LoadShifted",
            },
            0x90: {
                0x0b: "0Play",
                0x0c: "0Cue",
                0x58: "0Sync",
                0x75: "0SyncShifted",
                0x3f: "0Shift",
                0x54: "0Pfl",
                0x4d: "0LoopButton",
                0x10: "0LoopIn",
                0x11: "0LoopOut",
                0x36: "0JogTouchButton",
                0x5e: "0SearchBackward",
                0x5f: "0SearchForward",
            },
            0xb0: {
                0x21: "0JogEncoderUntouched",
                0x22: "0JogEncoderTouched",
                0x00: "0TempoMsb",
                0x20: "0TempoLsb",
                0x04: "0GainMsb",
                0x24: "0GainLsb",
                0x07: "0EqHighMsb",
                0x27: "0EqHighLsb",
                0x0b: "0EqMidMsb",
                0x2b: "0EqMidLsb",
                0x0f: "0EqLowMsb",
                0x2f: "0EqLowLsb",
                0x13: "0VolumeMsb",
                0x33: "0VolumeLsb",
            },
            0x97: {
                0x00: "0Hotcue0",
                0x01: "0Hotcue1",
                0x02: "0Hotcue2",
                0x03: "0Hotcue3",
                0x21: "0BeatjumpBackward",
                0x22: "0BeatjumpForward",
            },
            0x98: {
                0x00: "0Hotcue0Shifted",
                0x01: "0Hotcue1Shifted",
                0x02: "0Hotcue2Shifted",
                0x03: "0Hotcue3Shifted",
            },
            0x91: {
                0x0b: "1Play",
                0x0c: "1Cue",
                0x58: "1Sync",
                0x75: "1SyncShifted",
                0x3f: "1Shift",
                0x54: "1Pfl",
                0x4d: "1LoopButton",
                0x10: "1LoopIn",
                0x11: "1LoopOut",
                0x36: "1JogTouchButton",
                0x5e: "1SearchBackward",
                0x5f: "1SearchForward",
            },
            0xb1: {
                0x21: "1JogEncoderUntouched",
                0x22: "1JogEncoderTouched",
                0x00: "1TempoMsb",
                0x20: "1TempoLsb",
                0x04: "1GainMsb",
                0x24: "1GainLsb",
                0x07: "1EqHighMsb",
                0x27: "1EqHighLsb",
                0x0b: "1EqMidMsb",
                0x2b: "1EqMidLsb",
                0x0f: "1EqLowMsb",
                0x2f: "1EqLowLsb",
                0x13: "1VolumeMsb",
                0x33: "1VolumeLsb",
            },
            0x99: {
                0x00: "1Hotcue0",
                0x01: "1Hotcue1",
                0x02: "1Hotcue2",
                0x03: "1Hotcue3",
                0x21: "1BeatjumpBackward",
                0x22: "1BeatjumpForward",
            },
            0x9a: {
                0x00: "1Hotcue0Shifted",
                0x01: "1Hotcue1Shifted",
                0x02: "1Hotcue2Shifted",
                0x03: "1Hotcue3Shifted",
            },
        };
        MidiMapping.reversedMapping = {};
        return MidiMapping;
    }());

    function clamp(value, min, max) {
        return value <= min ? min : value >= max ? max : value;
    }
    function toggleControl(channel, key) {
        engine.setValue(channel, key, !engine.getValue(channel, key));
    }
    function activate(channel, key) {
        engine.setValue(channel, key, 1);
    }
    function makeLedConnection(channel, key, midiLedStatus, midiLedNo, ledValue) {
        if (ledValue === void 0) { ledValue = 0x7f; }
        return engine.makeConnection(channel, key, function (value) {
            midi.sendShortMsg(midiLedStatus, midiLedNo, value * ledValue);
        });
    }
    function setLed(controlName, value) {
        var _a = MidiMapping.getMidiForControl(controlName), status = _a[0], midiNo = _a[1];
        midi.sendShortMsg(status, midiNo, value);
    }

    var Deck = /** @class */ (function () {
        function Deck(channel) {
            var _this = this;
            this.channel = channel;
            this.connections = [];
            this.index = channel - 1;
            this.group = "[Channel".concat(channel, "]");
            var eqGroup = "[EqualizerRack1_".concat(this.group, "_Effect1]");
            var filterEffectGroup = "[QuickEffectRack1_".concat(this.group, "]");
            this.controls = [
                new DeckButton(this.index, "Play", {
                    onPressed: function () {
                        _this.toggleControl("play");
                    },
                }),
                new DeckButton(this.index, "Sync", {
                    onPressed: function () {
                        _this.activate("beatsync");
                    },
                }),
                new DeckButton(this.index, "Pfl", {
                    onPressed: function () {
                        _this.toggleControl("pfl");
                    },
                }),
                // Loop
                new DeckButton(this.index, "LoopButton", {
                    onPressed: function () {
                        _this.activate("beatloop_".concat(_this.getValue("beatloop_size"), "_toggle"));
                    },
                }),
                // Loop size
                new DeckButton(this.index, "LoopIn", {
                    onPressed: function () {
                        _this.activate("loop_halve");
                    },
                }),
                new DeckButton(this.index, "LoopOut", {
                    onPressed: function () {
                        _this.activate("loop_double");
                    },
                }),
                // Gain
                new DeckFineMidiControl(this.index, "Gain", {
                    onValueChanged: function (value) {
                        _this.setParameter("pregain", value);
                    },
                }),
                // EQ
                new DeckFineMidiControl(this.index, "EqLow", {
                    onValueChanged: function (value) {
                        engine.setParameter(eqGroup, "parameter1", value);
                    },
                }),
                new DeckFineMidiControl(this.index, "EqMid", {
                    onValueChanged: function (value) {
                        engine.setParameter(eqGroup, "parameter2", value);
                    },
                }),
                new DeckFineMidiControl(this.index, "EqHigh", {
                    onValueChanged: function (value) {
                        engine.setParameter(eqGroup, "parameter3", value);
                    },
                }),
                // Quick Effect / Filter
                new DeckFineMidiControl(this.index, "Filter", {
                    onValueChanged: function (value) {
                        engine.setParameter(filterEffectGroup, "super1", value);
                    },
                }),
                new DeckFineMidiControl(this.index, "Volume", {
                    onValueChanged: function (value) {
                        _this.setParameter("volume", value);
                    },
                }),
                // Beatjump
                new DeckButton(this.index, "BeatjumpBackward", {
                    onPressed: function () {
                        _this.activate("beatjump_backward");
                    },
                }),
                new DeckButton(this.index, "BeatjumpForward", {
                    onPressed: function () {
                        _this.activate("beatjump_forward");
                    },
                }),
                // Beatjump size
                new DeckButton(this.index, "SearchBackward", {
                    onPressed: function () {
                        _this.modifyAndClampBeatjumpSize(0.5);
                    },
                }),
                new DeckButton(this.index, "SearchForward", {
                    onPressed: function () {
                        _this.modifyAndClampBeatjumpSize(2);
                    },
                }),
                // Jog wheel
                new DeckButton(this.index, "JogTouchButton", {
                    onPressed: function () {
                        var alpha = 1.0 / 8;
                        var beta = alpha / 32;
                        engine.scratchEnable(channel, 16384, 33 + 1 / 3, alpha, beta, false);
                    },
                    onReleased: function () {
                        engine.scratchDisable(channel, false);
                    },
                }),
                new DeckMidiControl(this.index, "JogEncoderTouched", false, {
                    onNewValue: function (value) {
                        if (engine.isScratching(_this.channel)) {
                            var centeredValue = value - 0x40;
                            engine.scratchTick(_this.channel, centeredValue * 2);
                        }
                    },
                }),
                new DeckMidiControl(this.index, "JogEncoderUntouched", false, {
                    onNewValue: function (value) {
                        if (!engine.isScratching(_this.channel)) {
                            var centeredValue = value - 0x40;
                            _this.setParameter("jog", centeredValue * 0.25);
                        }
                    },
                }),
            ];
            this.rateControl = new DeckFineMidiControl(this.index, "Tempo", {
                onValueChanged: function (value) {
                    _this.setParameter("rate", 1 - value);
                },
            });
            this.controls.push(this.rateControl);
            // Hotcues
            var hotcueIndices = [0, 1, 2, 3];
            hotcueIndices.forEach(function (hotcueIndex) {
                var hotcueNumber = hotcueIndex + 1;
                _this.controls.push(new DeckButton(_this.index, "Hotcue".concat(hotcueIndex), {
                    onValueChanged: function (pressed) {
                        _this.setValue("hotcue_".concat(hotcueNumber, "_activate"), pressed);
                    },
                }));
                _this.controls.push(new DeckButton(_this.index, "Hotcue".concat(hotcueIndex, "Shifted"), {
                    onPressed: function () {
                        _this.activate("hotcue_".concat(hotcueNumber, "_clear"));
                    },
                }));
                _this.makeLedConnection("hotcue_".concat(hotcueNumber, "_enabled"), "Hotcue".concat(hotcueIndex), 0x02); // blue
                _this.makeLedConnection("hotcue_".concat(hotcueNumber, "_enabled"), "Hotcue".concat(hotcueIndex, "Shifted"), 0x29); // red
            });
            // Load track
            this.controls.push(new DeckButton(this.index, "Load", {
                onPressed: function () {
                    _this.activate("LoadSelectedTrack");
                },
            }));
            // Eject track
            this.controls.push(new DeckButton(this.index, "LoadShifted", {
                onPressed: function () {
                    if (!_this.getValue("play"))
                        _this.activate("eject");
                },
            }));
            // SoftTakeover
            engine.softTakeover(this.group, "rate", true);
            // Leds
            this.makeLedConnection("play", "Play");
            this.makeLedConnection("pfl", "Pfl");
            this.makeLedConnection("loop_enabled", "LoopButton");
            // Beatjump buttons
            setLed("".concat(this.index, "BeatjumpBackward"), 0x15); // green
            setLed("".concat(this.index, "BeatjumpForward"), 0x15);
            setLed("".concat(this.index, "LoopIn"), 0x7F);
            setLed("".concat(this.index, "LoopOut"), 0x7F);
            setLed("".concat(this.index, "Sync"), 0x7F);
            this.triggerConnections();
        }
        Deck.prototype.triggerConnections = function () {
            for (var _i = 0, _a = this.connections; _i < _a.length; _i++) {
                var connection = _a[_i];
                connection.trigger();
            }
        };
        Deck.prototype.modifyAndClampBeatjumpSize = function (factor) {
            this.setValue("beatjump_size", clamp(this.getValue("beatjump_size") * factor, 0.03125, 128));
        };
        Deck.prototype.getParameter = function (key) {
            return engine.getParameter(this.group, key);
        };
        Deck.prototype.setParameter = function (key, value) {
            engine.setParameter(this.group, key, value);
        };
        Deck.prototype.getValue = function (key) {
            return engine.getValue(this.group, key);
        };
        Deck.prototype.setValue = function (key, value) {
            engine.setValue(this.group, key, value);
        };
        Deck.prototype.activate = function (key) {
            activate(this.group, key);
        };
        Deck.prototype.toggleControl = function (key) {
            toggleControl(this.group, key);
        };
        Deck.prototype.makeConnection = function (key, callback) {
            this.connections.push(engine.makeConnection(this.group, key, callback));
        };
        Deck.prototype.makeLedConnection = function (key, controlName, ledValue) {
            if (ledValue === void 0) { ledValue = 0x7f; }
            var _a = MidiMapping.getMidiForControl("".concat(this.index).concat(controlName)), status = _a[0], midiNo = _a[1];
            this.connections.push(makeLedConnection(this.group, key, status, midiNo, ledValue));
        };
        return Deck;
    }());

    var decks;
    var deckIndependentControls;
    var controls = [];
    function init() {
        MidiMapping.initReversedMapping();
        decks = [1, 2].map(function (channel) { return new Deck(channel); });
        var tracklistSelected = false;
        deckIndependentControls = [
            new FineMidiControl("Crossfader", {
                onValueChanged: function (value) {
                    engine.setParameter("[Master]", "crossfader", value);
                },
            }),
            new Button("TraxButton", {
                onPressed: function () {
                    tracklistSelected = !tracklistSelected;
                },
            }),
            new Button("LibraryView", {
                onPressed: function () {
                    toggleControl("[Master]", "maximize_library");
                },
            }),
        ];
        function traxControl(name, factor) {
            return new MidiControl(name, false, {
                onNewValue: function (value) {
                    var direction = value > 0x40 ? value - 0x80 : value;
                    if (tracklistSelected) {
                        engine.setValue("[Playlist]", "SelectTrackKnob", direction * factor);
                    }
                    else {
                        engine.setValue("[Playlist]", "SelectPlaylist", direction * factor);
                    }
                },
            });
        }
        deckIndependentControls.push(traxControl("TraxEncoder", 1));
        deckIndependentControls.push(traxControl("TraxEncoderShifted", 5));
        registerControls(deckIndependentControls);
        for (var _i = 0, decks_1 = decks; _i < decks_1.length; _i++) {
            var deck = decks_1[_i];
            registerControls(deck.controls);
        }
        midi.sendShortMsg(0x9F, 0x09, 0x7F);
    }
    function midiInput(channel, midiNo, value, status, group) {
        //engine.log(`Channel ${channel}, MidiNo: ${midiNo}, Value: ${value}, Status: ${status}, Group: ${group}`);
        var controlName = MidiMapping.mapping[status][midiNo];
        if (controlName == null)
            return;
        //engine.log(`${controlName}: ${value}`);
        for (var _i = 0, controls_1 = controls; _i < controls_1.length; _i++) {
            var control = controls_1[_i];
            control.offerValue(controlName, value);
        }
    }
    function registerControls(newControls) {
        controls.push.apply(controls, newControls);
    }

    exports.init = init;
    exports.midiInput = midiInput;

    return exports;

})({});
