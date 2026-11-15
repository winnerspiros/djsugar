// Pioneer-DDJ-FLX6-script.js
// ****************************************************************************
// * DJ Sugar mapping script for the Pioneer DDJ-FLX6.
// * Built on DJ Sugar's DDJ-FLX4 script (itself from Mixxx's DDJ-400 and
// * DDJ-FLX4 mappings: Warker, nschloe, dj3730, jusko, Robert904), for the
// * FLX6's four decks and four-channel mixer, written from Pioneer's
// * "DDJ-FLX6 List of MIDI messages".
// ****************************************************************************
//
//  The FLX6 sends each deck on its own MIDI channel: decks 1 to 4 on
//  channels 1 to 4, their pads on channels 8/9, 10/11, 12/13 and 14/15
//  (the second of each pair with SHIFT held). The DECK buttons pick which
//  deck each side plays; the mixer always has all four channels.
//
//  As on the FLX4 (see its script), the pad mode buttons open the deck's
//  pad tabs and the pads play whatever page is in the tab.
//
//  BEAT FX (the FLX6's FX1 and FX2, three slots each) drive DJ Sugar's one
//  Beat FX unit: any slot's ON/OFF turns it on or off, any LEVEL/DEPTH is
//  its depth, FX SELECT steps the effect (SHIFT: backwards), BEAT < >
//  halve and double its length (SHIFT: AUTO and TAP tempo) and CH SELECT
//  sends it to one deck or, on MST, all four.
//
//  MERGE FX drives the side's deck FX unit (unit 1 to 4 for the deck that
//  side plays): turning it raises the unit's super knob, pressing it turns
//  the unit's effects on and off, its PRESET button steps the chain.
//
//  The jog rings show the playhead going round as on a 33 1/3 record.
//
//  Not mapped yet: JOG CUTTER, CUE/LOOP MEMORY, MASTER CUE, INPUT SELECT,
//  the fader start messages, Serato's SP. SCRATCH.

var PioneerDDJFLX6 = {};

// The four decks: "[ChannelN]" from 0 to 3.
PioneerDDJFLX6.decks = ["[Channel1]", "[Channel2]", "[Channel3]", "[Channel4]"];

// A deck's index (0 to 3) from its group.
PioneerDDJFLX6.deckIndex = function(group) {
    return parseInt(group.match(script.channelRegEx)[1]) - 1;
};

// The status of a deck's notes and its pads (without SHIFT; +1 with it).
PioneerDDJFLX6.deckStatus = function(group) {
    return 0x90 + PioneerDDJFLX6.deckIndex(group);
};
PioneerDDJFLX6.padStatus = function(group) {
    return 0x97 + 2 * PioneerDDJFLX6.deckIndex(group);
};

// The deck playing on each side (left decks 1 and 3, right 2 and 4), as
// the DECK buttons last set it.
PioneerDDJFLX6.sideDeck = [0, 1];

PioneerDDJFLX6.noteSide = function(deck) {
    PioneerDDJFLX6.sideDeck[deck % 2] = deck;
};

