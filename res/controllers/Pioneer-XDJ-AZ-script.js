// Pioneer-XDJ-AZ-script.js
// ****************************************************************************
// * DJ Sugar mapping script for the Pioneer XDJ-AZ in PC (MIDI) control mode.
// * Written from AlphaTheta's "XDJ-AZ MIDI Message List", from DJ Sugar's
// * XDJ-RX3 script (itself from Mixxx's DDJ-400 and DDJ-FLX4 mappings:
// * Warker, nschloe, dj3730, jusko, Robert904).
// ****************************************************************************
//
//  The XDJ-AZ sends each deck on its own MIDI channel (decks 1 to 4 on
//  channels 1 to 4, DECK SELECT picks which), the mixer and BEAT FX on
//  channel 5, each deck's pads 1-4 on channels 6 to 9 and pads 5-8 on
//  channels 10 to 13, and the LOAD lights and jog displays on channel 14.
//  Its SHIFT button sends nothing: every control sends its own message with
//  SHIFT held.
//
//  Each pad sends its own note for each of the XDJ-AZ's pad modes (19 notes
//  a pad). The script routes them onto the notes of the XDJ-RX3 mapping's
//  pad pages (8 pads a page, 8 higher with SHIFT), pads 5-8 on the second
//  pad channel, so both mappings share their pad code.
//
//  The pad mode buttons open the deck's pad tabs as on the FLX4: HOT CUE,
//  BEAT LOOP and BEAT JUMP their tabs, SLIP LOOP the loop rolls, and
//  SHIFT + them (GATE CUE, BEAT LOOP2, KEY SHIFT, BEAT JUMP2) PAD FX,
//  SAMPLER, KEYBOARD and KEY SHIFT. The pads play whatever page is in the
//  tab.
//
//  The XDJ-AZ has no MIDI message to hand its mixer to the computer; its
//  mixer section sends MIDI, which DJ Sugar's mixer follows.
//
//  The jog displays show the playhead going round and the cue point.
//
//  Not mapped yet: CUE/LOOP MEMORY/DELETE, BEAT FX TIME, X-PAD, FX
//  QUANTIZE, FREQUENCY, PARAMETER, MASTER CUE, LINK CUE, master and mic EQ,
//  MIC, TALKOVER, INPUT SELECT, TRACK FILTER.

var PioneerXDJAZ = {};

PioneerXDJAZ.decks = ["[Channel1]", "[Channel2]", "[Channel3]", "[Channel4]"];

PioneerXDJAZ.deckIndex = function(group) {
    return parseInt(group.match(script.channelRegEx)[1]) - 1;
};

// The status of a deck's notes and its pads.
PioneerXDJAZ.deckStatus = function(group) {
    return 0x90 + PioneerXDJAZ.deckIndex(group);
};
PioneerXDJAZ.padStatus = function(group, note) {
    return 0x95 + PioneerXDJAZ.deckIndex(group) + ((note & 0x07) >= 4 ? 4 : 0);
};

// The deck a pad channel (5-8 or 9-12, from 0) belongs to.
PioneerXDJAZ.padDeck = function(status) {
    return PioneerXDJAZ.decks[((status & 0x0F) - 5) % 4];
};

// Pads light in a colour number (1-127); 0 dims them.
PioneerXDJAZ.padOn = 0x7F;

// Lights a pad note and the same pad with SHIFT (8 notes on).
PioneerXDJAZ.padLight = function(group, note, on) {
    const status = PioneerXDJAZ.padStatus(group, note);
    const value = on ? PioneerXDJAZ.padOn : 0x00;
    midi.sendShortMsg(status, note, value);
    midi.sendShortMsg(status, note + 8, value);
};

PioneerXDJAZ.timers = {};

// Jog wheel constants
PioneerXDJAZ.vinylMode = [true, true, true, true];
PioneerXDJAZ.alpha = 1.0/8;
PioneerXDJAZ.beta = PioneerXDJAZ.alpha/32;
PioneerXDJAZ.fastSeekScale = 75;

PioneerXDJAZ.tempoRanges = [0.06, 0.10, 0.16, 0.25];

PioneerXDJAZ.loopAdjustIn = [false, false, false, false];
PioneerXDJAZ.loopAdjustOut = [false, false, false, false];
PioneerXDJAZ.loopAdjustMultiply = 50;

// Beatjump pads, by the note they play (see panelNotes).
PioneerXDJAZ.beatjumpSizeForPad = {
    0x28: -4, 0x29: 4, 0x2A: -8, 0x2B: 8, 0x2C: -16, 0x2D: 16, 0x2E: -32, 0x2F: 32,
};

// The notes the KEYBOARD (and stems) and KEY SHIFT pages play.
PioneerXDJAZ.stemMutePadsFirstControl = 0x40;
PioneerXDJAZ.pitchPadsFirstControl = 0x60;

PioneerXDJAZ.quickJumpSize = 32;

PioneerXDJAZ.highResMSB = {
    "[Channel1]": {}, "[Channel2]": {}, "[Channel3]": {}, "[Channel4]": {},
};

//
// Init
//

