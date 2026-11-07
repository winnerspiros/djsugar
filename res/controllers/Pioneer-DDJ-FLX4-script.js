// Pioneer-DDJ-FLX4-script.js
// ****************************************************************************
// * Mixxx mapping script file for the Pioneer DDJ-FLX4.
// * Mostly adapted from the DDJ-400 mapping script
// * Authors: Warker, nschloe, dj3730, jusko, Robert904
// ****************************************************************************
//
//  Implemented (as per manufacturer's manual):
//      * Mixer Section (Faders, EQ, Filter, Gain, Cue)
//      * Browsing and loading + Waveform zoom (shift)
//      * Jogwheels, Scratching, Bending, Loop adjust
//      * Cycle Temporange
//      * Beat Sync
//      * Hot Cue Mode
//      * Beat Loop Mode
//      * Beat Jump Mode
//      * Sampler Mode
//      * Keyshift mode
//
//  Custom (Mixxx specific mappings):
//      * BeatFX: Assigned Effect Unit 1
//                v FX_SELECT Load next effect.
//                SHIFT + v FX_SELECT Load previous effect.
//                < LEFT Cycle effect focus leftward
//                > RIGHT Cycle effect focus rightward
//                ON/OFF toggles focused effect slot
//                SHIFT + ON/OFF disables all three effect slots.
//
//      * 32 beat jump forward & back (Shift + </> CUE/LOOP CALL arrows)
//      * Toggle quantize (Shift + channel cue)
//      * Stems selection using PADs (using controller's Keyboard mode)
//
//  Not implemented (after discussion and trial attempts):
//      * Loop Section:
//        * -4BEAT auto loop (hacky---prefer a clean way to set a 4 beat loop
//                            from a previous position on long press)
//
//        * CUE/LOOP CALL - memory & delete (complex and not useful. Hot cues are sufficient)
//
//      * Secondary pad modes (trial attempts complex and too experimental)
//        * Keyboard mode
//        * Pad FX1
//        * Pad FX2
//
//  Not implemented yet (but might be in the future):
//      * Smart CFX
//      * Smart fader

var PioneerDDJFLX4 = {};

PioneerDDJFLX4.lights = {
    beatFx: {
        status: 0x94,
        data1: 0x47,
    },
    shiftBeatFx: {
        status: 0x94,
        data1: 0x43,
    },
    deck1: {
        vuMeter: {
            status: 0xB0,
            data1: 0x02,
        },
        playPause: {
            status: 0x90,
            data1: 0x0B,
        },
        shiftPlayPause: {
            status: 0x90,
            data1: 0x47,
        },
        cue: {
            status: 0x90,
            data1: 0x0C,
        },
        shiftCue: {
            status: 0x90,
            data1: 0x48,
        },
        hotcueMode: {
            status: 0x90,
            data1: 0x1B,
        },
        keyboardMode: {
            status: 0x90,
            data1: 0x69,
        },
        padFX1Mode: {
            status: 0x90,
            data1: 0x1E,
        },
        padFX2Mode: {
            status: 0x90,
            data1: 0x6B,
        },
        beatJumpMode: {
            status: 0x90,
            data1: 0x20,
        },
        beatLoopMode: {
            status: 0x90,
            data1: 0x6D,
        },
        samplerMode: {
            status: 0x90,
            data1: 0x22,
        },
        keyShiftMode: {
            status: 0x90,
            data1: 0x6F,
        },
    },
    deck2: {
        vuMeter: {
            status: 0xB0,
            data1: 0x02,
        },
        playPause: {
            status: 0x91,
            data1: 0x0B,
        },
        shiftPlayPause: {
            status: 0x91,
            data1: 0x47,
        },
        cue: {
            status: 0x91,
            data1: 0x0C,
        },
        shiftCue: {
            status: 0x91,
            data1: 0x48,
        },
        hotcueMode: {
            status: 0x91,
            data1: 0x1B,
        },
        keyboardMode: {
            status: 0x91,
            data1: 0x69,
        },
        padFX1Mode: {
            status: 0x91,
            data1: 0x1E,
        },
        padFX2Mode: {
            status: 0x91,
            data1: 0x6B,
        },
        beatJumpMode: {
            status: 0x91,
            data1: 0x20,
        },
        beatLoopMode: {
            status: 0x91,
            data1: 0x6D,
        },
        samplerMode: {
            status: 0x91,
            data1: 0x22,
        },
        keyShiftMode: {
            status: 0x91,
            data1: 0x6F,
        },
    },
};

// Store timer IDs
PioneerDDJFLX4.timers = {};

// Keep alive timer
PioneerDDJFLX4.sendKeepAlive = function() {
    midi.sendSysexMsg([0xF0, 0x00, 0x40, 0x05, 0x00, 0x00, 0x04, 0x05, 0x00, 0x50, 0x02, 0xf7], 12); // This was reverse engineered with Wireshark
};

// Jog wheel constants
PioneerDDJFLX4.vinylMode = true;
PioneerDDJFLX4.alpha = 1.0/8;
PioneerDDJFLX4.beta = PioneerDDJFLX4.alpha/32;

// Multiplier for fast seek through track using SHIFT+JOGWHEEL
PioneerDDJFLX4.fastSeekScale = 150;
PioneerDDJFLX4.bendScale = 0.8;

PioneerDDJFLX4.tempoRanges = [0.06, 0.10, 0.16, 0.25];

PioneerDDJFLX4.shiftButtonDown = [false, false];

// Jog wheel loop adjust
PioneerDDJFLX4.loopAdjustIn = [false, false];
PioneerDDJFLX4.loopAdjustOut = [false, false];
PioneerDDJFLX4.loopAdjustMultiply = 50;

// Beatjump pad (beatjump_size values)
PioneerDDJFLX4.beatjumpSizeForPad = {
    0x20: -1, // PAD 1
    0x21: 1,  // PAD 2
    0x22: -2, // PAD 3
    0x23: 2,  // PAD 4
    0x24: -4, // PAD 5
    0x25: 4,  // PAD 6
    0x26: -8, // PAD 7
    0x27: 8   // PAD 8
};

// Stems (KEYBOARD) pads mode status for deck 1 and 2, without or with SHIFT pressed
PioneerDDJFLX4.stemsPadsModesStatus = {
    "[Channel1]": [0x97, 0x98],
    "[Channel2]": [0x99, 0x9a],
};

// Stems (KEYBOARD) pad 1 control (pad control = [this value] + [pad  number] - 1)
PioneerDDJFLX4.stemMutePadsFirstControl = 0x40;

// Stems (KEYBOARD) pad 5 control (pad control = [this value] + [pad  number] - 1)
PioneerDDJFLX4.stemFxPadsFirstControl = 0x44;

// Pitch shift (KEY SHIFT) pads mode status for deck 1 and 2, without or with SHIFT pressed
PioneerDDJFLX4.pitchPadsModesStatus = {
    "[Channel1]": [0x97, 0x98],
    "[Channel2]": [0x99, 0x9a],
};

// Pitch shift (KEY SHIFT) pad 1 control (pad control = [this value] + [pad  number] - 1)
PioneerDDJFLX4.pitchPadsFirstControl = 0x70;

PioneerDDJFLX4.quickJumpSize = 32;

// rekordbox behaviour (mapping settings, see Pioneer-DDJ-FLX4.midi.xml). The
// defaults keep the Mixxx behaviour of this mapping.
PioneerDDJFLX4.option = function(name) {
    return engine.getSetting(name);
};

// Memory cues of CUE/LOOP CALL use the hot cues beyond the 8 pads
PioneerDDJFLX4.firstMemoryCue = 9;
PioneerDDJFLX4.lastMemoryCue = 36;
PioneerDDJFLX4.calledMemoryCue = {"[Channel1]": 0, "[Channel2]": 0};

// BEAT SYNC: whether the press became a long press (rekordbox mode)
PioneerDDJFLX4.syncLongPress = {"[Channel1]": false, "[Channel2]": false};

// LOAD: time of the last press, for Instant Doubles
PioneerDDJFLX4.lastLoadPress = {"[Channel1]": 0, "[Channel2]": 0};
PioneerDDJFLX4.instantDoublesTime = 400;

// rekordbox Keyboard mode: the hot cue played and the pitch range
PioneerDDJFLX4.keyboardCue = {"[Channel1]": 0, "[Channel2]": 0};
PioneerDDJFLX4.keyboardOffset = {"[Channel1]": 0, "[Channel2]": 0};

// rekordbox Key Shift and Keyboard pads, in semitones: top row, then bottom row
PioneerDDJFLX4.rekordboxPitchPads = [4, 5, 6, 7, 0, 1, 2, 3];
PioneerDDJFLX4.keyShiftOffset = {"[Channel1]": 0, "[Channel2]": 0};