// The pad mode buttons' lights, the same notes on every deck.
PioneerDDJFLX6.padModeLights = {
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
PioneerDDJFLX6.timers = {};

// Jog wheel constants
// DJ Sugar: whether touching the top scratches follows each deck's VINYL
// switch ([ChannelN] vinyl_mode, on by default) instead of this constant.
PioneerDDJFLX6.vinylMode = function(deckNum) {
    return engine.getValue("[Channel" + deckNum + "]", "vinyl_mode") > 0;
};
PioneerDDJFLX6.alpha = 1.0/8;
PioneerDDJFLX6.beta = PioneerDDJFLX6.alpha/32;

// Multiplier for fast seek through track using SHIFT+JOGWHEEL
// DJ Sugar: half of Mixxx's 150, which searched too fast.
PioneerDDJFLX6.fastSeekScale = 75;

PioneerDDJFLX6.tempoRanges = [0.06, 0.10, 0.16, 0.25];

PioneerDDJFLX6.shiftButtonDown = [false, false, false, false];

// Jog wheel loop adjust
PioneerDDJFLX6.loopAdjustIn = [false, false, false, false];
PioneerDDJFLX6.loopAdjustOut = [false, false, false, false];
PioneerDDJFLX6.loopAdjustMultiply = 50;

// Beatjump pad (beatjump_size values)
// DJ Sugar: 4, 8, 16 and 32 beats back and forward, as on the deck's screen.
PioneerDDJFLX6.beatjumpSizeForPad = {
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
PioneerDDJFLX6.stemsPadsModesStatus = {
    "[Channel1]": [0x97, 0x98],
    "[Channel2]": [0x99, 0x9a],
    "[Channel3]": [0x9b, 0x9c],
    "[Channel4]": [0x9d, 0x9e],
};

// Stems (KEYBOARD) pad 1 control (pad control = [this value] + [pad  number] - 1)
PioneerDDJFLX6.stemMutePadsFirstControl = 0x40;

// Pitch shift (KEY SHIFT) pads mode status for each deck, without or with SHIFT pressed
PioneerDDJFLX6.pitchPadsModesStatus = PioneerDDJFLX6.stemsPadsModesStatus;

// Pitch shift (KEY SHIFT) pad 1 control (pad control = [this value] + [pad  number] - 1)
PioneerDDJFLX6.pitchPadsFirstControl = 0x70;

PioneerDDJFLX6.quickJumpSize = 32;

// Used for tempo slider
PioneerDDJFLX6.highResMSB = {
    "[Channel1]": {},
    "[Channel2]": {},
    "[Channel3]": {},
    "[Channel4]": {},
};

PioneerDDJFLX6.toggleLight = function(midiIn, active) {
    midi.sendShortMsg(midiIn.status, midiIn.data1, active ? 0x7F : 0);
};

//
// Init
//

PioneerDDJFLX6.init = function() {
    const decks = PioneerDDJFLX6.decks;
    decks.forEach(function(group) {
        engine.makeConnection(group, "vu_meter", PioneerDDJFLX6.vuMeterUpdate);
        midi.sendShortMsg(0xB0 + PioneerDDJFLX6.deckIndex(group), 0x02, 0);
        engine.softTakeover(group, "rate", true);
    });
    engine.softTakeover(PioneerDDJFLX6.beatFxSlot, "meta", true);

    const samplerCount = 16;
    if (engine.getValue("[App]", "num_samplers") < samplerCount) {
        engine.setValue("[App]", "num_samplers", samplerCount);
    }
    for (let i = 1; i <= samplerCount; ++i) {
        engine.makeConnection("[Sampler" + i + "]", "play", PioneerDDJFLX6.samplerPlayOutputCallbackFunction);
        engine.makeConnection("[Sampler" + i + "]", "track_loaded", PioneerDDJFLX6.samplerPlayOutputCallbackFunction);
    }

    decks.forEach(function(group) {
        engine.makeConnection(group, "sampler_page", PioneerDDJFLX6.samplerPlayOutputCallbackFunction);

        PioneerDDJFLX6.setLoopButtonLights(PioneerDDJFLX6.deckStatus(group), 0x7F);

        // DJ Sugar: the controller starts in HOT CUE mode; light its button.
        midi.sendShortMsg(PioneerDDJFLX6.deckStatus(group), PioneerDDJFLX6.padModeLights.hotcueMode, 0x7F);

        // DJ Sugar: the pads follow the pages in the deck's pad tabs.
        for (let tab = 1; tab <= 8; ++tab) {
            engine.makeConnection(group, "pad_slot_" + tab, PioneerDDJFLX6.padSlotChanged);
        }

        engine.makeConnection(group, "loop_enabled", PioneerDDJFLX6.loopToggle);

        // Register callbacks for each deck, when a file is loaded and the number of stems is available
        engine.makeConnection(group, "stem_count", PioneerDDJFLX6.stemCountChanged);

        // Register callbacks for each stems of each decks, to change pad lights when muted/unmuted
        for (let stem = 1; stem <= 5; stem++) {
            engine.makeConnection(PioneerDDJFLX6.stemGroup(group, stem), "mute", PioneerDDJFLX6.stemMuteChanged);
        }

        // Register callbacks for each deck, when a file is loaded to reset pitch shift
        engine.makeConnection(group, "track_loaded", PioneerDDJFLX6.pitchAdjusted);

        // Register callbacks for each deck, when the pitch shift is modified
        engine.makeConnection(group, "pitch_adjust", PioneerDDJFLX6.pitchAdjusted);

        engine.makeConnection(group, "sync_leader", PioneerDDJFLX6.syncLeaderLight);
        PioneerDDJFLX6.routePads(group);
    });
    PioneerDDJFLX6.samplerPlayOutputCallbackFunction();

    // MERGE FX lights follow the deck FX units.
    for (let unit = 1; unit <= 4; ++unit) {
        for (let slot = 1; slot <= 3; ++slot) {
            engine.makeConnection("[EffectRack1_EffectUnit" + unit + "_Effect" + slot + "]", "enabled",
                PioneerDDJFLX6.mergeFxLights);
        }
    }
    PioneerDDJFLX6.mergeFxLights();

    // The jog rings show where each deck's playhead is.
    PioneerDDJFLX6.jogRings = [-1, -1, -1, -1];
    PioneerDDJFLX6.jogRingTimer = engine.beginTimer(40, PioneerDDJFLX6.jogRingUpdate);
};

//
// Waveform zoom
//

PioneerDDJFLX6.waveformZoom = function(midichan, control, value, status, group) {
    if (value === 0x7f) {
        script.triggerControl(group, "waveform_zoom_up", 100);
    } else {
        script.triggerControl(group, "waveform_zoom_down", 100);
    }
};

//
// Channel level lights
//

PioneerDDJFLX6.vuMeterUpdate = function(value, group) {
    midi.sendShortMsg(0xB0 + PioneerDDJFLX6.deckIndex(group), 0x02, value * 127);
};

//
// Jog rings
//

// The ring's 73 steps (0x00 to 0x48) go round once per turn of a record
// playing at 33 1/3 rpm.
PioneerDDJFLX6.jogRingUpdate = function() {
    PioneerDDJFLX6.decks.forEach(function(group, i) {
        let step = 0;
        if (engine.getValue(group, "track_loaded")) {
            const seconds = engine.getValue(group, "playposition") * engine.getValue(group, "duration");
            const turns = Math.max(0, seconds) * (100 / 3) / 60;
            step = Math.floor((turns - Math.floor(turns)) * 0x49);
        }
        if (step !== PioneerDDJFLX6.jogRings[i]) {
            PioneerDDJFLX6.jogRings[i] = step;
            midi.sendShortMsg(0xBB, i, step);
        }
    });
};

//
// Sync leader (MASTER)
//

// MASTER makes the deck the one the others sync to, and again lets the
// app choose.
PioneerDDJFLX6.masterPressed = function(_channel, _control, value, _status, group) {
    if (value) {
        engine.setValue(group, "sync_leader", engine.getValue(group, "sync_leader") === 2 ? 0 : 2);
    }
};

PioneerDDJFLX6.syncLeaderLight = function(value, group) {
    midi.sendShortMsg(PioneerDDJFLX6.deckStatus(group), 0x5C, value > 0 ? 0x7F : 0x00);
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
PioneerDDJFLX6.beatFxUnit = "[EffectRack1_EffectUnit5]";
PioneerDDJFLX6.beatFxSlot = "[EffectRack1_EffectUnit5_Effect1]";

PioneerDDJFLX6.shifted = function() {
    return PioneerDDJFLX6.shiftButtonDown.some(function(down) { return down; });
};

// The FLX6's BEAT FX has no lights of its own to follow the unit.

PioneerDDJFLX6.beatFxLevelDepthRotate = function(_channel, _control, value) {
    engine.setParameter(PioneerDDJFLX6.beatFxSlot, "meta", value / 0x7F);
};

// FX SELECT moves the FLX6's own pointer through FX1-1 to FX2-3, sending
// the slot it leaves (0x00) and the one it lands on (0x7F); SHIFT goes
// backwards. Each landing steps the effect.
PioneerDDJFLX6.beatFxSelectPressed = function(_channel, _control, value) {
    if (value === 0) { return; }

    const key = PioneerDDJFLX6.shifted() ? "prev_effect" : "next_effect";
    engine.setValue(PioneerDDJFLX6.beatFxSlot, key, 1);
    engine.setValue(PioneerDDJFLX6.beatFxSlot, key, 0);
};

// Halves (-1) or doubles (+1) the effect's length, 1/16 to 4 beats; effects
// without one (beats 0) stay as they are.
PioneerDDJFLX6.changeBeatFxBeats = function(direction) {
    const beats = engine.getValue(PioneerDDJFLX6.beatFxSlot, "beats");
    if (beats <= 0) { return; }
    const next = Math.min(4, Math.max(1 / 16, direction > 0 ? beats * 2 : beats / 2));
    engine.setValue(PioneerDDJFLX6.beatFxSlot, "beats", next);
};

PioneerDDJFLX6.pushBeatFxUnit = function(key) {
    engine.setValue(PioneerDDJFLX6.beatFxUnit, key, 1);
    engine.setValue(PioneerDDJFLX6.beatFxUnit, key, 0);
};

PioneerDDJFLX6.beatFxLeftPressed = function(_channel, _control, value) {
    if (value === 0) { return; }

    if (PioneerDDJFLX6.shifted()) {
        PioneerDDJFLX6.pushBeatFxUnit("bpm_auto");
    } else {
        PioneerDDJFLX6.changeBeatFxBeats(-1);
    }
};

PioneerDDJFLX6.beatFxRightPressed = function(_channel, _control, value) {
    if (value === 0) { return; }

    if (PioneerDDJFLX6.shifted()) {
        PioneerDDJFLX6.pushBeatFxUnit("bpm_tap");
    } else {
        PioneerDDJFLX6.changeBeatFxBeats(1);
    }
};

PioneerDDJFLX6.beatFxOnOffPressed = function(_channel, _control, value) {
    if (value === 0) { return; }

    const toggleEnabled = !engine.getValue(PioneerDDJFLX6.beatFxSlot, "enabled");
    engine.setValue(PioneerDDJFLX6.beatFxSlot, "enabled", toggleEnabled);
};

PioneerDDJFLX6.beatFxOnOffShiftPressed = function(_channel, _control, value) {
    if (value === 0) { return; }

    PioneerDDJFLX6.pushBeatFxUnit("release");
};

// CH SELECT sends the position it lands on (0x7F): CH1 to CH4 (notes 0x1C
// to 0x1F) or MST (0x14), which sends the effect to all four decks.
PioneerDDJFLX6.beatFxChannelSelect = function(_channel, control, value) {
    if (value !== 0x7F) { return; }

    PioneerDDJFLX6.decks.forEach(function(group, i) {
        const on = control === 0x14 || control === 0x1C + i;
        engine.setValue(PioneerDDJFLX6.beatFxUnit, "group_" + group + "_enable", on ? 1 : 0);
    });
};

//
// MERGE FX
//

// The deck FX unit of the deck a side plays (left 5, right 6 as MIDI channels).
PioneerDDJFLX6.mergeFxUnit = function(channel) {
    return "[EffectRack1_EffectUnit" + (PioneerDDJFLX6.sideDeck[channel - 4] + 1) + "]";
};

PioneerDDJFLX6.mergeFxOn = function(unit) {
    for (let slot = 1; slot <= 3; ++slot) {
        if (engine.getValue(unit.replace("]", "_Effect" + slot + "]"), "enabled")) {
            return true;
        }
    }
    return false;
};

PioneerDDJFLX6.mergeFxTurn = function(channel, _control, value) {
    const unit = PioneerDDJFLX6.mergeFxUnit(channel);
    const clicks = value < 0x40 ? value : value - 0x80;
    const next = Math.min(1, Math.max(0, engine.getValue(unit, "super1") + clicks / 64));
    engine.setValue(unit, "super1", next);
};

PioneerDDJFLX6.mergeFxPressed = function(channel, _control, value) {
    if (value === 0) { return; }

    const unit = PioneerDDJFLX6.mergeFxUnit(channel);
    const on = PioneerDDJFLX6.mergeFxOn(unit) ? 0 : 1;
    if (on) {
        engine.setValue(unit, "enabled", 1);
    }
    for (let slot = 1; slot <= 3; ++slot) {
        engine.setValue(unit.replace("]", "_Effect" + slot + "]"), "enabled", on);
    }
};

PioneerDDJFLX6.mergeFxPreset = function(channel, _control, value) {
    if (value === 0) { return; }

    const key = PioneerDDJFLX6.shifted() ? "prev_chain_preset" : "next_chain_preset";
    const unit = PioneerDDJFLX6.mergeFxUnit(channel);
    engine.setValue(unit, key, 1);
    engine.setValue(unit, key, 0);
};

PioneerDDJFLX6.mergeFxLights = function() {
    [0, 1].forEach(function(side) {
        const unit = PioneerDDJFLX6.mergeFxUnit(4 + side);
        midi.sendShortMsg(0xB4 + side, 0x10, PioneerDDJFLX6.mergeFxOn(unit) ? 0x7F : 0x00);
    });
};

// DECK 1/3 and 2/4: the FLX6 says which deck a side now plays with note 0x3C
// on that deck's channel ("control on").
PioneerDDJFLX6.deckSelected = function(channel, _control, value) {
    if (value === 0x7F) {
        PioneerDDJFLX6.noteSide(channel);
        PioneerDDJFLX6.mergeFxLights();
    }
};

//
// Loop IN/OUT ADJUST
//

PioneerDDJFLX6.toggleLoopAdjustIn = function(channel, _control, value, _status, group) {
    if (value === 0 || engine.getValue(group, "loop_enabled" === 0)) {
        return;
    }
    PioneerDDJFLX6.loopAdjustIn[channel] = !PioneerDDJFLX6.loopAdjustIn[channel];
    PioneerDDJFLX6.loopAdjustOut[channel] = false;
};

PioneerDDJFLX6.toggleLoopAdjustOut = function(channel, _control, value, _status, group) {
    if (value === 0 || engine.getValue(group, "loop_enabled" === 0)) {
        return;
    }
    PioneerDDJFLX6.loopAdjustOut[channel] = !PioneerDDJFLX6.loopAdjustOut[channel];
    PioneerDDJFLX6.loopAdjustIn[channel] = false;
};

// Two signals are sent here so that the light stays lit/unlit in its shift state too
PioneerDDJFLX6.setReloopLight = function(status, value) {
    midi.sendShortMsg(status, 0x4D, value);
    midi.sendShortMsg(status, 0x50, value);
};


PioneerDDJFLX6.setLoopButtonLights = function(status, value) {
    [0x10, 0x11, 0x77, 0x4C].forEach(function(control) {
        midi.sendShortMsg(status, control, value);
    });
};

PioneerDDJFLX6.startLoopLightsBlink = function(channel, control, status, group) {
    let blink = 0x7F;

    PioneerDDJFLX6.stopLoopLightsBlink(group, control, status);

    PioneerDDJFLX6.timers[group][control] = engine.beginTimer(500, () => {
        blink = 0x7F - blink;

        // When adjusting the loop out position, turn the loop in light off
        if (PioneerDDJFLX6.loopAdjustOut[channel]) {
            midi.sendShortMsg(status, 0x10, 0x00);
            midi.sendShortMsg(status, 0x4C, 0x00);
        } else {
            midi.sendShortMsg(status, 0x10, blink);
            midi.sendShortMsg(status, 0x4C, blink);
        }

        // When adjusting the loop in position, turn the loop out light off
        if (PioneerDDJFLX6.loopAdjustIn[channel]) {
            midi.sendShortMsg(status, 0x11, 0x00);
            midi.sendShortMsg(status, 0x77, 0x00);
        } else {
            midi.sendShortMsg(status, 0x11, blink);
            midi.sendShortMsg(status, 0x77, blink);
        }
    });

};

PioneerDDJFLX6.stopLoopLightsBlink = function(group, control, status) {
    PioneerDDJFLX6.timers[group] = PioneerDDJFLX6.timers[group] || {};

    if (PioneerDDJFLX6.timers[group][control] !== undefined) {
        engine.stopTimer(PioneerDDJFLX6.timers[group][control]);
    }
    PioneerDDJFLX6.timers[group][control] = undefined;
    PioneerDDJFLX6.setLoopButtonLights(status, 0x7F);
};

PioneerDDJFLX6.loopToggle = function(value, group, control) {
    const status = PioneerDDJFLX6.deckStatus(group),
        channel = PioneerDDJFLX6.deckIndex(group);

    PioneerDDJFLX6.setReloopLight(status, value ? 0x7F : 0x00);

    if (value) {
        PioneerDDJFLX6.startLoopLightsBlink(channel, control, status, group);
    } else {
        PioneerDDJFLX6.stopLoopLightsBlink(group, control, status);
        PioneerDDJFLX6.loopAdjustIn[channel] = false;
        PioneerDDJFLX6.loopAdjustOut[channel] = false;
    }
};

//
// CUE/LOOP CALL
//

PioneerDDJFLX6.cueLoopCallLeft = function(_channel, _control, value, _status, group) {
    // DJ Sugar: loop_halve rather than loop_scale, so the loop size box on
    // screen follows, and sets the next loop's size when none is on.
    engine.setValue(group, "loop_halve", value ? 1 : 0);
};

PioneerDDJFLX6.cueLoopCallRight = function(_channel, _control, value, _status, group) {
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
PioneerDDJFLX6.syncPressed = function(channel, control, value, status, group) {
    if (value > 0) {
        script.toggleControl(group, "sync_enabled");
    }
};

PioneerDDJFLX6.syncLongPressed = function(channel, control, value, status, group) {
    if (value) {
        engine.setValue(group, "sync_enabled", 1);
    }
};

PioneerDDJFLX6.cycleTempoRange = function(_channel, _control, value, _status, group) {
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

PioneerDDJFLX6.jogTurn = function(channel, _control, value, _status, group) {
    const deckNum = channel + 1;
    // wheel center at 64; <64 rew >64 fwd
    let newVal = value - 64;

    // loop_in / out adjust
    const loopEnabled = engine.getValue(group, "loop_enabled");
    if (loopEnabled > 0) {
        if (PioneerDDJFLX6.loopAdjustIn[channel]) {
            newVal = newVal * PioneerDDJFLX6.loopAdjustMultiply + engine.getValue(group, "loop_start_position");
            engine.setValue(group, "loop_start_position", newVal);
            return;
        }
        if (PioneerDDJFLX6.loopAdjustOut[channel]) {
            newVal = newVal * PioneerDDJFLX6.loopAdjustMultiply + engine.getValue(group, "loop_end_position");
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


PioneerDDJFLX6.jogSearch = function(_channel, _control, value, _status, group) {
    const newVal = (value - 64) * PioneerDDJFLX6.fastSeekScale;
    engine.setValue(group, "jog", newVal);
};

PioneerDDJFLX6.jogTouch = function(channel, _control, value) {
    const deckNum = channel + 1;
    PioneerDDJFLX6.noteSide(channel);

    // skip while adjusting the loop points
    if (PioneerDDJFLX6.loopAdjustIn[channel] || PioneerDDJFLX6.loopAdjustOut[channel]) {
        return;
    }

    if (value !== 0 && PioneerDDJFLX6.vinylMode(deckNum)) {
        engine.scratchEnable(deckNum, 720, 33+1/3, this.alpha, this.beta);
    } else {
        engine.scratchDisable(deckNum);
    }
};

//
// Shift button
//

PioneerDDJFLX6.shiftPressed = function(channel, _control, value, _status, _group) {
    PioneerDDJFLX6.shiftButtonDown[channel] = value === 0x7F;
};


//
// Tempo sliders
//
// The tempo option in Mixxx's deck preferences determine whether down/up
// increases/decreases the rate. Therefore it must be inverted here so that the
// UI and the control sliders always move in the same direction.
//

PioneerDDJFLX6.tempoSliderMSB = function(channel, control, value, status, group) {
    PioneerDDJFLX6.highResMSB[group].tempoSlider = value;
};

PioneerDDJFLX6.tempoSliderLSB = function(channel, control, value, status, group) {
    const fullValue = (PioneerDDJFLX6.highResMSB[group].tempoSlider << 7) + value;

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

PioneerDDJFLX6.beatjumpPadPressed = function(_channel, control, value, _status, group) {
    if (value === 0) {
        return;
    }
    engine.setValue(group, "beatjump_size", Math.abs(PioneerDDJFLX6.beatjumpSizeForPad[control]));
    engine.setValue(group, "beatjump", PioneerDDJFLX6.beatjumpSizeForPad[control]);
};

PioneerDDJFLX6.increaseBeatjumpSizes = function(_channel, control, value, _status, group) {
    if (value === 0 || PioneerDDJFLX6.beatjumpSizeForPad[0x21] * 16 > 16) {
        return;
    }
    Object.keys(PioneerDDJFLX6.beatjumpSizeForPad).forEach(function(pad) {
        PioneerDDJFLX6.beatjumpSizeForPad[pad] = PioneerDDJFLX6.beatjumpSizeForPad[pad] * 16;
    });
    engine.setValue(group, "beatjump_size", PioneerDDJFLX6.beatjumpSizeForPad[0x21]);
};

PioneerDDJFLX6.decreaseBeatjumpSizes = function(_channel, control, value, _status, group) {
    if (value === 0 || PioneerDDJFLX6.beatjumpSizeForPad[0x21] / 16 < 1/16) {
        return;
    }
    Object.keys(PioneerDDJFLX6.beatjumpSizeForPad).forEach(function(pad) {
        PioneerDDJFLX6.beatjumpSizeForPad[pad] = PioneerDDJFLX6.beatjumpSizeForPad[pad] / 16;
    });
    engine.setValue(group, "beatjump_size", PioneerDDJFLX6.beatjumpSizeForPad[0x21]);
};

//
// Sampler mode
//

// DJ Sugar: each deck's sampler pads play the 8 slots of its sampler page
// (`sampler_page`: 0 slots 1-8, 1 slots 9-16), the same 8 the screen shows.
// The mapping names the rekordbox layout (left pads 1-4 and 9-12, right
// pads 5-8 and 13-16), which only says which pad was pressed; the status
// says which deck.
PioneerDDJFLX6.samplerSlot = function(group, status) {
    const n = parseInt(group.match(script.samplerRegEx)[1]) - 1;
    const pad = n % 4 + (n >= 8 ? 4 : 0);
    const deck = PioneerDDJFLX6.decks[Math.floor(((status & 0x0F) - 7) / 2)];
    const page = engine.getValue(deck, "sampler_page");
    return "[Sampler" + (page * 8 + pad + 1) + "]";
};

// Lights a deck's sampler pads: on for a slot with a sample, blinking while
// it plays.
PioneerDDJFLX6.samplerLights = function(deck) {
    const channel = PioneerDDJFLX6.padStatus(deck);
    const page = engine.getValue(deck, "sampler_page");
    for (let pad = 0; pad < 8; ++pad) {
        const group = "[Sampler" + (page * 8 + pad + 1) + "]";
        PioneerDDJFLX6.stopSamplerBlink(channel, 0x30 + pad);
        if (engine.getValue(group, "play") === 1) {
            PioneerDDJFLX6.startSamplerBlink(channel, 0x30 + pad, group);
        } else {
            const on = engine.getValue(group, "track_loaded") ? 0x7F : 0x00;
            midi.sendShortMsg(channel, 0x30 + pad, on);
            midi.sendShortMsg(channel + 1, 0x30 + pad, on);
        }
    }
};

PioneerDDJFLX6.samplerPlayOutputCallbackFunction = function(_value, _group, _control) {
    PioneerDDJFLX6.decks.forEach(PioneerDDJFLX6.samplerLights);
};

// DJ Sugar: the pad tab each pad mode button opens on the deck's screen (see
// the deck's `pad_tab` control): tabs 1-4 for the four buttons, 5-8 for the
// same buttons with SHIFT, whatever page is in the tab.
PioneerDDJFLX6.padTabForMode = {
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
PioneerDDJFLX6.padModeNotes = [0x00, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70];

// DJ Sugar: the pad notes that play each pad panel (numbers as for the deck's
// `pad_panel`): its own pad mode's, or for loop rolls notes of their own
// (see the LOOP ROLL controls). Stems share KEYBOARD's notes (keyboardStems).
PioneerDDJFLX6.panelNotes = [0x00, 0x10, 0x20, 0x30, 0x40, 0x60, 0x50, 0x40, 0x70, 0x78];

// DJ Sugar: the pads follow the screen. Each pad mode's pads play the panel in
// its tab (the deck's `pad_slot_N`), with that panel's lights, whatever it
// is: put STEMS in tab 4 and the SAMPLER button gives stems pads.
PioneerDDJFLX6.routePads = function(group) {
    const first = PioneerDDJFLX6.padStatus(group) & 0x0F;
    const channels = [first, first + 1];
    for (let tab = 0; tab < 8; ++tab) {
        const panel = engine.getValue(group, "pad_slot_" + (tab + 1));
        const to = PioneerDDJFLX6.panelNotes[panel];
        for (let pad = 0; pad < 8; ++pad) {
            const from = PioneerDDJFLX6.padModeNotes[tab] + pad;
            channels.forEach(function(channel) {
                midi.routeNote(channel, from, to === undefined ? from : to + pad);
            });
        }
    }
    // KEYBOARD's notes play stems while the open tab shows them.
    const open = PioneerDDJFLX6.padTabForMode[PioneerDDJFLX6.padMode[group]];
    const stems = engine.getValue(group, "pad_slot_" + (open + 1)) === 4;
    if (stems !== PioneerDDJFLX6.keyboardStems[group]) {
        PioneerDDJFLX6.keyboardStems[group] = stems;
        PioneerDDJFLX6.keyboardLights(group);
    }
};

PioneerDDJFLX6.padSlotChanged = function(_value, group, _control) {
    PioneerDDJFLX6.routePads(group);
};

// DJ Sugar: the pad mode button last pressed on each deck.
PioneerDDJFLX6.padMode = {"[Channel1]": 0x1B, "[Channel2]": 0x1B, "[Channel3]": 0x1B, "[Channel4]": 0x1B};

// DJ Sugar: KEYBOARD plays the last hot cue pressed at eight pitches, as in
// rekordbox. Pressing KEYBOARD again turns its pads into the stems pads
// (see stemPads), and back.
PioneerDDJFLX6.keyboardStems = {"[Channel1]": false, "[Channel2]": false, "[Channel3]": false, "[Channel4]": false};

PioneerDDJFLX6.padModeKeyPressed = function(channel, control, value, _status, _group) {
    const group = PioneerDDJFLX6.decks[channel];
    const status = 0x90 + channel;

    if (value && control in PioneerDDJFLX6.padTabForMode) {
        const again = PioneerDDJFLX6.padMode[group] === control;
        const tab = PioneerDDJFLX6.padTabForMode[control];
        PioneerDDJFLX6.padMode[group] = control;
        engine.setValue(group, "pad_tab", tab);
        // KEYBOARD again swaps its tab between keyboard and stems.
        const slot = "pad_slot_" + (tab + 1);
        const panel = engine.getValue(group, slot);
        if (control === 0x69 && again && (panel === 7 || panel === 4)) {
            engine.setValue(group, slot, panel === 7 ? 4 : 7);
        }
        PioneerDDJFLX6.routePads(group);
    }
    // DJ Sugar: only the chosen mode stays lit.
    if (value) {
        Object.keys(PioneerDDJFLX6.padModeLights).forEach(function(mode) {
            midi.sendShortMsg(status, PioneerDDJFLX6.padModeLights[mode], 0x00);
        });
    }
    const light = {
        0x1B: "hotcueMode", 0x69: "keyboardMode", 0x1E: "padFX1Mode", 0x6B: "padFX2Mode",
        0x20: "beatJumpMode", 0x6D: "beatLoopMode", 0x22: "samplerMode", 0x6F: "keyShiftMode",
    }[control];
    if (light) {
        midi.sendShortMsg(status, PioneerDDJFLX6.padModeLights[light], 0x7F);
    }
};

//
// DJ Sugar: KEYBOARD mode
//

// The engine's KEYBOARD_SEMITONES, pad by pad (keyboard_N_activate).
PioneerDDJFLX6.keyboardSemitones = [0, 1, 2, 3, -4, -3, -2, -1];

PioneerDDJFLX6.keyboardPadPressed = function(channel, control, value, status, group) {
    if (PioneerDDJFLX6.keyboardStems[group]) {
        PioneerDDJFLX6.stemMutePadPressed(channel, control, value, status, group);
        return;
    }
    const pad = control - PioneerDDJFLX6.stemMutePadsFirstControl + 1;
    engine.setValue(group, "keyboard_" + pad + "_activate", value === 0x7F ? 1 : 0);
};

PioneerDDJFLX6.keyboardPadShiftPressed = function(channel, control, value, status, group) {
    if (PioneerDDJFLX6.keyboardStems[group]) {
        PioneerDDJFLX6.stemMutePadShiftPressed(channel, control, value, status, group);
        return;
    }
    // SHIFT + pad plays its pitch too.
    PioneerDDJFLX6.keyboardPadPressed(channel, control, value, status, group);
};

// Lights the KEYBOARD pads: the stems in stems mode, else the pad of the key
// shift playing.
PioneerDDJFLX6.keyboardLights = function(group) {
    if (PioneerDDJFLX6.keyboardStems[group]) {
        PioneerDDJFLX6.stemCountChanged(0, group, "stem_count");
        return;
    }
    const pitch = engine.getValue(group, "pitch_adjust");
    for (let i = 0; i < 8; i++) {
        const lit = Math.abs(PioneerDDJFLX6.keyboardSemitones[i] - pitch) < 0.01;
        PioneerDDJFLX6.stemsPadsModesStatus[group].forEach(function(status) {
            midi.sendShortMsg(status, PioneerDDJFLX6.stemMutePadsFirstControl + i, lit ? 0x7F : 0x00);
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
PioneerDDJFLX6.padFx = {
    0x10: [1, 0.6], 0x11: [2, 0.7], 0x12: [3, 0.2], 0x13: [3, 0.8],
    0x14: [4, 0.7], 0x15: [6, 0.7], 0x16: [5, 0.7], 0x17: [1, 1.0],
    0x50: [8, 0.5], 0x51: [7, 0.5], 0x52: [9, 0.6], 0x53: [10, 0.5],
    0x54: [11, 0.7], 0x55: [12, 0.7], 0x56: [7, 0.9], 0x57: [2, 1.0],
};

// Holding a pad puts its effect on the deck through the deck's pad FX unit
// (units 6 to 9 for decks 1 to 4); letting go turns it off.
PioneerDDJFLX6.padFxPressed = function(_channel, control, value, status, group) {
    const fx = PioneerDDJFLX6.padFx[control];
    if (fx === undefined) {
        return;
    }
    const deck = PioneerDDJFLX6.deckIndex(group) + 1;
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

PioneerDDJFLX6.samplerPadPressed = function(_channel, _control, value, status, mapped) {
    const group = PioneerDDJFLX6.samplerSlot(mapped, status);
    if (engine.getValue(group, "track_loaded")) {
        engine.setValue(group, "cue_gotoandplay", value);
    } else {
        engine.setValue(group, "LoadSelectedTrack", value);
    }
};

PioneerDDJFLX6.samplerPadShiftPressed = function(_channel, _control, value, status, mapped) {
    const group = PioneerDDJFLX6.samplerSlot(mapped, status);
    if (engine.getValue(group, "play")) {
        engine.setValue(group, "cue_gotoandstop", value);
    } else if (engine.getValue(group, "track_loaded")) {
        engine.setValue(group, "eject", value);
    }
};

PioneerDDJFLX6.startSamplerBlink = function(channel, control, group) {
    let val = 0x7f;

    PioneerDDJFLX6.stopSamplerBlink(channel, control);
    PioneerDDJFLX6.timers[channel][control] = engine.beginTimer(250, () => {
        val = 0x7f - val;

        // blink the appropriate pad
        midi.sendShortMsg(channel, control, val);
        // also blink the pad while SHIFT is pressed
        midi.sendShortMsg((channel+1), control, val);

        const isPlaying = engine.getValue(group, "play") === 1;

        if (!isPlaying) {
            // kill timer
            PioneerDDJFLX6.stopSamplerBlink(channel, control);
            // set the pad LED to ON
            midi.sendShortMsg(channel, control, 0x7f);
            // set the pad LED to ON while SHIFT is pressed
            midi.sendShortMsg((channel+1), control, 0x7f);
        }
    });
};

PioneerDDJFLX6.stopSamplerBlink = function(channel, control) {
    PioneerDDJFLX6.timers[channel] = PioneerDDJFLX6.timers[channel] || {};

    if (PioneerDDJFLX6.timers[channel][control] !== undefined) {
        engine.stopTimer(PioneerDDJFLX6.timers[channel][control]);
        PioneerDDJFLX6.timers[channel][control] = undefined;
    }
};


PioneerDDJFLX6.toggleQuantize = function(_channel, _control, value, _status, group) {
    if (value) {
        script.toggleControl(group, "quantize");
    }
};

PioneerDDJFLX6.quickJumpForward = function(_channel, _control, value, _status, group) {
    if (value) {
        engine.setValue(group, "beatjump", PioneerDDJFLX6.quickJumpSize);
    }
};

PioneerDDJFLX6.quickJumpBack = function(_channel, _control, value, _status, group) {
    if (value) {
        engine.setValue(group, "beatjump", -PioneerDDJFLX6.quickJumpSize);
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
PioneerDDJFLX6.stemPads = [4, 3, 2, 0, 1, 5, -2, -1];

PioneerDDJFLX6.stemGroup = function(group, stem) {
    return `[${group.substring(1, group.length-1)}_Stem${stem}]`;
};

// Whether each of the deck's stems plays (index 1 to 5); a stem the track
// doesn't have counts as playing.
PioneerDDJFLX6.stemsOn = function(group) {
    const count = engine.getValue(group, "stem_count");
    const on = [false];
    for (let stem = 1; stem <= 5; stem++) {
        on.push(stem > count || engine.getValue(PioneerDDJFLX6.stemGroup(group, stem), "mute") <= 0.5);
    }
    return on;
};

// Whether the pad for `kind` (see stemPads) is lit.
PioneerDDJFLX6.stemPadLit = function(group, kind) {
    const count = engine.getValue(group, "stem_count");
    if (count < 4) {
        return false;
    }
    const on = PioneerDDJFLX6.stemsOn(group);
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
PioneerDDJFLX6.setStems = function(group, vocals, rest) {
    for (let stem = 1; stem <= 5; stem++) {
        const on = stem === 4 ? vocals : rest;
        engine.setValue(PioneerDDJFLX6.stemGroup(group, stem), "mute", on ? 0 : 1);
    }
};

PioneerDDJFLX6.stemMutePadPressed = function(_channel, control, value, _status, group) {
    if (value !== 0x7f) {
        return;
    }
    const kind = PioneerDDJFLX6.stemPads[control - PioneerDDJFLX6.stemMutePadsFirstControl];
    const count = engine.getValue(group, "stem_count");
    if (count < 4 || kind === undefined || kind > count) {
        return;
    }
    if (kind > 0) {
        const stemGroup = PioneerDDJFLX6.stemGroup(group, kind);
        engine.setValue(stemGroup, "mute", engine.getValue(stemGroup, "mute") ? 0 : 1);
    } else if (kind === 0) {
        PioneerDDJFLX6.setStems(group, true, PioneerDDJFLX6.stemPadLit(group, 0));
    } else if (kind === -1) {
        PioneerDDJFLX6.setStems(group, PioneerDDJFLX6.stemPadLit(group, -1), true);
    }
};

// SHIFT + a stem's pad plays that stem alone.
PioneerDDJFLX6.stemMutePadShiftPressed = function(_channel, control, value, _status, group) {
    if (value !== 0x7f) {
        return;
    }
    const kind = PioneerDDJFLX6.stemPads[control - PioneerDDJFLX6.stemMutePadsFirstControl];
    const count = engine.getValue(group, "stem_count");
    if (count < 4 || kind === undefined || kind <= 0 || kind > count) {
        PioneerDDJFLX6.stemMutePadPressed(_channel, control, value, _status, group);
        return;
    }
    for (let stem = 1; stem <= count; stem++) {
        engine.setValue(PioneerDDJFLX6.stemGroup(group, stem), "mute", stem === kind ? 0 : 1);
    }
};

PioneerDDJFLX6.stemCountChanged = function(_value, group, _control) {
    const channel = group.match(/\[Channel\d+/)[0] + "]";
    if (!PioneerDDJFLX6.keyboardStems[channel]) {
        return;
    }
    for (let pad = 0; pad < 8; pad++) {
        const lit = PioneerDDJFLX6.stemPadLit(channel, PioneerDDJFLX6.stemPads[pad]);
        PioneerDDJFLX6.stemsPadsModesStatus[channel].forEach(function(status) {
            midi.sendShortMsg(status, PioneerDDJFLX6.stemMutePadsFirstControl + pad, lit ? 0x7F : 0x00);
        });
    }
};

PioneerDDJFLX6.stemMuteChanged = PioneerDDJFLX6.stemCountChanged;

//
// Pitch Shift mode
//

PioneerDDJFLX6.pitchAdjusted = function(_value, group, _control) {
    // DJ Sugar: the KEYBOARD pads show the key shift too.
    PioneerDDJFLX6.keyboardLights(group);
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

        PioneerDDJFLX6.pitchPadsModesStatus[group].forEach(
            (padMode) => midi.sendShortMsg(
                padMode,
                PioneerDDJFLX6.pitchPadsFirstControl + i,
                code,
            )
        );
    }
};

PioneerDDJFLX6.pitchPadPressed = function(_channel, control, value, _status, group) {
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

PioneerDDJFLX6.pitchPadShiftPressed = function(_channel, control, value, _status, group) {
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

PioneerDDJFLX6.shutdown = function() {
    engine.stopTimer(PioneerDDJFLX6.jogRingTimer);

    PioneerDDJFLX6.decks.forEach(function(group, i) {
        const status = PioneerDDJFLX6.deckStatus(group);
        const pads = PioneerDDJFLX6.padStatus(group);

        // reset vumeter and jog ring
        midi.sendShortMsg(0xB0 + i, 0x02, 0x00);
        midi.sendShortMsg(0xBB, i, 0x00);

        // turn off all pad LEDs
        for (let pad = 0; pad <= 0x7F; ++pad) {
            midi.sendShortMsg(pads, pad, 0x00);
            midi.sendShortMsg(pads + 1, pad, 0x00);
        }

        // turn off loop in and out lights and reloop lights
        PioneerDDJFLX6.setLoopButtonLights(status, 0x00);
        PioneerDDJFLX6.setReloopLight(status, 0x00);
    });
    midi.sendShortMsg(0xB4, 0x10, 0x00);
    midi.sendShortMsg(0xB5, 0x10, 0x00);
};
