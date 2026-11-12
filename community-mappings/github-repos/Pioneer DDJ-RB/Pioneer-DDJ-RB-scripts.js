var PioneerDDJRB = {};

///////////////////////////////////////////////////////////////
//                       USER OPTIONS                        //
///////////////////////////////////////////////////////////////

// If true, vinyl mode will be enabled when Mixxx starts.
PioneerDDJRB.vinylModeOnStartup = true;

// If true, pressing shift + cue will play the track in reverse and enable slip mode,
// which can be used like a censor effect. If false, pressing shift + cue jumps to
// the beginning of the track and stops playback.
PioneerDDJRB.reverseRollOnShiftCue = false;

// Sets the jogwheels sensitivity. 1 is default, 2 is twice as sensitive, 0.5 is half as sensitive.
PioneerDDJRB.jogwheelSensitivity = 5.0;

// Sets how much more sensitive the jogwheels get when holding shift.
// Set to 1 to disable jogwheel sensitivity increase when holding shift.
PioneerDDJRB.jogwheelShiftMultiplier = 20;

// If true Level-Meter shows VU-Master left & right. If false shows level of active deck.
PioneerDDJRB.showVumeterMaster = false;

// If true VU-Level twinkle if AutoDJ is ON.
PioneerDDJRB.twinkleVumeterAutodjOn = true;

// If true, releasing browser knob jumps forward to jumpPreviewPosition.
PioneerDDJRB.jumpPreviewEnabled = true;
// Position in the track to jump to. 0 is the beginning of the track and 1 is the end.
PioneerDDJRB.jumpPreviewPosition = 0.5;

PioneerDDJRB.looprollIntervals = [1 / 16, 1 / 8, 1 / 4, 1 / 2, 1, 2, 4, 8];

/*
    Pioneer DDJ-RB mapping for Mixxx
    Copyright (c) 2025 Luis Mita, licensed under GPL version 2 or later
    Copyright (c) 2017 Be (be.0@gmx.com), licensed under GPL version 2 or later
    Copyright (c) 2014-2015 various contributors, licensed under MIT license

    Contributors and change log:
    - Fix Loop Buttons, Effect Knobs and Jog Wheel
    - Imported from DDJ-SB3 template

    TODO:
    - Test all controls on DDJ-RB hardware
*/

PioneerDDJRB.PadMode = {
    HotCue: 0x1B,
    FxFadeMix: 0x1E,
    PadScratch: 0x20,
    Sampler: 0x22,
    BeatJump: 0x69,
    Roll: 0x6B,
    Slicer: 0x6D,
    Trans: 0x6E
};

///////////////////////////////////////////////////////////////
//               INIT, SHUTDOWN & GLOBAL HELPER              //
///////////////////////////////////////////////////////////////
PioneerDDJRB.trackLoaded = function(value, group) {
    var deckIndex = PioneerDDJRB.channelGroups[group];

    if (value) {
        midi.sendShortMsg(0x96, 0x46 + deckIndex, 0x7F);
    } else {
        midi.sendShortMsg(0x96, 0x46 + deckIndex, 0x0);
    }
};

// When unloading a deck, it should pass 0 for BPM
//
// Controller should know track BPM to match the TRANS pads velocity
PioneerDDJRB.updateBPM = function(bpm, group) {
    var bpmValue = Math.round(bpm * 100);
    var bpmBits = bpmValue.toString(2);

    var bpmBitsPadded = [];

    var offset = 16 - bpmBits.length;

    var i;

    for (i = 0; i < 16; i++) {
        if (i < offset) {
            bpmBitsPadded[i] = "0";
        } else {
            bpmBitsPadded[i] = bpmBits[i - offset];
        }
    }

    var bytes = [];

    for (i = 0; i < 4; i++) {
        var mbyte = 0;

        for (var j = 0; j < 4; j++) {
            var bitIndex = (i * 4) + j;
            var bit = parseInt(bpmBitsPadded[bitIndex]);
            mbyte = mbyte | (bit << (3 - j));
        }

        bytes[i] = mbyte;
    }

    var deckIndex = PioneerDDJRB.channelGroups[group];
    var deckByte = 0x11 + deckIndex;

    var sysexMessage = [0xF0, 0x00, 0x20, 0x7F, deckByte, 0x00, 0x00, bytes[0], bytes[1], bytes[2], bytes[3], 0xF7];
    midi.sendSysexMsg(sysexMessage, sysexMessage.length);
};

PioneerDDJRB.longButtonPress = false;

PioneerDDJRB.flasher = {};

PioneerDDJRB.flasher.functions = [];

PioneerDDJRB.flasher.init = function() {
    var flag = true;

    PioneerDDJRB.flasher.timer = engine.beginTimer(500, () => {
        flag = !flag;

        for (var i = 0; i < PioneerDDJRB.flasher.functions.length; i++) {
            PioneerDDJRB.flasher.functions[i](flag);
        }
    });
};

PioneerDDJRB.flasher.shutdown = function() {
    engine.stopTimer(PioneerDDJRB.flasher.timer);
};

PioneerDDJRB.flasher.addFunction = function(fn) {
    PioneerDDJRB.flasher.functions.push(fn);
};

PioneerDDJRB.flasher.removeFunction = function(fn) {
    PioneerDDJRB.flasher.functions = _.filter(PioneerDDJRB.flasher.functions, function(f) {
        return fn !== f;
    });
};

PioneerDDJRB.midiOutputBeatLedsStart = 0x60;
PioneerDDJRB.midiOutputBeatLedsCount = 8;

PioneerDDJRB.scratchSettings = {
    "alpha": 1.0 / 8,
    "beta": 1.0 / 8 / 32,
    "jogResolution": 720,
    "vinylSpeed": 33 + 1 / 3
};

PioneerDDJRB.channelGroups = {
    "[Channel1]": 0x00,
    "[Channel2]": 0x01
};

PioneerDDJRB.samplerGroups = {
    "[Sampler1]": {channels: ["[Channel1]"], ledNumber: 0x00},
    "[Sampler2]": {channels: ["[Channel1]"], ledNumber: 0x01},
    "[Sampler3]": {channels: ["[Channel1]"], ledNumber: 0x02},
    "[Sampler4]": {channels: ["[Channel1]"], ledNumber: 0x03},
    "[Sampler5]": {channels: ["[Channel2]"], ledNumber: 0x00},
    "[Sampler6]": {channels: ["[Channel2]"], ledNumber: 0x01},
    "[Sampler7]": {channels: ["[Channel2]"], ledNumber: 0x02},
    "[Sampler8]": {channels: ["[Channel2]"], ledNumber: 0x03},
};

PioneerDDJRB.ledGroups = {
    "hotCue": 0x00,
    "fxFade": 0x10,
    "padScratch": 0x20,
    "sampler": 0x30,
    "roll": 0x50,
};

PioneerDDJRB.nonPadLeds = {
    "headphoneCue": 0x54,
    "shiftHeadphoneCue": 0x68,
    "cue": 0x0C,
    "shiftCue": 0x48,
    "keyLock": 0x1A,
    "shiftKeyLock": 0x60,
    "play": 0x0B,
    "shiftPlay": 0x47,
    "vinyl": 0x17,
    "shiftVinyl": 0x40,
    "sync": 0x58,
    "shiftSync": 0x5C,
    "autoLoop": 0x14,
    "shiftAutoLoop": 0x50,
    "loopIn": 0x10,
    "loopOut": 0x11,
};

PioneerDDJRB.loopInBlinking = {
    "[Channel1]": false,
    "[Channel2]": false,
};

PioneerDDJRB.channelsToPadNumber = {
    "[Channel1]": 1,
    "[Channel2]": 2
};

PioneerDDJRB.channelsToEffectUnitNumber = {
    "[Channel1]": 1,
    "[Channel2]": 2
};

