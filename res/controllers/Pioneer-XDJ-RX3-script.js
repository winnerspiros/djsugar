// Pioneer-XDJ-RX3-script.js
// ****************************************************************************
// * DJ Sugar mapping script for the Pioneer XDJ-RX3 in PC (MIDI) control mode.
// * Written from AlphaTheta's "XDJ-RX3 MIDI Message List", reusing DJ Sugar's
// * DDJ-FLX4/FLX6 scripts (from Mixxx's DDJ-400 and DDJ-FLX4 mappings:
// * Warker, nschloe, dj3730, jusko, Robert904).
// ****************************************************************************
//
//  The XDJ-RX3 sends each deck on its own MIDI channel (decks 1 to 4 on
//  channels 1 to 4, DECK 1/3 and 2/4 pick which), the mixer and BEAT FX on
//  channel 5 and each deck's pads on channels 6 to 9. Its SHIFT button sends
//  nothing: every control sends its own message with SHIFT held.
//
//  On start the script puts the mixer in MIDI control mode, so DJ Sugar mixes
//  and plays the master through the XDJ-RX3's sound card; it puts it back on
//  shutdown.
//
//  The pad mode buttons open the deck's pad tabs as on the FLX4: HOT CUE,
//  BEAT LOOP and BEAT JUMP their tabs, SLIP LOOP the loop rolls, and
//  SHIFT + them (the EXTENSION pages) PAD FX, SAMPLER, KEYBOARD and KEY
//  SHIFT. The pads play whatever page is in the tab.
//
//  The jog displays show the playhead going round and the cue point.
//
//  Not mapped yet: CUE/LOOP MEMORY/DELETE, TIME MODE, BEAT FX TIME, X-PAD,
//  PARAMETER, MASTER CUE, LINK CUE, crossfader assign, AUX, TRACK FILTER.

var PioneerXDJRX3 = {};

PioneerXDJRX3.decks = ["[Channel1]", "[Channel2]", "[Channel3]", "[Channel4]"];

PioneerXDJRX3.deckIndex = function(group) {
    return parseInt(group.match(script.channelRegEx)[1]) - 1;
};

// The status of a deck's notes and its pads.
PioneerXDJRX3.deckStatus = function(group) {
    return 0x90 + PioneerXDJRX3.deckIndex(group);
};
PioneerXDJRX3.padStatus = function(group) {
    return 0x95 + PioneerXDJRX3.deckIndex(group);
};

// Pads light in a colour number (1-127); 0 dims them.
PioneerXDJRX3.padOn = 0x7F;

// Lights a pad note and the same pad with SHIFT (8 notes on).
PioneerXDJRX3.padLight = function(group, note, on) {
    const status = PioneerXDJRX3.padStatus(group);
    const value = on ? PioneerXDJRX3.padOn : 0x00;
    midi.sendShortMsg(status, note, value);
    midi.sendShortMsg(status, note + 8, value);
};

PioneerXDJRX3.timers = {};

// Jog wheel constants
PioneerXDJRX3.vinylMode = [true, true, true, true];
PioneerXDJRX3.alpha = 1.0/8;
PioneerXDJRX3.beta = PioneerXDJRX3.alpha/32;
PioneerXDJRX3.fastSeekScale = 75;

PioneerXDJRX3.tempoRanges = [0.06, 0.10, 0.16, 0.25];

PioneerXDJRX3.loopAdjustIn = [false, false, false, false];
PioneerXDJRX3.loopAdjustOut = [false, false, false, false];
PioneerXDJRX3.loopAdjustMultiply = 50;

// Beatjump pads, by the note they play (see panelNotes).
PioneerXDJRX3.beatjumpSizeForPad = {
    0x28: -4, 0x29: 4, 0x2A: -8, 0x2B: 8, 0x2C: -16, 0x2D: 16, 0x2E: -32, 0x2F: 32,
};

// The notes the KEYBOARD (and stems) and KEY SHIFT pages play.
PioneerXDJRX3.stemMutePadsFirstControl = 0x40;
PioneerXDJRX3.pitchPadsFirstControl = 0x60;

PioneerXDJRX3.quickJumpSize = 32;

PioneerXDJRX3.highResMSB = {
    "[Channel1]": {}, "[Channel2]": {}, "[Channel3]": {}, "[Channel4]": {},
};

//
// Init
//