// PAD FX1: loop rolls (beats) of pads 1-8
PioneerDDJFLX4.padFxRolls = [0.0625, 0.125, 0.25, 0.5, 1, 2, 4, 8];
// PAD FX2: the effects held by the pads, set while a pad is held
PioneerDDJFLX4.padFxHeld = {};

// Used for tempo slider
PioneerDDJFLX4.highResMSB = {
    "[Channel1]": {},
    "[Channel2]": {}
};

PioneerDDJFLX4.trackLoadedLED = function(value, group, _control) {
    midi.sendShortMsg(
        0x9F,
        group.match(script.channelRegEx)[1] - 1,
        value > 0 ? 0x7F : 0x00
    );
};

PioneerDDJFLX4.toggleLight = function(midiIn, active) {
    midi.sendShortMsg(midiIn.status, midiIn.data1, active ? 0x7F : 0);
};

//
// Init
//

PioneerDDJFLX4.init = function() {
    engine.setValue("[EffectRack1_EffectUnit1]", "show_focus", 1);
    // focused_effect is 0 (no focus) on a fresh install, so BEAT FX would
    // address a non-existing effect slot until BEAT LEFT/RIGHT is pressed
    if (engine.getValue("[EffectRack1_EffectUnit1]", "focused_effect") < 1) {
        engine.setValue("[EffectRack1_EffectUnit1]", "focused_effect", 1);
    }

    engine.makeConnection("[Channel1]", "vu_meter", PioneerDDJFLX4.vuMeterUpdate);
    engine.makeConnection("[Channel2]", "vu_meter", PioneerDDJFLX4.vuMeterUpdate);

    PioneerDDJFLX4.toggleLight(PioneerDDJFLX4.lights.deck1.vuMeter, false);
    PioneerDDJFLX4.toggleLight(PioneerDDJFLX4.lights.deck2.vuMeter, false);

    engine.softTakeover("[Channel1]", "rate", true);
    engine.softTakeover("[Channel2]", "rate", true);
    engine.softTakeover("[EffectRack1_EffectUnit1_Effect1]", "meta", true);
    engine.softTakeover("[EffectRack1_EffectUnit1_Effect2]", "meta", true);
    engine.softTakeover("[EffectRack1_EffectUnit1_Effect3]", "meta", true);
    engine.softTakeover("[EffectRack1_EffectUnit1]", "mix", true);

    const samplerCount = 16;
    if (engine.getValue("[App]", "num_samplers") < samplerCount) {
        engine.setValue("[App]", "num_samplers", samplerCount);
    }
    for (let i = 1; i <= samplerCount; ++i) {
        engine.makeConnection("[Sampler" + i + "]", "play", PioneerDDJFLX4.samplerPlayOutputCallbackFunction);
        // Lit by the script for both pad layouts (mapping setting)
        engine.makeConnection("[Sampler" + i + "]", "track_loaded", PioneerDDJFLX4.samplerLoadedLED).trigger();
    }

    engine.makeConnection("[QuickEffectRack1_[Channel1]]", "super1", PioneerDDJFLX4.cfxMoved);
    engine.makeConnection("[QuickEffectRack1_[Channel2]]", "super1", PioneerDDJFLX4.cfxMoved);
    PioneerDDJFLX4.smartCfxGroups.forEach(group =>
        engine.makeConnection(group, "enabled", PioneerDDJFLX4.smartCfxLED));
    PioneerDDJFLX4.smartCfxLED();
    engine.makeConnection("[Channel1]", "track_loaded", PioneerDDJFLX4.trackLoadedLED);
    engine.makeConnection("[Channel2]", "track_loaded", PioneerDDJFLX4.trackLoadedLED);

    // play the "track loaded" animation on both decks at startup
    midi.sendShortMsg(0x9F, 0x00, 0x7F);
    midi.sendShortMsg(0x9F, 0x01, 0x7F);

    PioneerDDJFLX4.setLoopButtonLights(0x90, 0x7F);
    PioneerDDJFLX4.setLoopButtonLights(0x91, 0x7F);

    engine.makeConnection("[Channel1]", "loop_enabled", PioneerDDJFLX4.loopToggle);
    engine.makeConnection("[Channel2]", "loop_enabled", PioneerDDJFLX4.loopToggle);

    for (i = 1; i <= 3; i++) {
        engine.makeConnection("[EffectRack1_EffectUnit1_Effect" + i +"]", "enabled", PioneerDDJFLX4.toggleFxLight);
    }
    engine.makeConnection("[EffectRack1_EffectUnit1]", "focused_effect", PioneerDDJFLX4.toggleFxLight);
    // Show the state of effects restored from the previous session
    PioneerDDJFLX4.toggleFxLight();

    // Register callbacks for each deck, when a file is loaded and the number of stems is available
    engine.makeConnection("[Channel1]", "stem_count", PioneerDDJFLX4.stemCountChanged);
    engine.makeConnection("[Channel2]", "stem_count", PioneerDDJFLX4.stemCountChanged);

    // Register callbacks for each stems of each decks, to change pad lights when muted/unmuted/FX
    for (let stem=1; stem<=4; stem++) {
        for (let deck=1; deck<=2; deck++) {
            engine.makeConnection(`[Channel${deck}_Stem${stem}]`, "mute", PioneerDDJFLX4.stemMuteChanged);
            engine.makeConnection(`[QuickEffectRack1_[Channel${deck}_Stem${stem}]]`, "enabled", PioneerDDJFLX4.stemFxChanged);
        }
    }

    // Register callbacks for each deck, when a file is loaded to reset pitch shift
    engine.makeConnection("[Channel1]", "track_loaded", PioneerDDJFLX4.pitchAdjusted);
    engine.makeConnection("[Channel2]", "track_loaded", PioneerDDJFLX4.pitchAdjusted);

    // Register callbacks for each deck, when the pitch shift is modified
    engine.makeConnection("[Channel1]", "pitch_adjust", PioneerDDJFLX4.pitchAdjusted);
    engine.makeConnection("[Channel2]", "pitch_adjust", PioneerDDJFLX4.pitchAdjusted);

    PioneerDDJFLX4.keepAliveTimer = engine.beginTimer(200, PioneerDDJFLX4.sendKeepAlive);

    // query the controller for current control positions on startup
    PioneerDDJFLX4.sendKeepAlive(); // the query seems to double as a keep alive message
};

//
// Waveform zoom
//

PioneerDDJFLX4.waveformZoom = function(midichan, control, value, status, group) {
    if (value === 0x7f) {
        script.triggerControl(group, "waveform_zoom_up", 100);
    } else {
        script.triggerControl(group, "waveform_zoom_down", 100);
    }
};

//
// Channel level lights
//

PioneerDDJFLX4.vuMeterUpdate = function(value, group) {
    const newVal = value * 127;

    switch (group) {
    case "[Channel1]":
        midi.sendShortMsg(0xB0, 0x02, newVal);
        break;

    case "[Channel2]":
        midi.sendShortMsg(0xB1, 0x02, newVal);
        break;
    }
};

//
// Effects
//

PioneerDDJFLX4.toggleFxLight = function(_value, _group, _control) {
    const enabled = engine.getValue(PioneerDDJFLX4.focusedFxGroup(), "enabled");

    PioneerDDJFLX4.toggleLight(PioneerDDJFLX4.lights.beatFx, enabled);
    PioneerDDJFLX4.toggleLight(PioneerDDJFLX4.lights.shiftBeatFx, enabled);
};

PioneerDDJFLX4.focusedFxGroup = function() {
    // The skin can remove the focus (0); fall back to the first slot
    const focusedFx = Math.min(3, Math.max(1,
        engine.getValue("[EffectRack1_EffectUnit1]", "focused_effect")));
    return "[EffectRack1_EffectUnit1_Effect" + focusedFx + "]";
};

// LEVEL/DEPTH changes the dry/wet of the unit and, with SHIFT, the amount
// of the selected effect, or the other way round (mapping setting)
PioneerDDJFLX4.beatFxLevelDepthRotate = function(_channel, _control, value) {
    const shifted = PioneerDDJFLX4.shiftButtonDown[0] || PioneerDDJFLX4.shiftButtonDown[1];
    if (shifted !== (engine.getSetting("levelDepthMode") === "meta")) {
        engine.softTakeoverIgnoreNextValue("[EffectRack1_EffectUnit1]", "mix");
        engine.setParameter(PioneerDDJFLX4.focusedFxGroup(), "meta", value / 0x7F);
    } else {
        engine.softTakeoverIgnoreNextValue(PioneerDDJFLX4.focusedFxGroup(), "meta");
        engine.setParameter("[EffectRack1_EffectUnit1]", "mix", value / 0x7F);
    }
};