PioneerDDJRB.init = function() {
    var initSysBytes = [0xF0, 0x00, 0x20, 0x7F, 0x03, 0x01, 0xF7];
    midi.sendSysexMsg(initSysBytes, initSysBytes.length);

    PioneerDDJRB.shiftPressed = false;

    PioneerDDJRB.chFaderStart = [
        null,
        null
    ];

    PioneerDDJRB.scratchMode = [false, false];

    PioneerDDJRB.valueVuMeter = {
        "[Channel1]_current": 0,
        "[Channel2]_current": 0,
        "[Channel1]_enabled": 1,
        "[Channel2]_enabled": 1,
    };

    if (engine.getValue("[App]", "num_samplers") < 8) {
        engine.setValue("[App]", "num_samplers", 8);
    }

    PioneerDDJRB.deck = [];
    PioneerDDJRB.deck[1] = new PioneerDDJRB.Deck(1);
    PioneerDDJRB.deck[2] = new PioneerDDJRB.Deck(2);

    PioneerDDJRB.effectUnit = [];
    PioneerDDJRB.effectUnit[1] = new PioneerDDJRB.EffectUnit(1);
    PioneerDDJRB.effectUnit[2] = new PioneerDDJRB.EffectUnit(2);

    PioneerDDJRB.padForDeck = [];
    PioneerDDJRB.padForDeck[1] = new PioneerDDJRB.Pad(1);
    PioneerDDJRB.padForDeck[2] = new PioneerDDJRB.Pad(2);

    PioneerDDJRB.bindNonDeckControlConnections(false);
    PioneerDDJRB.initDeck("[Channel1]");
    PioneerDDJRB.initDeck("[Channel2]");

    if (PioneerDDJRB.twinkleVumeterAutodjOn) {
        PioneerDDJRB.vuMeterTimer = engine.beginTimer(100, PioneerDDJRB.vuMeterTwinkle);
    }

    // request the positions of the knobs and faders from the controller
    midi.sendShortMsg(0x9B, 0x09, 0x7f);

    PioneerDDJRB.flasher.addFunction(function(flag) {
        for (var i = 1; i <= 2; i++) {
            var group = "[Channel" + i + "]";
            if (PioneerDDJRB.loopInBlinking[group] && !engine.getValue(group, "loop_enabled")) {
                PioneerDDJRB.nonPadLedControl(group, PioneerDDJRB.nonPadLeds.loopIn, flag);
                PioneerDDJRB.nonPadLedControl(group, PioneerDDJRB.nonPadLeds.loopOut, false);
            }
        }
    });

    PioneerDDJRB.flasher.init();
    PioneerDDJRB.initFlashingPadLedControl();
};

PioneerDDJRB.shiftListeners = [];

PioneerDDJRB.Deck = function(deckNumber) {
    var theDeck = this;
    this.group = "[Channel" + deckNumber + "]";

    this.shiftButton = function(channel, control, value, status, group) {
        var i;
        if (value > 0) {
            theDeck.shift();
            PioneerDDJRB.shiftPressed = true;
            PioneerDDJRB.chFaderStart[deckNumber] = null;
            for (i = 0; i < PioneerDDJRB.shiftListeners.length; i++) {
                PioneerDDJRB.shiftListeners[i](group, true);
            }
        } else {
            theDeck.unshift();
            PioneerDDJRB.shiftPressed = false;
            for (i = 0; i < PioneerDDJRB.shiftListeners.length; i++) {
                PioneerDDJRB.shiftListeners[i](group, false);
            }
        }
    };
    this.playButton = new components.PlayButton({
        midi: [0x90 + deckNumber - 1, 0x0B],
        shiftOffset: 60,
        shiftControl: true,
        sendShifted: true
    });

    this.cueButton = new components.CueButton({
        midi: [0x90 + deckNumber - 1, 0x0C],
        shiftOffset: 60,
        shiftControl: true,
        sendShifted: true,
        reverseRollOnShift: PioneerDDJRB.reverseRollOnShiftCue,
    });

    this.syncButton = new components.SyncButton({
        midi: [0x90 + deckNumber - 1, 0x58],
        shiftOffset: 4,
        shiftControl: true,
        sendShifted: true,
    });

    var effectUnitNumber = deckNumber;

    // The Mixxx UI call this Gain, but on the controller the knob is labeled TRIM
    this.gainKnob = new components.Pot({
        unshift: function() {
            this.group = "[Channel" + deckNumber + "]";
            this.inKey = "pregain";
            this.disconnect();
            this.connect();
        }
    });

    this.eqKnob = [];
    for (var k = 1; k <= 3; k++) {
        this.eqKnob[k] = new components.Pot({
            number: k,
            unshift: function() {
                this.group = "[EqualizerRack1_[Channel" + deckNumber + "]_Effect1]";
                this.inKey = "parameter" + this.number;
                this.disconnect();
                this.connect();
            }
        });
    }

    this.quickEffectKnob = new components.Pot({
        unshift: function() {
            this.group = "[QuickEffectRack1_[Channel" + deckNumber + "]]";
            this.inKey = "super1";
            this.disconnect();
            this.connect();
        },
        shift: function() {
            var focusedEffect = engine.getValue("[EffectRack1_EffectUnit" + effectUnitNumber + "]", "focused_effect");
            this.group = "[EffectRack1_EffectUnit" + effectUnitNumber + "_Effect" + focusedEffect + "]";
            this.inKey = "parameter5";
            this.disconnect();
            this.connect();
        },
    });

    this.tempoFader = new components.Pot({
        inKey: "rate",
        invert: true,
    });

    this.forEachComponent(function(c) {
        if (c.group === undefined) {
            c.group = theDeck.group;
            c.connect();
            c.trigger();
        }
    });

    engine.setValue("[Channel" + deckNumber + "]", "rate_dir", -1);

    this.loadConnection = engine.makeConnection("[Channel" + deckNumber + "]", "track_loaded", PioneerDDJRB.trackLoaded);
    this.bpmConnection = engine.makeConnection("[Channel" + deckNumber + "]", "bpm", PioneerDDJRB.updateBPM);
    this.bpmConnection.trigger();
};
PioneerDDJRB.Deck.prototype = components.ComponentContainer.prototype;

PioneerDDJRB.Pad = function(padNumber) {
    var _this = this;

    this.padNumber = padNumber;

    this.slicerButtons = [];

    for (var i = 1; i <= PioneerDDJRB.midiOutputBeatLedsCount; i++) {
        (function(beat) {
            _this.slicerButtons[beat] = function(channel, control, value, status) {
                if (_this.slicer) {
                    _this.slicer.buttons[beat](channel, control, value, status);
                }
            };
        })(i);
    }

    // Change BeatJump leds when shifted
    PioneerDDJRB.shiftListeners.push(function(group, isShifted) {
        if (PioneerDDJRB.channelsToPadNumber[group] === padNumber) {
            if (isShifted) {
                for (var i = 0; i < 8; i++) {
                    midi.sendShortMsg(0x97 + padNumber - 1, 0x40 + i, 0x7F);
                }
            } else {
                midi.sendShortMsg(0x97 + padNumber - 1, 0x40, 0x0);
                midi.sendShortMsg(0x97 + padNumber - 1, 0x41, 0x0);
                midi.sendShortMsg(0x97 + padNumber - 1, 0x42, 0x7F);
                midi.sendShortMsg(0x97 + padNumber - 1, 0x43, 0x7F);
                midi.sendShortMsg(0x97 + padNumber - 1, 0x44, 0x0);
                midi.sendShortMsg(0x97 + padNumber - 1, 0x45, 0x0);
                midi.sendShortMsg(0x97 + padNumber - 1, 0x46, 0x0);
                midi.sendShortMsg(0x97 + padNumber - 1, 0x47, 0x0);
            }
        }
    });
};