PioneerXDJRX3.init = function() {
    // DJ Sugar mixes: the mixer sends MIDI rather than mixing itself.
    midi.sendShortMsg(0xB4, 0x65, 0x00);

    PioneerXDJRX3.decks.forEach(function(group, i) {
        engine.makeConnection(group, "vu_meter", PioneerXDJRX3.vuMeterUpdate);
        midi.sendShortMsg(0xB0 + i, 0x02, 0);
        engine.softTakeover(group, "rate", true);

        engine.makeConnection(group, "sampler_page", PioneerXDJRX3.samplerPlayOutputCallbackFunction);
        engine.makeConnection(group, "loop_enabled", PioneerXDJRX3.loopToggle);
        PioneerXDJRX3.setLoopButtonLights(PioneerXDJRX3.deckStatus(group), 0x7F);
        midi.sendShortMsg(PioneerXDJRX3.deckStatus(group), 0x22, 0x7F);
        midi.sendShortMsg(PioneerXDJRX3.deckStatus(group), 0x12, 0x7F);

        for (let tab = 1; tab <= 8; ++tab) {
            engine.makeConnection(group, "pad_slot_" + tab, PioneerXDJRX3.padSlotChanged);
        }
        engine.makeConnection(group, "stem_count", PioneerXDJRX3.stemCountChanged);
        for (let stem = 1; stem <= 5; stem++) {
            engine.makeConnection(PioneerXDJRX3.stemGroup(group, stem), "mute", PioneerXDJRX3.stemMuteChanged);
        }
        engine.makeConnection(group, "track_loaded", PioneerXDJRX3.pitchAdjusted);
        engine.makeConnection(group, "pitch_adjust", PioneerXDJRX3.pitchAdjusted);
        engine.makeConnection(group, "track_loaded", PioneerXDJRX3.trackLoadedLED);
        engine.makeConnection(group, "sync_leader", PioneerXDJRX3.syncLeaderLight);
        engine.makeConnection("[QuickEffectRack1_" + group + "]", "loaded_chain_preset", PioneerXDJRX3.colorFxLights);
        PioneerXDJRX3.routePads(group);
    });
    engine.softTakeover(PioneerXDJRX3.beatFxSlot, "meta", true);

    const samplerCount = 16;
    if (engine.getValue("[App]", "num_samplers") < samplerCount) {
        engine.setValue("[App]", "num_samplers", samplerCount);
    }
    for (let i = 1; i <= samplerCount; ++i) {
        engine.makeConnection("[Sampler" + i + "]", "play", PioneerXDJRX3.samplerPlayOutputCallbackFunction);
        engine.makeConnection("[Sampler" + i + "]", "track_loaded", PioneerXDJRX3.samplerPlayOutputCallbackFunction);
    }
    PioneerXDJRX3.samplerPlayOutputCallbackFunction();

    engine.makeConnection(PioneerXDJRX3.beatFxSlot, "enabled", PioneerXDJRX3.beatFxLight);
    PioneerXDJRX3.colorFxLights();

    PioneerXDJRX3.jogDisplays = [[-1, -1], [-1, -1], [-1, -1], [-1, -1]];
    PioneerXDJRX3.jogDisplayTimer = engine.beginTimer(40, PioneerXDJRX3.jogDisplayUpdate);
};

//
// Lights
//

// The LOAD button blinks when a track loads.
PioneerXDJRX3.trackLoadedLED = function(value, group) {
    midi.sendShortMsg(0x9B, PioneerXDJRX3.deckIndex(group), value > 0 ? 0x7F : 0x00);
};

// The channel meters light from 60 (-24 dB) to 109 (+15 dB).
PioneerXDJRX3.vuMeterUpdate = function(value, group) {
    const level = value > 0.01 ? Math.round(55 + value * 54) : 0;
    midi.sendShortMsg(0xB0 + PioneerXDJRX3.deckIndex(group), 0x02, level);
};

// A 14-bit value as two CCs on the jog display channel (12).
PioneerXDJRX3.sendJogDisplay = function(msb, value) {
    midi.sendShortMsg(0xBB, msb, (value >> 7) & 0x7F);
    midi.sendShortMsg(0xBB, msb + 0x20, value & 0x7F);
};

// The jog display's position bar (0 to 359 degrees) goes round once per
// turn of a record at 33 1/3 rpm; its cue marker shows where the cue point
// sits on that turn, hidden with no track.
PioneerXDJRX3.jogDisplayUpdate = function() {
    const degrees = function(seconds) {
        const turns = Math.max(0, seconds) * (100 / 3) / 60;
        return Math.floor((turns - Math.floor(turns)) * 360) % 360;
    };
    PioneerXDJRX3.decks.forEach(function(group, i) {
        let bar = 0;
        let cue = 0x3FFF;
        if (engine.getValue(group, "track_loaded")) {
            const duration = engine.getValue(group, "duration");
            bar = degrees(engine.getValue(group, "playposition") * duration);
            const samples = engine.getValue(group, "track_samples");
            const rate = engine.getValue(group, "track_samplerate");
            const point = engine.getValue(group, "cue_point");
            if (samples > 0 && rate > 0 && point >= 0) {
                cue = degrees(point / rate / 2);
            }
        }
        const last = PioneerXDJRX3.jogDisplays[i];
        if (bar !== last[0]) {
            last[0] = bar;
            PioneerXDJRX3.sendJogDisplay(i, bar);
        }
        if (cue !== last[1]) {
            last[1] = cue;
            PioneerXDJRX3.sendJogDisplay(4 + i, cue);
        }
    });
};

//
// Browser
//

PioneerXDJRX3.waveformZoom = function(_channel, _control, value) {
    const up = value < 0x40;
    PioneerXDJRX3.decks.forEach(function(group) {
        script.triggerControl(group, up ? "waveform_zoom_up" : "waveform_zoom_down", 100);
    });
};

//
// Deck
//

PioneerXDJRX3.syncPressed = function(_channel, _control, value, _status, group) {
    if (value > 0) {
        script.toggleControl(group, "sync_enabled");
    }
};

