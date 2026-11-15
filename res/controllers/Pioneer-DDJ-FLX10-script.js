// Pioneer-DDJ-FLX10-script.js
// ****************************************************************************
// * DJ Sugar mapping script for the Pioneer DDJ-FLX10.
// * Built on DJ Sugar's DDJ-FLX6 script (itself from its DDJ-FLX4 script and
// * Mixxx's DDJ-400 and DDJ-FLX4 mappings: Warker, nschloe, dj3730, jusko,
// * Robert904), written from AlphaTheta's "DDJ-FLX10 MIDI message list".
// ****************************************************************************
//
//  The FLX10 sends each deck on its own MIDI channel: decks 1 to 4 on
//  channels 1 to 4, their pads on channels 8/9, 10/11, 12/13 and 14/15
//  (the second of each pair with SHIFT held), as the FLX6 does. The DECK
//  buttons pick which deck each side plays; the mixer always has all four
//  channels.
//
//  As on the FLX4 (see its script), the pad mode buttons open the deck's
//  pad tabs and the pads play whatever page is in the tab.
//
//  BEAT FX drives DJ Sugar's one Beat FX unit: BEAT FX SELECT picks the
//  nearest DJ Sugar effect, CH SELECT sends it to one deck or, on MASTER, all
//  four, LEVEL/DEPTH is its depth, ON/OFF turns it on and off (SHIFT:
//  release) and BEAT < > halve and double its length (SHIFT: AUTO and TAP
//  tempo). The SOUND COLOR FX buttons pick what the COLOR knobs play.
//
//  ACTIVE PART mutes and unmutes a deck's drums, vocals or instruments
//  stems.
//
//  The jog displays show the playhead going round, the cue point, BPM and
//  time, and light MASTER, SYNC and MASTER TEMPO.
//
//  Not mapped yet: MIX POINT, CUE/LOOP MEMORY, MASTER CUE, INPUT SELECT, the
//  mic, SAMPLER CUE and VOL, FX PART SELECT, the PITCH colour FX, the pad
//  PAGE buttons, the fader start messages and the jog's playing speed.

var PioneerDDJFLX10 = {};

// The four decks: "[ChannelN]" from 0 to 3.
PioneerDDJFLX10.decks = ["[Channel1]", "[Channel2]", "[Channel3]", "[Channel4]"];

// A deck's index (0 to 3) from its group.
PioneerDDJFLX10.deckIndex = function(group) {
    return parseInt(group.match(script.channelRegEx)[1]) - 1;
};

// The status of a deck's notes and its pads (without SHIFT; +1 with it).
PioneerDDJFLX10.deckStatus = function(group) {
    return 0x90 + PioneerDDJFLX10.deckIndex(group);
};
PioneerDDJFLX10.padStatus = function(group) {
    return 0x97 + 2 * PioneerDDJFLX10.deckIndex(group);
};

// The deck playing on each side (left decks 1 and 3, right 2 and 4), as
// the DECK buttons last set it.
PioneerDDJFLX10.sideDeck = [0, 1];

PioneerDDJFLX10.noteSide = function(deck) {
    PioneerDDJFLX10.sideDeck[deck % 2] = deck;
};

// The pad mode buttons' lights, the same notes on every deck.
PioneerDDJFLX10.padModeLights = {
    hotcueMode: 0x1B,
    keyboardMode: 0x69,
    padFX1Mode: 0x1E,
    padFX2Mode: 0x6F,   // SHIFT + PAD FX lights the note SHIFT + SAMPLER sends
    beatJumpMode: 0x20,
    beatLoopMode: 0x6D,
    samplerMode: 0x22,
    keyShiftMode: 0x6B, // and the other way round
};

// Store timer IDs
PioneerDDJFLX10.timers = {};

// Jog wheel constants
// DJ Sugar: whether touching the top scratches follows each deck's VINYL
// switch ([ChannelN] vinyl_mode, on by default) instead of this constant.
PioneerDDJFLX10.vinylMode = function(deckNum) {
    return engine.getValue("[Channel" + deckNum + "]", "vinyl_mode") > 0;
};
PioneerDDJFLX10.alpha = 1.0/8;
PioneerDDJFLX10.beta = PioneerDDJFLX10.alpha/32;

// Multiplier for fast seek through track using SHIFT+JOGWHEEL
// DJ Sugar: half of Mixxx's 150, which searched too fast.
PioneerDDJFLX10.fastSeekScale = 75;

PioneerDDJFLX10.tempoRanges = [0.06, 0.10, 0.16, 0.25];

PioneerDDJFLX10.shiftButtonDown = [false, false, false, false];

// Jog wheel loop adjust
PioneerDDJFLX10.loopAdjustIn = [false, false, false, false];
PioneerDDJFLX10.loopAdjustOut = [false, false, false, false];
PioneerDDJFLX10.loopAdjustMultiply = 50;

// Beatjump pad (beatjump_size values)
// DJ Sugar: 4, 8, 16 and 32 beats back and forward, as on the deck's screen.
PioneerDDJFLX10.beatjumpSizeForPad = {
    0x20: -4,  // PAD 1
    0x21: 4,   // PAD 2
    0x22: -8,  // PAD 3
    0x23: 8,   // PAD 4
    0x24: -16, // PAD 5
    0x25: 16,  // PAD 6
    0x26: -32, // PAD 7
    0x27: 32   // PAD 8
};

// Stems (KEYBOARD) pads mode status for each deck, without or with SHIFT pressed
PioneerDDJFLX10.stemsPadsModesStatus = {
    "[Channel1]": [0x97, 0x98],
    "[Channel2]": [0x99, 0x9a],
    "[Channel3]": [0x9b, 0x9c],
    "[Channel4]": [0x9d, 0x9e],
};

// Stems (KEYBOARD) pad 1 control (pad control = [this value] + [pad  number] - 1)
PioneerDDJFLX10.stemMutePadsFirstControl = 0x40;

// Pitch shift (KEY SHIFT) pads mode status for each deck, without or with SHIFT pressed
PioneerDDJFLX10.pitchPadsModesStatus = PioneerDDJFLX10.stemsPadsModesStatus;

// Pitch shift (KEY SHIFT) pad 1 control (pad control = [this value] + [pad  number] - 1)
PioneerDDJFLX10.pitchPadsFirstControl = 0x70;

