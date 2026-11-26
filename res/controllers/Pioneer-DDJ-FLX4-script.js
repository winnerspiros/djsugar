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
//      * DJ Sugar: Keyboard mode, Pad FX1 and Pad FX2
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
//      * Stems selection using PADs (press KEYBOARD a second time)
//
//  Not implemented (after discussion and trial attempts):
//      * Loop Section:
//        * -4BEAT auto loop (hacky---prefer a clean way to set a 4 beat loop
//                            from a previous position on long press)
//
//        * CUE/LOOP CALL - memory & delete (complex and not useful. Hot cues are sufficient)
//
//  Not implemented yet (but might be in the future):
//      * Smart CFX (only SHIFT + SMART CFX, which steps the Color FX)
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
// DJ Sugar: whether touching the top scratches follows each deck's VINYL
// switch ([ChannelN] vinyl_mode, on by default) instead of this constant.
PioneerDDJFLX4.vinylMode = function(deckNum) {
    return engine.getValue("[Channel" + deckNum + "]", "vinyl_mode") > 0;
};
PioneerDDJFLX4.alpha = 1.0/8;
PioneerDDJFLX4.beta = PioneerDDJFLX4.alpha/32;

// Multiplier for fast seek through track using SHIFT+JOGWHEEL
// DJ Sugar: half of Mixxx's 150, which searched too fast.
PioneerDDJFLX4.fastSeekScale = 75;

PioneerDDJFLX4.tempoRanges = [0.06, 0.10, 0.16, 0.25];

PioneerDDJFLX4.shiftButtonDown = [false, false];

// Jog wheel loop adjust
PioneerDDJFLX4.loopAdjustIn = [false, false];
PioneerDDJFLX4.loopAdjustOut = [false, false];
PioneerDDJFLX4.loopAdjustMultiply = 50;

// Beatjump pad (beatjump_size values)
// DJ Sugar: 4, 8, 16 and 32 beats back and forward, as on the deck's screen.
PioneerDDJFLX4.beatjumpSizeForPad = {
    0x20: -4,  // PAD 1
    0x21: 4,   // PAD 2
    0x22: -8,  // PAD 3
    0x23: 8,   // PAD 4
    0x24: -16, // PAD 5
    0x25: 16,  // PAD 6
    0x26: -32, // PAD 7
    0x27: 32   // PAD 8
};

// Stems (KEYBOARD) pads mode status for deck 1 and 2, without or with SHIFT pressed
PioneerDDJFLX4.stemsPadsModesStatus = {
    "[Channel1]": [0x97, 0x98],
    "[Channel2]": [0x99, 0x9a],
};

// Stems (KEYBOARD) pad 1 control (pad control = [this value] + [pad  number] - 1)
PioneerDDJFLX4.stemMutePadsFirstControl = 0x40;

// Pitch shift (KEY SHIFT) pads mode status for deck 1 and 2, without or with SHIFT pressed
PioneerDDJFLX4.pitchPadsModesStatus = {
    "[Channel1]": [0x97, 0x98],
    "[Channel2]": [0x99, 0x9a],
};

// Pitch shift (KEY SHIFT) pad 1 control (pad control = [this value] + [pad  number] - 1)
PioneerDDJFLX4.pitchPadsFirstControl = 0x70;