PioneerXDJAZ.init = function() {
    PioneerXDJAZ.decks.forEach(function(group, i) {
        engine.softTakeover(group, "rate", true);

        engine.makeConnection(group, "sampler_page", PioneerXDJAZ.samplerPlayOutputCallbackFunction);
        engine.makeConnection(group, "loop_enabled", PioneerXDJAZ.loopToggle);
        PioneerXDJAZ.setLoopButtonLights(PioneerXDJAZ.deckStatus(group), 0x7F);
        midi.sendShortMsg(PioneerXDJAZ.deckStatus(group), 0x22, 0x7F);
        midi.sendShortMsg(PioneerXDJAZ.deckStatus(group), 0x12, 0x7F);

        for (let tab = 1; tab <= 8; ++tab) {
            engine.makeConnection(group, "pad_slot_" + tab, PioneerXDJAZ.padSlotChanged);
        }
        engine.makeConnection(group, "stem_count", PioneerXDJAZ.stemCountChanged);
        for (let stem = 1; stem <= 5; stem++) {
            engine.makeConnection(PioneerXDJAZ.stemGroup(group, stem), "mute", PioneerXDJAZ.stemMuteChanged);
        }
        engine.makeConnection(group, "track_loaded", PioneerXDJAZ.pitchAdjusted);
        engine.makeConnection(group, "pitch_adjust", PioneerXDJAZ.pitchAdjusted);
        engine.makeConnection(group, "track_loaded", PioneerXDJAZ.trackLoadedLED);
        engine.makeConnection(group, "sync_leader", PioneerXDJAZ.syncLeaderLight);
        engine.makeConnection("[QuickEffectRack1_" + group + "]", "loaded_chain_preset", PioneerXDJAZ.colorFxLights);
        PioneerXDJAZ.clearPadRoutes(group);
        PioneerXDJAZ.routePads(group);
    });
    engine.softTakeover(PioneerXDJAZ.beatFxSlot, "meta", true);

    const samplerCount = 16;
    if (engine.getValue("[App]", "num_samplers") < samplerCount) {
        engine.setValue("[App]", "num_samplers", samplerCount);
    }
    for (let i = 1; i <= samplerCount; ++i) {
        engine.makeConnection("[Sampler" + i + "]", "play", PioneerXDJAZ.samplerPlayOutputCallbackFunction);
        engine.makeConnection("[Sampler" + i + "]", "track_loaded", PioneerXDJAZ.samplerPlayOutputCallbackFunction);
    }
    PioneerXDJAZ.samplerPlayOutputCallbackFunction();

    engine.makeConnection(PioneerXDJAZ.beatFxSlot, "enabled", PioneerXDJAZ.beatFxLight);
    PioneerXDJAZ.colorFxLights();

    PioneerXDJAZ.jogDisplays = [[-1, -1], [-1, -1], [-1, -1], [-1, -1]];
    PioneerXDJAZ.jogDisplayTimer = engine.beginTimer(40, PioneerXDJAZ.jogDisplayUpdate);
};

//
// Lights
//

// The LOAD button blinks when a track loads.
PioneerXDJAZ.trackLoadedLED = function(value, group) {
    midi.sendShortMsg(0x9D, PioneerXDJAZ.deckIndex(group), value > 0 ? 0x7F : 0x00);
};

// A 14-bit value as two CCs on the jog display channel (14).
PioneerXDJAZ.sendJogDisplay = function(msb, value) {
    midi.sendShortMsg(0xBD, msb, (value >> 7) & 0x7F);
    midi.sendShortMsg(0xBD, msb + 0x20, value & 0x7F);
};

// The jog display's position bar (0 to 359 degrees) goes round once per
// turn of a record at 33 1/3 rpm; its cue marker shows where the cue point
// sits on that turn, hidden with no track.
PioneerXDJAZ.jogDisplayUpdate = function() {
    const degrees = function(seconds) {
        const turns = Math.max(0, seconds) * (100 / 3) / 60;
        return Math.floor((turns - Math.floor(turns)) * 360) % 360;
    };
    PioneerXDJAZ.decks.forEach(function(group, i) {
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
        const last = PioneerXDJAZ.jogDisplays[i];
        if (bar !== last[0]) {
            last[0] = bar;
            PioneerXDJAZ.sendJogDisplay(i, bar);
        }
        if (cue !== last[1]) {
            last[1] = cue;
            PioneerXDJAZ.sendJogDisplay(4 + i, cue);
        }
    });
};

//
// Browser
//

PioneerXDJAZ.waveformZoom = function(_channel, _control, value) {
    const up = value < 0x40;
    PioneerXDJAZ.decks.forEach(function(group) {
        script.triggerControl(group, up ? "waveform_zoom_up" : "waveform_zoom_down", 100);
    });
};

//
// Deck
//

PioneerXDJAZ.syncPressed = function(_channel, _control, value, _status, group) {
    if (value > 0) {
        script.toggleControl(group, "sync_enabled");
    }
};

PioneerXDJAZ.masterPressed = function(_channel, _control, value, _status, group) {
    if (value) {
        engine.setValue(group, "sync_leader", engine.getValue(group, "sync_leader") === 2 ? 0 : 2);
    }
};

PioneerXDJAZ.syncLeaderLight = function(value, group) {
    midi.sendShortMsg(PioneerXDJAZ.deckStatus(group), 0x1E, value > 0 ? 0x7F : 0x00);
};