PioneerDDJFLX10.quickJumpSize = 32;

// Used for tempo slider
PioneerDDJFLX10.highResMSB = {
    "[Channel1]": {},
    "[Channel2]": {},
    "[Channel3]": {},
    "[Channel4]": {},
};

PioneerDDJFLX10.toggleLight = function(midiIn, active) {
    midi.sendShortMsg(midiIn.status, midiIn.data1, active ? 0x7F : 0);
};

//
// Init
//

PioneerDDJFLX10.init = function() {
    const decks = PioneerDDJFLX10.decks;
    decks.forEach(function(group) {
        engine.makeConnection(group, "vu_meter", PioneerDDJFLX10.vuMeterUpdate);
        midi.sendShortMsg(0xB0 + PioneerDDJFLX10.deckIndex(group), 0x02, 0);
        engine.softTakeover(group, "rate", true);
    });
    engine.softTakeover(PioneerDDJFLX10.beatFxSlot, "meta", true);

    const samplerCount = 16;
    if (engine.getValue("[App]", "num_samplers") < samplerCount) {
        engine.setValue("[App]", "num_samplers", samplerCount);
    }
    for (let i = 1; i <= samplerCount; ++i) {
        engine.makeConnection("[Sampler" + i + "]", "play", PioneerDDJFLX10.samplerPlayOutputCallbackFunction);
        engine.makeConnection("[Sampler" + i + "]", "track_loaded", PioneerDDJFLX10.samplerPlayOutputCallbackFunction);
    }

    decks.forEach(function(group) {
        engine.makeConnection(group, "sampler_page", PioneerDDJFLX10.samplerPlayOutputCallbackFunction);

        PioneerDDJFLX10.setLoopButtonLights(PioneerDDJFLX10.deckStatus(group), 0x7F);

        // DJ Sugar: the controller starts in HOT CUE mode; light its button.
        midi.sendShortMsg(PioneerDDJFLX10.deckStatus(group), PioneerDDJFLX10.padModeLights.hotcueMode, 0x7F);

        // DJ Sugar: the pads follow the pages in the deck's pad tabs.
        for (let tab = 1; tab <= 8; ++tab) {
            engine.makeConnection(group, "pad_slot_" + tab, PioneerDDJFLX10.padSlotChanged);
        }

        engine.makeConnection(group, "loop_enabled", PioneerDDJFLX10.loopToggle);

        // Register callbacks for each deck, when a file is loaded and the number of stems is available
        engine.makeConnection(group, "stem_count", PioneerDDJFLX10.stemCountChanged);

        // Register callbacks for each stems of each decks, to change pad lights when muted/unmuted
        for (let stem = 1; stem <= 5; stem++) {
            engine.makeConnection(PioneerDDJFLX10.stemGroup(group, stem), "mute", PioneerDDJFLX10.stemMuteChanged);
        }

        // Register callbacks for each deck, when a file is loaded to reset pitch shift
        engine.makeConnection(group, "track_loaded", PioneerDDJFLX10.pitchAdjusted);

        // Register callbacks for each deck, when the pitch shift is modified
        engine.makeConnection(group, "pitch_adjust", PioneerDDJFLX10.pitchAdjusted);

        engine.makeConnection(group, "sync_leader", PioneerDDJFLX10.syncLeaderLight);
        PioneerDDJFLX10.routePads(group);
    });
    PioneerDDJFLX10.samplerPlayOutputCallbackFunction();

    // The jog displays: show them, light the rings, follow the decks.
    PioneerDDJFLX10.decks.forEach(function(group, i) {
        midi.sendShortMsg(0x9F, 0x5D + i, 0x00);
        midi.sendShortMsg(0xBF, 0x09 + i, 0x01);
        engine.makeConnection(group, "keylock", PioneerDDJFLX10.keylockDisplay).trigger();
        engine.makeConnection(group, "sync_leader", PioneerDDJFLX10.leaderDisplay).trigger();
        engine.makeConnection(group, "sync_enabled", PioneerDDJFLX10.syncDisplay).trigger();
        engine.makeConnection("[QuickEffectRack1_" + group + "]", "loaded_chain_preset", PioneerDDJFLX10.colorFxLights);
    });
    PioneerDDJFLX10.colorFxLights();
    PioneerDDJFLX10.jogDisplays = [[-1, -1, -1, -1, -1], [-1, -1, -1, -1, -1], [-1, -1, -1, -1, -1], [-1, -1, -1, -1, -1]];
    PioneerDDJFLX10.jogDisplayTimer = engine.beginTimer(40, PioneerDDJFLX10.jogDisplayUpdate);
};

//
// Waveform zoom
//

PioneerDDJFLX10.waveformZoom = function(midichan, control, value, status, group) {
    if (value === 0x7f) {
        script.triggerControl(group, "waveform_zoom_up", 100);
    } else {
        script.triggerControl(group, "waveform_zoom_down", 100);
    }
};

//
// Channel level lights
//

PioneerDDJFLX10.vuMeterUpdate = function(value, group) {
    midi.sendShortMsg(0xB0 + PioneerDDJFLX10.deckIndex(group), 0x02, value * 127);
};

//
// Jog displays
//

// A 14-bit value as two CCs on the display channel (16).
PioneerDDJFLX10.sendDisplay = function(msb, value) {
    midi.sendShortMsg(0xBF, msb, (value >> 7) & 0x7F);
    midi.sendShortMsg(0xBF, msb + 0x20, value & 0x7F);
};

// Degrees (0 to 359) round a record at 33 1/3 rpm after some seconds.
PioneerDDJFLX10.degrees = function(seconds) {
    const turns = Math.max(0, seconds) * (100 / 3) / 60;
    return Math.floor((turns - Math.floor(turns)) * 360) % 360;
};