PioneerXDJRX3.masterPressed = function(_channel, _control, value, _status, group) {
    if (value) {
        engine.setValue(group, "sync_leader", engine.getValue(group, "sync_leader") === 2 ? 0 : 2);
    }
};

PioneerXDJRX3.syncLeaderLight = function(value, group) {
    midi.sendShortMsg(PioneerXDJRX3.deckStatus(group), 0x1E, value > 0 ? 0x7F : 0x00);
};

PioneerXDJRX3.cycleTempoRange = function(_channel, _control, value, _status, group) {
    if (value === 0) { return; }
    const curr = engine.getValue(group, "rateRange");
    let idx = 0;
    for (let i = 0; i < PioneerXDJRX3.tempoRanges.length; i++) {
        if (curr === PioneerXDJRX3.tempoRanges[i]) {
            idx = (i + 1) % PioneerXDJRX3.tempoRanges.length;
            break;
        }
    }
    engine.setValue(group, "rateRange", PioneerXDJRX3.tempoRanges[idx]);
};

PioneerXDJRX3.toggleButton = function(key) {
    return function(_channel, _control, value, _status, group) {
        if (value) {
            script.toggleControl(group, key);
        }
    };
};
PioneerXDJRX3.togglePlay = PioneerXDJRX3.toggleButton("play");
PioneerXDJRX3.toggleKeylock = PioneerXDJRX3.toggleButton("keylock");
PioneerXDJRX3.toggleQuantize = PioneerXDJRX3.toggleButton("quantize");
PioneerXDJRX3.toggleSlip = PioneerXDJRX3.toggleButton("slip_enabled");
PioneerXDJRX3.toggleReverse = PioneerXDJRX3.toggleButton("reverse");

// JOG MODE: VINYL (scratch on touch) or CDJ (the top bends too).
PioneerXDJRX3.jogModePressed = function(channel, _control, value, status) {
    if (value) {
        PioneerXDJRX3.vinylMode[channel] = !PioneerXDJRX3.vinylMode[channel];
        midi.sendShortMsg(status, 0x12, PioneerXDJRX3.vinylMode[channel] ? 0x7F : 0x00);
    }
};

PioneerXDJRX3.quickJumpForward = function(_channel, _control, value, _status, group) {
    if (value) {
        engine.setValue(group, "beatjump", PioneerXDJRX3.quickJumpSize);
    }
};

PioneerXDJRX3.quickJumpBack = function(_channel, _control, value, _status, group) {
    if (value) {
        engine.setValue(group, "beatjump", -PioneerXDJRX3.quickJumpSize);
    }
};

// NEEDLE POSITION: touching the strip on the waveform jumps there.
PioneerXDJRX3.needleSearch = function(_channel, _control, value, _status, group) {
    if (value > 0 && !engine.getValue(group, "play")) {
        engine.setValue(group, "playposition", (value - 1) / 126);
    }
};

PioneerXDJRX3.tempoSliderMSB = function(_channel, _control, value, _status, group) {
    PioneerXDJRX3.highResMSB[group].tempoSlider = value;
};

// "-" side 0, "+" side 16383.
PioneerXDJRX3.tempoSliderLSB = function(_channel, _control, value, _status, group) {
    const fullValue = (PioneerXDJRX3.highResMSB[group].tempoSlider << 7) + value;
    engine.setValue(group, "rate", 1 - (fullValue / 0x2000));
};

//
// Jog wheels
//

PioneerXDJRX3.jogTurn = function(channel, _control, value, _status, group) {
    const deckNum = channel + 1;
    let newVal = value - 64;

    if (engine.getValue(group, "loop_enabled") > 0) {
        if (PioneerXDJRX3.loopAdjustIn[channel]) {
            engine.setValue(group, "loop_start_position",
                newVal * PioneerXDJRX3.loopAdjustMultiply + engine.getValue(group, "loop_start_position"));
            return;
        }
        if (PioneerXDJRX3.loopAdjustOut[channel]) {
            engine.setValue(group, "loop_end_position",
                newVal * PioneerXDJRX3.loopAdjustMultiply + engine.getValue(group, "loop_end_position"));
            return;
        }
    }

    if (engine.isScratching(deckNum)) {
        engine.scratchTick(deckNum, newVal);
    } else {
        engine.setValue(group, "jog_bend", newVal);
    }
};

PioneerXDJRX3.jogSearch = function(_channel, _control, value, _status, group) {
    engine.setValue(group, "jog", (value - 64) * PioneerXDJRX3.fastSeekScale);
};

PioneerXDJRX3.jogTouch = function(channel, _control, value) {
    const deckNum = channel + 1;
    if (PioneerXDJRX3.loopAdjustIn[channel] || PioneerXDJRX3.loopAdjustOut[channel]) {
        return;
    }
    if (value !== 0 && PioneerXDJRX3.vinylMode[channel]) {
        engine.scratchEnable(deckNum, 720, 33+1/3, PioneerXDJRX3.alpha, PioneerXDJRX3.beta);
    } else {
        engine.scratchDisable(deckNum);
    }
};

//
// Loops: IN (long press: adjust), OUT, RELOOP/EXIT
//