PioneerXDJAZ.cycleTempoRange = function(_channel, _control, value, _status, group) {
    if (value === 0) { return; }
    const curr = engine.getValue(group, "rateRange");
    let idx = 0;
    for (let i = 0; i < PioneerXDJAZ.tempoRanges.length; i++) {
        if (curr === PioneerXDJAZ.tempoRanges[i]) {
            idx = (i + 1) % PioneerXDJAZ.tempoRanges.length;
            break;
        }
    }
    engine.setValue(group, "rateRange", PioneerXDJAZ.tempoRanges[idx]);
};

PioneerXDJAZ.toggleButton = function(key) {
    return function(_channel, _control, value, _status, group) {
        if (value) {
            script.toggleControl(group, key);
        }
    };
};
PioneerXDJAZ.togglePlay = PioneerXDJAZ.toggleButton("play");
PioneerXDJAZ.toggleKeylock = PioneerXDJAZ.toggleButton("keylock");
PioneerXDJAZ.toggleQuantize = PioneerXDJAZ.toggleButton("quantize");
PioneerXDJAZ.toggleSlip = PioneerXDJAZ.toggleButton("slip_enabled");
PioneerXDJAZ.toggleReverse = PioneerXDJAZ.toggleButton("reverse");

// JOG MODE: VINYL (scratch on touch) or CDJ (the top bends too).
PioneerXDJAZ.jogModePressed = function(channel, _control, value, status) {
    if (value) {
        PioneerXDJAZ.vinylMode[channel] = !PioneerXDJAZ.vinylMode[channel];
        midi.sendShortMsg(status, 0x12, PioneerXDJAZ.vinylMode[channel] ? 0x7F : 0x00);
    }
};

PioneerXDJAZ.quickJumpForward = function(_channel, _control, value, _status, group) {
    if (value) {
        engine.setValue(group, "beatjump", PioneerXDJAZ.quickJumpSize);
    }
};

PioneerXDJAZ.quickJumpBack = function(_channel, _control, value, _status, group) {
    if (value) {
        engine.setValue(group, "beatjump", -PioneerXDJAZ.quickJumpSize);
    }
};

// SEARCH ◀◀ / ▶▶: jump a beat every 100 ms while held.
PioneerXDJAZ.searchTimers = {};
PioneerXDJAZ.search = function(beats) {
    return function(_channel, _control, value, _status, group) {
        if (PioneerXDJAZ.searchTimers[group] !== undefined) {
            engine.stopTimer(PioneerXDJAZ.searchTimers[group]);
            PioneerXDJAZ.searchTimers[group] = undefined;
        }
        if (value) {
            engine.setValue(group, "beatjump", beats);
            PioneerXDJAZ.searchTimers[group] = engine.beginTimer(100, () => {
                engine.setValue(group, "beatjump", beats);
            });
        }
    };
};
PioneerXDJAZ.searchForward = PioneerXDJAZ.search(1);
PioneerXDJAZ.searchBack = PioneerXDJAZ.search(-1);

// BEAT JUMP ◀ / ▶ jump by the deck's beat jump size; with SHIFT they halve
// or double it.
PioneerXDJAZ.beatJump = function(direction) {
    return function(_channel, _control, value, _status, group) {
        if (value) {
            engine.setValue(group, "beatjump", direction * engine.getValue(group, "beatjump_size"));
        }
    };
};
PioneerXDJAZ.beatJumpForward = PioneerXDJAZ.beatJump(1);
PioneerXDJAZ.beatJumpBack = PioneerXDJAZ.beatJump(-1);
PioneerXDJAZ.beatJumpSize = function(factor) {
    return function(_channel, _control, value, _status, group) {
        if (value) {
            const size = engine.getValue(group, "beatjump_size") * factor;
            engine.setValue(group, "beatjump_size", Math.min(64, Math.max(0.03125, size)));
        }
    };
};
PioneerXDJAZ.beatJumpSizeUp = PioneerXDJAZ.beatJumpSize(2);
PioneerXDJAZ.beatJumpSizeDown = PioneerXDJAZ.beatJumpSize(0.5);

// CROSSFADER ASSIGN (one CC a channel, 0x41 to 0x44): A, THRU or B.
PioneerXDJAZ.crossfaderAssign = function(_channel, control, value) {
    const group = PioneerXDJAZ.decks[control - 0x41];
    engine.setValue(group, "orientation", value < 0x20 ? 0 : (value > 0x60 ? 2 : 1));
};

// NEEDLE POSITION: touching the strip on the waveform jumps there.
PioneerXDJAZ.needleSearch = function(_channel, _control, value, _status, group) {
    if (value > 0 && !engine.getValue(group, "play")) {
        engine.setValue(group, "playposition", (value - 1) / 126);
    }
};

PioneerXDJAZ.tempoSliderMSB = function(_channel, _control, value, _status, group) {
    PioneerXDJAZ.highResMSB[group].tempoSlider = value;
};

// "-" side 0, "+" side 16383.
PioneerXDJAZ.tempoSliderLSB = function(_channel, _control, value, _status, group) {
    const fullValue = (PioneerXDJAZ.highResMSB[group].tempoSlider << 7) + value;
    engine.setValue(group, "rate", 1 - (fullValue / 0x2000));
};

//
// Jog wheels
//