PioneerDDJRB.Pad.prototype.setModeActive = function(activeMode) {
    midi.sendShortMsg(0x90 + this.padNumber - 1, PioneerDDJRB.PadMode.HotCue, activeMode === PioneerDDJRB.PadMode.HotCue ? 0x7F : 0x0);
    midi.sendShortMsg(0x90 + this.padNumber - 1, PioneerDDJRB.PadMode.FxFadeMix, activeMode === PioneerDDJRB.PadMode.FxFadeMix ? 0x7F : 0x0);
    midi.sendShortMsg(0x90 + this.padNumber - 1, PioneerDDJRB.PadMode.PadScratch, activeMode === PioneerDDJRB.PadMode.PadScratch ? 0x7F : 0x0);
    midi.sendShortMsg(0x90 + this.padNumber - 1, PioneerDDJRB.PadMode.Sampler, activeMode === PioneerDDJRB.PadMode.Sampler ? 0x7F : 0x0);
    midi.sendShortMsg(0x90 + this.padNumber - 1, PioneerDDJRB.PadMode.BeatJump, activeMode === PioneerDDJRB.PadMode.BeatJump ? 0x7F : 0x0);
    midi.sendShortMsg(0x90 + this.padNumber - 1, PioneerDDJRB.PadMode.Roll, activeMode === PioneerDDJRB.PadMode.Roll ? 0x7F : 0x0);
    midi.sendShortMsg(0x90 + this.padNumber - 1, PioneerDDJRB.PadMode.Slicer, activeMode === PioneerDDJRB.PadMode.Slicer ? 0x7F : 0x0);
    midi.sendShortMsg(0x90 + this.padNumber - 1, PioneerDDJRB.PadMode.Trans, activeMode === PioneerDDJRB.PadMode.Trans ? 0x7F : 0x0);
};

PioneerDDJRB.Pad.prototype.clearSlicer = function() {
    if (this.slicer) {
        this.slicer.shutdown();
        this.slicer = null;
    }
};

PioneerDDJRB.Pad.prototype.hotcueMode = function(channel, control, value) {
    if (value) {
        this.setModeActive(PioneerDDJRB.PadMode.HotCue);
        this.clearSlicer();
    }
};

PioneerDDJRB.Pad.prototype.beatJumpMode = function(channel, control, value) {
    if (value) {
        this.setModeActive(PioneerDDJRB.PadMode.BeatJump);
        this.clearSlicer();

        // Let jump pad led on
        midi.sendShortMsg(0x97 + this.padNumber - 1, 0x42, 0x7F);
        midi.sendShortMsg(0x97 + this.padNumber - 1, 0x43, 0x7F);
    }
};

PioneerDDJRB.Pad.prototype.fxFadeMode = function(channel, control, value) {
    if (value) {
        this.setModeActive(PioneerDDJRB.PadMode.FxFadeMix);
        this.clearSlicer();
    }
};

PioneerDDJRB.Pad.prototype.rollMode = function(channel, control, value) {
    if (value) {
        this.setModeActive(PioneerDDJRB.PadMode.Roll);
        this.clearSlicer();
    }
};

PioneerDDJRB.Pad.prototype.padScratchMode = function(channel, control, value) {
    if (value) {
        this.setModeActive(PioneerDDJRB.PadMode.PadScratch);
        this.clearSlicer();
    }
};

PioneerDDJRB.Pad.prototype.slicerMode = function(channel, control, value) {
    if (value) {
        this.setModeActive(PioneerDDJRB.PadMode.Slicer);

        if (!this.slicer) {
            var group = "[Channel" + this.padNumber + "]";
            var midiOutputOp = 0x97 + this.padNumber - 1;

            this.slicer = new PioneerDDJRB.Slicer(group, midiOutputOp);
        }
    }
};

PioneerDDJRB.Pad.prototype.samplerMode = function(channel, control, value) {
    if (value) {
        this.setModeActive(PioneerDDJRB.PadMode.Sampler);
        this.clearSlicer();
    }
};

PioneerDDJRB.Pad.prototype.transMode = function(channel, control, value) {
    if (value) {
        this.setModeActive(PioneerDDJRB.PadMode.Trans);
        this.clearSlicer();
    }
};

PioneerDDJRB.Pad.prototype.beatJumpMultiply = function(channel, control, value, status, group) {
    if (value) {
        var size = engine.getValue(group, "beatjump_size");
        engine.setValue(group, "beatjump_size", size * 2.0);
    }
};

PioneerDDJRB.Pad.prototype.beatJumpDivide = function(channel, control, value, status, group) {
    if (value) {
        var size = engine.getValue(group, "beatjump_size");
        engine.setValue(group, "beatjump_size", size / 2.0);
    }
};

PioneerDDJRB.shutdown = function() {
    // turn off button LEDs
    var skip = [0x72, 0x1B, 0x69, 0x1E, 0x6B, 0x20, 0x6D, 0x22, 0x6F, 0x70, 0x75];
    for (var channel = 0; channel <= 10; channel++) {
        for (var control = 0; control <= 127; control++) {
            // skip deck toggle buttons and pad mode buttons
            if (skip.indexOf(control) > -1) {
                continue;
            }
            midi.sendShortMsg(0x90 + channel, control, 0);
        }
    }


    // switch to decks 1 and 2 to turn off deck indication lights
    midi.sendShortMsg(0x90, 0x72, 0x7f);
    midi.sendShortMsg(0x91, 0x72, 0x7f);

    // turn off level meters
    for (channel = 0; channel <= 1; channel++) {
        midi.sendShortMsg(0xB0 + channel, 2, 0);
    }

    PioneerDDJRB.flasher.shutdown();
};

PioneerDDJRB.longButtonPressWait = function() {
    engine.stopTimer(PioneerDDJRB.longButtonPressTimer);
    PioneerDDJRB.longButtonPress = true;
};

///////////////////////////////////////////////////////////////
//                      VU - Meter                           //
///////////////////////////////////////////////////////////////

PioneerDDJRB.blinkAutodjState = 0; // new for DDJ-SB2

PioneerDDJRB.vuMeterTwinkle = function() {
    if (engine.getValue("[AutoDJ]", "enabled")) {
        PioneerDDJRB.blinkAutodjState = PioneerDDJRB.blinkAutodjState + 1;
        if (PioneerDDJRB.blinkAutodjState > 3) {
            PioneerDDJRB.blinkAutodjState = 0;
        }
        if (PioneerDDJRB.blinkAutodjState === 0) {
            PioneerDDJRB.valueVuMeter["[Channel1]_enabled"] = 0;
            PioneerDDJRB.valueVuMeter["[Channel2]_enabled"] = 0;
        }
        if (PioneerDDJRB.blinkAutodjState === 1) {
            PioneerDDJRB.valueVuMeter["[Channel1]_enabled"] = 1;
            PioneerDDJRB.valueVuMeter["[Channel2]_enabled"] = 0;
        }
        if (PioneerDDJRB.blinkAutodjState === 2) {
            PioneerDDJRB.valueVuMeter["[Channel1]_enabled"] = 1;
            PioneerDDJRB.valueVuMeter["[Channel2]_enabled"] = 1;
        }
        if (PioneerDDJRB.blinkAutodjState === 3) {
            PioneerDDJRB.valueVuMeter["[Channel1]_enabled"] = 0;
            PioneerDDJRB.valueVuMeter["[Channel2]_enabled"] = 1;
        }
    } else {
        PioneerDDJRB.valueVuMeter["[Channel1]_enabled"] = 1;
        PioneerDDJRB.valueVuMeter["[Channel2]_enabled"] = 1;
    }
};


///////////////////////////////////////////////////////////////
//                        AutoDJ                             //
///////////////////////////////////////////////////////////////

PioneerDDJRB.autodjSkipNext = function(channel, control, value) {
    if (value === 0) {
        return;
    }
    if (engine.getValue("[AutoDJ]", "enabled")) {
        engine.setValue("[AutoDJ]", "skip_next", true);
    }
};

PioneerDDJRB.autodjToggle = function(channel, control, value) {
    if (value === 0) {
        return;
    }
    if (engine.getValue("[AutoDJ]", "enabled")) {
        engine.setValue("[AutoDJ]", "enabled", false);
    } else {
        engine.setValue("[AutoDJ]", "enabled", true);
    }
};


///////////////////////////////////////////////////////////////
//                      CONTROL BINDING                      //
///////////////////////////////////////////////////////////////

PioneerDDJRB.bindSamplerControlConnections = function(samplerGroup) {
    engine.makeConnection(samplerGroup, "duration", PioneerDDJRB.samplerLedsDuration);
    engine.makeConnection(samplerGroup, "play", PioneerDDJRB.samplerLedsPlay);
};