PioneerXDJRX3.toggleLoopAdjustIn = function(channel, _control, value, _status, group) {
    if (value === 0 || !engine.getValue(group, "loop_enabled")) {
        return;
    }
    PioneerXDJRX3.loopAdjustIn[channel] = !PioneerXDJRX3.loopAdjustIn[channel];
    PioneerXDJRX3.loopAdjustOut[channel] = false;
};

PioneerXDJRX3.setLoopButtonLights = function(status, value) {
    midi.sendShortMsg(status, 0x06, value);
    midi.sendShortMsg(status, 0x07, value);
};

PioneerXDJRX3.loopToggle = function(value, group, control) {
    const status = PioneerXDJRX3.deckStatus(group),
        channel = PioneerXDJRX3.deckIndex(group);
    midi.sendShortMsg(status, 0x08, value ? 0x7F : 0x00);
    PioneerXDJRX3.timers[group] = PioneerXDJRX3.timers[group] || {};
    if (PioneerXDJRX3.timers[group][control] !== undefined) {
        engine.stopTimer(PioneerXDJRX3.timers[group][control]);
        PioneerXDJRX3.timers[group][control] = undefined;
    }
    if (value) {
        let blink = 0x7F;
        PioneerXDJRX3.timers[group][control] = engine.beginTimer(500, () => {
            blink = 0x7F - blink;
            midi.sendShortMsg(status, 0x06, PioneerXDJRX3.loopAdjustOut[channel] ? 0x00 : blink);
            midi.sendShortMsg(status, 0x07, PioneerXDJRX3.loopAdjustIn[channel] ? 0x00 : blink);
        });
    } else {
        PioneerXDJRX3.setLoopButtonLights(status, 0x7F);
        PioneerXDJRX3.loopAdjustIn[channel] = false;
        PioneerXDJRX3.loopAdjustOut[channel] = false;
    }
};

PioneerXDJRX3.cueLoopCallLeft = function(_channel, _control, value, _status, group) {
    engine.setValue(group, "loop_halve", value ? 1 : 0);
};

PioneerXDJRX3.cueLoopCallRight = function(_channel, _control, value, _status, group) {
    engine.setValue(group, "loop_double", value ? 1 : 0);
};

//
// Mixer
//

PioneerXDJRX3.pflPressed = function(_channel, control, value) {
    if (value) {
        script.toggleControl(PioneerXDJRX3.decks[control - 0x46], "pfl");
    }
};

// COLOR FX buttons: each picks the Color FX the COLOR knobs play; pressing
// the lit one goes back to the filter. Numbers as in the engine's COLOR_FX
// (1 Filter, 2 Space, 3 Dub Echo, 4 Noise, 5 Crush); SWEEP plays the filter.
PioneerXDJRX3.colorFxButtons = {0x69: 2, 0x6B: 3, 0x6A: 1, 0x55: 4, 0x56: 5, 0x57: 1};

PioneerXDJRX3.colorFxPressed = function(_channel, control, value) {
    if (!value) { return; }
    const preset = PioneerXDJRX3.colorFxButtons[control];
    const current = engine.getValue("[QuickEffectRack1_[Channel1]]", "loaded_chain_preset");
    const next = current === preset && control !== 0x57 ? 1 : preset;
    PioneerXDJRX3.decks.forEach(function(group) {
        engine.setValue("[QuickEffectRack1_" + group + "]", "loaded_chain_preset", next);
    });
};

PioneerXDJRX3.colorFxLights = function() {
    const current = engine.getValue("[QuickEffectRack1_[Channel1]]", "loaded_chain_preset");
    Object.keys(PioneerXDJRX3.colorFxButtons).forEach(function(control) {
        const preset = PioneerXDJRX3.colorFxButtons[control];
        const lit = preset === current && !(preset === 1 && parseInt(control) === 0x6A);
        midi.sendShortMsg(0xB4, parseInt(control), lit ? 0x7F : 0x00);
    });
};

//
// BEAT FX: DJ Sugar's Beat FX unit
//

PioneerXDJRX3.beatFxUnit = "[EffectRack1_EffectUnit5]";
PioneerXDJRX3.beatFxSlot = "[EffectRack1_EffectUnit5_Effect1]";

// EFFECT SELECT, by the CC each position sends, to the engine's effects
// (loaded_effect: 1 Echo, 2 Reverb, 3 Filter, 4 Flanger, 6 Phaser, 7 Gate,
// 8 Roll, 10 Brake, 12 Spiral).
PioneerXDJRX3.beatFxEffects = {
    0x2A: 1, // DELAY
    0x37: 1, // ECHO
    0x33: 1, // PING PONG
    0x2B: 12, // SPIRAL
    0x36: 2, // REVERB
    0x35: 7, // TRANS
    0x3B: 3, // FILTER
    0x32: 4, // FLANGER
    0x39: 6, // PHASER
    0x3F: 4, // PITCH (no pitch effect yet)
    0x3A: 8, // SLIP ROLL
    0x2E: 8, // ROLL
    0x3D: 10, // VINYL BRAKE
    0x3E: 12, // HELIX
};

PioneerXDJRX3.beatFxSelect = function(_channel, control, value) {
    if (value === 0x7F) {
        engine.setValue(PioneerXDJRX3.beatFxSlot, "loaded_effect", PioneerXDJRX3.beatFxEffects[control]);
    }
};