PioneerDDJFLX4.changeFocusedEffectBy = function(numberOfSteps) {
    let focusedEffect = engine.getValue("[EffectRack1_EffectUnit1]", "focused_effect");

    // Convert to zero-based index
    focusedEffect -= 1;

    // Standard Euclidean modulo by use of two plain modulos
    const numberOfEffectsPerEffectUnit = 3;
    focusedEffect = (((focusedEffect + numberOfSteps) % numberOfEffectsPerEffectUnit) + numberOfEffectsPerEffectUnit) % numberOfEffectsPerEffectUnit;

    // Convert back to one-based index
    focusedEffect += 1;

    engine.setValue("[EffectRack1_EffectUnit1]", "focused_effect", focusedEffect);
};

PioneerDDJFLX4.beatFxSelectPressed = function(_channel, _control, value) {
    if (value === 0) { return; }

    engine.setValue(PioneerDDJFLX4.focusedFxGroup(), "next_effect", value);
};

PioneerDDJFLX4.beatFxSelectShiftPressed = function(_channel, _control, value) {
    if (value === 0) { return; }

    engine.setValue(PioneerDDJFLX4.focusedFxGroup(), "prev_effect", value);
};

PioneerDDJFLX4.beatFxLeftPressed = function(_channel, _control, value) {
    if (value === 0) { return; }

    PioneerDDJFLX4.changeFocusedEffectBy(-1);
};

PioneerDDJFLX4.beatFxRightPressed = function(_channel, _control, value) {
    if (value === 0) { return; }

    PioneerDDJFLX4.changeFocusedEffectBy(1);
};

PioneerDDJFLX4.beatFxOnOffPressed = function(_channel, _control, value) {
    if (value === 0) { return; }

    const toggleEnabled = !engine.getValue(PioneerDDJFLX4.focusedFxGroup(), "enabled");
    engine.setValue(PioneerDDJFLX4.focusedFxGroup(), "enabled", toggleEnabled);
};

PioneerDDJFLX4.beatFxOnOffShiftPressed = function(_channel, _control, value) {
    if (value === 0) { return; }

    engine.setParameter("[EffectRack1_EffectUnit1]", "mix", 0);
    engine.softTakeoverIgnoreNextValue("[EffectRack1_EffectUnit1]", "mix");

    for (let i = 1; i <= 3; i++) {
        engine.setValue("[EffectRack1_EffectUnit1_Effect" + i + "]", "enabled", 0);
    }
    PioneerDDJFLX4.toggleLight(PioneerDDJFLX4.lights.beatFx, false);
    PioneerDDJFLX4.toggleLight(PioneerDDJFLX4.lights.shiftBeatFx, false);
};

PioneerDDJFLX4.beatFxChannel1 = function(_channel, control, value, _status, group) {
    let enableChannel = 0;

    if (value === 0x7f) { enableChannel = 1; }

    engine.setValue(group, "group_[Channel1]_enable", enableChannel);
};

PioneerDDJFLX4.beatFxChannel2 = function(_channel, control, value, _status, group) {
    let enableChannel = 0;

    if (value === 0x7f) { enableChannel = 1; }

    engine.setValue(group, "group_[Channel2]_enable", enableChannel);
};

//
// Loop IN/OUT ADJUST
//

PioneerDDJFLX4.toggleLoopAdjustIn = function(channel, _control, value, _status, group) {
    if (value === 0 || engine.getValue(group, "loop_enabled") === 0) {
        return;
    }
    PioneerDDJFLX4.loopAdjustIn[channel] = !PioneerDDJFLX4.loopAdjustIn[channel];
    PioneerDDJFLX4.loopAdjustOut[channel] = false;
};

PioneerDDJFLX4.toggleLoopAdjustOut = function(channel, _control, value, _status, group) {
    if (value === 0 || engine.getValue(group, "loop_enabled") === 0) {
        return;
    }
    PioneerDDJFLX4.loopAdjustOut[channel] = !PioneerDDJFLX4.loopAdjustOut[channel];
    PioneerDDJFLX4.loopAdjustIn[channel] = false;
};

// Two signals are sent here so that the light stays lit/unlit in its shift state too
PioneerDDJFLX4.setReloopLight = function(status, value) {
    midi.sendShortMsg(status, 0x4D, value);
    midi.sendShortMsg(status, 0x50, value);
};


PioneerDDJFLX4.setLoopButtonLights = function(status, value) {
    [0x10, 0x11, 0x4E, 0x4C].forEach(function(control) {
        midi.sendShortMsg(status, control, value);
    });
};

PioneerDDJFLX4.startLoopLightsBlink = function(channel, control, status, group) {
    let blink = 0x7F;

    PioneerDDJFLX4.stopLoopLightsBlink(group, control, status);

    PioneerDDJFLX4.timers[group][control] = engine.beginTimer(500, () => {
        blink = 0x7F - blink;

        // When adjusting the loop out position, turn the loop in light off
        if (PioneerDDJFLX4.loopAdjustOut[channel]) {
            midi.sendShortMsg(status, 0x10, 0x00);
            midi.sendShortMsg(status, 0x4C, 0x00);
        } else {
            midi.sendShortMsg(status, 0x10, blink);
            midi.sendShortMsg(status, 0x4C, blink);
        }

        // When adjusting the loop in position, turn the loop out light off
        if (PioneerDDJFLX4.loopAdjustIn[channel]) {
            midi.sendShortMsg(status, 0x11, 0x00);
            midi.sendShortMsg(status, 0x4E, 0x00);
        } else {
            midi.sendShortMsg(status, 0x11, blink);
            midi.sendShortMsg(status, 0x4E, blink);
        }
    });

};

PioneerDDJFLX4.stopLoopLightsBlink = function(group, control, status) {
    PioneerDDJFLX4.timers[group] = PioneerDDJFLX4.timers[group] || {};

    if (PioneerDDJFLX4.timers[group][control] !== undefined) {
        engine.stopTimer(PioneerDDJFLX4.timers[group][control]);
    }
    PioneerDDJFLX4.timers[group][control] = undefined;
    PioneerDDJFLX4.setLoopButtonLights(status, 0x7F);
};

PioneerDDJFLX4.loopToggle = function(value, group, control) {
    const status = group === "[Channel1]" ? 0x90 : 0x91,
        channel = group === "[Channel1]" ? 0 : 1;

    PioneerDDJFLX4.setReloopLight(status, value ? 0x7F : 0x00);

    if (value) {
        PioneerDDJFLX4.startLoopLightsBlink(channel, control, status, group);
    } else {
        PioneerDDJFLX4.stopLoopLightsBlink(group, control, status);
        PioneerDDJFLX4.loopAdjustIn[channel] = false;
        PioneerDDJFLX4.loopAdjustOut[channel] = false;
    }
};

// IN and OUT: in rekordbox mode, pressed during a loop they adjust its in or
// out point with the jog wheel, like SHIFT + IN/OUT
// Whether the last press of IN/OUT adjusts the loop (rekordbox mode with a
// loop on), by deck and button. The release must go the same way as the
// press: the press may have turned a loop on, and a loop_in/loop_out press
// that is never released leaves the engine adjusting the loop.
PioneerDDJFLX4.loopButtonAdjusts = {};

PioneerDDJFLX4.loopButtonPressed = function(key, toggleAdjust, channel, control, value, status, group) {
    const id = `${group}${key}`;
    if (value) {
        PioneerDDJFLX4.loopButtonAdjusts[id] =
                PioneerDDJFLX4.option("loopButtonsMode") === "rekordbox" &&
                engine.getValue(group, "loop_enabled") > 0;
    }
    if (PioneerDDJFLX4.loopButtonAdjusts[id]) {
        toggleAdjust(channel, control, value, status, group);
        return;
    }
    engine.setValue(group, key, value ? 1 : 0);
};

PioneerDDJFLX4.loopInPressed = function(channel, control, value, status, group) {
    PioneerDDJFLX4.loopButtonPressed("loop_in", PioneerDDJFLX4.toggleLoopAdjustIn,
        channel, control, value, status, group);
};

PioneerDDJFLX4.loopOutPressed = function(channel, control, value, status, group) {
    PioneerDDJFLX4.loopButtonPressed("loop_out", PioneerDDJFLX4.toggleLoopAdjustOut,
        channel, control, value, status, group);
};