PioneerDDJRB.bindDeckControlConnections = function(channelGroup, isUnbinding) {
    var i,
        index,
        controlsToFunctions = {
            "pfl": PioneerDDJRB.headphoneCueLed,
            "keylock": PioneerDDJRB.keyLockLed,
            "loop_enabled": PioneerDDJRB.autoLoopLed,
        };

    controlsToFunctions.slipEnabled = PioneerDDJRB.slipLed;

    for (i = 1; i <= 8; i++) {
        controlsToFunctions["hotcue_" + i + "_enabled"] = PioneerDDJRB.hotCueLeds;
    }

    for (index in PioneerDDJRB.looprollIntervals) {
        controlsToFunctions["beatlooproll_" + PioneerDDJRB.looprollIntervals[index] + "_activate"] = PioneerDDJRB.beatlooprollLeds;
    }

    script.bindConnections(channelGroup, controlsToFunctions, isUnbinding);
};

PioneerDDJRB.bindNonDeckControlConnections = function(isUnbinding) {
    var samplerIndex;

    for (samplerIndex = 1; samplerIndex <= 8; samplerIndex++) {
        PioneerDDJRB.bindSamplerControlConnections("[Sampler" + samplerIndex + "]", isUnbinding);
    }

    if (PioneerDDJRB.showVumeterMaster) {
        engine.connectControl("[Main]", "vu_meter_left", PioneerDDJRB.VuMeterLeds, isUnbinding);
        engine.connectControl("[Main]", "vu_meter_right", PioneerDDJRB.VuMeterLeds, isUnbinding);
    } else {
        engine.connectControl("[Channel1]", "vu_meter", PioneerDDJRB.VuMeterLeds, isUnbinding);
        engine.connectControl("[Channel2]", "vu_meter", PioneerDDJRB.VuMeterLeds, isUnbinding);
    }
};

///////////////////////////////////////////////////////////////
//                       DECK SWITCHING                      //
///////////////////////////////////////////////////////////////

PioneerDDJRB.deckSwitchTable = {
    "[Channel1]": "[Channel1]",
    "[Channel2]": "[Channel2]"
};


PioneerDDJRB.initDeck = function(group) {
    PioneerDDJRB.bindDeckControlConnections(group, false);
    engine.makeConnection(group, "loop_start_position", function() {
        PioneerDDJRB.updateLoopLedBlink(group);
    });
    engine.makeConnection(group, "loop_end_position", function() {
        PioneerDDJRB.updateLoopLedBlink(group);
    });
    PioneerDDJRB.nonPadLedControl(group, PioneerDDJRB.nonPadLeds.shiftKeyLock, PioneerDDJRB.channelGroups[group] > 1);
    PioneerDDJRB.toggleScratch(null, null, PioneerDDJRB.vinylModeOnStartup, null, group);
};


///////////////////////////////////////////////////////////////
//            HIGH RESOLUTION MIDI INPUT HANDLERS            //
///////////////////////////////////////////////////////////////

PioneerDDJRB.highResMSB = {
    "[Channel1]": {},
    "[Channel2]": {}
};

PioneerDDJRB.deckFaderMSB = function(channel, control, value, status, group) {
    PioneerDDJRB.highResMSB[group].deckFader = value;
};

PioneerDDJRB.deckFaderLSB = function(channel, control, value, status, group) {
    var fullValue = (PioneerDDJRB.highResMSB[group].deckFader << 7) + value;

    if (PioneerDDJRB.shiftPressed &&
        engine.getValue(group, "volume") === 0 &&
        fullValue !== 0 &&
        engine.getValue(group, "play") === 0
    ) {
        PioneerDDJRB.chFaderStart[channel] = engine.getValue(group, "playposition");
        engine.setValue(group, "play", 1);
    } else if (
        PioneerDDJRB.shiftPressed &&
        engine.getValue(group, "volume") !== 0 &&
        fullValue === 0 &&
        engine.getValue(group, "play") === 1 &&
        PioneerDDJRB.chFaderStart[channel] !== null
    ) {
        engine.setValue(group, "play", 0);
        engine.setValue(group, "playposition", PioneerDDJRB.chFaderStart[channel]);
        PioneerDDJRB.chFaderStart[channel] = null;
    }
    engine.setValue(group, "volume", fullValue / 0x3FFF);
};

///////////////////////////////////////////////////////////////
//           SINGLE MESSAGE MIDI INPUT HANDLERS              //
///////////////////////////////////////////////////////////////

PioneerDDJRB.beatloopRollButtons = function(channel, control, value, status, group) {
    var index = control - 0x50;
    engine.setValue(
        PioneerDDJRB.deckSwitchTable[group],
        "beatlooproll_" + PioneerDDJRB.looprollIntervals[index] + "_activate",
        value
    );
};

PioneerDDJRB.loopInButton = function(channel, control, value, status, group) {
    engine.setValue(PioneerDDJRB.deckSwitchTable[group], 'loop_in', value ? 1 : 0);
};

PioneerDDJRB.loopOutButton = function(channel, control, value, status, group) {
    var deck = PioneerDDJRB.deckSwitchTable[group];
    if (!value) {
        engine.setValue(deck, "loop_out", 0);
    } else if (engine.getValue(deck, "loop_enabled")) {
        engine.setValue(deck, "reloop_exit", 1);
    } else {
        engine.setValue(deck, "loop_out", 1);
    }
};

PioneerDDJRB.loopExitButton = function(channel, control, value, status, group) {
    if (value) {
        engine.setValue(PioneerDDJRB.deckSwitchTable[group], 'reloop_exit', 1);
    }
};

PioneerDDJRB.vinylButton = function(channel, control, value, status, group) {
    PioneerDDJRB.toggleScratch(channel, control, value, status, group);
};

PioneerDDJRB.slipButton = function(channel, control, value, status, group) {
    if (value) {
        script.toggleControl(group, "slipEnabled");
    }
};

PioneerDDJRB.keyLockButton = function(channel, control, value, status, group) {
    if (value) {
        script.toggleControl(group, "keylock");
    }
};

PioneerDDJRB.shiftKeyLockButton = function(channel, control, value, status, group) {
    if (value) {
        var currentTempoRange = engine.getValue(group, "rateRange");
        var deckIndex = status - 0x90 + 1;

        PioneerDDJRB.deck[deckIndex].tempoFader.skipSoftTakeover();

        if (currentTempoRange < 0.081) {
            engine.setValue(group, "rateRange", 0.16);
        } else if (currentTempoRange < 0.161) {
            engine.setValue(group, "rateRange", 0.50);
        } else if (currentTempoRange < 0.501) {
            engine.setValue(group, "rateRange", 1.0);
        } else {
            engine.setValue(group, "rateRange", 0.08);
        }
    }
};

PioneerDDJRB.deck1Button = function(channel, control, value, status, group) {
    if (value) {
        var bpm = engine.getValue(group, "bpm");
        PioneerDDJRB.updateBPM(bpm, group);
        midi.sendShortMsg(0xB0, 0x02, 0x0);
    }
};

PioneerDDJRB.deck2Button = function(channel, control, value, status, group) {
    if (value) {
        var bpm = engine.getValue(group, "bpm");
        PioneerDDJRB.updateBPM(bpm, group);
        midi.sendShortMsg(0xB1, 0x02, 0x0);
    }
};

PioneerDDJRB.autoLoopButton = function(channel, control, value, status, group) {
    if (value) {
        if (engine.getValue(group, "loop_enabled")) {
            engine.setValue(group, "reloop_toggle", true);
        } else {
            engine.setValue(group, "beatloop_activate", true);
        }
    }
};

PioneerDDJRB.reloopButton = function(channel, control, value, status, group) {
    if (value) {
        engine.setValue(group, "reloop_toggle", true);
    }
};

PioneerDDJRB.loadButton = function(channel, control, value, status, group) {
    if (value) {
        engine.setValue(group, "LoadSelectedTrack", 1);
    }
};

///////////////////////////////////////////////////////////////
//                          HEADPHONE                      //
///////////////////////////////////////////////////////////////