PioneerXDJRX3.beatFxLevelDepthRotate = function(_channel, _control, value) {
    engine.setParameter(PioneerXDJRX3.beatFxSlot, "meta", value / 0x7F);
};

PioneerXDJRX3.changeBeatFxBeats = function(direction) {
    const beats = engine.getValue(PioneerXDJRX3.beatFxSlot, "beats");
    if (beats <= 0) { return; }
    const next = Math.min(4, Math.max(1 / 16, direction > 0 ? beats * 2 : beats / 2));
    engine.setValue(PioneerXDJRX3.beatFxSlot, "beats", next);
};

PioneerXDJRX3.pushBeatFxUnit = function(key) {
    engine.setValue(PioneerXDJRX3.beatFxUnit, key, 1);
    engine.setValue(PioneerXDJRX3.beatFxUnit, key, 0);
};

PioneerXDJRX3.beatFxLeftPressed = function(_channel, _control, value) {
    if (value) { PioneerXDJRX3.changeBeatFxBeats(-1); }
};

PioneerXDJRX3.beatFxRightPressed = function(_channel, _control, value) {
    if (value) { PioneerXDJRX3.changeBeatFxBeats(1); }
};

// TAP taps the tempo; SHIFT + TAP goes back to the track's.
PioneerXDJRX3.beatFxTapPressed = function(_channel, control, value) {
    if (value) { PioneerXDJRX3.pushBeatFxUnit(control === 0x7E ? "bpm_auto" : "bpm_tap"); }
};

PioneerXDJRX3.beatFxOnOffPressed = function(_channel, _control, value) {
    if (value) {
        engine.setValue(PioneerXDJRX3.beatFxSlot, "enabled", !engine.getValue(PioneerXDJRX3.beatFxSlot, "enabled"));
    }
};

PioneerXDJRX3.beatFxOnOffShiftPressed = function(_channel, _control, value) {
    if (value) { PioneerXDJRX3.pushBeatFxUnit("release"); }
};

PioneerXDJRX3.beatFxLight = function(value) {
    midi.sendShortMsg(0xB4, 0x72, value ? 0x7F : 0x00);
    midi.sendShortMsg(0xB4, 0x34, value ? 0x7F : 0x00);
};

// CH SELECT, by the CC each position sends, to the decks it sends the Beat
// FX: CH1 the left channel (decks 1 and 3), CH2 the right (2 and 4), CF.A
// and CF.B the same sides, MASTER all four, MIC and AUX none.
PioneerXDJRX3.beatFxChannels = {
    0x22: [1, 3], 0x23: [2, 4], 0x27: [1, 3], 0x28: [2, 4], 0x29: [1, 2, 3, 4], 0x1C: [], 0x20: [],
};

PioneerXDJRX3.beatFxChannelSelect = function(_channel, control, value) {
    if (value !== 0x7F) { return; }
    const decks = PioneerXDJRX3.beatFxChannels[control];
    PioneerXDJRX3.decks.forEach(function(group, i) {
        engine.setValue(PioneerXDJRX3.beatFxUnit, "group_" + group + "_enable", decks.indexOf(i + 1) >= 0 ? 1 : 0);
    });
};

//
// Pads
//

// The pad mode buttons (deck channel notes) and the pages they open: a pad
// tab of the deck (0-7, see the deck's pad_tab), or for SLIP LOOP the loop
// rolls. The pads send notes 0x00, 0x10 ... 0x70 on each page, 8 higher
// with SHIFT.
PioneerXDJRX3.padModes = {
    0x22: {page: 0, tab: 0}, // HOT CUE
    0x23: {page: 1, tab: 6}, // BEAT LOOP
    0x24: {page: 2, panel: 9}, // SLIP LOOP: loop rolls
    0x25: {page: 3, tab: 2}, // BEAT JUMP
    0x26: {page: 4, tab: 1}, // SHIFT + HOT CUE (EXTENSION 1): PAD FX
    0x27: {page: 5, tab: 3}, // SHIFT + BEAT LOOP (EXTENSION 2): SAMPLER
    0x28: {page: 6, tab: 4}, // SHIFT + SLIP LOOP (EXTENSION 3): KEYBOARD
    0x29: {page: 7, tab: 7}, // SHIFT + BEAT JUMP (EXTENSION 4): KEY SHIFT
};

// The notes the mapping binds for each pad panel (numbers as for the
// deck's pad_panel), and with SHIFT: the pads are routed onto them.
PioneerXDJRX3.panelNotes = {
    0: [0x00, 0x08], // hot cues
    1: [0x50, 0x50], // pad FX 1
    2: [0x28, 0x28], // beat jump
    3: [0x30, 0x38], // sampler
    4: [0x40, 0x48], // stems (on KEYBOARD's notes)
    5: [0x10, 0x10], // beat loop
    6: [0x18, 0x18], // pad FX 2
    7: [0x40, 0x48], // keyboard
    8: [0x60, 0x68], // key shift
    9: [0x20, 0x20], // loop roll
};

PioneerXDJRX3.padMode = {"[Channel1]": 0x22, "[Channel2]": 0x22, "[Channel3]": 0x22, "[Channel4]": 0x22};
PioneerXDJRX3.keyboardStems = {"[Channel1]": false, "[Channel2]": false, "[Channel3]": false, "[Channel4]": false};

