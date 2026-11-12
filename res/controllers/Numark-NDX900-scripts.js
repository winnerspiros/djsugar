// Numark NDX900 (and NDX800) in USB MIDI mode. Experimental: Numark
// publishes no MIDI table; the messages are those of its Traktor mapping.
// The unit always sends on channel 1, so the deck is the "Deck" setting: with
// two units, choose Deck 1 for one and Deck 2 for the other.

// eslint-disable-next-line no-var
var NumarkNDX900 = {
    searching: false,
    scratchMode: false,
    scratching: false,
    // Notes of the buttons: [control, how]
    buttons: {
        0x30: ["loop_in", "hold"],
        0x31: ["loop_out", "hold"],
        0x32: ["reloop_toggle", "press"],
        0x33: ["beatloop_1_toggle", "press"],
        0x34: ["beatloop_4_toggle", "press"],
        0x35: ["beatloop_8_toggle", "press"],
        0x38: ["cue_default", "hold"],
        0x39: ["play", "toggle"],
        0x3A: ["pause", "special"],
        0x3B: ["keylock", "toggle"],
        0x3C: ["rate_temp_down", "hold"],
        0x3D: ["rate_temp_up", "hold"],
        0x3E: ["loop_halve", "press"],
        0x3F: ["loop_double", "press"],
        0x40: ["start", "press"],
        0x41: ["search", "special"],
        0x42: ["scratch", "special"],
        0x46: ["LoadSelectedTrack", "press"],
        0x51: ["sync_enabled", "toggle"],
        0x59: ["touch", "special"],
    },
};

NumarkNDX900.init = function() {};
NumarkNDX900.shutdown = function() {};

NumarkNDX900.deck = function() {
    return parseInt(engine.getSetting("deck"), 10) || 1;
};

NumarkNDX900.button = function(channel, control, value) {
    const entry = NumarkNDX900.buttons[control];
    if (!entry) {
        return;
    }
    const deck = NumarkNDX900.deck();
    const group = `[Channel${deck}]`;
    const pressed = value > 0;
    switch (entry[1]) {
    case "toggle":
        if (pressed) {
            script.toggleControl(group, entry[0]);
        }
        break;
    case "hold":
        engine.setValue(group, entry[0], pressed ? 1 : 0);
        break;
    case "press":
        if (pressed) {
            engine.setValue(group, entry[0], 1);
        }
        break;
    case "special":
        if (entry[0] === "pause" && pressed) {
            engine.setValue(group, "play", 0);
        } else if (entry[0] === "search" && pressed) {
            NumarkNDX900.searching = !NumarkNDX900.searching;
        } else if (entry[0] === "scratch" && pressed) {
            NumarkNDX900.scratchMode = !NumarkNDX900.scratchMode;
        } else if (entry[0] === "touch") {
            if (pressed && NumarkNDX900.scratchMode) {
                engine.scratchEnable(deck, 512, 33 + 1 / 3, 1 / 8, 1 / 8 / 32, true);
                NumarkNDX900.scratching = true;
            } else if (!pressed && NumarkNDX900.scratching) {
                engine.scratchDisable(deck, true);
                NumarkNDX900.scratching = false;
            }
        }
        break;
    }
};

/// The jog wheel (CC 0x20): relative, 0x40 is still
NumarkNDX900.jog = function(channel, control, value) {
    const deck = NumarkNDX900.deck();
    const group = `[Channel${deck}]`;
    const delta = value - 0x40;
    if (NumarkNDX900.scratching) {
        engine.scratchTick(deck, delta);
    } else if (NumarkNDX900.searching) {
        const position = engine.getValue(group, "playposition");
        engine.setValue(group, "playposition", Math.max(0, Math.min(1, position + delta * 0.001)));
    } else {
        engine.setValue(group, "jog", delta / 2);
    }
};

/// The track knob (CC 0x21): 0x01 forward, 0x7F back
NumarkNDX900.browse = function(channel, control, value) {
    engine.setValue("[Library]", "MoveVertical", value < 0x40 ? value : value - 0x80);
};

/// The pitch fader: 14 bit pitch bend
NumarkNDX900.pitch = function(channel, control, value, status) {
    // Mixxx passes the low 7 bits as control and the high 7 bits as value
    const position = (value << 7) | control;
    const group = `[Channel${NumarkNDX900.deck()}]`;
    engine.setValue(group, "rate", Math.max(-1, Math.min(1, (position - 8192) / 8192)));
};