// 4 BEAT/EXIT: exits a playing loop; otherwise turns the last loop on again
// (Mixxx) or starts a loop of 4 beats (rekordbox)
PioneerDDJFLX4.fourBeatExitPressed = function(_channel, _control, value, _status, group) {
    if (PioneerDDJFLX4.option("loopButtonsMode") !== "rekordbox" || engine.getValue(group, "loop_enabled")) {
        engine.setValue(group, "reloop_toggle", value ? 1 : 0);
        return;
    }
    if (value) {
        script.triggerControl(group, "beatloop_4_activate");
    }
};

//
// CUE/LOOP CALL
//

// rekordbox mode: memory cues are the hot cues 9-36. Calls the memory cue
// before (direction < 0) or after the play position.
PioneerDDJFLX4.callMemoryCue = function(group, direction) {
    const position = engine.getValue(group, "playposition") * engine.getValue(group, "track_samples");
    // At a cue, ◀ calls the one before it
    const tolerance = engine.getValue(group, "track_samplerate") * 2 * 0.05;
    let best = 0;
    let bestPosition = 0;
    for (let n = PioneerDDJFLX4.firstMemoryCue; n <= PioneerDDJFLX4.lastMemoryCue; n++) {
        if (engine.getValue(group, `hotcue_${n}_status`) <= 0) {
            continue;
        }
        const cuePosition = engine.getValue(group, `hotcue_${n}_position`);
        if (direction < 0 && cuePosition < position - tolerance && (best === 0 || cuePosition > bestPosition)) {
            best = n;
            bestPosition = cuePosition;
        } else if (direction > 0 && cuePosition > position + tolerance && (best === 0 || cuePosition < bestPosition)) {
            best = n;
            bestPosition = cuePosition;
        }
    }
    if (best > 0) {
        script.triggerControl(group, `hotcue_${best}_goto`);
        PioneerDDJFLX4.calledMemoryCue[group] = best;
    }
};

// rekordbox mode: SHIFT + ▶ saves a memory cue (or the playing loop) in the
// first free hot cue from 9 on
PioneerDDJFLX4.saveMemoryCue = function(group) {
    for (let n = PioneerDDJFLX4.firstMemoryCue; n <= PioneerDDJFLX4.lastMemoryCue; n++) {
        if (engine.getValue(group, `hotcue_${n}_status`) <= 0) {
            script.triggerControl(group, engine.getValue(group, "loop_enabled") ? `hotcue_${n}_setloop` : `hotcue_${n}_setcue`);
            PioneerDDJFLX4.calledMemoryCue[group] = n;
            return;
        }
    }
};

// rekordbox mode: SHIFT + ◀ deletes the memory cue called last
PioneerDDJFLX4.deleteMemoryCue = function(group) {
    const n = PioneerDDJFLX4.calledMemoryCue[group];
    if (n > 0 && engine.getValue(group, `hotcue_${n}_status`) > 0) {
        script.triggerControl(group, `hotcue_${n}_clear`);
    }
    PioneerDDJFLX4.calledMemoryCue[group] = 0;
};

PioneerDDJFLX4.cueLoopCallLeft = function(_channel, _control, value, _status, group) {
    if (value && PioneerDDJFLX4.option("cueLoopCallMode") === "rekordbox" && !engine.getValue(group, "loop_enabled")) {
        PioneerDDJFLX4.callMemoryCue(group, -1);
        return;
    }
    if (value) {
        engine.setValue(group, "loop_scale", 0.5);
    }
};

PioneerDDJFLX4.cueLoopCallRight = function(_channel, _control, value, _status, group) {
    if (value && PioneerDDJFLX4.option("cueLoopCallMode") === "rekordbox" && !engine.getValue(group, "loop_enabled")) {
        PioneerDDJFLX4.callMemoryCue(group, 1);
        return;
    }
    if (value) {
        engine.setValue(group, "loop_scale", 2.0);
    }
};

//
// BEAT SYNC
//
// Note that the controller sends different signals for a short press and a long
// press of the same button.
//

PioneerDDJFLX4.syncPressed = function(channel, control, value, status, group) {
    // rekordbox mode: acts on release, unless the press became a long press
    if (PioneerDDJFLX4.option("syncMode") === "rekordbox") {
        if (value) {
            PioneerDDJFLX4.syncLongPress[group] = false;
        } else if (!PioneerDDJFLX4.syncLongPress[group]) {
            script.toggleControl(group, "sync_enabled");
        }
        return;
    }
    if (engine.getValue(group, "sync_enabled") && value > 0) {
        engine.setValue(group, "sync_enabled", 0);
    } else {
        engine.setValue(group, "beatsync", value);
    }
};

PioneerDDJFLX4.syncLongPressed = function(channel, control, value, status, group) {
    // rekordbox mode: a long press makes the deck the master (sync leader)
    if (PioneerDDJFLX4.option("syncMode") === "rekordbox") {
        if (value) {
            PioneerDDJFLX4.syncLongPress[group] = true;
            engine.setValue(group, "sync_leader", 1);
        }
        return;
    }
    if (value) {
        engine.setValue(group, "sync_enabled", 1);
    }
};

PioneerDDJFLX4.cycleTempoRange = function(_channel, _control, value, _status, group) {
    if (value === 0) { return; } // ignore release

    const currRange = engine.getValue(group, "rateRange");
    let idx = 0;

    for (let i = 0; i < this.tempoRanges.length; i++) {
        if (currRange === this.tempoRanges[i]) {
            // idx get the index of the value in tempoRanges following the currently configured one
            // or cycle back to 0 if the current is the last value of the list.
            idx = (i + 1) % this.tempoRanges.length;
            break;
        }
    }
    engine.setValue(group, "rateRange", this.tempoRanges[idx]);
};

// SHIFT + BEAT SYNC: key lock (MASTER TEMPO), or the tempo range like
// rekordbox
PioneerDDJFLX4.shiftSyncPressed = function(channel, control, value, status, group) {
    if (PioneerDDJFLX4.option("shiftSyncMode") === "tempoRange") {
        PioneerDDJFLX4.cycleTempoRange(channel, control, value, status, group);
        return;
    }
    if (value === 0) { return; } // ignore release
    script.toggleControl(group, "keylock");
};

//
// TRIM, with the TRIM / CFX guard option
//

PioneerDDJFLX4.trimMsbValue = {"[Channel1]": 0, "[Channel2]": 0};
PioneerDDJFLX4.trimLockedUntil = {"[Channel1]": 0, "[Channel2]": 0};
PioneerDDJFLX4.trimPickedUp = {"[Channel1]": false, "[Channel2]": false};
PioneerDDJFLX4.trimPickupTimer = {"[Channel1]": 0, "[Channel2]": 0};
// After CFX moved, TRIM is ignored this long
PioneerDDJFLX4.trimCfxGuardMs = 2000;
// TRIM changes once the knob is this close to the current value
PioneerDDJFLX4.trimPickupDistance = 0.02;
// After this long without TRIM, it has to be picked up again
PioneerDDJFLX4.trimPickupHoldMs = 1000;

PioneerDDJFLX4.trimMsb = function(_channel, _control, value, _status, group) {
    PioneerDDJFLX4.trimMsbValue[group] = value;
};

PioneerDDJFLX4.trimLsb = function(_channel, _control, value, _status, group) {
    const parameter = ((PioneerDDJFLX4.trimMsbValue[group] << 7) + value) / 0x3FFF;
    if (!PioneerDDJFLX4.option("trimCfxGuard")) {
        engine.setParameter(group, "pregain", parameter);
        return;
    }
    // Some units send stray TRIM messages while CFX is turned
    if (Date.now() < PioneerDDJFLX4.trimLockedUntil[group]) {
        PioneerDDJFLX4.trimPickedUp[group] = false;
        return;
    }
    if (!PioneerDDJFLX4.trimPickedUp[group]) {
        if (Math.abs(parameter - engine.getParameter(group, "pregain")) > PioneerDDJFLX4.trimPickupDistance) {
            return;
        }
        PioneerDDJFLX4.trimPickedUp[group] = true;
    }
    engine.setParameter(group, "pregain", parameter);
    if (PioneerDDJFLX4.trimPickupTimer[group]) {
        engine.stopTimer(PioneerDDJFLX4.trimPickupTimer[group]);
    }
    PioneerDDJFLX4.trimPickupTimer[group] = engine.beginTimer(PioneerDDJFLX4.trimPickupHoldMs, function() {
        PioneerDDJFLX4.trimPickupTimer[group] = 0;
        PioneerDDJFLX4.trimPickedUp[group] = false;
    }, true);
};

// CFX of a deck moved: its TRIM is locked for a while
PioneerDDJFLX4.cfxMoved = function(_value, group, _control) {
    const deck = group.match(/\[Channel(\d)\]/);
    if (!deck || !PioneerDDJFLX4.option("trimCfxGuard")) {
        return;
    }
    const channelGroup = `[Channel${deck[1]}]`;
    PioneerDDJFLX4.trimLockedUntil[channelGroup] = Date.now() + PioneerDDJFLX4.trimCfxGuardMs;
    PioneerDDJFLX4.trimPickedUp[channelGroup] = false;
};