PioneerXDJAZ.jogTurn = function(channel, _control, value, _status, group) {
    const deckNum = channel + 1;
    let newVal = value - 64;

    if (engine.getValue(group, "loop_enabled") > 0) {
        if (PioneerXDJAZ.loopAdjustIn[channel]) {
            engine.setValue(group, "loop_start_position",
                newVal * PioneerXDJAZ.loopAdjustMultiply + engine.getValue(group, "loop_start_position"));
            return;
        }
        if (PioneerXDJAZ.loopAdjustOut[channel]) {
            engine.setValue(group, "loop_end_position",
                newVal * PioneerXDJAZ.loopAdjustMultiply + engine.getValue(group, "loop_end_position"));
            return;
        }
    }

    if (engine.isScratching(deckNum)) {
        engine.scratchTick(deckNum, newVal);
    } else {
        engine.setValue(group, "jog_bend", newVal);
    }
};

PioneerXDJAZ.jogSearch = function(_channel, _control, value, _status, group) {
    engine.setValue(group, "jog", (value - 64) * PioneerXDJAZ.fastSeekScale);
};

PioneerXDJAZ.jogTouch = function(channel, _control, value) {
    const deckNum = channel + 1;
    if (PioneerXDJAZ.loopAdjustIn[channel] || PioneerXDJAZ.loopAdjustOut[channel]) {
        return;
    }
    if (value !== 0 && PioneerXDJAZ.vinylMode[channel]) {
        engine.scratchEnable(deckNum, 720, 33+1/3, PioneerXDJAZ.alpha, PioneerXDJAZ.beta);
    } else {
        engine.scratchDisable(deckNum);
    }
};

//
// Loops: IN (long press: adjust), OUT, RELOOP/EXIT
//

PioneerXDJAZ.toggleLoopAdjustIn = function(channel, _control, value, _status, group) {
    if (value === 0 || !engine.getValue(group, "loop_enabled")) {
        return;
    }
    PioneerXDJAZ.loopAdjustIn[channel] = !PioneerXDJAZ.loopAdjustIn[channel];
    PioneerXDJAZ.loopAdjustOut[channel] = false;
};

PioneerXDJAZ.setLoopButtonLights = function(status, value) {
    midi.sendShortMsg(status, 0x06, value);
    midi.sendShortMsg(status, 0x07, value);
};

PioneerXDJAZ.loopToggle = function(value, group, control) {
    const status = PioneerXDJAZ.deckStatus(group),
        channel = PioneerXDJAZ.deckIndex(group);
    midi.sendShortMsg(status, 0x08, value ? 0x7F : 0x00);
    PioneerXDJAZ.timers[group] = PioneerXDJAZ.timers[group] || {};
    if (PioneerXDJAZ.timers[group][control] !== undefined) {
        engine.stopTimer(PioneerXDJAZ.timers[group][control]);
        PioneerXDJAZ.timers[group][control] = undefined;
    }
    if (value) {
        let blink = 0x7F;
        PioneerXDJAZ.timers[group][control] = engine.beginTimer(500, () => {
            blink = 0x7F - blink;
            midi.sendShortMsg(status, 0x06, PioneerXDJAZ.loopAdjustOut[channel] ? 0x00 : blink);
            midi.sendShortMsg(status, 0x07, PioneerXDJAZ.loopAdjustIn[channel] ? 0x00 : blink);
        });
    } else {
        PioneerXDJAZ.setLoopButtonLights(status, 0x7F);
        PioneerXDJAZ.loopAdjustIn[channel] = false;
        PioneerXDJAZ.loopAdjustOut[channel] = false;
    }
};

PioneerXDJAZ.cueLoopCallLeft = function(_channel, _control, value, _status, group) {
    engine.setValue(group, "loop_halve", value ? 1 : 0);
};

PioneerXDJAZ.cueLoopCallRight = function(_channel, _control, value, _status, group) {
    engine.setValue(group, "loop_double", value ? 1 : 0);
};

//
// Mixer
//

PioneerXDJAZ.pflPressed = function(_channel, control, value) {
    if (value) {
        script.toggleControl(PioneerXDJAZ.decks[control - 0x46], "pfl");
    }
};

// COLOR FX buttons: each picks the Color FX the COLOR knobs play; pressing
// the lit one goes back to the filter. Numbers as in the engine's COLOR_FX
// (1 Filter, 2 Space, 3 Dub Echo, 4 Noise, 5 Crush); SWEEP plays the filter.
PioneerXDJAZ.colorFxButtons = {0x69: 2, 0x6B: 3, 0x6A: 1, 0x55: 4, 0x56: 5, 0x57: 1};

PioneerXDJAZ.colorFxPressed = function(_channel, control, value) {
    if (!value) { return; }
    const preset = PioneerXDJAZ.colorFxButtons[control];
    const current = engine.getValue("[QuickEffectRack1_[Channel1]]", "loaded_chain_preset");
    const next = current === preset && control !== 0x57 ? 1 : preset;
    PioneerXDJAZ.decks.forEach(function(group) {
        engine.setValue("[QuickEffectRack1_" + group + "]", "loaded_chain_preset", next);
    });
};

