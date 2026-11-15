// Pioneer-DDJ-800-script.js
// ****************************************************************************
// * DJ Sugar mapping script for the Pioneer DDJ-800: what it adds to DJ Sugar's
// * DDJ-FLX4 script, which drives the rest (the DDJ-800 numbers its decks,
// * pads, mixer and browser as the DDJ-400 and DDJ-FLX4 do). Written from
// * Pioneer's "DDJ-800 MIDI Message List".
// ****************************************************************************
//
//  Added here: QUANTIZE (in the XML), MASTER TEMPO, KEY SYNC, SEARCH, the
//  Beat FX CH SELECT and FX SELECT switches, and the jog displays (BPM,
//  time and the playhead going round).
//
//  Not mapped yet: MEMORY, DELETE, the pad PAGE buttons, the COLOR FX
//  buttons (the COLOR FX knobs are filters), TEMPO RANGE with SHIFT.

var PioneerDDJ800 = {};

PioneerDDJ800.decks = ["[Channel1]", "[Channel2]"];
PioneerDDJ800.beatFxUnit = "[EffectRack1_EffectUnit5]";
PioneerDDJ800.beatFxSlot = "[EffectRack1_EffectUnit5_Effect1]";

// FX SELECT positions (notes 0x20 to 0x2D) and the closest of DJ Sugar's
// effects, as `loaded_effect` numbers: 1 Echo, 2 Reverb, 4 Flanger,
// 6 Phaser, 7 Gate, 8 Roll, 12 Spiral, 14 Chorus, 15 Ladder, 18 Filter Echo.
PioneerDDJ800.beatFxEffects = {
    0x20: 18, // LOW CUT ECHO
    0x21: 1, // ECHO
    0x22: 1, // MT DELAY
    0x23: 12, // SPIRAL
    0x24: 2, // REVERB
    0x25: 7, // TRANS
    0x26: 4, // ENIGMA JET
    0x27: 4, // FLANGER
    0x28: 6, // PHASER
    0x29: 14, // PITCH
    0x2A: 8, // SLIP ROLL
    0x2B: 8, // ROLL
    0x2C: 15, // MOBIUS SAW
    0x2D: 15, // MOBIUS TRI
};

// CH SELECT positions and the group each sends the Beat FX to.
PioneerDDJ800.beatFxChannels = {
    0x1C: "[Channel1]",
    0x1D: "[Channel2]",
    0x1E: "[Channel3]",
    0x1F: "[Channel4]",
    0x14: "[Master]",
    0x16: "[Master]",
};

PioneerDDJ800.connections = [];
PioneerDDJ800.searchTimers = {};

PioneerDDJ800.deckIndex = function(group) {
    return parseInt(group.match(script.channelRegEx)[1]) - 1;
};

PioneerDDJ800.init = function() {
    PioneerDDJ800.decks.forEach(function(group) {
        const status = 0x90 + PioneerDDJ800.deckIndex(group);
        const light = function(note) {
            return function(value) {
                midi.sendShortMsg(status, note, value ? 0x7F : 0x00);
            };
        };
        [["quantize", 0x35], ["keylock", 0x1A]].forEach(function(c) {
            const conn = engine.makeConnection(group, c[0], light(c[1]));
            conn.trigger();
            PioneerDDJ800.connections.push(conn);
        });
        // Jog display: show it; a timer keeps BPM, time and position current.
        midi.sendShortMsg(status, 0x5D, 0x00);
        PioneerDDJ800.updateJogDisplay(group);
    });
    PioneerDDJ800.jogTimer = engine.beginTimer(50, function() {
        PioneerDDJ800.decks.forEach(PioneerDDJ800.updateJogDisplay);
    });
};