//
// SMART CFX: the CFX (QuickEffect) of decks 1 and 2 on or off, with SHIFT
// the next CFX preset
//

PioneerDDJFLX4.smartCfxGroups = ["[QuickEffectRack1_[Channel1]]", "[QuickEffectRack1_[Channel2]]"];

PioneerDDJFLX4.smartCfxPressed = function(_channel, control, value) {
    if (value === 0) { return; } // ignore release
    const groups = PioneerDDJFLX4.smartCfxGroups;
    if (control === 0x08) {
        groups.forEach(group => engine.setValue(group, "next_chain_preset", 1));
        return;
    }
    const on = groups.some(group => engine.getValue(group, "enabled") > 0);
    groups.forEach(group => engine.setValue(group, "enabled", on ? 0 : 1));
};

PioneerDDJFLX4.smartCfxLED = function() {
    const on = PioneerDDJFLX4.smartCfxGroups.some(group => engine.getValue(group, "enabled") > 0);
    midi.sendShortMsg(0x96, 0x00, on ? 0x7F : 0x00);
};

//
// Jog wheels
//

PioneerDDJFLX4.jogTurn = function(channel, _control, value, _status, group) {
    const deckNum = channel + 1;
    // wheel center at 64; <64 rew >64 fwd
    let newVal = value - 64;

    // loop_in / out adjust
    const loopEnabled = engine.getValue(group, "loop_enabled");
    if (loopEnabled > 0) {
        if (PioneerDDJFLX4.loopAdjustIn[channel]) {
            newVal = newVal * PioneerDDJFLX4.loopAdjustMultiply + engine.getValue(group, "loop_start_position");
            engine.setValue(group, "loop_start_position", newVal);
            return;
        }
        if (PioneerDDJFLX4.loopAdjustOut[channel]) {
            newVal = newVal * PioneerDDJFLX4.loopAdjustMultiply + engine.getValue(group, "loop_end_position");
            engine.setValue(group, "loop_end_position", newVal);
            return;
        }
    }

    if (engine.isScratching(deckNum)) {
        engine.scratchTick(deckNum, newVal);
    } else { // fallback
        engine.setValue(group, "jog", newVal * this.bendScale);
    }
};


PioneerDDJFLX4.jogSearch = function(_channel, _control, value, _status, group) {
    const newVal = (value - 64) * PioneerDDJFLX4.fastSeekScale;
    engine.setValue(group, "jog", newVal);
};

PioneerDDJFLX4.jogTouch = function(channel, _control, value) {
    const deckNum = channel + 1;

    // skip while adjusting the loop points
    if (PioneerDDJFLX4.loopAdjustIn[channel] || PioneerDDJFLX4.loopAdjustOut[channel]) {
        return;
    }

    if (value !== 0 && this.vinylMode) {
        engine.scratchEnable(deckNum, 720, 33+1/3, this.alpha, this.beta);
    } else {
        engine.scratchDisable(deckNum);
    }
};

//
// Shift button
//

PioneerDDJFLX4.shiftPressed = function(channel, _control, value, _status, _group) {
    PioneerDDJFLX4.shiftButtonDown[channel] = value === 0x7F;
};


//
// Tempo sliders
//
// The tempo option in Mixxx's deck preferences determine whether down/up
// increases/decreases the rate. Therefore it must be inverted here so that the
// UI and the control sliders always move in the same direction.
//

PioneerDDJFLX4.tempoSliderMSB = function(channel, control, value, status, group) {
    PioneerDDJFLX4.highResMSB[group].tempoSlider = value;
};

PioneerDDJFLX4.tempoSliderLSB = function(channel, control, value, status, group) {
    const fullValue = (PioneerDDJFLX4.highResMSB[group].tempoSlider << 7) + value;

    engine.setValue(
        group,
        "rate",
        1 - (fullValue / 0x2000)
    );
};

//
// Beat Jump mode
//
// Note that when we increase/decrease the sizes on the pad buttons, we use the
// value of the first pad (0x21) as an upper/lower limit beyond which we don't
// allow further increasing/decreasing of all the values.
//

PioneerDDJFLX4.beatjumpPadPressed = function(_channel, control, value, _status, group) {
    if (value === 0) {
        return;
    }
    engine.setValue(group, "beatjump_size", Math.abs(PioneerDDJFLX4.beatjumpSizeForPad[control]));
    engine.setValue(group, "beatjump", PioneerDDJFLX4.beatjumpSizeForPad[control]);
};

PioneerDDJFLX4.increaseBeatjumpSizes = function(_channel, control, value, _status, group) {
    if (value === 0 || PioneerDDJFLX4.beatjumpSizeForPad[0x21] * 16 > 16) {
        return;
    }
    Object.keys(PioneerDDJFLX4.beatjumpSizeForPad).forEach(function(pad) {
        PioneerDDJFLX4.beatjumpSizeForPad[pad] = PioneerDDJFLX4.beatjumpSizeForPad[pad] * 16;
    });
    engine.setValue(group, "beatjump_size", PioneerDDJFLX4.beatjumpSizeForPad[0x21]);
};

PioneerDDJFLX4.decreaseBeatjumpSizes = function(_channel, control, value, _status, group) {
    if (value === 0 || PioneerDDJFLX4.beatjumpSizeForPad[0x21] / 16 < 1/16) {
        return;
    }
    Object.keys(PioneerDDJFLX4.beatjumpSizeForPad).forEach(function(pad) {
        PioneerDDJFLX4.beatjumpSizeForPad[pad] = PioneerDDJFLX4.beatjumpSizeForPad[pad] / 16;
    });
    engine.setValue(group, "beatjump_size", PioneerDDJFLX4.beatjumpSizeForPad[0x21]);
};

//
// Keyboard mode (rekordbox), instead of stems
//
// The first pad pressed picks the hot cue (and sets it if it's empty); then
// the pads play it at the pitches of rekordboxPitchPads. SHIFT + pad 7/8
// moves the range up or down, SHIFT + pads 1-6 delete the hot cue. Pressing
// SHIFT + HOT CUE again picks another hot cue.
//

PioneerDDJFLX4.keyboardPadPressed = function(group, pad, value, shift) {
    if (value !== 0x7f) {
        return;
    }
    if (shift) {
        if (pad === 6) {
            PioneerDDJFLX4.keyboardOffset[group] = Math.min(PioneerDDJFLX4.keyboardOffset[group] + 4, 5);
        } else if (pad === 7) {
            PioneerDDJFLX4.keyboardOffset[group] = Math.max(PioneerDDJFLX4.keyboardOffset[group] - 4, -12);
        } else {
            const cue = PioneerDDJFLX4.keyboardCue[group];
            if (cue > 0 && engine.getValue(group, `hotcue_${cue}_status`) > 0) {
                script.triggerControl(group, `hotcue_${cue}_clear`);
            }
            PioneerDDJFLX4.keyboardCue[group] = 0;
            PioneerDDJFLX4.keyboardLights(group);
        }
        return;
    }
    const cue = PioneerDDJFLX4.keyboardCue[group];
    if (cue === 0) {
        const picked = pad + 1;
        if (engine.getValue(group, `hotcue_${picked}_status`) <= 0) {
            script.triggerControl(group, `hotcue_${picked}_set`);
        }
        PioneerDDJFLX4.keyboardCue[group] = picked;
        PioneerDDJFLX4.keyboardLights(group);
        return;
    }
    engine.setValue(group, "pitch_adjust",
        PioneerDDJFLX4.rekordboxPitchPads[pad] + PioneerDDJFLX4.keyboardOffset[group]);
    script.triggerControl(group, `hotcue_${cue}_gotoandplay`);
};

// All pads lit once a hot cue is picked
PioneerDDJFLX4.keyboardLights = function(group) {
    const code = PioneerDDJFLX4.keyboardCue[group] > 0 ? 0x7F : 0x00;
    for (let i = 0; i < 8; i++) {
        PioneerDDJFLX4.stemsPadsModesStatus[group].forEach(
            (padMode) => midi.sendShortMsg(padMode, PioneerDDJFLX4.stemMutePadsFirstControl + i, code));
    }
};

//
// Sampler mode
//