PioneerXDJAZ.colorFxLights = function() {
    const current = engine.getValue("[QuickEffectRack1_[Channel1]]", "loaded_chain_preset");
    Object.keys(PioneerXDJAZ.colorFxButtons).forEach(function(control) {
        const preset = PioneerXDJAZ.colorFxButtons[control];
        const lit = preset === current && !(preset === 1 && parseInt(control) === 0x6A);
        midi.sendShortMsg(0xB4, parseInt(control), lit ? 0x7F : 0x00);
    });
};

//
// BEAT FX: DJ Sugar's Beat FX unit
//

PioneerXDJAZ.beatFxUnit = "[EffectRack1_EffectUnit5]";
PioneerXDJAZ.beatFxSlot = "[EffectRack1_EffectUnit5_Effect1]";

// EFFECT SELECT, by the CC each position sends, to the engine's effects
// (loaded_effect: 1 Echo, 2 Reverb, 3 Filter, 4 Flanger, 6 Phaser, 7 Gate,
// 8 Roll, 12 Spiral).
PioneerXDJAZ.beatFxEffects = {
    0x2A: 1, // DELAY
    0x37: 1, // ECHO
    0x33: 1, // PING PONG
    0x2B: 12, // SPIRAL
    0x3E: 12, // HELIX
    0x36: 2, // REVERB
    0x32: 4, // FLANGER
    0x39: 6, // PHASER
    0x3B: 3, // FILTER
    0x3D: 3, // TRIPLET FILTER
    0x35: 7, // TRANS
    0x2E: 8, // ROLL
    0x3F: 8, // TRIPLET ROLL
    0x3A: 6, // MOBIUS
};

PioneerXDJAZ.beatFxSelect = function(_channel, control, value) {
    if (value === 0x7F) {
        engine.setValue(PioneerXDJAZ.beatFxSlot, "loaded_effect", PioneerXDJAZ.beatFxEffects[control]);
    }
};

PioneerXDJAZ.beatFxLevelDepthRotate = function(_channel, _control, value) {
    engine.setParameter(PioneerXDJAZ.beatFxSlot, "meta", value / 0x7F);
};

PioneerXDJAZ.changeBeatFxBeats = function(direction) {
    const beats = engine.getValue(PioneerXDJAZ.beatFxSlot, "beats");
    if (beats <= 0) { return; }
    const next = Math.min(4, Math.max(1 / 16, direction > 0 ? beats * 2 : beats / 2));
    engine.setValue(PioneerXDJAZ.beatFxSlot, "beats", next);
};

PioneerXDJAZ.pushBeatFxUnit = function(key) {
    engine.setValue(PioneerXDJAZ.beatFxUnit, key, 1);
    engine.setValue(PioneerXDJAZ.beatFxUnit, key, 0);
};

PioneerXDJAZ.beatFxLeftPressed = function(_channel, _control, value) {
    if (value) { PioneerXDJAZ.changeBeatFxBeats(-1); }
};

PioneerXDJAZ.beatFxRightPressed = function(_channel, _control, value) {
    if (value) { PioneerXDJAZ.changeBeatFxBeats(1); }
};

// TAP taps the tempo; SHIFT + TAP goes back to the track's.
PioneerXDJAZ.beatFxTapPressed = function(_channel, control, value) {
    if (value) { PioneerXDJAZ.pushBeatFxUnit(control === 0x45 ? "bpm_auto" : "bpm_tap"); }
};

PioneerXDJAZ.beatFxOnOffPressed = function(_channel, _control, value) {
    if (value) {
        engine.setValue(PioneerXDJAZ.beatFxSlot, "enabled", !engine.getValue(PioneerXDJAZ.beatFxSlot, "enabled"));
    }
};

PioneerXDJAZ.beatFxOnOffShiftPressed = function(_channel, _control, value) {
    if (value) { PioneerXDJAZ.pushBeatFxUnit("release"); }
};

PioneerXDJAZ.beatFxLight = function(value) {
    midi.sendShortMsg(0xB4, 0x72, value ? 0x7F : 0x00);
};

// CH SELECT, by the CC each position sends, to the decks it sends the Beat
// FX: CH1 to CH4 that deck, MASTER all four, MIC none; CF.A and CF.B the
// decks the crossfader assign puts on that side.
PioneerXDJAZ.beatFxChannels = {
    0x23: [1], 0x24: [2], 0x22: [3], 0x25: [4], 0x29: [1, 2, 3, 4], 0x1C: [],
};

PioneerXDJAZ.beatFxChannelSelect = function(_channel, control, value) {
    if (value !== 0x7F) { return; }
    let decks = PioneerXDJAZ.beatFxChannels[control];
    if (control === 0x27 || control === 0x28) {
        const side = control === 0x27 ? 0 : 2;
        decks = [1, 2, 3, 4].filter(function(deck) {
            return engine.getValue("[Channel" + deck + "]", "orientation") === side;
        });
    }
    PioneerXDJAZ.decks.forEach(function(group, i) {
        engine.setValue(PioneerXDJAZ.beatFxUnit, "group_" + group + "_enable", decks.indexOf(i + 1) >= 0 ? 1 : 0);
    });
};

//
// Pads
//