PioneerDDJ800.shutdown = function() {
    PioneerDDJ800.connections.forEach(function(c) { c.disconnect(); });
    if (PioneerDDJ800.jogTimer !== undefined) {
        engine.stopTimer(PioneerDDJ800.jogTimer);
    }
    Object.keys(PioneerDDJ800.searchTimers).forEach(function(group) {
        engine.stopTimer(PioneerDDJ800.searchTimers[group]);
    });
    PioneerDDJ800.decks.forEach(function(group) {
        const status = 0x90 + PioneerDDJ800.deckIndex(group);
        midi.sendShortMsg(status, 0x35, 0x00);
        midi.sendShortMsg(status, 0x1A, 0x00);
        midi.sendShortMsg(status, 0x5D, 0x7F);
    });
};

// Sends a 14-bit value as MSB on `cc` and LSB on `cc + 0x20`.
PioneerDDJ800.send14 = function(status, cc, value) {
    const v = Math.max(0, Math.min(0x3FFF, Math.round(value)));
    midi.sendShortMsg(status, cc, v >> 7);
    midi.sendShortMsg(status, cc + 0x20, v & 0x7F);
};

PioneerDDJ800.updateJogDisplay = function(group) {
    const deck = PioneerDDJ800.deckIndex(group);
    const cc = 0xB0 + deck;
    const note = 0x90 + deck;
    const duration = engine.getValue(group, "duration");
    const seconds = engine.getValue(group, "playposition") * duration;
    // BPM x 10 (0 to 999.9).
    PioneerDDJ800.send14(cc, 0x15, engine.getValue(group, "bpm") * 10);
    // Playhead angle: one turn every 1.8 s, as a record at 33 1/3.
    const angle = duration > 0 ? (seconds / 1.8 * 360) % 360 : 0;
    PioneerDDJ800.send14(cc, 0x14, angle);
    const t = Math.max(0, Math.floor(seconds));
    midi.sendShortMsg(note, 0x42, Math.min(99, Math.floor(t / 60)));
    midi.sendShortMsg(note, 0x43, t % 60);
};

PioneerDDJ800.keylock = function(_channel, _control, value, _status, group) {
    if (value) {
        script.toggleControl(group, "keylock");
    }
};

PioneerDDJ800.keySync = function(_channel, _control, value, _status, group) {
    if (value) {
        engine.setValue(group, "sync_key", 1);
    }
};

PioneerDDJ800.keyReset = function(_channel, _control, value, _status, group) {
    if (value) {
        engine.setValue(group, "reset_key", 1);
    }
};

// SEARCH: a beat each press, then a beat every 100 ms while held.
PioneerDDJ800.search = function(group, value, beats) {
    if (PioneerDDJ800.searchTimers[group] !== undefined) {
        engine.stopTimer(PioneerDDJ800.searchTimers[group]);
        delete PioneerDDJ800.searchTimers[group];
    }
    if (value) {
        engine.setValue(group, "beatjump", beats);
        PioneerDDJ800.searchTimers[group] = engine.beginTimer(100, function() {
            engine.setValue(group, "beatjump", beats);
        });
    }
};

PioneerDDJ800.searchBack = function(_channel, _control, value, _status, group) {
    PioneerDDJ800.search(group, value, -1);
};

PioneerDDJ800.searchForward = function(_channel, _control, value, _status, group) {
    PioneerDDJ800.search(group, value, 1);
};

// CH SELECT is a rotary switch: the new position sends note on.
PioneerDDJ800.beatFxChannel = function(_channel, control, value) {
    if (!value) {
        return;
    }
    const target = PioneerDDJ800.beatFxChannels[control];
    ["[Channel1]", "[Channel2]", "[Channel3]", "[Channel4]", "[Master]"].forEach(function(g) {
        engine.setValue(PioneerDDJ800.beatFxUnit, "group_" + g + "_enable", g === target ? 1 : 0);
    });
};

PioneerDDJ800.beatFxSelect = function(_channel, control, value) {
    if (!value) {
        return;
    }
    const effect = PioneerDDJ800.beatFxEffects[control];
    if (effect !== undefined) {
        engine.setValue(PioneerDDJ800.beatFxSlot, "loaded_effect", effect);
    }
};