// The pad of a sampler slot, as [status, control] of the pad without SHIFT
PioneerDDJFLX4.samplerPad = function(slot) {
    if (PioneerDDJFLX4.option("samplerLayout") === "rekordbox") {
        // Deck 1: slots 1-8, deck 2: slots 9-16
        return [slot <= 8 ? 0x97 : 0x99, 0x30 + (slot - 1) % 8];
    }
    // Deck 1: slots 1-4 and 9-12, deck 2: slots 5-8 and 13-16
    const deck2 = (slot >= 5 && slot <= 8) || slot >= 13;
    return [deck2 ? 0x99 : 0x97, 0x30 + (slot <= 8 ? (slot - 1) % 4 : 4 + (slot - 9) % 4)];
};

// The sampler group of a pad; the mapping file names the Mixxx layout
PioneerDDJFLX4.samplerGroupForPad = function(status, control, group) {
    if (PioneerDDJFLX4.option("samplerLayout") !== "rekordbox") {
        return group;
    }
    const deck = (status === 0x97 || status === 0x98) ? 0 : 1;
    return `[Sampler${deck * 8 + control - 0x30 + 1}]`;
};

PioneerDDJFLX4.samplerLoadedLED = function(value, group, _control) {
    const pad = PioneerDDJFLX4.samplerPad(Number(group.match(script.samplerRegEx)[1]));
    midi.sendShortMsg(pad[0], pad[1], value > 0.5 ? 0x7F : 0x00);
    midi.sendShortMsg(pad[0] + 1, pad[1], value > 0.5 ? 0x7F : 0x00);
};

PioneerDDJFLX4.samplerPlayOutputCallbackFunction = function(value, group, _control) {
    if (value === 1) {
        const pad = PioneerDDJFLX4.samplerPad(Number(group.match(script.samplerRegEx)[1]));
        PioneerDDJFLX4.startSamplerBlink(pad[0], pad[1], group);
    }
};

PioneerDDJFLX4.padModeKeyPressed = function(_channel, _control, value, _status, _group) {
    // SHIFT + HOT CUE (Keyboard mode) picks a new hot cue
    if (value && _control === 0x69 && PioneerDDJFLX4.option("keyboardMode") === "keyboard") {
        const group = _status === 0x90 ? "[Channel1]" : "[Channel2]";
        PioneerDDJFLX4.keyboardCue[group] = 0;
        PioneerDDJFLX4.keyboardLights(group);
    }
    const deck = (_status === 0x90 ? PioneerDDJFLX4.lights.deck1 : PioneerDDJFLX4.lights.deck2);

    if (_control === 0x1B) {
        PioneerDDJFLX4.toggleLight(deck.hotcueMode, true);
    } else if (_control === 0x69) {
        PioneerDDJFLX4.toggleLight(deck.keyboardMode, true);
    } else if (_control === 0x1E) {
        PioneerDDJFLX4.toggleLight(deck.padFX1Mode, true);
    } else if (_control === 0x6B) {
        PioneerDDJFLX4.toggleLight(deck.padFX2Mode, true);
    } else if (_control === 0x20) {
        PioneerDDJFLX4.toggleLight(deck.beatJumpMode, true);
    } else if (_control === 0x6D) {
        PioneerDDJFLX4.toggleLight(deck.beatLoopMode, true);
    } else if (_control === 0x22) {
        PioneerDDJFLX4.toggleLight(deck.samplerMode, true);
    } else if (_control === 0x6F) {
        PioneerDDJFLX4.toggleLight(deck.keyShiftMode, true);
    }
};

PioneerDDJFLX4.samplerPadPressed = function(_channel, _control, value, _status, group) {
    if (PioneerDDJFLX4.option("samplerLayout") === "rekordbox") {
        // rekordbox: plays the slot; loading is SHIFT + pad
        group = PioneerDDJFLX4.samplerGroupForPad(_status, _control, group);
        if (engine.getValue(group, "track_loaded")) {
            engine.setValue(group, "cue_gotoandplay", value);
        }
        return;
    }
    if (engine.getValue(group, "track_loaded")) {
        engine.setValue(group, "cue_gotoandplay", value);
    } else {
        engine.setValue(group, "LoadSelectedTrack", value);
    }
};

PioneerDDJFLX4.samplerPadShiftPressed = function(_channel, _control, value, _status, group) {
    if (PioneerDDJFLX4.option("samplerLayout") === "rekordbox") {
        // rekordbox: stops a playing slot, or loads the selected track
        group = PioneerDDJFLX4.samplerGroupForPad(_status, _control, group);
        if (engine.getValue(group, "play")) {
            engine.setValue(group, "cue_gotoandstop", value);
        } else {
            engine.setValue(group, "LoadSelectedTrack", value);
        }
        return;
    }
    if (engine.getValue(group, "play")) {
        engine.setValue(group, "cue_gotoandstop", value);
    } else if (engine.getValue(group, "track_loaded")) {
        engine.setValue(group, "eject", value);
    }
};

PioneerDDJFLX4.startSamplerBlink = function(channel, control, group) {
    let val = 0x7f;

    PioneerDDJFLX4.stopSamplerBlink(channel, control);
    PioneerDDJFLX4.timers[channel][control] = engine.beginTimer(250, () => {
        val = 0x7f - val;

        // blink the appropriate pad
        midi.sendShortMsg(channel, control, val);
        // also blink the pad while SHIFT is pressed
        midi.sendShortMsg((channel+1), control, val);

        const isPlaying = engine.getValue(group, "play") === 1;

        if (!isPlaying) {
            // kill timer
            PioneerDDJFLX4.stopSamplerBlink(channel, control);
            // set the pad LED to ON
            midi.sendShortMsg(channel, control, 0x7f);
            // set the pad LED to ON while SHIFT is pressed
            midi.sendShortMsg((channel+1), control, 0x7f);
        }
    });
};

PioneerDDJFLX4.stopSamplerBlink = function(channel, control) {
    PioneerDDJFLX4.timers[channel] = PioneerDDJFLX4.timers[channel] || {};

    if (PioneerDDJFLX4.timers[channel][control] !== undefined) {
        engine.stopTimer(PioneerDDJFLX4.timers[channel][control]);
        PioneerDDJFLX4.timers[channel][control] = undefined;
    }
};


PioneerDDJFLX4.toggleQuantize = function(_channel, _control, value, _status, group) {
    if (PioneerDDJFLX4.option("shiftHeadphoneCue") === "tap") {
        engine.setValue(group, "bpm_tap", value ? 1 : 0);
        return;
    }
    if (value) {
        script.toggleControl(group, "quantize");
    }
};

PioneerDDJFLX4.quickJumpForward = function(_channel, _control, value, _status, group) {
    if (value && PioneerDDJFLX4.option("cueLoopCallMode") === "rekordbox") {
        PioneerDDJFLX4.saveMemoryCue(group);
        return;
    }
    if (value) {
        engine.setValue(group, "beatjump", PioneerDDJFLX4.quickJumpSize);
    }
};

PioneerDDJFLX4.quickJumpBack = function(_channel, _control, value, _status, group) {
    if (value && PioneerDDJFLX4.option("cueLoopCallMode") === "rekordbox") {
        PioneerDDJFLX4.deleteMemoryCue(group);
        return;
    }
    if (value) {
        engine.setValue(group, "beatjump", -PioneerDDJFLX4.quickJumpSize);
    }
};

//
// Stems mode
//

PioneerDDJFLX4.stemMutePadPressed = function(_channel, control, value, _status, group) {
    if (PioneerDDJFLX4.option("keyboardMode") === "keyboard") {
        PioneerDDJFLX4.keyboardPadPressed(group, control - PioneerDDJFLX4.stemMutePadsFirstControl, value, false);
        return;
    }
    if (value !== 0x7f) {
        return;
    }

    const stemCount = Math.min(engine.getValue(group, "stem_count"), 4);

    if (control - PioneerDDJFLX4.stemMutePadsFirstControl + 1 > stemCount) {
        return;
    }

    const stemGroup = `[${group.substring(1, group.length-1)}_Stem${control - PioneerDDJFLX4.stemMutePadsFirstControl + 1}]`;

    if (engine.getValue(stemGroup, "mute")) {
        engine.setValue(stemGroup, "mute", 0);
    } else {
        engine.setValue(stemGroup, "mute", 1);
    }
};

PioneerDDJFLX4.stemMutePadShiftPressed = function(_channel, control, value, _status, group) {
    if (PioneerDDJFLX4.option("keyboardMode") === "keyboard") {
        PioneerDDJFLX4.keyboardPadPressed(group, control - PioneerDDJFLX4.stemMutePadsFirstControl, value, true);
        return;
    }
    if (value !== 0x7f) {
        return;
    }

    const stemCount = Math.min(engine.getValue(group, "stem_count"), 4);

    if (control - PioneerDDJFLX4.stemMutePadsFirstControl + 1 > stemCount) {
        return;
    }

    for (let stemIdx=1; stemIdx<=stemCount; stemIdx++) {
        const stemGroup = `[${group.substring(1, group.length-1)}_Stem${stemIdx}]`;

        if (stemIdx + PioneerDDJFLX4.stemMutePadsFirstControl - 1 === control) {
            engine.setValue(stemGroup, "mute", 0);
        } else {
            engine.setValue(stemGroup, "mute", 1);
        }
    }
};