PioneerXDJRX3.modePanel = function(group, mode) {
    return mode.tab === undefined ? mode.panel : engine.getValue(group, "pad_slot_" + (mode.tab + 1));
};

PioneerXDJRX3.routePads = function(group) {
    const channel = PioneerXDJRX3.padStatus(group) & 0x0F;
    Object.keys(PioneerXDJRX3.padModes).forEach(function(button) {
        const mode = PioneerXDJRX3.padModes[button];
        const notes = PioneerXDJRX3.panelNotes[PioneerXDJRX3.modePanel(group, mode)];
        for (let pad = 0; pad < 8; ++pad) {
            const from = mode.page * 0x10 + pad;
            midi.routeNote(channel, from, notes === undefined ? from : notes[0] + pad);
            midi.routeNote(channel, from + 8, notes === undefined ? from + 8 : notes[1] + pad);
        }
    });
    const stems = PioneerXDJRX3.modePanel(group, PioneerXDJRX3.padModes[PioneerXDJRX3.padMode[group]]) === 4;
    if (stems !== PioneerXDJRX3.keyboardStems[group]) {
        PioneerXDJRX3.keyboardStems[group] = stems;
        PioneerXDJRX3.keyboardLights(group);
    }
};

PioneerXDJRX3.padSlotChanged = function(_value, group) {
    PioneerXDJRX3.routePads(group);
};

PioneerXDJRX3.padModeKeyPressed = function(channel, control, value) {
    if (!value) { return; }
    const group = PioneerXDJRX3.decks[channel];
    const mode = PioneerXDJRX3.padModes[control];
    const again = PioneerXDJRX3.padMode[group] === control;
    PioneerXDJRX3.padMode[group] = control;
    if (mode.tab === undefined) {
        engine.setValue(group, "pad_panel", mode.panel);
    } else {
        engine.setValue(group, "pad_tab", mode.tab);
        // KEYBOARD again swaps its tab between keyboard and stems.
        const slot = "pad_slot_" + (mode.tab + 1);
        const panel = engine.getValue(group, slot);
        if (control === 0x28 && again && (panel === 7 || panel === 4)) {
            engine.setValue(group, slot, panel === 7 ? 4 : 7);
        }
    }
    PioneerXDJRX3.routePads(group);
    // Only the chosen mode stays lit (the EXTENSION pages light their
    // button's plain note).
    for (let n = 0x22; n <= 0x25; ++n) {
        midi.sendShortMsg(0x90 + channel, n, n === (control - 0x22) % 4 + 0x22 ? 0x7F : 0x00);
    }
};

//
// Sampler pads: each deck's pads play the 8 slots of its sampler page.
//

PioneerXDJRX3.samplerSlot = function(group, status) {
    const n = parseInt(group.match(script.samplerRegEx)[1]) - 1;
    const pad = n % 4 + (n >= 8 ? 4 : 0);
    const deck = PioneerXDJRX3.decks[(status & 0x0F) - 5];
    const page = engine.getValue(deck, "sampler_page");
    return "[Sampler" + (page * 8 + pad + 1) + "]";
};

PioneerXDJRX3.samplerLights = function(deck) {
    const page = engine.getValue(deck, "sampler_page");
    for (let pad = 0; pad < 8; ++pad) {
        const group = "[Sampler" + (page * 8 + pad + 1) + "]";
        PioneerXDJRX3.stopSamplerBlink(deck, 0x30 + pad);
        if (engine.getValue(group, "play") === 1) {
            PioneerXDJRX3.startSamplerBlink(deck, 0x30 + pad, group);
        } else {
            PioneerXDJRX3.padLight(deck, 0x30 + pad, engine.getValue(group, "track_loaded"));
        }
    }
};

PioneerXDJRX3.samplerPlayOutputCallbackFunction = function() {
    PioneerXDJRX3.decks.forEach(PioneerXDJRX3.samplerLights);
};

PioneerXDJRX3.startSamplerBlink = function(deck, control, group) {
    let on = true;
    PioneerXDJRX3.stopSamplerBlink(deck, control);
    PioneerXDJRX3.timers[deck + "sampler"][control] = engine.beginTimer(250, () => {
        on = !on;
        PioneerXDJRX3.padLight(deck, control, on);
        if (engine.getValue(group, "play") !== 1) {
            PioneerXDJRX3.stopSamplerBlink(deck, control);
            PioneerXDJRX3.padLight(deck, control, true);
        }
    });
};

PioneerXDJRX3.stopSamplerBlink = function(deck, control) {
    const timers = PioneerXDJRX3.timers[deck + "sampler"] = PioneerXDJRX3.timers[deck + "sampler"] || {};
    if (timers[control] !== undefined) {
        engine.stopTimer(timers[control]);
        timers[control] = undefined;
    }
};


//
// Beat jump pads: back on the left, forward on the right
//

PioneerXDJRX3.beatjumpPadPressed = function(_channel, control, value, _status, group) {
    if (value === 0) {
        return;
    }
    engine.setValue(group, "beatjump_size", Math.abs(PioneerXDJRX3.beatjumpSizeForPad[control]));
    engine.setValue(group, "beatjump", PioneerXDJRX3.beatjumpSizeForPad[control]);
};