PioneerDDJRB.headphoneCueMaster = false;

PioneerDDJRB.masterCueButton = function(channel, control, value) {
    if (value) {
        PioneerDDJRB.headphoneCueMaster = !PioneerDDJRB.headphoneCueMaster;
        PioneerDDJRB.headphoneMasterUpdate();
    }
};

PioneerDDJRB.headphoneCueButton = function(channel, control, value, status, group) {
    if (value) {
        script.toggleControl(group, "pfl");
        PioneerDDJRB.headphoneMasterUpdate();
    }
};

PioneerDDJRB.headphoneMasterUpdate = function() {
    var anyDeckCue = false;
    var masterCue = PioneerDDJRB.headphoneCueMaster;

    for (var i = 1; i <= 2; i++) {
        if (engine.getValue("[Channel" + i + "]", "pfl")) {
            anyDeckCue = true;
        }
    }

    if (masterCue) {
        if (anyDeckCue) {
            // 50% master 50% cue
            engine.setValue("[Master]", "headMix", 0);
        } else {
            // 100% master
            // Check if 1 is all master or all cue
            engine.setValue("[Master]", "headMix", 1);
        }
    } else {
        // 0% master
        // Check if 1 is all master or all cue
        engine.setValue("[Master]", "headMix", -1);
    }
};

///////////////////////////////////////////////////////////////
//                          LED HELPERS                      //
///////////////////////////////////////////////////////////////

PioneerDDJRB.deckConverter = function(group) {
    var index;

    if (typeof group === "string") {
        for (index in PioneerDDJRB.deckSwitchTable) {
            if (group === PioneerDDJRB.deckSwitchTable[index]) {
                return PioneerDDJRB.channelGroups[group];
            }
        }
        return null;
    }
    return group;
};

PioneerDDJRB.padLedControl = function(deck, groupNumber, shiftGroup, ledNumber, shift, active) {
    var midiChannelOffset = PioneerDDJRB.deckConverter(deck);

    if (midiChannelOffset === null || midiChannelOffset === undefined) {
        return;
    }

    var padLedsBaseChannel = 0x97;
    var padLedControl = (shiftGroup ? 0x40 : 0x00) + (shift ? 0x08 : 0x00) + groupNumber + ledNumber;

    midi.sendShortMsg(
        padLedsBaseChannel + midiChannelOffset,
        padLedControl,
        active ? 0x7F : 0x00
    );
};

PioneerDDJRB.flashingPadLedControl = [];

PioneerDDJRB.initFlashingPadLedControl = function() {
    PioneerDDJRB.flasher.addFunction(function(flag) {
        for (var i = 0; i < PioneerDDJRB.flashingPadLedControl.length; i++) {
            PioneerDDJRB.padLedControl(
                PioneerDDJRB.flashingPadLedControl[i].deck,
                PioneerDDJRB.flashingPadLedControl[i].groupNumber,
                PioneerDDJRB.flashingPadLedControl[i].shiftGroup,
                PioneerDDJRB.flashingPadLedControl[i].ledNumber,
                PioneerDDJRB.flashingPadLedControl[i].shift,
                flag
            );
        }
    });
};

PioneerDDJRB.startFlashingPadLedControl = function(deck, groupNumber, shiftGroup, ledNumber, shift) {
    PioneerDDJRB.flashingPadLedControl.push({
        deck: deck,
        groupNumber: groupNumber,
        shiftGroup: shiftGroup,
        ledNumber: ledNumber,
        shift: shift
    });
};

PioneerDDJRB.stopFlashingPadLedControl = function(deck, groupNumber, shiftGroup, ledNumber, shift) {
    var target = {
        deck: deck,
        groupNumber: groupNumber,
        shiftGroup: shiftGroup,
        ledNumber: ledNumber,
        shift: shift
    };

    PioneerDDJRB.flashingPadLedControl = _.filter(PioneerDDJRB.flashingPadLedControl, function(obj) { return _.isEqual(obj, target); });
};

PioneerDDJRB.nonPadLedControl = function(deck, ledNumber, active) {
    var midiChannelOffset = PioneerDDJRB.deckConverter(deck);

    if (midiChannelOffset === null || midiChannelOffset === undefined) {
        return;
    }

    var nonPadLedsBaseChannel = 0x90;

    midi.sendShortMsg(
        nonPadLedsBaseChannel + midiChannelOffset,
        ledNumber,
        active ? 0x7F : 0x00
    );
};


///////////////////////////////////////////////////////////////
//                             LEDS                          //
///////////////////////////////////////////////////////////////

PioneerDDJRB.headphoneCueLed = function(value, group) {
    PioneerDDJRB.nonPadLedControl(group, PioneerDDJRB.nonPadLeds.headphoneCue, value);
    PioneerDDJRB.nonPadLedControl(group, PioneerDDJRB.nonPadLeds.shiftHeadphoneCue, value);
};

PioneerDDJRB.keyLockLed = function(value, group) {
    PioneerDDJRB.nonPadLedControl(group, PioneerDDJRB.nonPadLeds.keyLock, value);
    PioneerDDJRB.nonPadLedControl(group, PioneerDDJRB.nonPadLeds.shiftKeyLock, value);
};

PioneerDDJRB.vinylLed = function(value, group) {
    PioneerDDJRB.nonPadLedControl(group, PioneerDDJRB.nonPadLeds.vinyl, value);
};

PioneerDDJRB.slipLed = function(value, group) {
    PioneerDDJRB.nonPadLedControl(group, PioneerDDJRB.nonPadLeds.shiftVinyl, value);
};

PioneerDDJRB.autoLoopLed = function(value, group) {
    PioneerDDJRB.nonPadLedControl(group, PioneerDDJRB.nonPadLeds.autoLoop, value);
    PioneerDDJRB.nonPadLedControl(group, PioneerDDJRB.nonPadLeds.loopIn, value);
    PioneerDDJRB.nonPadLedControl(group, PioneerDDJRB.nonPadLeds.loopOut, value);
};

PioneerDDJRB.updateLoopLedBlink = function(group) {
    var startPos = engine.getValue(group, "loop_start_position");
    var loopEnabled = engine.getValue(group, "loop_enabled");
    var trackLoaded = engine.getValue(group, "track_loaded");
    PioneerDDJRB.loopInBlinking[group] = (trackLoaded && startPos !== -1 && !loopEnabled);
    if (PioneerDDJRB.loopInBlinking[group]) {
        PioneerDDJRB.nonPadLedControl(group, PioneerDDJRB.nonPadLeds.loopOut, false);
    }
};

PioneerDDJRB.samplerLedsDuration = function(value, group) {
    var sampler = PioneerDDJRB.samplerGroups[group];

    sampler.loaded = value;

    PioneerDDJRB.samplerLeds(sampler);
};

PioneerDDJRB.samplerLedsPlay = function(value, group) {
    var sampler = PioneerDDJRB.samplerGroups[group];

    sampler.playing = value;

    PioneerDDJRB.samplerLeds(sampler);
};

PioneerDDJRB.samplerLeds = function(sampler) {
    for (var i = 0; i < sampler.channels.length; i++) {

        if (sampler.playing) {
            PioneerDDJRB.startFlashingPadLedControl(sampler.channels[i], PioneerDDJRB.ledGroups.sampler, false, sampler.ledNumber, false);
        } else {
            PioneerDDJRB.stopFlashingPadLedControl(sampler.channels[i], PioneerDDJRB.ledGroups.sampler, false, sampler.ledNumber, false);
            PioneerDDJRB.padLedControl(sampler.channels[i], PioneerDDJRB.ledGroups.sampler, false, sampler.ledNumber, false, sampler.loaded);
        }
    }
};

PioneerDDJRB.beatlooprollLeds = function(value, group, control) {
    var index,
        padNum,
        shifted;

    for (index in PioneerDDJRB.looprollIntervals) {
        if (control === "beatlooproll_" + PioneerDDJRB.looprollIntervals[index] + "_activate") {
            padNum = index;
            shifted = false;
            PioneerDDJRB.padLedControl(group, PioneerDDJRB.ledGroups.roll, true, padNum, shifted, value);
        }
    }
};