PioneerDDJFLX4.stemFxPadPressed = function(_channel, control, value, _status, group) {
    if (PioneerDDJFLX4.option("keyboardMode") === "keyboard") {
        PioneerDDJFLX4.keyboardPadPressed(group, control - PioneerDDJFLX4.stemMutePadsFirstControl, value, false);
        return;
    }
    if (value !== 0x7f) {
        return;
    }

    if (control - PioneerDDJFLX4.stemFxPadsFirstControl + 1 > 4) {
        return;
    }

    const stemGroup = `[QuickEffectRack1_[${group.substring(1, group.length-1)}_Stem${control - PioneerDDJFLX4.stemFxPadsFirstControl + 1}]]`;

    if (engine.getValue(stemGroup, "enabled")) {
        engine.setValue(stemGroup, "enabled", 0);
    } else {
        engine.setValue(stemGroup, "enabled", 1);
    }
};

PioneerDDJFLX4.stemFxPadShiftPressed = function(_channel, control, value, _status, group) {
    if (PioneerDDJFLX4.option("keyboardMode") === "keyboard") {
        PioneerDDJFLX4.keyboardPadPressed(group, control - PioneerDDJFLX4.stemMutePadsFirstControl, value, true);
        return;
    }
    if (value !== 0x7f) {
        return;
    }

    if (control - PioneerDDJFLX4.stemFxPadsFirstControl + 1 > 4) {
        return;
    }

    const stemGroup = `[QuickEffectRack1_[${group.substring(1, group.length-1)}_Stem${control - PioneerDDJFLX4.stemFxPadsFirstControl + 1}]]`;

    engine.setValue(stemGroup, "next_chain_preset", 1);
};

PioneerDDJFLX4.stemCountChanged = function(_value, group, _control) {

    for (let stem=1; stem<=4; stem++) {
        // Stem mute pads
        PioneerDDJFLX4.stemMuteChanged(
            engine.getValue(`[${group.substring(1, group.length-1)}_Stem${stem}]`, "mute"),
            `[${group.substring(1, group.length-1)}_Stem${stem}]`,
            _control,
        );

        // Stem FX pads
        PioneerDDJFLX4.stemFxChanged(
            engine.getValue(`[QuickEffectRack1_[${group.substring(1, group.length-1)}_Stem${stem}]]`, "enabled"),
            `[QuickEffectRack1_[${group.substring(1, group.length-1)}_Stem${stem}]]`,
            _control,
        );
    }
};

PioneerDDJFLX4.stemMuteChanged = function(value, group, _control) {
    // The pads show the Keyboard mode instead
    if (PioneerDDJFLX4.option("keyboardMode") === "keyboard") {
        return;
    }
    const channelStem = group.match(/\[Channel(\d+)_Stem(\d+)\]/);
    const deck = Number(channelStem[1]);
    const stem = Number(channelStem[2]);
    const channel = `[Channel${deck}]`;

    if (stem > 4) {
        return;
    }

    const stemCount = engine.getValue(channel, "stem_count");

    let code = 0x00;
    if (stem <= stemCount && value <= 0.5) {
        code = 0x7f;
    }

    for (let i=0; i<PioneerDDJFLX4.stemsPadsModesStatus[channel].length; i++) {
        midi.sendShortMsg(
            PioneerDDJFLX4.stemsPadsModesStatus[channel][i],
            PioneerDDJFLX4.stemMutePadsFirstControl + stem -1,
            code,
        );
    }
};

PioneerDDJFLX4.stemFxChanged = function(value, group, _control) {
    // The pads show the Keyboard mode instead
    if (PioneerDDJFLX4.option("keyboardMode") === "keyboard") {
        return;
    }
    const channelStem = group.match(/\[QuickEffectRack1_\[Channel(\d+)_Stem(\d+)\]\]/);
    const deck = Number(channelStem[1]);
    const stem = Number(channelStem[2]);
    const channel = `[Channel${deck}]`;

    if (stem > 4) {
        return;
    }

    for (let i=0; i<PioneerDDJFLX4.stemsPadsModesStatus[channel].length; i++) {
        midi.sendShortMsg(
            PioneerDDJFLX4.stemsPadsModesStatus[channel][i],
            PioneerDDJFLX4.stemFxPadsFirstControl + stem -1,
            value <= 0.5 ? 0x00 : 0x7f,
        );
    }
};

//
// Pitch Shift mode
//

PioneerDDJFLX4.pitchAdjusted = function(_value, group, _control) {
    // rekordbox layout: the pad of the current key is lit
    if (PioneerDDJFLX4.option("keyShiftLayout") === "rekordbox") {
        const pitch = Math.round(engine.getValue(group, "pitch_adjust"));
        for (let i = 0; i < 8; i++) {
            const lit = PioneerDDJFLX4.rekordboxPitchPads[i] + PioneerDDJFLX4.keyShiftOffset[group] === pitch;
            PioneerDDJFLX4.pitchPadsModesStatus[group].forEach(
                (padMode) => midi.sendShortMsg(padMode, PioneerDDJFLX4.pitchPadsFirstControl + i, lit ? 0x7F : 0x00));
        }
        return;
    }
    const pitchAdjust = Math.round(engine.getValue(group, "pitch_adjust"));
    let lights = 0b00000000;

    if (pitchAdjust === 0) {
        lights = 0b10000001;
    } else if (pitchAdjust === 1) {
        lights = 0b01000000;
    } else if (pitchAdjust === 2) {
        lights = 0b00100000;
    } else if (pitchAdjust === 3) {
        lights = 0b00010000;
    } else if (pitchAdjust === 4) {
        lights = 0b10010000;
    } else if (pitchAdjust === 5) {
        lights = 0b01010000;
    } else if (pitchAdjust === 6) {
        lights = 0b00110000;
    } else if (pitchAdjust === 7) {
        lights = 0b10110000;
    } else if (pitchAdjust === 8) {
        lights = 0b01110000;
    } else if (pitchAdjust > 8) {
        lights = 0b11110000;
    } else if (pitchAdjust === -1) {
        lights = 0b00000010;
    } else if (pitchAdjust === -2) {
        lights = 0b00000100;
    } else if (pitchAdjust === -3) {
        lights = 0b00001000;
    } else if (pitchAdjust === -4) {
        lights = 0b00001001;
    } else if (pitchAdjust === -5) {
        lights = 0b00001010;
    } else if (pitchAdjust === -6) {
        lights = 0b00001100;
    } else if (pitchAdjust === -7) {
        lights = 0b00001101;
    } else if (pitchAdjust === -8) {
        lights = 0b00001110;
    } else if (pitchAdjust < -8) {
        lights = 0b00001111;
    } else {
        lights = 0b11111111;
    }

    for (let i=0; i<8; i++) {
        let code = 0x00;
        const pad = 0b10000000 >>> i;

        if (lights & pad) {
            code = 0x7f;
        } else {
            code = 0x00;
        }

        PioneerDDJFLX4.pitchPadsModesStatus[group].forEach(
            (padMode) => midi.sendShortMsg(
                padMode,
                PioneerDDJFLX4.pitchPadsFirstControl + i,
                code,
            )
        );
    }
};

PioneerDDJFLX4.pitchPadPressed = function(_channel, control, value, _status, group) {
    if (PioneerDDJFLX4.option("keyShiftLayout") === "rekordbox") {
        if (value === 0x7f) {
            engine.setValue(group, "pitch_adjust",
                PioneerDDJFLX4.rekordboxPitchPads[control - this.pitchPadsFirstControl] +
                PioneerDDJFLX4.keyShiftOffset[group]);
        }
        return;
    }
    if (value !== 0x7f) {
        return;
    }

    const pad = control - this.pitchPadsFirstControl;
    let pitch = 0;

    if (pad === 0) {
        pitch = 0;
    } else if (pad === 1) {
        pitch = 1;
    } else if (pad === 2) {
        pitch = 2;
    } else if (pad === 3) {
        pitch = 3;
    } else if (pad === 4) {
        pitch = -3;
    } else if (pad === 5) {
        pitch = -2;
    } else if (pad === 6) {
        pitch = -1;
    } else if (pad === 7) {
        pitch = 0;
    }

    engine.setValue(group, "pitch_adjust", pitch);
};