PioneerDDJFLX4.quickJumpSize = 32;

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
    engine.makeConnection("[Channel1]", "vu_meter", PioneerDDJFLX4.vuMeterUpdate);
    engine.makeConnection("[Channel2]", "vu_meter", PioneerDDJFLX4.vuMeterUpdate);

    PioneerDDJFLX4.toggleLight(PioneerDDJFLX4.lights.deck1.vuMeter, false);
    PioneerDDJFLX4.toggleLight(PioneerDDJFLX4.lights.deck2.vuMeter, false);

    engine.softTakeover("[Channel1]", "rate", true);
    engine.softTakeover("[Channel2]", "rate", true);
    engine.softTakeover(PioneerDDJFLX4.beatFxSlot, "meta", true);

    const samplerCount = 16;
    if (engine.getValue("[App]", "num_samplers") < samplerCount) {
        engine.setValue("[App]", "num_samplers", samplerCount);
    }
    for (let i = 1; i <= samplerCount; ++i) {
        engine.makeConnection("[Sampler" + i + "]", "play", PioneerDDJFLX4.samplerPlayOutputCallbackFunction);
        engine.makeConnection("[Sampler" + i + "]", "track_loaded", PioneerDDJFLX4.samplerPlayOutputCallbackFunction);
    }
    engine.makeConnection("[Channel1]", "sampler_page", PioneerDDJFLX4.samplerPlayOutputCallbackFunction);
    engine.makeConnection("[Channel2]", "sampler_page", PioneerDDJFLX4.samplerPlayOutputCallbackFunction);
    PioneerDDJFLX4.samplerPlayOutputCallbackFunction();

    engine.makeConnection("[Channel1]", "track_loaded", PioneerDDJFLX4.trackLoadedLED);
    engine.makeConnection("[Channel2]", "track_loaded", PioneerDDJFLX4.trackLoadedLED);

    // play the "track loaded" animation on both decks at startup
    midi.sendShortMsg(0x9F, 0x00, 0x7F);
    midi.sendShortMsg(0x9F, 0x01, 0x7F);

    PioneerDDJFLX4.setLoopButtonLights(0x90, 0x7F);
    PioneerDDJFLX4.setLoopButtonLights(0x91, 0x7F);

    // DJ Sugar: the controller starts in HOT CUE mode; light its button.
    PioneerDDJFLX4.toggleLight(PioneerDDJFLX4.lights.deck1.hotcueMode, true);
    PioneerDDJFLX4.toggleLight(PioneerDDJFLX4.lights.deck2.hotcueMode, true);

    // DJ Sugar: the pads follow the pages in the deck's pad tabs.
    for (let tab = 1; tab <= 8; ++tab) {
        engine.makeConnection("[Channel1]", "pad_slot_" + tab, PioneerDDJFLX4.padSlotChanged);
        engine.makeConnection("[Channel2]", "pad_slot_" + tab, PioneerDDJFLX4.padSlotChanged);
    }
    engine.makeConnection("[Controller]", "deck3", function() { PioneerDDJFLX4.routePads(true); });
    engine.makeConnection("[Controller]", "deck4", function() { PioneerDDJFLX4.routePads(false); });
    PioneerDDJFLX4.routePads(true);
    PioneerDDJFLX4.routePads(false);

    engine.makeConnection("[Channel1]", "loop_enabled", PioneerDDJFLX4.loopToggle);
    engine.makeConnection("[Channel2]", "loop_enabled", PioneerDDJFLX4.loopToggle);

    engine.makeConnection(PioneerDDJFLX4.beatFxSlot, "enabled", PioneerDDJFLX4.toggleFxLight);

    // Register callbacks for each deck, when a file is loaded and the number of stems is available
    engine.makeConnection("[Channel1]", "stem_count", PioneerDDJFLX4.stemCountChanged);
    engine.makeConnection("[Channel2]", "stem_count", PioneerDDJFLX4.stemCountChanged);

    // DJ Sugar: and when the pads switch to and from the Stems FX layer.
    for (const deck of [1, 2]) {
        engine.makeConnection(`[Channel${deck}]`, "stem_fx_mode", PioneerDDJFLX4.stemCountChanged);
    }

    // Register callbacks for each stems of each decks, to change pad lights when muted/unmuted
    for (let stem=1; stem<=5; stem++) {
        for (let deck=1; deck<=2; deck++) {
            engine.makeConnection(`[Channel${deck}_Stem${stem}]`, "mute", PioneerDDJFLX4.stemMuteChanged);
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

// DJ Sugar: the BEAT FX section drives DJ Sugar's Beat FX unit, one effect
// sent to deck 1, deck 2 or both, as in rekordbox:
//   FX SELECT steps through the effects (SHIFT: backwards),
//   BEAT < > halve and double its length (SHIFT: AUTO tempo, TAP tempo),
//   LEVEL/DEPTH is the effect's depth (its meta knob),
//   ON/OFF turns it on and off (SHIFT: release, letting the tail ring out).
PioneerDDJFLX4.beatFxUnit = "[EffectRack1_EffectUnit5]";
PioneerDDJFLX4.beatFxSlot = "[EffectRack1_EffectUnit5_Effect1]";

PioneerDDJFLX4.shifted = function() {
    return PioneerDDJFLX4.shiftButtonDown[0] || PioneerDDJFLX4.shiftButtonDown[1];
};

PioneerDDJFLX4.toggleFxLight = function(_value, _group, _control) {
    const enabled = engine.getValue(PioneerDDJFLX4.beatFxSlot, "enabled");

    PioneerDDJFLX4.toggleLight(PioneerDDJFLX4.lights.beatFx, enabled);
    PioneerDDJFLX4.toggleLight(PioneerDDJFLX4.lights.shiftBeatFx, enabled);
};

PioneerDDJFLX4.beatFxLevelDepthRotate = function(_channel, _control, value) {
    engine.setParameter(PioneerDDJFLX4.beatFxSlot, "meta", value / 0x7F);
};

PioneerDDJFLX4.beatFxSelectPressed = function(_channel, _control, value) {
    if (value === 0) { return; }

    engine.setValue(PioneerDDJFLX4.beatFxSlot, "next_effect", 1);
    engine.setValue(PioneerDDJFLX4.beatFxSlot, "next_effect", 0);
};

PioneerDDJFLX4.beatFxSelectShiftPressed = function(_channel, _control, value) {
    if (value === 0) { return; }

    engine.setValue(PioneerDDJFLX4.beatFxSlot, "prev_effect", 1);
    engine.setValue(PioneerDDJFLX4.beatFxSlot, "prev_effect", 0);
};

// Halves (-1) or doubles (+1) the effect's length, 1/16 to 4 beats; effects
// without one (beats 0) stay as they are.
PioneerDDJFLX4.changeBeatFxBeats = function(direction) {
    const beats = engine.getValue(PioneerDDJFLX4.beatFxSlot, "beats");
    if (beats <= 0) { return; }
    const next = Math.min(4, Math.max(1 / 16, direction > 0 ? beats * 2 : beats / 2));
    engine.setValue(PioneerDDJFLX4.beatFxSlot, "beats", next);
};

PioneerDDJFLX4.pushBeatFxUnit = function(key) {
    engine.setValue(PioneerDDJFLX4.beatFxUnit, key, 1);
    engine.setValue(PioneerDDJFLX4.beatFxUnit, key, 0);
};

PioneerDDJFLX4.beatFxLeftPressed = function(_channel, _control, value) {
    if (value === 0) { return; }

    if (PioneerDDJFLX4.shifted()) {
        PioneerDDJFLX4.pushBeatFxUnit("bpm_auto");
    } else {
        PioneerDDJFLX4.changeBeatFxBeats(-1);
    }
};

PioneerDDJFLX4.beatFxRightPressed = function(_channel, _control, value) {
    if (value === 0) { return; }

    if (PioneerDDJFLX4.shifted()) {
        PioneerDDJFLX4.pushBeatFxUnit("bpm_tap");
    } else {
        PioneerDDJFLX4.changeBeatFxBeats(1);
    }
};

PioneerDDJFLX4.beatFxOnOffPressed = function(_channel, _control, value) {
    if (value === 0) { return; }

    const toggleEnabled = !engine.getValue(PioneerDDJFLX4.beatFxSlot, "enabled");
    engine.setValue(PioneerDDJFLX4.beatFxSlot, "enabled", toggleEnabled);
};

PioneerDDJFLX4.beatFxOnOffShiftPressed = function(_channel, _control, value) {
    if (value === 0) { return; }

    PioneerDDJFLX4.pushBeatFxUnit("release");
};

PioneerDDJFLX4.beatFxChannel1 = function(_channel, _control, value) {
    let enableChannel = 0;

    if (value === 0x7f) { enableChannel = 1; }

    engine.setValue(PioneerDDJFLX4.beatFxUnit, "group_[Channel1]_enable", enableChannel);
};

PioneerDDJFLX4.beatFxChannel2 = function(_channel, _control, value) {
    let enableChannel = 0;

    if (value === 0x7f) { enableChannel = 1; }

    engine.setValue(PioneerDDJFLX4.beatFxUnit, "group_[Channel2]_enable", enableChannel);
};

//
// Loop IN/OUT ADJUST
//

PioneerDDJFLX4.toggleLoopAdjustIn = function(channel, _control, value, _status, group) {
    if (value === 0 || engine.getValue(group, "loop_enabled" === 0)) {
        return;
    }
    PioneerDDJFLX4.loopAdjustIn[channel] = !PioneerDDJFLX4.loopAdjustIn[channel];
    PioneerDDJFLX4.loopAdjustOut[channel] = false;
};

PioneerDDJFLX4.toggleLoopAdjustOut = function(channel, _control, value, _status, group) {
    if (value === 0 || engine.getValue(group, "loop_enabled" === 0)) {
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

//
// CUE/LOOP CALL
//

PioneerDDJFLX4.cueLoopCallLeft = function(_channel, _control, value, _status, group) {
    // DJ Sugar: loop_halve rather than loop_scale, so the loop size box on
    // screen follows, and sets the next loop's size when none is on.
    engine.setValue(group, "loop_halve", value ? 1 : 0);
};

PioneerDDJFLX4.cueLoopCallRight = function(_channel, _control, value, _status, group) {
    // DJ Sugar: loop_double rather than loop_scale, so the loop size box on
    // screen follows, and sets the next loop's size when none is on.
    engine.setValue(group, "loop_double", value ? 1 : 0);
};

//
// BEAT SYNC
//
// Note that the controller sends different signals for a short press and a long
// press of the same button.
//

// DJ Sugar: a press turns sync on, as the SYNC button on screen does, and
// another turns it off.
PioneerDDJFLX4.syncPressed = function(channel, control, value, status, group) {
    if (value > 0) {
        script.toggleControl(group, "sync_enabled");
    }
};

PioneerDDJFLX4.syncLongPressed = function(channel, control, value, status, group) {
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
    } else {
        // DJ Sugar: the rim (and the platter when not touched) bends the
        // speed while it turns, faster clockwise and slower
        // counterclockwise, as in rekordbox. jog_bend takes rim ticks.
        engine.setValue(group, "jog_bend", newVal);
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

    if (value !== 0 && PioneerDDJFLX4.vinylMode(deckNum)) {
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
// Sampler mode
//

// DJ Sugar: each side's sampler pads play the 8 slots of its deck's sampler
// page (`sampler_page`: 0 slots 1-8, 1 slots 9-16), the same 8 the screen
// shows. The mapping names the rekordbox layout (left pads 1-4 and 9-12,
// right pads 5-8 and 13-16), which only says which pad was pressed.
// A side switched to deck 3 or 4 reads that deck's page.
PioneerDDJFLX4.samplerSide = function(left) {
    return left ? "[Channel1]" : "[Channel2]";
};

PioneerDDJFLX4.samplerSlot = function(group) {
    const n = parseInt(group.match(script.samplerRegEx)[1]) - 1;
    const left = n % 8 < 4;
    const pad = n % 4 + (n >= 8 ? 4 : 0);
    const page = engine.getValue(PioneerDDJFLX4.samplerSide(left), "sampler_page");
    return "[Sampler" + (page * 8 + pad + 1) + "]";
};

// Lights a side's sampler pads: on for a slot with a sample, blinking while
// it plays.
PioneerDDJFLX4.samplerLights = function(left) {
    const channel = left ? 0x97 : 0x99;
    const page = engine.getValue(PioneerDDJFLX4.samplerSide(left), "sampler_page");
    for (let pad = 0; pad < 8; ++pad) {
        const group = "[Sampler" + (page * 8 + pad + 1) + "]";
        PioneerDDJFLX4.stopSamplerBlink(channel, 0x30 + pad);
        if (engine.getValue(group, "play") === 1) {
            PioneerDDJFLX4.startSamplerBlink(channel, 0x30 + pad, group);
        } else {
            const on = engine.getValue(group, "track_loaded") ? 0x7F : 0x00;
            midi.sendShortMsg(channel, 0x30 + pad, on);
            midi.sendShortMsg(channel + 1, 0x30 + pad, on);
        }
    }
};

PioneerDDJFLX4.samplerPlayOutputCallbackFunction = function(_value, _group, _control) {
    PioneerDDJFLX4.samplerLights(true);
    PioneerDDJFLX4.samplerLights(false);
};

// DJ Sugar: the pad tab each pad mode button opens on the deck's screen (see
// the deck's `pad_tab` control): tabs 1-4 for the four buttons, 5-8 for the
// same buttons with SHIFT, whatever page is in the tab.
PioneerDDJFLX4.padTabForMode = {
    0x1B: 0, // HOT CUE
    0x1E: 1, // PAD FX1
    0x20: 2, // BEAT JUMP
    0x22: 3, // SAMPLER
    0x69: 4, // KEYBOARD (SHIFT + HOT CUE)
    0x6B: 5, // PAD FX2 (SHIFT + PAD FX1)
    0x6D: 6, // BEAT LOOP (SHIFT + BEAT JUMP)
    0x6F: 7, // KEY SHIFT (SHIFT + SAMPLER)
};

// DJ Sugar: the first pad note of each pad mode, in tab order.
PioneerDDJFLX4.padModeNotes = [0x00, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70];

// DJ Sugar: the pad notes that play each pad panel (numbers as for the deck's
// `pad_panel`): its own pad mode's, or for loop rolls notes of their own
// (see the LOOP ROLL controls). Stems share KEYBOARD's notes (keyboardStems).
PioneerDDJFLX4.panelNotes = [0x00, 0x10, 0x20, 0x30, 0x40, 0x60, 0x50, 0x40, 0x70, 0x78];

// DJ Sugar: the pads follow the screen. Each pad mode's pads play the panel in
// its tab (the deck's `pad_slot_N`), with that panel's lights, whatever it
// is: put STEMS in tab 4 and the SAMPLER button gives stems pads.
PioneerDDJFLX4.routePads = function(left) {
    const group = left ? "[Channel1]" : "[Channel2]";
    const channels = left ? [0x07, 0x08] : [0x09, 0x0A];
    for (let tab = 0; tab < 8; ++tab) {
        const panel = engine.getValue(group, "pad_slot_" + (tab + 1));
        const to = PioneerDDJFLX4.panelNotes[panel];
        for (let pad = 0; pad < 8; ++pad) {
            const from = PioneerDDJFLX4.padModeNotes[tab] + pad;
            channels.forEach(function(channel) {
                midi.routeNote(channel, from, to === undefined ? from : to + pad);
            });
        }
    }
    // KEYBOARD's notes play stems while the open tab shows them.
    const open = PioneerDDJFLX4.padTabForMode[PioneerDDJFLX4.padMode[group]];
    const stems = engine.getValue(group, "pad_slot_" + (open + 1)) === 4;
    if (stems !== PioneerDDJFLX4.keyboardStems[group]) {
        PioneerDDJFLX4.keyboardStems[group] = stems;
        PioneerDDJFLX4.keyboardLights(group);
    }
};

PioneerDDJFLX4.padSlotChanged = function(_value, group, _control) {
    PioneerDDJFLX4.routePads(group === "[Channel1]");
};

// DJ Sugar: the pad mode button last pressed on each deck.
PioneerDDJFLX4.padMode = {"[Channel1]": 0x1B, "[Channel2]": 0x1B};

// DJ Sugar: KEYBOARD plays the last hot cue pressed at eight pitches, as in
// rekordbox. Pressing KEYBOARD again turns its pads into the stems pads
// (see stemPads), and back.
PioneerDDJFLX4.keyboardStems = {"[Channel1]": false, "[Channel2]": false};

// DJ Sugar: turns off a deck's pad mode button lights.
PioneerDDJFLX4.padModeLightsOff = function(deck) {
    ["hotcueMode", "keyboardMode", "padFX1Mode", "padFX2Mode", "beatJumpMode",
        "beatLoopMode", "samplerMode", "keyShiftMode"].forEach(function(mode) {
        PioneerDDJFLX4.toggleLight(deck[mode], false);
    });
};

PioneerDDJFLX4.padModeKeyPressed = function(_channel, _control, value, _status, _group) {
    const deck = (_status === 0x90 ? PioneerDDJFLX4.lights.deck1 : PioneerDDJFLX4.lights.deck2);
    const group = _status === 0x90 ? "[Channel1]" : "[Channel2]";

    if (value && _control in PioneerDDJFLX4.padTabForMode) {
        const again = PioneerDDJFLX4.padMode[group] === _control;
        const tab = PioneerDDJFLX4.padTabForMode[_control];
        PioneerDDJFLX4.padMode[group] = _control;
        engine.setValue(group, "pad_tab", tab);
        // KEYBOARD again swaps its tab between keyboard and stems.
        const slot = "pad_slot_" + (tab + 1);
        const panel = engine.getValue(group, slot);
        if (_control === 0x69 && again && (panel === 7 || panel === 4)) {
            engine.setValue(group, slot, panel === 7 ? 4 : 7);
        }
        PioneerDDJFLX4.routePads(group === "[Channel1]");
    }
    // DJ Sugar: only the chosen mode stays lit.
    if (value) {
        PioneerDDJFLX4.padModeLightsOff(deck);
    }

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

//
// DJ Sugar: KEYBOARD mode
//

// The engine's KEYBOARD_SEMITONES, pad by pad (keyboard_N_activate).
PioneerDDJFLX4.keyboardSemitones = [0, 1, 2, 3, -4, -3, -2, -1];

PioneerDDJFLX4.keyboardPadPressed = function(channel, control, value, status, group) {
    if (PioneerDDJFLX4.keyboardStems[group]) {
        PioneerDDJFLX4.stemMutePadPressed(channel, control, value, status, group);
        return;
    }
    const pad = control - PioneerDDJFLX4.stemMutePadsFirstControl + 1;
    engine.setValue(group, "keyboard_" + pad + "_activate", value === 0x7F ? 1 : 0);
};

PioneerDDJFLX4.keyboardPadShiftPressed = function(channel, control, value, status, group) {
    if (PioneerDDJFLX4.keyboardStems[group]) {
        PioneerDDJFLX4.stemMutePadShiftPressed(channel, control, value, status, group);
        return;
    }
    // SHIFT + pad plays its pitch too.
    PioneerDDJFLX4.keyboardPadPressed(channel, control, value, status, group);
};

// Lights the KEYBOARD pads: the stems in stems mode, else the pad of the key
// shift playing.
PioneerDDJFLX4.keyboardLights = function(group) {
    if (PioneerDDJFLX4.keyboardStems[group]) {
        PioneerDDJFLX4.stemCountChanged(0, group, "stem_count");
        return;
    }
    const pitch = engine.getValue(group, "pitch_adjust");
    for (let i = 0; i < 8; i++) {
        const lit = Math.abs(PioneerDDJFLX4.keyboardSemitones[i] - pitch) < 0.01;
        PioneerDDJFLX4.stemsPadsModesStatus[group].forEach(function(status) {
            midi.sendShortMsg(status, PioneerDDJFLX4.stemMutePadsFirstControl + i, lit ? 0x7F : 0x00);
        });
    }
};

//
// DJ Sugar: PAD FX1 and PAD FX2
//

// What each pad holds on, as on the deck's screen (deck.slint's pad-fx and
// pad-fx2): a loaded_effect (1 Echo, 2 Reverb, 3 Filter, 4 Flanger,
// 5 Bitcrusher, 6 Phaser, 7 Gate, 8 Roll, 9 Echo Out, 10 Brake, 11 Noise,
// 12 Spiral) and where the meta knob sits.
PioneerDDJFLX4.padFx = {
    0x10: [1, 0.6], 0x11: [2, 0.7], 0x12: [3, 0.2], 0x13: [3, 0.8],
    0x14: [4, 0.7], 0x15: [6, 0.7], 0x16: [5, 0.7], 0x17: [1, 1.0],
    0x50: [8, 0.5], 0x51: [7, 0.5], 0x52: [9, 0.6], 0x53: [10, 0.5],
    0x54: [11, 0.7], 0x55: [12, 0.7], 0x56: [7, 0.9], 0x57: [2, 1.0],
};

// Holding a pad puts its effect on the deck through the deck's pad FX unit
// (units 6 to 9 for decks 1 to 4); letting go turns it off.
PioneerDDJFLX4.padFxPressed = function(_channel, control, value, status, group) {
    const fx = PioneerDDJFLX4.padFx[control];
    if (fx === undefined) {
        return;
    }
    // A side switched to deck 3 or 4 uses that deck's unit.
    const left = group === "[Channel1]";
    const layer = engine.getValue("[Controller]", left ? "deck3" : "deck4");
    const deck = (left ? 1 : 2) + (layer ? 2 : 0);
    const unit = "[EffectRack1_EffectUnit" + (deck + 5) + "]";
    const slot = "[EffectRack1_EffectUnit" + (deck + 5) + "_Effect1]";
    if (value === 0x7F) {
        engine.setValue(slot, "loaded_effect", fx[0]);
        engine.setValue(slot, "meta", fx[1]);
        engine.setValue(unit, "mix", 1);
        engine.setValue(unit, "enabled", 1);
        engine.setValue(unit, "group_[Channel" + deck + "]_enable", 1);
        engine.setValue(slot, "enabled", 1);
    } else {
        engine.setValue(slot, "enabled", 0);
    }
    midi.sendShortMsg(status, control, value === 0x7F ? 0x7F : 0x00);
};

PioneerDDJFLX4.samplerPadPressed = function(_channel, _control, value, _status, mapped) {
    const group = PioneerDDJFLX4.samplerSlot(mapped);
    if (engine.getValue(group, "track_loaded")) {
        engine.setValue(group, "cue_gotoandplay", value);
    } else {
        engine.setValue(group, "LoadSelectedTrack", value);
    }
};

PioneerDDJFLX4.samplerPadShiftPressed = function(_channel, _control, value, _status, mapped) {
    const group = PioneerDDJFLX4.samplerSlot(mapped);
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
    if (value) {
        script.toggleControl(group, "quantize");
    }
};

PioneerDDJFLX4.quickJumpForward = function(_channel, _control, value, _status, group) {
    if (value) {
        engine.setValue(group, "beatjump", PioneerDDJFLX4.quickJumpSize);
    }
};

PioneerDDJFLX4.quickJumpBack = function(_channel, _control, value, _status, group) {
    if (value) {
        engine.setValue(group, "beatjump", -PioneerDDJFLX4.quickJumpSize);
    }
};

//
// Stems mode
//

// DJ Sugar: the stems pads, laid out as on screen (VirtualDJ's): Vocal,
// Instru, Bass, Acapella on top; Kick, HiHat, Stems FX, Instrumental below.
// A number from 1 is the stem ([ChannelN_StemM]: 1 kick, 2 bass, 3
// instruments, 4 vocals, 5 hi-hats); 0 is Acapella, -1 Instrumental and -2
// Stems FX, which switches the pads to echo out ([ChannelN],stem_fx_mode):
// a stem's pad then sends it into the deck's stem effect and mutes it, and
// brings it back dry on a second press.
PioneerDDJFLX4.stemPads = [4, 3, 2, 0, 1, 5, -2, -1];

PioneerDDJFLX4.stemGroup = function(group, stem) {
    return `[${group.substring(1, group.length-1)}_Stem${stem}]`;
};

// Whether each of the deck's stems plays (index 1 to 5); a stem the track
// doesn't have counts as playing.
PioneerDDJFLX4.stemsOn = function(group) {
    const count = engine.getValue(group, "stem_count");
    const on = [false];
    for (let stem = 1; stem <= 5; stem++) {
        on.push(stem > count || engine.getValue(PioneerDDJFLX4.stemGroup(group, stem), "mute") <= 0.5);
    }
    return on;
};

// Whether the pad for `kind` (see stemPads) is lit.
PioneerDDJFLX4.stemPadLit = function(group, kind) {
    const count = engine.getValue(group, "stem_count");
    if (count < 4) {
        return false;
    }
    const on = PioneerDDJFLX4.stemsOn(group);
    const others = [1, 2, 3, 5].filter(function(s) { return s <= count; });
    if (kind > 0) {
        return kind <= count && on[kind];
    } else if (kind === 0) {
        return on[4] && others.every(function(s) { return !on[s]; });
    } else if (kind === -1) {
        return !on[4] && others.every(function(s) { return on[s]; });
    } else if (kind === -2) {
        return engine.getValue(group, "stem_fx_mode") > 0;
    }
    return false;
};

PioneerDDJFLX4.stemFxGroup = function(group, stem) {
    return `[QuickEffectRack1_[${group.substring(1, group.length-1)}_Stem${stem}]]`;
};

// Echoes a stem out (sent into the stem effect and muted) or, with `out`
// false, brings it back dry. A stem already silent stays as it is.
PioneerDDJFLX4.echoStem = function(group, stem, out) {
    const stemGroup = PioneerDDJFLX4.stemGroup(group, stem);
    const fxGroup = PioneerDDJFLX4.stemFxGroup(group, stem);
    if (!out) {
        engine.setValue(stemGroup, "mute", 0);
        engine.setValue(fxGroup, "enabled", 0);
    } else if (engine.getValue(stemGroup, "mute") <= 0.5) {
        engine.setValue(fxGroup, "enabled", 1);
        engine.setValue(stemGroup, "mute", 1);
    }
};

// Outside the Stems FX layer the pads cut and bring back stems dry.
PioneerDDJFLX4.clearStemFx = function(group) {
    for (let stem = 1; stem <= 5; stem++) {
        engine.setValue(PioneerDDJFLX4.stemFxGroup(group, stem), "enabled", 0);
    }
};

// Mutes or unmutes the vocals and the rest.
PioneerDDJFLX4.setStems = function(group, vocals, rest) {
    const echo = engine.getValue(group, "stem_fx_mode") > 0;
    const count = engine.getValue(group, "stem_count");
    for (let stem = 1; stem <= 5; stem++) {
        const on = stem === 4 ? vocals : rest;
        if (echo) {
            if (stem <= count) {
                PioneerDDJFLX4.echoStem(group, stem, !on);
            }
        } else {
            engine.setValue(PioneerDDJFLX4.stemGroup(group, stem), "mute", on ? 0 : 1);
        }
    }
    if (!echo) {
        PioneerDDJFLX4.clearStemFx(group);
    }
};

PioneerDDJFLX4.stemMutePadPressed = function(_channel, control, value, _status, group) {
    if (value !== 0x7f) {
        return;
    }
    const kind = PioneerDDJFLX4.stemPads[control - PioneerDDJFLX4.stemMutePadsFirstControl];
    const count = engine.getValue(group, "stem_count");
    if (count < 4 || kind === undefined || kind > count) {
        return;
    }
    if (kind > 0) {
        const stemGroup = PioneerDDJFLX4.stemGroup(group, kind);
        const playing = engine.getValue(stemGroup, "mute") <= 0.5;
        if (engine.getValue(group, "stem_fx_mode") > 0) {
            PioneerDDJFLX4.echoStem(group, kind, playing);
        } else {
            engine.setValue(stemGroup, "mute", playing ? 1 : 0);
            engine.setValue(PioneerDDJFLX4.stemFxGroup(group, kind), "enabled", 0);
        }
    } else if (kind === -2) {
        engine.setValue(group, "stem_fx_mode", engine.getValue(group, "stem_fx_mode") > 0 ? 0 : 1);
    } else if (kind === 0) {
        PioneerDDJFLX4.setStems(group, true, PioneerDDJFLX4.stemPadLit(group, 0));
    } else if (kind === -1) {
        PioneerDDJFLX4.setStems(group, PioneerDDJFLX4.stemPadLit(group, -1), true);
    }
};

// SHIFT + a stem's pad plays that stem alone.
PioneerDDJFLX4.stemMutePadShiftPressed = function(_channel, control, value, _status, group) {
    if (value !== 0x7f) {
        return;
    }
    const kind = PioneerDDJFLX4.stemPads[control - PioneerDDJFLX4.stemMutePadsFirstControl];
    const count = engine.getValue(group, "stem_count");
    if (count < 4 || kind === undefined || kind <= 0 || kind > count) {
        PioneerDDJFLX4.stemMutePadPressed(_channel, control, value, _status, group);
        return;
    }
    for (let stem = 1; stem <= count; stem++) {
        engine.setValue(PioneerDDJFLX4.stemGroup(group, stem), "mute", stem === kind ? 0 : 1);
    }
};

PioneerDDJFLX4.stemCountChanged = function(_value, group, _control) {
    const channel = group.match(/\[Channel\d+/)[0] + "]";
    if (!PioneerDDJFLX4.keyboardStems[channel]) {
        return;
    }
    for (let pad = 0; pad < 8; pad++) {
        const lit = PioneerDDJFLX4.stemPadLit(channel, PioneerDDJFLX4.stemPads[pad]);
        PioneerDDJFLX4.stemsPadsModesStatus[channel].forEach(function(status) {
            midi.sendShortMsg(status, PioneerDDJFLX4.stemMutePadsFirstControl + pad, lit ? 0x7F : 0x00);
        });
    }
};

PioneerDDJFLX4.stemMuteChanged = PioneerDDJFLX4.stemCountChanged;

//
// Pitch Shift mode
//

PioneerDDJFLX4.pitchAdjusted = function(_value, group, _control) {
    // DJ Sugar: the KEYBOARD pads show the key shift too.
    PioneerDDJFLX4.keyboardLights(group);
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

PioneerDDJFLX4.shutdown = function() {
    // reset vumeter
    PioneerDDJFLX4.toggleLight(PioneerDDJFLX4.lights.deck1.vuMeter, false);
    PioneerDDJFLX4.toggleLight(PioneerDDJFLX4.lights.deck2.vuMeter, false);

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