// The playhead marker, the cue point, BPM and time, sent when they change.
PioneerDDJFLX10.jogDisplayUpdate = function() {
    PioneerDDJFLX10.decks.forEach(function(group, i) {
        const loaded = engine.getValue(group, "track_loaded");
        const duration = engine.getValue(group, "duration");
        const seconds = engine.getValue(group, "playposition") * duration;
        let cue = 0x3FFF;
        const rate = engine.getValue(group, "track_samplerate");
        const point = engine.getValue(group, "cue_point");
        if (loaded && rate > 0 && point >= 0) {
            cue = PioneerDDJFLX10.degrees(point / rate / 2);
        }
        const left = Math.max(0, Math.floor(loaded ? seconds : 0));
        const now = [
            loaded ? PioneerDDJFLX10.degrees(seconds) : 0,
            cue,
            loaded ? Math.min(9999, Math.round(engine.getValue(group, "bpm") * 10)) : 0,
            Math.min(99, Math.floor(left / 60)),
            left % 60,
        ];
        const last = PioneerDDJFLX10.jogDisplays[i];
        if (now[0] !== last[0]) { PioneerDDJFLX10.sendDisplay(0x10 + i, now[0]); }
        if (now[1] !== last[1]) { PioneerDDJFLX10.sendDisplay(0x1C + i, now[1]); }
        if (now[2] !== last[2]) { PioneerDDJFLX10.sendDisplay(0x14 + i, now[2]); }
        if (now[3] !== last[3]) { midi.sendShortMsg(0xBF, 0x42 + 2 * i, now[3]); }
        if (now[4] !== last[4]) { midi.sendShortMsg(0xBF, 0x43 + 2 * i, now[4]); }
        PioneerDDJFLX10.jogDisplays[i] = now;
    });
};

// MASTER TEMPO, MASTER and SYNC on the jog displays.
PioneerDDJFLX10.displayLight = function(first) {
    return function(value, group) {
        midi.sendShortMsg(0x9F, first + PioneerDDJFLX10.deckIndex(group), value > 0 ? 0x7F : 0x00);
    };
};
PioneerDDJFLX10.keylockDisplay = PioneerDDJFLX10.displayLight(0x20);
PioneerDDJFLX10.leaderDisplay = PioneerDDJFLX10.displayLight(0x18);
PioneerDDJFLX10.syncDisplay = PioneerDDJFLX10.displayLight(0x1C);

//
// Sync leader (MASTER)
//

// MASTER makes the deck the one the others sync to, and again lets the
// app choose.
PioneerDDJFLX10.masterPressed = function(_channel, _control, value, _status, group) {
    if (value) {
        engine.setValue(group, "sync_leader", engine.getValue(group, "sync_leader") === 2 ? 0 : 2);
    }
};