PioneerDDJFLX4.pitchPadShiftPressed = function(_channel, control, value, _status, group) {
    // rekordbox: SHIFT + pad 7/8 moves the range of the pads up or down
    if (PioneerDDJFLX4.option("keyShiftLayout") === "rekordbox") {
        const pad = control - this.pitchPadsFirstControl;
        if (value === 0x7f && pad === 6) {
            PioneerDDJFLX4.keyShiftOffset[group] = Math.min(PioneerDDJFLX4.keyShiftOffset[group] + 4, 5);
        } else if (value === 0x7f && pad === 7) {
            PioneerDDJFLX4.keyShiftOffset[group] = Math.max(PioneerDDJFLX4.keyShiftOffset[group] - 4, -12);
        }
        PioneerDDJFLX4.pitchAdjusted(0, group);
        return;
    }
    if (value !== 0x7f) {
        return;
    }

    const pad = control - this.pitchPadsFirstControl;

    let currentPitch = engine.getValue(group, "pitch_adjust");

    if (pad === 0) {
        currentPitch += 1;
    } else if (pad === 1) {
        currentPitch += 2;
    } else if (pad === 2) {
        currentPitch += 3;
    } else if (pad === 3) {
        currentPitch += 4;
    } else if (pad === 4) {
        currentPitch += -4;
    } else if (pad === 5) {
        currentPitch += -3;
    } else if (pad === 6) {
        currentPitch += -2;
    } else if (pad === 7) {
        currentPitch += -1;
    }

    engine.setValue(group, "pitch_adjust", currentPitch);
};


//
// Shutdown
//

//
// PAD FX modes
//
// PAD FX1: loop rolls (with SHIFT reverse rolls) while a pad is held.
// PAD FX2: while a pad is held, pads 1-3 play the effects of effect unit 2 and
// pads 4-6 those of unit 3 on this deck, pad 7 brakes and pad 8 spins back;
// with SHIFT, pads 1-3 play effect unit 4. The effects are chosen in Mixxx.
//

// What each held PAD FX pad turned on, by deck and pad, released the same
// way: SHIFT may be let go before the pad
PioneerDDJFLX4.padFxReleases = {};

PioneerDDJFLX4.padFxPressed = function(_channel, control, value, status, group) {
    if (!PioneerDDJFLX4.option("padFx")) {
        return;
    }
    const shift = status === 0x98 || status === 0x9A;
    const pad = control & 0x07;
    const deck = group === "[Channel1]" ? 1 : 2;
    const id = `${group}${control < 0x50 ? 1 : 2}_${pad}`;

    midi.sendShortMsg(status, control, value ? 0x7F : 0x00);

    if (!value) {
        const release = PioneerDDJFLX4.padFxReleases[id];
        delete PioneerDDJFLX4.padFxReleases[id];
        if (release) {
            release();
        }
        return;
    }

    let hold;
    if (control < 0x50) {
        const key = `beatlooproll_${shift ? "r" : ""}${PioneerDDJFLX4.padFxRolls[pad]}_activate`;
        hold = (held) => engine.setValue(group, key, held ? 1 : 0);
    } else if (!shift && pad === 6) {
        hold = (held) => engine.brake(deck, held);
    } else if (!shift && pad === 7) {
        hold = (held) => engine.spinback(deck, held);
    } else if (shift && pad > 2) {
        return;
    } else {
        const unit = shift ? 4 : (pad < 3 ? 2 : 3);
        hold = (held) => PioneerDDJFLX4.holdEffect(group, unit, pad % 3 + 1, held);
    }
    hold(true);
    PioneerDDJFLX4.padFxReleases[id] = () => hold(false);
};

// Turns an effect of an effect unit on for this deck while a pad is held,
// and restores the unit afterwards
PioneerDDJFLX4.holdEffect = function(group, unit, slot, held) {
    const unitGroup = `[EffectRack1_EffectUnit${unit}]`;
    const slotGroup = `[EffectRack1_EffectUnit${unit}_Effect${slot}]`;
    const route = `group_${group}_enable`;
    const key = unitGroup + slot + group;

    if (held) {
        PioneerDDJFLX4.padFxHeld[key] = {
            route: engine.getValue(unitGroup, route),
            enabled: engine.getValue(slotGroup, "enabled"),
            mix: engine.getValue(unitGroup, "mix"),
        };
        engine.setValue(unitGroup, route, 1);
        if (engine.getValue(unitGroup, "mix") === 0) {
            engine.setValue(unitGroup, "mix", 1);
        }
        engine.setValue(slotGroup, "enabled", 1);
        return;
    }
    const before = PioneerDDJFLX4.padFxHeld[key];
    if (before === undefined) {
        return;
    }
    engine.setValue(slotGroup, "enabled", before.enabled);
    engine.setValue(unitGroup, route, before.route);
    engine.setValue(unitGroup, "mix", before.mix);
    delete PioneerDDJFLX4.padFxHeld[key];
};

//
// Fader Start (SHIFT + channel fader or crossfader)
//

PioneerDDJFLX4.faderStartPlay = function(_channel, _control, value, _status, group) {
    if (value && PioneerDDJFLX4.option("faderStart")) {
        engine.setValue(group, "play", 1);
    }
};

PioneerDDJFLX4.faderStartCue = function(_channel, _control, value, _status, group) {
    if (value && PioneerDDJFLX4.option("faderStart")) {
        script.triggerControl(group, "cue_gotoandstop");
    }
};

//
// LOAD, with Instant Doubles: pressed twice, loads the other deck's track
// at the same position
//

PioneerDDJFLX4.loadPressed = function(_channel, _control, value, _status, group) {
    if (value && PioneerDDJFLX4.option("instantDoubles")) {
        const now = Date.now();
        if (now - PioneerDDJFLX4.lastLoadPress[group] < PioneerDDJFLX4.instantDoublesTime) {
            PioneerDDJFLX4.lastLoadPress[group] = 0;
            engine.setValue(group, "CloneFromDeck", group === "[Channel1]" ? 2 : 1);
            return;
        }
        PioneerDDJFLX4.lastLoadPress[group] = now;
    }
    engine.setValue(group, "LoadSelectedTrack", value ? 1 : 0);
};

PioneerDDJFLX4.shutdown = function() {
    // reset vumeter
    PioneerDDJFLX4.toggleLight(PioneerDDJFLX4.lights.deck1.vuMeter, false);
    PioneerDDJFLX4.toggleLight(PioneerDDJFLX4.lights.deck2.vuMeter, false);

    // SMART CFX
    midi.sendShortMsg(0x96, 0x00, 0x00);

    // housekeeping
    // turn off all Sampler LEDs
    for (var i = 0; i <= 7; ++i) {
        midi.sendShortMsg(0x97, 0x30 + i, 0x00);    // Deck 1 pads
        midi.sendShortMsg(0x98, 0x30 + i, 0x00);    // Deck 1 pads with SHIFT
        midi.sendShortMsg(0x99, 0x30 + i, 0x00);    // Deck 2 pads
        midi.sendShortMsg(0x9A, 0x30 + i, 0x00);    // Deck 2 pads with SHIFT
    }
    // turn off all Hotcue LEDs
    for (i = 0; i <= 7; ++i) {
        midi.sendShortMsg(0x97, 0x00 + i, 0x00);    // Deck 1 pads
        midi.sendShortMsg(0x98, 0x00 + i, 0x00);    // Deck 1 pads with SHIFT
        midi.sendShortMsg(0x99, 0x00 + i, 0x00);    // Deck 2 pads
        midi.sendShortMsg(0x9A, 0x00 + i, 0x00);    // Deck 2 pads with SHIFT
    }

    // turn off all Pad FX and Keyboard LEDs
    for (i = 0; i <= 7; ++i) {
        [0x97, 0x98, 0x99, 0x9A].forEach((status) => {
            midi.sendShortMsg(status, 0x10 + i, 0x00);
            midi.sendShortMsg(status, 0x50 + i, 0x00);
            midi.sendShortMsg(status, 0x40 + i, 0x00);
        });
    }

    // turn off loop in and out lights
    PioneerDDJFLX4.setLoopButtonLights(0x90, 0x00);
    PioneerDDJFLX4.setLoopButtonLights(0x91, 0x00);

    // turn off reloop lights
    PioneerDDJFLX4.setReloopLight(0x90, 0x00);
    PioneerDDJFLX4.setReloopLight(0x91, 0x00);

    // stop any flashing lights
    PioneerDDJFLX4.toggleLight(PioneerDDJFLX4.lights.beatFx, false);
    PioneerDDJFLX4.toggleLight(PioneerDDJFLX4.lights.shiftBeatFx, false);

    // stop the keepalive timer
    engine.stopTimer(PioneerDDJFLX4.keepAliveTimer);
};