//
// Keyboard
//

PioneerXDJRX3.keyboardSemitones = [0, 1, 2, 3, -4, -3, -2, -1];

PioneerXDJRX3.keyboardPadPressed = function(channel, control, value, status, group) {
    if (PioneerXDJRX3.keyboardStems[group]) {
        PioneerXDJRX3.stemMutePadPressed(channel, control, value, status, group);
        return;
    }
    const pad = control - PioneerXDJRX3.stemMutePadsFirstControl + 1;
    engine.setValue(group, "keyboard_" + pad + "_activate", value === 0x7F ? 1 : 0);
};

PioneerXDJRX3.keyboardPadShiftPressed = function(channel, control, value, status, group) {
    if (PioneerXDJRX3.keyboardStems[group]) {
        PioneerXDJRX3.stemMutePadShiftPressed(channel, control, value, status, group);
        return;
    }
    // SHIFT + pad plays its pitch too.
    PioneerXDJRX3.keyboardPadPressed(channel, control, value, status, group);
};

// Lights the KEYBOARD pads: the stems in stems mode, else the pad of the key
// shift playing.
PioneerXDJRX3.keyboardLights = function(group) {
    if (PioneerXDJRX3.keyboardStems[group]) {
        PioneerXDJRX3.stemCountChanged(0, group, "stem_count");
        return;
    }
    const pitch = engine.getValue(group, "pitch_adjust");
    for (let i = 0; i < 8; i++) {
        const lit = Math.abs(PioneerXDJRX3.keyboardSemitones[i] - pitch) < 0.01;
        PioneerXDJRX3.padLight(group, PioneerXDJRX3.stemMutePadsFirstControl + i, lit);
    }
};

//
// DJ Sugar: PAD FX1 and PAD FX2
//

// What each pad holds on, as on the deck's screen (deck.slint's pad-fx and
// pad-fx2): a loaded_effect (1 Echo, 2 Reverb, 3 Filter, 4 Flanger,
// 5 Bitcrusher, 6 Phaser, 7 Gate, 8 Roll, 9 Echo Out, 10 Brake, 11 Noise,
// 12 Spiral) and where the meta knob sits.
PioneerXDJRX3.padFx = {
    // PAD FX 1
    0x50: [1, 0.6], 0x51: [2, 0.7], 0x52: [3, 0.2], 0x53: [3, 0.8],
    0x54: [4, 0.7], 0x55: [6, 0.7], 0x56: [5, 0.7], 0x57: [1, 1.0],
    // PAD FX 2
    0x18: [8, 0.5], 0x19: [7, 0.5], 0x1A: [9, 0.6], 0x1B: [10, 0.5],
    0x1C: [11, 0.7], 0x1D: [12, 0.7], 0x1E: [7, 0.9], 0x1F: [2, 1.0],
};