PioneerDDJFLX10.syncLeaderLight = function(value, group) {
    midi.sendShortMsg(PioneerDDJFLX10.deckStatus(group), 0x5C, value > 0 ? 0x7F : 0x00);
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
PioneerDDJFLX10.beatFxUnit = "[EffectRack1_EffectUnit5]";
PioneerDDJFLX10.beatFxSlot = "[EffectRack1_EffectUnit5_Effect1]";

PioneerDDJFLX10.shifted = function() {
    return PioneerDDJFLX10.shiftButtonDown.some(function(down) { return down; });
};

// The FLX10's BEAT FX has no lights of its own to follow the unit.

PioneerDDJFLX10.beatFxLevelDepthRotate = function(_channel, _control, value) {
    engine.setParameter(PioneerDDJFLX10.beatFxSlot, "meta", value / 0x7F);
};

// BEAT FX SELECT sends the position it lands on (notes 0x20 to 0x2D), here
// to the nearest engine effect (loaded_effect: 1 Echo, 2 Reverb, 3 Filter,
// 4 Flanger, 6 Phaser, 7 Gate, 8 Roll, 12 Spiral).
PioneerDDJFLX10.beatFxEffects = {
    0x20: 1, // LOW CUT ECHO
    0x21: 1, // ECHO
    0x22: 1, // MT DELAY
    0x23: 12, // SPIRAL
    0x24: 2, // REVERB
    0x25: 7, // TRANS
    0x26: 4, // ENIGMA JET
    0x27: 4, // FLANGER
    0x28: 6, // PHASER
    0x29: 3, // STRETCH
    0x2A: 8, // SLIP ROLL
    0x2B: 8, // ROLL
    0x2C: 6, // MOBIUS SAW
    0x2D: 6, // MOBIUS TRI
};

PioneerDDJFLX10.beatFxSelect = function(_channel, control, value) {
    if (value === 0) { return; }
    engine.setValue(PioneerDDJFLX10.beatFxSlot, "loaded_effect", PioneerDDJFLX10.beatFxEffects[control]);
};

// Halves (-1) or doubles (+1) the effect's length, 1/16 to 4 beats; effects
// without one (beats 0) stay as they are.
PioneerDDJFLX10.changeBeatFxBeats = function(direction) {
    const beats = engine.getValue(PioneerDDJFLX10.beatFxSlot, "beats");
    if (beats <= 0) { return; }
    const next = Math.min(4, Math.max(1 / 16, direction > 0 ? beats * 2 : beats / 2));
    engine.setValue(PioneerDDJFLX10.beatFxSlot, "beats", next);
};

PioneerDDJFLX10.pushBeatFxUnit = function(key) {
    engine.setValue(PioneerDDJFLX10.beatFxUnit, key, 1);
    engine.setValue(PioneerDDJFLX10.beatFxUnit, key, 0);
};

PioneerDDJFLX10.beatFxLeftPressed = function(_channel, _control, value) {
    if (value === 0) { return; }

    if (PioneerDDJFLX10.shifted()) {
        PioneerDDJFLX10.pushBeatFxUnit("bpm_auto");
    } else {
        PioneerDDJFLX10.changeBeatFxBeats(-1);
    }
};

PioneerDDJFLX10.beatFxRightPressed = function(_channel, _control, value) {
    if (value === 0) { return; }

    if (PioneerDDJFLX10.shifted()) {
        PioneerDDJFLX10.pushBeatFxUnit("bpm_tap");
    } else {
        PioneerDDJFLX10.changeBeatFxBeats(1);
    }
};

PioneerDDJFLX10.beatFxOnOffPressed = function(_channel, _control, value) {
    if (value === 0) { return; }

    const toggleEnabled = !engine.getValue(PioneerDDJFLX10.beatFxSlot, "enabled");
    engine.setValue(PioneerDDJFLX10.beatFxSlot, "enabled", toggleEnabled);
};

PioneerDDJFLX10.beatFxOnOffShiftPressed = function(_channel, _control, value) {
    if (value === 0) { return; }

    PioneerDDJFLX10.pushBeatFxUnit("release");
};

// CH SELECT sends the position it lands on: CH1 to CH4 (notes 0x10 to
// 0x13), MASTER (0x14), which sends the effect to all four decks, or MIC
// and SAMPLER (0x15, 0x16), which send it to none.
PioneerDDJFLX10.beatFxChannelSelect = function(_channel, control, value) {
    if (value === 0) { return; }

    PioneerDDJFLX10.decks.forEach(function(group, i) {
        const on = control === 0x14 || control === 0x10 + i;
        engine.setValue(PioneerDDJFLX10.beatFxUnit, "group_" + group + "_enable", on ? 1 : 0);
    });
};

// DECK 1/3 and 2/4: the FLX10 says which deck a side now plays with note 0x3C
// on that deck's channel ("control on").
PioneerDDJFLX10.deckSelected = function(channel, _control, value) {
    if (value === 0x7F) {
        PioneerDDJFLX10.noteSide(channel);
    }
};

//
// DJ Sugar: the FLX10's own buttons
//

// SOUND COLOR FX: each picks the Color FX the COLOR knobs play; pressing
// the lit one goes back to the filter. Numbers as in the engine's COLOR_FX
// (1 Filter, 2 Space, 3 Dub Echo, 4 Noise, 5 Crush).
PioneerDDJFLX10.colorFxButtons = {0x00: 2, 0x01: 3, 0x02: 5, 0x04: 4, 0x05: 1};

PioneerDDJFLX10.colorFxPressed = function(_channel, control, value) {
    if (!value) { return; }
    const preset = PioneerDDJFLX10.colorFxButtons[control];
    const current = engine.getValue("[QuickEffectRack1_[Channel1]]", "loaded_chain_preset");
    const next = current === preset ? 1 : preset;
    PioneerDDJFLX10.decks.forEach(function(group) {
        engine.setValue("[QuickEffectRack1_" + group + "]", "loaded_chain_preset", next);
    });
};

PioneerDDJFLX10.colorFxLights = function() {
    const current = engine.getValue("[QuickEffectRack1_[Channel1]]", "loaded_chain_preset");
    Object.keys(PioneerDDJFLX10.colorFxButtons).forEach(function(control) {
        const preset = PioneerDDJFLX10.colorFxButtons[control];
        midi.sendShortMsg(0x96, parseInt(control), preset === current && preset !== 1 ? 0x7F : 0x00);
    });
};

// CROSSFADER ASSIGN sends 0x7F on the note of the side it moves to: A
// (0x16), THRU (0x1D) or B (0x18).
PioneerDDJFLX10.crossfaderSides = {0x16: 0, 0x1D: 1, 0x18: 2};
PioneerDDJFLX10.crossfaderAssign = function(_channel, control, value, _status, group) {
    if (value === 0x7F) {
        engine.setValue(group, "orientation", PioneerDDJFLX10.crossfaderSides[control]);
    }
};

// ACTIVE PART: DRUMS (kick and hi-hats), VOCAL and INST (bass and
// instruments) mute and unmute those stems ([ChannelN_StemM]: 1 kick,
// 2 bass, 3 instruments, 4 vocals, 5 hi-hats).
PioneerDDJFLX10.activeParts = {0x0D: [1, 5], 0x0E: [4], 0x0F: [2, 3]};
PioneerDDJFLX10.activePartPressed = function(_channel, control, value, _status, group) {
    if (!value) { return; }
    const stems = PioneerDDJFLX10.activeParts[control];
    const muted = engine.getValue(PioneerDDJFLX10.stemGroup(group, stems[0]), "mute") ? 0 : 1;
    stems.forEach(function(stem) {
        engine.setValue(PioneerDDJFLX10.stemGroup(group, stem), "mute", muted);
    });
};

// 4 BEAT JUMP < >.
PioneerDDJFLX10.beatJump = function(beats) {
    return function(_channel, _control, value, _status, group) {
        if (value) {
            engine.setValue(group, "beatjump", beats);
        }
    };
};
PioneerDDJFLX10.beatJumpBack = PioneerDDJFLX10.beatJump(-4);
PioneerDDJFLX10.beatJumpForward = PioneerDDJFLX10.beatJump(4);

// SLIP.
PioneerDDJFLX10.toggleSlip = function(_channel, _control, value, _status, group) {
    if (value) {
        script.toggleControl(group, "slip_enabled");
    }
};

//
// Loop IN/OUT ADJUST
//

PioneerDDJFLX10.toggleLoopAdjustIn = function(channel, _control, value, _status, group) {
    if (value === 0 || engine.getValue(group, "loop_enabled" === 0)) {
        return;
    }
    PioneerDDJFLX10.loopAdjustIn[channel] = !PioneerDDJFLX10.loopAdjustIn[channel];
    PioneerDDJFLX10.loopAdjustOut[channel] = false;
};

PioneerDDJFLX10.toggleLoopAdjustOut = function(channel, _control, value, _status, group) {
    if (value === 0 || engine.getValue(group, "loop_enabled" === 0)) {
        return;
    }
    PioneerDDJFLX10.loopAdjustOut[channel] = !PioneerDDJFLX10.loopAdjustOut[channel];
    PioneerDDJFLX10.loopAdjustIn[channel] = false;
};

// Two signals are sent here so that the light stays lit/unlit in its shift state too
PioneerDDJFLX10.setReloopLight = function(status, value) {
    midi.sendShortMsg(status, 0x14, value);
    midi.sendShortMsg(status, 0x50, value);
};


PioneerDDJFLX10.setLoopButtonLights = function(status, value) {
    [0x10, 0x11, 0x4D, 0x4C].forEach(function(control) {
        midi.sendShortMsg(status, control, value);
    });
};

PioneerDDJFLX10.startLoopLightsBlink = function(channel, control, status, group) {
    let blink = 0x7F;

    PioneerDDJFLX10.stopLoopLightsBlink(group, control, status);

    PioneerDDJFLX10.timers[group][control] = engine.beginTimer(500, () => {
        blink = 0x7F - blink;

        // When adjusting the loop out position, turn the loop in light off
        if (PioneerDDJFLX10.loopAdjustOut[channel]) {
            midi.sendShortMsg(status, 0x10, 0x00);
            midi.sendShortMsg(status, 0x4C, 0x00);
        } else {
            midi.sendShortMsg(status, 0x10, blink);
            midi.sendShortMsg(status, 0x4C, blink);
        }

        // When adjusting the loop in position, turn the loop out light off
        if (PioneerDDJFLX10.loopAdjustIn[channel]) {
            midi.sendShortMsg(status, 0x11, 0x00);
            midi.sendShortMsg(status, 0x4D, 0x00);
        } else {
            midi.sendShortMsg(status, 0x11, blink);
            midi.sendShortMsg(status, 0x4D, blink);
        }
    });

};

PioneerDDJFLX10.stopLoopLightsBlink = function(group, control, status) {
    PioneerDDJFLX10.timers[group] = PioneerDDJFLX10.timers[group] || {};

    if (PioneerDDJFLX10.timers[group][control] !== undefined) {
        engine.stopTimer(PioneerDDJFLX10.timers[group][control]);
    }
    PioneerDDJFLX10.timers[group][control] = undefined;
    PioneerDDJFLX10.setLoopButtonLights(status, 0x7F);
};

PioneerDDJFLX10.loopToggle = function(value, group, control) {
    const status = PioneerDDJFLX10.deckStatus(group),
        channel = PioneerDDJFLX10.deckIndex(group);

    PioneerDDJFLX10.setReloopLight(status, value ? 0x7F : 0x00);

    if (value) {
        PioneerDDJFLX10.startLoopLightsBlink(channel, control, status, group);
    } else {
        PioneerDDJFLX10.stopLoopLightsBlink(group, control, status);
        PioneerDDJFLX10.loopAdjustIn[channel] = false;
        PioneerDDJFLX10.loopAdjustOut[channel] = false;
    }
};

//
// CUE/LOOP CALL
//

PioneerDDJFLX10.cueLoopCallLeft = function(_channel, _control, value, _status, group) {
    // DJ Sugar: loop_halve rather than loop_scale, so the loop size box on
    // screen follows, and sets the next loop's size when none is on.
    engine.setValue(group, "loop_halve", value ? 1 : 0);
};

PioneerDDJFLX10.cueLoopCallRight = function(_channel, _control, value, _status, group) {
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
PioneerDDJFLX10.syncPressed = function(channel, control, value, status, group) {
    if (value > 0) {
        script.toggleControl(group, "sync_enabled");
    }
};

PioneerDDJFLX10.syncLongPressed = function(channel, control, value, status, group) {
    if (value) {
        engine.setValue(group, "sync_enabled", 1);
    }
};

PioneerDDJFLX10.cycleTempoRange = function(_channel, _control, value, _status, group) {
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

PioneerDDJFLX10.jogTurn = function(channel, _control, value, _status, group) {
    const deckNum = channel + 1;
    // wheel center at 64; <64 rew >64 fwd
    let newVal = value - 64;

    // loop_in / out adjust
    const loopEnabled = engine.getValue(group, "loop_enabled");
    if (loopEnabled > 0) {
        if (PioneerDDJFLX10.loopAdjustIn[channel]) {
            newVal = newVal * PioneerDDJFLX10.loopAdjustMultiply + engine.getValue(group, "loop_start_position");
            engine.setValue(group, "loop_start_position", newVal);
            return;
        }
        if (PioneerDDJFLX10.loopAdjustOut[channel]) {
            newVal = newVal * PioneerDDJFLX10.loopAdjustMultiply + engine.getValue(group, "loop_end_position");
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


PioneerDDJFLX10.jogSearch = function(_channel, _control, value, _status, group) {
    const newVal = (value - 64) * PioneerDDJFLX10.fastSeekScale;
    engine.setValue(group, "jog", newVal);
};

PioneerDDJFLX10.jogTouch = function(channel, _control, value) {
    const deckNum = channel + 1;
    PioneerDDJFLX10.noteSide(channel);

    // skip while adjusting the loop points
    if (PioneerDDJFLX10.loopAdjustIn[channel] || PioneerDDJFLX10.loopAdjustOut[channel]) {
        return;
    }

    if (value !== 0 && PioneerDDJFLX10.vinylMode(deckNum)) {
        engine.scratchEnable(deckNum, 720, 33+1/3, this.alpha, this.beta);
    } else {
        engine.scratchDisable(deckNum);
    }
};

//
// Shift button
//

PioneerDDJFLX10.shiftPressed = function(channel, _control, value, _status, _group) {
    PioneerDDJFLX10.shiftButtonDown[channel] = value === 0x7F;
};


//
// Tempo sliders
//
// The tempo option in Mixxx's deck preferences determine whether down/up
// increases/decreases the rate. Therefore it must be inverted here so that the
// UI and the control sliders always move in the same direction.
//

PioneerDDJFLX10.tempoSliderMSB = function(channel, control, value, status, group) {
    PioneerDDJFLX10.highResMSB[group].tempoSlider = value;
};

PioneerDDJFLX10.tempoSliderLSB = function(channel, control, value, status, group) {
    const fullValue = (PioneerDDJFLX10.highResMSB[group].tempoSlider << 7) + value;

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

PioneerDDJFLX10.beatjumpPadPressed = function(_channel, control, value, _status, group) {
    if (value === 0) {
        return;
    }
    engine.setValue(group, "beatjump_size", Math.abs(PioneerDDJFLX10.beatjumpSizeForPad[control]));
    engine.setValue(group, "beatjump", PioneerDDJFLX10.beatjumpSizeForPad[control]);
};

PioneerDDJFLX10.increaseBeatjumpSizes = function(_channel, control, value, _status, group) {
    if (value === 0 || PioneerDDJFLX10.beatjumpSizeForPad[0x21] * 16 > 16) {
        return;
    }
    Object.keys(PioneerDDJFLX10.beatjumpSizeForPad).forEach(function(pad) {
        PioneerDDJFLX10.beatjumpSizeForPad[pad] = PioneerDDJFLX10.beatjumpSizeForPad[pad] * 16;
    });
    engine.setValue(group, "beatjump_size", PioneerDDJFLX10.beatjumpSizeForPad[0x21]);
};

PioneerDDJFLX10.decreaseBeatjumpSizes = function(_channel, control, value, _status, group) {
    if (value === 0 || PioneerDDJFLX10.beatjumpSizeForPad[0x21] / 16 < 1/16) {
        return;
    }
    Object.keys(PioneerDDJFLX10.beatjumpSizeForPad).forEach(function(pad) {
        PioneerDDJFLX10.beatjumpSizeForPad[pad] = PioneerDDJFLX10.beatjumpSizeForPad[pad] / 16;
    });
    engine.setValue(group, "beatjump_size", PioneerDDJFLX10.beatjumpSizeForPad[0x21]);
};

//
// Sampler mode
//

// DJ Sugar: each deck's sampler pads play the 8 slots of its sampler page
// (`sampler_page`: 0 slots 1-8, 1 slots 9-16), the same 8 the screen shows.
// The mapping names the rekordbox layout (left pads 1-4 and 9-12, right
// pads 5-8 and 13-16), which only says which pad was pressed; the status
// says which deck.
PioneerDDJFLX10.samplerSlot = function(group, status) {
    const n = parseInt(group.match(script.samplerRegEx)[1]) - 1;
    const pad = n % 4 + (n >= 8 ? 4 : 0);
    const deck = PioneerDDJFLX10.decks[Math.floor(((status & 0x0F) - 7) / 2)];
    const page = engine.getValue(deck, "sampler_page");
    return "[Sampler" + (page * 8 + pad + 1) + "]";
};

// Lights a deck's sampler pads: on for a slot with a sample, blinking while
// it plays.
PioneerDDJFLX10.samplerLights = function(deck) {
    const channel = PioneerDDJFLX10.padStatus(deck);
    const page = engine.getValue(deck, "sampler_page");
    for (let pad = 0; pad < 8; ++pad) {
        const group = "[Sampler" + (page * 8 + pad + 1) + "]";
        PioneerDDJFLX10.stopSamplerBlink(channel, 0x30 + pad);
        if (engine.getValue(group, "play") === 1) {
            PioneerDDJFLX10.startSamplerBlink(channel, 0x30 + pad, group);
        } else {
            const on = engine.getValue(group, "track_loaded") ? 0x7F : 0x00;
            midi.sendShortMsg(channel, 0x30 + pad, on);
            midi.sendShortMsg(channel + 1, 0x30 + pad, on);
        }
    }
};

PioneerDDJFLX10.samplerPlayOutputCallbackFunction = function(_value, _group, _control) {
    PioneerDDJFLX10.decks.forEach(PioneerDDJFLX10.samplerLights);
};

// DJ Sugar: the pad tab each pad mode button opens on the deck's screen (see
// the deck's `pad_tab` control): tabs 1-4 for the four buttons, 5-8 for the
// same buttons with SHIFT, whatever page is in the tab.
PioneerDDJFLX10.padTabForMode = {
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
PioneerDDJFLX10.padModeNotes = [0x00, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70];

// DJ Sugar: the pad notes that play each pad panel (numbers as for the deck's
// `pad_panel`): its own pad mode's, or for loop rolls notes of their own
// (see the LOOP ROLL controls). Stems share KEYBOARD's notes (keyboardStems).
PioneerDDJFLX10.panelNotes = [0x00, 0x10, 0x20, 0x30, 0x40, 0x60, 0x50, 0x40, 0x70, 0x78];

// DJ Sugar: the pads follow the screen. Each pad mode's pads play the panel in
// its tab (the deck's `pad_slot_N`), with that panel's lights, whatever it
// is: put STEMS in tab 4 and the SAMPLER button gives stems pads.
PioneerDDJFLX10.routePads = function(group) {
    const first = PioneerDDJFLX10.padStatus(group) & 0x0F;
    const channels = [first, first + 1];
    for (let tab = 0; tab < 8; ++tab) {
        const panel = engine.getValue(group, "pad_slot_" + (tab + 1));
        const to = PioneerDDJFLX10.panelNotes[panel];
        for (let pad = 0; pad < 8; ++pad) {
            const from = PioneerDDJFLX10.padModeNotes[tab] + pad;
            channels.forEach(function(channel) {
                midi.routeNote(channel, from, to === undefined ? from : to + pad);
            });
        }
    }
    // KEYBOARD's notes play stems while the open tab shows them.
    const open = PioneerDDJFLX10.padTabForMode[PioneerDDJFLX10.padMode[group]];
    const stems = engine.getValue(group, "pad_slot_" + (open + 1)) === 4;
    if (stems !== PioneerDDJFLX10.keyboardStems[group]) {
        PioneerDDJFLX10.keyboardStems[group] = stems;
        PioneerDDJFLX10.keyboardLights(group);
    }
};

PioneerDDJFLX10.padSlotChanged = function(_value, group, _control) {
    PioneerDDJFLX10.routePads(group);
};

// DJ Sugar: the pad mode button last pressed on each deck.
PioneerDDJFLX10.padMode = {"[Channel1]": 0x1B, "[Channel2]": 0x1B, "[Channel3]": 0x1B, "[Channel4]": 0x1B};

// DJ Sugar: KEYBOARD plays the last hot cue pressed at eight pitches, as in
// rekordbox. Pressing KEYBOARD again turns its pads into the stems pads
// (see stemPads), and back.
PioneerDDJFLX10.keyboardStems = {"[Channel1]": false, "[Channel2]": false, "[Channel3]": false, "[Channel4]": false};

PioneerDDJFLX10.padModeKeyPressed = function(channel, control, value, _status, _group) {
    const group = PioneerDDJFLX10.decks[channel];
    const status = 0x90 + channel;

    if (value && control in PioneerDDJFLX10.padTabForMode) {
        const again = PioneerDDJFLX10.padMode[group] === control;
        const tab = PioneerDDJFLX10.padTabForMode[control];
        PioneerDDJFLX10.padMode[group] = control;
        engine.setValue(group, "pad_tab", tab);
        // KEYBOARD again swaps its tab between keyboard and stems.
        const slot = "pad_slot_" + (tab + 1);
        const panel = engine.getValue(group, slot);
        if (control === 0x69 && again && (panel === 7 || panel === 4)) {
            engine.setValue(group, slot, panel === 7 ? 4 : 7);
        }
        PioneerDDJFLX10.routePads(group);
    }
    // DJ Sugar: only the chosen mode stays lit.
    if (value) {
        Object.keys(PioneerDDJFLX10.padModeLights).forEach(function(mode) {
            midi.sendShortMsg(status, PioneerDDJFLX10.padModeLights[mode], 0x00);
        });
    }
    const light = {
        0x1B: "hotcueMode", 0x69: "keyboardMode", 0x1E: "padFX1Mode", 0x6B: "padFX2Mode",
        0x20: "beatJumpMode", 0x6D: "beatLoopMode", 0x22: "samplerMode", 0x6F: "keyShiftMode",
    }[control];
    if (light) {
        midi.sendShortMsg(status, PioneerDDJFLX10.padModeLights[light], 0x7F);
    }
};

//
// DJ Sugar: KEYBOARD mode
//

// The engine's KEYBOARD_SEMITONES, pad by pad (keyboard_N_activate).
PioneerDDJFLX10.keyboardSemitones = [0, 1, 2, 3, -4, -3, -2, -1];

PioneerDDJFLX10.keyboardPadPressed = function(channel, control, value, status, group) {
    if (PioneerDDJFLX10.keyboardStems[group]) {
        PioneerDDJFLX10.stemMutePadPressed(channel, control, value, status, group);
        return;
    }
    const pad = control - PioneerDDJFLX10.stemMutePadsFirstControl + 1;
    engine.setValue(group, "keyboard_" + pad + "_activate", value === 0x7F ? 1 : 0);
};

PioneerDDJFLX10.keyboardPadShiftPressed = function(channel, control, value, status, group) {
    if (PioneerDDJFLX10.keyboardStems[group]) {
        PioneerDDJFLX10.stemMutePadShiftPressed(channel, control, value, status, group);
        return;
    }
    // SHIFT + pad plays its pitch too.
    PioneerDDJFLX10.keyboardPadPressed(channel, control, value, status, group);
};

// Lights the KEYBOARD pads: the stems in stems mode, else the pad of the key
// shift playing.
PioneerDDJFLX10.keyboardLights = function(group) {
    if (PioneerDDJFLX10.keyboardStems[group]) {
        PioneerDDJFLX10.stemCountChanged(0, group, "stem_count");
        return;
    }
    const pitch = engine.getValue(group, "pitch_adjust");
    for (let i = 0; i < 8; i++) {
        const lit = Math.abs(PioneerDDJFLX10.keyboardSemitones[i] - pitch) < 0.01;
        PioneerDDJFLX10.stemsPadsModesStatus[group].forEach(function(status) {
            midi.sendShortMsg(status, PioneerDDJFLX10.stemMutePadsFirstControl + i, lit ? 0x7F : 0x00);
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
PioneerDDJFLX10.padFx = {
    0x10: [1, 0.6], 0x11: [2, 0.7], 0x12: [3, 0.2], 0x13: [3, 0.8],
    0x14: [4, 0.7], 0x15: [6, 0.7], 0x16: [5, 0.7], 0x17: [1, 1.0],
    0x50: [8, 0.5], 0x51: [7, 0.5], 0x52: [9, 0.6], 0x53: [10, 0.5],
    0x54: [11, 0.7], 0x55: [12, 0.7], 0x56: [7, 0.9], 0x57: [2, 1.0],
};

// Holding a pad puts its effect on the deck through the deck's pad FX unit
// (units 6 to 9 for decks 1 to 4); letting go turns it off.
PioneerDDJFLX10.padFxPressed = function(_channel, control, value, status, group) {
    const fx = PioneerDDJFLX10.padFx[control];
    if (fx === undefined) {
        return;
    }
    const deck = PioneerDDJFLX10.deckIndex(group) + 1;
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

PioneerDDJFLX10.samplerPadPressed = function(_channel, _control, value, status, mapped) {
    const group = PioneerDDJFLX10.samplerSlot(mapped, status);
    if (engine.getValue(group, "track_loaded")) {
        engine.setValue(group, "cue_gotoandplay", value);
    } else {
        engine.setValue(group, "LoadSelectedTrack", value);
    }
};

PioneerDDJFLX10.samplerPadShiftPressed = function(_channel, _control, value, status, mapped) {
    const group = PioneerDDJFLX10.samplerSlot(mapped, status);
    if (engine.getValue(group, "play")) {
        engine.setValue(group, "cue_gotoandstop", value);
    } else if (engine.getValue(group, "track_loaded")) {
        engine.setValue(group, "eject", value);
    }
};

PioneerDDJFLX10.startSamplerBlink = function(channel, control, group) {
    let val = 0x7f;

    PioneerDDJFLX10.stopSamplerBlink(channel, control);
    PioneerDDJFLX10.timers[channel][control] = engine.beginTimer(250, () => {
        val = 0x7f - val;

        // blink the appropriate pad
        midi.sendShortMsg(channel, control, val);
        // also blink the pad while SHIFT is pressed
        midi.sendShortMsg((channel+1), control, val);

        const isPlaying = engine.getValue(group, "play") === 1;

        if (!isPlaying) {
            // kill timer
            PioneerDDJFLX10.stopSamplerBlink(channel, control);
            // set the pad LED to ON
            midi.sendShortMsg(channel, control, 0x7f);
            // set the pad LED to ON while SHIFT is pressed
            midi.sendShortMsg((channel+1), control, 0x7f);
        }
    });
};

PioneerDDJFLX10.stopSamplerBlink = function(channel, control) {
    PioneerDDJFLX10.timers[channel] = PioneerDDJFLX10.timers[channel] || {};

    if (PioneerDDJFLX10.timers[channel][control] !== undefined) {
        engine.stopTimer(PioneerDDJFLX10.timers[channel][control]);
        PioneerDDJFLX10.timers[channel][control] = undefined;
    }
};


PioneerDDJFLX10.toggleQuantize = function(_channel, _control, value, _status, group) {
    if (value) {
        script.toggleControl(group, "quantize");
    }
};

PioneerDDJFLX10.quickJumpForward = function(_channel, _control, value, _status, group) {
    if (value) {
        engine.setValue(group, "beatjump", PioneerDDJFLX10.quickJumpSize);
    }
};

PioneerDDJFLX10.quickJumpBack = function(_channel, _control, value, _status, group) {
    if (value) {
        engine.setValue(group, "beatjump", -PioneerDDJFLX10.quickJumpSize);
    }
};

//
// Stems mode
//

// DJ Sugar: the stems pads, laid out as on screen (VirtualDJ's): Vocal,
// Instru, Bass, Acapella on top; Kick, HiHat, Stems FX, Instrumental below.
// A number from 1 is the stem ([ChannelN_StemM]: 1 kick, 2 bass, 3
// instruments, 4 vocals, 5 hi-hats); 0 is Acapella, -1 Instrumental and -2
// Stems FX, which does nothing yet.
PioneerDDJFLX10.stemPads = [4, 3, 2, 0, 1, 5, -2, -1];

PioneerDDJFLX10.stemGroup = function(group, stem) {
    return `[${group.substring(1, group.length-1)}_Stem${stem}]`;
};

// Whether each of the deck's stems plays (index 1 to 5); a stem the track
// doesn't have counts as playing.
PioneerDDJFLX10.stemsOn = function(group) {
    const count = engine.getValue(group, "stem_count");
    const on = [false];
    for (let stem = 1; stem <= 5; stem++) {
        on.push(stem > count || engine.getValue(PioneerDDJFLX10.stemGroup(group, stem), "mute") <= 0.5);
    }
    return on;
};

// Whether the pad for `kind` (see stemPads) is lit.
PioneerDDJFLX10.stemPadLit = function(group, kind) {
    const count = engine.getValue(group, "stem_count");
    if (count < 4) {
        return false;
    }
    const on = PioneerDDJFLX10.stemsOn(group);
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
PioneerDDJFLX10.setStems = function(group, vocals, rest) {
    for (let stem = 1; stem <= 5; stem++) {
        const on = stem === 4 ? vocals : rest;
        engine.setValue(PioneerDDJFLX10.stemGroup(group, stem), "mute", on ? 0 : 1);
    }
};

PioneerDDJFLX10.stemMutePadPressed = function(_channel, control, value, _status, group) {
    if (value !== 0x7f) {
        return;
    }
    const kind = PioneerDDJFLX10.stemPads[control - PioneerDDJFLX10.stemMutePadsFirstControl];
    const count = engine.getValue(group, "stem_count");
    if (count < 4 || kind === undefined || kind > count) {
        return;
    }
    if (kind > 0) {
        const stemGroup = PioneerDDJFLX10.stemGroup(group, kind);
        engine.setValue(stemGroup, "mute", engine.getValue(stemGroup, "mute") ? 0 : 1);
    } else if (kind === 0) {
        PioneerDDJFLX10.setStems(group, true, PioneerDDJFLX10.stemPadLit(group, 0));
    } else if (kind === -1) {
        PioneerDDJFLX10.setStems(group, PioneerDDJFLX10.stemPadLit(group, -1), true);
    }
};

// SHIFT + a stem's pad plays that stem alone.
PioneerDDJFLX10.stemMutePadShiftPressed = function(_channel, control, value, _status, group) {
    if (value !== 0x7f) {
        return;
    }
    const kind = PioneerDDJFLX10.stemPads[control - PioneerDDJFLX10.stemMutePadsFirstControl];
    const count = engine.getValue(group, "stem_count");
    if (count < 4 || kind === undefined || kind <= 0 || kind > count) {
        PioneerDDJFLX10.stemMutePadPressed(_channel, control, value, _status, group);
        return;
    }
    for (let stem = 1; stem <= count; stem++) {
        engine.setValue(PioneerDDJFLX10.stemGroup(group, stem), "mute", stem === kind ? 0 : 1);
    }
};

PioneerDDJFLX10.stemCountChanged = function(_value, group, _control) {
    const channel = group.match(/\[Channel\d+/)[0] + "]";
    if (!PioneerDDJFLX10.keyboardStems[channel]) {
        return;
    }
    for (let pad = 0; pad < 8; pad++) {
        const lit = PioneerDDJFLX10.stemPadLit(channel, PioneerDDJFLX10.stemPads[pad]);
        PioneerDDJFLX10.stemsPadsModesStatus[channel].forEach(function(status) {
            midi.sendShortMsg(status, PioneerDDJFLX10.stemMutePadsFirstControl + pad, lit ? 0x7F : 0x00);
        });
    }
};

PioneerDDJFLX10.stemMuteChanged = PioneerDDJFLX10.stemCountChanged;

//
// Pitch Shift mode
//

PioneerDDJFLX10.pitchAdjusted = function(_value, group, _control) {
    // DJ Sugar: the KEYBOARD pads show the key shift too.
    PioneerDDJFLX10.keyboardLights(group);
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

        PioneerDDJFLX10.pitchPadsModesStatus[group].forEach(
            (padMode) => midi.sendShortMsg(
                padMode,
                PioneerDDJFLX10.pitchPadsFirstControl + i,
                code,
            )
        );
    }
};

PioneerDDJFLX10.pitchPadPressed = function(_channel, control, value, _status, group) {
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

PioneerDDJFLX10.pitchPadShiftPressed = function(_channel, control, value, _status, group) {
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

PioneerDDJFLX10.shutdown = function() {
    engine.stopTimer(PioneerDDJFLX10.jogDisplayTimer);

    PioneerDDJFLX10.decks.forEach(function(group, i) {
        const status = PioneerDDJFLX10.deckStatus(group);
        const pads = PioneerDDJFLX10.padStatus(group);

        // reset vumeter and jog display
        midi.sendShortMsg(0xB0 + i, 0x02, 0x00);
        midi.sendShortMsg(0xBF, 0x09 + i, 0x00);
        midi.sendShortMsg(0x9F, 0x5D + i, 0x7F);

        // turn off all pad LEDs
        for (let pad = 0; pad <= 0x7F; ++pad) {
            midi.sendShortMsg(pads, pad, 0x00);
            midi.sendShortMsg(pads + 1, pad, 0x00);
        }

        // turn off loop in and out lights and reloop lights
        PioneerDDJFLX10.setLoopButtonLights(status, 0x00);
        PioneerDDJFLX10.setReloopLight(status, 0x00);
    });
    midi.sendShortMsg(0x96, 0x00, 0x00);
};