PioneerDDJRB.hotCueLedStates = {
    "[Channel1]": {states: {}, isShifted: false},
    "[Channel2]": {states: {}, isShifted: false},
};

PioneerDDJRB.hotCueLeds = function(value, group, control) {
    var shiftedGroup = false,
        padNum = null,
        hotCueNum;

    for (hotCueNum = 1; hotCueNum <= 8; hotCueNum++) {
        if (control === "hotcue_" + hotCueNum + "_enabled") {
            padNum = (hotCueNum - 1);
            PioneerDDJRB.padLedControl(group, PioneerDDJRB.ledGroups.hotCue, shiftedGroup, padNum, false, value);
        }
    }
};

PioneerDDJRB.shiftListeners.push(function(group, isShifted) {
    PioneerDDJRB.hotCueLedStates[group].isShifted = isShifted;
});

PioneerDDJRB.VuMeterLeds = function(value, group, control) {
    // The red LED lights up with MIDI values 119 (0x77) and above. That should only light up when
    // the track is clipping.
    if (engine.getValue(group, "peak_indicator") === 1) {
        value = 119;
    } else {
        // 117 was determined experimentally so the yellow LED only lights
        // up when the level meter in Mixxx is in the yellow region.
        value = Math.floor(value * 117);
    }

    if (!(PioneerDDJRB.twinkleVumeterAutodjOn && engine.getValue("[AutoDJ]", "enabled"))) {
        var midiChannel;
        if (PioneerDDJRB.showVumeterMaster) {
            if (control === "vu_meter_left") {
                midiChannel = 0;
            } else if (control === "vu_meter_right") {
                midiChannel = 1;
            }
            midi.sendShortMsg(0xB0 + midiChannel, 2, value);
        } else {
            midiChannel = parseInt(group.substring(8, 9) - 1);
            midi.sendShortMsg(0xB0 + midiChannel, 2, value);
        }
    } else {
        if (group === "[Master]") {
            if (control === "vu_meter_left") {
                PioneerDDJRB.valueVuMeter["[Channel1]_current"] = value;
            } else {
                PioneerDDJRB.valueVuMeter["[Channel2]_current"] = value;
            }
        } else {
            PioneerDDJRB.valueVuMeter[group + "_current"] = value;
        }

        for (var channel = 0; channel < 2; channel++) {
            var midiOut = PioneerDDJRB.valueVuMeter["[Channel" + (channel + 1) + "]_current"];
            if (PioneerDDJRB.valueVuMeter["[Channel" + (channel + 1) + "]_enabled"]) {
                midiOut = 0;
            }
            if (midiOut < 5 && PioneerDDJRB.valueVuMeter["[Channel" + (channel + 1) + "]_enabled"] === 0) {
                midiOut = 5;
            }

            midi.sendShortMsg(
                0xB0 + channel,
                2,
                midiOut
            );
        }
    }
};


///////////////////////////////////////////////////////////////
//                          JOGWHEELS                        //
///////////////////////////////////////////////////////////////

PioneerDDJRB.getJogWheelDelta = function(value) { // O
    // The Wheel control centers on 0x40; find out how much it's moved by.
    return value - 0x40;
};

PioneerDDJRB.jogRingTick = function(channel, control, value, status, group) {
    PioneerDDJRB.pitchBendFromJog(group, PioneerDDJRB.getJogWheelDelta(value));
};

PioneerDDJRB.jogRingTickShift = function(channel, control, value, status, group) {
    PioneerDDJRB.pitchBendFromJog(
        PioneerDDJRB.deckSwitchTable[group],
        PioneerDDJRB.getJogWheelDelta(value) * PioneerDDJRB.jogwheelShiftMultiplier
    );
};

PioneerDDJRB.jogPlatterTick = function(channel, control, value, status, group) {
    var deck = PioneerDDJRB.channelGroups[PioneerDDJRB.deckSwitchTable[group]];
    if (PioneerDDJRB.scratchMode[deck]) {
        engine.scratchTick(deck + 1, PioneerDDJRB.getJogWheelDelta(value));
    } else {
        PioneerDDJRB.pitchBendFromJog(PioneerDDJRB.deckSwitchTable[group], PioneerDDJRB.getJogWheelDelta(value));
    }
};

PioneerDDJRB.jogPlatterTickShift = function(channel, control, value, status, group) {
    var deck = PioneerDDJRB.channelGroups[PioneerDDJRB.deckSwitchTable[group]];
    /*if (PioneerDDJRB.scratchMode[deck]) {
        engine.scratchTick(deck + 1, PioneerDDJRB.getJogWheelDelta(value));
    } else {*/
        PioneerDDJRB.pitchBendFromJog(
            PioneerDDJRB.deckSwitchTable[group],
            PioneerDDJRB.getJogWheelDelta(value) * PioneerDDJRB.jogwheelShiftMultiplier
        );
    /*}*/
};

PioneerDDJRB.jogTouch = function(channel, control, value, status, group) {
    var deck = PioneerDDJRB.channelGroups[PioneerDDJRB.deckSwitchTable[group]];

    if (PioneerDDJRB.scratchMode[deck]) {
        if (value) {
            engine.scratchEnable(
                deck + 1,
                PioneerDDJRB.scratchSettings.jogResolution,
                PioneerDDJRB.scratchSettings.vinylSpeed,
                PioneerDDJRB.scratchSettings.alpha,
                PioneerDDJRB.scratchSettings.beta,
                true
            );
        } else {
            engine.scratchDisable(deck + 1, true);

            if (engine.getValue(group, "slipEnabled")) {
                engine.setValue(group, "slipEnabled", false);

                engine.beginTimer(250, () => {
                    engine.setValue(group, "slipEnabled", true);
                }, true);
            }
        }
    }
};

PioneerDDJRB.toggleScratch = function(channel, control, value, status, group) {
    var deck = PioneerDDJRB.channelGroups[group];
    if (value) {
        PioneerDDJRB.scratchMode[deck] = !PioneerDDJRB.scratchMode[deck];

        PioneerDDJRB.vinylLed(PioneerDDJRB.scratchMode[deck], group);

        if (!PioneerDDJRB.scratchMode[deck]) {
            engine.scratchDisable(deck + 1, true);
        }
    }
};

PioneerDDJRB.pitchBendFromJog = function(channel, movement) {
    var group = (typeof channel === "string" ? channel : "[Channel" + channel + 1 + "]");

    engine.setValue(group, "jog", movement / 5 * PioneerDDJRB.jogwheelSensitivity);
};


///////////////////////////////////////////////////////////////
//                        ROTARY SELECTOR                    //
///////////////////////////////////////////////////////////////

PioneerDDJRB.rotarySelectorChanged = false; // new for DDJ-SB2

PioneerDDJRB.getRotaryDelta = function(value) {
    var delta = 0x40 - Math.abs(0x40 - value),
        isCounterClockwise = value > 0x40;

    if (isCounterClockwise) {
        delta *= -1;
    }
    return delta;
};

PioneerDDJRB.rotarySelector = function(channel, control, value) {
    var delta = PioneerDDJRB.getRotaryDelta(value);
    engine.setValue("[Playlist]", "SelectTrackKnob", delta);

    PioneerDDJRB.rotarySelectorChanged = true;
};

PioneerDDJRB.shiftedRotarySelector = function(channel, control, value) {
    var delta = PioneerDDJRB.getRotaryDelta(value),
        f = (delta > 0 ? "SelectNextPlaylist" : "SelectPrevPlaylist");

    engine.setValue("[Playlist]", f, Math.abs(delta));
};

PioneerDDJRB.rotarySelectorClick = function(channel, control, value) {
    if (PioneerDDJRB.rotarySelectorChanged === true) {
        if (value) {
            engine.setValue("[PreviewDeck1]", "LoadSelectedTrackAndPlay", true);
        } else {
            if (PioneerDDJRB.jumpPreviewEnabled) {
                engine.setValue("[PreviewDeck1]", "playposition", PioneerDDJRB.jumpPreviewPosition);
            }
            PioneerDDJRB.rotarySelectorChanged = false;
        }
    } else {
        if (value) {
            engine.setValue("[PreviewDeck1]", "stop", 1);
        } else {
            PioneerDDJRB.rotarySelectorChanged = true;
        }
    }
};