// The pad mode buttons (deck channel notes) and the pages they open: a pad
// tab of the deck (0-7, see the deck's pad_tab), or for SLIP LOOP the loop
// rolls. The pads send notes 0x00, 0x10 ... 0x70 on each page, 8 higher
// with SHIFT.
PioneerXDJAZ.padModes = {
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
PioneerXDJAZ.panelNotes = {
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

PioneerXDJAZ.padMode = {"[Channel1]": 0x22, "[Channel2]": 0x22, "[Channel3]": 0x22, "[Channel4]": 0x22};
PioneerXDJAZ.keyboardStems = {"[Channel1]": false, "[Channel2]": false, "[Channel3]": false, "[Channel4]": false};

PioneerXDJAZ.modePanel = function(group, mode) {
    return mode.tab === undefined ? mode.panel : engine.getValue(group, "pad_slot_" + (mode.tab + 1));
};

// The XDJ-AZ's pad notes: pad N (1-4, and 5-8 on the second pad channel)
// sends 19 * ((N - 1) % 4) plus its mode's number, and one more with
// SHIFT. By page (as in padModes): HOT CUE, BEAT LOOP, SLIP LOOP, BEAT
// JUMP, GATE CUE, BEAT LOOP2, KEY SHIFT, BEAT JUMP2. SLIP LOOP also sends
// 7 (the PAD FX bank 2 page 2 note); PAD FX (4, 5) has no page here.
PioneerXDJAZ.padsPerChannel = 19;
PioneerXDJAZ.pageNotes = [[0, 1], [2, 3], [6, 8], [9, 10], [11, 12], [13, 14], [15, 16], [17, 18]];
PioneerXDJAZ.unusedNote = 0x7F;

// Points every note of a deck's pad channels at nothing first, so a note
// with no page does nothing.
PioneerXDJAZ.clearPadRoutes = function(group) {
    [0, 4].forEach(function(pad) {
        const channel = PioneerXDJAZ.padStatus(group, pad) & 0x0F;
        for (let note = 0; note < 0x7F; ++note) {
            midi.routeNote(channel, note, PioneerXDJAZ.unusedNote);
        }
    });
};

PioneerXDJAZ.routePads = function(group) {
    Object.keys(PioneerXDJAZ.padModes).forEach(function(button) {
        const mode = PioneerXDJAZ.padModes[button];
        const notes = PioneerXDJAZ.panelNotes[PioneerXDJAZ.modePanel(group, mode)];
        const sent = PioneerXDJAZ.pageNotes[mode.page];
        for (let pad = 0; pad < 8; ++pad) {
            const channel = PioneerXDJAZ.padStatus(group, pad) & 0x0F;
            const first = PioneerXDJAZ.padsPerChannel * (pad % 4);
            const plain = notes === undefined ? PioneerXDJAZ.unusedNote : notes[0] + pad;
            const shifted = notes === undefined ? PioneerXDJAZ.unusedNote : notes[1] + pad;
            midi.routeNote(channel, first + sent[0], plain);
            midi.routeNote(channel, first + sent[1], shifted);
            if (mode.page === 2) {
                midi.routeNote(channel, first + 7, plain);
            }
        }
    });
    const stems = PioneerXDJAZ.modePanel(group, PioneerXDJAZ.padModes[PioneerXDJAZ.padMode[group]]) === 4;
    if (stems !== PioneerXDJAZ.keyboardStems[group]) {
        PioneerXDJAZ.keyboardStems[group] = stems;
        PioneerXDJAZ.keyboardLights(group);
    }
};

PioneerXDJAZ.padSlotChanged = function(_value, group) {
    PioneerXDJAZ.routePads(group);
};

PioneerXDJAZ.padModeKeyPressed = function(channel, control, value) {
    if (!value) { return; }
    const group = PioneerXDJAZ.decks[channel];
    const mode = PioneerXDJAZ.padModes[control];
    const again = PioneerXDJAZ.padMode[group] === control;
    PioneerXDJAZ.padMode[group] = control;
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
    PioneerXDJAZ.routePads(group);
    // Only the chosen mode stays lit (the EXTENSION pages light their
    // button's plain note).
    for (let n = 0x22; n <= 0x25; ++n) {
        midi.sendShortMsg(0x90 + channel, n, n === (control - 0x22) % 4 + 0x22 ? 0x7F : 0x00);
    }
};

//
// Sampler pads: each deck's pads play the 8 slots of its sampler page.
//

PioneerXDJAZ.samplerSlot = function(group, status) {
    const n = parseInt(group.match(script.samplerRegEx)[1]) - 1;
    const pad = n % 4 + (n >= 8 ? 4 : 0);
    const deck = PioneerXDJAZ.padDeck(status);
    const page = engine.getValue(deck, "sampler_page");
    return "[Sampler" + (page * 8 + pad + 1) + "]";
};

PioneerXDJAZ.samplerLights = function(deck) {
    const page = engine.getValue(deck, "sampler_page");
    for (let pad = 0; pad < 8; ++pad) {
        const group = "[Sampler" + (page * 8 + pad + 1) + "]";
        PioneerXDJAZ.stopSamplerBlink(deck, 0x30 + pad);
        if (engine.getValue(group, "play") === 1) {
            PioneerXDJAZ.startSamplerBlink(deck, 0x30 + pad, group);
        } else {
            PioneerXDJAZ.padLight(deck, 0x30 + pad, engine.getValue(group, "track_loaded"));
        }
    }
};

PioneerXDJAZ.samplerPlayOutputCallbackFunction = function() {
    PioneerXDJAZ.decks.forEach(PioneerXDJAZ.samplerLights);
};

PioneerXDJAZ.startSamplerBlink = function(deck, control, group) {
    let on = true;
    PioneerXDJAZ.stopSamplerBlink(deck, control);
    PioneerXDJAZ.timers[deck + "sampler"][control] = engine.beginTimer(250, () => {
        on = !on;
        PioneerXDJAZ.padLight(deck, control, on);
        if (engine.getValue(group, "play") !== 1) {
            PioneerXDJAZ.stopSamplerBlink(deck, control);
            PioneerXDJAZ.padLight(deck, control, true);
        }
    });
};

PioneerXDJAZ.stopSamplerBlink = function(deck, control) {
    const timers = PioneerXDJAZ.timers[deck + "sampler"] = PioneerXDJAZ.timers[deck + "sampler"] || {};
    if (timers[control] !== undefined) {
        engine.stopTimer(timers[control]);
        timers[control] = undefined;
    }
};


//
// Beat jump pads: back on the left, forward on the right
//

PioneerXDJAZ.beatjumpPadPressed = function(_channel, control, value, _status, group) {
    if (value === 0) {
        return;
    }
    engine.setValue(group, "beatjump_size", Math.abs(PioneerXDJAZ.beatjumpSizeForPad[control]));
    engine.setValue(group, "beatjump", PioneerXDJAZ.beatjumpSizeForPad[control]);
};

//
// Keyboard
//

PioneerXDJAZ.keyboardSemitones = [0, 1, 2, 3, -4, -3, -2, -1];

PioneerXDJAZ.keyboardPadPressed = function(channel, control, value, status, group) {
    if (PioneerXDJAZ.keyboardStems[group]) {
        PioneerXDJAZ.stemMutePadPressed(channel, control, value, status, group);
        return;
    }
    const pad = control - PioneerXDJAZ.stemMutePadsFirstControl + 1;
    engine.setValue(group, "keyboard_" + pad + "_activate", value === 0x7F ? 1 : 0);
};

PioneerXDJAZ.keyboardPadShiftPressed = function(channel, control, value, status, group) {
    if (PioneerXDJAZ.keyboardStems[group]) {
        PioneerXDJAZ.stemMutePadShiftPressed(channel, control, value, status, group);
        return;
    }
    // SHIFT + pad plays its pitch too.
    PioneerXDJAZ.keyboardPadPressed(channel, control, value, status, group);
};

// Lights the KEYBOARD pads: the stems in stems mode, else the pad of the key
// shift playing.
PioneerXDJAZ.keyboardLights = function(group) {
    if (PioneerXDJAZ.keyboardStems[group]) {
        PioneerXDJAZ.stemCountChanged(0, group, "stem_count");
        return;
    }
    const pitch = engine.getValue(group, "pitch_adjust");
    for (let i = 0; i < 8; i++) {
        const lit = Math.abs(PioneerXDJAZ.keyboardSemitones[i] - pitch) < 0.01;
        PioneerXDJAZ.padLight(group, PioneerXDJAZ.stemMutePadsFirstControl + i, lit);
    }
};

//
// DJ Sugar: PAD FX1 and PAD FX2
//

// What each pad holds on, as on the deck's screen (deck.slint's pad-fx and
// pad-fx2): a loaded_effect (1 Echo, 2 Reverb, 3 Filter, 4 Flanger,
// 5 Bitcrusher, 6 Phaser, 7 Gate, 8 Roll, 9 Echo Out, 10 Brake, 11 Noise,
// 12 Spiral) and where the meta knob sits.
PioneerXDJAZ.padFx = {
    // PAD FX 1
    0x50: [1, 0.6], 0x51: [2, 0.7], 0x52: [3, 0.2], 0x53: [3, 0.8],
    0x54: [4, 0.7], 0x55: [6, 0.7], 0x56: [5, 0.7], 0x57: [1, 1.0],
    // PAD FX 2
    0x18: [8, 0.5], 0x19: [7, 0.5], 0x1A: [9, 0.6], 0x1B: [10, 0.5],
    0x1C: [11, 0.7], 0x1D: [12, 0.7], 0x1E: [7, 0.9], 0x1F: [2, 1.0],
};

// Holding a pad puts its effect on the deck through the deck's pad FX unit
// (units 6 to 9 for decks 1 to 4); letting go turns it off.
PioneerXDJAZ.padFxPressed = function(_channel, control, value, status, group) {
    const fx = PioneerXDJAZ.padFx[control];
    if (fx === undefined) {
        return;
    }
    const deck = PioneerXDJAZ.deckIndex(group) + 1;
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

PioneerXDJAZ.samplerPadPressed = function(_channel, _control, value, status, mapped) {
    const group = PioneerXDJAZ.samplerSlot(mapped, status);
    if (engine.getValue(group, "track_loaded")) {
        engine.setValue(group, "cue_gotoandplay", value);
    } else {
        engine.setValue(group, "LoadSelectedTrack", value);
    }
};

PioneerXDJAZ.samplerPadShiftPressed = function(_channel, _control, value, status, mapped) {
    const group = PioneerXDJAZ.samplerSlot(mapped, status);
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
PioneerXDJAZ.stemPads = [4, 3, 2, 0, 1, 5, -2, -1];

PioneerXDJAZ.stemGroup = function(group, stem) {
    return `[${group.substring(1, group.length-1)}_Stem${stem}]`;
};

// Whether each of the deck's stems plays (index 1 to 5); a stem the track
// doesn't have counts as playing.
PioneerXDJAZ.stemsOn = function(group) {
    const count = engine.getValue(group, "stem_count");
    const on = [false];
    for (let stem = 1; stem <= 5; stem++) {
        on.push(stem > count || engine.getValue(PioneerXDJAZ.stemGroup(group, stem), "mute") <= 0.5);
    }
    return on;
};

// Whether the pad for `kind` (see stemPads) is lit.
PioneerXDJAZ.stemPadLit = function(group, kind) {
    const count = engine.getValue(group, "stem_count");
    if (count < 4) {
        return false;
    }
    const on = PioneerXDJAZ.stemsOn(group);
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
PioneerXDJAZ.setStems = function(group, vocals, rest) {
    for (let stem = 1; stem <= 5; stem++) {
        const on = stem === 4 ? vocals : rest;
        engine.setValue(PioneerXDJAZ.stemGroup(group, stem), "mute", on ? 0 : 1);
    }
};

PioneerXDJAZ.stemMutePadPressed = function(_channel, control, value, _status, group) {
    if (value !== 0x7f) {
        return;
    }
    const kind = PioneerXDJAZ.stemPads[control - PioneerXDJAZ.stemMutePadsFirstControl];
    const count = engine.getValue(group, "stem_count");
    if (count < 4 || kind === undefined || kind > count) {
        return;
    }
    if (kind > 0) {
        const stemGroup = PioneerXDJAZ.stemGroup(group, kind);
        engine.setValue(stemGroup, "mute", engine.getValue(stemGroup, "mute") ? 0 : 1);
    } else if (kind === 0) {
        PioneerXDJAZ.setStems(group, true, PioneerXDJAZ.stemPadLit(group, 0));
    } else if (kind === -1) {
        PioneerXDJAZ.setStems(group, PioneerXDJAZ.stemPadLit(group, -1), true);
    }
};

// SHIFT + a stem's pad plays that stem alone.
PioneerXDJAZ.stemMutePadShiftPressed = function(_channel, control, value, _status, group) {
    if (value !== 0x7f) {
        return;
    }
    const kind = PioneerXDJAZ.stemPads[control - PioneerXDJAZ.stemMutePadsFirstControl];
    const count = engine.getValue(group, "stem_count");
    if (count < 4 || kind === undefined || kind <= 0 || kind > count) {
        PioneerXDJAZ.stemMutePadPressed(_channel, control, value, _status, group);
        return;
    }
    for (let stem = 1; stem <= count; stem++) {
        engine.setValue(PioneerXDJAZ.stemGroup(group, stem), "mute", stem === kind ? 0 : 1);
    }
};

PioneerXDJAZ.stemCountChanged = function(_value, group, _control) {
    const channel = group.match(/\[Channel\d+/)[0] + "]";
    if (!PioneerXDJAZ.keyboardStems[channel]) {
        return;
    }
    for (let pad = 0; pad < 8; pad++) {
        const lit = PioneerXDJAZ.stemPadLit(channel, PioneerXDJAZ.stemPads[pad]);
        PioneerXDJAZ.padLight(channel, PioneerXDJAZ.stemMutePadsFirstControl + pad, lit);
    }
};

PioneerXDJAZ.stemMuteChanged = PioneerXDJAZ.stemCountChanged;

//
// Pitch Shift mode
//

PioneerXDJAZ.pitchAdjusted = function(_value, group, _control) {
    // DJ Sugar: the KEYBOARD pads show the key shift too.
    PioneerXDJAZ.keyboardLights(group);
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

        PioneerXDJAZ.padLight(group, PioneerXDJAZ.pitchPadsFirstControl + i, code !== 0x00);
    }
};

PioneerXDJAZ.pitchPadPressed = function(_channel, control, value, _status, group) {
    if (value !== 0x7f) {
        return;
    }

    const pad = control - PioneerXDJAZ.pitchPadsFirstControl;
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

PioneerXDJAZ.pitchPadShiftPressed = function(_channel, control, value, _status, group) {
    if (value !== 0x7f) {
        return;
    }

    const pad = control - PioneerXDJAZ.pitchPadsFirstControl;

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

PioneerXDJAZ.shutdown = function() {
    engine.stopTimer(PioneerXDJAZ.jogDisplayTimer);

    PioneerXDJAZ.decks.forEach(function(group, i) {
        const status = PioneerXDJAZ.deckStatus(group);

        // The LOAD light and the jog display's playhead and cue.
        midi.sendShortMsg(0x9D, i, 0x00);
        PioneerXDJAZ.sendJogDisplay(0x04 + i, 0x3FFF);
        PioneerXDJAZ.sendJogDisplay(i, 0);

        [0, 4].forEach(function(pad) {
            const pads = PioneerXDJAZ.padStatus(group, pad);
            for (let note = 0; note <= 0x7F; ++note) {
                midi.sendShortMsg(pads, note, 0x00);
            }
        });
        PioneerXDJAZ.setLoopButtonLights(status, 0x00);
        midi.sendShortMsg(status, 0x08, 0x00);
    });
    midi.sendShortMsg(0xB4, 0x72, 0x00);
};