// Holding a pad puts its effect on the deck through the deck's pad FX unit
// (units 6 to 9 for decks 1 to 4); letting go turns it off.
PioneerXDJRX3.padFxPressed = function(_channel, control, value, status, group) {
    const fx = PioneerXDJRX3.padFx[control];
    if (fx === undefined) {
        return;
    }
    const deck = PioneerXDJRX3.deckIndex(group) + 1;
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

PioneerXDJRX3.samplerPadPressed = function(_channel, _control, value, status, mapped) {
    const group = PioneerXDJRX3.samplerSlot(mapped, status);
    if (engine.getValue(group, "track_loaded")) {
        engine.setValue(group, "cue_gotoandplay", value);
    } else {
        engine.setValue(group, "LoadSelectedTrack", value);
    }
};

PioneerXDJRX3.samplerPadShiftPressed = function(_channel, _control, value, status, mapped) {
    const group = PioneerXDJRX3.samplerSlot(mapped, status);
    if (engine.getValue(group, "play")) {
        engine.setValue(group, "cue_gotoandstop", value);
    } else if (engine.getValue(group, "track_loaded")) {
        engine.setValue(group, "eject", value);
    }
};

//
// Stems mode (KEYBOARD pressed again)
//
//

// DJ Sugar: the stems pads, laid out as on screen (VirtualDJ's): Vocal,
// Instru, Bass, Acapella on top; Kick, HiHat, Stems FX, Instrumental below.
// A number from 1 is the stem ([ChannelN_StemM]: 1 kick, 2 bass, 3
// instruments, 4 vocals, 5 hi-hats); 0 is Acapella, -1 Instrumental and -2
// Stems FX, which does nothing yet.
PioneerXDJRX3.stemPads = [4, 3, 2, 0, 1, 5, -2, -1];

PioneerXDJRX3.stemGroup = function(group, stem) {
    return `[${group.substring(1, group.length-1)}_Stem${stem}]`;
};

// Whether each of the deck's stems plays (index 1 to 5); a stem the track
// doesn't have counts as playing.
PioneerXDJRX3.stemsOn = function(group) {
    const count = engine.getValue(group, "stem_count");
    const on = [false];
    for (let stem = 1; stem <= 5; stem++) {
        on.push(stem > count || engine.getValue(PioneerXDJRX3.stemGroup(group, stem), "mute") <= 0.5);
    }
    return on;
};

// Whether the pad for `kind` (see stemPads) is lit.
PioneerXDJRX3.stemPadLit = function(group, kind) {
    const count = engine.getValue(group, "stem_count");
    if (count < 4) {
        return false;
    }
    const on = PioneerXDJRX3.stemsOn(group);
    const others = [1, 2, 3, 5].filter(function(s) { return s <= count; });
    if (kind > 0) {
        return kind <= count && on[kind];
    } else if (kind === 0) {
        return on[4] && others.every(function(s) { return !on[s]; });
    } else if (kind === -1) {
        return !on[4] && others.every(function(s) { return on[s]; });
    }
    return false;
};

// Mutes or unmutes the vocals and the rest.
PioneerXDJRX3.setStems = function(group, vocals, rest) {
    for (let stem = 1; stem <= 5; stem++) {
        const on = stem === 4 ? vocals : rest;
        engine.setValue(PioneerXDJRX3.stemGroup(group, stem), "mute", on ? 0 : 1);
    }
};

PioneerXDJRX3.stemMutePadPressed = function(_channel, control, value, _status, group) {
    if (value !== 0x7f) {
        return;
    }
    const kind = PioneerXDJRX3.stemPads[control - PioneerXDJRX3.stemMutePadsFirstControl];
    const count = engine.getValue(group, "stem_count");
    if (count < 4 || kind === undefined || kind > count) {
        return;
    }
    if (kind > 0) {
        const stemGroup = PioneerXDJRX3.stemGroup(group, kind);
        engine.setValue(stemGroup, "mute", engine.getValue(stemGroup, "mute") ? 0 : 1);
    } else if (kind === 0) {
        PioneerXDJRX3.setStems(group, true, PioneerXDJRX3.stemPadLit(group, 0));
    } else if (kind === -1) {
        PioneerXDJRX3.setStems(group, PioneerXDJRX3.stemPadLit(group, -1), true);
    }
};

// SHIFT + a stem's pad plays that stem alone.
PioneerXDJRX3.stemMutePadShiftPressed = function(_channel, control, value, _status, group) {
    if (value !== 0x7f) {
        return;
    }
    const kind = PioneerXDJRX3.stemPads[control - PioneerXDJRX3.stemMutePadsFirstControl];
    const count = engine.getValue(group, "stem_count");
    if (count < 4 || kind === undefined || kind <= 0 || kind > count) {
        PioneerXDJRX3.stemMutePadPressed(_channel, control, value, _status, group);
        return;
    }
    for (let stem = 1; stem <= count; stem++) {
        engine.setValue(PioneerXDJRX3.stemGroup(group, stem), "mute", stem === kind ? 0 : 1);
    }
};

PioneerXDJRX3.stemCountChanged = function(_value, group, _control) {
    const channel = group.match(/\[Channel\d+/)[0] + "]";
    if (!PioneerXDJRX3.keyboardStems[channel]) {
        return;
    }
    for (let pad = 0; pad < 8; pad++) {
        const lit = PioneerXDJRX3.stemPadLit(channel, PioneerXDJRX3.stemPads[pad]);
        PioneerXDJRX3.padLight(channel, PioneerXDJRX3.stemMutePadsFirstControl + pad, lit);
    }
};

PioneerXDJRX3.stemMuteChanged = PioneerXDJRX3.stemCountChanged;

//
// Pitch Shift mode
//

PioneerXDJRX3.pitchAdjusted = function(_value, group, _control) {
    // DJ Sugar: the KEYBOARD pads show the key shift too.
    PioneerXDJRX3.keyboardLights(group);
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

        PioneerXDJRX3.padLight(group, PioneerXDJRX3.pitchPadsFirstControl + i, code !== 0x00);
    }
};

PioneerXDJRX3.pitchPadPressed = function(_channel, control, value, _status, group) {
    if (value !== 0x7f) {
        return;
    }

    const pad = control - PioneerXDJRX3.pitchPadsFirstControl;
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

PioneerXDJRX3.pitchPadShiftPressed = function(_channel, control, value, _status, group) {
    if (value !== 0x7f) {
        return;
    }

    const pad = control - PioneerXDJRX3.pitchPadsFirstControl;

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

PioneerXDJRX3.shutdown = function() {
    engine.stopTimer(PioneerXDJRX3.jogDisplayTimer);

    PioneerXDJRX3.decks.forEach(function(group, i) {
        const status = PioneerXDJRX3.deckStatus(group);
        const pads = PioneerXDJRX3.padStatus(group);

        // The meter, the LOAD light and the jog display's playhead and cue.
        midi.sendShortMsg(0xB0 + i, 0x02, 0x00);
        midi.sendShortMsg(0x9B, i, 0x00);
        PioneerXDJRX3.sendJogDisplay(0x04 + i, 0x3FFF);
        PioneerXDJRX3.sendJogDisplay(i, 0);

        for (let pad = 0; pad <= 0x7F; ++pad) {
            midi.sendShortMsg(pads, pad, 0x00);
        }
        PioneerXDJRX3.setLoopButtonLights(status, 0x00);
        midi.sendShortMsg(status, 0x08, 0x00);
    });
    midi.sendShortMsg(0xB4, 0x72, 0x00);

    // The mixer mixes on its own again.
    midi.sendShortMsg(0xB4, 0x65, 0x7F);
};