PioneerDDJRB.rotarySelectorShiftedClick = function(channel, control, value) {
    if (value) {
        engine.setValue("[Playlist]", "ToggleSelectedSidebarItem", 1);
    }
};


///////////////////////////////////////////////////////////////
//                             FX                            //
///////////////////////////////////////////////////////////////

PioneerDDJRB.EffectUnit = function(unitNumber) {
    var eu = this;
    this.isShifted = false;
    this.group = "[EffectRack1_EffectUnit" + unitNumber + "]";
    engine.setValue(this.group, "show_focus", 1);

    this.buttonLights = [];

    this.EffectButtonLight = function(buttonNumber) {
        this.isEnabled = false;
        this.isFocused = false;

        this.midi = [0x93 + unitNumber, 0x46 + buttonNumber];
    };

    this.EffectButtonLight.prototype.update = function() {
        if (eu.isShifted) {
            engine.log("isEnabled" + this.isEnabled);
            midi.sendShortMsg(this.midi[0], this.midi[1], (this.isFocused ? 0x7F : 0x0));
        } else {
            midi.sendShortMsg(this.midi[0], this.midi[1], (this.isEnabled ? 0x7F : 0x0));
        }
    };

    var i;

    for (i = 1; i <= 3; i++) {
        this.buttonLights[i] = new this.EffectButtonLight(i);
    }

    this.EffectFocusButton = function(buttonNumber) {
        this.buttonNumber = buttonNumber;

        this.group = eu.group;
        this.midi = [0x93 + unitNumber, 0x62 + buttonNumber];

        components.Button.call(this);
    };
    this.EffectFocusButton.prototype = new components.Button({
        input: function(channel, control, value) {
            if (value) {
                var focusedEffect = engine.getValue(eu.group, "focused_effect");

                if (focusedEffect === this.buttonNumber) {
                    engine.setValue(eu.group, "focused_effect", 0);
                } else {
                    engine.setValue(eu.group, "focused_effect", this.buttonNumber);
                }
            }
        },
        outKey: "focused_effect",
        output: function(value) {
            eu.buttonLights[this.buttonNumber].isFocused = (value === this.buttonNumber);
            eu.buttonLights[this.buttonNumber].update();
        },
        sendShifted: false,
        shiftControl: false,
    });

    this.EffectEnableButton = function(buttonNumber) {
        this.buttonNumber = buttonNumber;

        this.group = "[EffectRack1_EffectUnit" + unitNumber + "_Effect" + buttonNumber + "]";
        this.midi = [0x93 + unitNumber, 0x46 + buttonNumber];

        components.Button.call(this);
    };
    this.EffectEnableButton.prototype = new components.Button({
        input: function(channel, control, value) {
            if (value) {
                var effectEnabled = engine.getValue(this.group, "enabled");

                if (effectEnabled) {
                    engine.setValue(this.group, "enabled", false);
                } else {
                    engine.setValue(this.group, "enabled", true);
                }
            }
        },
        outKey: "enabled",
        output: function(value) {
            eu.buttonLights[this.buttonNumber].isEnabled = !!value;
            eu.buttonLights[this.buttonNumber].update();
        },
        sendShifted: false,
        shiftControl: false,
    });

    this.button = [];

    for (i = 1; i <= 3; i++) {
        this.button[i] = new this.EffectEnableButton(i);
        this.button[i + 3] = new this.EffectFocusButton(i);

        var effectGroup = "[EffectRack1_EffectUnit" + unitNumber + "_Effect" + i + "]";
        engine.softTakeover(effectGroup, "meta", true);
        engine.softTakeover(eu.group, "mix", true);
    }

    this.knob = new components.Pot({
        unshift: function() {
            this.input = function(channel, control, value) {
                value = (this.MSB << 7) + value;

                var focusedEffect = engine.getValue(eu.group, "focused_effect");
                if (focusedEffect === 0) {
                    engine.setParameter(eu.group, "mix", value / this.max);
                } else {
                    var effectGroup = "[EffectRack1_EffectUnit" + unitNumber + "_Effect" + focusedEffect + "]";
                    engine.setParameter(effectGroup, "meta", value / this.max);
                }
            };
        },
    });

    this.knobSoftTakeoverHandler = engine.makeConnection(eu.group, "focused_effect", function(value) {
        if (value === 0) {
            engine.softTakeoverIgnoreNextValue(eu.group, "mix");
        } else {
            var effectGroup = "[EffectRack1_EffectUnit" + unitNumber + "_Effect" + value + "]";
            engine.softTakeoverIgnoreNextValue(effectGroup, "meta");
        }
    });

    PioneerDDJRB.shiftListeners.push(function(group, shifted) {
        if (PioneerDDJRB.channelsToEffectUnitNumber[group] === unitNumber) {
            eu.setShift(shifted);
        }
    });
};

PioneerDDJRB.EffectUnit.prototype.setShift = function(value) {
    this.isShifted = value;
    for (var i = 1; i <= 3; i++) {
        this.buttonLights[i].update();
    }
};

PioneerDDJRB.fxUnit1Effect1MetaDecrease = function(channel, control, value) {
    if (value) {
        if (PioneerDDJRB.effectMetaTimerDecrease) {
            engine.stopTimer(PioneerDDJRB.effectMetaTimerDecrease);
        }
        if (PioneerDDJRB.effectMetaTimerIncrease) {
            engine.stopTimer(PioneerDDJRB.effectMetaTimerIncrease);
            PioneerDDJRB.effectMetaTimerIncrease = null;
        }
        var step = function() {
            var current = engine.getParameter("[EffectRack1_EffectUnit1_Effect1]", "meta");
            engine.setParameter("[EffectRack1_EffectUnit1_Effect1]", "meta", Math.max(0, current - 0.05));
        };
        step();
        PioneerDDJRB.effectMetaTimerDecrease = engine.beginTimer(150, step);
    } else {
        if (PioneerDDJRB.effectMetaTimerDecrease) {
            engine.stopTimer(PioneerDDJRB.effectMetaTimerDecrease);
            PioneerDDJRB.effectMetaTimerDecrease = null;
        }
    }
};

PioneerDDJRB.fxUnit1Effect1MetaIncrease = function(channel, control, value) {
    if (value) {
        if (PioneerDDJRB.effectMetaTimerIncrease) {
            engine.stopTimer(PioneerDDJRB.effectMetaTimerIncrease);
        }
        if (PioneerDDJRB.effectMetaTimerDecrease) {
            engine.stopTimer(PioneerDDJRB.effectMetaTimerDecrease);
            PioneerDDJRB.effectMetaTimerDecrease = null;
        }
        var step = function() {
            var current = engine.getParameter("[EffectRack1_EffectUnit1_Effect1]", "meta");
            engine.setParameter("[EffectRack1_EffectUnit1_Effect1]", "meta", Math.min(1, current + 0.05));
        };
        step();
        PioneerDDJRB.effectMetaTimerIncrease = engine.beginTimer(150, step);
    } else {
        if (PioneerDDJRB.effectMetaTimerIncrease) {
            engine.stopTimer(PioneerDDJRB.effectMetaTimerIncrease);
            PioneerDDJRB.effectMetaTimerIncrease = null;
        }
    }
};

PioneerDDJRB.fxUnit2Effect1MetaDecrease = function(channel, control, value) {
    if (value) {
        if (PioneerDDJRB.effect2MetaTimerDecrease) {
            engine.stopTimer(PioneerDDJRB.effect2MetaTimerDecrease);
        }
        if (PioneerDDJRB.effect2MetaTimerIncrease) {
            engine.stopTimer(PioneerDDJRB.effect2MetaTimerIncrease);
            PioneerDDJRB.effect2MetaTimerIncrease = null;
        }
        var step = function() {
            var current = engine.getParameter("[EffectRack1_EffectUnit2_Effect1]", "meta");
            engine.setParameter("[EffectRack1_EffectUnit2_Effect1]", "meta", Math.max(0, current - 0.05));
        };
        step();
        PioneerDDJRB.effect2MetaTimerDecrease = engine.beginTimer(150, step);
    } else {
        if (PioneerDDJRB.effect2MetaTimerDecrease) {
            engine.stopTimer(PioneerDDJRB.effect2MetaTimerDecrease);
            PioneerDDJRB.effect2MetaTimerDecrease = null;
        }
    }
};

