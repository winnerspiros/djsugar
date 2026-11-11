// The FX Station (Android) from 8 pads and 8 knobs: Akai LPD8 (MK1 and
// MK2) and MPK mini (MK2 and MK3) with their factory settings. The pads of
// the LPD8 send the notes 36 to 43, those of the MPK mini 3 a drum layout
// (see mpkNotes), or the programs 0 to 7 in PROG CHANGE mode. The knobs send
// CC 1 to 8 (LPD8 MK1, MPK mini MK2) or CC 70 to 77 (LPD8 MK2, MPK mini
// MK3), on any MIDI channel.
//
// Pads, top row (5-8): the X-Pad instrument, kick, snare, clap and hi-hat.
//   A pad selects it and plays it once (as hard as hit); held, it rolls at
//   1/1 (a hit per beat).
// Pads, bottom row (1-4): the X-Pad roll of the selected instrument, 1/2,
//   1/4, 1/8 and 1/16 of a beat, while held (in PROG CHANGE mode, where a
//   pad can't be held, a press starts it and another press stops it).
// Knobs:
//   1  Scene amount (at 0 the scene is off)
//   2  Scene: time (or tone)      3  Scene: feedback (or size)
//   4  Scene: which one (5 Build Up, then 5 Break Down, as on the screen)
//   5-7  Isolate: low, mid, high (middle = off)
//   8  X-Pad level

// eslint-disable-next-line no-var
var AkaiFxStation = {};

AkaiFxStation.group = "[FxStation]";
AkaiFxStation.firstNote = 36;
AkaiFxStation.sceneTypes = 10;
// Knobs within this of the middle are at 0 for bipolar controls
AkaiFxStation.deadZone = 2;
// A top pad held longer than this rolls at 1/1
AkaiFxStation.holdMs = 300;

AkaiFxStation.init = function() {
    // The roll started by a press in PROG CHANGE mode (0: none)
    AkaiFxStation.latchedRoll = 0;
    // The timer of the held top pad, and whether it rolls
    AkaiFxStation.holdTimer = 0;
    AkaiFxStation.holdRolling = false;
};

AkaiFxStation.shutdown = function() {
    // Nothing keeps rolling without the controller
    AkaiFxStation.stopRolls();
};

AkaiFxStation.stopRolls = function() {
    for (let roll = 1; roll <= 5; roll++) {
        engine.setValue(AkaiFxStation.group, "xpad_roll_" + roll, 0);
    }
};

/// A top pad: select the instrument and play it once; held, roll at 1/1
AkaiFxStation.instrument = function(instrument, pressed, velocity, canHold) {
    const group = AkaiFxStation.group;
    const hit = "xpad_hit_" + (instrument + 1);
    if (AkaiFxStation.holdTimer) {
        engine.stopTimer(AkaiFxStation.holdTimer);
        AkaiFxStation.holdTimer = 0;
    }
    if (AkaiFxStation.holdRolling) {
        AkaiFxStation.holdRolling = false;
        engine.setValue(group, "xpad_roll_1", 0);
    }
    if (!pressed) {
        engine.setValue(group, hit, 0);
        return;
    }
    if (canHold) {
        AkaiFxStation.holdTimer = engine.beginTimer(AkaiFxStation.holdMs, () => {
            AkaiFxStation.holdTimer = 0;
            AkaiFxStation.holdRolling = true;
            engine.setValue(group, "xpad_instrument", instrument);
            engine.setValue(group, "xpad_roll_1", 1);
        }, true);
    }
    engine.setValue(group, "xpad_instrument", instrument);
    // Soft hits still clearly heard over the music
    engine.setValue(group, hit, 0.5 + 0.5 * velocity / 127);
};

// The pad (0 to 7, bottom row first) of each note of the MPK mini 3, banks
// A and B
AkaiFxStation.mpkNotes = {
    37: 0, 36: 1, 42: 2, 54: 3, 40: 4, 38: 5, 46: 6, 44: 7,
    49: 0, 55: 1, 51: 2, 53: 3, 48: 4, 47: 5, 45: 6, 43: 7,
};

/// The pads of the LPD8: notes 36 to 43
AkaiFxStation.padLpd8 = function(channel, control, value, status) {
    const program = (status & 0xF0) === 0xC0;
    AkaiFxStation.pad(program ? control : control - AkaiFxStation.firstNote, value, status);
};

/// The pads of the MPK mini: its drum layout
AkaiFxStation.padMpk = function(channel, control, value, status) {
    const program = (status & 0xF0) === 0xC0;
    const pad = program ? control : AkaiFxStation.mpkNotes[control];
    if (pad !== undefined) {
        AkaiFxStation.pad(pad, value, status);
    }
};

AkaiFxStation.pad = function(pad, value, status) {
    const group = AkaiFxStation.group;
    const program = (status & 0xF0) === 0xC0;
    if (pad < 0 || pad > 7) {
        return;
    }
    if (pad >= 4) {
        if (program) {
            // No release: a short hit
            AkaiFxStation.instrument(pad - 4, true, 127, false);
            engine.beginTimer(30, () => AkaiFxStation.instrument(pad - 4, false, 0, false), true);
        } else {
            AkaiFxStation.instrument(pad - 4, (status & 0xF0) === 0x90 && value > 0, value, true);
        }
        return;
    }
    // 1/2 (xpad_roll_2) to 1/16 (xpad_roll_5)
    const roll = pad + 2;
    if (program) {
        const stop = AkaiFxStation.latchedRoll === roll;
        AkaiFxStation.stopRolls();
        AkaiFxStation.latchedRoll = stop ? 0 : roll;
        if (!stop) {
            engine.setValue(group, "xpad_roll_" + roll, 1);
        }
        return;
    }
    engine.setValue(group, "xpad_roll_" + roll, (status & 0xF0) === 0x90 && value > 0 ? 1 : 0);
};

/// A bipolar control from a knob: -1 to 1, 0 around the middle
AkaiFxStation.bipolar = function(value) {
    if (Math.abs(value - 64) <= AkaiFxStation.deadZone) {
        return 0;
    }
    return value < 64 ? (value - 64) / 64 : (value - 64) / 63;
};

AkaiFxStation.knob = function(channel, control, value) {
    const group = AkaiFxStation.group;
    // CC 1-8 or CC 70-77
    const knob = control >= 70 ? control - 70 : control - 1;
    switch (knob) {
    case 0:
        engine.setValue(group, "scene_dial", value / 127);
        engine.setValue(group, "scene_on", value > 0 ? 1 : 0);
        break;
    case 1:
        engine.setValue(group, "scene_param1", value / 127);
        break;
    case 2:
        engine.setValue(group, "scene_param2", value / 127);
        break;
    case 3:
        engine.setValue(group, "scene_type", Math.min(AkaiFxStation.sceneTypes - 1,
                Math.floor(value * AkaiFxStation.sceneTypes / 128)));
        break;
    case 4:
        engine.setValue(group, "isolate_low", AkaiFxStation.bipolar(value));
        break;
    case 5:
        engine.setValue(group, "isolate_mid", AkaiFxStation.bipolar(value));
        break;
    case 6:
        engine.setValue(group, "isolate_high", AkaiFxStation.bipolar(value));
        break;
    case 7:
        engine.setValue(group, "xpad_volume", value / 127);
        break;
    }
};