PioneerDDJRB.fxUnit2Effect1MetaIncrease = function(channel, control, value) {
    if (value) {
        if (PioneerDDJRB.effect2MetaTimerIncrease) {
            engine.stopTimer(PioneerDDJRB.effect2MetaTimerIncrease);
        }
        if (PioneerDDJRB.effect2MetaTimerDecrease) {
            engine.stopTimer(PioneerDDJRB.effect2MetaTimerDecrease);
            PioneerDDJRB.effect2MetaTimerDecrease = null;
        }
        var step = function() {
            var current = engine.getParameter("[EffectRack1_EffectUnit2_Effect1]", "meta");
            engine.setParameter("[EffectRack1_EffectUnit2_Effect1]", "meta", Math.min(1, current + 0.05));
        };
        step();
        PioneerDDJRB.effect2MetaTimerIncrease = engine.beginTimer(150, step);
    } else {
        if (PioneerDDJRB.effect2MetaTimerIncrease) {
            engine.stopTimer(PioneerDDJRB.effect2MetaTimerIncrease);
            PioneerDDJRB.effect2MetaTimerIncrease = null;
        }
    }
};

///////////////////////////////////////////////////////////////
//                             SLICER                        //
///////////////////////////////////////////////////////////////

PioneerDDJRB.Slicer = function(group, midiOutputOp) {
    var _this = this;

    this.group = group;

    this.midiOutputOp = midiOutputOp;

    this.beatPositions = [];

    this.currentBeat = 0;

    this.latestPlayPosition = 0.0;

    this.connections = [];

    this.buttons = [];

    for (var i = 1; i <= PioneerDDJRB.midiOutputBeatLedsCount; i++) {
        (function(beat) {
            _this.buttons[beat] = function(channel, control, value) {
                if (value) {
                    var beatPosition = _this.beatPositions[beat - 1];

                    if (beatPosition) {
                        _this.moveToSample(beatPosition.sample);
                    }
                }
            };
        })(i);
    }

    this.calculateBeats();
    this.getFirstBeat();
    this.generateBeatPositions();
    this.midiOuputUpdate();

    this.connections.push(engine.makeConnection(group, "playposition", function(value, group, control) {
        _this.playPositionChange(value, group, control);
    }));

    this.connections.push(engine.makeConnection(group, "track_samples", function() {
        _this.calculateBeats();
        _this.getFirstBeat();
        _this.generateBeatPositions();
        _this.midiOuputUpdate();
    }));

    this.connections.push(engine.makeConnection(group, "bpm", function() {
        _this.calculateBeats();
        _this.generateBeatPositions();
        _this.midiOuputUpdate();
    }));
};

PioneerDDJRB.Slicer.prototype.PLAY_POSITION_FLOOR = 0;
PioneerDDJRB.Slicer.prototype.PLAY_POSITION_RANGE = 1 - PioneerDDJRB.Slicer.prototype.PLAY_POSITION_FLOOR;

PioneerDDJRB.Slicer.prototype.shutdown = function() {
    var i;

    for (i = 0; i < PioneerDDJRB.midiOutputBeatLedsCount.length; i++) {
        var ledMidi = PioneerDDJRB.midiOutputBeatLedsStart + i;

        if (ledMidi) {
            midi.sendShortMsg(this.midiOutputOp, ledMidi, 0x0);
        }
    }

    for (i = 0; i < this.connections.length; i++) {
        this.connections[i].disconnect();
    }
};

PioneerDDJRB.Slicer.prototype.calculateBeats = function() {
    var trackSamplesPerSecond = engine.getValue(this.group, "track_samplerate");
    this.trackSamples = engine.getValue(this.group, "track_samples");

    var bpm = engine.getValue(this.group, "bpm");
    var bps = bpm / 60.0;

    this.samplesPerBeat = (trackSamplesPerSecond * 2) / bps;
    this.positionPerBeat = (this.PLAY_POSITION_RANGE * this.samplesPerBeat) / this.trackSamples;
    this.playPositionDelta = (this.PLAY_POSITION_RANGE * this.samplesPerBeat) / (this.trackSamples * 20);
};

PioneerDDJRB.Slicer.prototype.generateBeatPositions = function() {
    this.beatPositions = [];

    for (var i = 0; i < PioneerDDJRB.midiOutputBeatLedsCount; i++) {
        var sample = this.firstBeatSample + (i * this.samplesPerBeat);
        var nextSample = this.firstBeatSample + ((i + 1) * this.samplesPerBeat);

        if (sample < this.trackSamples) {
            var bp = {
                sample: sample,
                positionIn: (this.PLAY_POSITION_RANGE * sample - 1) / this.trackSamples,
                positionOut: (this.PLAY_POSITION_RANGE * nextSample - 1) / this.trackSamples,
            };

            this.beatPositions.push(bp);
        }
    }

};

PioneerDDJRB.Slicer.prototype.getFirstBeat = function() {
    this.currentBeat = 0;

    var oldCuePosition = engine.getValue(this.group, "hotcue_8_position");
    var oldQuantize = engine.getValue(this.group, "quantize");

    this.oldCuePosition = oldCuePosition;

    engine.setValue(this.group, "quantize", true);
    engine.setValue(this.group, "hotcue_8_set", true);

    this.firstBeatSample = engine.getValue(this.group, "hotcue_8_position");

    if (oldCuePosition !== -1) {
        engine.setValue(this.group, "hotcue_8_position", oldCuePosition);
    } else {
        engine.setValue(this.group, "hotcue_8_clear", true);
    }

    engine.setValue(this.group, "quantize", oldQuantize);
};

PioneerDDJRB.Slicer.prototype.moveToSample = function(sample) {
    var oldCuePosition = engine.getValue(this.group, "hotcue_8_position");

    engine.setValue(this.group, "hotcue_8_set", true);
    engine.setValue(this.group, "hotcue_8_position", sample);
    engine.setValue(this.group, "hotcue_8_goto", true);

    if (oldCuePosition !== -1) {
        engine.setValue(this.group, "hotcue_8_position", oldCuePosition);
    } else {
        engine.setValue(this.group, "hotcue_8_clear", true);
    }
};

PioneerDDJRB.Slicer.prototype.playPositionChange = function(value) {
    var playPositionDelta = Math.abs(this.latestPlayPosition - value);
    var oldCurrentBeat = this.currentBeat;

    if (playPositionDelta > this.playPositionDelta) {
        this.latestPlayPosition = value;
        var found = false;

        for (var i = 0; i < this.beatPositions.length; i++) {
            var beatPosition = this.beatPositions[i];

            if (value >= beatPosition.positionIn && value < beatPosition.positionOut) {
                this.currentBeat = i;
                found = true;
            }
        }

        if (!found) {
            this.getFirstBeat();
            this.generateBeatPositions();
        }

        if (oldCurrentBeat !== this.currentBeat) {
            this.midiOuputUpdate();
        }
    }
};

PioneerDDJRB.Slicer.prototype.midiOuputUpdate = function() {
    var onLedMidi = PioneerDDJRB.midiOutputBeatLedsStart + this.currentBeat;

    for (var i = 0; i < PioneerDDJRB.midiOutputBeatLedsCount; i++) {
        var ledMidi = PioneerDDJRB.midiOutputBeatLedsStart + i;

        if (ledMidi !== onLedMidi) {
            midi.sendShortMsg(this.midiOutputOp, ledMidi, 0x0);
        }
    }

    if (onLedMidi === 0 || onLedMidi) {
        midi.sendShortMsg(this.midiOutputOp, onLedMidi, 0x7F);
    }
};
